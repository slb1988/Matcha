//! 启动器单独放在空目录、PATH 里也没有 `matcha` 时返回 103（两种启动器都一样）。
use std::path::Path;
use std::process::Command;

fn run_alone(launcher: &str, name: &str) -> Option<i32> {
    let dir = std::env::temp_dir().join(format!(
        "matcha-launcher-alone-{}-{name}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let copy = dir.join(Path::new(launcher).file_name().unwrap());
    std::fs::copy(launcher, &copy).unwrap();
    let status = Command::new(&copy)
        .args(["a.txt", "b.txt"])
        .env("PATH", "")
        .status()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    status.code()
}

#[test]
fn launchers_exit_103_when_the_main_program_is_missing() {
    assert_eq!(
        run_alone(env!("CARGO_BIN_EXE_matcha-comp"), "comp"),
        Some(103)
    );
    assert_eq!(
        run_alone(env!("CARGO_BIN_EXE_matcha-compc"), "compc"),
        Some(103)
    );
}

#[test]
fn console_launcher_prints_usage_itself() {
    let out = Command::new(env!("CARGO_BIN_EXE_matcha-compc"))
        .arg("/?")
        .env("PATH", "")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out.stdout).contains("/solo"));
}
