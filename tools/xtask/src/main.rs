//! `cargo xtask dist [--debug] [--target <triple>] [--no-build]`
//!
//! 组装发行目录 `target/dist/matcha-<version>-<platform>/`。三个可执行文件来自两个包：
//! `bcompare`（`matcha`，GUI 主程序）与 `bcomp`（`matcha-comp` 立即返回、`matcha-compc` 等待）。
//!
//! 为什么需要这一步：Windows 的子系统在链接期写死，“双击不弹控制台”与“命令行里被 shell
//! 等待、能打印、能读退出码”只能分给两个文件。参考实现的做法是同名不同后缀：
//!
//! | 文件 | 子系统 | 用途 |
//! |---|---|---|
//! | `matcha.exe` | GUI | 双击 / Explorer 菜单 / 快捷方式：直接出窗口 |
//! | `matcha.com` | 控制台 | 命令行敲 `matcha …`（`PATHEXT` 中 `.COM` 先于 `.EXE`）：等待、输出、退出码 |
//! | `matcha-comp.exe` | GUI | VCS 集成：转交后立即返回 |
//!
//! Cargo 只能产出 `.exe`，所以 `matcha.com` 是 `matcha-compc.exe` 的改名副本（Windows 按
//! PE 头而非扩展名加载）。
//!
//! macOS 没有子系统之分，但 GUI 主程序与命令行工具仍同名（`matcha`），所以分处两地：
//! `Matcha.app/Contents/MacOS/` 放三个二进制，`bin/matcha` → `matcha-compc`、
//! `bin/matcha-comp` → `matcha-comp` 两个相对符号链接供加入 `PATH`（或再链到
//! `/usr/local/bin`）。启动器按解析链接后的真实位置找同目录的主程序。签名、公证、图标、
//! 通过 LaunchServices 激活窗口等留待 macOS 支持落地时补齐。
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const VERSION: &str = env!("CARGO_PKG_VERSION");
/// 待 macOS 发行前确认的占位标识。
const MAC_BUNDLE_ID: &str = "dev.matcha.Matcha";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Platform {
    Windows,
    MacOs,
}

impl Platform {
    fn host() -> Result<Self, String> {
        Self::from_os(std::env::consts::OS)
    }

    fn from_target(triple: &str) -> Result<Self, String> {
        if triple.contains("windows") {
            Ok(Platform::Windows)
        } else if triple.contains("apple-darwin") {
            Ok(Platform::MacOs)
        } else {
            Err(format!(
                "no distribution layout for target {triple:?} (windows, apple-darwin)"
            ))
        }
    }

    fn from_os(os: &str) -> Result<Self, String> {
        match os {
            "windows" => Ok(Platform::Windows),
            "macos" => Ok(Platform::MacOs),
            other => Err(format!(
                "no distribution layout for {other:?} yet (windows, macos)"
            )),
        }
    }

    fn exe_suffix(self) -> &'static str {
        match self {
            Platform::Windows => ".exe",
            Platform::MacOs => "",
        }
    }

    fn tag(self) -> &'static str {
        match self {
            Platform::Windows => "windows",
            Platform::MacOs => "macos",
        }
    }
}

