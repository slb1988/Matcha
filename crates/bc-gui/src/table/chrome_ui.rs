//! Classic chrome of the table view: menu bar (inventory transcribed from the
//! offline tokens), large-icon toolbar, per-side path bar and file info row,
//! per-pane status, status bar, window title. Commands route to the existing
//! view actions; inventory items without an implementation stay disabled.
use super::{
    cache::Filter,
    find::Action,
    grid,
    view::{FileMode, PathDialog, Pending, TableCompareView},
};
use crate::chrome::menubar::{MenuItem, MenuItemKind};
use bc_engine_table::{compare::Importance, rules::Layers};
use bc_formats::model::TableType;
use bc_session::prefs::Prefs;
use eframe::egui;
use std::path::PathBuf;

/// Inventory items appended to the reference menus for existing table
/// features that have no reference counterpart (same convention as the text
/// view's extra items).
const EXTRAS: [(usize, &[&str]); 4] = [
    (0, &["Export Summary as CSV..."]),
    (1, &["Export as CSV..."]),
    (
        2,
        &[
            "Sort Ascending",
            "Sort Descending",
            "Clear Alignment Overrides",
        ],
    ),
    (
        4,
        &[
            "Show None",
            "Compare Values",
            "Compare Values + Formulas",
            "Compare Values + Formulas + Number Formats",
        ],
    ),
];
const ENABLED: &[&str] = &[
    "Session Settings...",
    "New Tab",
    "Close Tab",
    "Swap Sides",
    "Reload Files",
    "Recompare Files",
    "Table Compare Report...",
    "Export Summary as CSV...",
    "Table Compare Info",
    "Using:Text Compare",
    "Exit",
    "Open File...",
    "Open Clipboard",
    "Save File",
    "Save File As...",
    "Export as CSV...",
    "Undo",
    "Redo",
    "Align With...",
    "Copy to Right",
    "Copy Cell to Right",
    "Cut",
    "Copy",
    "Paste",
    "Delete",
    "Delete Row",
    "Delete Column",
    "Insert Row Before",
    "Insert Row After",
    "Insert Column Before",
    "Insert Column After",
    "Modify...",
    "Select All",
    "Sort Ascending",
    "Sort Descending",
    "Clear Alignment Overrides",
    "Next Difference Cell",
    "Previous Difference Cell",
    "Next Difference Row",
    "Previous Difference Row",
    "Next Edit",
    "Previous Edit",
    "Find...",
    "Replace...",
    "Find Next",
    "Find Previous",
    "Go To...",
    "Resize Columns to Fit",
];

/// Lazily resolved info-row labels of one side (modified time off the UI
/// thread, encoding/line ending from the first 64 KiB of the file).
#[derive(Default)]
pub(super) struct FileLabels {
    key: Option<(PathBuf, u64)>,
    modified: crate::text::status_bar::ModifiedLabel,
    encoding: String,
    ending: &'static str,
}

