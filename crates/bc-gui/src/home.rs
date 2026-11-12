//! Home：参考布局的会话管理页——菜单栏、左侧会话树（New / Auto-saved，
//! Auto-saved 下为最近比较）、中央会话类型图标网格、底部帮助面板；拖放两个
//! 文件/文件夹直接打开，点击会话类型新开一个空会话标签页（路径在会话内输入）。
use crate::chrome::{
    icons::{self, SessionIcon},
    menubar::{self, MenuItem, MenuItemKind, MenuSpec},
    theme,
};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use std::path::PathBuf;
use std::time::SystemTime;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomeAction {
    OpenTextCompare {
        left: PathBuf,
        right: PathBuf,
    },
    OpenTableCompare {
        left: PathBuf,
        right: PathBuf,
    },
    OpenHexCompare {
        left: PathBuf,
        right: PathBuf,
    },
    OpenFolderCompare {
        left: PathBuf,
        right: PathBuf,
    },
    OpenFolderSync {
        left: PathBuf,
        right: PathBuf,
    },
    /// 新标签页中打开该类型的空会话；`left` 为单个拖入路径的预填。
    NewSession {
        kind: SessionKind,
        left: Option<PathBuf>,
    },
    NewTab,
    CloseTab,
    /// Tools > Options（应用层持有对话框与偏好）。
    Options,
    Exit,
}
pub fn table_paths(left: &std::path::Path, right: &std::path::Path) -> bool {
    let registry = crate::table::formats::load_registry()
        .unwrap_or_else(|_| bc_formats::registry::FormatRegistry::with_builtins());
    [left, right]
        .iter()
        .any(|path| crate::table::formats::table_path(path, &registry))
}
/// 内置 hex 格式掩码（初版 `*.bin`；其余未知扩展仍走 Text）。
pub fn hex_paths(left: &std::path::Path, right: &std::path::Path) -> bool {
    [left, right].iter().any(|path| {
        path.extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("bin"))
    })
}
pub(crate) fn routed(left: PathBuf, right: PathBuf) -> HomeAction {
    if left.is_dir() && right.is_dir() {
        HomeAction::OpenFolderCompare { left, right }
    } else if hex_paths(&left, &right) {
        HomeAction::OpenHexCompare { left, right }
    } else if table_paths(&left, &right) {
        HomeAction::OpenTableCompare { left, right }
    } else {
        HomeAction::OpenTextCompare { left, right }
    }
}
/// Session types offered on Home, in icon-grid order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    /// Routed by the paths (folders / hex mask / table formats / text).
    Auto,
    FolderCompare,
    FolderSync,
    TextCompare,
    TableCompare,
    HexCompare,
}
const KINDS: [SessionKind; 5] = [
    SessionKind::FolderCompare,
    SessionKind::FolderSync,
    SessionKind::TextCompare,
    SessionKind::TableCompare,
    SessionKind::HexCompare,
];
impl SessionKind {
    /// Auto 按预填路径定型（文件夹 / 表格格式 / 文本）；无路径时取图标网格首项。
    fn resolve(self, path: Option<&std::path::Path>) -> Self {
        match (self, path) {
            (Self::Auto, Some(p)) if p.is_dir() => Self::FolderCompare,
            (Self::Auto, Some(p)) if hex_paths(p, p) => Self::HexCompare,
            (Self::Auto, Some(p)) if table_paths(p, p) => Self::TableCompare,
            (Self::Auto, Some(_)) => Self::TextCompare,
            (Self::Auto, None) => Self::FolderCompare,
            (kind, _) => kind,
        }
    }
    fn new_session(self, left: Option<PathBuf>) -> HomeAction {
        HomeAction::NewSession {
            kind: self.resolve(left.as_deref()),
            left,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Compare",
            Self::FolderCompare => "Folder Compare",
            Self::FolderSync => "Folder Sync",
            Self::TextCompare => "Text Compare",
            Self::TableCompare => "Table Compare",
            Self::HexCompare => "Hex Compare",
        }
    }
    pub fn icon(self) -> SessionIcon {
        match self {
            Self::FolderCompare | Self::Auto => SessionIcon::FolderCompare,
            Self::FolderSync => SessionIcon::FolderSync,
            Self::TextCompare => SessionIcon::TextCompare,
            Self::TableCompare => SessionIcon::TableCompare,
            Self::HexCompare => SessionIcon::HexCompare,
        }
    }
}
/// Session-tree selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Node {
    #[default]
    New,
    AutoSaved,
    Kind(usize),
    /// Auto-saved 下的日期分组（[`DAY_GROUPS`] 下标）。
    Day(usize),
    Recent(usize),
}
/// Auto-saved 日期分组（参考：Today / Yesterday / N days ago / More than 6 days ago）。
const DAY_GROUPS: usize = 8;
fn day_label(group: usize) -> String {
    match group {
        0 => "Today".into(),
        1 => "Yesterday".into(),
        2..=6 => format!("{group} days ago"),
        _ => "More than 6 days ago".into(),
    }
}
/// 本地日历日序号（含该时刻的时区/夏令时偏移）。
fn local_day(time: SystemTime) -> i64 {
    let seconds = match time.duration_since(std::time::UNIX_EPOCH) {
        Ok(after) => after.as_secs() as i64,
        Err(before) => -(before.duration().as_secs() as i64) - 1,
    };
    let offset = bc_script::platform::local(time).offset_minutes;
    (seconds + i64::from(offset) * 60).div_euclid(86_400)
}
/// `when` 相对今天（`today` 为 [`local_day`]）落入的日期分组；未来时刻算 Today。
fn day_group(today: i64, when: SystemTime) -> usize {
    (today - local_day(when)).clamp(0, DAY_GROUPS as i64 - 1) as usize
}
/// 一条最近比较：左右路径与最后一次打开的时刻。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Recent {
    left: PathBuf,
    right: PathBuf,
    when: SystemTime,
}
/// Icons per page of the grid (the reference shows three, then a chevron).
const PAGE: usize = 3;
// Reference geometry (client pixels, 1384x961 window).
const TREE_WIDTH: f32 = 275.0;
const HEADER_BOTTOM: f32 = 60.0;
const TREE_ROW: f32 = 20.0;
/// Session toolbar height below the tree's bottom rule (y785..828).
const TOOLBAR_HEIGHT: f32 = 39.0;
/// White rule plus canvas-colored rows between the body and the panel.
const BODY_GAP: f32 = 4.0;
const PANEL_HEIGHT: f32 = 132.0;
const CELL: Vec2 = Vec2::new(158.0, 100.0);
const TREE_BG: Color32 = Color32::from_rgb(0xE3, 0xE7, 0xED);
const CANVAS_BG: Color32 = Color32::from_rgb(0xF9, 0xF9, 0xF9);
const SELECT_FILL: Color32 = Color32::from_rgb(0xB6, 0xD5, 0xF1);
const SELECT_EDGE: Color32 = Color32::from_rgb(0x00, 0x78, 0xD4);
const EXPANDER: Color32 = Color32::from_rgb(0x8B, 0x8B, 0x8B);
const CHEVRON: Color32 = Color32::from_rgb(0x80, 0x80, 0x80);
const NEW_DRAWER: (Color32, Color32) = (
    Color32::from_rgb(0x9B, 0xD9, 0x7F),
    Color32::from_rgb(0x65, 0x8B, 0x50),
);
const SAVED_DRAWER: (Color32, Color32) = (
    Color32::from_rgb(0xE6, 0xC0, 0x8A),
    Color32::from_rgb(0x9C, 0x77, 0x35),
);

