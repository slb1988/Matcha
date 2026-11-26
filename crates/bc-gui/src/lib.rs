//! # bc-gui
//!
//! eframe 应用外壳、基于 TextMap 的正式虚拟化文本比较控件，以及文件夹比较/同步视图。
//! 引擎不依赖 GUI；基准沿用 --bench 与 bench.* 协议，文件夹基准为 --bench-dir。
//!
//! | module | BC classes |
//! |---|---|
//! | [`app`] | `TBcApplication`, `TBcView`, `TEmptyView`, main form |
//! | [`home`] | `THomeView`, `THomeState`, `THomeActions`, `THomeCommands` |
//! | [`text`] | `TUiTextEditor`, `TUiTextBody`, `TUiTextGutter`, `TUiTextRuler`, `TUiTextNav`, `TTextCompareView` |
//! | [`bench`] | 正式控件滚动基准 |
//! | [`dir_view`] / [`sync_view`] | `TDirCompareView`, `TDirSyncView`, `TDirTreeView`, `TDirSyncTreeView` |
//! | [`bench_dir`] | 文件夹视图原生窗口基准 |
#![forbid(unsafe_code)]

pub mod app;
pub mod bench;
pub mod bench_dir;
pub mod chrome;
pub mod dialogs;
pub mod dir_view;
pub mod disk_watch;
pub mod filters_bar;
pub mod hex;
pub mod home;
pub mod layout;
pub mod legend;
pub mod log_panel;
pub mod merge;
pub mod perf;
pub mod remote;
pub mod sessions;
pub mod settings;
pub mod snapshot;
pub mod sync_view;
pub mod table;
pub mod table_merge;
pub mod text;
pub mod using;

use std::path::PathBuf;

/// What to open at start-up (filled by the `matcha` binary from the command line).
#[derive(Debug, Clone, Default)]
pub struct LaunchOptions {
    pub left: Option<PathBuf>,
    pub right: Option<PathBuf>,
    /// 三方合并调用（3/4 个路径或 `/center=`）；有它时不看 `left` / `right`。
    pub merge: Option<merge::MergeLaunch>,
    /// `/fv=<type>` 强制会话类型（hex-compare 起生效；None = 按格式路由）。
    pub force_view: Option<bc_session::SessionKind>,
    /// 命令行 `matcha "<会话名>"`：打开该已保存会话（见 `sessions`）。
    pub session: Option<String>,
    /// Run the scroll benchmark and exit.
    pub bench: Option<bench::BenchOptions>,
    /// Run the 600-frame two-dimensional table benchmark.
    pub bench_table: bool,
    /// Benchmark a loaded folder tree using the real native view.
    pub bench_dir: bool,
    /// Capture this application's own framebuffer as BMP, even on a locked desktop.
    pub screenshot: Option<PathBuf>,
    pub snapshot: Option<String>,
    /// PNG destination for deterministic snapshot mode.
    pub snapshot_out: Option<PathBuf>,
    pub window_size: Option<[f32; 2]>,
}

/// Start the native window. Blocks until the window closes.
pub fn run(options: LaunchOptions) -> Result<(), eframe::Error> {
    run_with_remote(options, None).map(|_| ())
}

/// 同 [`run`]，并把单实例转交请求（`remote`）接入窗口：每个请求开一个新标签页。
/// 返回启动标签页的会话结果码：合并页为 0 / 14 / 101（没能打开为 100），其余为 0。
pub fn run_with_remote(
    options: LaunchOptions,
    remote: Option<remote::RemoteInbox>,
) -> Result<i32, eframe::Error> {
    perf::mark("run:enter");
    let options =
        snapshot::prepare_options(options).map_err(|e| eframe::Error::AppCreation(Box::new(e)))?;
    let snapshot_mode = options.snapshot.is_some();
    // 快照与基准要求确定的布局：不读不写用户的分隔线位置。
    if snapshot_mode || options.bench.is_some() || options.bench_table || options.bench_dir {
        layout::disable_persistence();
        settings::disable_persistence();
    }
    // Overlap the system-font lookup (and the saved editor font) with window creation.
    chrome::fonts::prefetch(settings::load().editor_font.family);
    perf::mark("run:settings-loaded");
    let result: snapshot::CaptureResult = Default::default();
    let capture_result = result.clone();
    let launch_result: remote::ResultSlot = Default::default();
    let launch_slot = launch_result.clone();
    let merge_requested = options.merge.is_some();
    let size = options.window_size.unwrap_or(if snapshot_mode {
        snapshot::DEFAULT_WINDOW_SIZE
    } else {
        [1400.0, 900.0]
    });
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size(size)
        .with_title("Matcha");
    if let Some(icon) = app_icon() {
        viewport = viewport.with_icon(icon);
    }
    perf::mark("run:icon-decoded");
    let native = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "matcha",
        native,
        Box::new(move |cc| {
            let mut app = app::MatchaApp::new(cc, options, capture_result);
            app.watch_launch(launch_slot, merge_requested);
            if let Some(inbox) = remote {
                app.attach_remote(&cc.egui_ctx, inbox);
            }
            Ok(Box::new(app))
        }),
    )?;
    perf::mark("run:window-closed");
    if snapshot_mode {
        let outcome = result
            .lock()
            .unwrap()
            .take()
            .unwrap_or_else(|| Err("snapshot window closed before PNG was saved".into()));
        outcome.map_err(|e| eframe::Error::AppCreation(Box::new(std::io::Error::other(e))))?;
    }
    // 快照运行只关心 PNG 是否写出；会话结果码只对真实的合并调用有意义。
    let code = launch_result.lock().unwrap().take();
    Ok(if snapshot_mode { 0 } else { code.unwrap_or(0) })
}

/// 窗口图标（标题栏 / 任务栏 / Alt-Tab）：抹茶叶，与 exe 内嵌的 `matcha.ico` 同源
/// （`tools/icon/gen_icon.py`）。解码失败只回退到 eframe 默认图标。
fn app_icon() -> Option<egui::IconData> {
    const PNG: &[u8] = include_bytes!("../../../assets/icon/matcha-256.png");
    eframe::icon_data::from_png_bytes(PNG).ok()
}

#[cfg(test)]
mod app_icon_tests {
    #[test]
    fn embedded_icon_decodes() {
        let icon = super::app_icon().expect("assets/icon/matcha-256.png decodes");
        assert_eq!((icon.width, icon.height), (256, 256));
    }
}

pub use eframe::egui;
