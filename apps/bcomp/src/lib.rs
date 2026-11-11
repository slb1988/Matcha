//! Launcher logic shared by `matcha-comp` (GUI launcher, returns at once) and
//! `matcha-compc` (console launcher, waits for the comparison and returns its exit code).
//!
//! Reference behaviour (command_line_reference "program trio"):
//! * locate the main executable next to the launcher, then on `PATH` (exit 103 when missing);
//! * forward all arguments unchanged; the waiting launcher additionally sets the hidden
//!   [`bc_instance::WAIT_ENV`] marker (exit 102 when waiting is impossible);
//! * `matcha-compc` must have a console; it prints `/?` usage itself.
//!
//! 发行布局（`cargo xtask dist`）：Windows 上控制台启动器以 `matcha.com` 与 `matcha.exe`
//! 并列发布，命令行敲 `matcha …` 经 `PATHEXT` 先解析到 `.com`；macOS 上它在
//! `Matcha.app/Contents/MacOS/` 里，经 `bin/matcha` 符号链接进入 `PATH`。因此启动器按
//! 解析符号链接后的真实位置找主程序，并且永远不把自己当成主程序（Unix 上二者可能同名）。
//!
//! 启动器只负责找到并启动 `matcha`；单实例判定与转交都在 `matcha` 里。等待时
//! `matcha` 若转交给常驻实例，会阻塞到该标签页关闭并以回复的退出码退出；若自己成为
//! 首个实例，则在窗口关闭（关掉唯一的标签页即关窗）时退出。启动器只需等子进程退出并
//! 传播其退出码，因此不需要 unsafe 或额外的同步句柄。
#![forbid(unsafe_code)]

use bc_script::ExitCode;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Which launcher flavour is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchMode {
    /// `matcha-comp`: start the GUI (or hand off to the running instance) and return 0.
    ReturnImmediately,
    /// `matcha-compc`: wait for the comparison/script to finish and return its exit code.
    WaitForExitCode,
}

/// Name of the main executable looked up next to the launcher.
pub const MAIN_EXECUTABLE: &str = if cfg!(windows) {
    "matcha.exe"
} else {
    "matcha"
};

/// The launcher's own path with symlinks resolved (`bin/matcha` → `Matcha.app/…`).
fn launcher_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(std::fs::canonicalize(&exe).unwrap_or(exe))
}

/// Find the main executable (same directory as the launcher, then `PATH`).
pub fn locate_main_executable() -> Result<PathBuf, ExitCode> {
    let launcher = launcher_path();
    locate_in(launcher.as_deref(), std::env::var_os("PATH"))
}

/// [`locate_main_executable`] 的可测部分：`launcher` 为启动器自身的真实路径。
fn locate_in(launcher: Option<&Path>, path: Option<OsString>) -> Result<PathBuf, ExitCode> {
    let myself = launcher.and_then(|exe| std::fs::canonicalize(exe).ok());
    let beside = launcher
        .and_then(Path::parent)
        .map(|dir| dir.join(MAIN_EXECUTABLE));
    let on_path = path
        .iter()
        .flat_map(std::env::split_paths)
        .map(|dir| dir.join(MAIN_EXECUTABLE));
    beside
        .into_iter()
        .chain(on_path)
        .filter(|candidate| candidate.is_file())
        // PATH 里的 `matcha` 可能正是指向本启动器的符号链接：跳过，免得无限自启动。
        .find(|candidate| myself.is_none() || std::fs::canonicalize(candidate).ok() != myself)
        .ok_or(ExitCode::LauncherCannotFindMain)
}

