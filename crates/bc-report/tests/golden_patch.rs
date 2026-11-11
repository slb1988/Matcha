//! patch 黄金回归：对 `text/2way`（30 例）与 `text/patch-extra`（多差异块探针）逐份比对参考
//! 工具的原始 patch 输出（协议见 `tools/golden/README.md` 的“patch 报告协议”）。只归一化两处：
//! 文件头里的时间戳，以及采集时的工作目录前缀；其余字节严格比较。样本缺失即失败，不跳过。
#[path = "../../bc-core/tests/golden/support/mod.rs"]
#[allow(dead_code)]
mod support;

use bc_core::align::AlignStrategy;
use bc_engine_text::{rules::TextRules, TextContent, TextMap};
use bc_report::{patch, PatchFormat, ReportFile};
use std::path::Path;

const WORK: &str = r"D:\Github\Matcha\target\golden-work\";
const FORMATS: [(PatchFormat, &str); 3] = [
    (PatchFormat::Normal, "normal"),
    (PatchFormat::Context, "context"),
    (PatchFormat::Unified, "unified"),
];

/// 参考头部时间戳的形状：`YYYY-MM-DD HH:MM:SS.000000000 +hhmm`。
fn is_reference_time(text: &str) -> bool {
    text.len() == 35
        && text.chars().enumerate().all(|(i, c)| match i {
            4 | 7 => c == '-',
            10 | 29 => c == ' ',
            13 | 16 => c == ':',
            19 => c == '.',
            20..=28 => c == '0',
            30 => c == '+' || c == '-',
            _ => c.is_ascii_digit(),
        })
}

/// 文件头前两行里 TAB 之后的时间戳换成 `<time>`（并校验形状），工作目录前缀换成 `<work>\`。
fn normalize(text: &str) -> String {
    let mut out = String::new();
    for (index, line) in text.split_inclusive("\r\n").enumerate() {
        let line = line.replace(WORK, r"<work>\");
        match line.split_once('\t') {
            Some((head, time)) if index < 2 => {
                let time = time.trim_end_matches("\r\n");
                assert!(is_reference_time(time), "unexpected header time {time:?}");
                out.push_str(&format!("{head}\t<time>\r\n"));
            }
            _ => out.push_str(&line),
        }
    }
    out
}

fn rules(case: &support::Case, strategy: &str) -> TextRules {
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
    rules
}

fn compare(case: &support::Case, strategy: &str) -> TextMap {
    let format = case.properties.get("format").map(|name| {
        bc_formats::builtin_formats()
            .iter()
            .find(|f| f.name == *name && f.kind == bc_formats::FormatKind::Text)
            .expect("fixture format")
            .clone()
    });
    let left = TextContent::from_bytes(case.input("left").unwrap(), format.clone()).unwrap();
    let right = TextContent::from_bytes(case.input("right").unwrap(), format).unwrap();
    TextMap::compare(left, right, rules(case, strategy)).unwrap()
}

/// 与采集脚本相同的工作副本命名。
fn files(case: &support::Case) -> [ReportFile; 2] {
    let extension = if case.properties.contains_key("reference_extension") {
        ".matcha-golden-long"
    } else {
        match case.property("format", "").as_str() {
            "CPP" => ".cpp",
            "PAS" => ".pas",
            _ => ".txt",
        }
    };
    ["left", "right"].map(|side| ReportFile {
        path: format!(r"<work>\{}\{side}{extension}", case.name),
        modified: "<time>".into(),
    })
}

fn check(root: &Path, expected_cases: usize) -> (usize, Vec<String>) {
    let cases = support::corpus(root).unwrap();
    assert_eq!(cases.len(), expected_cases, "{}", root.display());
    let mut compared = 0;
    let mut failures = Vec::new();
    for case in &cases {
        let names = files(case);
        for strategy in ["myers", "standard"] {
            let map = compare(case, strategy);
            let mut samples: Vec<(String, PatchFormat, bool)> = FORMATS
                .iter()
                .map(|(format, name)| {
                    (
                        format!("expected.{strategy}.patch-{name}.txt"),
                        *format,
                        false,
                    )
                })
                .collect();
            for (suffix, ignore) in [("title", false), ("ignore-unimportant", true)] {
                let leaf = format!("expected.{strategy}.patch-unified.{suffix}.txt");
                if case.directory.join(&leaf).exists() {
                    samples.push((leaf, PatchFormat::Unified, ignore));
                }
            }
            for (leaf, format, ignore) in samples {
                let path = case.directory.join(&leaf);
                let bytes = std::fs::read(&path)
                    .unwrap_or_else(|e| panic!("missing sample {}: {e}", path.display()));
                let expected = normalize(std::str::from_utf8(&bytes).expect("UTF-8 sample"));
                let actual = patch::render_map(&map, &names, format, ignore);
                compared += 1;
                if actual != expected {
                    failures.push(format!(
                        "{}/{leaf}\n--- expected\n{expected}--- actual\n{actual}",
                        case.name
                    ));
                }
            }
        }
    }
    (compared, failures)
}

#[test]
fn patch_reports_match_the_reference_byte_for_byte() {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("../bc-core/tests/golden/text");
    let (two_way, mut failures) = check(&golden.join("2way"), 30);
    let (extra, more) = check(&golden.join("patch-extra"), 10);
    failures.extend(more);
    let total = two_way + extra;
    println!(
        "golden.patch={}/{total} (2way {two_way}, patch-extra {extra})",
        total - failures.len()
    );
    // 30 x 2 x 3 + 1 title + 7 x 2 ignore-unimportant; 10 x 2 x 3.
    assert_eq!((two_way, extra), (195, 60));
    assert!(
        failures.is_empty(),
        "{} mismatch(es):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// 输出字节：无 BOM 的非 ASCII UTF-8 原样输出（028）；带 BOM 的 UTF-8（024）与 UTF-16（025、027）
/// 输入都输出带 BOM 的 UTF-8。逐字节比较完整的 `Report::to_bytes`。
#[test]
fn report_bytes_follow_the_reference_encoding() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../bc-core/tests/golden/text/2way");
    let cases = support::corpus(&root).unwrap();
    for (name, bom) in [
        ("028-unicode-change", false),
        ("024-utf8-bom", true),
        ("025-utf16le-versus-utf8", true),
        ("027-utf16-endianness", true),
        ("005-insert-middle-three", false),
    ] {
        let case = cases.iter().find(|c| c.name == name).unwrap();
        let map = compare(case, "myers");
        let request = bc_report::ReportRequest {
            kind: bc_session::SessionKind::TextCompare,
            layout: bc_report::Layout::Patch,
            options: bc_report::ReportOptions {
                patch_format: PatchFormat::Unified,
                ..Default::default()
            },
            title: None,
            output: bc_report::Output::Stdout,
            output_options: Default::default(),
            comparison: None,
        };
        use bc_report::Renderer;
        let renderer = bc_report::TextReportRenderer {
            map: &map,
            files: files(case),
        };
        let bytes = renderer.render(&request).unwrap().to_bytes();
        let sample =
            std::fs::read(case.directory.join("expected.myers.patch-unified.txt")).unwrap();
        let expected = normalize(std::str::from_utf8(&sample).unwrap()).into_bytes();
        assert_eq!(bytes.starts_with(b"\xef\xbb\xbf"), bom, "{name}");
        assert_eq!(bytes, expected, "{name}");
    }
}
