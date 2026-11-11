//! 应用接线集中于 View 和 action 分支：文本、表格、文件夹比较与文件夹同步。
use crate::{
    bench::{BenchState, Step},
    bench_dir::DirBench,
    dir_view::DirCompareView,
    home::{HomeAction, HomeView},
    sync_view::SyncView,
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
    FolderCompare(Box<DirCompareView>),
    FolderSync(Box<SyncView>),
}
pub struct MatchaApp {
    view: View,
    bench: Option<BenchState>,
    bench_dir: Option<DirBench>,
    prefs: Prefs,
    last_update_ms: f64,
    frame_cpu_ms: Option<f64>,
    exit_confirmed: bool,
    screenshot: Option<std::path::PathBuf>,
    snapshot: Option<String>,
    snapshot_capture: Option<std::sync::Arc<crate::snapshot::Capture>>,
    snapshot_preset_applied: bool,
    chrome_demo: crate::snapshot::ChromeDemo,
    capture_requested: bool,
    table_perf_reported: bool,
    frame_log: Option<crate::perf::FrameLog>,
}
impl MatchaApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        options: LaunchOptions,
        capture_result: crate::snapshot::CaptureResult,
    ) -> Self {
        crate::perf::mark("app:new-enter");
        crate::chrome::theme::apply(&cc.egui_ctx);
        crate::chrome::fonts::install(&cc.egui_ctx);
        let snapshot_capture = options
            .snapshot_out
            .as_ref()
            .filter(|_| options.snapshot.is_some())
            .map(|path| {
                crate::chrome::fonts::snapshot_scale(&cc.egui_ctx);
                cc.egui_ctx.all_styles_mut(|style| {
                    style.animation_time = 0.0;
                    style.visuals.text_cursor.blink = false;
                });
                crate::snapshot::Capture::install(
                    &cc.egui_ctx,
                    path.clone(),
                    options
                        .window_size
                        .unwrap_or(crate::snapshot::DEFAULT_WINDOW_SIZE),
                    capture_result,
                )
            });
        crate::perf::mark("app:fonts-installed");
        let mut home = HomeView::new(options.left.as_ref(), options.right.as_ref());
        let view = match (&options.left, &options.right) {
            (Some(l), Some(r)) if l.is_dir() && r.is_dir() => View::FolderCompare(Box::new(
                DirCompareView::open(&cc.egui_ctx, l, r, false, options.bench_dir),
            )),
            (Some(l), Some(r)) if options.bench_table || crate::home::table_paths(l, r) => {
                match TableCompareView::open(l, r) {
                    Ok(mut v) => {
                        if options.snapshot.is_some() {
                            v.snapshot_options();
                        }
                        if options.snapshot.is_none() {
                            HomeView::record(l, r);
                        }
                        View::TableCompare(Box::new(v))
                    }
                    Err(e) => {
                        home.set_error(format!("Cannot open: {e}"));
                        View::Home(home)
                    }
                }
            }
            (Some(l), Some(r)) => match TextCompareView::open(l, r) {
                Ok(mut v) => {
                    if options.snapshot.is_some() {
                        v.snapshot_rules();
                    }
                    if options.snapshot.is_none() {
                        HomeView::record(l, r);
                    }
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
        let bench_dir = options.bench_dir.then(DirBench::default);
        crate::perf::mark("app:view-created");
        Self {
            view,
            bench,
            bench_dir,
            prefs: Prefs::default(),
            last_update_ms: 0.0,
            frame_cpu_ms: None,
            exit_confirmed: false,
            screenshot: options.screenshot,
            snapshot: options.snapshot,
            snapshot_capture,
            snapshot_preset_applied: false,
            chrome_demo: Default::default(),
            capture_requested: false,
            table_perf_reported: false,
            frame_log: crate::perf::frames_enabled().then(Default::default),
        }
    }
    fn top_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("top_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Home").clicked() {
                    match &mut self.view {
                        View::TextCompare(v) => v.request_home(),
                        View::TableCompare(v) => v.request_home(),
                        View::FolderCompare(v) => {
                            if !v.request_text_close(false) {
                                self.view = View::Home(HomeView::default());
                            }
                        }
                        View::FolderSync(_) => self.view = View::Home(HomeView::default()),
                        View::Home(_) => {}
                    }
                }
                ui.separator();
                ui.label(match &self.view {
                    View::Home(_) => "Home",
                    View::TextCompare(_) => "Text Compare",
                    View::TableCompare(_) => "Table Compare",
                    View::FolderCompare(_) => "Folder Compare",
                    View::FolderSync(_) => "Folder Sync",
                });
            });
        });
    }
    /// 文本/表格/文件夹/Home 视图自带 chrome 菜单栏，不再绘制应用顶条。
    fn has_own_chrome(&self) -> bool {
        matches!(
            self.view,
            View::TextCompare(_) | View::TableCompare(_) | View::FolderCompare(_) | View::Home(_)
        )
    }
}
impl eframe::App for MatchaApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::from(crate::chrome::theme::PANEL_BG).to_array()
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, input: &mut egui::RawInput) {
        if self.snapshot.is_some() {
            input.time = Some(1.0);
            input.events.clear();
            // Whether the OS gives the snapshot window foreground focus varies
            // run to run; egui paints the text caret only in a focused
            // viewport, so pin it to unfocused for byte-stable captures.
            input.focused = false;
        }
    }
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
        if !self.exit_confirmed && ctx.input(|i| i.viewport().close_requested()) {
            if let View::FolderCompare(v) = &mut self.view {
                // Edited text opened from the folder view gets the same save prompt.
                if v.request_text_close(true) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
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
                    Step::Smooth(n) => view.scroll_to_row(view.top_row(view.row_height()) + n),
                    Step::Jump(row) => view.scroll_to_row(row),
                }
            }
        }
        if benchmark_ready {
            if let (Some(bench), View::TableCompare(view)) = (&mut self.bench, &mut self.view) {
                match bench.next_step(view.row_count()) {
                    Step::Smooth(n) => view.scroll_to_row(view.top_row(18.0) + n),
                    Step::Jump(row) => {
                        view.scroll_to_row(row);
                        view.scroll_to_column(row % 10);
                    }
                }
            }
        }
        if benchmark_ready && self.snapshot.is_none() && !self.capture_requested {
            if let Some(path) = self.screenshot.take() {
                ctx.request_screenshot(move |image| {
                    if let Err(error) = crate::bench::save_screenshot(&path, &image) {
                        eprintln!("screenshot: {error}");
                    }
                });
                self.capture_requested = true;
            }
        }
        if let (Some(bench), View::FolderCompare(view)) = (&mut self.bench_dir, &mut self.view) {
            view.poll(&ctx);
            bench.drive(view, &ctx);
        }
        if let Some(scene) = self
            .snapshot
            .as_deref()
            .filter(|s| crate::snapshot::is_demo(s))
        {
            self.chrome_demo.show(ui, scene);
            if let Some(capture) = &self.snapshot_capture {
                capture.set_ready(true);
            }
            return;
        }
        if !self.has_own_chrome() {
            self.top_bar(ui);
        }
        if self.bench.is_some() || self.bench_dir.is_some() {
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
        }
        let mut home_action = None;
        let mut view_action = None;
        let mut table_action = None;
        let mut folder_switch = None;
        let panel = if self.has_own_chrome() {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(crate::chrome::theme::PANEL_BG))
        } else {
            egui::CentralPanel::default()
        };
        panel.show(ui, |ui| match &mut self.view {
            View::Home(home) => home_action = home.ui(ui),
            View::TextCompare(view) => view_action = view.ui(ui, &mut self.prefs),
            View::TableCompare(view) => table_action = view.ui(ui, &mut self.prefs),
            View::FolderCompare(view) => folder_switch = view.ui(ui),
            View::FolderSync(view) => folder_switch = view.ui(ui),
        });
        if let Some(capture) = &self.snapshot_capture {
            let mut ready = match &mut self.view {
                View::TextCompare(v) => v.is_ready() && !v.labels_pending(),
                View::TableCompare(v) => v.is_ready() && !v.labels_pending(),
                View::FolderCompare(v) => !v.busy(),
                View::Home(_) => true,
                _ => false,
            };
            if ready && !self.snapshot_preset_applied {
                match &mut self.view {
                    View::TextCompare(v) => {
                        let scene = self.snapshot.as_deref().unwrap_or_default();
                        v.snapshot_preset(scene);
                        if scene == "text-menu-edit" {
                            crate::chrome::menubar::request_open(&ctx, "Edit");
                        }
                    }
                    View::TableCompare(v) => {
                        v.snapshot_preset(self.snapshot.as_deref().unwrap_or_default())
                    }
                    View::FolderCompare(v) => {
                        v.snapshot_preset(self.snapshot.as_deref().unwrap_or_default())
                    }
                    _ => {}
                }
                self.snapshot_preset_applied = true;
                ready = false;
            }
            capture.set_ready(ready);
        }
        // The central panel has just drawn the comparison when the view first
        // reports ready: this is the first interactive frame.
        if !self.table_perf_reported && crate::perf::enabled() {
            let ready = match &self.view {
                View::TextCompare(v) => v.is_ready(),
                View::TableCompare(v) => v.is_ready(),
                View::FolderCompare(v) => !v.busy(),
                View::Home(_) => true,
                _ => false,
            };
            if ready {
                self.table_perf_reported = true;
                crate::perf::mark("app:first-ready-frame");
                if let View::TableCompare(v) = &self.view {
                    if let Some(s) = v.statistics() {
                        eprintln!(
                            "[table-perf] fingerprint rows={} same={} unimportant={} important={} left_only={} right_only={}",
                            v.row_count(), s.same, s.unimportant, s.important, s.left_only, s.right_only
                        );
                    }
                }
                if crate::perf::exit_after_ready() {
                    self.exit_confirmed = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
        let mut folder_go_home = false;
        if let View::FolderCompare(view) = &mut self.view {
            if view.take_exit_request() {
                self.exit_confirmed = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if view.take_home_request() {
                folder_go_home = true;
            }
        }
        if folder_go_home {
            self.view = View::Home(HomeView::default());
        }
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
        } else if let Some(HomeAction::OpenFolderCompare { left, right }) = &home_action {
            self.view = View::FolderCompare(Box::new(DirCompareView::open(
                &ctx, left, right, false, false,
            )));
        } else if let Some(HomeAction::OpenFolderSync { left, right }) = &home_action {
            self.view = View::FolderSync(Box::new(SyncView::open(&ctx, left, right)));
        } else if let Some(HomeAction::Exit) = &home_action {
            self.exit_confirmed = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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
                ViewAction::CompareParentFolders { left, right } => {
                    self.view = View::FolderCompare(Box::new(DirCompareView::open(
                        &ctx, &left, &right, false, false,
                    )));
                    self.bench = None;
                }
            }
        }
        if let Some((sync, paths)) = folder_switch {
            self.view = if sync {
                View::FolderSync(Box::new(SyncView::open(&ctx, &paths[0], &paths[1])))
            } else {
                View::FolderCompare(Box::new(DirCompareView::open(
                    &ctx, &paths[0], &paths[1], false, false,
                )))
            };
        }
        self.last_update_ms = started.elapsed().as_secs_f64() * 1e3;
        if let Some(log) = &mut self.frame_log {
            let view = match &self.view {
                View::Home(_) => "home",
                View::TextCompare(_) => "text",
                View::TableCompare(_) => "table",
                View::FolderCompare(_) => "folder",
                View::FolderSync(_) => "sync",
            };
            log.record(started, self.last_update_ms as f32, view);
        }
        let mut dir_done = false;
        if let (Some(bench), View::FolderCompare(view)) = (&mut self.bench_dir, &self.view) {
            if bench.record(started.elapsed(), view) {
                println!("{}", bench.summary(view));
                self.exit_confirmed = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                dir_done = true;
            } else {
                ctx.request_repaint();
            }
        }
        if dir_done {
            self.bench_dir = None;
        }
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
