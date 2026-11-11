//! `/qc` 快速比较 → 退出码（参考命令行表）：字节模式 1/11，规则模式 2/12/13。
use bc_engine_dir::ContentCompare;
use bc_script::runner::quick_compare_files;
use bc_script::ExitCode;
use std::path::PathBuf;

fn pair(tag: &str, name: &str, left: &[u8], right: &[u8]) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("matcha-qc-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (l, r) = (dir.join(format!("l-{name}")), dir.join(format!("r-{name}")));
    std::fs::write(&l, left).unwrap();
    std::fs::write(&r, right).unwrap();
    (l, r)
}

fn qc(tag: &str, name: &str, left: &[u8], right: &[u8], mode: Option<ContentCompare>) -> ExitCode {
    let (l, r) = pair(tag, name, left, right);
    let code = quick_compare_files(&l, &r, mode).unwrap();
    let _ = std::fs::remove_dir_all(l.parent().unwrap());
    code
}

#[test]
fn rules_based_is_the_default_and_ignores_line_endings() {
    assert_eq!(
        qc("same", "a.txt", b"a\nb\n", b"a\nb\n", None),
        ExitCode::RulesBasedSame
    );
    assert_eq!(
        qc("crlf", "a.txt", b"a\r\nb\r\n", b"a\nb\n", None),
        ExitCode::RulesBasedSame
    );
    assert_eq!(
        qc("diff", "a.txt", b"a\nb\n", b"a\nc\n", None),
        ExitCode::RulesBasedDifferent
    );
    let rules = Some(ContentCompare::RulesBased);
    assert_eq!(
        qc("explicit", "a.txt", b"a\n", b"b\n", rules),
        ExitCode::RulesBasedDifferent
    );
}

#[test]
fn binary_input_in_rules_mode_reports_binary_differences() {
    assert_eq!(
        qc("bin", "a.dat", b"\0\x01\x02", b"\0\x01\x03", None),
        ExitCode::BinaryDifferent
    );
}

#[test]
fn byte_modes_report_binary_same_or_different() {
    for mode in [
        ContentCompare::Binary,
        ContentCompare::Crc,
        ContentCompare::Size,
    ] {
        let m = Some(mode);
        assert_eq!(
            qc("b-same", "a.txt", b"abc", b"abc", m),
            ExitCode::BinarySame,
            "{mode:?}"
        );
        assert_eq!(
            qc("b-crlf", "a.txt", b"a\r\n", b"a\n", m),
            ExitCode::BinaryDifferent,
            "{mode:?}"
        );
    }
    // size 只看长度：等长内容不同仍算相同（参考 /qc=size 的语义）。
    let size = Some(ContentCompare::Size);
    assert_eq!(
        qc("size", "a.txt", b"abc", b"xyz", size),
        ExitCode::BinarySame
    );
    let binary = Some(ContentCompare::Binary);
    assert_eq!(
        qc("bin-eq-len", "a.txt", b"abc", b"xyz", binary),
        ExitCode::BinaryDifferent
    );
}

#[test]
fn folders_and_missing_files_are_errors() {
    let (l, _) = pair("err", "a.txt", b"a", b"a");
    let dir = l.parent().unwrap().to_path_buf();
    let folder = quick_compare_files(&dir, &l, None).unwrap_err();
    assert!(folder.contains("is a folder"), "{folder}");
    let missing = quick_compare_files(&l, &dir.join("nope.txt"), None).unwrap_err();
    assert!(missing.contains("does not exist"), "{missing}");
    let _ = std::fs::remove_dir_all(&dir);
}
