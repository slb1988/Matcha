//! Windows 子系统与发行布局：`matcha.exe` 是 GUI 子系统（双击不弹控制台）；与它并列的
//! `matcha.com`（控制台启动器改名）负责命令行：等待、把文本送到终端/重定向、传播退出码。
#![cfg(windows)]
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// PE 可选头里的 Subsystem 字段（PE32 与 PE32+ 都在可选头偏移 68 处）。
fn subsystem(exe: &Path) -> u16 {
    let bytes = std::fs::read(exe).unwrap();
    let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    let pe = u32_at(0x3c) as usize;
    assert_eq!(
        &bytes[pe..pe + 4],
        b"PE\0\0",
        "{} is not a PE image",
        exe.display()
    );
    let optional = pe + 4 + 20;
    u16::from_le_bytes(bytes[optional + 68..optional + 70].try_into().unwrap())
}

const GUI: u16 = 2;

fn matcha() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_matcha"))
}

#[test]
fn main_program_is_a_gui_program() {
    assert_eq!(subsystem(&matcha()), GUI);
}

/// 发行布局：同一目录里 `matcha.exe` + `matcha.com`。启动器属于另一个包，只有
/// `cargo test --workspace`（或先构建过 bcomp）时才在同一目录里。
fn shipped(tag: &str) -> Option<(PathBuf, PathBuf)> {
    let compc = matcha().with_file_name("matcha-compc.exe");
    if !compc.is_file() {
        eprintln!("skipped: {} not built", compc.display());
        return None;
    }
    let dir = std::env::temp_dir().join(format!("matcha-shipped-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(matcha(), dir.join("matcha.exe")).unwrap();
    std::fs::copy(&compc, dir.join("matcha.com")).unwrap();
    Some((dir.join("matcha.com"), dir))
}

fn run(com: &Path, args: &[&str]) -> Output {
    Command::new(com)
        .args(args)
        .current_dir(com.parent().unwrap())
        .env("PATH", "")
        .env(
            "MATCHA_INSTANCE",
            format!("Matcha-test-shipped-{}", std::process::id()),
        )
        .output()
        .unwrap()
}

/// GUI 子系统的 `matcha.exe` 仍把命令行结果送回 `matcha.com` 的 stderr，退出码原样传播。
#[test]
fn errors_of_the_gui_program_reach_the_console_launcher() {
    let Some((com, dir)) = shipped("error") else {
        return;
    };
    let out = run(&com, &["/qc", "missing-a.txt", "missing-b.txt"]);
    let _ = std::fs::remove_dir_all(&dir);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(100), "{stderr}");
    assert!(stderr.contains("missing-a.txt does not exist"), "{stderr}");
}

/// `matcha /qc a b`：不开窗口，结果一行进 stdout，退出码为参考表中的值；`/silent` 不输出。
#[test]
fn quick_compare_prints_the_result_and_returns_its_code() {
    let Some((com, dir)) = shipped("qc") else {
        return;
    };
    std::fs::write(dir.join("a.txt"), "one\r\ntwo\r\n").unwrap();
    std::fs::write(dir.join("b.txt"), "one\ntwo\n").unwrap();
    std::fs::write(dir.join("c.txt"), "one\nthree\n").unwrap();
    let same = run(&com, &["/qc", "a.txt", "b.txt"]);
    let different = run(&com, &["/qc", "a.txt", "c.txt"]);
    let binary = run(&com, &["/qc=binary", "a.txt", "b.txt"]);
    let silent = run(&com, &["/silent", "/qc", "a.txt", "c.txt"]);
    let _ = std::fs::remove_dir_all(&dir);
    let stdout = |out: &Output| String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert_eq!(
        (same.status.code(), stdout(&same).as_str()),
        (Some(2), "Rules-based same")
    );
    assert_eq!(
        (different.status.code(), stdout(&different).as_str()),
        (Some(13), "Rules-based differences")
    );
    assert_eq!(
        (binary.status.code(), stdout(&binary).as_str()),
        (Some(11), "Binary differences")
    );
    assert_eq!(
        (silent.status.code(), stdout(&silent).as_str()),
        (Some(13), "")
    );
}

/// 直接运行 GUI 子系统的 `matcha.exe /? > help.txt`：重定向的 stdout 照常可写。
#[test]
fn output_of_the_gui_program_can_be_redirected() {
    let out = Command::new(matcha()).arg("/?").output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.starts_with("Usage: matcha ["), "{stdout}");
    assert!(stdout.contains("scenes: "), "{stdout}");
}
