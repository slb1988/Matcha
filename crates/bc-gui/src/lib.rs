//! # bc-gui
//!
//! GUI shell. Scaffold milestone: an eframe/egui application with
//!
//! * a Home placeholder page (`THomeView` / `ViewHome`), and
//! * a dual-pane text view prototype (`TUiTextEditor` / `TTextCompareView`) whose only
//!   job is to prove that virtual scrolling over a ≥100 MB file is smooth.
//!
//! Findings are recorded in `docs/ui-framework-decision.md`. Nothing here is final UI;
//! the real views (`TextView`, `DirTreeView`, `PixView`, …) are added after the framework
//! decision. Engine crates never depend on this crate.
//!
//! | module | BC classes |
//! |---|---|
//! | [`app`] | `TBcApplication`, `TBcView`, `TEmptyView`, main form |
//! | [`home`] | `THomeView`, `THomeState`, `THomeActions`, `THomeCommands` |
//! | [`text_view`] | `TUiTextEditor`, `TUiTextBody`, `TUiTextGutter`, `TUiTextRuler`, `TUiTextNav`, `TTextCompareView` |
//! | [`bench`] | prototype-only scroll benchmark driver |
#![deny(unsafe_code)] // one audited `unsafe` (mmap) in text_view.rs

pub mod app;
pub mod bench;
pub mod home;
pub mod text_view;
pub mod dir_view;
pub mod sync_view;
pub mod bench_dir;
pub mod legend;
pub mod log_panel;
pub mod filters_bar;
pub mod dialogs;

use std::path::PathBuf;

/// What to open at start-up (filled by the `matcha` binary from the command line).
#[derive(Debug, Clone, Default)]
pub struct LaunchOptions {
    pub left: Option<PathBuf>,
    pub right: Option<PathBuf>,
    /// Run the scroll benchmark and exit.
    pub bench: Option<bench::BenchOptions>,
    /// Benchmark a loaded folder tree using the real native view.
    pub bench_dir: bool,
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