/// The name the user typed for this launcher: `matcha` for the shipped `matcha.com` /
/// `bin/matcha`, otherwise the build name (`matcha-compc`).
fn invoked_name() -> String {
    std::env::args_os()
        .next()
        .map(PathBuf::from)
        .and_then(|arg0| {
            arg0.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "matcha-compc".into())
}

/// `/?` text of the waiting launcher, named as invoked.
fn wait_usage(name: &str) -> String {
    let usage = bc_script::cli::usage();
    let usage = if name == "matcha" {
        usage
    } else {
        usage.replacen("matcha", name, 2)
    };
    format!(
        "{usage}\n{name} waits until the comparison is closed and returns its exit code.\n\
         Double-click {MAIN_EXECUTABLE} (or use matcha-comp) to open the GUI without waiting.\n"
    )
}

fn is_help(arg: &str) -> bool {
    let body = arg
        .strip_prefix("--")
        .or_else(|| arg.strip_prefix('-'))
        .or_else(|| arg.strip_prefix('/'));
    matches!(
        body.map(str::to_ascii_lowercase).as_deref(),
        Some("?" | "h" | "help")
    )
}

/// 子进程退出状态 → 参考表中的退出码；表外的值（如被强杀）算未知错误。
fn translate(status: Option<i32>) -> ExitCode {
    status
        .and_then(ExitCode::from_code)
        .unwrap_or(ExitCode::UnknownError)
}

/// Run the launcher with the process arguments (without argv[0]).
pub fn launch(mode: LaunchMode, args: Vec<String>) -> ExitCode {
    if mode == LaunchMode::WaitForExitCode && args.iter().any(|a| is_help(a)) {
        print!("{}", wait_usage(&invoked_name()));
        return ExitCode::Success;
    }
    let main = match locate_main_executable() {
        Ok(main) => main,
        Err(code) => {
            eprintln!("cannot find {MAIN_EXECUTABLE} next to the launcher or on PATH");
            return code;
        }
    };
    let mut command = Command::new(&main);
    command.args(&args);
    match mode {
        LaunchMode::ReturnImmediately => {
            command.env_remove(bc_instance::WAIT_ENV);
            match command.spawn() {
                Ok(_) => ExitCode::Success,
                Err(e) => {
                    eprintln!("cannot start {}: {e}", main.display());
                    ExitCode::LauncherCannotFindMain
                }
            }
        }
        LaunchMode::WaitForExitCode => {
            command.env(bc_instance::WAIT_ENV, "1");
            let mut child = match command.spawn() {
                Ok(child) => child,
                Err(e) => {
                    eprintln!("cannot start {}: {e}", main.display());
                    return ExitCode::LauncherCannotFindMain;
                }
            };
            match child.wait() {
                Ok(status) => translate(status.code()),
                Err(e) => {
                    eprintln!("cannot wait for {}: {e}", main.display());
                    ExitCode::LauncherCannotWait
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_executable_is_found_beside_the_launcher_before_path() {
        let root = std::env::temp_dir().join(format!("matcha-locate-{}", std::process::id()));
        let (beside, on_path) = (root.join("bin"), root.join("path"));
        for dir in [&beside, &on_path] {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(dir.join(MAIN_EXECUTABLE), b"").unwrap();
        }
        let path = std::env::join_paths([root.join("nowhere"), on_path.clone()]).ok();
        let launcher = |dir: &Path| dir.join("matcha.com");
        assert_eq!(
            locate_in(Some(&launcher(&beside)), path.clone()),
            Ok(beside.join(MAIN_EXECUTABLE))
        );
        assert_eq!(
            locate_in(Some(&launcher(&root)), path),
            Ok(on_path.join(MAIN_EXECUTABLE))
        );
        assert_eq!(
            locate_in(Some(&launcher(&root)), None),
            Err(ExitCode::LauncherCannotFindMain)
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_launcher_never_starts_itself() {
        // Unix 布局里启动器（经 `bin/matcha` 链接）与主程序同名：PATH 上找到的若是它自己，
        // 必须跳过。这里直接让启动器路径就是 PATH 上的 `MAIN_EXECUTABLE`。
        let root = std::env::temp_dir().join(format!("matcha-self-{}", std::process::id()));
        let (bin, other) = (root.join("bin"), root.join("other"));
        for dir in [&bin, &other] {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(dir.join(MAIN_EXECUTABLE), b"").unwrap();
        }
        let me = bin.join(MAIN_EXECUTABLE);
        let only_me = std::env::join_paths([bin.clone()]).ok();
        assert_eq!(
            locate_in(Some(&me), only_me),
            Err(ExitCode::LauncherCannotFindMain)
        );
        let both = std::env::join_paths([bin.clone(), other.clone()]).ok();
        assert_eq!(locate_in(Some(&me), both), Ok(other.join(MAIN_EXECUTABLE)));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn usage_is_named_after_the_invoked_launcher() {
        let shipped = wait_usage("matcha");
        assert!(shipped.starts_with("Usage: matcha ["));
        assert!(shipped.contains("\nmatcha waits until"));
        assert!(!shipped.contains("matcha-compc"));
        let built = wait_usage("matcha-compc");
        assert!(built.starts_with("Usage: matcha-compc ["));
        assert!(built.contains("\nmatcha-compc waits until"));
    }

    #[test]
    fn help_switches_and_exit_status_translation() {
        assert!(["/?", "-h", "--help", "/HELP"].iter().all(|a| is_help(a)));
        assert!(!is_help("help.txt"));
        assert_eq!(translate(Some(13)), ExitCode::RulesBasedDifferent);
        assert_eq!(translate(Some(0)), ExitCode::Success);
        assert_eq!(translate(Some(42)), ExitCode::UnknownError);
        assert_eq!(translate(None), ExitCode::UnknownError);
    }
}