#[derive(Debug)]
pub struct HomeView {
    error: Option<String>,
    recent: Vec<Recent>,
    selected: Node,
    new_open: bool,
    auto_open: bool,
    /// 各日期分组的展开状态（默认仅 Today 展开）。
    day_open: [bool; DAY_GROUPS],
    filter: String,
    page: usize,
    hide_tree: bool,
    hide_panel: bool,
    about: bool,
}
/// 会话树默认展开 New 与 Auto-saved（其下仅 Today 展开）。
impl Default for HomeView {
    fn default() -> Self {
        let mut day_open = [false; DAY_GROUPS];
        day_open[0] = true;
        Self {
            error: None,
            recent: Vec::new(),
            selected: Node::New,
            new_open: true,
            auto_open: true,
            day_open,
            filter: String::new(),
            page: 0,
            hide_tree: false,
            hide_panel: false,
            about: false,
        }
    }
}
fn item(label: &str, id: &str, enabled: bool) -> MenuItem {
    MenuItem {
        enabled,
        ..MenuItem::action(label, id)
    }
}
fn check(label: &str, id: &str, checked: bool, enabled: bool) -> MenuItem {
    MenuItem {
        kind: MenuItemKind::Check { checked },
        enabled,
        ..MenuItem::action(label, id)
    }
}
fn short(mut item: MenuItem, shortcut: &str) -> MenuItem {
    item.shortcut = Some(shortcut.into());
    item
}
fn sep() -> MenuItem {
    MenuItem {
        kind: MenuItemKind::Separator,
        enabled: false,
        ..MenuItem::action("", "")
    }
}
fn sub(label: &str, enabled: bool, children: Vec<MenuItem>) -> MenuItem {
    MenuItem {
        kind: MenuItemKind::Submenu(children),
        enabled,
        ..MenuItem::action(label, "")
    }
}
impl HomeView {
    pub fn new() -> Self {
        Self {
            recent: load_recent(),
            ..Default::default()
        }
    }
    #[cfg(test)]
    pub(crate) fn with_recent(recent: Vec<(PathBuf, PathBuf)>) -> Self {
        Self {
            recent: recent
                .into_iter()
                .map(|(left, right)| Recent {
                    left,
                    right,
                    when: SystemTime::now(),
                })
                .collect(),
            ..Default::default()
        }
    }
    #[cfg(test)]
    pub(crate) fn new_open(&self) -> bool {
        self.new_open
    }
    /// 重读最近比较（其他标签页刚记录了新的比较）。
    pub fn refresh_recent(&mut self) {
        self.recent = load_recent();
        if matches!(self.selected, Node::Recent(i) if i >= self.recent.len()) {
            self.selected = Node::AutoSaved;
        }
    }
    pub fn set_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
    }
    pub fn record(left: &std::path::Path, right: &std::path::Path) {
        let left = std::fs::canonicalize(left).unwrap_or_else(|_| left.to_path_buf());
        let right = std::fs::canonicalize(right).unwrap_or_else(|_| right.to_path_buf());
        let mut recent = load_recent();
        recent.retain(|p| p.left != left || p.right != right);
        recent.insert(
            0,
            Recent {
                left,
                right,
                when: SystemTime::now(),
            },
        );
        recent.truncate(12);
        save_recent(&recent);
    }
    /// Remove one recent pair (session toolbar "−").
    fn forget(&mut self, index: usize) {
        if index < self.recent.len() {
            self.recent.remove(index);
            save_recent(&self.recent);
        }
        self.selected = Node::AutoSaved;
    }
    /// Menu inventory of the reference Home; unimplemented entries are
    /// disabled and brand-specific entries are replaced by Matcha's own.
    fn menus(&self) -> Vec<MenuSpec> {
        let menu = |title: &str, mnemonic: char, items: Vec<MenuItem>| MenuSpec {
            title: title.into(),
            mnemonic: Some(mnemonic),
            items,
        };
        vec![
            menu(
                "Session",
                's',
                vec![
                    sub(
                        "New Session",
                        true,
                        vec![
                            item("Folder Compare", "new:folder", true),
                            item("Folder Sync", "new:sync", true),
                            item("Text Compare", "new:text", true),
                            item("Hex Compare", "", false),
                            item("Media Compare", "", false),
                            item("Picture Compare", "", false),
                            item("Table Compare", "new:table", true),
                            item("Hex Compare", "new:hex", true),
                            item("Version Compare", "", false),
                        ],
                    ),
                    short(item("New Tab", "new-tab", true), "Ctrl+T"),
                    item("New Window", "", false),
                    short(item("Open Session", "", false), "Shift+Ctrl+O"),
                    sep(),
                    sub(
                        "Load Workspace",
                        false,
                        vec![item("Manage Workspaces...", "", false)],
                    ),
                    item("Save Workspace As...", "", false),
                    sep(),
                    short(item("Close Tab", "close-tab", true), "Ctrl+W"),
                    sep(),
                    short(item("Exit", "exit", true), "Ctrl+Q"),
                ],
            ),
            menu(
                "View",
                'v',
                vec![
                    check(
                        "Show Session Management",
                        "view:tree",
                        !self.hide_tree,
                        true,
                    ),
                    check("Maximize Session Management", "", true, false),
                    check("Show Help Panel", "view:panel", !self.hide_panel, true),
                ],
            ),
            menu(
                "Tools",
                't',
                vec![
                    item("Options...", "options", true),
                    item("File Formats...", "", false),
                    item("Profiles...", "", false),
                    sep(),
                    item("Export Settings...", "", false),
                    item("Import Settings...", "", false),
                    item("Restore Factory Defaults...", "", false),
                    sep(),
                    item("Save Snapshot...", "", false),
                    item("Edit Text File", "", false),
                    item("View Patch...", "", false),
                ],
            ),
            menu(
                "Help",
                'h',
                vec![
                    item("Contents", "", false),
                    short(item("Context Sensitive Help", "", false), "F1"),
                    sep(),
                    item("Check for Updates", "", false),
                    sep(),
                    item("About Matcha", "about", true),
                ],
            ),
        ]
    }
    fn command(&mut self, id: &str) -> Option<HomeAction> {
        match id {
            "new:folder" => return Some(SessionKind::FolderCompare.new_session(None)),
            "new:sync" => return Some(SessionKind::FolderSync.new_session(None)),
            "new:text" => return Some(SessionKind::TextCompare.new_session(None)),
            "new:table" => return Some(SessionKind::TableCompare.new_session(None)),
            "new:hex" => return Some(SessionKind::HexCompare.new_session(None)),
            "new-tab" => return Some(HomeAction::NewTab),
            "close-tab" => return Some(HomeAction::CloseTab),
            "options" => return Some(HomeAction::Options),
            "view:tree" => self.hide_tree = !self.hide_tree,
            "view:panel" => self.hide_panel = !self.hide_panel,
            "about" => self.about = true,
            "exit" => return Some(HomeAction::Exit),
            _ => {}
        }
        None
    }
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<HomeAction> {
        let mut action = None;
        let dropped = ui.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect::<Vec<_>>()
        });
        if dropped.len() >= 2 {
            action = Some(routed(dropped[0].clone(), dropped[1].clone()));
        } else if let Some(path) = dropped.first() {
            // 单个路径：按其类型开空会话标签页，并预填左侧。
            action = Some(SessionKind::Auto.new_session(Some(path.clone())));
        }
        if ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Q)) {
            action = Some(HomeAction::Exit);
        }
        let full = ui.max_rect();
        let o = full.min.to_vec2();
        let span = |x0: f32, y0: f32, x1: f32, y1: f32| {
            Rect::from_min_max(Pos2::new(x0, y0) + o, Pos2::new(x1, y1) + o)
        };
        let painter = ui.painter_at(full);
        painter.rect_filled(full, 0.0, CANVAS_BG);
        let mut menu_action = None;
        let menus = self.menus();
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(span(0.0, 0.0, full.width(), theme::MENU_HEIGHT)),
            |ui| menu_action = menubar::show(ui, &menus),
        );
        if let Some(id) = menu_action {
            action = self.command(&id).or(action);
        }
        let (w, h) = (full.width(), full.height());
        painter.rect_filled(span(0.0, 27.0, w, 28.0), 0.0, theme::BORDER);
        painter.rect_filled(span(0.0, 28.0, w, 29.0), 0.0, Color32::WHITE);
        let body_bottom = if self.hide_panel { h } else { h - PANEL_HEIGHT };
        let content_bottom = body_bottom - BODY_GAP;
        painter.rect_filled(
            span(0.0, content_bottom, w, content_bottom + 1.0),
            0.0,
            Color32::WHITE,
        );
        let canvas_left = if self.hide_tree {
            0.0
        } else {
            self.tree_panel(ui, &painter, o, content_bottom, &mut action);
            TREE_WIDTH + 1.0
        };
        self.canvas(
            ui,
            &painter,
            span(canvas_left, 29.0, w, content_bottom),
            &mut action,
        );
        if !self.hide_panel {
            self.help_panel(ui, &painter, span(0.0, body_bottom, w, h));
        }
        if self.about {
            egui::Window::new("About Matcha")
                .open(&mut self.about)
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.label(format!("Matcha {}", env!("CARGO_PKG_VERSION")));
                    ui.label("Folder, text and table comparison.");
                });
        }
        action
    }
    fn tree_panel(
        &mut self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        o: Vec2,
        content_bottom: f32,
        action: &mut Option<HomeAction>,
    ) {
        let at = |x: f32, y: f32| Pos2::new(x, y) + o;
        let span = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(at(x0, y0), at(x1, y1));
        let font = FontId::proportional(theme::UI_SIZE);
        // "Sessions" header: #F9 / #F0 then one step per two rows to #E1.
        let mut mesh = egui::Mesh::default();
        for y in 29..60u8 {
            let v = match y {
                29 => 0xF9,
                30 => 0xF0,
                31 => 0xEF,
                _ => 0xEE - (y - 32) / 2,
            };
            mesh.add_colored_rect(
                span(0.0, y as f32, TREE_WIDTH, y as f32 + 1.0),
                Color32::from_gray(v),
            );
        }
        painter.add(egui::Shape::mesh(mesh));
        painter.rect_filled(
            span(0.0, HEADER_BOTTOM, TREE_WIDTH, HEADER_BOTTOM + 1.0),
            0.0,
            theme::BORDER,
        );
        painter.text(
            at(10.0, 43.5),
            egui::Align2::LEFT_CENTER,
            "Sessions",
            font.clone(),
            Color32::BLACK,
        );
        let toolbar_top = content_bottom - TOOLBAR_HEIGHT - 1.0;
        painter.rect_filled(
            span(0.0, HEADER_BOTTOM + 1.0, TREE_WIDTH, toolbar_top),
            0.0,
            TREE_BG,
        );
        painter.rect_filled(
            span(0.0, toolbar_top, TREE_WIDTH, toolbar_top + 1.0),
            0.0,
            theme::BORDER,
        );
        painter.rect_filled(
            span(0.0, toolbar_top + 1.0, TREE_WIDTH, content_bottom),
            0.0,
            theme::PANEL_BG,
        );
        painter.rect_filled(
            span(TREE_WIDTH, 28.0, TREE_WIDTH + 1.0, content_bottom),
            0.0,
            theme::BORDER,
        );
        let filter = self.filter.trim().to_lowercase();
        let auto_open = self.auto_open || !filter.is_empty();
        let mut rows: Vec<(Node, usize, String)> = vec![(Node::New, 0, "New".into())];
        if self.new_open {
            rows.extend(
                KINDS
                    .iter()
                    .enumerate()
                    .map(|(i, kind)| (Node::Kind(i), 1, kind.label().into())),
            );
        }
        rows.push((Node::AutoSaved, 0, "Auto-saved".into()));
        if auto_open {
            let name = |p: &PathBuf| {
                p.file_name()
                    .map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into())
            };
            let today = local_day(SystemTime::now());
            let mut groups: [Vec<usize>; DAY_GROUPS] = Default::default();
            for (i, item) in self.recent.iter().enumerate() {
                let full = format!("{} {}", item.left.display(), item.right.display());
                if filter.is_empty() || full.to_lowercase().contains(&filter) {
                    groups[day_group(today, item.when)].push(i);
                }
            }
            for (group, items) in groups.iter().enumerate() {
                if items.is_empty() {
                    continue;
                }
                rows.push((Node::Day(group), 1, day_label(group)));
                if self.day_open[group] || !filter.is_empty() {
                    rows.extend(items.iter().map(|&i| {
                        let item = &self.recent[i];
                        let label = format!("{} <--> {}", name(&item.left), name(&item.right));
                        (Node::Recent(i), 2, label)
                    }));
                }
            }
        }
        let clip = painter.with_clip_rect(span(0.0, HEADER_BOTTOM + 1.0, TREE_WIDTH, toolbar_top));
        let glyph = |node: Node, indent: f32, cy: f32| match node {
            Node::New | Node::AutoSaved | Node::Day(_) => {
                let (fill, handle) = if node == Node::New {
                    NEW_DRAWER
                } else {
                    SAVED_DRAWER
                };
                icons::paint_drawer(
                    &clip,
                    span(23.0 + indent, cy - 5.5, 37.0 + indent, cy + 5.5),
                    fill,
                    handle,
                );
            }
            Node::Kind(i) => icons::paint_session(
                &clip.with_clip_rect(span(23.0 + indent, cy - 7.0, 37.0 + indent, cy + 7.0)),
                span(14.0 + indent, cy - 16.0, 46.0 + indent, cy + 16.0),
                KINDS[i].icon(),
            ),
            Node::Recent(_) => {}
        };
        for (index, (node, depth, label)) in rows.into_iter().enumerate() {
            let top = HEADER_BOTTOM + 1.0 + index as f32 * TREE_ROW;
            if top + TREE_ROW > toolbar_top {
                break;
            }
            let group = matches!(node, Node::New | Node::AutoSaved | Node::Day(_));
            let indent = depth as f32 * 19.0;
            let cy = top + TREE_ROW / 2.0;
            let response = ui.interact(
                span(0.0, top, TREE_WIDTH, top + TREE_ROW),
                egui::Id::unique(("home-tree", index)),
                Sense::click(),
            );
            if group {
                // Expander chevron ">" (open: "v") and the dotted stub.
                let open = match node {
                    Node::New => self.new_open,
                    Node::Day(g) => self.day_open[g] || !filter.is_empty(),
                    _ => auto_open,
                };
                let c = at(10.5 + indent, cy);
                let points = if open {
                    vec![
                        c + Vec2::new(-3.0, -1.5),
                        c + Vec2::new(0.0, 1.5),
                        c + Vec2::new(3.0, -1.5),
                    ]
                } else {
                    vec![
                        c + Vec2::new(-1.5, -3.5),
                        c + Vec2::new(1.5, 0.0),
                        c + Vec2::new(-1.5, 3.5),
                    ]
                };
                clip.add(egui::Shape::line(points, Stroke::new(1.0, EXPANDER)));
                let mut dots = egui::Mesh::default();
                for x in [15.0 + indent, 17.0 + indent, 19.0 + indent] {
                    dots.add_colored_rect(span(x, cy, x + 1.0, cy + 1.0), EXPANDER);
                }
                clip.add(egui::Shape::mesh(dots));
            }
            let galley = ui
                .painter()
                .layout_no_wrap(label, font.clone(), Color32::BLACK);
            let text_left = 43.0 + indent;
            if self.selected == node {
                let sel = span(
                    20.0 + indent,
                    top,
                    text_left + galley.size().x + 4.0,
                    top + TREE_ROW,
                );
                clip.rect_filled(sel, 0.0, SELECT_FILL);
                clip.rect_stroke(sel, 0.0, Stroke::new(1.0, SELECT_EDGE), StrokeKind::Inside);
            }
            glyph(node, indent, cy);
            clip.galley(
                at(text_left, cy - galley.size().y / 2.0),
                galley,
                Color32::BLACK,
            );
            if response.clicked() {
                self.selected = node;
                let on_expander = response
                    .interact_pointer_pos()
                    .is_some_and(|p| p.x < at(20.0 + indent, 0.0).x);
                if group && on_expander {
                    self.toggle(node);
                }
            }
            if response.double_clicked() {
                match node {
                    Node::New | Node::AutoSaved | Node::Day(_) => self.toggle(node),
                    Node::Kind(i) => *action = Some(KINDS[i].new_session(None)),
                    Node::Recent(i) => {
                        let item = self.recent[i].clone();
                        *action = Some(routed(item.left, item.right));
                    }
                }
            }
        }
        self.session_toolbar(ui, painter, o, toolbar_top, action);
    }
    fn toggle(&mut self, node: Node) {
        match node {
            Node::New => self.new_open = !self.new_open,
            Node::AutoSaved => self.auto_open = !self.auto_open,
            Node::Day(g) => self.day_open[g] = !self.day_open[g],
            _ => {}
        }
    }
    /// "+" (new session for the selected type) / "−" (forget the selected
    /// recent comparison) and the tree filter box.
    fn session_toolbar(
        &mut self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        o: Vec2,
        toolbar_top: f32,
        action: &mut Option<HomeAction>,
    ) {
        let span = |x0: f32, y0: f32, x1: f32, y1: f32| {
            Rect::from_min_max(Pos2::new(x0, y0) + o, Pos2::new(x1, y1) + o)
        };
        let bt = toolbar_top + 6.0;
        let plus = span(6.0, bt, 36.0, bt + 28.0);
        let minus = span(38.0, bt, 67.0, bt + 28.0);
        let removable = matches!(self.selected, Node::Recent(_));
        for (rect, enabled, cross) in [(plus, true, true), (minus, removable, false)] {
            let (edge, fill, ink) = if enabled {
                (0xD0, 0xFD, 0x4F)
            } else {
                (0xE9, 0xF9, 0xB3)
            };
            let ink = Color32::from_gray(ink);
            painter.rect_filled(rect, 3.0, Color32::from_gray(fill));
            painter.rect_stroke(
                rect,
                3.0,
                Stroke::new(1.0, Color32::from_gray(edge)),
                StrokeKind::Inside,
            );
            let c = rect.center();
            painter.rect_filled(Rect::from_center_size(c, Vec2::new(8.0, 2.0)), 0.0, ink);
            if cross {
                painter.rect_filled(Rect::from_center_size(c, Vec2::new(2.0, 8.0)), 0.0, ink);
            }
        }
        let id = egui::Id::unique("home-session-toolbar");
        if ui
            .interact(plus, id.with("home-plus"), Sense::click())
            .clicked()
        {
            let kind = match self.selected {
                Node::Kind(i) => KINDS[i],
                _ => SessionKind::Auto,
            };
            *action = Some(kind.new_session(None));
        }
        if removable
            && ui
                .interact(minus, id.with("home-minus"), Sense::click())
                .clicked()
        {
            if let Node::Recent(i) = self.selected {
                self.forget(i);
            }
        }
        let search = span(73.0, bt + 1.0, 271.0, bt + 26.0);
        painter.rect_filled(search, 0.0, Color32::WHITE);
        painter.rect_stroke(
            search,
            0.0,
            Stroke::new(1.0, Color32::from_gray(0xEC)),
            StrokeKind::Inside,
        );
        painter.rect_filled(
            Rect::from_min_max(
                search.left_bottom() - Vec2::new(0.0, 1.0),
                search.right_bottom(),
            ),
            0.0,
            Color32::from_gray(0x83),
        );
        // Magnifier: ring with a handle toward the lower left.
        let ring = search.left_top() + Vec2::new(12.0, 10.0);
        let ink = Color32::from_gray(0x66);
        painter.circle_stroke(ring, 3.5, Stroke::new(1.6, ink));
        painter.line_segment(
            [ring + Vec2::new(-2.5, 2.5), ring + Vec2::new(-5.5, 5.5)],
            Stroke::new(2.0, ink),
        );
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(span(93.0, bt + 5.0, 268.0, bt + 23.0)),
            |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.filter)
                        .frame(egui::Frame::NONE)
                        .desired_width(ui.available_width()),
                );
            },
        );
    }
    fn canvas(
        &mut self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        rect: Rect,
        action: &mut Option<HomeAction>,
    ) {
        let font = FontId::proportional(theme::UI_SIZE);
        // Reference: text centered 2.5 px and the grid 4 px right of the
        // canvas middle; the grid row sits 56 px above it.
        let cx = rect.center().x + 2.5;
        let grid_cx = rect.center().x + 4.0;
        let icon_top = (rect.center().y - 56.0).round();
        for (text, y) in [
            ("Drag folders or files here", icon_top - 64.0),
            ("or click a session icon to begin:", icon_top - 42.0),
        ] {
            painter.text(
                Pos2::new(cx, y),
                egui::Align2::CENTER_CENTER,
                text,
                font.clone(),
                Color32::BLACK,
            );
        }
        let pages = KINDS.len().div_ceil(PAGE);
        self.page = self.page.min(pages - 1);
        let shown = &KINDS[self.page * PAGE..((self.page + 1) * PAGE).min(KINDS.len())];
        for (i, kind) in shown.iter().enumerate() {
            let center_x = (grid_cx + (i as f32 - (PAGE as f32 - 1.0) / 2.0) * CELL.x).round();
            let cell =
                Rect::from_min_size(Pos2::new(center_x - CELL.x / 2.0, icon_top - 4.0), CELL);
            let response = ui
                .interact(cell, egui::Id::unique(("home-kind", i)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            icons::paint_session(
                painter,
                Rect::from_min_size(Pos2::new(center_x - 32.0, icon_top), Vec2::splat(64.0)),
                kind.icon(),
            );
            painter.text(
                Pos2::new(center_x, icon_top + 77.0),
                egui::Align2::CENTER_CENTER,
                kind.label(),
                font.clone(),
                Color32::BLACK,
            );
            if response.clicked() {
                *action = Some(kind.new_session(None));
            }
        }
        // Page chevrons, only when the types do not fit one page.
        for (right, show) in [(true, self.page + 1 < pages), (false, self.page > 0)] {
            if !show {
                continue;
            }
            let dir = if right { 1.0 } else { -1.0 };
            let c = Pos2::new(grid_cx + 461.5 * dir, icon_top + 31.5);
            let points = vec![
                c + Vec2::new(-8.0 * dir, -15.5),
                c + Vec2::new(8.0 * dir, 0.0),
                c + Vec2::new(-8.0 * dir, 15.5),
            ];
            let hit = Rect::from_points(&points).expand(8.0);
            for p in &points {
                painter.circle_filled(*p, 3.0, CHEVRON);
            }
            painter.add(egui::Shape::line(points, Stroke::new(6.0, CHEVRON)));
            if ui
                .interact(hit, egui::Id::unique(("home-page", right)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                if right {
                    self.page += 1;
                } else {
                    self.page -= 1;
                }
            }
        }
        if let Some(error) = &self.error {
            painter.text(
                Pos2::new(cx, icon_top + 110.0),
                egui::Align2::CENTER_CENTER,
                error,
                font,
                Color32::from_rgb(200, 0, 0),
            );
        }
    }
    /// Help panel: the reference's three-column strip with a close button,
    /// carrying Matcha's own neutral content (no brand color, no logo).
    fn help_panel(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, rect: Rect) {
        painter.rect_filled(rect, 0.0, theme::PANEL_BG);
        painter.rect_filled(
            Rect::from_min_size(rect.min, Vec2::new(rect.width(), 1.0)),
            0.0,
            theme::BORDER,
        );
        let font = FontId::proportional(theme::UI_SIZE);
        let columns: [(f32, &str, [&str; 2]); 3] = [
            (
                21.0,
                "GETTING STARTED",
                [
                    "Drop two files or two folders on",
                    "this window to compare them.",
                ],
            ),
            (
                284.0,
                "SESSIONS",
                [
                    "Recent comparisons are kept under",
                    "Auto-saved in the Sessions tree.",
                ],
            ),
            (
                536.0,
                "ABOUT",
                ["Matcha compares folders, text and", "tables side by side."],
            ),
        ];
        for (x, title, lines) in columns {
            painter.text(
                rect.min + Vec2::new(x, 31.5),
                egui::Align2::LEFT_CENTER,
                title,
                font.clone(),
                Color32::from_gray(0x40),
            );
            for (i, line) in lines.iter().enumerate() {
                painter.text(
                    rect.min + Vec2::new(x, 53.5 + 17.0 * i as f32),
                    egui::Align2::LEFT_CENTER,
                    *line,
                    font.clone(),
                    Color32::from_gray(0x20),
                );
            }
        }
        for x in [264.0, 516.0, 769.0] {
            painter.rect_filled(
                Rect::from_min_size(
                    rect.min + Vec2::new(x, 1.0),
                    Vec2::new(1.0, rect.height() - 1.0),
                ),
                0.0,
                Color32::from_gray(0xD0),
            );
        }
        // Close (×) hides the panel, like View > Show Help Panel.
        let c = Pos2::new(rect.right() - 16.5, rect.top() + 13.5);
        let ink = Stroke::new(2.0, Color32::from_gray(0x40));
        painter.line_segment([c + Vec2::new(-4.0, -4.0), c + Vec2::new(4.0, 4.0)], ink);
        painter.line_segment([c + Vec2::new(-4.0, 4.0), c + Vec2::new(4.0, -4.0)], ink);
        if ui
            .interact(
                Rect::from_center_size(c, Vec2::splat(16.0)),
                egui::Id::unique("home-panel-close"),
                Sense::click(),
            )
            .clicked()
        {
            self.hide_panel = true;
        }
    }
}
/// `recent-text.txt` v2：首行标记，之后每条一行 `纪元秒\t左\t右`（路径不含制表符）。
const RECENT_V2: &str = "# matcha recent v2";
fn save_recent(recent: &[Recent]) {
    if let Some(file) = recent_path() {
        if let Some(parent) = file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(file, format_recent(recent));
    }
}
fn format_recent(recent: &[Recent]) -> String {
    let mut text = format!("{RECENT_V2}\n");
    for item in recent {
        let seconds = item
            .when
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        text += &format!(
            "{seconds}\t{}\t{}\n",
            item.left.display(),
            item.right.display()
        );
    }
    text
}
fn recent_path() -> Option<PathBuf> {
    // 单元测试会经 app 流程记录比较，不能写进用户的真实最近列表。
    if cfg!(test) {
        return Some(
            std::env::temp_dir().join(format!("matcha-test-recent-{}.txt", std::process::id())),
        );
    }
    bc_formats::registry::user_formats_dir()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("recent-text.txt")))
}
fn load_recent() -> Vec<Recent> {
    let Some(file) = recent_path() else {
        return Vec::new();
    };
    let Ok(text) = std::fs::read_to_string(&file) else {
        return Vec::new();
    };
    // 旧格式（每对两行、无时间）以文件修改时间为准。
    let modified = std::fs::metadata(&file)
        .and_then(|m| m.modified())
        .unwrap_or_else(|_| SystemTime::now());
    parse_recent(&text, modified)
}
fn parse_recent(text: &str, legacy_time: SystemTime) -> Vec<Recent> {
    let mut lines = text.lines();
    if text.starts_with(RECENT_V2) {
        lines.next();
        return lines
            .filter_map(|line| {
                let mut parts = line.splitn(3, '\t');
                let seconds = parts.next()?.parse::<u64>().ok()?;
                let (left, right) = (parts.next()?, parts.next()?);
                Some(Recent {
                    left: left.into(),
                    right: right.into(),
                    when: std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds),
                })
            })
            .take(12)
            .collect();
    }
    let lines: Vec<_> = lines.collect();
    lines
        .as_chunks::<2>()
        .0
        .iter()
        .take(12)
        .map(|p| Recent {
            left: p[0].into(),
            right: p[1].into(),
            when: legacy_time,
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    thread_local! { static TICK: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) }; }
    fn frame(
        ctx: &egui::Context,
        home: &mut HomeView,
        events: Vec<egui::Event>,
    ) -> Option<HomeAction> {
        let mut action = None;
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1384.0, 961.0))),
                events,
                // Advancing time lets egui pair presses into double clicks.
                time: Some(TICK.with(|t| {
                    t.set(t.get() + 0.05);
                    t.get()
                })),
                ..Default::default()
            },
            |ui| action = home.ui(ui),
        )
        .drop_without_applying_deltas();
        action
    }
    fn click(ctx: &egui::Context, home: &mut HomeView, pos: Pos2) -> Option<HomeAction> {
        frame(ctx, home, vec![egui::Event::PointerMoved(pos)]);
        let press = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        frame(ctx, home, vec![press(true)]).or(frame(ctx, home, vec![press(false)]))
    }
    #[test]
    fn icon_grid_pages_and_opens_an_empty_session_tab() {
        let ctx = egui::Context::default();
        let mut home = HomeView::default();
        frame(&ctx, &mut home, vec![]);
        let empty = |kind| Some(HomeAction::NewSession { kind, left: None });
        // Page one: Folder Compare, Folder Sync, Text Compare (reference y403).
        for (x, kind) in [
            (676.0, SessionKind::FolderCompare),
            (834.0, SessionKind::FolderSync),
            (992.0, SessionKind::TextCompare),
        ] {
            assert_eq!(click(&ctx, &mut home, Pos2::new(x, 403.0)), empty(kind));
        }

        // Four types: the chevron pages to Table Compare and back.
        click(&ctx, &mut home, Pos2::new(1295.0, 403.0));
        assert_eq!(home.page, 1);
        assert_eq!(
            click(&ctx, &mut home, Pos2::new(676.0, 403.0)),
            empty(SessionKind::TableCompare)
        );
        click(&ctx, &mut home, Pos2::new(1668.0 - 1295.0, 403.0));
        assert_eq!(home.page, 0);
        // Session toolbar "+": the selected New-tree type, else Folder Compare.
        assert_eq!(
            click(&ctx, &mut home, Pos2::new(21.0, 804.0)),
            empty(SessionKind::FolderCompare)
        );
    }
    #[test]
    fn single_dropped_path_opens_a_prefilled_session_by_type() {
        let dir = std::env::temp_dir();
        assert_eq!(
            SessionKind::Auto.new_session(Some(dir.clone())),
            HomeAction::NewSession {
                kind: SessionKind::FolderCompare,
                left: Some(dir)
            }
        );
        let text = PathBuf::from("notes.txt");
        assert_eq!(
            SessionKind::Auto.new_session(Some(text.clone())),
            HomeAction::NewSession {
                kind: SessionKind::TextCompare,
                left: Some(text)
            }
        );
        // An explicit type is never re-routed by the path.
        assert_eq!(
            SessionKind::FolderSync.resolve(Some(std::path::Path::new("a.csv"))),
            SessionKind::FolderSync
        );
    }
    #[test]
    fn bin_mask_routes_to_hex_and_unknown_to_text() {
        let corpus = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/reference-parity/hex-compare/corpus/"
        );
        let (bin_l, bin_r) = (
            PathBuf::from(format!("{corpus}l2.bin")),
            PathBuf::from(format!("{corpus}r2.bin")),
        );
        assert!(matches!(
            routed(bin_l.clone(), bin_r.clone()),
            HomeAction::OpenHexCompare { .. }
        ));
        // 未知扩展（非目录、非 hex 掩码、非表格格式）仍路由到 Text。
        let dir = std::env::temp_dir().join(format!("matcha-route-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a.xyz123"), dir.join("b.xyz123"));
        std::fs::write(&a, b"x").unwrap();
        std::fs::write(&b, b"y").unwrap();
        assert!(matches!(
            routed(a.clone(), b.clone()),
            HomeAction::OpenTextCompare { .. }
        ));
        let _ = std::fs::remove_dir_all(&dir);
        // Home 会话网格与 New Session 菜单含 Hex Compare。
        assert!(KINDS.contains(&SessionKind::HexCompare));
        let mut home = HomeView::default();
        assert_eq!(
            home.command("new:hex"),
            Some(HomeAction::NewSession {
                kind: SessionKind::HexCompare,
                left: None
            })
        );
    }

    #[test]
    fn session_kinds_menus_and_tree_keep_home_actions() {
        let (l, r) = (PathBuf::from("a"), PathBuf::from("b"));
        let mut home = HomeView::default();
        let menus = home.menus();
        assert_eq!(
            menus.iter().map(|m| m.title.as_str()).collect::<Vec<_>>(),
            ["Session", "View", "Tools", "Help"]
        );
        assert_eq!(menus[3].items.last().unwrap().label, "About Matcha");
        assert_eq!(
            home.command("new:sync"),
            Some(HomeAction::NewSession {
                kind: SessionKind::FolderSync,
                left: None
            })
        );
        assert_eq!(home.command("new-tab"), Some(HomeAction::NewTab));
        assert_eq!(home.command("close-tab"), Some(HomeAction::CloseTab));
        assert_eq!(home.command("exit"), Some(HomeAction::Exit));
        assert_eq!(home.command("options"), Some(HomeAction::Options));
        home.command("view:panel");
        assert!(home.hide_panel);
        // Tree (expanded by default): New, its 5 types, Auto-saved, Today,
        // then the recent pair (row 8, y221..241); double-click opens.
        home.recent = HomeView::with_recent(vec![(l.clone(), r.clone())]).recent;
        assert!(home.new_open && home.auto_open && home.day_open[0]);
        let ctx = egui::Context::default();
        frame(&ctx, &mut home, vec![]);
        // Expander of "Auto-saved" (row 6, y181..201) collapses and reopens it.
        click(&ctx, &mut home, Pos2::new(10.0, 191.0));
        assert!(!home.auto_open);
        for _ in 0..8 {
            frame(&ctx, &mut home, vec![]);
        }
        click(&ctx, &mut home, Pos2::new(10.0, 191.0));
        assert!(home.auto_open);
        for _ in 0..8 {
            frame(&ctx, &mut home, vec![]);
        }
        click(&ctx, &mut home, Pos2::new(100.0, 231.0));
        assert_eq!(home.selected, Node::Recent(0));
        let pos = Pos2::new(100.0, 231.0);
        let press = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        // Let the earlier clicks age out (egui would count a third quick
        // click as a triple click).
        for _ in 0..8 {
            frame(&ctx, &mut home, vec![]);
        }
        let mut action = None;
        for pressed in [true, false, true, false] {
            action = action.or(frame(&ctx, &mut home, vec![press(pressed)]));
        }
        assert_eq!(
            action,
            Some(HomeAction::OpenTextCompare { left: l, right: r })
        );
    }
    #[test]
    fn auto_saved_groups_recent_pairs_by_local_day() {
        use std::time::Duration;
        let now = SystemTime::now();
        let today = local_day(now);
        let days = |n: u64| now - Duration::from_secs(n * 86_400);
        assert_eq!(day_group(today, now), 0);
        assert_eq!(day_group(today, now + Duration::from_secs(3 * 86_400)), 0);
        assert_eq!(day_group(today, days(1)), 1);
        assert_eq!(day_group(today, days(5)), 5);
        assert_eq!(day_group(today, days(30)), DAY_GROUPS - 1);
        assert_eq!(
            (0..DAY_GROUPS).map(day_label).collect::<Vec<_>>(),
            [
                "Today",
                "Yesterday",
                "2 days ago",
                "3 days ago",
                "4 days ago",
                "5 days ago",
                "6 days ago",
                "More than 6 days ago"
            ]
        );
        // v2 round trip keeps whole seconds; the legacy two-line format takes
        // the file time.
        let when = std::time::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let items = vec![Recent {
            left: PathBuf::from(r"C:\a b\left.xlsx"),
            right: PathBuf::from(r"C:\right.xlsx"),
            when,
        }];
        assert_eq!(parse_recent(&format_recent(&items), now), items);
        assert_eq!(
            parse_recent("a.txt\nb.txt\n", when),
            vec![Recent {
                left: "a.txt".into(),
                right: "b.txt".into(),
                when
            }]
        );
    }
    #[test]
    fn tree_shows_day_groups_with_only_today_open() {
        use std::time::Duration;
        let now = SystemTime::now();
        let pair = |l: &str, r: &str, when| Recent {
            left: l.into(),
            right: r.into(),
            when,
        };
        let mut home = HomeView {
            recent: vec![
                pair("left.xlsx", "right.xlsx", now),
                pair("l2.bin", "r2.bin", now - Duration::from_secs(86_400)),
            ],
            ..Default::default()
        };
        let ctx = egui::Context::default();
        frame(&ctx, &mut home, vec![]);
        // Rows: New + 5 types (0..6), Auto-saved (6), Today (7), pair (8),
        // Yesterday (9, collapsed). Clicking the Yesterday expander opens it.
        click(&ctx, &mut home, Pos2::new(29.0, 251.0));
        assert!(home.day_open[1]);
        for _ in 0..8 {
            frame(&ctx, &mut home, vec![]);
        }
        click(&ctx, &mut home, Pos2::new(100.0, 271.0));
        assert_eq!(home.selected, Node::Recent(1));
    }
}
