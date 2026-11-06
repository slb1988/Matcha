//! 应用接线集中于 View 和两个 action 分支，便于合并文件夹视图。
use crate::{
    bench::{BenchState, Step},
    home::{HomeAction, HomeView},
    table::{TableAction, TableCompareView},
    text::{TextCompareView, ViewAction},
    LaunchOptions,
};
use bc_session::prefs::Prefs;
use eframe::egui;
use std::time::Instant;

pub enum View {
    Home(HomeView),
    TextCompare(Box<TextCompareView>),
    TableCompare(Box<TableCompareView>),
}
pub struct MatchaApp {
    view: View,
    bench: Option<BenchState>,
    prefs: Prefs,
    last_update_ms: f64,
    frame_cpu_ms: Option<f64>,
    exit_confirmed: bool,
    screenshot: Option<std::path::PathBuf>,
    capture_requested: bool,
}
impl MatchaApp {
    pub fn new(cc: &eframe::CreationContext<'_>, options: LaunchOptions) -> Self {
        cc.egui_ctx.all_styles_mut(|style| {
            style
                .text_styles
                .insert(egui::TextStyle::Monospace, egui::FontId::monospace(13.0));
        });
        install_cjk_font(&cc.egui_ctx);
        let mut home = HomeView::new(options.left.as_ref(), options.right.as_ref());
        let view = match (&options.left, &options.right) {
            (Some(l), Some(r)) if options.bench_table || crate::home::table_paths(l, r) => {
                match TableCompareView::open(l, r) {
                    Ok(v) => {
                        HomeView::record(l, r);
                        View::TableCompare(Box::new(v))
                    }
                    Err(e) => {
                        home.set_error(format!("Cannot open: {e}"));
                        View::Home(home)
                    }
                }
            }
            (Some(l), Some(r)) => match TextCompareView::open(l, r) {
                Ok(v) => {
                    HomeView::record(l, r);
                    View::TextCompare(Box::new(v))
                }
                Err(e) => {
                    home.set_error(format!("Cannot open: {e}"));
                    View::Home(home)
                }
            },
            _ => View::Home(home),
        };
        let bench = match (&view, options.bench) {
            (View::TextCompare(_), Some(b)) | (View::TableCompare(_), Some(b)) => {
                Some(BenchState::new(b))
            }
            (_, Some(_)) => {
                eprintln!("--bench requires two readable files");
                None
            }
            _ => None,
        };
        Self {
            view,
            bench,
            prefs: Prefs::default(),
            last_update_ms: 0.0,
            frame_cpu_ms: None,
            exit_confirmed: false,
            screenshot: options.screenshot,
            capture_requested: false,
        }
    }
    fn top_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("top_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Home").clicked() {
                    match &mut self.view {
                        View::TextCompare(v) => v.request_home(),
                        View::TableCompare(v) => v.request_home(),
                        View::Home(_) => {}
                    }
                }
                ui.separator();
                ui.label(match &self.view {
                    View::Home(_) => "Home",
                    View::TextCompare(_) => "Text Compare",
                    View::TableCompare(_) => "Table Compare",
                });
            });
        });
    }
}
fn install_cjk_font(ctx: &egui::Context) {
    // System fallback, never bundled: Chinese IME commits must have visible glyphs.
    let Some(windows) = std::env::var_os("WINDIR") else {
        return;
    };
    let path = std::path::PathBuf::from(windows)
        .join("Fonts")
        .join("msyh.ttc");
    if let Ok(bytes) = std::fs::read(path) {
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "system-cjk".into(),
            egui::FontData::from_owned(bytes).into(),
        );
        for family in [egui::FontFamily::Monospace, egui::FontFamily::Proportional] {
            fonts
                .families
                .entry(family)
                .or_default()
                .push("system-cjk".into());
        }
        ctx.set_fonts(fonts);
    }
}
impl eframe::App for MatchaApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let started = Instant::now();
        let ctx = ui.ctx().clone();
        self.frame_cpu_ms = frame.info().cpu_usage.map(|s| s as f64 * 1e3);
        if !self.exit_confirmed && ctx.input(|i| i.viewport().close_requested()) {
            if let View::TextCompare(v) = &mut self.view {
                if v.is_dirty() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    v.request_exit();
                }
            }
        }
        if !self.exit_confirmed && ctx.input(|i| i.viewport().close_requested()) {
            if let View::TableCompare(v) = &mut self.view {
                if v.is_dirty() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    v.request_exit();
                }
            }
        }
        if let View::TextCompare(v) = &mut self.view {
            v.poll();
        }
        if let View::TableCompare(v) = &mut self.view {
            v.poll();
        }
        let benchmark_ready = matches!(&self.view,View::TextCompare(v) if v.is_ready())
            || matches!(&self.view,View::TableCompare(v) if v.is_ready());
        if benchmark_ready {
            if let (Some(bench), View::TextCompare(view)) = (&mut self.bench, &mut self.view) {
                match bench.next_step(view.row_count()) {
                    Step::Smooth(n) => view.scroll_to_row(view.top_row(18.0) + n),
                    Step::Jump(row) => view.scroll_to_row(row),
                }
            }
        }
        if benchmark_ready {
            if let (Some(bench), View::TableCompare(view)) = (&mut self.bench, &mut self.view) {
                match bench.next_step(view.row_count()) {
                    Step::Smooth(n) => view.scroll_to_row(view.top_row(22.0) + n),
                    Step::Jump(row) => {
                        view.scroll_to_row(row);
                        view.scroll_to_column(row % 10);
                    }
                }
            }
        }
        if benchmark_ready && !self.capture_requested {
            if let Some(path) = self.screenshot.take() {
                ctx.request_screenshot(move |image| {
                    if let Err(error) = crate::bench::save_screenshot(&path, &image) {
                        eprintln!("screenshot: {error}");
                    }
                });
                self.capture_requested = true;
            }
        }
        self.top_bar(ui);
        egui::Panel::bottom("performance_status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("update {:.2} ms", self.last_update_ms));
                if let Some(cpu) = self.frame_cpu_ms {
                    ui.label(format!("frame cpu {cpu:.2} ms"));
                }
                if let View::TextCompare(v) = &self.view {
                    ui.label(format!(
                        "rows drawn {} · total rows {}",
                        v.last_stats.rows_drawn,
                        v.row_count()
                    ));
                }
                if let View::TableCompare(v) = &self.view {
                    ui.label(format!(
                        "rows drawn {} · cells drawn {} · total rows {}",
                        v.rows_drawn(),
                        v.cells_drawn(),
                        v.row_count()
                    ));
                }
                if self.bench.is_some() {
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        if benchmark_ready {
                            "benchmark running"
                        } else {
                            "benchmark waiting for alignment"
                        },
                    );
                }
            });
        });
        let mut home_action = None;
        let mut view_action = None;
        let mut table_action = None;
        egui::CentralPanel::default().show(ui, |ui| match &mut self.view {
            View::Home(home) => home_action = home.ui(ui),
            View::TextCompare(view) => view_action = view.ui(ui, &mut self.prefs),
            View::TableCompare(view) => table_action = view.ui(ui, &mut self.prefs),
        });
        if let Some(HomeAction::OpenTextCompare { left, right }) = home_action {
            match TextCompareView::open(&left, &right) {
                Ok(v) => {
                    HomeView::record(&left, &right);
                    self.view = View::TextCompare(Box::new(v));
                }
                Err(e) => {
                    if let View::Home(home) = &mut self.view {
                        home.set_error(format!("Cannot open: {e}"));
                    }
                }
            }
        } else if let Some(HomeAction::OpenTableCompare { left, right }) = home_action {
            match TableCompareView::open(&left, &right) {
                Ok(v) => {
                    HomeView::record(&left, &right);
                    self.view = View::TableCompare(Box::new(v));
                }
                Err(e) => {
                    if let View::Home(home) = &mut self.view {
                        home.set_error(format!("Cannot open: {e}"));
                    }
                }
            }
        }
        if let Some(action) = table_action {
            match action {
                TableAction::Home => {
                    let paths = match &self.view {
                        View::TableCompare(v) => v.paths(),
                        _ => [None, None],
                    };
                    self.view = View::Home(HomeView::new(paths[0].as_ref(), paths[1].as_ref()));
                    self.bench = None;
                }
                TableAction::Exit => {
                    self.exit_confirmed = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                TableAction::Text { left, right } => match TextCompareView::open(&left, &right) {
                    Ok(v) => self.view = View::TextCompare(Box::new(v)),
                    Err(e) => eprintln!("Cannot open text: {e}"),
                },
            }
        }
        if let Some(action) = view_action {
            match action {
                ViewAction::Home => {
                    let paths = match &self.view {
                        View::TextCompare(v) => v.paths(),
                        _ => [None, None],
                    };
                    self.view = View::Home(HomeView::new(paths[0].as_ref(), paths[1].as_ref()));
                    self.bench = None;
                }
                ViewAction::Exit => {
                    self.exit_confirmed = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                ViewAction::CompareParentFolders { .. } => {} // folder-compare-sync 在此接入；当前命令禁用。
            }
        }
        self.last_update_ms = started.elapsed().as_secs_f64() * 1e3;
        let mut finished = false;
        if benchmark_ready {
            if let (Some(bench), View::TextCompare(view)) = (&mut self.bench, &self.view) {
                bench.record(
                    started.elapsed(),
                    frame.info().cpu_usage,
                    view.last_stats.rows_drawn,
                );
                if bench.is_done() {
                    println!(
                        "{}",
                        bench.summary(
                            view.files[0].byte_len,
                            view.files[1].byte_len,
                            view.files[0].load_time,
                            view.files[1].load_time,
                            view.index_bytes()
                        )
                    );
                    self.exit_confirmed = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    finished = true;
                }
            }
        }
        if benchmark_ready {
            if let (Some(bench), View::TableCompare(view)) = (&mut self.bench, &self.view) {
                bench.record(started.elapsed(), frame.info().cpu_usage, view.rows_drawn());
                if bench.is_done() {
                    println!(
                        "{}",
                        bench
                            .summary(
                                view.files[0].byte_len as usize,
                                view.files[1].byte_len as usize,
                                view.files[0].load_time,
                                view.files[1].load_time,
                                view.index_bytes()
                            )
                            .replace("bench.", "bench.table.")
                    );
                    println!("bench.table.last_frame_cells_drawn={}", view.cells_drawn());
                    self.exit_confirmed = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    finished = true;
                }
            }
        }
        if self.bench.is_some() {
            ctx.request_repaint();
        }
        if finished {
            self.bench = None;
        }
    }
}
