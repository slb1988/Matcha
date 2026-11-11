//! 端到端：经发行布局里的 `matcha.com`（控制台启动器）运行脚本与 `/report`，检查报告字节、
//! 写文件、退出码与错误输出。`matcha.exe` 是 GUI 子系统程序，这里证明它的输出确实经启动器
//! 到达标准输出/标准错误。不打开任何窗口。
#![cfg(windows)]
use std::path::PathBuf;
use std::process::{Command, Output};

fn matcha() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_matcha"))
}

/// 临时目录里的发行布局（`matcha.exe` + `matcha.com`）与两份输入。启动器属于另一个包，
/// 只有 `cargo test --workspace`（或先构建过 bcomp）时才在同一目录里。
struct Layout {
    dir: PathBuf,
    com: PathBuf,
}

impl Layout {
    fn new(tag: &str) -> Option<Self> {
        let compc = matcha().with_file_name("matcha-compc.exe");
        if !compc.is_file() {
            eprintln!("skipped: {} not built", compc.display());
            return None;
        }
        let dir = std::env::temp_dir().join(format!("matcha-reports-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::copy(matcha(), dir.join("matcha.exe")).unwrap();
        std::fs::copy(&compc, dir.join("matcha.com")).unwrap();
        std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
        std::fs::write(dir.join("b.txt"), "one\n2\nthree\nfour\n").unwrap();
        Some(Layout {
            com: dir.join("matcha.com"),
            dir,
        })
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(&self.com)
            .args(args)
            .current_dir(&self.dir)
            .env("PATH", "")
            .env(
                "MATCHA_INSTANCE",
                format!("Matcha-test-reports-{}", std::process::id()),
            )
            .output()
            .unwrap()
    }

    fn script(&self, name: &str, text: &str) -> String {
        std::fs::write(self.dir.join(name), text).unwrap();
        format!("@{name}")
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }
}

impl Drop for Layout {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn unified_body(stdout: &[u8]) -> String {
    let text = String::from_utf8(stdout.to_vec()).unwrap();
    text.split_once("@@")
        .map(|(_, b)| format!("@@{b}"))
        .unwrap_or(text)
}

#[test]
fn script_writes_a_patch_to_stdout() {
    let Some(l) = Layout::new("stdout") else {
        return;
    };
    let script = l.script(
        "s.txt",
        "# 生成补丁\ntext-report layout:patch options:patch-unified &\n  output-to:- \"%1\" \"%2\"\n",
    );
    let out = l.run(&[&script, "a.txt", "b.txt"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(out.stderr.is_empty(), "{}", stderr(&out));
    let a = l.path("a.txt").display().to_string();
    let text = String::from_utf8(out.stdout.clone()).unwrap();
    assert!(text.starts_with(&format!("--- {a}\t")), "{text}");
    assert_eq!(
        unified_body(&out.stdout),
        "@@ -1,3 +1,4 @@\r\n one\r\n-two\r\n+2\r\n three\r\n+four\r\n"
    );
}

#[test]
fn script_writes_a_patch_file() {
    let Some(l) = Layout::new("file") else { return };
    let script = l.script(
        "s.txt",
        "text-report layout:patch output-to:out.patch a.txt b.txt\n",
    );
    let out = l.run(&[&script]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(out.stdout.is_empty());
    let written = std::fs::read_to_string(l.path("out.patch")).unwrap();
    assert!(
        written.ends_with("2c2\r\n< two\r\n---\r\n> 2\r\n3a4\r\n> four\r\n"),
        "{written}"
    );
}

#[test]
fn report_shortcut_matches_the_equivalent_script() {
    let Some(l) = Layout::new("shortcut") else {
        return;
    };
    let script = l.script(
        "s.txt",
        "text-report layout:patch options:patch-context,ignore-unimportant output-to:- a.txt b.txt\n",
    );
    let from_script = l.run(&[&script]);
    let from_switch = l.run(&[
        "/report=patch-context",
        "/reportoptions=ignore-unimportant",
        "a.txt",
        "b.txt",
    ]);
    assert_eq!(
        from_switch.status.code(),
        Some(0),
        "{}",
        stderr(&from_switch)
    );
    assert!(!from_switch.stdout.is_empty());
    assert_eq!(from_switch.stdout, from_script.stdout);
}

#[test]
fn report_shortcut_to_a_file() {
    let Some(l) = Layout::new("reportfile") else {
        return;
    };
    let to_stdout = l.run(&["/report=patch", "a.txt", "b.txt"]);
    let to_file = l.run(&[
        "/report=patch",
        "/reportfile=out\\diff.patch",
        "a.txt",
        "b.txt",
    ]);
    // `out` 不存在：写出失败，退出码 100，且不留残缺文件。
    assert_eq!(to_file.status.code(), Some(100), "{}", stderr(&to_file));
    assert!(
        stderr(&to_file).contains("cannot write the report"),
        "{}",
        stderr(&to_file)
    );
    assert!(!l.path("out").exists());
    std::fs::create_dir(l.path("out")).unwrap();
    let to_file = l.run(&[
        "/report=patch",
        "/reportfile=out\\diff.patch",
        "a.txt",
        "b.txt",
    ]);
    assert_eq!(to_file.status.code(), Some(0), "{}", stderr(&to_file));
    assert!(to_file.stdout.is_empty());
    assert_eq!(
        std::fs::read(l.path("out\\diff.patch")).unwrap(),
        to_stdout.stdout
    );
}

#[test]
fn exit_codes_105_106_107_and_100() {
    let Some(l) = Layout::new("codes") else {
        return;
    };
    let missing_script = l.run(&["@nope.txt"]);
    assert_eq!(missing_script.status.code(), Some(105));
    assert!(stderr(&missing_script).contains("nope.txt"));

    let syntax = l.script(
        "syntax.txt",
        "beep\n\ntext-reprot layout:patch output-to:- a.txt b.txt\n",
    );
    let out = l.run(&[&syntax]);
    assert_eq!(out.status.code(), Some(106));
    assert!(stderr(&out).contains("line 3"), "{}", stderr(&out));

    let missing_file = l.script(
        "missing.txt",
        "text-report layout:patch output-to:- nope.txt b.txt\n",
    );
    let out = l.run(&[&missing_file]);
    assert_eq!(out.status.code(), Some(107));
    assert!(
        stderr(&out).contains("line 1: cannot read"),
        "{}",
        stderr(&out)
    );

    let unsupported = l.script(
        "load.txt",
        "load a b\ntext-report layout:patch output-to:- a.txt b.txt\n",
    );
    let out = l.run(&[&unsupported]);
    assert_eq!(out.status.code(), Some(100));
    assert!(out.stdout.is_empty(), "nothing runs");
    assert!(
        stderr(&out).contains("line 1") && stderr(&out).contains("load"),
        "{}",
        stderr(&out)
    );

    let layout = l.run(&["/report=side-by-side", "a.txt", "b.txt"]);
    assert_eq!(layout.status.code(), Some(100));
    assert!(
        stderr(&layout).contains("not supported yet"),
        "{}",
        stderr(&layout)
    );

    let one_file = l.run(&["/report=patch", "a.txt"]);
    assert_eq!(one_file.status.code(), Some(100));
    assert!(
        stderr(&one_file).contains("two files"),
        "{}",
        stderr(&one_file)
    );

    let folder = l.run(&["/report=patch", ".", "a.txt"]);
    assert_eq!(folder.status.code(), Some(100));
    assert!(
        stderr(&folder).contains("is a folder"),
        "{}",
        stderr(&folder)
    );

    let missing = l.run(&["/report=patch", "nope.txt", "b.txt"]);
    assert_eq!(missing.status.code(), Some(107));
    assert!(
        stderr(&missing).starts_with("cannot read "),
        "{}",
        stderr(&missing)
    );
}

#[test]
fn silent_changes_nothing_in_the_output() {
    let Some(l) = Layout::new("silent") else {
        return;
    };
    let script = l.script(
        "s.txt",
        "text-report layout:patch output-to:- \"%1\" \"%2\"\n",
    );
    let plain = l.run(&[&script, "a.txt", "b.txt"]);
    let silent = l.run(&[&script, "/silent", "a.txt", "b.txt"]);
    assert_eq!(silent.status.code(), Some(0), "{}", stderr(&silent));
    assert_eq!(silent.stdout, plain.stdout);
}

#[test]
fn reports_still_exit_zero_when_files_differ() {
    let Some(l) = Layout::new("zero") else { return };
    let out = l.run(&["/report=patch-unified", "a.txt", "b.txt"]);
    assert_eq!(out.status.code(), Some(0));
    let same = l.run(&["/report=patch-unified", "a.txt", "a.txt"]);
    assert_eq!(same.status.code(), Some(0));
    assert!(!String::from_utf8_lossy(&same.stdout).contains("@@"));
}

/// 直接运行 GUI 子系统的 `matcha.exe /report=…`（重定向时）同样可用。
#[test]
fn gui_program_report_with_redirected_output() {
    let dir = std::env::temp_dir().join(format!("matcha-reports-direct-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.txt"), "x\n").unwrap();
    std::fs::write(dir.join("b.txt"), "y\n").unwrap();
    let out = Command::new(matcha())
        .args(["/report=patch", "a.txt", "b.txt"])
        .current_dir(&dir)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).ends_with("1c1\r\n< x\r\n---\r\n> y\r\n"));
}
