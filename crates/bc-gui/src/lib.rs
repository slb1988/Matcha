//! # bc-gui
//!
//! eframe 应用外壳与基于 TextMap 的正式虚拟化文本比较控件。
//! 引擎不依赖 GUI；基准沿用 --bench 与 bench.* 协议。
//!
//! | module | BC classes |
//! |---|---|
//! | [`app`] | `TBcApplication`, `TBcView`, `TEmptyView`, main form |
//! | [`home`] | `THomeView`, `THomeState`, `THomeActions`, `THomeCommands` |
//! | [`text`] | `TUiTextEditor`, `TUiTextBody`, `TUiTextGutter`, `TUiTextRuler`, `TUiTextNav`, `TTextCompareView` |
//! | [`bench`] | 正式控件滚动基准 |
#![forbid(unsafe_code)]

pub mod app;
pub mod bench;
pub mod home;
pub mod text;

use std::path::PathBuf;

/// What to open at start-up (filled by the `matcha` binary from the command line).
#[derive(Debug, Clone, Default)]
pub struct LaunchOptions {
    pub left: Option<PathBuf>,
    pub right: Option<PathBuf>,
    /// Run the scroll benchmark and exit.
    pub bench: Option<bench::BenchOptions>,
    /// Capture this application's own framebuffer as BMP, even on a locked desktop.
    pub screenshot: Option<PathBuf>,
}

/// Start the native window. Blocks until the window closes.
pub fn run(options: LaunchOptions) -> Result<(), eframe::Error> {
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