impl TableCompareView {
    pub fn title(&self) -> String {
        let name = |s: usize| {
            self.files[s]
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Clipboard".into())
        };
        format!("{} <--> {} - Table Compare - Matcha", name(0), name(1))
    }
    /// Refresh the per-side info labels when the file or its size changed;
    /// true while a modified-time lookup is still running.
    pub fn labels_pending(&mut self) -> bool {
        let mut pending = false;
        for s in 0..2 {
            let current = self.files[s]
                .path
                .as_ref()
                .map(|p| (p, self.files[s].byte_len));
            if self.labels[s].key.as_ref().map(|(p, b)| (p, *b)) != current {
                let key = current.map(|(p, b)| (p.clone(), b));
                let labels = &mut self.labels[s];
                labels.key = key.clone();
                let path = key.map(|(p, _)| p);
                labels.modified = crate::text::status_bar::ModifiedLabel::resolve(
                    path.as_ref()
                        .and_then(|p| std::fs::metadata(p).ok())
                        .and_then(|m| m.modified().ok()),
                );
                let (encoding, ending) = sniff(path.as_deref(), &self.options[s]);
                labels.encoding = encoding;
                labels.ending = ending;
            }
            pending |= self.labels[s].modified.poll();
        }
        pending
    }
    pub(super) fn checked(&self, id: &str) -> Option<bool> {
        let rules = self.pair.as_ref().map(|p| p.map.rules());
        Some(match id {
            "Show All" => self.filter == Filter::All,
            "Show Differences" => self.filter == Filter::Differences,
            "Show Same" => self.filter == Filter::Same,
            "Show None" => self.filter == Filter::None,
            "Ignore Unimportant Differences" => rules.as_ref()?.ignore_unimportant,
            "Hide Same Columns" => self.hide_same,
            "Visible Whitespace" => self.grid.whitespace,
            "Row Numbers" => self.grid.row_numbers,
            "Side-by-Side Layout" => !self.grid.stacked,
            "Over-Under Layout" => self.grid.stacked,
            "Thumbnail" => self.show_thumbnail,
            "Text Details" => self.show_details,
            "File Info" => self.show_info,
            "Toolbar" => self.show_toolbar,
            "Compare Values" => rules.as_ref()?.layers == Layers::Values,
            "Compare Values + Formulas" => rules.as_ref()?.layers == Layers::ValuesAndFormulas,
            "Compare Values + Formulas + Number Formats" => rules.as_ref()?.layers == Layers::All,
            _ => return None,
        })
    }
    fn workbook(&self, s: usize) -> bool {
        self.pair.as_ref().is_some_and(|p| {
            if s == 0 {
                p.map.left.borrow().source_is_workbook()
            } else {
                p.map.right.borrow().source_is_workbook()
            }
        })
    }
    fn update_menus(&mut self) {
        fn update(view: &TableCompareView, items: &mut [MenuItem]) {
            for item in items {
                if let MenuItemKind::Submenu(children) = &mut item.kind {
                    update(view, children);
                    item.enabled = children
                        .iter()
                        .any(|c| c.enabled && !matches!(c.kind, MenuItemKind::Separator));
                    continue;
                }
                item.enabled = ENABLED.contains(&item.action_id.as_str());
                let to = if view.grid.active == 0 {
                    "Right"
                } else {
                    "Left"
                };
                match item.action_id.as_str() {
                    "Copy to Right" => item.label = format!("Copy to {to}"),
                    "Copy Cell to Right" => item.label = format!("Copy Cell to {to}"),
                    "Save File" | "Save File As..." => {
                        item.enabled = !view.workbook(view.grid.active)
                    }
                    _ => {}
                }
                if let Some(checked) = view.checked(&item.action_id) {
                    item.kind = MenuItemKind::Check { checked };
                    item.enabled = true;
                }
            }
        }
        if self.menus.is_empty() {
            let mut menus = super::menu::inventory();
            // Session › Compare Files Using › Text Compare shares its label
            // with Session › New Session › Text Compare.
            for item in &mut menus[0].items {
                if item.label == "Compare Files Using" {
                    if let MenuItemKind::Submenu(children) = &mut item.kind {
                        for child in children {
                            child.action_id = format!("Using:{}", child.label);
                        }
                    }
                }
            }
            for (index, names) in EXTRAS {
                menus[index].items.push(MenuItem {
                    label: String::new(),
                    mnemonic: None,
                    shortcut: None,
                    kind: MenuItemKind::Separator,
                    enabled: false,
                    action_id: String::new(),
                });
                menus[index]
                    .items
                    .extend(names.iter().map(|n| MenuItem::action(*n, *n)));
            }
            self.menus = menus;
        }
        let mut menus = std::mem::take(&mut self.menus);
        for menu in &mut menus {
            update(self, &mut menu.items);
        }
        self.menus = menus;
    }
    fn set_filter(&mut self, filter: Filter) {
        if self.filter != filter {
            self.filter = filter;
            if let Some(p) = &mut self.pair {
                p.projection = p.cache.project(self.filter);
            }
            self.grid.scroll.y = 0.0;
        }
    }
    fn set_layers(&mut self, layers: Layers) {
        let changed = self.pair.as_ref().is_some_and(|p| {
            let mut rules = p.map.rules_mut();
            let changed = rules.layers != layers;
            rules.layers = layers;
            changed
        });
        if changed {
            self.project();
        }
    }
    fn file_dialog(&mut self, mode: FileMode) {
        self.dialog = Some(PathDialog {
            mode,
            path: String::new(),
        });
    }
    /// Execute a menu/toolbar/shortcut command by its action id.
    pub(super) fn command(&mut self, id: &str, ctx: &egui::Context, prefs: &Prefs) {
        let s = self.grid.active;
        match id {
            // Home 只切换标签页、不丢弃会话，故无需确认。
            "Home" => self.action = Some(super::TableAction::Home),
            "Close Tab" => self.request(Pending::Close),
            "New Tab" => self.action = Some(super::TableAction::NewTab),
            "Exit" => self.request(Pending::Exit),
            "Session Settings..." => self.settings_open(),
            "Swap Sides" => self.swap(),
            "Reload Files" => self.request(Pending::Reload),
            "Recompare Files" => self.recompare(),
            "Table Compare Report..." => self.file_dialog(FileMode::Summary(false)),
            "Export Summary as CSV..." => self.file_dialog(FileMode::Summary(true)),
            "Table Compare Info" => self.show_statistics = true,
            "Using:Text Compare" => self.request(Pending::Text),
            "Using:Hex Compare" => self.request(Pending::Hex),
            "Open File..." => self.file_dialog(FileMode::Open(s)),
            "Open Clipboard" => {
                self.clipboard_side = Some(s);
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
            }
            "Save File" => {
                if let Err(e) = self.save(s, prefs) {
                    self.message = e.to_string();
                }
            }
            "Save File As..." => self.file_dialog(FileMode::Save(s)),
            "Export as CSV..." => self.file_dialog(FileMode::Export(s)),
            "Undo" => self.undo(false),
            "Redo" => self.undo(true),
            "Align With..." => {
                if !self.is_ready() {
                    self.message = "Wait for comparison before aligning rows".into();
                } else if let Ok(r) = self.source_row(s, self.grid.cursor.row) {
                    self.align_with = Some((s, r));
                }
            }
            "Clear Alignment Overrides" => self.clear_alignment(),
            "Copy to Right" => self.copy(true),
            "Copy Cell to Right" => self.copy(false),
            "Cut" => {
                if let Some(text) = self.clipboard_text() {
                    ctx.copy_text(text);
                    self.clear_selected();
                }
            }
            "Copy" => {
                if let Some(text) = self.clipboard_text() {
                    ctx.copy_text(text);
                }
            }
            "Paste" => {
                self.paste_requested = true;
                self.focus_grid = Some(s);
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
            }
            "Delete" => self.clear_selected(),
            "Modify..." => self.begin_edit(s, self.grid.cursor, None),
            "Select All" => self.select_all(),
            "Insert Row Before"
            | "Insert Row After"
            | "Delete Row"
            | "Insert Column Before"
            | "Insert Column After"
            | "Delete Column"
            | "Sort Descending"
            | "Sort Ascending" => {
                let op = match id {
                    "Insert Row Before" => 0,
                    "Insert Row After" => 1,
                    "Delete Row" => 2,
                    "Insert Column Before" => 3,
                    "Insert Column After" => 4,
                    "Delete Column" => 5,
                    "Sort Descending" => 6,
                    _ => 7,
                };
                if let Err(e) = self.structure(op) {
                    self.message = e.to_string();
                }
            }
            "Next Difference Cell" => self.navigate(true, 0),
            "Previous Difference Cell" => self.navigate(false, 0),
            "Next Difference Row" => self.navigate(true, 1),
            "Previous Difference Row" => self.navigate(false, 1),
            "Next Edit" => self.navigate(true, 2),
            "Previous Edit" => self.navigate(false, 2),
            "Find..." | "Replace..." => {
                self.find.open = true;
                self.key_focus = false;
            }
            "Find Next" | "Find Previous" => {
                self.find.open = true;
                self.key_focus = false;
                let action = if id == "Find Next" {
                    Action::Next
                } else {
                    Action::Previous
                };
                if let Err(e) = self.find_action(action) {
                    self.message = e.to_string();
                }
            }
            "Go To..." => self.show_goto = true,
            "Show All" => self.set_filter(Filter::All),
            "Show Differences" => self.set_filter(Filter::Differences),
            "Show Same" => self.set_filter(Filter::Same),
            "Show None" => self.set_filter(Filter::None),
            "Ignore Unimportant Differences" => {
                let changed = self.pair.as_ref().is_some_and(|p| {
                    let mut rules = p.map.rules_mut();
                    rules.ignore_unimportant = !rules.ignore_unimportant;
                    true
                });
                if changed {
                    self.project();
                }
            }
            "Compare Values" => self.set_layers(Layers::Values),
            "Compare Values + Formulas" => self.set_layers(Layers::ValuesAndFormulas),
            "Compare Values + Formulas + Number Formats" => self.set_layers(Layers::All),
            "Hide Same Columns" => {
                self.hide_same = !self.hide_same;
                self.refresh_grid();
            }
            "Resize Columns to Fit" => self.project_with_fit(true),
            "Visible Whitespace" => self.grid.whitespace = !self.grid.whitespace,
            "Row Numbers" => self.grid.row_numbers = !self.grid.row_numbers,
            "Side-by-Side Layout" => self.grid.stacked = false,
            "Over-Under Layout" => self.grid.stacked = true,
            "Thumbnail" => self.show_thumbnail = !self.show_thumbnail,
            "Text Details" => self.show_details = !self.show_details,
            "File Info" => self.show_info = !self.show_info,
            "Toolbar" => self.show_toolbar = !self.show_toolbar,
            _ => {}
        }
    }
    /// Global shortcuts listed in the menus (consumed before grid keys).
    pub(super) fn shortcuts(&mut self, ctx: &egui::Context, prefs: &Prefs) {
        use egui::{Key, Modifiers};
        let shift_command = Modifiers::COMMAND | Modifiers::SHIFT;
        let table: [(Modifiers, Key, &str); 15] = [
            (shift_command, Key::N, "Next Difference Cell"),
            (shift_command, Key::P, "Previous Difference Cell"),
            (shift_command, Key::R, "Copy Cell to Right"),
            (Modifiers::COMMAND, Key::F5, "Recompare Files"),
            (Modifiers::NONE, Key::F5, "Reload Files"),
            (Modifiers::COMMAND, Key::N, "Next Difference Row"),
            (Modifiers::COMMAND, Key::P, "Previous Difference Row"),
            (Modifiers::COMMAND, Key::R, "Copy to Right"),
            (Modifiers::COMMAND, Key::G, "Go To..."),
            (Modifiers::COMMAND, Key::H, "Replace..."),
            (Modifiers::COMMAND, Key::I, "Table Compare Info"),
            (Modifiers::COMMAND, Key::O, "Open File..."),
            (Modifiers::COMMAND, Key::Q, "Exit"),
            (Modifiers::NONE, Key::F12, "Save File As..."),
            (Modifiers::NONE, Key::F7, "Align With..."),
        ];
        for (modifiers, key, id) in table {
            if ctx.input_mut(|i| i.consume_key(modifiers, key)) {
                self.command(id, ctx, prefs);
            }
        }
    }
    /// Menu bar, toolbar and toolbar dropdowns.
    pub(super) fn chrome_top(&mut self, ui: &mut egui::Ui, prefs: &Prefs) {
        use crate::chrome::{icons::Icon, menubar, toolbar};
        use toolbar::{ToolItem, ToolItemKind as Kind};
        self.update_menus();
        if let Some(action) = menubar::show(ui, &self.menus) {
            self.command(&action, ui.ctx(), prefs);
        }
        if !self.show_toolbar {
            return;
        }
        let ignore = self
            .pair
            .as_ref()
            .is_some_and(|p| p.map.rules().ignore_unimportant);
        let items: Vec<ToolItem> = [
            (Icon::Home, "Home", "Home", Kind::Normal, "0", false),
            (
                Icon::Sessions,
                "Sessions",
                "Sessions",
                Kind::Split,
                "0",
                false,
            ),
            (
                Icon::All,
                "All",
                "Show All",
                Kind::Toggle,
                "1",
                self.filter == Filter::All,
            ),
            (
                Icon::Diffs,
                "Diffs",
                "Show Differences",
                Kind::Toggle,
                "1",
                self.filter == Filter::Differences,
            ),
            (
                Icon::Same,
                "Same",
                "Show Same",
                Kind::Toggle,
                "1",
                self.filter == Filter::Same,
            ),
            (
                Icon::Minor,
                "Minor",
                "Ignore Unimportant Differences",
                Kind::Toggle,
                "2",
                ignore,
            ),
            (
                Icon::Rules,
                "Rules",
                "Session Settings...",
                Kind::Normal,
                "3",
                false,
            ),
            (Icon::Format, "Format", "Format", Kind::Split, "3", false),
            (
                Icon::Copy,
                "Copy",
                "Copy Cell to Right",
                Kind::Normal,
                "4",
                false,
            ),
            (
                Icon::Next,
                "Next Diff",
                "Next Difference Row",
                Kind::Normal,
                "5",
                false,
            ),
            (
                Icon::Prev,
                "Prev Diff",
                "Previous Difference Row",
                Kind::Normal,
                "5",
                false,
            ),
            (Icon::Swap, "Swap", "Swap Sides", Kind::Normal, "6", false),
            (
                Icon::Reload,
                "Reload",
                "Reload Files",
                Kind::Normal,
                "6",
                false,
            ),
        ]
        .into_iter()
        .map(|(icon, label, id, kind, group, selected)| ToolItem {
            label: label.into(),
            action_id: id.into(),
            kind,
            enabled: true,
            selected,
            icon,
            group: Some(group.into()),
        })
        .collect();
        let response = toolbar::show(ui, &items);
        if let Some(action) = response.clicked {
            // The Format body opens the Format page of Session Settings.
            let action = if action == "Format" {
                "Session Settings...".to_string()
            } else {
                action
            };
            self.command(&action, ui.ctx(), prefs);
        }
        if let Some(action) = response.dropdown {
            self.tool_dropdown = Some((
                action,
                ui.ctx().pointer_latest_pos().unwrap_or(ui.cursor().min),
            ));
        }
        if let Some((name, pos)) = self.tool_dropdown.clone() {
            let mut chosen = None;
            let popup = egui::Area::new(egui::Id::unique("table-toolbar-dropdown"))
                .order(egui::Order::Foreground)
                .fixed_pos(pos)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        let entries: &[&str] = if name == "Sessions" {
                            &["Session Settings...", "Table Compare Info"]
                        } else {
                            &[
                                "Compare Values",
                                "Compare Values + Formulas",
                                "Compare Values + Formulas + Number Formats",
                            ]
                        };
                        for entry in entries {
                            let checked = self.checked(entry).unwrap_or(false);
                            if ui.selectable_label(checked, *entry).clicked() {
                                chosen = Some(*entry);
                            }
                        }
                    });
                });
            if let Some(action) = chosen {
                self.command(action, ui.ctx(), prefs);
                self.tool_dropdown = None;
            } else if ui.input(|i| {
                i.key_pressed(egui::Key::Escape)
                    || (i.pointer.any_pressed()
                        && i.pointer
                            .interact_pos()
                            .is_some_and(|p| !popup.response.rect.contains(p)))
            }) {
                self.tool_dropdown = None;
            }
        }
    }
    /// Path bar (26 px) and file info row (27 px) of one side.
    pub(super) fn file_header(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        s: usize,
        prefs: &Prefs,
    ) {
        use crate::chrome::pathbar;
        if self.path_sources[s] != self.files[s].path {
            self.path_sources[s] = self.files[s].path.clone();
            self.path_edits[s] = self.files[s]
                .path
                .as_ref()
                .map(|p| {
                    std::path::absolute(p)
                        .unwrap_or_else(|_| p.clone())
                        .display()
                        .to_string()
                })
                .unwrap_or_default();
        }
        let path_rect = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), 26.0));
        ui.painter().line_segment(
            [path_rect.left_top(), path_rect.right_top()],
            egui::Stroke::new(1.0, egui::Color32::WHITE),
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .id_salt(("table-path", s))
                .max_rect(egui::Rect::from_min_max(
                    path_rect.min + egui::vec2(0.0, 1.0),
                    path_rect.max,
                )),
            |ui| {
                let response = pathbar::show_with_history(
                    ui,
                    &mut self.path_edits[s],
                    &[],
                    self.snapshot_select_paths && s == 0,
                );
                if response.submitted {
                    let path = PathBuf::from(self.path_edits[s].trim());
                    self.request(Pending::Open(s, path));
                } else {
                    match response.action.as_deref() {
                        Some("save") => {
                            if let Err(e) = self.save(s, prefs) {
                                self.message = e.to_string();
                            }
                        }
                        Some("browse" | "history") => self.file_dialog(FileMode::Open(s)),
                        _ => {}
                    }
                }
            },
        );
        let info = egui::Rect::from_min_size(
            rect.min + egui::vec2(0.0, 26.0),
            egui::vec2(rect.width(), 27.0),
        );
        let painter = ui.painter().with_clip_rect(info.intersect(ui.clip_rect()));
        let y = info.top() + 13.5;
        let font = egui::FontId::proportional(crate::chrome::theme::UI_SIZE);
        let options = &self.options[s];
        let format = if self.workbook(s) {
            "Excel Workbook"
        } else {
            match options.format.table_type {
                TableType::Fixed => "Fixed Width",
                _ => match options.format.delimiters.first() {
                    Some('\t') => "Tab Separated Values",
                    _ => "Comma Separated Values",
                },
            }
        };
        let delimiter = match options.format.table_type {
            TableType::Fixed => "Fixed",
            _ => match options.format.delimiters.first() {
                Some('\t') => "Tab",
                Some(';') => "Semicolon",
                Some('|') => "Pipe",
                Some(' ') => "Space",
                _ => "Comma",
            },
        };
        let dirty = self.pair.as_ref().is_some_and(|p| p.history[s].is_dirty())
            || self.files[s].clipboard_dirty;
        let state = match (self.files[s].readonly, dirty) {
            (true, true) => "Read-only · Modified",
            (true, false) => "Read-only",
            (false, true) => "Modified",
            _ => "",
        };
        let labels = &self.labels[s];
        let bytes = format!("{} bytes", self.files[s].byte_len);
        // Items flow left to right by their measured width (no fixed slots,
        // so long dates/sizes never overlap); format and encoding carry a
        // dropdown triangle and open Session Settings.
        let ink = crate::chrome::theme::TEXT_MUTED;
        let mut x = info.left() + 11.0;
        let mut switches = Vec::new();
        for (text, dropdown) in [
            (labels.modified.text(), false),
            (bytes.as_str(), false),
            (format, true),
            (labels.encoding.as_str(), true),
            (labels.ending, false),
            (delimiter, false),
            (state, false),
        ] {
            if text.is_empty() {
                continue;
            }
            let galley = painter.layout_no_wrap(text.to_owned(), font.clone(), ink);
            let width = galley.size().x;
            painter.galley(egui::pos2(x, y - galley.size().y / 2.0), galley, ink);
            let mut end = x + width;
            if dropdown {
                crate::chrome::icons::dropdown_triangle(
                    &painter,
                    egui::pos2(end + 9.0, y),
                    ink,
                );
                end += 16.0;
                switches.push((x - 4.0, end - x + 8.0));
            }
            x = end + 16.0;
        }
        // Format/encoding switches live on the Session Settings Format page.
        for ((x, width), tip) in switches.into_iter().zip([
            "Change format (Session Settings)",
            "Change encoding (Session Settings)",
        ]) {
            let r = egui::Rect::from_min_size(
                egui::pos2(x, info.top() + 3.0),
                egui::vec2(width, 21.0),
            );
            let response = ui
                .interact(
                    r,
                    ui.scope_id().with(("table-info", s, x as u32)),
                    egui::Sense::click(),
                )
                .on_hover_text(tip);
            if response.clicked() {
                self.settings_open();
            }
        }
    }
    /// Per-pane status strip (19 px): current row, field separators.
    pub(super) fn pane_status(&self, ui: &egui::Ui, rect: egui::Rect, s: usize) {
        let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        painter.rect_filled(rect, 0.0, crate::chrome::theme::SURFACE_SUBTLE);
        for edge in [
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.top() + 1.0)),
            egui::Rect::from_min_max(rect.min, egui::pos2(rect.left() + 1.0, rect.bottom())),
            egui::Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - 1.0), rect.max),
        ] {
            painter.rect_filled(edge, 0.0, crate::chrome::theme::LINE);
        }
        for x in [98.0, 229.0] {
            painter.rect_filled(
                egui::Rect::from_min_size(
                    rect.min + egui::vec2(x, 4.0),
                    egui::vec2(1.0, rect.height() - 8.0),
                ),
                0.0,
                crate::chrome::theme::LINE,
            );
        }
        let row = self.pair.as_ref().map(|p| {
            let row = grid::mapped(&p.map, self.grid.cursor.row);
            (if s == 0 { row.left } else { row.right }).unwrap_or(self.grid.cursor.row)
        });
        let center = rect.top() + 9.0;
        let font = egui::FontId::proportional(crate::chrome::theme::UI_SIZE);
        if let Some(row) = row {
            painter.text(
                egui::pos2(rect.left() + 3.0, center),
                egui::Align2::LEFT_CENTER,
                format!("Row {}", row + 1),
                font.clone(),
                egui::Color32::BLACK,
            );
        }
        painter.text(
            egui::pos2(rect.left() + 247.0, center),
            egui::Align2::LEFT_CENTER,
            "\u{2014}",
            font,
            egui::Color32::from_gray(0x85),
        );
    }
    /// Bottom status bar: filter legend until the user places the cursor,
    /// then difference-row count, current cell status and load time.
    pub(super) fn status_bar(&self, ui: &mut egui::Ui, rect: egui::Rect) {
        use crate::chrome::statusbar::{self, StatusField};
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let busy = self
                .worker
                .as_ref()
                .map(super::worker::Worker::progress)
                .filter(|s| !s.is_empty());
            if let Some(progress) = busy {
                statusbar::show(ui, &[StatusField::flexible(progress)]);
                return;
            }
            let Some(p) = &self.pair else {
                statusbar::show(ui, &[StatusField::flexible(self.message.clone())]);
                return;
            };
            if !self.grid.selection_visible && !self.message_is_notice() {
                let legend = match self.filter {
                    Filter::All => "Shows same, left orphan, right orphan and difference rows.",
                    Filter::Differences => "Shows left orphan, right orphan and difference rows.",
                    Filter::Same => "Shows same rows.",
                    Filter::None => "Shows no rows.",
                };
                statusbar::show(ui, &[StatusField::flexible(legend)]);
                return;
            }
            let current = if p.map.is_stale() {
                "Comparing"
            } else {
                match grid::mapped(&p.map, self.grid.cursor.row)
                    .importance
                    .projected(p.map.rules().ignore_unimportant)
                {
                    Importance::Same => "Same",
                    Importance::Unimportant => "Unimportant Difference",
                    _ => "Important Difference",
                }
            };
            let mut count = StatusField::fixed(
                format!("{} difference row(s)", p.cache.diff_rows.len()),
                293.0,
            );
            count.icon = Some(crate::chrome::icons::Icon::Diffs);
            let last = if self.message_is_notice() {
                self.message.clone()
            } else {
                format!(
                    "Load time: {} seconds",
                    (self.files[0].load_time + self.files[1].load_time).as_secs()
                )
            };
            statusbar::show(
                ui,
                &[
                    count,
                    StatusField::fixed(current, 392.0),
                    StatusField::flexible(last),
                ],
            );
        });
        // Size grip (reference: three diagonal rows of 2x2 #BFBFBF dots).
        let dot = egui::Color32::from_rgb(191, 191, 191);
        for (dx, dy) in [(4, 15), (7, 18), (4, 18), (10, 21), (7, 21), (4, 21)] {
            ui.painter().rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(rect.right() - dx as f32, rect.top() + dy as f32),
                    egui::vec2(2.0, 2.0),
                ),
                0.0,
                dot,
            );
        }
    }
    /// Messages other than routine completion notes are surfaced in the
    /// status bar's last field.
    fn message_is_notice(&self) -> bool {
        !self.message.is_empty()
            && self.message != "Comparison complete"
            && !self.message.starts_with("Row ")
    }
    pub(super) fn goto_window(&mut self, ctx: &egui::Context) {
        if !self.show_goto {
            return;
        }
        let mut open = true;
        let mut go = false;
        egui::Window::new("Go To")
            .open(&mut open)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Row:");
                    ui.add(egui::DragValue::new(&mut self.goto[0]).range(1..=usize::MAX));
                    ui.label("Column:");
                    ui.add(egui::DragValue::new(&mut self.goto[1]).range(1..=usize::MAX));
                    go = ui.button("Go").clicked();
                });
            });
        if go {
            self.go_to();
        }
        // Like the reference dialog, Go closes it.
        self.show_goto = open && !go;
    }
}
/// Encoding and line-ending labels from the first 64 KiB (never the whole
/// file); explicit session encodings win over detection.
fn sniff(
    path: Option<&std::path::Path>,
    options: &bc_engine_table::source::ReadOptions,
) -> (String, &'static str) {
    use std::io::Read;
    let mut head = Vec::new();
    if let Some(path) = path {
        if let Ok(file) = std::fs::File::open(path) {
            let _ = file.take(64 * 1024).read_to_end(&mut head);
        }
    }
    let encoding = match options.encoding {
        Some(bc_core::encoding::Encoding::Utf8) => "UTF-8".to_string(),
        Some(bc_core::encoding::Encoding::Utf16Le) => "UTF-16 LE".to_string(),
        Some(bc_core::encoding::Encoding::Utf16Be) => "UTF-16 BE".to_string(),
        Some(bc_core::encoding::Encoding::CodePage(cp)) => format!("Code page {cp}"),
        Some(other) => format!("{other:?}"),
        None if head.starts_with(&[0xEF, 0xBB, 0xBF]) => "UTF-8 BOM".to_string(),
        None if head.starts_with(&[0xFF, 0xFE]) => "UTF-16 LE".to_string(),
        None if head.starts_with(&[0xFE, 0xFF]) => "UTF-16 BE".to_string(),
        None if path.is_none() => String::new(),
        None if std::str::from_utf8(&head).is_ok() => "UTF-8".to_string(),
        None => format!("Code page {}", options.default_code_page),
    };
    let ending = if head.windows(2).any(|w| w == b"\r\n") {
        "PC"
    } else if head.contains(&b'\n') {
        "Unix"
    } else if head.contains(&b'\r') {
        "Mac"
    } else {
        ""
    };
    (encoding, ending)
}
