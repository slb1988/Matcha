//! Native HTML is compared verbatim; JSON only transports embedded NULs as text.
//! No colour normalization or synthesized oracle.
#[path = "support/xlsx.rs"]
mod xlsx;
use bc_core::{align::AlignStrategy, cancel::NeverCancel, DiffKind};
use bc_engine_table::{
    columns::ColumnMapping,
    compare::Importance,
    rules::{ColumnType, DateTolerance, KeySelection},
    source::{read_text, ReadOptions},
    workbook::Workbook,
    TableMap, TableRules,
};
use bc_formats::model::{DateOrder, HeaderMode};
use serde_json::Value;
use std::{io::Cursor, path::Path, sync::Arc};
#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpectedRow {
    kind: DiffKind,
    importance: Importance,
    cells: Vec<Importance>,
}
#[derive(Default)]
struct Td {
    class: String,
    text: String,
}
fn decode(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
}
fn classification(class: &str) -> Importance {
    if class.contains("Insig") {
        Importance::Unimportant
    } else if class.contains("SigDiff") || class.contains("SigOrphan") {
        Importance::Important
    } else {
        Importance::Same
    }
}
/// Quote-aware HTML tokenizer; ignores decorative image tags/CSS, not data rows.
fn parse_html(html: &str) -> Vec<Vec<ExpectedRow>> {
    let mut tables = Vec::new();
    let mut active = false;
    let mut row = Vec::<Td>::new();
    let mut cell = None::<Td>;
    let mut pos = 0;
    while pos < html.len() {
        if html.as_bytes()[pos] != b'<' {
            let end = html[pos..].find('<').map_or(html.len(), |n| pos + n);
            if let Some(c) = &mut cell {
                c.text.push_str(&decode(&html[pos..end]));
            }
            pos = end;
            continue;
        }
        let mut quote = None;
        let mut end = pos + 1;
        while end < html.len() {
            let b = html.as_bytes()[end];
            if let Some(q) = quote {
                if b == q {
                    quote = None;
                }
            } else if b == b'\'' || b == b'"' {
                quote = Some(b);
            } else if b == b'>' {
                break;
            }
            end += 1;
        }
        let tag = &html[pos + 1..end];
        let lower = tag.to_ascii_lowercase();
        if lower.starts_with("table ") && lower.contains("class=\"fc\"") {
            active = true;
            tables.push(Vec::new());
        } else if lower == "/table" {
            active = false;
        } else if active && lower.starts_with("tr") {
            row.clear();
        } else if active && lower.starts_with("td") {
            let class = tag
                .split("class=\"")
                .nth(1)
                .and_then(|s| s.split('"').next())
                .unwrap_or("");
            cell = Some(Td {
                class: class.into(),
                text: String::new(),
            });
        } else if active && lower == "/td" {
            if let Some(td) = cell.take() {
                row.push(td);
            }
        } else if active && lower.starts_with("br") {
            if let Some(c) = &mut cell {
                c.text.push('\n');
            }
        } else if active
            && lower == "/tr"
            && !row.is_empty()
            && !row[0].class.contains("TableItemNum")
        {
            if let Some(mid) = row.iter().position(|c| {
                c.class.split_whitespace().any(|c| c == "AlignCenter")
                    && !c.class.contains("TableItem")
            }) {
                let symbol = row[mid].text.trim();
                let kind = match symbol {
                    "+-" => DiffKind::LeftOnly,
                    "-+" => DiffKind::RightOnly,
                    "=" => DiffKind::Same,
                    "<>" => DiffKind::Changed,
                    _ => panic!("unknown native row marker {symbol:?}"),
                };
                let right = &row[mid + 1..];
                let width = mid.max(right.len());
                let cells = (0..width)
                    .map(|i| {
                        classification(row.get(i).map_or("", |c| &c.class))
                            .max(classification(right.get(i).map_or("", |c| &c.class)))
                    })
                    .collect::<Vec<_>>();
                let importance = if matches!(kind, DiffKind::LeftOnly | DiffKind::RightOnly) {
                    Importance::Important
                } else {
                    cells.iter().copied().max().unwrap_or_default()
                };
                tables.last_mut().unwrap().push(ExpectedRow {
                    kind,
                    importance,
                    cells,
                });
            }
        }
        pos = end.saturating_add(1);
    }
    tables
}
fn read_report(case: &Path) -> String {
    let raw = case.join("expected.html");
    let encoded = case.join("expected.html.json");
    assert_ne!(
        raw.is_file(),
        encoded.is_file(),
        "expected exactly one raw or JSON-wrapped native report: {}",
        case.display()
    );
    if encoded.is_file() {
        serde_json::from_slice(&std::fs::read(encoded).unwrap()).unwrap()
    } else {
        std::fs::read_to_string(raw).unwrap()
    }
}
fn model(path: &Path, props: &Value) -> bc_engine_table::Sheet {
    let mut options = ReadOptions {
        header: Some(HeaderMode::Data),
        ..Default::default()
    };
    if props["date_order"] == "Mdy" {
        options.format.regional.use_system = false;
        options.format.regional.date_order = DateOrder::Mdy;
        options.format.regional.date_separator = '/';
    }
    read_text(
        Cursor::new(std::fs::read(path).unwrap()),
        &options,
        &NeverCancel,
    )
    .unwrap()
    .sheet
}
#[test]
fn reference_table_rows_and_cells() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../bc-core/tests/golden/table");
    let mut cases = std::fs::read_dir(&root)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.join("case.json").is_file())
        .collect::<Vec<_>>();
    cases.sort();
    if cases.is_empty() {
        eprintln!("table golden corpus absent; skipping");
        return;
    }
    assert!(cases.len() >= 20, "incomplete formal table corpus");
    let mut known_seen = false;
    let (mut rows, mut row_matches, mut cells, mut cell_matches, mut exact_cases) = (0, 0, 0, 0, 0);
    let mut mismatches = Vec::new();
    for case in &cases {
        assert!(
            case.join("native-rules.xml").is_file(),
            "missing native profile for {}",
            case.display()
        );
        let props: Value =
            serde_json::from_slice(&std::fs::read(case.join("case.json")).unwrap()).unwrap();
        assert_eq!(
            props["header"], "Data",
            "golden explicitly compares the native report first record as data"
        );
        let left = model(&case.join("left.csv"), &props);
        let right = model(&case.join("right.csv"), &props);
        let expected = parse_html(&read_report(case));
        assert!(!expected.is_empty());
        let count = if props["multiple"] == true { 2 } else { 1 };
        assert_eq!(expected.len(), count);
        let mut lhs = Vec::new();
        let mut rhs = Vec::new();
        for i in 0..count {
            let mut l = left.clone();
            let mut r = right.clone();
            l.name = if i == 0 { "Data" } else { "Audit" }.into();
            r.name = l.name.clone();
            lhs.push(l);
            rhs.push(r);
        }
        if props["xlsx"] == true {
            let mut lb =
                Workbook::from_reader(Cursor::new(xlsx::workbook(&lhs)), Arc::new(NeverCancel))
                    .unwrap();
            let mut rb =
                Workbook::from_reader(Cursor::new(xlsx::workbook(&rhs)), Arc::new(NeverCancel))
                    .unwrap();
            lhs = lb
                .sheet_names()
                .to_vec()
                .iter()
                .map(|n| lb.load_sheet(n).unwrap())
                .collect();
            rhs = rb
                .sheet_names()
                .to_vec()
                .iter()
                .map(|n| rb.load_sheet(n).unwrap())
                .collect();
        }
        let mut case_equal = true;
        for sheet in 0..count {
            let (left, right) = (lhs[sheet].clone(), rhs[sheet].clone());
            let keys = props["keys"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as usize)
                .collect::<Vec<_>>();
            let mut rules = TableRules {
                columns: ColumnMapping::by_number(left.column_count(), right.column_count()),
                keys: if keys.is_empty() {
                    KeySelection::AllColumns
                } else {
                    KeySelection::Manual(keys)
                },
                sort_by_key: props["sort"].as_bool().unwrap(),
                strategy: if props["strategy"] == "None" {
                    AlignStrategy::None
                } else {
                    AlignStrategy::Quick
                },
                use_closeness: false,
                ..Default::default()
            };
            for (col, h) in props["handling"].as_object().unwrap() {
                let handling = &mut rules.columns.0[col.parse::<usize>().unwrap()].handling;
                handling.use_default = false;
                handling.unimportant = h["Insig"] == true;
                handling.ignore_case = h["StrCaseInsig"] == true;
                handling.ignore_whitespace = h["StrWSInsig"] == true;
                if h["AsText"] == true {
                    handling.column_type = ColumnType::Text;
                }
                handling.numeric_tolerance = h["NumTolerance"].as_f64().unwrap_or(0.0);
                handling.date_tolerance =
                    DateTolerance(h["DateTimeTolerance"].as_i64().unwrap_or(0) * 1000);
            }
            let map = TableMap::new(left, right, rules, &NeverCancel).unwrap();
            let oracle = &expected[sheet];
            let mut differences = Vec::new();
            for i in 0..oracle.len().max(map.row_count()) {
                rows += 1;
                let actual = map.row(i);
                let expected = oracle.get(i);
                let same = actual
                    .zip(expected)
                    .is_some_and(|(a, e)| a.kind == e.kind && a.importance == e.importance);
                row_matches += usize::from(same);
                let width = expected
                    .map_or(0, |e| e.cells.len())
                    .max(map.column_count());
                for c in 0..width {
                    cells += 1;
                    cell_matches += usize::from(
                        actual
                            .and_then(|_| map.cell_diff(i, c))
                            .zip(expected.and_then(|e| e.cells.get(c)))
                            .is_some_and(|(a, e)| a.value == *e),
                    );
                }
                if !same
                    || expected.is_some_and(|e| {
                        e.cells
                            .iter()
                            .enumerate()
                            .any(|(c, e)| map.cell_diff(i, c).is_none_or(|a| a.value != *e))
                    })
                {
                    differences.push(format!(
                        "row {i}: native={expected:?} actual={actual:?} cells={:?}",
                        (0..map.column_count())
                            .filter_map(|c| map.cell_diff(i, c).map(|d| d.value))
                            .collect::<Vec<_>>()
                    ));
                }
            }
            if !differences.is_empty() {
                assert_eq!(
                    case.file_name().unwrap().to_str().unwrap(),
                    "024-unaligned",
                    "new unlisted golden difference: {}",
                    differences.join("\n")
                );
                assert_eq!(sheet, 0);
                assert_eq!(oracle.len(), 3);
                assert_eq!(map.row_count(), 3);
                for (i, expected) in oracle.iter().enumerate() {
                    assert_eq!(
                        expected,
                        &ExpectedRow {
                            kind: DiffKind::Same,
                            importance: Importance::Same,
                            cells: vec![Importance::Same; 3]
                        }
                    );
                    let actual = map.row(i).unwrap();
                    assert_eq!((actual.left, actual.right), (Some(i), Some(i)));
                    assert_eq!(
                        actual.importance,
                        if i == 0 {
                            Importance::Same
                        } else {
                            Importance::Important
                        }
                    );
                    assert_eq!(
                        actual.kind,
                        if i == 0 {
                            DiffKind::Same
                        } else {
                            DiffKind::Changed
                        }
                    );
                    for c in 0..3 {
                        assert_eq!(
                            map.cell_diff(i, c).unwrap().value,
                            if i == 0 {
                                Importance::Same
                            } else {
                                Importance::Important
                            }
                        );
                    }
                }
                known_seen = true;
                case_equal = false;
                mismatches.push(format!(
                    "{} sheet {sheet}:\n{}",
                    case.file_name().unwrap().to_string_lossy(),
                    differences.join("\n")
                ));
            }
        }
        exact_cases += usize::from(case_equal);
    }
    eprintln!("table.golden cases={exact_cases}/{} rows={row_matches}/{rows} ({:.2}%) cells={cell_matches}/{cells} ({:.2}%) differences={}\n{}",cases.len(),100.0*row_matches as f64/rows as f64,100.0*cell_matches as f64/cells as f64,mismatches.len(),mismatches.join("\n"));
    assert!(
        known_seen,
        "KNOWN_DIFFS entry became stale; review instead of silently removing it"
    );
    assert!(row_matches * 100 >= rows * 95, "row agreement below 95%");
    assert!(cell_matches * 100 >= cells * 95, "cell agreement below 95%");
}
#[test]
fn quoted_newline_report_preserves_native_nuls_in_text_wrapper() {
    let case = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../bc-core/tests/golden/table/027-quoted-newline");
    if !case.join("case.json").is_file() {
        return; // The entire corpus may be absent in source-only distributions.
    }
    let encoded = std::fs::read(case.join("expected.html.json")).unwrap();
    assert!(!encoded.contains(&0), "committed wrapper must remain text");
    let html = read_report(&case);
    assert_eq!(html.bytes().filter(|&b| b == 0).count(), 2);
    for value in ["2", "3"] {
        assert!(html.contains(&format!(
            "line1\0line<span class=\"TableSegSigDiff\">{value}</span>"
        )));
    }
    let tables = parse_html(&html);
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].len(), 2);
    assert_eq!(
        tables[0][1].cells,
        [Importance::Same, Importance::Important, Importance::Same]
    );
}
#[test]
fn html_parser_handles_quoted_tag_characters_and_insignificant_cells() {
    let html = r#"<table class="fc"><tr><td class="TableItemNum"><img alt="<COL>">A</td></tr><tr><td class="TableItemInsigDiff">x</td><td class="AlignCenter Wrap">&lt;&gt;</td><td class="TableItemInsigDiff">X</td></tr></table>"#;
    let rows = parse_html(html);
    assert_eq!(rows[0].len(), 1);
    assert_eq!(rows[0][0].importance, Importance::Unimportant);
}
