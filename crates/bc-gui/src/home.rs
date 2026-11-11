//! Home：参考布局的会话管理页——菜单栏、左侧会话树（New / Auto-saved，
//! Auto-saved 下为最近比较）、中央会话类型图标网格、底部帮助面板；拖放两个
//! 文件/文件夹直接打开，点击会话类型弹出路径输入。
use crate::chrome::{
    icons::{self, SessionIcon},
    menubar::{self, MenuItem, MenuItemKind, MenuSpec},
    theme,
};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use std::path::PathBuf;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomeAction {
    OpenTextCompare { left: PathBuf, right: PathBuf },
    OpenTableCompare { left: PathBuf, right: PathBuf },
    OpenFolderCompare { left: PathBuf, right: PathBuf },
    OpenFolderSync { left: PathBuf, right: PathBuf },
    Exit,
}
pub fn table_paths(left: &std::path::Path, right: &std::path::Path) -> bool {
    let registry = crate::table::formats::load_registry()
        .unwrap_or_else(|_| bc_formats::registry::FormatRegistry::with_builtins());
    [left, right]
        .iter()
        .any(|path| crate::table::formats::table_path(path, &registry))
}
fn routed(left: PathBuf, right: PathBuf) -> HomeAction {
    if left.is_dir() && right.is_dir() {
        HomeAction::OpenFolderCompare { left, right }
    } else if table_paths(&left, &right) {
        HomeAction::OpenTableCompare { left, right }
    } else {
        HomeAction::OpenTextCompare { left, right }
    }
}
/// Session types offered on Home, in icon-grid order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    /// Routed by the paths (folders / table formats / text).
    Auto,
    FolderCompare,
    FolderSync,
    TextCompare,
    TableCompare,
}
const KINDS: [SessionKind; 4] = [
    SessionKind::FolderCompare,
    SessionKind::FolderSync,
    SessionKind::TextCompare,
    SessionKind::TableCompare,
];
impl SessionKind {
    fn label(self) -> &'static str {
        match self {
            Self::Auto => "Compare",
            Self::FolderCompare => "Folder Compare",
            Self::FolderSync => "Folder Sync",
            Self::TextCompare => "Text Compare",
            Self::TableCompare => "Table Compare",
        }
    }
    fn icon(self) -> SessionIcon {
        match self {
            Self::FolderCompare | Self::Auto => SessionIcon::FolderCompare,
            Self::FolderSync => SessionIcon::FolderSync,
            Self::TextCompare => SessionIcon::TextCompare,
            Self::TableCompare => SessionIcon::TableCompare,
        }
    }
    fn action(self, left: PathBuf, right: PathBuf) -> HomeAction {
        match self {
            Self::Auto => routed(left, right),
            Self::FolderCompare => HomeAction::OpenFolderCompare { left, right },
            Self::FolderSync => HomeAction::OpenFolderSync { left, right },
            Self::TextCompare => HomeAction::OpenTextCompare { left, right },
            Self::TableCompare => HomeAction::OpenTableCompare { left, right },
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
    Recent(usize),
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

#[derive(Debug, Default)]
pub struct HomeView {
    left: String,
    right: String,
    error: Option<String>,
    recent: Vec<(PathBuf, PathBuf)>,
    selected: Node,
    new_open: bool,
    auto_open: bool,
    filter: String,
    page: usize,
    hide_tree: bool,
    hide_panel: bool,
    /// Open "new session" path prompt.
    prompt: Option<SessionKind>,
    about: bool,
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
    pub fn new(left: Option<&PathBuf>, right: Option<&PathBuf>) -> Self {
        Self {
            left: left.map(|p| p.display().to_string()).unwrap_or_default(),
            right: right.map(|p| p.display().to_string()).unwrap_or_default(),
            recent: load_recent(),
            ..Default::default()
        }
    }
    pub fn set_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
    }
    pub fn record(left: &std::path::Path, right: &std::path::Path) {
        let left = std::fs::canonicalize(left).unwrap_or_else(|_| left.to_path_buf());
        let right = std::fs::canonicalize(right).unwrap_or_else(|_| right.to_path_buf());
        let mut recent = load_recent();
        recent.retain(|p| p.0 != left || p.1 != right);
        recent.insert(0, (left, right));
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
                            item("Version Compare", "", false),
                        ],
                    ),
                    short(item("New Tab", "", false), "Ctrl+T"),
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
                    short(item("Close Tab", "", false), "Ctrl+W"),
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
                    item("Options...", "", false),
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
            "new:folder" => self.prompt = Some(SessionKind::FolderCompare),
            "new:sync" => self.prompt = Some(SessionKind::FolderSync),
            "new:text" => self.prompt = Some(SessionKind::TextCompare),
            "new:table" => self.prompt = Some(SessionKind::TableCompare),
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
            if self.left.is_empty() {
                self.left = path.display().to_string();
            } else {
                self.right = path.display().to_string();
            }
            self.prompt.get_or_insert(SessionKind::Auto);
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
        self.canvas(ui, &painter, span(canvas_left, 29.0, w, content_bottom));
        if !self.hide_panel {
            self.help_panel(ui, &painter, span(0.0, body_bottom, w, h));
        }
        if let Some(kind) = self.prompt {
            action = self.prompt_window(ui.ctx(), kind).or(action);
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
            for (i, (l, r)) in self.recent.iter().enumerate() {
                let full = format!("{} {}", l.display(), r.display()).to_lowercase();
                if !filter.is_empty() && !full.contains(&filter) {
                    continue;
                }
                let name = |p: &PathBuf| {
                    p.file_name()
                        .map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into())
                };
                rows.push((Node::Recent(i), 1, format!("{} ↔ {}", name(l), name(r))));
            }
        }
        let clip = painter.with_clip_rect(span(0.0, HEADER_BOTTOM + 1.0, TREE_WIDTH, toolbar_top));
        let glyph = |node: Node, indent: f32, cy: f32| match node {
            Node::New | Node::AutoSaved => {
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
            let group = matches!(node, Node::New | Node::AutoSaved);
            let indent = depth as f32 * 19.0;
            let cy = top + TREE_ROW / 2.0;
            let response = ui.interact(
                span(0.0, top, TREE_WIDTH, top + TREE_ROW),
                egui::Id::unique(("home-tree", index)),
                Sense::click(),
            );
            if group {
                // Expander chevron ">" (open: "v") and the dotted stub.
                let open = if node == Node::New {
                    self.new_open
                } else {
                    auto_open
                };
                let c = at(10.5, cy);
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
                for x in [15.0, 17.0, 19.0] {
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
                    .is_some_and(|p| p.x < at(20.0, 0.0).x);
                if group && on_expander {
                    self.toggle(node);
                }
            }
            if response.double_clicked() {
                match node {
                    Node::New | Node::AutoSaved => self.toggle(node),
                    Node::Kind(i) => self.prompt = Some(KINDS[i]),
                    Node::Recent(i) => {
                        let (l, r) = self.recent[i].clone();
                        *action = Some(routed(l, r));
                    }
                }
            }
        }
        self.session_toolbar(ui, painter, o, toolbar_top);
    }
    fn toggle(&mut self, node: Node) {
        match node {
            Node::New => self.new_open = !self.new_open,
            Node::AutoSaved => self.auto_open = !self.auto_open,
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
            self.prompt = Some(match self.selected {
                Node::Kind(i) => KINDS[i],
                _ => SessionKind::Auto,
            });
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
    fn canvas(&mut self, ui: &mut egui::Ui, painter: &egui::Painter, rect: Rect) {
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
                self.prompt = Some(*kind);
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
    fn prompt_window(&mut self, ctx: &egui::Context, kind: SessionKind) -> Option<HomeAction> {
        let mut action = None;
        let mut open = true;
        let mut close = false;
        egui::Window::new(format!("New {} session", kind.label()))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                egui::Grid::new("home_paths").num_columns(2).show(ui, |ui| {
                    ui.label("Left");
                    ui.add(egui::TextEdit::singleline(&mut self.left).desired_width(520.0));
                    ui.end_row();
                    ui.label("Right");
                    ui.add(egui::TextEdit::singleline(&mut self.right).desired_width(520.0));
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    if ui.button("Compare").clicked() {
                        if self.left.trim().is_empty() || self.right.trim().is_empty() {
                            self.error = Some("Both paths are required".into());
                        } else {
                            action = Some(kind.action(
                                PathBuf::from(self.left.trim()),
                                PathBuf::from(self.right.trim()),
                            ));
                            close = true;
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
                if let Some(error) = &self.error {
                    ui.colored_label(Color32::RED, error);
                }
            });
        if !open || close {
            self.prompt = None;
        }
        action
    }
}
fn save_recent(recent: &[(PathBuf, PathBuf)]) {
    if let Some(file) = recent_path() {
        if let Some(parent) = file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let text = recent
            .iter()
            .map(|(l, r)| format!("{}\n{}\n", l.display(), r.display()))
            .collect::<String>();
        let _ = std::fs::write(file, text);
    }
}
fn recent_path() -> Option<PathBuf> {
    bc_formats::registry::user_formats_dir()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("recent-text.txt")))
}
fn load_recent() -> Vec<(PathBuf, PathBuf)> {
    let Some(text) = recent_path().and_then(|p| std::fs::read_to_string(p).ok()) else {
        return Vec::new();
    };
    let lines: Vec<_> = text.lines().collect();
    lines
        .as_chunks::<2>()
        .0
        .iter()
        .take(12)
        .map(|p| (PathBuf::from(p[0]), PathBuf::from(p[1])))
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
    fn icon_grid_pages_and_opens_the_typed_prompt() {
        let ctx = egui::Context::default();
        let mut home = HomeView::default();
        frame(&ctx, &mut home, vec![]);
        // Third cell of page one is Text Compare (reference x992, y403).
        click(&ctx, &mut home, Pos2::new(992.0, 403.0));
        assert_eq!(home.prompt, Some(SessionKind::TextCompare));
        home.prompt = None;
        // Four types: the chevron pages to Table Compare and back.
        click(&ctx, &mut home, Pos2::new(1295.0, 403.0));
        assert_eq!(home.page, 1);
        click(&ctx, &mut home, Pos2::new(676.0, 403.0));
        assert_eq!(home.prompt, Some(SessionKind::TableCompare));
        home.prompt = None;
        click(&ctx, &mut home, Pos2::new(1668.0 - 1295.0, 403.0));
        assert_eq!(home.page, 0);
    }
    #[test]
    fn session_kinds_menus_and_tree_keep_home_actions() {
        let (l, r) = (PathBuf::from("a"), PathBuf::from("b"));
        assert_eq!(
            SessionKind::FolderSync.action(l.clone(), r.clone()),
            HomeAction::OpenFolderSync {
                left: l.clone(),
                right: r.clone()
            }
        );
        assert_eq!(
            SessionKind::TableCompare.action(l.clone(), r.clone()),
            HomeAction::OpenTableCompare {
                left: l.clone(),
                right: r.clone()
            }
        );
        let mut home = HomeView::default();
        let menus = home.menus();
        assert_eq!(
            menus.iter().map(|m| m.title.as_str()).collect::<Vec<_>>(),
            ["Session", "View", "Tools", "Help"]
        );
        assert_eq!(menus[3].items.last().unwrap().label, "About Matcha");
        assert_eq!(home.command("new:sync"), None);
        assert_eq!(home.prompt, Some(SessionKind::FolderSync));
        assert_eq!(home.command("exit"), Some(HomeAction::Exit));
        home.command("view:panel");
        assert!(home.hide_panel);
        // Tree: Auto-saved children are the recent pairs; double-click opens.
        home.recent = vec![(l.clone(), r.clone())];
        let ctx = egui::Context::default();
        frame(&ctx, &mut home, vec![]);
        // Expander of "Auto-saved" (second row, y81..100).
        click(&ctx, &mut home, Pos2::new(10.0, 90.0));
        assert!(home.auto_open);
        click(&ctx, &mut home, Pos2::new(100.0, 110.0));
        assert_eq!(home.selected, Node::Recent(0));
        let pos = Pos2::new(100.0, 110.0);
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
}
