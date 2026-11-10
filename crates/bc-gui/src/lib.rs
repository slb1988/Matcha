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
pub mod home;
pub mod perf;
pub mod table;
pub mod text;
pub mod dir_view;
pub mod sync_view;
pub mod bench_dir;
pub mod legend;
pub mod log_panel;
pub mod filters_bar;
pub mod dialogs;
pub mod chrome;
pub mod snapshot;

use std::path::PathBuf;

/// What to open at start-up (filled by the `matcha` binary from the command line).
#[derive(Debug, Clone, Default)]
pub struct LaunchOptions {
    pub left: Option<PathBuf>,
    pub right: Option<PathBuf>,
    /// Run the scroll benchmark and exit.
    pub bench: Option<bench::BenchOptions>,
    /// Run the 600-frame two-dimensional table benchmark.
    pub bench_table: bool,
    /// Benchmark a loaded folder tree using the real native view.
    pub bench_dir: bool,
    /// Capture this application's own framebuffer as BMP, even on a locked desktop.
    pub screenshot: Option<PathBuf>,
    pub snapshot: Option<String>,
    pub window_size: Option<[f32; 2]>,
}

/// Start the native window. Blocks until the window closes.
pub fn run(options: LaunchOptions) -> Result<(), eframe::Error> {
    perf::mark("run:enter");
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_title("Matcha"),
        ..Default::default()
    };
    eframe::run_native(
        "matcha",
        native,
        Box::new(move |cc| Ok(Box::new(app::MatchaApp::new(cc, options)))),
    )
}

pub use eframe::egui;


