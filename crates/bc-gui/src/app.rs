//! 应用接线：标签页容器（每页一个 View）与各视图 action 分支——文本、表格、
//! 文件夹比较与文件夹同步。只有活动页绘制；关闭/退出经各视图自己的保存确认。
#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
use crate::{
    bench::{BenchState, Step},
    bench_dir::DirBench,
    chrome::{
        icons::Icon,
        tabstrip::{self, TabEvent, TabSpec},
    },
    dir_view::DirCompareView,
    home::{HomeAction, HomeView, SessionKind},
    remote::{Completion, Outcome, RemoteInbox, RemoteOpen},
    sync_view::SyncView,
    hex::{HexAction, HexCompareView},
    table::{TableAction, TableCompareView},
    text::{TextCompareView, ViewAction},
    LaunchOptions,
};
use bc_session::prefs::Prefs;
use eframe::egui;
use std::{path::PathBuf, time::Instant};

pub enum View {
    Home(HomeView),
    TextCompare(Box<TextCompareView>),
    TableCompare(Box<TableCompareView>),
    HexCompare(Box<HexCompareView>),
    FolderCompare(Box<DirCompareView>),
    FolderSync(Box<SyncView>),
}
impl View {
    fn kind(&self) -> &'static str {
        match self {
            View::Home(_) => "Home",
            View::TextCompare(_) => "Text Compare",
            View::TableCompare(_) => "Table Compare",
            View::HexCompare(_) => "Hex Compare",
            View::FolderCompare(_) => "Folder Compare",
            View::FolderSync(_) => "Folder Sync",
        }
    }
    fn icon(&self) -> Icon {
        match self {
            View::Home(_) => Icon::Home,
            View::TextCompare(_) => Icon::File,
            View::TableCompare(_) => Icon::Structure,
            View::HexCompare(_) => Icon::File,
            View::FolderCompare(_) => Icon::Folder,
            View::FolderSync(_) => Icon::Sync,
        }
    }
    /// 两侧显示名（文件名，无文件名时用整条路径）；空会话两侧皆为 None。
    fn side_names(&self) -> [Option<String>; 2] {
        fn name(path: &std::path::Path) -> String {
            path.file_name().map_or_else(
                || path.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            )
        }
        let texts = |texts: [String; 2]| {
            texts.map(|t| (!t.trim().is_empty()).then(|| name(std::path::Path::new(t.trim()))))
        };
        match self {
            View::Home(_) => [None, None],
            View::TextCompare(v) => v.paths().map(|p| p.as_deref().map(name)),
            View::TableCompare(v) => v.paths().map(|p| p.as_deref().map(name)),
            View::HexCompare(v) => v.paths().map(|p| p.as_deref().map(name)),
            View::FolderCompare(v) => texts(v.path_texts()),
            View::FolderSync(v) => texts(v.folder.path_texts()),
        }
    }
    /// 标签页标题：会话为 "左 <--> 右"，空会话为类型名。
    fn caption(&self) -> String {
        match self.side_names() {
            [None, None] => self.kind().into(),
            [l, r] => format!(
                "{} <--> {}",
                l.as_deref().unwrap_or("Untitled"),
                r.as_deref().unwrap_or("Untitled")
            ),
        }
    }
    fn window_title(&self) -> String {
        match (self, self.side_names()) {
            (View::Home(_), _) | (_, [None, None]) => format!("{} - Matcha", self.kind()),
            _ => format!("{} - {} - Matcha", self.caption(), self.kind()),
        }
    }
    /// 关闭会丢失的未保存编辑（文件夹视图为其内嵌文本窗口）。
    fn is_dirty(&self) -> bool {
        match self {
            View::Home(_) => false,
            View::TextCompare(v) => v.is_dirty(),
            View::TableCompare(v) => v.is_dirty(),
            View::HexCompare(v) => v.is_dirty(),
            View::FolderCompare(v) => v.text_dirty(),
            View::FolderSync(v) => v.folder.text_dirty(),
        }
    }
    fn prompt_open(&self) -> bool {
        match self {
            View::Home(_) => false,
            View::TextCompare(v) => v.prompt_open(),
            View::TableCompare(v) => v.prompt_open(),
            View::HexCompare(v) => v.prompt_open(),
            View::FolderCompare(v) => v.prompt_open(),
            View::FolderSync(v) => v.folder.prompt_open(),
        }
    }
    /// 退出前的保存确认（沿用各视图原有的 Exit 确认）。
    fn request_exit(&mut self) {
        match self {
            View::Home(_) => {}
            View::TextCompare(v) => v.request_exit(),
            View::TableCompare(v) => v.request_exit(),
            View::HexCompare(v) => v.request_exit(),
            View::FolderCompare(v) => {
                v.request_text_close(true);
            }
            View::FolderSync(v) => {
                v.folder.request_text_close(true);
            }
        }
    }
    /// 关闭标签页前的保存确认；确认后视图回报 Close。
    fn request_close(&mut self) {
        match self {
            View::Home(_) => {}
            View::TextCompare(v) => v.request_close(),
            View::TableCompare(v) => v.request_close(),
            View::HexCompare(v) => v.request_close(),
            View::FolderCompare(v) => v.request_tab_close(),
            View::FolderSync(v) => v.folder.request_tab_close(),
        }
    }
}
/// 一个标签页及其在退出流程中的状态。
pub struct Tab {
    pub view: View,
    /// 退出流程已向本页发出保存确认。
    exit_asked: bool,
    /// 本页已确认可以退出（保存或放弃）。
    exit_cleared: bool,
    /// 转交来的等待型会话：本页关闭（或随应用退出）时通知等待方。
    on_close: Option<Completion>,
}
impl Tab {
    fn new(view: View) -> Self {
        Self {
            view,
            exit_asked: false,
            exit_cleared: false,
            on_close: None,
        }
    }
}
impl Drop for Tab {
    fn drop(&mut self) {
        // 关页、原位换类型以外的丢弃（含应用退出）都算会话结束。
        if let Some(done) = self.on_close.take() {
            done(Outcome::Closed);
        }
    }
}
/// 本帧活动页产生的标签页级命令，在绘制结束后统一执行。
enum TabCommand {
    Home,
    NewTab,
    Close,
    /// 开始退出；`true` 表示活动页已自行确认。
    Exit(bool),
    Open(View),
    Replace(View),
}
pub struct MatchaApp {
    tabs: Vec<Tab>,
    active: usize,
    /// 退出流程进行中：逐个脏标签页弹保存确认。
    exiting: bool,
    window_title: String,
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
    /// 单实例转交的请求队列（仅常驻实例有）。
    remote: Option<RemoteInbox>,
    /// Home 的 Tools > Options（比较视图各自持有自己的对话框）。
    options: crate::dialogs::options::OptionsDialog,
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
        let view = Self::launch_view(&cc.egui_ctx, &options);
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
        let mut app = Self::with_view(view);
        app.bench = bench;
        app.bench_dir = bench_dir;
        app.screenshot = options.screenshot;
        app.snapshot = options.snapshot;
        app.snapshot_capture = snapshot_capture;
        app
    }
    /// 单标签页的应用（启动、快照、基准与测试共用）。
    pub fn with_view(view: View) -> Self {
        Self {
            tabs: vec![Tab::new(view)],
            active: 0,
            exiting: false,
            window_title: String::new(),
            bench: None,
            bench_dir: None,
            prefs: Prefs::default(),
            last_update_ms: 0.0,
            frame_cpu_ms: None,
            exit_confirmed: false,
            screenshot: None,
            snapshot: None,
            snapshot_capture: None,
            snapshot_preset_applied: false,
            chrome_demo: Default::default(),
            capture_requested: false,
            table_perf_reported: false,
            frame_log: crate::perf::frames_enabled().then(Default::default),
            remote: None,
            options: Default::default(),
        }
    }
    /// 接入转交队列。
    pub fn attach_remote(&mut self, ctx: &egui::Context, inbox: RemoteInbox) {
        inbox.attach(ctx);
        self.remote = Some(inbox);
    }
    /// 处理已到达的转交请求（`App::logic` 每帧调用，窗口最小化时也会调用）。
    pub fn poll_remote(&mut self, ctx: &egui::Context) {
        let requests = self.remote.as_ref().map(RemoteInbox::drain);
        for request in requests.into_iter().flatten() {
            self.open_remote(ctx, request);
        }
    }
    /// 转交请求：两侧路径齐全则开新比较标签页（打不开时回 Home 显示错误），然后把
    /// 窗口提到前台；没有路径时只提到前台。
    pub fn open_remote(&mut self, ctx: &egui::Context, request: RemoteOpen) {
        let RemoteOpen { left, right, done } = request;
        match (left, right) {
            (Some(left), Some(right)) => {
                match Self::comparison_view(ctx, crate::home::routed(left, right)) {
                    Some(Ok(view)) => {
                        self.bench = None;
                        self.open_tab(view);
                        self.tabs[self.active].on_close = done;
                    }
                    error => {
                        let message = match error {
                            Some(Err(e)) => e,
                            _ => "unsupported comparison".into(),
                        };
                        self.go_home();
                        if let View::Home(home) = self.view_mut() {
                            home.set_error(format!("Cannot open: {message}"));
                        }
                        if let Some(done) = done {
                            done(Outcome::Failed(message));
                        }
                    }
                }
            }
            _ => {
                if let Some(done) = done {
                    done(Outcome::Closed);
                }
            }
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }
    /// 命令行给了两个路径时直接打开比较（唯一标签页，不附带 Home）；否则为 Home。
    fn launch_view(ctx: &egui::Context, options: &LaunchOptions) -> View {
        let home = HomeView::new;
        // `/fv=` 强制会话类型（仅文件对；其它类型继续走格式路由）。
        if let (Some(l), Some(r)) = (&options.left, &options.right) {
            let forced = match options.force_view {
                Some(bc_session::SessionKind::HexCompare) => Some("hex"),
                Some(bc_session::SessionKind::TextCompare) => Some("text"),
                Some(bc_session::SessionKind::TableCompare) => Some("table"),
                _ => None,
            };
            if let Some(which) = forced {
                let opened = match which {
                    "hex" => HexCompareView::open(l, r).map(|v| View::HexCompare(Box::new(v))),
                    "table" => TableCompareView::open(l, r).map(|v| View::TableCompare(Box::new(v))),
                    _ => TextCompareView::open(l, r).map(|v| View::TextCompare(Box::new(v))),
                };
                return match opened {
                    Ok(view) => {
                        HomeView::record(l, r);
                        view
                    }
                    Err(e) => {
                        let mut home = home();
                        home.set_error(format!("Cannot open: {e}"));
                        View::Home(home)
                    }
                };
            }
        }
        match (&options.left, &options.right) {
            (Some(l), Some(r))
                if l.is_dir()
                    && r.is_dir()
                    && options.snapshot.as_deref() == Some("folder-sync") =>
            {
                View::FolderSync(Box::new(SyncView::open(ctx, l, r)))
            }
            (Some(l), Some(r)) if l.is_dir() && r.is_dir() => View::FolderCompare(Box::new(
                DirCompareView::open(ctx, l, r, false, options.bench_dir),
            )),
            (Some(l), Some(r)) if !options.bench_table && crate::home::hex_paths(l, r) => {
                match HexCompareView::open(l, r) {
                    Ok(v) => {
                        if options.snapshot.is_none() {
                            HomeView::record(l, r);
                        }
                        View::HexCompare(Box::new(v))
                    }
                    Err(e) => {
                        let mut home = home();
                        home.set_error(format!("Cannot open: {e}"));
                        View::Home(home)
                    }
                }
            }
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
                        let mut home = home();
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
                    let mut home = home();
                    home.set_error(format!("Cannot open: {e}"));
                    View::Home(home)
                }
            },
            _ => View::Home(home()),
        }
    }
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }
    pub fn active_tab(&self) -> usize {
        self.active
    }
    pub fn active_view(&self) -> &View {
        &self.tabs[self.active].view
    }
    fn view_mut(&mut self) -> &mut View {
        &mut self.tabs[self.active].view
    }
    fn activate(&mut self, index: usize) {
        self.active = index.min(self.tabs.len() - 1);
        // 其他标签页可能刚记录了新的最近比较。
        if let View::Home(home) = self.view_mut() {
            home.refresh_recent();
        }
    }
    /// 追加一个标签页并激活。
    pub fn open_tab(&mut self, view: View) {
        self.tabs.push(Tab::new(view));
        self.activate(self.tabs.len() - 1);
    }
    /// Home：激活已有的 Home 页；没有时在最前插入一个。
    fn go_home(&mut self) {
        match self
            .tabs
            .iter()
            .position(|t| matches!(t.view, View::Home(_)))
        {
            Some(index) => self.activate(index),
            None => {
                self.tabs.insert(0, Tab::new(View::Home(HomeView::new())));
                self.active = 0;
            }
        }
        self.bench = None;
    }
    /// 关闭标签页：有未保存编辑时激活该页并走其保存确认，确认后由视图回报 Close。
    pub fn close_tab(&mut self, ctx: &egui::Context, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        if self.tabs[index].view.is_dirty() {
            self.activate(index);
            self.view_mut().request_close();
        } else {
            self.remove_tab(ctx, index);
        }
    }
    /// 移除标签页；关闭最后一页即退出应用（命令行打开的会话关闭后进程结束）。
    fn remove_tab(&mut self, ctx: &egui::Context, index: usize) {
        if self.tabs.len() == 1 {
            self.exit_confirmed = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        self.tabs.remove(index);
        if index < self.active || self.active >= self.tabs.len() {
            self.active = self.active.saturating_sub(1);
        }
        self.activate(self.active);
        self.bench = None;
    }
    fn begin_exit(&mut self, active_confirmed: bool) {
        self.exiting = true;
        if active_confirmed {
            self.tabs[self.active].exit_cleared = true;
        }
    }
    /// 退出流程一步：下一个未确认的脏页弹保存确认；用户取消则中止；全部确认后关窗。
    fn step_exit(&mut self, ctx: &egui::Context) {
        let next = self
            .tabs
            .iter()
            .position(|t| !t.exit_cleared && t.view.is_dirty());
        match next {
            None => {
                self.exiting = false;
                self.exit_confirmed = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Some(index) if self.tabs[index].exit_asked => {
                if !self.tabs[index].view.prompt_open() {
                    // 确认框已关而页面仍脏且未放行：用户取消了退出。
                    self.exiting = false;
                    for tab in &mut self.tabs {
                        tab.exit_asked = false;
                        tab.exit_cleared = false;
                    }
                }
            }
            Some(index) => {
                self.activate(index);
                let tab = &mut self.tabs[index];
                tab.exit_asked = true;
                tab.view.request_exit();
            }
        }
    }
    /// 新会话标签页：空会话（可预填左侧）；Home 的会话图标、树节点、菜单与 "+" 共用。
    fn new_session(&mut self, ctx: &egui::Context, kind: SessionKind, left: Option<PathBuf>) {
        let text = |p: &Option<PathBuf>| p.as_ref().map(|p| p.display().to_string());
        let view = match kind {
            SessionKind::FolderCompare | SessionKind::Auto => {
                let mut v = DirCompareView::empty(ctx, false, false);
                if let Some(left) = text(&left) {
                    v.open_paths([left, String::new()]);
                }
                View::FolderCompare(Box::new(v))
            }
            SessionKind::FolderSync => {
                let mut v = SyncView::empty(ctx);
                if let Some(left) = text(&left) {
                    v.folder.open_paths([left, String::new()]);
                }
                View::FolderSync(Box::new(v))
            }
            SessionKind::TextCompare => match TextCompareView::empty() {
                Ok(mut v) => {
                    if let Some(path) = &left {
                        if let Err(e) = v.open_path(0, path) {
                            v.message = e.to_string();
                        }
                    }
                    View::TextCompare(Box::new(v))
                }
                Err(e) => {
                    if let View::Home(home) = self.view_mut() {
                        home.set_error(format!("Cannot open: {e}"));
                    }
                    return;
                }
            },
            SessionKind::TableCompare => {
                let mut v = TableCompareView::empty();
                if let Some(path) = left {
                    v.open_path(0, path);
                }
                View::TableCompare(Box::new(v))
            }
            SessionKind::HexCompare => {
                let mut v = HexCompareView::empty();
                if let Some(path) = &left {
                    if let Err(e) = v.open_path(0, path) {
                        v.message = e.to_string();
                    }
                }
                View::HexCompare(Box::new(v))
            }
        };
        self.open_tab(view);
    }
    /// Home 打开两侧路径的比较：成功则新开标签页，失败把错误写回 Home。
    fn open_comparison(&mut self, ctx: &egui::Context, action: HomeAction) {
        let Some(opened) = Self::comparison_view(ctx, action) else {
            return;
        };
        match opened {
            Ok(view) => self.open_tab(view),
            Err(e) => {
                if let View::Home(home) = self.view_mut() {
                    home.set_error(format!("Cannot open: {e}"));
                }
            }
        }
    }
    /// 两侧路径的比较视图；`None` 表示不是打开比较的动作。
    fn comparison_view(ctx: &egui::Context, action: HomeAction) -> Option<Result<View, String>> {
        Some(match action {
            HomeAction::OpenTextCompare { left, right } => TextCompareView::open(&left, &right)
                .map(|v| {
                    HomeView::record(&left, &right);
                    View::TextCompare(Box::new(v))
                })
                .map_err(|e| e.to_string()),
            HomeAction::OpenTableCompare { left, right } => TableCompareView::open(&left, &right)
                .map(|v| {
                    HomeView::record(&left, &right);
                    View::TableCompare(Box::new(v))
                })
                .map_err(|e| e.to_string()),
            HomeAction::OpenHexCompare { left, right } => HexCompareView::open(&left, &right)
                .map(|v| {
                    HomeView::record(&left, &right);
                    View::HexCompare(Box::new(v))
                })
                .map_err(|e| e.to_string()),
            HomeAction::OpenFolderCompare { left, right } => Ok(View::FolderCompare(Box::new(
                DirCompareView::open(ctx, &left, &right, false, false),
            ))),
            HomeAction::OpenFolderSync { left, right } => Ok(View::FolderSync(Box::new(
                SyncView::open(ctx, &left, &right),
            ))),
            _ => return None,
        })
    }
    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let mut home = false;
        egui::Panel::top("top_bar").show(ui, |ui| {
            ui.horizontal(|ui| {
                home = ui.button("Home").clicked();
                ui.separator();
                ui.label(self.active_view().kind());
            });
        });
        if home {
            self.go_home();
        }
    }
    /// 文本/表格/文件夹/Home 视图自带 chrome 菜单栏，不再绘制应用顶条。
    fn has_own_chrome(&self) -> bool {
        matches!(
            self.active_view(),
            View::TextCompare(_)
                | View::TableCompare(_)
                | View::HexCompare(_)
                | View::FolderCompare(_)
                | View::FolderSync(_)
                | View::Home(_)
        )
    }
    /// 标签条：两页及以上才显示（单会话窗口与参考截图一致，无标签条）。
    fn tab_strip(&mut self, ui: &mut egui::Ui) -> Option<TabEvent> {
        if self.tabs.len() < 2 {
            return None;
        }
        let specs: Vec<TabSpec> = self
            .tabs
            .iter()
            .map(|t| TabSpec {
                title: t.view.caption(),
                icon: Some(t.view.icon()),
            })
            .collect();
        let active = self.active;
        egui::Panel::top("tab_strip")
            .exact_size(tabstrip::HEIGHT)
            .resizable(false)
            .show_separator_line(false)
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let rect = ui.max_rect();
                tabstrip::show(ui, rect, &specs, active)
            })
            .inner
    }
    /// 一帧：快捷键与标签条 → 活动页绘制 → 执行其命令。
    pub fn frame(&mut self, ui: &mut egui::Ui, frame_cpu: Option<f32>) {
        let started = Instant::now();
        let ctx = ui.ctx().clone();
        self.frame_cpu_ms = frame_cpu.map(|s| s as f64 * 1e3);
        if !self.exit_confirmed && ctx.input(|i| i.viewport().close_requested()) {
            // 有未保存编辑的标签页时拦下关闭，逐页走保存确认。
            if self.exiting || self.tabs.iter().any(|t| t.view.is_dirty()) {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.begin_exit(false);
            }
        }
        if self.exiting {
            self.step_exit(&ctx);
        }
        // Ctrl+T / Ctrl+W 属于标签页容器，先于视图消费，避免视图各自处理。
        let (new_tab, close_tab) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::T),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::W),
            )
        });
        if !self.exiting {
            if new_tab {
                self.open_tab(View::Home(HomeView::new()));
            }
            if close_tab {
                self.close_tab(&ctx, self.active);
            }
        }
        if let View::TextCompare(v) = self.view_mut() {
            v.poll();
        }
        if let View::TableCompare(v) = self.view_mut() {
            v.poll();
        }
        if let View::HexCompare(v) = self.view_mut() {
            v.poll();
        }
        let benchmark_ready = matches!(self.active_view(), View::TextCompare(v) if v.is_ready())
            || matches!(self.active_view(), View::TableCompare(v) if v.is_ready());
        if benchmark_ready {
            if let (Some(bench), View::TextCompare(view)) =
                (&mut self.bench, &mut self.tabs[self.active].view)
            {
                match bench.next_step(view.row_count()) {
                    Step::Smooth(n) => view.scroll_to_row(view.top_row(view.row_height()) + n),
                    Step::Jump(row) => view.scroll_to_row(row),
                }
            }
        }
        if benchmark_ready {
            if let (Some(bench), View::TableCompare(view)) =
                (&mut self.bench, &mut self.tabs[self.active].view)
            {
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
        if let (Some(bench), View::FolderCompare(view)) =
            (&mut self.bench_dir, &mut self.tabs[self.active].view)
        {
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
        let title = self.active_view().window_title();
        if self.window_title != title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
        match self.tab_strip(ui) {
            Some(TabEvent::Activate(index)) => self.activate(index),
            Some(TabEvent::Close(index)) => self.close_tab(&ctx, index),
            Some(TabEvent::New) => self.open_tab(View::Home(HomeView::new())),
            None => {}
        }
        if self.bench.is_some() || self.bench_dir.is_some() {
            egui::Panel::bottom("performance_status").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("update {:.2} ms", self.last_update_ms));
                    if let Some(cpu) = self.frame_cpu_ms {
                        ui.label(format!("frame cpu {cpu:.2} ms"));
                    }
                    if let View::TextCompare(v) = &self.tabs[self.active].view {
                        ui.label(format!(
                            "rows drawn {} · total rows {}",
                            v.last_stats.rows_drawn,
                            v.row_count()
                        ));
                    }
                    if let View::TableCompare(v) = &self.tabs[self.active].view {
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
        let mut hex_action = None;
        let mut folder_switch = None;
        let panel = if self.has_own_chrome() {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(crate::chrome::theme::PANEL_BG))
        } else {
            egui::CentralPanel::default()
        };
        let prefs = &mut self.prefs;
        let home_modal = self.options.open;
        panel.show(ui, |ui| match &mut self.tabs[self.active].view {
            View::Home(home) => {
                if home_modal {
                    ui.disable();
                }
                home_action = home.ui(ui);
            }
            View::TextCompare(view) => view_action = view.ui(ui, prefs),
            View::TableCompare(view) => table_action = view.ui(ui, prefs),
            View::HexCompare(view) => hex_action = view.ui(ui),
            View::FolderCompare(view) => folder_switch = view.ui(ui),
            View::FolderSync(view) => folder_switch = view.ui(ui),
        });
        if let Some(capture) = &self.snapshot_capture {
            let mut ready = match &mut self.tabs[self.active].view {
                View::HexCompare(v) => v.is_ready(),
                View::TextCompare(v) => v.is_ready() && !v.labels_pending(),
                View::TableCompare(v) => v.is_ready() && !v.labels_pending(),
                View::FolderCompare(v) => !v.busy(),
                View::FolderSync(v) => !v.folder.busy() && v.folder.snapshot.sync_stats.is_some(),
                View::Home(_) => true,
            };
            if ready && !self.snapshot_preset_applied {
                match &mut self.tabs[self.active].view {
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
                    View::FolderSync(v) => v.snapshot_preset(),
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
            let ready = match self.active_view() {
                View::TextCompare(v) => v.is_ready(),
                View::TableCompare(v) => v.is_ready(),
                View::FolderCompare(v) => !v.busy(),
                View::Home(_) => true,
                _ => false,
            };
            if ready {
                self.table_perf_reported = true;
                crate::perf::mark("app:first-ready-frame");
                if let View::TableCompare(v) = self.active_view() {
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
        let mut commands = Vec::new();
        let folder = match self.view_mut() {
            View::FolderCompare(view) => Some(&mut **view),
            View::FolderSync(view) => Some(&mut view.folder),
            _ => None,
        };
        if let Some(view) = folder {
            if view.take_exit_request() {
                commands.push(TabCommand::Exit(false));
            }
            if view.take_home_request() {
                commands.push(TabCommand::Home);
            }
            if view.take_new_tab_request() {
                commands.push(TabCommand::NewTab);
            }
            if view.take_close_request() {
                commands.push(TabCommand::Close);
            }
        }
        match home_action {
            Some(HomeAction::NewSession { kind, left }) => self.new_session(&ctx, kind, left),
            Some(HomeAction::NewTab) => commands.push(TabCommand::NewTab),
            Some(HomeAction::CloseTab) => self.close_tab(&ctx, self.active),
            Some(HomeAction::Exit) => commands.push(TabCommand::Exit(false)),
            Some(HomeAction::Options) => self
                .options
                .show(&self.prefs, crate::chrome::dialog::Placement::Window),
            Some(action) => self.open_comparison(&ctx, action),
            None => {}
        }
        self.options.ui(&ctx, &mut self.prefs);
        if let Some(action) = table_action {
            let command = match action {
                TableAction::Home => Some(TabCommand::Home),
                TableAction::Close => Some(TabCommand::Close),
                TableAction::NewTab => Some(TabCommand::NewTab),
                TableAction::Exit => Some(TabCommand::Exit(true)),
                // Session > Using: Text Compare 原位换会话类型。
                TableAction::Text { left, right } => match TextCompareView::open(&left, &right) {
                    Ok(v) => Some(TabCommand::Replace(View::TextCompare(Box::new(v)))),
                    Err(e) => {
                        eprintln!("Cannot open text: {e}");
                        None
                    }
                },
                // Using: Hex Compare 原位换会话类型。
                TableAction::Hex { left, right } => match HexCompareView::open(&left, &right) {
                    Ok(v) => Some(TabCommand::Replace(View::HexCompare(Box::new(v)))),
                    Err(e) => {
                        eprintln!("Cannot open hex: {e}");
                        None
                    }
                },
            };
            commands.extend(command);
        }
        if let Some(action) = view_action {
            commands.extend(match action {
                ViewAction::Home => Some(TabCommand::Home),
                ViewAction::Close => Some(TabCommand::Close),
                ViewAction::NewTab => Some(TabCommand::NewTab),
                ViewAction::Exit => Some(TabCommand::Exit(true)),
                ViewAction::CompareParentFolders { left, right } => {
                    Some(TabCommand::Open(View::FolderCompare(Box::new(
                        DirCompareView::open(&ctx, &left, &right, false, false),
                    ))))
                }
                // Compare Selection to Clipboard：新标签页比较两段内存文本。
                ViewAction::CompareTexts { left, right } => {
                    match TextCompareView::from_texts(&left, &right) {
                        Ok(v) => Some(TabCommand::Open(View::TextCompare(Box::new(v)))),
                        Err(e) => {
                            eprintln!("Cannot compare selection: {e}");
                            None
                        }
                    }
                }
                // Session > Compare Files Using > Hex Compare：原位换会话类型。
                ViewAction::CompareAsHex { left, right } => match HexCompareView::open(&left, &right) {
                    Ok(v) => Some(TabCommand::Replace(View::HexCompare(Box::new(v)))),
                    Err(e) => {
                        eprintln!("Cannot open hex: {e}");
                        None
                    }
                },
            });
        }
        if let Some(action) = hex_action {
            let command = match action {
                HexAction::Home => Some(TabCommand::Home),
                HexAction::Close => Some(TabCommand::Close),
                HexAction::NewTab => Some(TabCommand::NewTab),
                HexAction::Exit => Some(TabCommand::Exit(true)),
                HexAction::CompareAsText { left, right } => match TextCompareView::open(&left, &right) {
                    Ok(v) => Some(TabCommand::Replace(View::TextCompare(Box::new(v)))),
                    Err(e) => {
                        eprintln!("Cannot open text: {e}");
                        None
                    }
                },
                HexAction::CompareAsTable { left, right } => match TableCompareView::open(&left, &right) {
                    Ok(v) => Some(TabCommand::Replace(View::TableCompare(Box::new(v)))),
                    Err(e) => {
                        eprintln!("Cannot open table: {e}");
                        None
                    }
                },
            };
            commands.extend(command);
        }
        if let Some((sync, paths)) = folder_switch {
            // 比较/同步互切沿用原位替换（同一会话换类型）。
            commands.push(TabCommand::Replace(if sync {
                View::FolderSync(Box::new(SyncView::open(&ctx, &paths[0], &paths[1])))
            } else {
                View::FolderCompare(Box::new(DirCompareView::open(
                    &ctx, &paths[0], &paths[1], false, false,
                )))
            }));
        }
        for command in commands {
            match command {
                TabCommand::Home => self.go_home(),
                TabCommand::NewTab => self.open_tab(View::Home(HomeView::new())),
                TabCommand::Close => self.remove_tab(&ctx, self.active),
                TabCommand::Exit(confirmed) => self.begin_exit(confirmed),
                TabCommand::Open(view) => {
                    self.bench = None;
                    self.open_tab(view);
                }
                TabCommand::Replace(view) => {
                    self.bench = None;
                    // 同一会话换类型：等待方仍等这个标签页。
                    let done = self.tabs[self.active].on_close.take();
                    self.tabs[self.active] = Tab::new(view);
                    self.tabs[self.active].on_close = done;
                }
            }
        }
        if self.exiting {
            ctx.request_repaint();
        }
        self.last_update_ms = started.elapsed().as_secs_f64() * 1e3;
        if let Some(log) = &mut self.frame_log {
            let view = match &self.tabs[self.active].view {
                View::Home(_) => "home",
                View::TextCompare(_) => "text",
                View::TableCompare(_) => "table",
                View::HexCompare(_) => "hex",
                View::FolderCompare(_) => "folder",
                View::FolderSync(_) => "sync",
            };
            log.record(started, self.last_update_ms as f32, view);
        }
        let mut dir_done = false;
        if let (Some(bench), View::FolderCompare(view)) =
            (&mut self.bench_dir, &self.tabs[self.active].view)
        {
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
            if let (Some(bench), View::TextCompare(view)) =
                (&mut self.bench, &self.tabs[self.active].view)
            {
                bench.record(started.elapsed(), frame_cpu, view.last_stats.rows_drawn);
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
            if let (Some(bench), View::TableCompare(view)) =
                (&mut self.bench, &self.tabs[self.active].view)
            {
                bench.record(started.elapsed(), frame_cpu, view.rows_drawn());
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
impl eframe::App for MatchaApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Rgba::from(crate::chrome::theme::PANEL_BG).to_array()
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_remote(ctx);
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
        self.frame(ui, frame.info().cpu_usage);
    }
}