/// 发行目录里的一个条目（`dest` 相对发行根目录）。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    dest: PathBuf,
    source: Source,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Source {
    /// Cargo 构建产物（bin 名，不含后缀）。
    Binary(&'static str),
    Text(String),
    /// 相对符号链接（目标相对链接所在目录）。
    Symlink(PathBuf),
}

fn entry(dest: impl Into<PathBuf>, source: Source) -> Entry {
    Entry {
        dest: dest.into(),
        source,
    }
}

fn layout(platform: Platform) -> Vec<Entry> {
    use Source::*;
    match platform {
        Platform::Windows => vec![
            entry("matcha.exe", Binary("matcha")),
            entry("matcha.com", Binary("matcha-compc")),
            entry("matcha-comp.exe", Binary("matcha-comp")),
            entry("README.txt", Text(WINDOWS_README.into())),
        ],
        Platform::MacOs => {
            let macos = Path::new("Matcha.app/Contents/MacOS");
            let from_bin = Path::new("..").join(macos);
            vec![
                entry("Matcha.app/Contents/Info.plist", Text(info_plist())),
                entry(macos.join("matcha"), Binary("matcha")),
                entry(macos.join("matcha-comp"), Binary("matcha-comp")),
                entry(macos.join("matcha-compc"), Binary("matcha-compc")),
                entry("bin/matcha", Symlink(from_bin.join("matcha-compc"))),
                entry("bin/matcha-comp", Symlink(from_bin.join("matcha-comp"))),
            ]
        }
    }
}

const WINDOWS_README: &str = "\
Matcha\r
\r
  matcha.exe       Double-click (or Explorer menu / shortcut): opens the GUI, no console.\r
  matcha.com       Command line. With this folder on PATH, typing `matcha ...` in cmd or\r
                   PowerShell runs it (PATHEXT tries .COM before .EXE): it waits for the\r
                   comparison, prints text to the console (redirectable with > file) and\r
                   returns the exit code. In Git Bash type `matcha.com ...`.\r
  matcha-comp.exe  Hands the comparison to the GUI and returns at once (VCS integration).\r
\r
`matcha /?` lists the switches.\r
";

fn info_plist() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>Matcha</string>
    <key>CFBundleDisplayName</key><string>Matcha</string>
    <key>CFBundleIdentifier</key><string>{MAC_BUNDLE_ID}</string>
    <key>CFBundleExecutable</key><string>matcha</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleVersion</key><string>{VERSION}</string>
    <key>CFBundleShortVersionString</key><string>{VERSION}</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
"#
    )
}

/// 把 `entries` 落到 `out`（`bins` 为构建产物目录）。
fn assemble(entries: &[Entry], platform: Platform, bins: &Path, out: &Path) -> Result<(), String> {
    let io = |what: &str, path: &Path, e: std::io::Error| format!("{what} {}: {e}", path.display());
    for Entry { dest, source } in entries {
        let dest = out.join(dest);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io("create", parent, e))?;
        }
        match source {
            Source::Binary(name) => {
                let from = bins.join(format!("{name}{}", platform.exe_suffix()));
                std::fs::copy(&from, &dest).map_err(|e| io("copy", &from, e))?;
            }
            Source::Text(text) => std::fs::write(&dest, text).map_err(|e| io("write", &dest, e))?,
            Source::Symlink(target) => symlink(target, &dest).map_err(|e| io("link", &dest, e))?,
        }
    }
    Ok(())
}

#[cfg(unix)]
fn symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(not(unix))]
fn symlink(_: &Path, _: &Path) -> std::io::Result<()> {
    Err(std::io::Error::other(
        "symbolic links of the macOS layout must be created on macOS",
    ))
}

struct Options {
    release: bool,
    target: Option<String>,
    build: bool,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        release: true,
        target: None,
        build: true,
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--debug" => options.release = false,
            "--no-build" => options.build = false,
            "--target" => {
                options.target = Some(args.next().ok_or("--target requires a triple")?.clone())
            }
            other => return Err(format!("unknown option {other:?}")),
        }
    }
    Ok(options)
}

fn workspace_root() -> PathBuf {
    // tools/xtask → 仓库根
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("xtask lives at tools/xtask")
        .to_path_buf()
}

fn dist(args: &[String]) -> Result<PathBuf, String> {
    let options = parse(args)?;
    let platform = match &options.target {
        Some(triple) => Platform::from_target(triple)?,
        None => Platform::host()?,
    };
    let root = workspace_root();
    if options.build {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut command = Command::new(cargo);
        command
            .current_dir(&root)
            .args(["build", "-p", "bcompare", "-p", "bcomp"]);
        if options.release {
            command.arg("--release");
        }
        if let Some(triple) = &options.target {
            command.args(["--target", triple]);
        }
        let status = command
            .status()
            .map_err(|e| format!("cannot run cargo: {e}"))?;
        if !status.success() {
            return Err(format!("cargo build failed ({status})"));
        }
    }
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    let mut bins = target_dir.clone();
    if let Some(triple) = &options.target {
        bins.push(triple);
    }
    bins.push(if options.release { "release" } else { "debug" });
    let arch = match &options.target {
        Some(triple) => triple.split('-').next().unwrap_or("unknown"),
        None => std::env::consts::ARCH,
    };
    let out = target_dir
        .join("dist")
        .join(format!("matcha-{VERSION}-{}-{arch}", platform.tag()));
    clean(&out)?;
    assemble(&layout(platform), platform, &bins, &out)?;
    Ok(out)
}

