//! `BCFileFormats` XML ↔ [`FileFormat`] mapping.
//!
//! Reads the embedded `BCFILEFORMATS` resource and user `BCFileFormats.xml` /
//! `.bcpkg` payloads (same schema). Shared between the library and `build.rs`.
//! The reader follows the "ignore unknown elements" compatibility rule: unknown
//! *format classes* are skipped, unknown *properties* are ignored, but an unknown
//! grammar item class is an error because dropping it would silently change highlighting.

use crate::model::{
    ConvertKind, ElementKind, FileFormat, FormatKind, GrammarItem, GrammarMatch, TableOptions,
    WeightItem, WeightPriority,
};
use bc_core::persist::{self, Node, PersistError};

/// Errors from reading or writing format XML.
#[derive(Debug)]
pub enum FormatError {
    Io(std::io::Error),
    Persist(PersistError),
    UnknownGrammarClass(String),
    UnknownElement(String),
    Missing(String),
    BadValue(String),
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormatError::Io(e) => write!(f, "io: {e}"),
            FormatError::Persist(e) => write!(f, "{e}"),
            FormatError::UnknownGrammarClass(c) => write!(f, "unknown grammar item class {c}"),
            FormatError::UnknownElement(e) => write!(f, "unknown ElementStr {e:?}"),
            FormatError::Missing(m) => write!(f, "missing {m}"),
            FormatError::BadValue(v) => write!(f, "bad value {v}"),
        }
    }
}

impl std::error::Error for FormatError {}

impl From<std::io::Error> for FormatError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<PersistError> for FormatError {
    fn from(e: PersistError) -> Self {
        FormatError::Persist(e)
    }
}

/// Root element name of the format library document.
pub const ROOT_ELEMENT: &str = "BCFileFormats";

/// Parse a whole `BCFileFormats` document.
pub fn parse_bcfileformats(xml: &str) -> Result<Vec<FileFormat>, FormatError> {
    let root = persist::parse_xml(xml)?;
    if root.name != ROOT_ELEMENT {
        return Err(FormatError::Missing(format!(
            "<{ROOT_ELEMENT}> root (found <{}>)",
            root.name
        )));
    }
    let mut formats = Vec::new();
    for child in &root.children {
        if let Some(kind) = FormatKind::from_bc_class(&child.name) {
            formats.push(format_from_node(kind, child)?);
        }
        // Unknown classes (e.g. THexFormat in user files) are skipped for read-compatibility.
    }
    Ok(formats)
}

