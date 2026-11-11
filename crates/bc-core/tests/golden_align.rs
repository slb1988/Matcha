#[path = "golden/support/mod.rs"]
#[allow(dead_code)] // exception helpers are shared with the engine harness
mod support;
use bc_core::{
    align::{AlignInput, Aligner, QuickAlign, SmartAlign, SmartAlignOptions},
    encoding,
    hash::LineHasher,
    line::LineList,
    DiffKind,
};
use std::path::Path;

#[test]
fn captured_myers_never_align_diagnostic_matches_current_core() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/text/2way");
    let case = support::corpus(&root)
        .unwrap()
        .into_iter()
        .find(|c| c.name == "012-unrelated-replacement")
        .unwrap();
    let left = LineList::from_bytes(case.input("left").unwrap());
    let right = LineList::from_bytes(case.input("right").unwrap());
    let lh = LineHasher::default().hash_lines(&left);
    let rh = LineHasher::default().hash_lines(&right);
    let map = QuickAlign.align2_unpaired(
        AlignInput {
            hashes: &lh,
            ..Default::default()
        },
        AlignInput {
            hashes: &rh,
            ..Default::default()
        },
    );
    let expected = support::parse_report(include_str!(
        "../../../tools/golden/evidence/012-myers-never-align.xml"
    ))
    .unwrap();
    let actual: Vec<_> = map
        .rows()
        .iter()
        .map(|r| (r.left, r.right, r.kind))
        .collect();
    assert_eq!(
        actual,
        expected
            .iter()
            .map(|r| (r.left, r.right, r.kind))
            .collect::<Vec<_>>()
    );
}

#[test]
fn reference_alignment_corpus() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/text/2way");
    let cases = support::corpus(&root).unwrap();
    if !support::has_expectations(&cases).unwrap() {
        eprintln!("golden oracle absent: alignment regression skipped");
        return;
    }
    let known = std::fs::read_to_string(root.join("../../KNOWN_DIFFS.md")).unwrap_or_default();
    assert!(
        support::documented_cases(&known, "standard").is_empty(),
        "the text-alignment spec leaves no formal Standard exception"
    );
    let mut failures = Vec::new();
    let mut checked = 0;
    for case in &cases {
        // Format-sensitive normalization belongs to the engine; its golden test
        // compares ALL cases, including these, with full classification.
        if case.properties.contains_key("format") {
            continue;
        }
        checked += 1;
        let left = LineList::from_bytes(
            encoding::decode(&case.input("left").unwrap(), case.encoding("left").unwrap())
                .unwrap()
                .into_bytes(),
        );
        let right = LineList::from_bytes(
            encoding::decode(
                &case.input("right").unwrap(),
                case.encoding("right").unwrap(),
            )
            .unwrap()
            .into_bytes(),
        );
        let hasher = LineHasher {
            ignore_leading_whitespace: case.flag("leading_unimportant"),
            ignore_embedded_whitespace: case.flag("embedded_unimportant"),
            ignore_trailing_whitespace: case.flag("trailing_unimportant"),
            ignore_case: case.flag("case_unimportant"),
        };
        let lh = hasher.hash_lines(&left);
        let rh = hasher.hash_lines(&right);
        for strategy in ["myers", "standard"] {
            let l = AlignInput {
                hashes: &lh,
                lines: Some(&left),
                ..Default::default()
            };
            let r = AlignInput {
                hashes: &rh,
                lines: Some(&right),
                ..Default::default()
            };
            let never_align = case.flag("never_align_diffs");
            let map = match strategy {
                "myers" if never_align => QuickAlign.align2_unpaired(l, r),
                "myers" => QuickAlign.align2(l, r),
                _ => SmartAlign {
                    options: SmartAlignOptions {
                        case_tolerant: case.flag("case_unimportant"),
                        skew_tolerance: 2000,
                        use_closeness: true,
                        never_align_diffs: never_align,
                        ..Default::default()
                    },
                }
                .align2(l, r),
            };
            let actual: Vec<_> = map
                .rows()
                .iter()
                .map(|r| (r.left, r.right, r.kind))
                .collect();
            let expected_rows = case.expected(strategy).unwrap();
            if expected_rows.iter().any(|row| {
                row.left.is_some_and(|i| i >= lh.len()) || row.right.is_some_and(|i| i >= rh.len())
            }) {
                failures.push(format!("{} {strategy}: reference line indices exceed physical input; check reference long-line splitting", case.name));
                continue;
            }
            let expected: Vec<_> = expected_rows
                .iter()
                .map(|r| {
                    let kind = match (r.left, r.right) {
                        (Some(l), Some(r)) if lh[l] == rh[r] => DiffKind::Same,
                        _ => r.kind,
                    };
                    (r.left, r.right, kind)
                })
                .collect();
            if actual != expected {
                failures.push(format!(
                    "{} {strategy}: actual {actual:?}, expected {expected:?}",
                    case.name
                ));
            }
        }
    }
    println!(
        "core normalized alignment cases={checked}; format-sensitive cases also run in engine"
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
