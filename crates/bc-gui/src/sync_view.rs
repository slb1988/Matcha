//! Folder sync view: classic chrome (menu bar from `inventories.sync`, large
//! toolbar, 40 px paths area, 191 px operations area) over the shared folder
//! tree / log / status renderers of [`DirCompareView`] with the 117 px sync
//! arrow column. The sync plan, overrides and Sync Now reuse the folder
//! worker; no new sync logic lives here.
use crate::{
    chrome::{
        icons::{self, Icon},
        menubar::{self, MenuItem, MenuItemKind, MenuSpec},
        theme,
        toolbar::{self, ToolItem, ToolItemKind},
    },
    dir_view::{choice_label, Command, DirCompareView, FolderNavigation, SyncSummary},
    egui,
};
use bc_engine_dir::sync::{CustomSyncRules, SyncChoice, SyncDirection, SyncMode};
use std::path::Path;

/// Reference geometry (tokens `folder-sync` regions).
const PATHS_HEIGHT: f32 = 40.0;
const OPERATIONS_HEIGHT: f32 = 191.0;
const LOG_HEIGHT: f32 = 106.0;
const COMBO_EDGE: egui::Color32 = egui::Color32::from_rgb(141, 141, 141);
const BUTTON_EDGE: egui::Color32 = egui::Color32::from_rgb(208, 208, 208);
const BUTTON_FILL: egui::Color32 = egui::Color32::from_rgb(253, 253, 253);
const BUTTON_SHADOW: egui::Color32 = egui::Color32::from_rgb(186, 186, 186);
const DISABLED_EDGE: egui::Color32 = egui::Color32::from_rgb(233, 233, 233);
const DISABLED_FILL: egui::Color32 = egui::Color32::from_rgb(249, 249, 249);
const DISABLED_TEXT: egui::Color32 = egui::Color32::from_rgb(160, 160, 160);

const METHODS: [&str; 6] = [
    "Update Left",
    "Update Right",
    "Update Both",
    "Mirror to Left",
    "Mirror to Right",
    "Custom Sync",
];
const DESCRIPTIONS: [&str; 6] = [
    "Copy newer and orphan files/folders to left side.",
    "Copy newer and orphan files/folders to right side.",
    "Copy newer and orphan files/folders to other side.",
    "Make left side match right side: copy different files, delete left orphans.",
    "Make right side match left side: copy different files, delete right orphans.",
    "Apply the custom rules below to newer, orphan and different files.",
];
/// Entries of existing features without a reference menu item, appended
/// after a separator: (menu index, label, action id; "" opens a submenu
/// whose children follow as (label, id) pairs).
fn extras() -> Vec<(usize, MenuItem)> {
    let leaf = |label: &str, id: &str| MenuItem::action(label, id);
    let sub = |label: &str, children: &[(&str, &str)]| MenuItem {
        label: label.into(),
        mnemonic: None,
        shortcut: None,
        kind: MenuItemKind::Submenu(children.iter().map(|(l, i)| leaf(l, i)).collect()),
        enabled: true,
        action_id: label.into(),
    };
    vec![
        (0, leaf("Back", "Back")),
        (0, leaf("Forward", "Forward")),
        (0, leaf("Up One Level", "up-both")),
        (0, leaf("Create Empty Folders", "Create Empty Folders")),
        (0, leaf("Visible Items Only", "Visible Items Only")),
        (1, leaf("Open", "Open Pair")),
        (1, leaf("Open Folder", "Open Folder")),
        (1, leaf("Compare To...", "Compare To")),
        (1, leaf("Align With...", "Align With")),
        (
            1,
            sub(
                "Quick Compare",
                &[
                    ("Size", "quick-size"),
                    ("CRC", "quick-crc"),
                    ("Binary", "quick-binary"),
                    ("Rules-based", "quick-rules"),
                ],
            ),
        ),
        (
            1,
            sub(
                "Compare Contents",
                &[
                    ("CRC", "compare-crc"),
                    ("Binary", "compare-binary"),
                    ("Rules-based", "compare-rules"),
                ],
            ),
        ),
        (1, leaf("Copy to Side...", "Copy to Side")),
        (1, leaf("Move to Side...", "Move to Side")),
        (1, leaf("Copy to Folder...", "Copy to Folder")),
        (1, leaf("Move to Folder...", "Move to Folder")),
        (1, leaf("Exchange...", "Exchange")),
        (1, leaf("Delete...", "Delete")),
        (1, leaf("Rename...", "Rename")),
        (1, leaf("Attributes...", "Attributes")),
        (1, leaf("Touch...", "Touch")),
        (1, leaf("New Folder...", "New Folder")),
        (1, leaf("Ignored", "Ignored")),
        (1, leaf("Set as Base Folder", "Set as Base Folder")),
        (
            1,
            leaf("Set as Base on Other Side", "Set as Base on Other Side"),
        ),
        (2, leaf("Select All Files", "Select All Files")),
        (
            2,
            sub(
                "Select Newer",
                &[
                    ("Left Side", "select-newer-left"),
                    ("Right Side", "select-newer-right"),
                    ("Both Sides", "select-newer-both"),
                ],
            ),
        ),
        (
            2,
            sub(
                "Select Orphans",
                &[
                    ("Left Side", "select-orphan-left"),
                    ("Right Side", "select-orphan-right"),
                    ("Both Sides", "select-orphan-both"),
                ],
            ),
        ),
        (3, leaf("Next Difference", "Next Difference")),
        (3, leaf("Previous Difference", "Previous Difference")),
        (4, leaf("Show All", "Show All")),
        (4, leaf("Show Differences", "Show Differences")),
    ]
}
/// Inventory items wired to existing actions; everything else stays disabled.
const ENABLED: &[&str] = &[
    "Session Settings...",
    "New Tab",
    "Close Tab",
    "Swap Sides",
    "Folder Sync Info",
    "Sync Now",
    "Compare Base Folders",
    "Compare Parent Folders",
    "Exit",
    "Leave Alone",
    "Copy Right to Left",
    "Copy Left to Right",
    "Delete Left",
    "Delete Right",
    "Close Folder",
    "Open Subfolders",
    "Close Subfolders",
    "Set as Base Folders",
    "Open in New View",
    "Exclude",
    "Copy Filename",
    "Refresh Selection",
    "Expand All",
    "Collapse All",
    "Select All",
    "Invert Selection",
    "Refresh",
    "Full Refresh",
    "Next Conflict",
    "Previous Conflict",
    "Find Filename...",
    "Find Next Filename",
    "Find Previous Filename",
    "Ignore Unimportant Differences",
    "Suppress Filters",
    "Size",
    "CRC",
    "Modified",
    "Attributes",
    "Version",
    "Legend",
    "Log",
    "Toolbar",
    "Create Empty Folders",
    "Visible Items Only",
    "Back",
    "Forward",
    "up-both",
    "Open Pair",
    "Open Folder",
    "Compare To",
    "Align With",
    "quick-size",
    "quick-crc",
    "quick-binary",
    "quick-rules",
    "compare-crc",
    "compare-binary",
    "compare-rules",
    "Copy to Side",
    "Move to Side",
    "Copy to Folder",
    "Move to Folder",
    "Exchange",
    "Delete",
    "Rename",
    "Attributes",
    "Touch",
    "New Folder",
    "Ignored",
    "Set as Base Folder",
    "Set as Base on Other Side",
    "Select All Files",
    "select-newer-left",
    "select-newer-right",
    "select-newer-both",
    "select-orphan-left",
    "select-orphan-right",
    "select-orphan-both",
    "Next Difference",
    "Previous Difference",
    "Show All",
    "Show Differences",
];

