//! Captured reference XML is the oracle; no external executable runs during cargo test.
use bc_engine_dir::{
    filter::{FilterClock, FolderDisplay},
    filter_script::FilterSettings,
    jobs::ContentCompareJob,
    refresh,
    view::ViewOptions,
    CompareStatus, Criteria, DirTree,
};
use bc_vfs::{FileAttrs, LocalFs, Vfs, VfsError, VfsManager, VfsPath};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Default)]
struct Xml {
    name: String,
    attrs: BTreeMap<String, String>,
    text: String,
    children: Vec<Xml>,
}
impl Xml {
    fn child(&self, name: &str) -> Option<&Self> {
        self.children.iter().find(|n| n.name == name)
    }
    fn value(&self, name: &str) -> Option<&str> {
        self.child(name).map(|n| n.text.trim())
    }
    fn attr(&self, name: &str) -> &str {
        self.attrs.get(name).map_or("", String::as_str)
    }
}
fn unescape(text: &str) -> Result<String, String> {
    let mut result = String::new();
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        result.push_str(&rest[..at]);
        let end = rest[at..].find(';').ok_or("unterminated entity")? + at;
        let entity = &rest[at + 1..end];
        let c = match entity {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            _ if entity.starts_with("#x") => char::from_u32(
                u32::from_str_radix(&entity[2..], 16).map_err(|_| "invalid hex entity")?,
            )
            .ok_or("invalid entity scalar")?,
            _ if entity.starts_with('#') => {
                char::from_u32(entity[1..].parse().map_err(|_| "invalid decimal entity")?)
                    .ok_or("invalid entity scalar")?
            }
            _ => return Err(format!("unknown entity {entity}")),
        };
        result.push(c);
        rest = &rest[end + 1..];
    }
    result.push_str(rest);
    Ok(result)
}
/// Strict, bounded parser for the small captured-report dialect. No DTD/entity expansion.
fn parse_xml(input: &str) -> Result<Xml, String> {
    if input.len() > 8 * 1024 * 1024 {
        return Err("fixture XML exceeds size bound".into());
    }
    let mut stack = vec![Xml::default()];
    let mut at = 0;
    while at < input.len() {
        let Some(start) = input[at..].find('<').map(|i| at + i) else {
            stack
                .last_mut()
                .unwrap()
                .text
                .push_str(&unescape(&input[at..])?);
            break;
        };
        stack
            .last_mut()
            .unwrap()
            .text
            .push_str(&unescape(&input[at..start])?);
        if input[start..].starts_with("<!--") {
            at = input[start + 4..]
                .find("-->")
                .ok_or("unterminated comment")?
                + start
                + 7;
            continue;
        }
        if input[start..].starts_with("<?") {
            at = input[start + 2..]
                .find("?>")
                .ok_or("unterminated declaration")?
                + start
                + 4;
            continue;
        }
        if input[start..].starts_with("<!") {
            return Err("DTD/CDATA not accepted in report fixtures".into());
        }
        let end = input[start..].find('>').ok_or("unterminated tag")? + start;
        let tag = input[start + 1..end].trim();
        if let Some(name) = tag.strip_prefix('/') {
            if stack.len() < 2 {
                return Err("unmatched close".into());
            }
            let node = stack.pop().unwrap();
            if node.name != name {
                return Err("mismatched close".into());
            }
            stack.last_mut().unwrap().children.push(node);
        } else {
            let empty = tag.ends_with('/');
            let tag = tag.trim_end_matches('/').trim();
            let name_end = tag.find(char::is_whitespace).unwrap_or(tag.len());
            let mut node = Xml {
                name: tag[..name_end].into(),
                ..Default::default()
            };
            if node.name.is_empty() {
                return Err("empty tag".into());
            }
            let mut attrs = tag[name_end..].trim();
            while !attrs.is_empty() {
                let eq = attrs.find('=').ok_or("attribute missing equals")?;
                let name = attrs[..eq].trim();
                let value = attrs[eq + 1..].trim_start();
                let quote = value.chars().next().ok_or("attribute missing value")?;
                if quote != '"' && quote != '\'' {
                    return Err("unquoted attribute".into());
                }
                let close = value[1..].find(quote).ok_or("unterminated attribute")? + 1;
                if node
                    .attrs
                    .insert(name.into(), unescape(&value[1..close])?)
                    .is_some()
                {
                    return Err("duplicate attribute".into());
                }
                attrs = value[close + 1..].trim_start();
            }
            if empty {
                stack.last_mut().unwrap().children.push(node);
            } else {
                if stack.len() > 128 {
                    return Err("XML nesting bound".into());
                }
                stack.push(node);
            }
        }
        at = end + 1;
    }
    if stack.len() != 1 {
        return Err("unclosed XML".into());
    }
    let mut root = stack.pop().unwrap();
    if root.children.len() != 1 {
        return Err("expected one root".into());
    }
    Ok(root.children.remove(0))
}
type Pair = [Option<String>; 2];
#[derive(Debug, Default)]
struct Rows {
    files: BTreeMap<Pair, String>,
    folders: BTreeSet<Pair>,
}
fn relative(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .trim_matches('/')
        .into()
}
fn append(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        relative(name)
    } else {
        format!("{}/{}", relative(parent), relative(name))
    }
}
fn report_rows(root: &Xml) -> Result<Rows, String> {
    let mut result = Rows::default();
    fn visit(node: &Xml, parents: &Pair, rows: &mut Rows) -> Result<(), String> {
        let names: Pair = std::array::from_fn(|side| {
            node.child(if side == 0 { "lt" } else { "rt" })
                .and_then(|n| n.value("name"))
                .map(str::to_owned)
        });
        let pair: Pair = std::array::from_fn(|side| {
            let name = names[side].as_deref()?;
            let item = node.child(if side == 0 { "lt" } else { "rt" })?;
            let prefix = item
                .value("path")
                .map(relative)
                .unwrap_or_else(|| parents[side].clone().unwrap_or_default());
            Some(append(&prefix, name))
        });
        if node.name == "filecomp" {
            let status = node.attr("status");
            if status.is_empty() {
                return Err("file status missing".into());
            }
            if rows.files.insert(pair, status.into()).is_some() {
                return Err("duplicate file pairing".into());
            }
        } else {
            let next = if node.name == "foldercomp" && names.iter().any(Option::is_some) {
                rows.folders.insert(pair.clone());
                pair
            } else {
                parents.clone()
            };
            for child in &node.children {
                if child.name == "filecomp" || child.name == "foldercomp" {
                    visit(child, &next, rows)?;
                }
            }
        }
        Ok(())
    }
    let container = root.child("foldercomp").ok_or("foldercomp root missing")?;
    visit(
        container,
        &[Some(String::new()), Some(String::new())],
        &mut result,
    )?;
    Ok(result)
}
fn status(status: CompareStatus) -> &'static str {
    use CompareStatus::*;
    match status {
        Unknown => "unknown",
        Same => "same",
        Different | SimilarUnimportant => "diff",
        LeftNewer => "ltnewer",
        RightNewer => "rtnewer",
        LeftOrphan => "ltonly",
        RightOrphan => "rtonly",
        Error => "error",
    }
}
struct Temp {
    root: PathBuf,
    files: Vec<PathBuf>,
}
impl Drop for Temp {
    fn drop(&mut self) {
        for p in &self.files {
            let _ = LocalFs::new().set_attrs(&VfsPath::local(p), FileAttrs::default());
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn run_case(case_dir: &Path, manifest: &Xml) -> Result<Rows, Box<dyn std::error::Error>> {
    let salt = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let mut temp = Temp {
        root: std::env::temp_dir().join(format!("matcha-golden-dir-{}-{salt}", std::process::id())),
        files: Vec::new(),
    };
    for side in ["left", "right"] {
        fs::create_dir_all(temp.root.join(side))?;
    }
    for file in &manifest.children {
        if file.name != "File" {
            return Err("unknown manifest entry".into());
        }
        let side = file.attr("side");
        if !["left", "right"].contains(&side) {
            return Err("invalid fixture side".into());
        }
        let name = Path::new(file.attr("path"));
        if name
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err("fixture path escapes its side".into());
        }
        let path = temp.root.join(side).join(name);
        fs::create_dir_all(path.parent().unwrap())?;
        fs::copy(case_dir.join(side).join(name), &path)?;
        temp.files.push(path.clone());
        let path = VfsPath::local(&path);
        LocalFs::new().set_modified(
            &path,
            UNIX_EPOCH + Duration::from_secs(file.attr("modified").parse()?),
        )?;
        LocalFs::new().set_attrs(
            &path,
            FileAttrs::parse_set(file.attr("attrs")).ok_or("bad attributes")?,
        )?;
    }
    let criteria = Criteria::parse_script(manifest.attr("criteria"))?;
    let vfs = Arc::new(VfsManager::local_only());
    let mut tree = DirTree::new();
    tree.bases = [
        Some(VfsPath::local(&temp.root.join("left"))),
        Some(VfsPath::local(&temp.root.join("right"))),
        None,
    ];
    refresh::full_refresh(&mut tree, &vfs, &|| false)?;
    tree.evaluate_quick(&criteria);
    let mut unsupported = BTreeSet::new();
    let ids: Vec<_> = tree
        .nodes()
        .filter(|(_, n)| !n.is_dir && n.sides[0].is_some() && n.sides[1].is_some())
        .map(|(id, _)| id)
        .collect();
    if criteria.needs_content() {
        for id in ids {
            let node = tree.node(id).unwrap();
            let job = ContentCompareJob {
                vfs: vfs.clone(),
                left: tree.bases[0]
                    .as_ref()
                    .unwrap()
                    .join(&tree.side_relative(id, 0)),
                right: tree.bases[1]
                    .as_ref()
                    .unwrap()
                    .join(&tree.side_relative(id, 1)),
                criteria: criteria.clone(),
                quick_status: node.quick_status,
                node: id,
                revision: node.revision,
            };
            match job.compare(&|| false) {
                Ok(result) => {
                    result.apply(&mut tree);
                }
                Err(VfsError::Unsupported(_))
                    if manifest.attr("knownDiff") == "rules-unavailable" =>
                {
                    unsupported.insert(id);
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
    let mut filters = FilterSettings::default();
    let clock = FilterClock {
        today_midnight: UNIX_EPOCH + Duration::from_secs(1_767_225_600),
        utc_offset_seconds: 0,
    };
    filters.apply_script(manifest.attr("filter"), clock)?;
    tree.apply_filters(&filters.names, &filters.other, clock, |_, _| true);
    tree.set_expanded(None, true);
    let view = ViewOptions {
        folders: if manifest.attr("flatten") == "true" {
            FolderDisplay::Flatten
        } else {
            FolderDisplay::FilesOnly
        },
        ..Default::default()
    };
    let mut rows = Rows::default();
    for row in tree.view_rows(view) {
        let pair: Pair =
            std::array::from_fn(|side| row.sides[side].map(|id| tree.side_relative(id, side)));
        if row
            .node
            .and_then(|id| tree.node(id))
            .is_some_and(|n| n.is_dir)
        {
            rows.folders.insert(pair);
        } else {
            let state = if row
                .sides
                .iter()
                .flatten()
                .any(|id| unsupported.contains(id))
            {
                "Unsupported"
            } else {
                status(row.status)
            };
            if rows.files.insert(pair, state.into()).is_some() {
                return Err("duplicate candidate pairing".into());
            }
        }
    }
    Ok(rows)
}
#[test]
fn golden_directory_statuses_and_structure() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../bc-core/tests/golden/dir");
    if !root.exists() {
        eprintln!("golden.dir: no corpus; skipped");
        return;
    }
    let mut cases: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("case.xml").exists())
        .collect();
    cases.sort();
    if cases.is_empty() {
        eprintln!("golden.dir: empty corpus; skipped");
        return;
    }
    assert!(cases.len() >= 20);
    let mut total = 0;
    let mut matches = 0;
    let mut supported_total = 0;
    let mut supported_matches = 0;
    let mut known = 0;
    let mut errors = Vec::new();
    let mut folders = 0;
    for case in &cases {
        let manifest = parse_xml(&fs::read_to_string(case.join("case.xml")).unwrap()).unwrap();
        let expected = report_rows(
            &parse_xml(&fs::read_to_string(case.join("expected.xml")).unwrap()).unwrap(),
        )
        .unwrap();
        let actual = run_case(case, &manifest).unwrap();
        let keys: BTreeSet<_> = expected
            .files
            .keys()
            .chain(actual.files.keys())
            .cloned()
            .collect();
        let exception = manifest.attr("knownDiff") == "rules-unavailable";
        for pair in keys {
            total += 1;
            let equal = expected.files.get(&pair) == actual.files.get(&pair);
            matches += usize::from(equal);
            if exception {
                if !equal {
                    known += 1;
                    println!(
                        "golden.dir.known_diff={} {:?}: {:?} -> {:?}",
                        manifest.attr("name"),
                        pair,
                        expected.files.get(&pair),
                        actual.files.get(&pair)
                    );
                }
            } else {
                supported_total += 1;
                supported_matches += usize::from(equal);
                if !equal {
                    errors.push(format!(
                        "{} {:?}: expected {:?}, actual {:?}",
                        manifest.attr("name"),
                        pair,
                        expected.files.get(&pair),
                        actual.files.get(&pair)
                    ));
                }
            }
        }
        folders += expected.folders.len();
        if expected.folders != actual.folders {
            errors.push(format!(
                "{} folder structure: expected {:?}, actual {:?}",
                manifest.attr("name"),
                expected.folders,
                actual.folders
            ));
        }
    }
    println!("golden.dir.cases={}; rows={total}; matched={matches}; overall_pct={:.6}; supported_rows={supported_total}; supported_matched={supported_matches}; supported_pct={:.6}; folder_rows={folders}; known_diffs={known}",
        cases.len(),matches as f64/total as f64*100.0,supported_matches as f64/supported_total as f64*100.0);
    assert_eq!(
        known, 1,
        "only the explicitly approved rules-based row may differ"
    );
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}
#[test]
fn captured_xml_parser_preserves_entities_and_rejects_malformed_documents() {
    let node = parse_xml(r#"<r text="a&amp;b&quot;c&#x21;"><x>z&lt;q</x></r>"#).unwrap();
    assert_eq!(node.attr("text"), "a&b\"c!");
    assert_eq!(node.value("x"), Some("z<q"));
    for bad in [
        "<r>",
        "<r></s>",
        "<!DOCTYPE r><r/>",
        "<r x='a' x='b'/>",
        "<r>&unknown;</r>",
    ] {
        assert!(parse_xml(bad).is_err());
    }
}
