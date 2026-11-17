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
/// 文件对的比较类型：统一走 [`bc_formats::route::pair_view`]（表格 → 十六进制 → 文本）。
pub fn file_view(left: &std::path::Path, right: &std::path::Path) -> bc_formats::route::FileView {
    let registry = crate::table::formats::load_registry()
        .unwrap_or_else(|_| bc_formats::registry::FormatRegistry::with_builtins());
    bc_formats::route::pair_view(left, right, &registry)
}
pub fn table_paths(left: &std::path::Path, right: &std::path::Path) -> bool {
    file_view(left, right) == bc_formats::route::FileView::Table
}
/// 打开后会是 Hex 比较（不是表格，且 `*.bin` 掩码或内容嗅探为二进制）。
pub fn hex_paths(left: &std::path::Path, right: &std::path::Path) -> bool {
    file_view(left, right) == bc_formats::route::FileView::Hex
}
pub(crate) fn routed(left: PathBuf, right: PathBuf) -> HomeAction {
    use bc_formats::route::FileView;
    if left.is_dir() && right.is_dir() {
        return HomeAction::OpenFolderCompare { left, right };
    }
    match file_view(&left, &right) {
        FileView::Table => HomeAction::OpenTableCompare { left, right },
        FileView::Hex => HomeAction::OpenHexCompare { left, right },
        FileView::Text => HomeAction::OpenTextCompare { left, right },
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
    /// Auto 按预填路径定型（文件夹 / 表格 / 十六进制 / 文本）；无路径时取图标网格首项。
    fn resolve(self, path: Option<&std::path::Path>) -> Self {
        use bc_formats::route::FileView;
        match (self, path) {
            (Self::Auto, Some(p)) if p.is_dir() => Self::FolderCompare,
            (Self::Auto, Some(p)) => match file_view(p, p) {
                FileView::Table => Self::TableCompare,
                FileView::Hex => Self::HexCompare,
                FileView::Text => Self::TextCompare,
            },
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
/// Session-type cards per page: as many as fit the canvas (all five on a
/// normal window); narrower windows page with round chevron buttons.
fn per_page(canvas_width: f32) -> usize {
    (((canvas_width - 140.0) / CELL.x).floor() as usize).clamp(1, KINDS.len())
}
// Reference geometry (client pixels, 1384x961 window).
const TREE_WIDTH: f32 = 275.0;
/// Sessions tree width (pixels), persisted.
const HOME_TREE: crate::chrome::splitter::Split =
    crate::chrome::splitter::Split::pixels("home.tree.w", TREE_WIDTH).min(160.0, 320.0);
const HEADER_BOTTOM: f32 = 60.0;
const TREE_ROW: f32 = 20.0;
/// Session toolbar height below the tree's bottom rule (y785..828).
const TOOLBAR_HEIGHT: f32 = 39.0;
/// White rule plus canvas-colored rows between the body and the panel.
const BODY_GAP: f32 = 4.0;
const PANEL_HEIGHT: f32 = 132.0;
const CELL: Vec2 = Vec2::new(158.0, 100.0);
const TREE_BG: Color32 = theme::SURFACE_CHROME;
const CANVAS_BG: Color32 = theme::SURFACE_SUBTLE;
const SELECT_FILL: Color32 = theme::ACCENT_SOFT;
const EXPANDER: Color32 = theme::TEXT_FAINT;
const CHEVRON: Color32 = theme::TEXT_MUTED;
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
        // canonicalize 在 Windows 返回 `\\?\` verbatim 路径，先还原成常规写法，
        // 否则地址栏与 recent-text.txt 都会带上 `\\?\` 前缀。
        let left = simplified(std::fs::canonicalize(left).unwrap_or_else(|_| left.to_path_buf()));
        let right = simplified(std::fs::canonicalize(right).unwrap_or_else(|_| right.to_path_buf()));
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
        painter.rect_filled(
            span(0.0, theme::MENU_HEIGHT - 1.0, w, theme::MENU_HEIGHT),
            0.0,
            theme::LINE,
        );
        let body_bottom = if self.hide_panel { h } else { h - PANEL_HEIGHT };
        let content_bottom = body_bottom - BODY_GAP;
        // 会话树宽度可拖动（右边缘，持久化，双击复位）。
        let tree_w = HOME_TREE.first(w);
        let canvas_left = if self.hide_tree {
            0.0
        } else {
            self.tree_panel(ui, &painter, o, tree_w, content_bottom, &mut action);
            tree_w + 1.0
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
        if !self.hide_tree {
            crate::chrome::splitter::vertical_line(
                ui,
                &HOME_TREE,
                "home-tree",
                full.left() + tree_w + 0.5,
                egui::Rangef::new(full.top() + theme::MENU_HEIGHT, full.top() + content_bottom),
                full.x_range(),
            );
        }
        if self.about {
            use crate::chrome::{
                controls::Role,
                modal::{Modal, Tone},
            };
            if Modal::new(
                "about",
                Tone::Info,
                format!("Matcha {}", env!("CARGO_PKG_VERSION")),
            )
            .message("Compare folders, text, tables and binary files side by side.")
            .action("Close", Role::Primary)
            .cancel(0)
            .width(380.0)
            .show(ui.ctx())
            .is_some()
            {
                self.about = false;
            }
        }
        action
    }
    fn tree_panel(
        &mut self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        o: Vec2,
        tree_w: f32,
        content_bottom: f32,
        action: &mut Option<HomeAction>,
    ) {
        let at = |x: f32, y: f32| Pos2::new(x, y) + o;
        let span = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(at(x0, y0), at(x1, y1));
        let font = FontId::proportional(theme::UI_SIZE);
        // Flat sidebar: small-caps section label, no gradient or rules.
        let toolbar_top = content_bottom - TOOLBAR_HEIGHT - 1.0;
        painter.rect_filled(
            span(0.0, theme::MENU_HEIGHT, tree_w, content_bottom),
            0.0,
            TREE_BG,
        );
        painter.text(
            at(16.0, 46.0),
            egui::Align2::LEFT_CENTER,
            "SESSIONS",
            theme::strong(ui.ctx(), 11.0),
            theme::TEXT_FAINT,
        );
        painter.rect_filled(
            span(12.0, toolbar_top, tree_w - 12.0, toolbar_top + 1.0),
            0.0,
            theme::LINE,
        );
        painter.rect_filled(
            span(tree_w, theme::MENU_HEIGHT, tree_w + 1.0, content_bottom),
            0.0,
            theme::LINE,
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
        let clip = painter.with_clip_rect(span(0.0, HEADER_BOTTOM + 1.0, tree_w, toolbar_top));
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
                span(0.0, top, tree_w, top + TREE_ROW),
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
                clip.add(egui::Shape::line(points, Stroke::new(1.3, EXPANDER)));
            }
            let ink = if self.selected == node {
                theme::ACCENT_PRESSED
            } else if group {
                theme::TEXT
            } else {
                theme::TEXT_MUTED
            };
            let galley = ui.painter().layout_no_wrap(label, font.clone(), ink);
            let text_left = 43.0 + indent;
            let row = span(6.0, top + 1.0, tree_w - 6.0, top + TREE_ROW - 1.0);
            if self.selected == node {
                clip.rect_filled(row, theme::RADIUS_MD, SELECT_FILL);
            } else if response.hovered() {
                clip.rect_filled(row, theme::RADIUS_MD, theme::HOVER);
            }
            glyph(node, indent, cy);
            clip.galley(at(text_left, cy - galley.size().y / 2.0), galley, ink);
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
        let hover = ui.ctx().pointer_hover_pos();
        for (rect, enabled, cross) in [(plus, true, true), (minus, removable, false)] {
            let ink = if enabled {
                theme::TEXT_MUTED
            } else {
                theme::TEXT_FAINT
            };
            if enabled && hover.is_some_and(|p| rect.contains(p)) {
                painter.rect_filled(rect, theme::RADIUS_MD, theme::HOVER);
            }
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
        let search = span(73.0, bt + 1.0, 266.0, bt + 27.0);
        painter.rect_filled(search, theme::RADIUS_MD, theme::SURFACE);
        painter.rect_stroke(
            search,
            theme::RADIUS_MD,
            Stroke::new(1.0, theme::LINE_STRONG),
            StrokeKind::Inside,
        );
        // Magnifier: ring with a handle toward the lower right.
        let ring = search.left_top() + Vec2::new(12.0, 12.0);
        let ink = theme::TEXT_FAINT;
        painter.circle_stroke(ring, 4.0, Stroke::new(1.4, ink));
        painter.line_segment(
            [ring + Vec2::new(3.0, 3.0), ring + Vec2::new(6.0, 6.0)],
            Stroke::new(1.6, ink),
        );
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(span(97.0, bt + 5.0, 262.0, bt + 23.0)),
            |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.filter)
                        .frame(egui::Frame::NONE)
                        .hint_text("Filter sessions")
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
        painter.text(
            Pos2::new(cx, icon_top - 74.0),
            egui::Align2::CENTER_CENTER,
            "Start a comparison",
            theme::strong(ui.ctx(), 22.0),
            theme::TEXT,
        );
        painter.text(
            Pos2::new(cx, icon_top - 44.0),
            egui::Align2::CENTER_CENTER,
            "Drop two files or folders anywhere in this window, or pick a session type.",
            font.clone(),
            theme::TEXT_MUTED,
        );
        let page_size = per_page(rect.width());
        let pages = KINDS.len().div_ceil(page_size);
        self.page = self.page.min(pages - 1);
        let shown = &KINDS[self.page * page_size..((self.page + 1) * page_size).min(KINDS.len())];
        for (i, kind) in shown.iter().enumerate() {
            let center_x =
                (grid_cx + (i as f32 - (shown.len() as f32 - 1.0) / 2.0) * CELL.x).round();
            let cell = Rect::from_min_max(
                Pos2::new(center_x - 68.0, icon_top - 18.0),
                Pos2::new(center_x + 68.0, icon_top + 100.0),
            );
            let response = ui
                .interact(cell, egui::Id::unique(("home-kind", i)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            let hovered = response.hovered();
            if hovered {
                painter.add(theme::shadow_popup().as_shape(cell, theme::RADIUS_XL));
            }
            painter.rect_filled(cell, theme::RADIUS_XL, theme::SURFACE);
            painter.rect_stroke(
                cell,
                theme::RADIUS_XL,
                Stroke::new(
                    1.0,
                    if hovered {
                        theme::ACCENT_SOFT_LINE
                    } else {
                        theme::LINE
                    },
                ),
                StrokeKind::Inside,
            );
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
                if hovered {
                    theme::ACCENT_PRESSED
                } else {
                    theme::TEXT
                },
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
            let c = Pos2::new(
                grid_cx + (page_size as f32 / 2.0 * CELL.x + 40.0) * dir,
                icon_top + 31.5,
            );
            let points = vec![
                c + Vec2::new(-8.0 * dir, -15.5),
                c + Vec2::new(8.0 * dir, 0.0),
                c + Vec2::new(-8.0 * dir, 15.5),
            ];
            let hit = Rect::from_center_size(c, Vec2::splat(40.0));
            let response = ui
                .interact(hit, egui::Id::unique(("home-page", right)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            painter.circle_filled(
                c,
                18.0,
                if response.hovered() {
                    theme::HOVER
                } else {
                    theme::SURFACE
                },
            );
            painter.circle_stroke(c, 18.0, Stroke::new(1.0, theme::LINE));
            let chevron: Vec<Pos2> = points
                .iter()
                .map(|p| c + (*p - c) * 0.33 + Vec2::new(dir, 0.0))
                .collect();
            painter.add(egui::Shape::line(chevron, Stroke::new(1.8, CHEVRON)));
            if response.clicked() {
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
                theme::DANGER,
            );
        }
    }
    /// Help panel: the reference's three-column strip with a close button,
    /// carrying Matcha's own neutral content (no brand color, no logo).
    fn help_panel(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, rect: Rect) {
        painter.rect_filled(rect, 0.0, theme::SURFACE);
        painter.rect_filled(
            Rect::from_min_size(rect.min, Vec2::new(rect.width(), 1.0)),
            0.0,
            theme::LINE,
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
                theme::strong(ui.ctx(), 11.0),
                theme::ACCENT,
            );
            for (i, line) in lines.iter().enumerate() {
                painter.text(
                    rect.min + Vec2::new(x, 53.5 + 17.0 * i as f32),
                    egui::Align2::LEFT_CENTER,
                    *line,
                    font.clone(),
                    theme::TEXT_MUTED,
                );
            }
        }
        for x in [264.0, 516.0] {
            painter.rect_filled(
                Rect::from_min_size(
                    rect.min + Vec2::new(x, 24.0),
                    Vec2::new(1.0, rect.height() - 48.0),
                ),
                0.0,
                theme::LINE,
            );
        }
        // Close (×) hides the panel, like View > Show Help Panel.
        let c = Pos2::new(rect.right() - 16.5, rect.top() + 13.5);
        let ink = Stroke::new(1.4, theme::TEXT_FAINT);
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
/// Windows 下 `std::fs::canonicalize` 返回 `\\?\` verbatim 路径；还原为常规写法
/// （`\\?\C:\…` → `C:\…`，`\\?\UNC\host\share\…` → `\\host\share\…`）。
/// 其他前缀形式（如 `\\?\Volume{…}`）与非 Windows 路径原样返回。
fn simplified(path: PathBuf) -> PathBuf {
    use std::path::{Component, Prefix};
    let mut components = path.components();
    let Some(Component::Prefix(prefix)) = components.next() else {
        return path;
    };
    let head = match prefix.kind() {
        Prefix::VerbatimDisk(drive) => format!(r"{}:\", drive as char),
        Prefix::VerbatimUNC(server, share) => {
            format!(r"\\{}\{}\", server.to_string_lossy(), share.to_string_lossy())
        }
        _ => return path,
    };
    let mut out = PathBuf::from(head);
    // 余下组件以根目录开头，剥掉后接到盘符/UNC 头后面。
    if let Ok(rest) = components.as_path().strip_prefix("\\") {
        out.push(rest);
    }
    out
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
                    // 旧版本可能存过带 `\\?\` 前缀的路径，读入时一并清理。
                    left: simplified(left.into()),
                    right: simplified(right.into()),
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
            left: simplified(p[0].into()),
            right: simplified(p[1].into()),
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
        // A normal window shows every session type on one row (y403).
        assert_eq!(per_page(1108.0), KINDS.len());
        for (x, kind) in [
            (518.0, SessionKind::FolderCompare),
            (676.0, SessionKind::FolderSync),
            (834.0, SessionKind::TextCompare),
            (992.0, SessionKind::TableCompare),
            (1150.0, SessionKind::HexCompare),
        ] {
            assert_eq!(click(&ctx, &mut home, Pos2::new(x, 403.0)), empty(kind));
        }
        assert_eq!(home.page, 0);
        // Narrow canvases page instead.
        assert_eq!(per_page(500.0), 2);
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
    fn binary_content_routes_to_hex_without_the_bin_extension() {
        let dir = std::env::temp_dir().join(format!("matcha-route-bin-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // exe/dll 等二进制内容（含 NUL），即使扩展名不在掩码内也路由到 Hex。
        let (a, b) = (dir.join("a.exe"), dir.join("b.exe"));
        std::fs::write(&a, b"MZ\x90\x00\x03\x00\x00\x00").unwrap();
        std::fs::write(&b, b"MZ\x90\x00\x04\x00\x00\x00").unwrap();
        assert!(matches!(
            routed(a.clone(), b.clone()),
            HomeAction::OpenHexCompare { .. }
        ));
        // 无扩展名的二进制同样被内容嗅探捕获。
        let bare = dir.join("firmware");
        std::fs::write(&bare, b"\x7fELF\x00\x00\x00\x00").unwrap();
        assert!(hex_paths(&bare, &bare));
        // 纯文本内容的未知扩展仍走 Text；读不到的路径不算二进制。
        let (c, d) = (dir.join("c.data"), dir.join("d.data"));
        std::fs::write(&c, b"hello\n").unwrap();
        std::fs::write(&d, b"world\n").unwrap();
        assert!(matches!(routed(c, d), HomeAction::OpenTextCompare { .. }));
        assert!(!hex_paths(
            &dir.join("missing.exe"),
            &dir.join("missing.exe")
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn workbooks_and_text_tables_route_to_table_before_the_binary_sniff() {
        let dir = std::env::temp_dir().join(format!("matcha-route-table-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // xlsx 是 ZIP：内容嗅探为二进制，但必须先判为表格。
        let book = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/reference-parity/table-report/corpus/key-none-rows-changed/"
        );
        let (l, r) = (
            PathBuf::from(format!("{book}left.xlsx")),
            PathBuf::from(format!("{book}right.xlsx")),
        );
        assert!(bc_formats::route::is_binary(&l));
        assert!(matches!(
            routed(l.clone(), r.clone()),
            HomeAction::OpenTableCompare { .. }
        ));
        assert!(!hex_paths(&l, &r));
        assert_eq!(
            SessionKind::Auto.resolve(Some(&l)),
            SessionKind::TableCompare
        );
        // 版本控制临时名：扩展名对不上，按包内容识别为工作簿。
        let temp = dir.join("left.xlsx.r42");
        std::fs::copy(&l, &temp).unwrap();
        assert!(matches!(
            routed(temp.clone(), r.clone()),
            HomeAction::OpenTableCompare { .. }
        ));
        for name in ["a.csv", "b.TSV", "c.tab"] {
            let path = dir.join(name);
            std::fs::write(
                &path,
                b"a,b
1,2
",
            )
            .unwrap();
            assert!(
                matches!(
                    routed(path.clone(), path),
                    HomeAction::OpenTableCompare { .. }
                ),
                "{name}"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
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
    #[cfg(windows)]
    #[test]
    fn verbatim_paths_are_simplified_for_display_and_old_files() {
        assert_eq!(
            simplified(PathBuf::from(r"\\?\C:\Users\admin\a.txt")),
            PathBuf::from(r"C:\Users\admin\a.txt")
        );
        assert_eq!(
            simplified(PathBuf::from(r"\\?\D:\")),
            PathBuf::from(r"D:\")
        );
        assert_eq!(
            simplified(PathBuf::from(r"\\?\UNC\host\share\b.txt")),
            PathBuf::from(r"\\host\share\b.txt")
        );
        // 常规路径原样保留。
        assert_eq!(
            simplified(PathBuf::from(r"C:\plain\c.txt")),
            PathBuf::from(r"C:\plain\c.txt")
        );
        // 存量 recent 文件里带 `\\?\` 前缀的记录，读入时一并清理。
        let now = SystemTime::now();
        let items = parse_recent(
            "# matcha recent v2\n1700000000\t\\\\?\\C:\\Temp\\a.txt\t\\\\?\\C:\\Temp\\b.txt\n",
            now,
        );
        assert_eq!(
            items,
            vec![Recent {
                left: PathBuf::from(r"C:\Temp\a.txt"),
                right: PathBuf::from(r"C:\Temp\b.txt"),
                when: std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000),
            }]
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