pub struct SyncView {
    pub folder: DirCompareView,
    method: usize,
    create_empty: bool,
    visible: bool,
    custom: [SyncChoice; 5],
    menus: Vec<MenuSpec>,
    path_edits: [String; 2],
    path_sources: [String; 2],
    filter_draft: String,
    filter_source: String,
    show_info: bool,
    sessions_popup: Option<egui::Pos2>,
}
impl SyncView {
    pub fn open(ctx: &egui::Context, left: &Path, right: &Path) -> Self {
        Self::with_folder(DirCompareView::open(ctx, left, right, true, false))
    }
    /// 空会话：路径区为空，两侧都填上后经 `open_paths` 加载。
    pub fn empty(ctx: &egui::Context) -> Self {
        Self::with_folder(DirCompareView::empty(ctx, true, false))
    }
    fn with_folder(folder: DirCompareView) -> Self {
        let view = Self {
            folder,
            // Reference default operation: Update Both.
            method: 2,
            create_empty: false,
            visible: false,
            custom: [SyncChoice::LeaveAlone; 5],
            menus: Vec::new(),
            path_edits: Default::default(),
            path_sources: Default::default(),
            filter_draft: String::new(),
            filter_source: String::new(),
            show_info: false,
            sessions_popup: None,
        };
        view.send_mode();
        view
    }
    /// Snapshot scene state: first row selected (reference default).
    pub fn snapshot_preset(&mut self) {
        self.folder.snapshot_preset("folder-sync");
    }
    fn mode(&self) -> SyncMode {
        match self.method {
            0 => SyncMode::Update(SyncDirection::RightToLeft),
            1 => SyncMode::Update(SyncDirection::LeftToRight),
            2 => SyncMode::Update(SyncDirection::Both),
            3 => SyncMode::Mirror(SyncDirection::RightToLeft),
            4 => SyncMode::Mirror(SyncDirection::LeftToRight),
            _ => SyncMode::Custom(CustomSyncRules {
                left_newer: self.custom[0],
                right_newer: self.custom[1],
                left_orphan: self.custom[2],
                right_orphan: self.custom[3],
                different: self.custom[4],
            }),
        }
    }
    fn send_mode(&self) {
        self.folder.send(Command::SyncMode(
            self.mode(),
            self.create_empty,
            self.visible,
        ));
    }
    pub fn title(&self) -> String {
        let name = |p: &str| {
            Path::new(p)
                .file_name()
                .map_or_else(|| p.to_string(), |n| n.to_string_lossy().into_owned())
        };
        let [left, right] = self.folder.path_texts();
        format!(
            "{} <--> {} - Folder Sync - Matcha",
            name(&left),
            name(&right)
        )
    }
    fn can_sync(&self) -> bool {
        !self.folder.busy()
            && self.folder.snapshot.complete
            && self.folder.snapshot.sync_stats.is_some()
    }
    /// Effective choice of the first selected item (Actions radio state).
    fn selected_choice(&self) -> Option<SyncChoice> {
        self.folder.selection.items.iter().next()?;
        Some(SyncChoice::LeaveAlone)
    }
    fn override_selected(&mut self, choice: SyncChoice) {
        if self.folder.busy() {
            return;
        }
        let nodes: Vec<_> = self
            .folder
            .selection
            .items
            .iter()
            .map(|&(n, _)| n)
            .collect();
        for node in nodes {
            self.folder.send(Command::SyncOverride(node, choice));
        }
    }
    fn checked(&self, id: &str) -> Option<bool> {
        Some(match id {
            "Create Empty Folders" => self.create_empty,
            "Visible Items Only" => self.visible,
            "Show All" => self.folder.display() == bc_engine_dir::filter::DisplayFilter::All,
            "Show Differences" => {
                self.folder.display() == bc_engine_dir::filter::DisplayFilter::Mismatches
            }
            "Size" => self.folder.view_checked("column-size")?,
            "Modified" => self.folder.view_checked("column-modified")?,
            "CRC" => self.folder.view_checked("column-crc")?,
            "Version" => self.folder.view_checked("column-version")?,
            "Attributes" => self.folder.view_checked("column-attrs")?,
            other => return self.folder.view_checked(other),
        })
    }
    fn update_menus(&mut self) {
        fn update(view: &SyncView, items: &mut [MenuItem]) {
            for item in items {
                if let MenuItemKind::Submenu(children) = &mut item.kind {
                    update(view, children);
                    item.enabled = children.iter().any(|c| c.enabled);
                    continue;
                }
                item.enabled = ENABLED.contains(&item.action_id.as_str());
                match item.action_id.as_str() {
                    "Sync Now" => item.enabled = view.can_sync(),
                    "Leave Alone" | "Copy Right to Left" | "Copy Left to Right" | "Delete Left"
                    | "Delete Right" | "Exclude" => {
                        item.enabled = view.selected_choice().is_some() && !view.folder.busy();
                    }
                    _ => {}
                }
                if let Some(checked) = view.checked(&item.action_id) {
                    item.kind = MenuItemKind::Check { checked };
                }
            }
        }
        if self.menus.is_empty() {
            let mut menus = inventory();
            let mut separated = [false; 7];
            for (index, item) in extras() {
                if !std::mem::replace(&mut separated[index], true) {
                    menus[index].items.push(sep());
                }
                menus[index].items.push(item);
            }
            self.menus = menus;
        }
        let mut menus = std::mem::take(&mut self.menus);
        for menu in &mut menus {
            update(self, &mut menu.items);
        }
        self.menus = menus;
    }
    /// Menu/toolbar command; unknown ids fall through to the shared folder
    /// commands (same action ids as the folder compare view).
    fn command(&mut self, id: &str) -> Option<FolderNavigation> {
        let choice = match id {
            "Leave Alone" => Some(SyncChoice::LeaveAlone),
            "Copy Right to Left" => Some(SyncChoice::CopyRightToLeft),
            "Copy Left to Right" => Some(SyncChoice::CopyLeftToRight),
            "Delete Left" => Some(SyncChoice::DeleteLeft),
            "Delete Right" => Some(SyncChoice::DeleteRight),
            "Exclude" => Some(SyncChoice::Exclude),
            _ => None,
        };
        if let Some(choice) = choice {
            self.override_selected(choice);
            return None;
        }
        match id {
            "Close Tab" | "New Tab" => self.folder.folder_command(id),
            "Session Settings..." | "Session Settings" => {
                self.folder.folder_command("Session Settings")
            }
            "Folder Sync Info" => {
                self.show_info = true;
                None
            }
            "Sync Now" => {
                if self.can_sync() {
                    self.folder.send(Command::SyncRun);
                }
                None
            }
            "Compare Base Folders" => Some((false, self.folder.paths())),
            "Next Conflict" => {
                self.folder.navigate(false, true);
                None
            }
            "Previous Conflict" => {
                self.folder.navigate(true, true);
                None
            }
            "Find Filename..." => self.folder.folder_command("Find Filename"),
            "Size" => self.folder.folder_command("column-size"),
            "Modified" => self.folder.folder_command("column-modified"),
            "CRC" => self.folder.folder_command("column-crc"),
            "Version" => self.folder.folder_command("column-version"),
            "Attributes" => self.folder.folder_command("column-attrs"),
            "Create Empty Folders" => {
                self.create_empty = !self.create_empty;
                self.send_mode();
                None
            }
            "Visible Items Only" => {
                self.visible = !self.visible;
                self.send_mode();
                None
            }
            other => self.folder.folder_command(other),
        }
    }
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<FolderNavigation> {
        let ctx = ui.ctx().clone();
        self.folder.poll(&ctx);
        let busy = self.folder.busy();
        let mut switch = None;
        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
        let full = ui.available_rect_before_wrap();
        let mut y = full.top();
        // Menu bar 27 px.
        self.update_menus();
        let menu_rect = egui::Rect::from_min_size(
            egui::pos2(full.left(), y),
            egui::vec2(full.width(), theme::MENU_HEIGHT),
        );
        let mut menu_action = None;
        ui.scope_builder(egui::UiBuilder::new().max_rect(menu_rect), |ui| {
            menu_action = menubar::show(ui, &self.menus);
        });
        y = menu_rect.bottom();
        if let Some(action) = menu_action {
            switch = self.command(&action);
        }
        // Toolbar 43 px.
        if self.folder.toolbar_visible() {
            let rect = egui::Rect::from_min_size(
                egui::pos2(full.left(), y),
                egui::vec2(full.width(), theme::TOOLBAR_HEIGHT),
            );
            let items = self.tool_items(busy);
            let mut response = toolbar::ToolbarResponse::default();
            ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                response = toolbar::show(ui, &items);
            });
            y = rect.bottom();
            if let Some(action) = response.clicked {
                switch = self.command(&action).or(switch);
            }
            if response.dropdown.is_some() {
                self.sessions_popup = ctx.pointer_latest_pos();
            }
        }
        switch = self.sessions_dropdown(ui).or(switch);
        let paths = egui::Rect::from_min_size(
            egui::pos2(full.left(), y),
            egui::vec2(full.width(), PATHS_HEIGHT),
        );
        y = paths.bottom();
        self.paths_area(ui, paths);
        let operations = egui::Rect::from_min_size(
            egui::pos2(full.left(), y),
            egui::vec2(full.width(), OPERATIONS_HEIGHT),
        );
        y = operations.bottom();
        self.operations_area(ui, operations, busy);
        let rest = egui::Rect::from_min_max(egui::pos2(full.left(), y), full.max);
        let log_h = if self.folder.log_visible() {
            LOG_HEIGHT
        } else {
            0.0
        };
        let grid_h = (rest.height() - log_h - theme::STATUS_HEIGHT).max(80.0);
        let grid = egui::Rect::from_min_size(rest.min, egui::vec2(rest.width(), grid_h));
        self.folder.draw_grid(ui, grid);
        if log_h > 0.0 {
            self.folder.draw_log(
                ui,
                egui::Rect::from_min_size(
                    egui::pos2(rest.left(), grid.bottom()),
                    egui::vec2(rest.width(), log_h),
                ),
            );
        }
        self.folder.draw_status(
            ui,
            egui::Rect::from_min_size(
                egui::pos2(rest.left(), grid.bottom() + log_h),
                egui::vec2(rest.width(), theme::STATUS_HEIGHT),
            ),
            busy,
        );
        ui.allocate_rect(full, egui::Sense::hover());
        self.info_window(&ctx);
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::I)) {
            self.show_info = true;
        }
        // Dialogs, keyboard, child windows of the shared folder view.
        self.folder.ui(ui).or(switch)
    }
    fn tool_items(&self, busy: bool) -> Vec<ToolItem> {
        let mk = |icon, label: &str, id: &str, kind, group: &str, enabled, selected| ToolItem {
            label: label.into(),
            action_id: id.into(),
            kind,
            enabled,
            selected,
            icon,
            group: Some(group.into()),
        };
        let checked = |id: &str| self.folder.view_checked(id).unwrap_or(false);
        vec![
            mk(
                Icon::Home,
                "Home",
                "Home",
                ToolItemKind::Normal,
                "0",
                true,
                false,
            ),
            mk(
                Icon::Sessions,
                "Sessions",
                "Sessions",
                ToolItemKind::Split,
                "0",
                true,
                false,
            ),
            mk(
                Icon::Minor,
                "Minor",
                "Ignore Unimportant Differences",
                ToolItemKind::Toggle,
                "1",
                true,
                checked("Ignore Unimportant Differences"),
            ),
            mk(
                Icon::Expand,
                "Expand",
                "Expand All",
                ToolItemKind::Normal,
                "2",
                true,
                false,
            ),
            mk(
                Icon::Collapse,
                "Collapse",
                "Collapse All",
                ToolItemKind::Normal,
                "2",
                true,
                false,
            ),
            mk(
                Icon::Select,
                "Select",
                "Select All",
                ToolItemKind::Normal,
                "2",
                true,
                false,
            ),
            mk(
                Icon::Refresh,
                "Refresh",
                "Refresh",
                ToolItemKind::Normal,
                "3",
                true,
                false,
            ),
            mk(
                Icon::Stop,
                "Stop",
                "Stop",
                ToolItemKind::Normal,
                "4",
                busy,
                false,
            ),
            mk(
                Icon::Peek,
                "Peek",
                "Suppress Filters",
                ToolItemKind::Toggle,
                "5",
                true,
                checked("Suppress Filters"),
            ),
        ]
    }
    fn sessions_dropdown(&mut self, ui: &mut egui::Ui) -> Option<FolderNavigation> {
        let pos = self.sessions_popup?;
        let mut picked = None;
        let area = egui::Area::new(egui::Id::unique("sync-sessions-dropdown"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    for (label, id) in [
                        ("Home", "Home"),
                        ("Session Settings...", "Session Settings..."),
                        ("Folder Sync Info", "Folder Sync Info"),
                    ] {
                        if ui.button(label).clicked() {
                            picked = Some(id);
                        }
                    }
                });
            });
        if let Some(id) = picked {
            self.sessions_popup = None;
            return self.command(id);
        }
        if ui.input(|i| {
            i.key_pressed(egui::Key::Escape)
                || (i.pointer.any_pressed()
                    && i.pointer
                        .interact_pos()
                        .is_some_and(|p| !area.response.rect.contains(p)))
        }) {
            self.sessions_popup = None;
        }
        None
    }
    /// Paths area (40 px): two editable folder boxes, "..." browse buttons
    /// and the swap button between them.
    fn paths_area(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let painter = ui.painter().clone();
        painter.hline(
            rect.left()..=rect.right(),
            rect.top(),
            egui::Stroke::new(1.0, egui::Color32::WHITE),
        );
        let sources = self.folder.path_texts();
        for (side, source) in sources.into_iter().enumerate() {
            if self.path_sources[side] != source {
                self.path_edits[side] = source.clone();
                self.path_sources[side] = source;
            }
        }
        let top = rect.top() + 11.0;
        let mut submit = false;
        for (side, (x0, x1, browse)) in [(10.0, 627.0, 638.0), (717.0, 1334.0, 1345.0)]
            .into_iter()
            .enumerate()
        {
            let edit = egui::Rect::from_min_max(
                egui::pos2(rect.left() + x0, top),
                egui::pos2(rect.left() + x1, top + 25.0),
            );
            submit |= combo_edit(ui, edit, &mut self.path_edits[side], ("sync-path", side));
            let button = egui::Rect::from_min_size(
                egui::pos2(rect.left() + browse, top + 1.0),
                egui::vec2(28.0, 27.0),
            );
            if push_button(ui, button, ("sync-browse", side), true, |p, r| {
                p.text(
                    r.center() + egui::vec2(0.0, 2.0),
                    egui::Align2::CENTER_CENTER,
                    "...",
                    egui::FontId::proportional(theme::UI_SIZE),
                    egui::Color32::BLACK,
                );
            }) {
                self.folder.folder_command(if side == 0 {
                    "browse-left"
                } else {
                    "browse-right"
                });
            }
        }
        let swap = egui::Rect::from_min_size(
            egui::pos2(rect.left() + 678.0, top + 1.0),
            egui::vec2(28.0, 27.0),
        );
        if push_button(ui, swap, "sync-swap", true, |p, r| {
            icons::paint(
                p,
                egui::Rect::from_center_size(r.center(), egui::vec2(16.0, 16.0)),
                Icon::Swap,
            );
        }) {
            self.folder.folder_command("Swap Sides");
        }
        if submit {
            let paths = [
                self.path_edits[0].trim().to_string(),
                self.path_edits[1].trim().to_string(),
            ];
            self.folder.open_paths(paths);
        }
    }
    /// Operations area (191 px): method dropdown + rules button, description,
    /// plan summary, filter box with Accept/Cancel, Sync Now.
    fn operations_area(&mut self, ui: &mut egui::Ui, rect: egui::Rect, busy: bool) {
        let painter = ui.painter().clone();
        let font = egui::FontId::proportional(theme::UI_SIZE);
        let at = |x: f32, y: f32| rect.min + egui::vec2(x, y);
        painter.text(
            at(239.0, 17.5),
            egui::Align2::LEFT_CENTER,
            "Operations:",
            font.clone(),
            egui::Color32::BLACK,
        );
        painter.text(
            at(677.0, 17.5),
            egui::Align2::LEFT_CENTER,
            "Filters:",
            font.clone(),
            egui::Color32::BLACK,
        );
        // Operation method dropdown.
        let combo = egui::Rect::from_min_max(at(239.0, 32.0), at(566.0, 57.0));
        let response = ui.interact(combo, egui::Id::unique("sync-method"), egui::Sense::click());
        frame_box(&painter, combo, egui::Color32::WHITE, COMBO_EDGE);
        paint_method_icon(&painter, egui::pos2(combo.left() + 4.0, combo.center().y));
        painter.text(
            egui::pos2(combo.left() + 20.0, combo.center().y),
            egui::Align2::LEFT_CENTER,
            METHODS[self.method],
            font.clone(),
            egui::Color32::BLACK,
        );
        chevron(&painter, egui::pos2(combo.right() - 9.0, combo.center().y));
        let mut method = self.method;
        egui::Popup::from_toggle_button_response(&response)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
            .show(|ui| {
                ui.set_min_width(combo.width() - 8.0);
                for (index, label) in METHODS.iter().enumerate() {
                    if ui.selectable_label(method == index, *label).clicked() {
                        method = index;
                    }
                }
            });
        if method != self.method && !busy {
            self.method = method;
            self.send_mode();
        }
        // Session rules button.
        let rules = egui::Rect::from_min_size(at(577.0, 33.0), egui::vec2(28.0, 27.0));
        if push_button(ui, rules, "sync-rules", true, |p, r| {
            icons::paint(
                p,
                egui::Rect::from_center_size(r.center(), egui::vec2(16.0, 16.0)),
                Icon::Rules,
            );
        }) {
            self.folder.folder_command("Session Settings");
        }
        // Filters box + filter button, Accept / Cancel.
        let applied = self.folder.filter_names().to_string();
        if self.filter_source != applied {
            self.filter_source = applied.clone();
            self.filter_draft = applied.clone();
        }
        let filter = egui::Rect::from_min_max(at(677.0, 32.0), at(1100.0, 57.0));
        let submitted = combo_edit(ui, filter, &mut self.filter_draft, "sync-filter");
        let funnel = egui::Rect::from_min_size(at(1111.0, 33.0), egui::vec2(28.0, 27.0));
        if push_button(ui, funnel, "sync-filter-rules", true, |p, r| {
            icons::paint(
                p,
                egui::Rect::from_center_size(r.center(), egui::vec2(16.0, 16.0)),
                Icon::Filters,
            );
        }) {
            self.folder.folder_command("Session Settings");
        }
        let edited = self.filter_draft != applied;
        let accept = egui::Rect::from_min_size(at(1174.0, 33.0), egui::vec2(93.0, 28.0));
        if push_button(ui, accept, "sync-accept", edited, |p, r| {
            labelled(p, r, "Accept", edited, Icon::Refresh);
        }) || (submitted && edited)
        {
            self.folder.set_filter_names(self.filter_draft.clone());
        }
        let cancel = egui::Rect::from_min_size(at(1279.0, 33.0), egui::vec2(93.0, 28.0));
        if push_button(ui, cancel, "sync-cancel", edited, |p, r| {
            labelled(p, r, "Cancel", edited, Icon::Stop);
        }) {
            self.filter_draft = applied;
        }
        // Sync Now (big button).
        let can_sync = self.can_sync();
        let sync = egui::Rect::from_min_size(at(1279.0, 71.0), egui::vec2(94.0, 72.0));
        if push_button(ui, sync, "sync-now", can_sync, |p, r| {
            p.text(
                egui::pos2(r.center().x, r.top() + 22.0),
                egui::Align2::CENTER_CENTER,
                "Sync Now",
                egui::FontId::proportional(theme::UI_SIZE),
                if can_sync {
                    egui::Color32::BLACK
                } else {
                    DISABLED_TEXT
                },
            );
            icons::paint(
                p,
                egui::Rect::from_center_size(
                    egui::pos2(r.center().x, r.top() + 47.0),
                    egui::vec2(20.0, 20.0),
                ),
                Icon::Sync,
            );
        }) {
            self.command("Sync Now");
        }
        // Description and plan summary.
        painter.text(
            at(239.0, 75.5),
            egui::Align2::LEFT_CENTER,
            DESCRIPTIONS[self.method],
            font.clone(),
            egui::Color32::BLACK,
        );
        if self.method == 5 {
            self.custom_rules(
                ui,
                egui::Rect::from_min_max(at(239.0, 92.0), at(1260.0, 186.0)),
            );
        } else if let Some(summary) = self.folder.snapshot.sync_summary {
            for (i, line) in summary_lines(&summary).iter().enumerate() {
                painter.text(
                    at(239.0, 102.5 + 17.0 * i as f32),
                    egui::Align2::LEFT_CENTER,
                    line,
                    font.clone(),
                    egui::Color32::BLACK,
                );
            }
        }
    }
    fn custom_rules(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let before = self.custom;
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 4.0);
            ui.horizontal_wrapped(|ui| {
                for (index, label) in [
                    "Left newer",
                    "Right newer",
                    "Left orphan",
                    "Right orphan",
                    "Different",
                ]
                .iter()
                .enumerate()
                {
                    egui::ComboBox::from_id_salt(("sync_custom", index))
                        .selected_text(format!("{label}: {}", choice_label(self.custom[index])))
                        .show_ui(ui, |ui| {
                            for choice in choices() {
                                ui.selectable_value(
                                    &mut self.custom[index],
                                    choice,
                                    choice_label(choice),
                                );
                            }
                        });
                }
            });
        });
        if before != self.custom && !self.folder.busy() {
            self.send_mode();
        }
    }
    fn info_window(&mut self, ctx: &egui::Context) {
        if !self.show_info {
            return;
        }
        let mut open = true;
        let stats = self.folder.snapshot.sync_stats;
        egui::Window::new("Folder Sync Info")
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| match stats {
                Some(stats) => {
                    ui.label(format!(
                        "Preview: {} copy, {} delete, {} create, {} leave, {} conflicts",
                        stats.copies, stats.deletes, stats.creates, stats.skipped, stats.conflicts
                    ));
                }
                None => {
                    ui.label("Sync preview is being prepared");
                }
            });
        self.show_info = open;
    }
}
/// Summary lines of the operations area, from the existing sync plan.
pub(crate) fn summary_lines(summary: &SyncSummary) -> Vec<String> {
    let mut lines = Vec::new();
    for ((files, folders, bytes), side) in [(summary.to_left, "left"), (summary.to_right, "right")]
    {
        if files + folders > 0 {
            lines.push(format!(
                "Copy {files} file(s) and {folders} folder(s) to {side}, {bytes} bytes"
            ));
        }
    }
    for ((files, folders), side) in [
        (summary.delete_left, "left"),
        (summary.delete_right, "right"),
    ] {
        if files + folders > 0 {
            lines.push(format!(
                "Delete {files} file(s) and {folders} folder(s) on {side}"
            ));
        }
    }
    let (files, folders) = summary.prompts;
    if files > 0 {
        lines.push(format!("Prompt for {files} file(s)"));
    }
    if folders > 0 {
        lines.push(format!("Prompt for {folders} folder(s)"));
    }
    lines
}
fn frame_box(painter: &egui::Painter, rect: egui::Rect, fill: egui::Color32, edge: egui::Color32) {
    painter.rect_filled(rect, 0.0, fill);
    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0, edge),
        egui::StrokeKind::Inside,
    );
}
fn chevron(painter: &egui::Painter, center: egui::Pos2) {
    let stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(96, 96, 96));
    painter.line_segment(
        [
            center + egui::vec2(-4.0, -2.0),
            center + egui::vec2(0.0, 2.0),
        ],
        stroke,
    );
    painter.line_segment(
        [
            center + egui::vec2(0.0, 2.0),
            center + egui::vec2(4.0, -2.0),
        ],
        stroke,
    );
}
/// Editable combo box (white, #8D8D8D edge, inline chevron); true on Enter.
fn combo_edit(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    text: &mut String,
    id: impl std::hash::Hash + std::fmt::Debug,
) -> bool {
    frame_box(ui.painter(), rect, egui::Color32::WHITE, COMBO_EDGE);
    chevron(
        ui.painter(),
        egui::pos2(rect.right() - 9.0, rect.center().y),
    );
    let inner = egui::Rect::from_min_max(
        rect.min + egui::vec2(1.0, 1.0),
        rect.max - egui::vec2(19.0, 1.0),
    );
    let mut submitted = false;
    ui.scope_builder(egui::UiBuilder::new().id_salt(id).max_rect(inner), |ui| {
        ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::NONE;
        ui.visuals_mut().widgets.hovered.bg_stroke = egui::Stroke::NONE;
        ui.visuals_mut().widgets.active.bg_stroke = egui::Stroke::NONE;
        ui.visuals_mut().selection.stroke = egui::Stroke::NONE;
        let response = ui.add_sized(
            inner.size(),
            egui::TextEdit::singleline(text)
                .frame(egui::Frame::NONE)
                .margin(egui::vec2(2.0, 4.0)),
        );
        submitted = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    });
    submitted
}
/// Classic push button (#D0D0D0 edge, #FDFDFD face, #BABABA bottom line);
/// disabled buttons are flat #F9F9F9 with a #E9E9E9 edge.
fn push_button(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    id: impl std::hash::Hash + std::fmt::Debug,
    enabled: bool,
    content: impl FnOnce(&egui::Painter, egui::Rect),
) -> bool {
    let response = ui.interact(
        rect,
        egui::Id::unique(("sync-button", id)),
        if enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    let painter = ui.painter();
    if enabled {
        let fill = if response.is_pointer_button_down_on() {
            egui::Color32::from_rgb(204, 228, 247)
        } else if response.hovered() {
            theme::HOVER_BLUE
        } else {
            BUTTON_FILL
        };
        frame_box(painter, rect, fill, BUTTON_EDGE);
        painter.hline(
            rect.left()..=rect.right() - 1.0,
            rect.bottom() - 0.5,
            egui::Stroke::new(1.0, BUTTON_SHADOW),
        );
    } else {
        frame_box(painter, rect, DISABLED_FILL, DISABLED_EDGE);
    }
    content(painter, rect);
    enabled && response.clicked()
}
fn labelled(painter: &egui::Painter, rect: egui::Rect, label: &str, enabled: bool, icon: Icon) {
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 25.0, rect.center().y),
        egui::vec2(16.0, 16.0),
    );
    icons::paint(painter, icon_rect, icon);
    if !enabled {
        // Washed-out icon like the reference's disabled buttons.
        painter.rect_filled(icon_rect, 0.0, DISABLED_FILL.gamma_multiply(0.7));
    }
    painter.text(
        egui::pos2(rect.left() + 37.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(theme::UI_SIZE),
        if enabled {
            egui::Color32::BLACK
        } else {
            DISABLED_TEXT
        },
    );
}
/// Original two-arrow glyph of the operation method (green, 14×8).
fn paint_method_icon(painter: &egui::Painter, left_center: egui::Pos2) {
    let green = egui::Color32::from_rgb(25, 205, 10);
    let edge = egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 80, 80));
    for (dx, right) in [(0.0, false), (7.0, true)] {
        let s = if right { 1.0 } else { -1.0 };
        let c = left_center + egui::vec2(4.0 + dx, 0.0);
        let points = vec![
            c + egui::vec2(-3.5 * s, -4.0),
            c + egui::vec2(3.5 * s, 0.0),
            c + egui::vec2(-3.5 * s, 4.0),
        ];
        painter.add(egui::Shape::convex_polygon(points, green, edge));
    }
}
fn choices() -> [SyncChoice; 6] {
    [
        SyncChoice::LeaveAlone,
        SyncChoice::CopyRightToLeft,
        SyncChoice::CopyLeftToRight,
        SyncChoice::DeleteLeft,
        SyncChoice::DeleteRight,
        SyncChoice::Exclude,
    ]
}
fn item(
    label: &str,
    mnemonic: Option<char>,
    shortcut: Option<&str>,
    children: Vec<MenuItem>,
) -> MenuItem {
    MenuItem {
        label: label.into(),
        mnemonic,
        shortcut: shortcut.map(str::to_owned),
        kind: if children.is_empty() {
            MenuItemKind::Action
        } else {
            MenuItemKind::Submenu(children)
        },
        enabled: false,
        action_id: label.into(),
    }
}
fn sep() -> MenuItem {
    MenuItem {
        label: String::new(),
        mnemonic: None,
        shortcut: None,
        kind: MenuItemKind::Separator,
        enabled: false,
        action_id: String::new(),
    }
}
/// Menu inventory transcribed from the offline `inventories.sync.menus`
/// (Help uses product-neutral wording).
pub(crate) fn inventory() -> Vec<MenuSpec> {
    vec![
        MenuSpec {
            title: "Session".into(),
            mnemonic: Some('s'),
            items: vec![
                item(
                    "New Session",
                    Some('n'),
                    None,
                    vec![
                        item("Folder Compare", Some('f'), None, vec![]),
                        item("Folder Sync", Some('o'), None, vec![]),
                        item("Text Compare", Some('t'), None, vec![]),
                        item("Hex Compare", Some('h'), None, vec![]),
                        item("Media Compare", Some('m'), None, vec![]),
                        item("Picture Compare", Some('p'), None, vec![]),
                        item("Table Compare", Some('b'), None, vec![]),
                        item("Version Compare", Some('v'), None, vec![]),
                    ],
                ),
                item("New Tab", Some('t'), Some("Ctrl+T"), vec![]),
                item("New Window", Some('w'), None, vec![]),
                item("Open Session", Some('o'), Some("Shift+Ctrl+O"), vec![]),
                sep(),
                item(
                    "Load Workspace",
                    Some('k'),
                    None,
                    vec![item("Manage Workspaces...", None, None, vec![])],
                ),
                item("Save Workspace As...", Some('v'), None, vec![]),
                sep(),
                item("Save Session", Some('s'), Some("Shift+Ctrl+S"), vec![]),
                item("Save Session As...", Some('a'), None, vec![]),
                item("Session Settings...", Some('e'), None, vec![]),
                item("Locked", Some('l'), None, vec![]),
                item("Clear Session", Some('c'), Some("Shift+Ctrl+C"), vec![]),
                item("Close Tab", None, Some("Ctrl+W"), vec![]),
                sep(),
                item("Swap Sides", Some('d'), None, vec![]),
                sep(),
                item("Folder Sync Info", Some('i'), Some("Ctrl+I"), vec![]),
                item("Sync Now", Some('y'), None, vec![]),
                sep(),
                item("Compare Base Folders", Some('m'), None, vec![]),
                item("Compare Parent Folders", Some('f'), None, vec![]),
                sep(),
                item("Exit", Some('x'), Some("Ctrl+Q"), vec![]),
            ],
        },
        MenuSpec {
            title: "Actions".into(),
            mnemonic: Some('a'),
            items: vec![
                item("Leave Alone", Some('v'), None, vec![]),
                item("Copy Right to Left", Some('l'), None, vec![]),
                item("Copy Left to Right", Some('r'), None, vec![]),
                item("Delete Left", Some('f'), None, vec![]),
                item("Delete Right", Some('g'), None, vec![]),
                sep(),
                item("Close Folder", None, None, vec![]),
                item("Open Subfolders", None, None, vec![]),
                item("Close Subfolders", None, None, vec![]),
                item("Set as Base Folders", Some('b'), None, vec![]),
                item("Open in New View", Some('o'), None, vec![]),
                item(
                    "Open With",
                    Some('w'),
                    None,
                    vec![
                        item("Associated Application", None, None, vec![]),
                        sep(),
                        item("Folder Compare", Some('f'), None, vec![]),
                        item("Folder Sync", Some('o'), None, vec![]),
                    ],
                ),
                sep(),
                item("Exclude", Some('e'), None, vec![]),
                item("Copy Filename", Some('i'), None, vec![]),
                item("Refresh Selection", Some('s'), Some("Shift+F5"), vec![]),
                sep(),
                item("Explorer", Some('x'), None, vec![]),
            ],
        },
        MenuSpec {
            title: "Edit".into(),
            mnemonic: Some('e'),
            items: vec![
                item("Expand All", Some('e'), None, vec![]),
                item("Collapse All", Some('c'), None, vec![]),
                sep(),
                item("Select All", Some('a'), Some("Ctrl+A"), vec![]),
                item("Invert Selection", Some('i'), None, vec![]),
                item("Refresh", Some('r'), Some("F5"), vec![]),
                item("Full Refresh", Some('u'), Some("Ctrl+F5"), vec![]),
            ],
        },
        MenuSpec {
            title: "Search".into(),
            mnemonic: Some('s'),
            items: vec![
                item("Next Conflict", Some('n'), Some("Ctrl+N"), vec![]),
                item("Previous Conflict", Some('p'), Some("Ctrl+P"), vec![]),
                item("Find Filename...", Some('f'), Some("Ctrl+F"), vec![]),
                item("Find Next Filename", Some('x'), Some("F3"), vec![]),
                item(
                    "Find Previous Filename",
                    Some('v'),
                    Some("Shift+F3"),
                    vec![],
                ),
            ],
        },
        MenuSpec {
            title: "View".into(),
            mnemonic: Some('v'),
            items: vec![
                item("Ignore Unimportant Differences", Some('u'), None, vec![]),
                item("Suppress Filters", Some('s'), None, vec![]),
                sep(),
                item(
                    "Columns",
                    Some('c'),
                    None,
                    vec![
                        item("Ext", Some('e'), None, vec![]),
                        item("Status", Some('s'), None, vec![]),
                        item("Revision", Some('i'), None, vec![]),
                        item("Version", Some('v'), None, vec![]),
                        item("Size", Some('z'), None, vec![]),
                        item("CRC", Some('c'), None, vec![]),
                        item("Modified", Some('m'), None, vec![]),
                        item("Attributes", Some('a'), None, vec![]),
                        item("Owner", Some('o'), None, vec![]),
                        item("Group", Some('g'), None, vec![]),
                    ],
                ),
                item("Legend", Some('l'), Some("Ctrl+Alt+L"), vec![]),
                item("Log", Some('o'), None, vec![]),
                item("Toolbar", Some('t'), None, vec![]),
            ],
        },
        MenuSpec {
            title: "Tools".into(),
            mnemonic: Some('t'),
            items: vec![
                item("Options...", Some('o'), None, vec![]),
                item("File Formats...", Some('f'), None, vec![]),
                item("Profiles...", Some('p'), None, vec![]),
                sep(),
                item("Export Settings...", Some('e'), None, vec![]),
                item("Import Settings...", Some('i'), None, vec![]),
                item("Restore Factory Defaults...", Some('r'), None, vec![]),
                sep(),
                item("Save Snapshot...", Some('h'), None, vec![]),
                item("Edit Text File", Some('d'), None, vec![]),
                item("View Patch...", Some('a'), None, vec![]),
            ],
        },
        MenuSpec {
            title: "Help".into(),
            mnemonic: Some('h'),
            items: vec![
                item("Contents", Some('c'), None, vec![]),
                item("Context Sensitive Help", Some('h'), Some("F1"), vec![]),
                sep(),
                item("Project Website", Some('w'), None, vec![]),
                item("Check for Updates", Some('u'), None, vec![]),
                item("Support", Some('s'), None, vec![]),
                item("Enter Key...", Some('e'), None, vec![]),
                sep(),
                item("About Matcha", Some('a'), None, vec![]),
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dir_view::{SYNC_CONFLICT, SYNC_TO_LEFT, SYNC_TO_RIGHT};
    use std::time::{Duration, Instant};

    #[test]
    fn summary_lines_follow_the_reference_wording() {
        let summary = SyncSummary {
            to_left: (3, 2, 45),
            to_right: (2, 2, 26),
            prompts: (1, 0),
            ..Default::default()
        };
        assert_eq!(
            summary_lines(&summary),
            [
                "Copy 3 file(s) and 2 folder(s) to left, 45 bytes",
                "Copy 2 file(s) and 2 folder(s) to right, 26 bytes",
                "Prompt for 1 file(s)",
            ]
        );
        assert!(summary_lines(&SyncSummary::default()).is_empty());
    }

    #[test]
    fn menu_inventory_matches_tokens_and_hides_the_reference_name() {
        let tokens: serde_json::Value = serde_json::from_str(
            std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/visual-parity/tokens.json"),
            )
            .unwrap()
            .trim_start_matches('\u{feff}'),
        )
        .unwrap();
        let menus = inventory();
        let reference = tokens["inventories"]["sync"]["menus"].as_array().unwrap();
        assert_eq!(menus.len(), reference.len());
        for (menu, expected) in menus.iter().zip(reference) {
            assert_eq!(menu.title, expected["name"].as_str().unwrap());
            let items = expected["items"].as_array().unwrap();
            assert_eq!(menu.items.len(), items.len(), "{}", menu.title);
            for (item, expected) in menu.items.iter().zip(items) {
                if menu.title != "Help" {
                    assert_eq!(item.label, expected["name"].as_str().unwrap_or_default());
                }
            }
        }
        assert!(menus[6].items.iter().any(|i| i.label == "About Matcha"));
    }

    #[test]
    fn sync_view_plans_update_both_with_arrows_summary_and_layout() {
        let root = std::env::temp_dir().join(format!("matcha-sync-view-{}", std::process::id()));
        for side in ["left", "right"] {
            std::fs::create_dir_all(root.join(side)).unwrap();
            std::fs::write(root.join(side).join("same.txt"), b"same").unwrap();
        }
        std::fs::write(root.join("left/only-left.txt"), b"12345").unwrap();
        std::fs::write(root.join("right/only-right.txt"), b"123").unwrap();
        let ctx = egui::Context::default();
        crate::chrome::theme::apply(&ctx);
        let mut view = SyncView::open(&ctx, &root.join("left"), &root.join("right"));
        let started = Instant::now();
        let run = |view: &mut SyncView| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1384.0, 961.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    view.ui(ui);
                },
            );
            out.textures_delta.clear();
            out
        };
        while view.folder.busy() || view.folder.snapshot.sync_summary.is_none() {
            run(&mut view);
            assert!(started.elapsed() < Duration::from_secs(20));
            std::thread::sleep(Duration::from_millis(5));
        }
        let out = run(&mut view);
        let summary = view.folder.snapshot.sync_summary.unwrap();
        assert_eq!(summary.to_right, (1, 0, 5));
        assert_eq!(summary.to_left, (1, 0, 3));
        assert_eq!(METHODS[view.method], "Update Both");
        // Differences only: the identical file is not listed.
        let names: Vec<_> = view
            .folder
            .snapshot
            .rows
            .iter()
            .flat_map(|r| r.sides.iter().flatten().map(|s| s.name.clone()))
            .collect();
        assert!(!names.contains(&"same.txt".to_string()), "{names:?}");
        let marks: Vec<_> = view
            .folder
            .snapshot
            .rows
            .iter()
            .map(|r| r.sync_mark)
            .collect();
        assert!(marks.contains(&SYNC_TO_RIGHT) && marks.contains(&SYNC_TO_LEFT));
        assert!(!marks.contains(&SYNC_CONFLICT));
        let has = |text: &str| {
            out.shapes
                .iter()
                .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == text))
        };
        assert!(has("Copy 1 file(s) and 0 folder(s) to right, 5 bytes"));
        assert!(has("Copy newer and orphan files/folders to other side."));
        assert!(has("Sync Now") && has("Operations:") && has("Filters:"));
        assert!(view.can_sync());
        assert_eq!(view.title(), "left <--> right - Folder Sync - Matcha");
        drop(view);
        let _ = std::fs::remove_dir_all(root);
    }
}