/// Convert one `<TTextFormat Value="…">` style node.
pub fn format_from_node(kind: FormatKind, node: &Node) -> Result<FileFormat, FormatError> {
    let name = node
        .value_str()
        .ok_or_else(|| FormatError::Missing(format!("Value on <{}>", node.name)))?
        .to_string();

    let description = node
        .child("Description")
        .map(Node::item_values)
        .unwrap_or_default();

    let masks = node
        .child_value("Mask")
        .map(|m| {
            m.split(';')
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    let mut grammar = Vec::new();
    if let Some(g) = node.child("Grammar") {
        for item in g.items() {
            grammar.push(
                grammar_item_from_node(item)
                    .map_err(|e| FormatError::BadValue(format!("format {name}: {e}")))?,
            );
        }
    }

    let tab_size = node.child_int("TabSize")?.map(|v| v as u32);
    let tab_as_spaces = node.child_bool("TabAsSpaces")?.unwrap_or(false);

    let convert = match node.child_value("ConvertKind") {
        None => None,
        Some(v) => Some(
            ConvertKind::parse(v)
                .ok_or_else(|| FormatError::BadValue(format!("ConvertKind {v:?}")))?,
        ),
    };

    let mut case_significance = Vec::new();
    for kind in ElementKind::ALL {
        if let Some(v) = node.child_bool(kind.case_sig_property())? {
            case_significance.push((kind, v));
        }
    }

    let mut weights = Vec::new();
    if let Some(w) = node.child("Weights") {
        for item in w.items() {
            weights.push(
                weight_from_node(item)
                    .map_err(|e| FormatError::BadValue(format!("format {name}: {e}")))?,
            );
        }
    }

    let table = if kind == FormatKind::Table {
        Some(TableOptions {
            delimiters: node
                .child_value("DlmDelimiters")
                .map(parse_char_codes)
                .transpose()?
                .unwrap_or_default(),
            qualifier: node
                .child_value("DlmQualifier")
                .map(parse_char_code)
                .transpose()?,
        })
    } else {
        None
    };

    Ok(FileFormat {
        name,
        kind,
        description,
        masks,
        grammar,
        tab_size,
        tab_as_spaces,
        convert,
        case_significance,
        weights,
        table,
        source_xml: Some(persist::write_xml(node, None)),
    })
}

/// Convert one `<_N Value="T…TextGrammarItem">` node.
pub fn grammar_item_from_node(node: &Node) -> Result<GrammarItem, FormatError> {
    let class = node.value_str().unwrap_or("");
    if !matches!(
        class,
        "TBasicTextGrammarItem"
            | "TDelimitedTextGrammarItem"
            | "TListTextGrammarItem"
            | "TColumnTextGrammarItem"
    ) {
        return Err(FormatError::UnknownGrammarClass(class.to_string()));
    }
    let element_str = node
        .child_value("ElementStr")
        .ok_or_else(|| FormatError::Missing("ElementStr".into()))?;
    let element = ElementKind::parse(element_str)
        .ok_or_else(|| FormatError::UnknownElement(element_str.to_string()))?;
    let case_sensitive = node.child_bool("CaseSensitive")?.unwrap_or(true);
    let is_regexp = node.child_bool("IsRegExp")?.unwrap_or(false);
    let text = || node.child_value("Text").unwrap_or("").to_string();

    let matcher = match class {
        "TBasicTextGrammarItem" => GrammarMatch::Basic {
            text: text(),
            is_regexp,
        },
        "TDelimitedTextGrammarItem" => GrammarMatch::Delimited {
            start: text(),
            is_regexp,
            stop: node.child_value("StopText").map(str::to_string),
            stop_eol: node.child_bool("StopEOL")?.unwrap_or(false),
            escape: node
                .child_value("Escape")
                .map(parse_char_code)
                .transpose()?,
        },
        "TListTextGrammarItem" => GrammarMatch::List {
            items: node
                .child("Items")
                .map(Node::item_values)
                .unwrap_or_default(),
            is_regexp,
        },
        "TColumnTextGrammarItem" => GrammarMatch::Columns {
            start_col: node.child_int("Col")?.unwrap_or(1) as u32,
            stop_col: node.child_int("StopCol")?.unwrap_or(0) as u32,
        },
        other => return Err(FormatError::UnknownGrammarClass(other.to_string())),
    };

    Ok(GrammarItem {
        element,
        case_sensitive,
        matcher,
    })
}

fn weight_from_node(node: &Node) -> Result<WeightItem, FormatError> {
    let priority = node.child_value("Priority").unwrap_or("wp1");
    let level = priority
        .strip_prefix("wp")
        .and_then(|n| n.parse::<u8>().ok())
        .ok_or_else(|| FormatError::BadValue(format!("Priority {priority:?}")))?;
    Ok(WeightItem {
        find_text: node.child_value("FindText").unwrap_or("").to_string(),
        is_regexp: node.child_bool("IsRegExp")?.unwrap_or(false),
        priority: WeightPriority(level),
    })
}

fn parse_char_code(s: &str) -> Result<char, FormatError> {
    s.trim()
        .parse::<u32>()
        .ok()
        .and_then(char::from_u32)
        .ok_or_else(|| FormatError::BadValue(format!("char code {s:?}")))
}

fn parse_char_codes(s: &str) -> Result<Vec<char>, FormatError> {
    s.split(',')
        .filter(|p| !p.trim().is_empty())
        .map(parse_char_code)
        .collect()
}

/// Serialise formats back to a `BCFileFormats` document (write-compatibility target,
/// spec §5). Depends on `bc_core::persist::write_xml`.
pub fn write_bcfileformats(formats: &[FileFormat]) -> Result<String, FormatError> {
    let mut root = Node::new(ROOT_ELEMENT);
    for format in formats {
        let current = canonical_node(format)?;
        let node = if let Some(source) = &format.source_xml {
            let mut raw = persist::parse_xml(source)?;
            let kind = FormatKind::from_bc_class(&raw.name).ok_or_else(|| {
                FormatError::BadValue(format!(
                    "format {}: invalid source class {}",
                    format.name, raw.name
                ))
            })?;
            let previous = canonical_node(&format_from_node(kind, &raw)?)?;
            patch_node(&mut raw, Some(&previous), &current);
            // Resource names carry STX lookup markers, newly authored user names do not.
            if previous.value != current.value {
                raw.stx = false;
            }
            raw
        } else {
            current
        };
        root.children.push(node);
    }
    Ok(persist::write_xml(&root, None))
}

fn value(name: &str, value: impl Into<String>, stx: bool) -> Node {
    Node {
        name: name.into(),
        value: Some(value.into()),
        stx,
        children: Vec::new(),
    }
}

fn flag(node: &mut Node, name: &str, enabled: bool) {
    if enabled {
        node.children.push(value(name, "True", false));
    }
}

/// Canonical values for newly authored properties. Existing source-tree markers,
/// explicit defaults, child order and unknown properties are preserved by patch_node.
fn canonical_node(format: &FileFormat) -> Result<Node, FormatError> {
    let mut root = value(format.kind.bc_class(), &format.name, false);
    if !format.description.is_empty() {
        let mut description = Node::new("Description");
        for (i, text) in format.description.iter().enumerate() {
            description
                .children
                .push(value(&format!("_{i}"), text, true));
        }
        root.children.push(description);
    }
    if !format.grammar.is_empty() {
        let mut grammar = Node::new("Grammar");
        for (i, item) in format.grammar.iter().enumerate() {
            let mut node = value(&format!("_{i}"), item.matcher.bc_class(), false);
            node.children
                .push(value("ElementStr", item.element.as_str(), true));
            if !item.case_sensitive {
                node.children.push(value("CaseSensitive", "False", false));
            }
            match &item.matcher {
                GrammarMatch::Basic { text, is_regexp } => {
                    flag(&mut node, "IsRegExp", *is_regexp);
                    node.children.push(value("Text", text, false));
                }
                GrammarMatch::Delimited {
                    start,
                    stop,
                    is_regexp,
                    stop_eol,
                    escape,
                } => {
                    flag(&mut node, "IsRegExp", *is_regexp);
                    node.children.push(value("Text", start, false));
                    if let Some(escape) = escape {
                        node.children
                            .push(value("Escape", (*escape as u32).to_string(), false));
                    }
                    if let Some(stop) = stop {
                        node.children.push(value("StopText", stop, false));
                    }
                    flag(&mut node, "StopEOL", *stop_eol);
                }
                GrammarMatch::List { items, is_regexp } => {
                    flag(&mut node, "IsRegExp", *is_regexp);
                    let mut list = Node::new("Items");
                    for (j, text) in items.iter().enumerate() {
                        list.children.push(value(&format!("_{j}"), text, false));
                    }
                    node.children.push(list);
                }
                GrammarMatch::Columns {
                    start_col,
                    stop_col,
                } => {
                    if *start_col == 0 || stop_col < start_col {
                        return Err(FormatError::BadValue(format!(
                            "format {}: invalid columns",
                            format.name
                        )));
                    }
                    node.children
                        .push(value("Col", start_col.to_string(), false));
                    node.children
                        .push(value("StopCol", stop_col.to_string(), false));
                }
            }
            grammar.children.push(node);
        }
        root.children.push(grammar);
    }
    if !format.masks.is_empty() {
        root.children
            .push(value("Mask", format.masks.join(";"), false));
    }
    if let Some(size) = format.tab_size {
        root.children
            .push(value("TabSize", size.to_string(), false));
    }
    flag(&mut root, "TabAsSpaces", format.tab_as_spaces);
    if let Some(convert) = format.convert {
        root.children
            .push(value("ConvertKind", convert.bc_name(), false));
    }
    for &(element, significant) in &format.case_significance {
        root.children.push(value(
            element.case_sig_property(),
            if significant { "True" } else { "False" },
            false,
        ));
    }
    if !format.weights.is_empty() {
        let mut weights = Node::new("Weights");
        for (i, item) in format.weights.iter().enumerate() {
            let mut node = Node::new(format!("_{i}"));
            node.children
                .push(value("FindText", &item.find_text, false));
            flag(&mut node, "IsRegExp", item.is_regexp);
            node.children
                .push(value("Priority", format!("wp{}", item.priority.0), false));
            weights.children.push(node);
        }
        root.children.push(weights);
    }
    if let Some(table) = &format.table {
        if format.kind != FormatKind::Table {
            return Err(FormatError::BadValue(format!(
                "format {}: table options on non-table format",
                format.name
            )));
        }
        root.children.push(value(
            "DlmDelimiters",
            table
                .delimiters
                .iter()
                .map(|c| (*c as u32).to_string())
                .collect::<Vec<_>>()
                .join(","),
            false,
        ));
        if let Some(qualifier) = table.qualifier {
            root.children
                .push(value("DlmQualifier", (qualifier as u32).to_string(), false));
        }
    } else if format.kind == FormatKind::Table {
        return Err(FormatError::Missing(format!(
            "format {}: table options",
            format.name
        )));
    }
    Ok(root)
}

fn patch_node(raw: &mut Node, old: Option<&Node>, new: &Node) {
    raw.name.clone_from(&new.name);
    if old.map(|n| &n.value) != Some(&new.value) {
        if raw.value.is_none() {
            raw.stx = new.stx;
        }
        raw.value.clone_from(&new.value);
    }
    if let Some(old) = old {
        // Remove only properties previously represented by the typed model.
        raw.children
            .retain(|child| old.child(&child.name).is_none() || new.child(&child.name).is_some());
    }
    for child in &new.children {
        let previous = old.and_then(|old| old.child(&child.name));
        if previous == Some(child) {
            continue;
        }
        if let Some(existing) = raw.children.iter_mut().find(|c| c.name == child.name) {
            patch_node(existing, previous, child);
        } else {
            raw.children.push(child.clone());
        }
    }
}
