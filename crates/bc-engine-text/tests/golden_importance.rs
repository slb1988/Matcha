#[path = "../../bc-core/tests/golden/support/mod.rs"]
mod support;
use bc_core::align::AlignStrategy;
use bc_engine_text::{rules::TextRules, TextContent, TextMap};
use std::path::Path;

#[test]
fn all_reference_rows_alignment_and_importance() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../bc-core/tests/golden/text/2way");
    let cases = support::corpus(&root).unwrap();
    if !support::has_expectations(&cases).unwrap() {
        eprintln!("golden oracle absent: engine regression skipped");
        return;
    }
    assert_eq!(
        cases.len(),
        30,
        "first corpus must contain all 30 planned cases"
    );
    let known = std::fs::read_to_string(root.join("../../KNOWN_DIFFS.md")).unwrap_or_default();
    let mut quick_failures = Vec::new();
    let mut smart_failures = Vec::new();
    for case in &cases {
        let format = case.properties.get("format").map(|name| {
            bc_formats::builtin_formats()
                .iter()
                .find(|f| f.name == *name && f.kind == bc_formats::FormatKind::Text)
                .expect("fixture format")
                .clone()
        });
        for strategy in ["myers", "standard"] {
            let mut rules = TextRules::default();
            rules.alignment.strategy = if strategy == "myers" {
                AlignStrategy::Quick
            } else {
                AlignStrategy::Smart
            };
            let flags = &mut rules.comparison.importance;
            flags.leading_whitespace_unimportant = case.flag("leading_unimportant");
            flags.embedded_whitespace_unimportant = case.flag("embedded_unimportant");
            flags.trailing_whitespace_unimportant = case.flag("trailing_unimportant");
            flags.case_unimportant = case.flag("case_unimportant");
            flags.line_endings_unimportant = !case.flag("compare_endings");
            if case.flag("comment_unimportant") {
                flags.unimportant.push(bc_formats::ElementKind::Comment);
            }
            let left =
                TextContent::from_bytes(case.input("left").unwrap(), format.clone()).unwrap();
            let right =
                TextContent::from_bytes(case.input("right").unwrap(), format.clone()).unwrap();
            let map = TextMap::compare(left, right, rules).unwrap();
            let actual: Vec<_> = map
                .rows()
                .map(|r| support::ExpectedRow {
                    left: r.left,
                    right: r.right,
                    kind: r.kind,
                    importance: r.importance as u8,
                })
                .collect();
            let expected = case.expected(strategy).unwrap();
            if actual != expected {
                let first = actual
                    .iter()
                    .zip(&expected)
                    .position(|(a, b)| a != b)
                    .unwrap_or(actual.len().min(expected.len()));
                let detail = format!(
                    "{}: first row {} actual={:?} expected={:?}; lengths {}/{}",
                    case.name,
                    first,
                    actual.get(first),
                    expected.get(first),
                    actual.len(),
                    expected.len()
                );
                if strategy == "myers" {
                    quick_failures.push(detail);
                } else {
                    if let Some(recorded) =
                        support::documented_project_rows(&known, strategy, &case.name).unwrap()
                    {
                        assert_eq!(actual, recorded, "known exception changed: {}", case.name);
                        assert_eq!(
                            expected,
                            support::documented_reference_rows(&known, strategy, &case.name)
                                .unwrap()
                                .expect("documented reference output"),
                            "known reference changed: {}",
                            case.name
                        );
                    }
                    smart_failures.push((case.name.clone(), detail));
                }
            }
        }
    }
    println!(
        "golden.myers={}/{} golden.standard={}/{} known_diffs={}",
        cases.len() - quick_failures.len(),
        cases.len(),
        cases.len() - smart_failures.len(),
        cases.len(),
        smart_failures.len()
    );
    if !smart_failures.is_empty() {
        eprintln!("Standard mismatches: {smart_failures:?}");
    }
    assert!(
        quick_failures.is_empty(),
        "Myers mismatches:\n{}",
        quick_failures.join("\n")
    );
    assert!(
        smart_failures.len() * 100 <= cases.len() * 5,
        "Standard below 95%:\n{:?}",
        smart_failures
    );
    assert_eq!(
        support::documented_cases(&known, "standard"),
        smart_failures
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>(),
        "stale or missing formal exceptions"
    );
    for (name, detail) in smart_failures {
        assert!(
            known.contains(&format!("| standard | {name} |")),
            "undocumented Standard mismatch: {detail}"
        );
    }
}
