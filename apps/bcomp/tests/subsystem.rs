//! Windows 子系统：`matcha-comp` 是 GUI 子系统（VCS/快捷方式启动不闪控制台），
//! `matcha-compc`（发行名 `matcha.com`）是控制台子系统（shell 等待它、可读退出码）。
#![cfg(windows)]

/// PE 可选头里的 Subsystem 字段（PE32 与 PE32+ 都在可选头偏移 68 处）。
fn subsystem(exe: &str) -> u16 {
    let bytes = std::fs::read(exe).unwrap();
    let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    let pe = u32_at(0x3c) as usize;
    assert_eq!(&bytes[pe..pe + 4], b"PE\0\0", "{exe} is not a PE image");
    let optional = pe + 4 + 20;
    u16::from_le_bytes(bytes[optional + 68..optional + 70].try_into().unwrap())
}

const GUI: u16 = 2;
const CONSOLE: u16 = 3;

#[test]
fn return_immediately_launcher_is_a_gui_program() {
    assert_eq!(subsystem(env!("CARGO_BIN_EXE_matcha-comp")), GUI);
}

#[test]
fn waiting_launcher_is_a_console_program() {
    assert_eq!(subsystem(env!("CARGO_BIN_EXE_matcha-compc")), CONSOLE);
}

/// 发行时它被改名为 `matcha.com`：`/?` 用用户敲的名字 `matcha`。
#[test]
fn shipped_as_matcha_com_it_introduces_itself_as_matcha() {
    let dir = std::env::temp_dir().join(format!("matcha-com-usage-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let com = dir.join("matcha.com");
    std::fs::copy(env!("CARGO_BIN_EXE_matcha-compc"), &com).unwrap();
    let out = std::process::Command::new(&com)
        .arg("/?")
        .env("PATH", "")
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0));
    assert!(text.starts_with("Usage: matcha ["), "{text}");
    assert!(text.contains("\nmatcha waits until"), "{text}");
    assert!(!text.contains("matcha-compc"), "{text}");
}
