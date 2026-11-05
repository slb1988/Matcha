//! Application shell – BC `TBcApplication` + main form: top bar, active view, status bar.

use crate::bench::{BenchState, Step};
use crate::home::{HomeAction, HomeView};
use crate::text_view::TextCompareView;
use crate::LaunchOptions;
use eframe::egui;
use std::time::Instant;

/// Active view (`TBcView` subclasses).
pub enum View {
    Home(HomeView),
    TextCompare(TextCompareView),
}

/// Top-level egui application.
pub struct MatchaApp {
    view: View,
    bench: Option<BenchState>,
    last_update_ms: f64,
    frame_cpu_ms: Option<f64>,
}

impl MatchaApp {
    pub fn new(cc: &eframe::CreationContext<'_>, options: LaunchOptions) -> Self {
        // Monospace rows are the hot path; keep the default fonts but a slightly larger
        // monospace size so line heights are realistic for a diff tool.
        cc.egui_ctx.all_styles_mut(|style| {
            style.text_styles.insert(
                egui::TextStyle::Monospace,
                egui::FontId::new(13.0, egui::FontFamily::Monospace),
            );
        });

        let mut home = HomeView::new(options.left.as_ref(), options.right.as_ref());
        let view = match (&options.left, &options.right) {
            (Some(l), Some(r)) => match TextCompareView::open(l, r) {
                Ok(v) => View::TextCompare(v),
                Err(e) => {
                    home.set_error(format!("cannot open: {e}"));
                    View::Home(home)
                }
            },
            _ => View::Home(home),
        };

        let bench = match (&view, options.bench) {
            (View::TextCompare(_), Some(b)) => Some(BenchState::new(b)),
            (_, Some(_)) => {
                eprintln!("--bench requires two readable files; running interactively");
                None
            }
            _ => None,
        };

        Self { view, bench, last_update_ms: 0.0, frame_cpu_ms: None }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("top_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Home").clicked() {
                    self.view = View::Home(HomeView::default());
                }
                ui.separator();
                match &self.view {
                    View::Home(_) => ui.label("Home"),
                    View::TextCompare(_) => ui.label("Text Compare (prototype)"),
                };
            });
        });
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        egui::Panel::bottom("status_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("update {:.2} ms", self.last_update_ms));
                if let Some(cpu) = self.frame_cpu_ms {
                    ui.label(format!("frame cpu {cpu:.2} ms"));
                }
                if let View::TextCompare(v) = &self.view {
                    ui.label(format!("rows drawn {}", v.last_stats.rows_drawn));
                    ui.label(format!("total rows {}", v.row_count()));
                }
                if self.bench.is_some() {
                    ui.colored_label(egui::Color32::YELLOW, "benchmark running");
                }
            });
        });
    }
}

impl eframe::App for MatchaApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let started = Instant::now();
        let ctx = ui.ctx().clone();
        self.frame_cpu_ms = frame.info().cpu_usage.map(|s| s as f64 * 1e3);

        // Drive the benchmark before drawing so the scroll target applies this frame.
        if let (Some(bench), View::TextCompare(view)) = (&mut self.bench, &mut self.view) {
            let row_height = ctx.global_style().text_styles[&egui::TextStyle::Monospace].size + 2.0;
            match bench.next_step(view.row_count()) {
                Step::Smooth(n) => {
                    let top = view.top_row(row_height);
                    view.scroll_to_row(top + n);
                }
                Step::Jump(row) => view.scroll_to_row(row),
            }
        }

        self.top_bar(ui);
        self.status_bar(ui);

        let mut action = None;
        egui::CentralPanel::default().show(ui, |ui| match &mut self.view {
            View::Home(home) => action = home.ui(ui),
            View::TextCompare(view) => view.ui(ui),
        });

        if let Some(HomeAction::OpenTextCompare { left, right }) = action {
            match TextCompareView::open(&left, &right) {
                Ok(v) => self.view = View::TextCompare(v),
                Err(e) => {
                    if let View::Home(home) = &mut self.view {
                        home.set_error(format!("cannot open: {e}"));
                    }
                }
            }
        }

        self.last_update_ms = started.elapsed().as_secs_f64() * 1e3;

        let mut finished = false;
        if let (Some(bench), View::TextCompare(view)) = (&mut self.bench, &self.view) {
            bench.record(started.elapsed(), frame.info().cpu_usage, view.last_stats.rows_drawn);
            if bench.is_done() {
                println!(
                    "{}",
                    bench.summary(
                        view.left.byte_len(),
                        view.right.byte_len(),
                        view.left.load_time,
                        view.right.load_time,
                        view.left.index_bytes() + view.right.index_bytes(),
                    )
                );
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                finished = true;
            } else {
                ctx.request_repaint();
            }
        }
        if finished {
            // Report exactly once even if the window needs another frame to close.
            self.bench = None;
        }
    }
}
