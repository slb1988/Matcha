//! 命令行路由：开发用 `--` 开关的预处理，以及“自己开窗口 / 转交常驻实例 / 报错”的判定。
//! 纯逻辑，无 I/O，便于单测。
use bc_gui::bench::BenchOptions;
use bc_script::cli::{CliArgs, Invocation, Switch};
use std::path::PathBuf;

/// 开发/测试用开关（不属于参考命令行）：`--bench[=N]`、`--bench-dir`、`--bench-table`、
/// `--snapshot=<scene>`、`--snapshot-out=<png>`、`--window-size=WxH`、`--screenshot=<bmp>`。
/// 在交给 `CliArgs::parse` 之前剥离，其余参数原样保留。
#[derive(Debug, Default)]
pub struct DevFlags {
    pub bench: Option<BenchOptions>,
    pub bench_table: bool,
    pub bench_dir: bool,
    pub snapshot: Option<String>,
    pub snapshot_out: Option<PathBuf>,
    pub window_size: Option<[f32; 2]>,
    pub screenshot: Option<PathBuf>,
}

impl DevFlags {
    /// 任一开发开关都要求独立进程：基准/快照不能落到别人的窗口里，也不能被单实例阻塞。
    pub fn any(&self) -> bool {
        self.bench.is_some()
            || self.bench_table
            || self.bench_dir
            || self.snapshot.is_some()
            || self.snapshot_out.is_some()
            || self.window_size.is_some()
            || self.screenshot.is_some()
    }

    /// 合成 GUI 启动选项。
    pub fn into_launch(self, pair: Option<(PathBuf, PathBuf)>) -> bc_gui::LaunchOptions {
        let (left, right) = pair.unzip();
        bc_gui::LaunchOptions {
            left,
            right,
            bench: self.bench,
            bench_table: self.bench_table,
            bench_dir: self.bench_dir,
            screenshot: self.screenshot,
            snapshot: self.snapshot,
            snapshot_out: self.snapshot_out,
            window_size: self.window_size,
        }
    }
}

pub const DEV_USAGE: &str = "\
Developer options:
  --bench[=<frames>]      Scroll benchmark on <left> <right>, then exit
  --bench-table           Two-dimensional table benchmark
  --bench-dir             Folder view benchmark on two folders
  --screenshot=<file.bmp> Capture the window once ready
  --snapshot=<scene>      Deterministic snapshot scene
  --snapshot-out=<png>    Snapshot destination
  --window-size=WxH       Client size in physical pixels
Developer options always run a separate instance (no single-instance hand-off).
";

/// 剥离开发开关；返回剩余参数（顺序不变）。
pub fn split_dev_flags(
    args: impl IntoIterator<Item = String>,
) -> Result<(DevFlags, Vec<String>), String> {
    let mut dev = DevFlags::default();
    let mut rest = Vec::new();
    for arg in args {
        if arg == "--bench-dir" {
            dev.bench_dir = true;
        } else if arg == "--bench-table" {
            dev.bench_table = true;
            dev.bench = Some(BenchOptions::default());
        } else if arg == "--bench" || arg.starts_with("--bench=") {
            let mut bench = BenchOptions::default();
            if let Some(n) = arg.strip_prefix("--bench=") {
                bench.frames = n
                    .parse()
                    .map_err(|_| format!("--bench expects a frame count, got {n:?}"))?;
            }
            dev.bench = Some(bench);
        } else if let Some(scene) = arg.strip_prefix("--snapshot=") {
            if !bc_gui::snapshot::is_scene(Some(scene)) {
                return Err(format!("unknown snapshot scene: {scene}"));
            }
            dev.snapshot = Some(scene.to_string());
        } else if let Some(path) = arg.strip_prefix("--snapshot-out=") {
            if path.is_empty() {
                return Err("--snapshot-out requires a PNG path".into());
            }
            dev.snapshot_out = Some(PathBuf::from(path));
        } else if let Some(size) = arg.strip_prefix("--window-size=") {
            dev.window_size = Some(parse_window_size(size)?);
        } else if let Some(path) = arg.strip_prefix("--screenshot=") {
            dev.screenshot = Some(PathBuf::from(path));
        } else {
            rest.push(arg);
        }
    }
    if dev.snapshot_out.is_some() && dev.snapshot.is_none() {
        return Err("--snapshot-out requires --snapshot=<scene>".into());
    }
    Ok((dev, rest))
}

/// 客户区物理像素，拒绝非有限、零、溢出及不完整的尺寸。
pub fn parse_window_size(value: &str) -> Result<[f32; 2], String> {
    let invalid = || format!("--window-size expects WxH (64..=8192), got {value:?}");
    let (w, h) = value.split_once('x').ok_or_else(invalid)?;
    let w = w.parse::<u32>().map_err(|_| invalid())?;
    let h = h.parse::<u32>().map_err(|_| invalid())?;
    if !(64..=8192).contains(&w) || !(64..=8192).contains(&h) {
        return Err(invalid());
    }
    Ok([w as f32, h as f32])
}

/// 本次启动怎么处理。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route {
    /// `/?`：打印用法。
    Help,
    /// 自己开窗口，不参与单实例（开发开关、`/solo`、性能计时环境变量）。
    Standalone,
    /// 默认：已有实例则转交，否则成为常驻实例。
    SingleInstance,
    /// 参考命令行支持、GUI 尚未实现的调用；退出码 100。
    Unsupported(String),
}