/// Empty `out` but keep the folder: an Explorer window or a shell sitting in it holds the
/// directory open, which would make removing the folder itself fail.
fn clean(out: &Path) -> Result<(), String> {
    let Ok(entries) = std::fs::read_dir(out) else {
        return Ok(());
    };
    for entry in entries {
        let path = entry
            .map_err(|e| format!("clean {}: {e}", out.display()))?
            .path();
        let removed = if path.is_dir() && !path.is_symlink() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        removed.map_err(|e| format!("clean {}: {e}", path.display()))?;
    }
    Ok(())
}

const USAGE: &str = "usage: cargo xtask dist [--debug] [--target <triple>] [--no-build]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("dist") => dist(&args[1..]),
        _ => Err(USAGE.into()),
    };
    match result {
        Ok(out) => {
            println!("{}", out.display());
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("xtask: {message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dests(platform: Platform) -> Vec<String> {
        layout(platform)
            .iter()
            .map(|e| e.dest.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    #[test]
    fn windows_ships_the_console_launcher_as_matcha_com() {
        let entries = layout(Platform::Windows);
        let find = |name: &str| entries.iter().find(|e| e.dest == Path::new(name)).cloned();
        assert_eq!(find("matcha.exe").unwrap().source, Source::Binary("matcha"));
        assert_eq!(
            find("matcha.com").unwrap().source,
            Source::Binary("matcha-compc")
        );
        assert_eq!(
            find("matcha-comp.exe").unwrap().source,
            Source::Binary("matcha-comp")
        );
        assert!(!dests(Platform::Windows)
            .iter()
            .any(|d| d.contains("matcha-compc")));
    }

    #[test]
    fn macos_bundles_the_binaries_and_links_the_command_line_tools() {
        assert_eq!(
            dests(Platform::MacOs),
            [
                "Matcha.app/Contents/Info.plist",
                "Matcha.app/Contents/MacOS/matcha",
                "Matcha.app/Contents/MacOS/matcha-comp",
                "Matcha.app/Contents/MacOS/matcha-compc",
                "bin/matcha",
                "bin/matcha-comp",
            ]
        );
        let entries = layout(Platform::MacOs);
        let Source::Symlink(target) = &entries[4].source else {
            panic!("bin/matcha must be a symlink");
        };
        assert_eq!(
            target.to_string_lossy().replace('\\', "/"),
            "../Matcha.app/Contents/MacOS/matcha-compc"
        );
        let Source::Text(plist) = &entries[0].source else {
            panic!("Info.plist must be generated");
        };
        assert!(plist.contains("<key>CFBundleExecutable</key><string>matcha</string>"));
        assert!(plist.contains(&format!("<string>{VERSION}</string>")));
    }

    #[test]
    fn platforms_follow_the_target_triple() {
        assert_eq!(
            Platform::from_target("x86_64-pc-windows-msvc"),
            Ok(Platform::Windows)
        );
        assert_eq!(
            Platform::from_target("aarch64-apple-darwin"),
            Ok(Platform::MacOs)
        );
        assert!(Platform::from_target("x86_64-unknown-linux-gnu").is_err());
    }

    #[test]
    fn assembling_the_windows_layout_copies_and_renames() {
        let root = std::env::temp_dir().join(format!("matcha-xtask-{}", std::process::id()));
        let (bins, out) = (root.join("bins"), root.join("out"));
        std::fs::create_dir_all(&bins).unwrap();
        for name in ["matcha", "matcha-comp", "matcha-compc"] {
            std::fs::write(bins.join(format!("{name}.exe")), name).unwrap();
        }
        assemble(&layout(Platform::Windows), Platform::Windows, &bins, &out).unwrap();
        let read = |name: &str| std::fs::read_to_string(out.join(name)).unwrap();
        assert_eq!(read("matcha.exe"), "matcha");
        assert_eq!(read("matcha.com"), "matcha-compc");
        assert_eq!(read("matcha-comp.exe"), "matcha-comp");
        assert!(read("README.txt").contains("PATHEXT"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
