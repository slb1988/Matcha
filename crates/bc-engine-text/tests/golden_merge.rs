//! Three-way merge corpus (`tests/golden/merge/<case>/{left,right,center}.txt`).
//!
//! When reference samples exist (`expected.<variant>.code` / `.out`, written by
//! `tools/golden/gen_merge_expected.ps1`) every case is compared with them byte for byte.
//! Until then the corpus pins the provisional semantics (design D3–D7): the number of
//! conflict sections per case. A partly collected corpus is an error, never a skip.
use bc_engine_text::{
    merge::{auto_merge, MergeLabels},
    rules::{Favor, TextMergeRules, TextRules},
    TextContent, TextMap,
};
use std::path::{Path, PathBuf};

const VARIANTS: [&str; 4] = ["automerge", "force", "iu", "iu-favorleft"];

/// Provisional expectation: conflict sections under the default rules.
const PROVISIONAL: [(&str, usize); 20] = [
    ("001-left-only", 0),
    ("002-both-far", 0),
    ("003-gap0", 1),
    ("004-gap1", 1),
    ("005-gap2", 1),
    ("006-gap3", 0),
    ("007-same-line", 1),
    ("008-same-change", 0),
    ("009-insert-unequal", 1),
    ("010-delete-vs-modify", 1),
    ("011-insert-identical", 0),
    ("012-ws-left-only", 0),
    ("013-ws-conflict", 1),
    ("014-ws-vs-real", 1),
    ("015-eol-left-lf", 0),
    ("016-enc-left-utf8bom", 0),
    ("017-no-final-newline", 1),
    ("018-all-same", 0),
    ("019-case-conflict", 1),
    ("020-far-and-conflict", 1),
];

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/merge")
}

fn rules(variant: &str) -> TextMergeRules {
    TextMergeRules {
        force_conflict_markers: variant == "force",
        ignore_unimportant: variant.starts_with("iu"),
        favor: if variant == "iu-favorleft" {
            Favor::Left
        } else {
            Favor::None
        },
        ..Default::default()
    }
}

fn load(case: &Path) -> TextMap {
    let side = |leaf: &str| {
        let bytes = std::fs::read(case.join(leaf)).unwrap_or_else(|e| panic!("{leaf}: {e}"));
        TextContent::from_bytes(bytes, None).unwrap()
    };
    TextMap::merge(
        side("left.txt"),
        side("right.txt"),
        side("center.txt"),
        TextRules::default(),
    )
    .unwrap()
}

#[test]
fn merge_corpus() {
    let root = corpus();
    let labels = MergeLabels {
        left: "left.txt".into(),
        right: "right.txt".into(),
    };
    let mut cases: Vec<PathBuf> = std::fs::read_dir(&root)
        .unwrap()
        .filter_map(|e| Some(e.ok()?.path()))
        .filter(|p| p.join("center.txt").is_file())
        .collect();
    cases.sort();
    assert_eq!(cases.len(), PROVISIONAL.len(), "corpus and table disagree");
    let sample = |case: &Path, variant: &str| case.join(format!("expected.{variant}.code"));
    let collected = cases
        .iter()
        .flat_map(|c| VARIANTS.iter().map(move |v| sample(c, v)))
        .filter(|p| p.is_file())
        .count();
    assert!(
        collected == 0 || collected == cases.len() * VARIANTS.len(),
        "reference samples are partly collected ({collected} of {})",
        cases.len() * VARIANTS.len()
    );
    for case in &cases {
        let name = case.file_name().unwrap().to_string_lossy().into_owned();
        let map = load(case);
        for variant in VARIANTS {
            let outcome = auto_merge(&map, &rules(variant), &labels)
                .unwrap_or_else(|e| panic!("{name} [{variant}]: {e:?}"));
            let context = format!("{name} [{variant}]");
            if collected == 0 {
                let expected = PROVISIONAL.iter().find(|(n, _)| *n == name).unwrap().1;
                assert_eq!(outcome.conflicts.len(), expected, "{context}");
                assert_eq!(
                    outcome.withheld,
                    expected > 0 && variant != "force",
                    "{context}"
                );
                continue;
            }
            let report = std::fs::read_to_string(sample(case, variant)).unwrap();
            let mut lines = report.lines();
            let code: i32 = lines.next().unwrap().trim().parse().unwrap();
            let written = lines.next() == Some("output: written");
            let ours = match (outcome.conflicts.is_empty(), outcome.withheld) {
                (true, _) => 0,
                (false, true) => 101,
                (false, false) => 14,
            };
            assert_eq!(ours, code, "{context}: exit code");
            assert_eq!(!outcome.withheld, written, "{context}: output written");
            if written {
                let expected = std::fs::read(case.join(format!("expected.{variant}.out"))).unwrap();
                assert_eq!(outcome.output, expected, "{context}: output bytes");
            }
        }
    }
}