/// 判定路由。`perf_env` = 设置了 `MATCHA_*PERF*` 计时环境变量（计时运行必须独立）。
pub fn route(dev: &DevFlags, cli: &CliArgs, perf_env: bool) -> Route {
    if cli.has(|s| matches!(s, Switch::Help)) {
        return Route::Help;
    }
    if let Some(reason) = unsupported(cli) {
        return Route::Unsupported(reason);
    }
    if dev.any() || perf_env || cli.solo() {
        Route::Standalone
    } else {
        Route::SingleInstance
    }
}

/// 尚未实现的调用（`/qc`、脚本、合并、会话名、补丁、包、stdin）。
pub fn unsupported(cli: &CliArgs) -> Option<String> {
    if cli.has(|s| matches!(s, Switch::QuickCompare(_))) {
        return Some("/qc quick compare is not implemented yet".into());
    }
    let what = match &cli.invocation {
        Invocation::Home
        | Invocation::Files(..)
        | Invocation::Folders(..)
        | Invocation::Single(_) => return None,
        Invocation::Session(name) => format!("opening saved session {name:?}"),
        Invocation::Workspace(name) => format!("opening workspace {name:?}"),
        Invocation::Merge3 { .. } | Invocation::Merge4 { .. } => "Text Merge".into(),
        Invocation::Script { .. } => "scripts".into(),
        Invocation::Package(_) => "settings packages".into(),
        Invocation::Patch(_) => "Text Patch".into(),
        Invocation::Stdin => "reading stdin".into(),
    };
    Some(format!("{what} is not supported yet"))
}

/// 两侧路径（文件或文件夹）；其余调用（Home、单个路径）没有比较可开。
pub fn comparison_pair(invocation: &Invocation) -> Option<(PathBuf, PathBuf)> {
    match invocation {
        Invocation::Files(l, r) | Invocation::Folders(l, r) => Some((l.clone(), r.clone())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }
    fn routed(args: &[&str]) -> Route {
        let (dev, rest) = split_dev_flags(strings(args)).unwrap();
        let cli = CliArgs::parse_in(&rest, Path::new("C:/work")).unwrap();
        route(&dev, &cli, false)
    }

    #[test]
    fn window_size_validates_both_client_dimensions() {
        assert_eq!(parse_window_size("1384x961").unwrap(), [1384.0, 961.0]);
        for bad in ["0x961", "-1x20", "1xNaN", "100", "100x100x100", "99999x100"] {
            assert!(parse_window_size(bad).is_err());
        }
    }

    #[test]
    fn dev_flags_are_stripped_and_the_rest_keeps_its_order() {
        let (dev, rest) = split_dev_flags(strings(&[
            "a",
            "--bench=30",
            "/ro",
            "b",
            "--window-size=800x600",
        ]))
        .unwrap();
        assert_eq!(dev.bench.unwrap().frames, 30);
        assert_eq!(dev.window_size, Some([800.0, 600.0]));
        assert_eq!(rest, strings(&["a", "/ro", "b"]));
        assert!(split_dev_flags(strings(&["--bench=x"])).is_err());
        assert!(split_dev_flags(strings(&["--snapshot-out=a.png"])).is_err());
        assert!(split_dev_flags(strings(&["--snapshot=no-such-scene"])).is_err());
    }

    #[test]
    fn plain_comparisons_and_home_join_the_running_instance() {
        assert_eq!(routed(&[]), Route::SingleInstance);
        assert_eq!(routed(&["a.txt", "b.txt"]), Route::SingleInstance);
        assert_eq!(routed(&["/ro", "a.txt", "b.txt"]), Route::SingleInstance);
    }

    #[test]
    fn solo_dev_flags_and_perf_runs_never_hand_off() {
        assert_eq!(routed(&["/solo", "a.txt", "b.txt"]), Route::Standalone);
        assert_eq!(routed(&["-solo"]), Route::Standalone);
        assert_eq!(routed(&["a", "b", "--bench"]), Route::Standalone);
        assert_eq!(routed(&["--snapshot=home"]), Route::Standalone);
        assert_eq!(routed(&["--screenshot=x.bmp", "a", "b"]), Route::Standalone);
        let cli = CliArgs::parse_in(["a", "b"], Path::new(".")).unwrap();
        assert_eq!(route(&DevFlags::default(), &cli, true), Route::Standalone);
    }

    #[test]
    fn help_and_unsupported_invocations() {
        assert_eq!(routed(&["/?"]), Route::Help);
        assert_eq!(routed(&["--help"]), Route::Help);
        assert_eq!(routed(&["/solo", "-h"]), Route::Help);
        for args in [
            &["/qc", "a", "b"][..],
            &["a", "b", "c"],
            &["@script.txt"],
            &["MySession"],
            &["x.patch"],
            &["-"],
        ] {
            assert!(
                matches!(routed(args), Route::Unsupported(_)),
                "{args:?} should be unsupported"
            );
        }
    }

    #[test]
    fn only_two_paths_make_a_comparison_pair() {
        let pair = |args: &[&str]| {
            comparison_pair(
                &CliArgs::parse_in(args, Path::new("C:/w"))
                    .unwrap()
                    .invocation,
            )
        };
        assert_eq!(
            pair(&["a", "b"]),
            Some((Path::new("C:/w").join("a"), Path::new("C:/w").join("b")))
        );
        assert_eq!(pair(&[]), None);
        assert_eq!(pair(&["C:/w/a"]), None);
    }
}
