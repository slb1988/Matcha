//! 表格合并视图：上排三个只读输入网格（左 / base / 右），底部输出网格（可逐格编辑）。
//! 四个网格共用对齐行网格与同一个滚动位置（纵向按对齐行、横向按列）；绘制只访问可见行。
#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
use super::model::TableMergeModel;
use crate::chrome::{
    self,
    menubar::{MenuItem, MenuItemKind, MenuSpec},
    theme,
};
use crate::merge::{MergeAction, MergeLaunch};
use bc_core::Side;
use bc_engine_table::merge::{CellChoice, CellSource, RowKind, SheetMerge};
use bc_session::prefs::Prefs;
use eframe::egui;
use std::path::PathBuf;

/// 输出网格的窗格号；0..3 为 left、center、right。
pub(crate) const OUT: usize = 3;
const SIDES: [Side; 3] = [Side::Left, Side::Center, Side::Right];
const BADGES: [&str; 4] = ["LEFT", "BASE", "RIGHT", "OUTPUT"];
/// 输入窗格（left、center、right）在上排的列位：BASE | LEFT | RIGHT。
const COLUMN_OF: [usize; 3] = [1, 0, 2];
pub(crate) const GUTTER: f32 = 46.0;
/// 窗格标题条。
pub(crate) const HEADER: f32 = 24.0;
/// 列名行。
pub(crate) const COLUMNS: f32 = 20.0;
pub(crate) const COLUMN_WIDTH: f32 = 110.0;
pub(crate) const ROW_HEIGHT: f32 = 20.0;
const SHEET_TABS: f32 = 32.0;
/// 选中格详情条的高度（状态条之上）。
const DETAILS: f32 = 62.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pending {
    Close,
    Exit,
}

enum Prompt {
    Unsaved(Pending),
    Conflicts {
        then: Option<Pending>,
        path: Option<PathBuf>,
    },
    SaveAs {
        path: String,
        then: Option<Pending>,
        confirmed: bool,
    },
}

/// 正在编辑的输出格。
struct Editing {
    row: usize,
    column: usize,
    text: String,
    focused: bool,
}

pub struct TableMergeView {
    pub model: TableMergeModel,
    /// 活动窗格（0..=3）。
    active: usize,
    /// 选中的格（对齐行, 列）。
    pub(crate) selected: (usize, usize),
    /// 纵向滚动位置（四窗格共用，行保持对齐）。
    scroll_y: f32,
    /// 横向滚动位置：每窗格独立（列对齐靠内容，宽度不同各滚各的）。
    scroll_x: [f32; 4],
    /// 每窗格的最大横向滚动量（>0 时该窗格显示横向滚动条）。
    max_x: [f32; 4],
    pending_scroll: Option<usize>,
    pub(crate) pane_rects: [egui::Rect; 4],
    editing: Option<Editing>,
    prompt: Option<Prompt>,
    action: Option<MergeAction>,
    pub message: String,
    menus: Vec<MenuSpec>,
    /// 标题条路径框持焦（上一帧）：持焦时无修饰键留给文本编辑。
    path_edit_focused: bool,
    /// 打开 / 切 sheet 时布局未就绪，布局完成后补做一次选中格可见保证。
    pending_ensure: bool,
    /// 详情条文本框持焦（上一帧）：持焦时无修饰键留给文本编辑。
    details_focused: bool,
}

fn menu_item(label: &str, shortcut: Option<&str>) -> MenuItem {
    MenuItem {
        label: label.into(),
        mnemonic: None,
        shortcut: shortcut.map(str::to_owned),
        kind: if label.is_empty() {
            MenuItemKind::Separator
        } else {
            MenuItemKind::Action
        },
        enabled: true,
        action_id: label.into(),
    }
}

fn menus() -> Vec<MenuSpec> {
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
                menu_item("Home", None),
                menu_item("New Tab", Some("Ctrl+T")),
                menu_item("", None),
                menu_item("Close Tab", Some("Ctrl+W")),
                menu_item("Exit", Some("Ctrl+Q")),
            ],
        ),
        menu(
            "File",
            'f',
            vec![
                menu_item("Save Output", Some("Ctrl+S")),
                menu_item("Save Output As...", Some("F12")),
            ],
        ),
        menu(
            "Edit",
            'e',
            vec![
                menu_item("Undo", Some("Ctrl+Z")),
                menu_item("Redo", Some("Ctrl+Y")),
                menu_item("", None),
                menu_item("Take Left", Some("Ctrl+L")),
                menu_item("Take Center", Some("Ctrl+B")),
                menu_item("Take Right", Some("Ctrl+R")),
                menu_item("Accept Current Content", Some("Ctrl+K")),
                menu_item("", None),
                menu_item("Take Left (Cell)", Some("Ctrl+Shift+L")),
                menu_item("Take Center (Cell)", Some("Ctrl+Shift+B")),
                menu_item("Take Right (Cell)", Some("Ctrl+Shift+R")),
                menu_item("Edit Output Cell", Some("F2")),
            ],
        ),
        menu(
            "Search",
            'r',
            vec![
                menu_item("Next Change Section", Some("Ctrl+N")),
                menu_item("Previous Change Section", Some("Ctrl+P")),
            ],
        ),
    ]
}

fn side_name(side: Side) -> &'static str {
    match side {
        Side::Left => "left",
        Side::Right => "right",
        _ => "center",
    }
}

impl TableMergeView {
    pub fn open(launch: MergeLaunch) -> Result<Self, String> {
        Ok(Self::from_model(TableMergeModel::load(&launch)?))
    }

    pub fn from_model(model: TableMergeModel) -> Self {
        let mut view = Self {
            model,
            active: OUT,
            selected: (0, 0),
            scroll_y: 0.0,
            scroll_x: [0.0; 4],
            max_x: [0.0; 4],
            pending_scroll: None,
            pane_rects: [egui::Rect::NOTHING; 4],
            editing: None,
            prompt: None,
            action: None,
            message: String::new(),
            menus: menus(),
            path_edit_focused: false,
            pending_ensure: false,
            details_focused: false,
        };
        // 打开即定位到第一个冲突（没有冲突则第一个改动）。
        if let Some(row) = view.model.first_conflict_or_change() {
            view.goto_change(row);
        }
        view
    }

    /// 快照场景：`table-merge-conflict` 把第一个冲突取右侧并停在下一个冲突，其余停在开头。
    pub(crate) fn snapshot_preset(&mut self, scene: &str) {
        if scene == "table-merge-conflict" {
            self.command("Take Right");
            self.command("Next Change Section");
        } else {
            self.goto_row(0);
        }
        self.message.clear();
    }

    /// left、right 两个输入的路径（标签页标题用）。
    pub fn paths(&self) -> [Option<PathBuf>; 2] {
        [
            Some(self.model.launch.spec.left.clone()),
            Some(self.model.launch.spec.right.clone()),
        ]
    }
    pub fn is_dirty(&self) -> bool {
        self.model.is_dirty()
    }
    pub fn prompt_open(&self) -> bool {
        self.prompt.is_some()
    }
    pub fn request_close(&mut self) {
        self.request(Pending::Close);
    }
    pub fn request_exit(&mut self) {
        self.request(Pending::Exit);
    }
    /// 关闭时回给等待方的结果码（0 / 14 / 101）。
    pub fn result_code(&self) -> i32 {
        self.model.result_code()
    }
    pub fn unresolved(&self) -> usize {
        self.model.unresolved()
    }

    fn request(&mut self, pending: Pending) {
        if self.model.is_dirty() {
            self.prompt = Some(Prompt::Unsaved(pending));
        } else {
            self.finish(pending);
        }
    }
    fn finish(&mut self, pending: Pending) {
        self.action = Some(match pending {
            Pending::Close => MergeAction::Close,
            Pending::Exit => MergeAction::Exit,
        });
    }

    fn save(&mut self, then: Option<Pending>) {
        if self.model.unresolved() > 0 {
            self.prompt = Some(Prompt::Conflicts { then, path: None });
        } else {
            self.save_now(then);
        }
    }
    fn save_now(&mut self, then: Option<Pending>) {
        match self.model.target.clone() {
            Some(path) => {
                self.save_to(path, then);
            }
            None => {
                self.prompt = Some(Prompt::SaveAs {
                    path: String::new(),
                    then,
                    confirmed: true,
                })
            }
        }
    }
    fn save_to(&mut self, path: PathBuf, then: Option<Pending>) -> bool {
        match self.model.save(&path) {
            Ok(()) => {
                self.message = format!("Saved {}", path.display());
                for warning in &self.model.save_warnings {
                    self.message.push_str(&format!(" (warning: {warning})"));
                }
                if let Some(pending) = then {
                    self.finish(pending);
                }
                true
            }
            Err(e) => {
                self.message = e;
                false
            }
        }
    }
    fn save_as(&mut self, path: String, then: Option<Pending>, confirmed: bool) {
        let target = PathBuf::from(path.trim());
        if !confirmed && self.model.unresolved() > 0 {
            self.prompt = Some(Prompt::Conflicts {
                then,
                path: Some(target),
            });
        } else if !self.save_to(target, then) {
            self.prompt = Some(Prompt::SaveAs {
                path,
                then,
                confirmed,
            });
        }
    }

    fn sheet(&self) -> &SheetMerge {
        self.model.current()
    }

    fn goto_row(&mut self, row: usize) {
        let rows = self.sheet().row_count();
        let row = row.min(rows.saturating_sub(1));
        self.selected.0 = row;
        self.pending_scroll = Some(row.saturating_sub(3));
    }

    fn switch_sheet(&mut self, sheet: usize) {
        if sheet != self.model.sheet {
            self.editing = None;
            self.model.set_sheet(sheet);
            self.selected = (0, 0);
            self.scroll_y = 0.0;
            self.scroll_x = [0.0; 4];
            if let Some(row) = self.model.first_conflict_or_change() {
                self.goto_change(row);
            }
        }
    }

    /// 选中行所在的冲突区段。
    fn section(&mut self) -> Option<std::ops::Range<usize>> {
        self.model.section_at(self.selected.0)
    }

    pub(crate) fn command(&mut self, id: &str) {
        self.message.clear();
        let (row, column) = self.selected;
        let rows = self.sheet().row_count();
        match id {
            "Home" => self.action = Some(MergeAction::Home),
            "New Tab" => self.action = Some(MergeAction::NewTab),
            "Close Tab" => self.request_close(),
            "Exit" => self.request_exit(),
            "Save Output" => self.save(None),
            "Save Output As..." => {
                self.prompt = Some(Prompt::SaveAs {
                    path: self
                        .model
                        .target
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default(),
                    then: None,
                    confirmed: false,
                })
            }
            "Undo" => {
                if self.model.undo().is_none() {
                    self.message = "Nothing to undo".into();
                }
            }
            "Redo" => {
                if self.model.redo().is_none() {
                    self.message = "Nothing to redo".into();
                }
            }
            "Take Left" | "Take Center" | "Take Right" => {
                let side = match id {
                    "Take Left" => Side::Left,
                    "Take Center" => Side::Center,
                    _ => Side::Right,
                };
                if let Some(rows) = self.section() {
                    self.model.take(rows, side);
                } else if row < rows {
                    self.model.take_cell(row, column, side);
                }
            }
            "Take Left (Cell)" | "Take Center (Cell)" | "Take Right (Cell)" => {
                let side = match id {
                    "Take Left (Cell)" => Side::Left,
                    "Take Center (Cell)" => Side::Center,
                    _ => Side::Right,
                };
                if row < rows && self.sheet().rows[row].index(side).is_some() {
                    self.model.take_cell(row, column, side);
                } else {
                    self.message = format!("The {} side has no row here", side_name(side));
                }
            }
            "Accept Current Content" => match self.section() {
                Some(rows) => {
                    let from = rows.start;
                    self.model.accept(rows);
                    if let Some(next) = self.model.next_conflict(from, true) {
                        self.goto_change(next);
                    }
                }
                None => self.message = "The selection is not on a conflict".into(),
            },
            "Next Change Section" | "Previous Change Section" => {
                let forward = id == "Next Change Section";
                if let Some(row) = self.model.next_change(row, forward) {
                    self.goto_change(row);
                } else if let Some(sheet) = self.model.next_change_sheet(forward) {
                    self.switch_sheet(sheet);
                    let target = if forward {
                        self.model.changes().first()
                    } else {
                        self.model.changes().last()
                    }
                    .map(|r| r.start);
                    if let Some(row) = target {
                        self.goto_change(row);
                    }
                } else {
                    self.message = if forward {
                        "No more changes below".into()
                    } else {
                        "No more changes above".into()
                    }
                }
            }
            "Edit Output Cell" => self.begin_edit(),
            _ => {}
        }
    }

    fn begin_edit(&mut self) {
        let (row, column) = self.selected;
        if row >= self.sheet().row_count() || column >= self.sheet().columns {
            return;
        }
        match self.sheet().output_cell(row, column) {
            Some((_, text, _)) => {
                self.active = OUT;
                self.editing = Some(Editing {
                    row,
                    column,
                    text,
                    focused: false,
                });
            }
            None => self.message = "This row is not in the output; take a side first".into(),
        }
    }

    fn commit_edit(&mut self) {
        if let Some(edit) = self.editing.take() {
            let unchanged = self
                .sheet()
                .output_cell(edit.row, edit.column)
                .is_some_and(|(_, text, source)| text == edit.text && source == CellSource::Edited);
            if !unchanged && !self.model.edit(edit.row, edit.column, &edit.text) {
                self.message = "This row is not in the output".into();
            }
        }
    }

    fn keyboard(&mut self, ctx: &egui::Context) {
        if self.editing.is_some() {
            return;
        }
        for event in ctx.input(|i| i.events.clone()) {
            let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = &event
            else {
                continue;
            };
            // 路径框 / 详情条持焦时，无修饰键（方向/Delete/F2/Enter）留给文本编辑。
            if (self.path_edit_focused || self.details_focused)
                && !modifiers.command
                && !modifiers.alt
            {
                continue;
            }
            let command = match (*key, modifiers.command, modifiers.shift) {
                (egui::Key::S, true, _) => Some("Save Output"),
                (egui::Key::F12, _, _) => Some("Save Output As..."),
                (egui::Key::Z, true, false) => Some("Undo"),
                (egui::Key::Z, true, true) | (egui::Key::Y, true, _) => Some("Redo"),
                (egui::Key::L, true, false) => Some("Take Left"),
                (egui::Key::B, true, false) => Some("Take Center"),
                (egui::Key::R, true, false) => Some("Take Right"),
                (egui::Key::L, true, true) => Some("Take Left (Cell)"),
                (egui::Key::B, true, true) => Some("Take Center (Cell)"),
                (egui::Key::R, true, true) => Some("Take Right (Cell)"),
                (egui::Key::K, true, _) => Some("Accept Current Content"),
                (egui::Key::N, true, _) => Some("Next Change Section"),
                (egui::Key::P, true, _) => Some("Previous Change Section"),
                (egui::Key::T, true, _) => Some("New Tab"),
                (egui::Key::W, true, _) => Some("Close Tab"),
                (egui::Key::Q, true, _) => Some("Exit"),
                (egui::Key::F2, false, _) => Some("Edit Output Cell"),
                (egui::Key::Enter, false, _) if self.active == OUT => Some("Edit Output Cell"),
                _ => None,
            };
            if let Some(command) = command {
                self.command(command);
                continue;
            }
            if modifiers.command || modifiers.alt {
                continue;
            }
            let (rows, columns) = (self.sheet().row_count(), self.sheet().columns);
            let (row, column) = &mut self.selected;
            match key {
                egui::Key::ArrowUp => *row = row.saturating_sub(1),
                egui::Key::ArrowDown => *row = (*row + 1).min(rows.saturating_sub(1)),
                egui::Key::ArrowLeft => *column = column.saturating_sub(1),
                egui::Key::ArrowRight => *column = (*column + 1).min(columns.saturating_sub(1)),
                egui::Key::Delete if self.active == OUT => {
                    let (r, c) = self.selected;
                    if r < rows && c < columns && !self.model.edit(r, c, "") {
                        self.message = "This row is not in the output".into();
                    }
                    continue;
                }
                _ => continue,
            }
            self.ensure_visible();
        }
    }

    fn ensure_visible(&mut self) {
        let row = self.selected.0;
        let hbar = if self.max_x[OUT] > 0.0 { 16.0 } else { 0.0 };
        let shown = ((self.pane_rects[OUT].height() - HEADER - COLUMNS - hbar) / ROW_HEIGHT)
            .max(1.0) as usize;
        let top = (self.scroll_y / ROW_HEIGHT) as usize;
        if row < top {
            self.pending_scroll = Some(row);
        } else if row + 1 >= top + shown {
            self.pending_scroll = Some(row + 2 - shown.min(row + 2));
        }
        // 水平：每窗格按自己的宽度调整，选中格在四窗格都可见（布局未就绪时跳过）。
        let left = self.selected.1 as f32 * COLUMN_WIDTH;
        let right = left + COLUMN_WIDTH;
        for (pane, rect) in self.pane_rects.iter().enumerate() {
            let view = rect.width() - GUTTER;
            if !view.is_finite() || view <= 0.0 {
                continue;
            }
            let x = &mut self.scroll_x[pane];
            if left < *x {
                *x = left;
            } else if right > *x + view {
                *x = right - view;
            }
        }
    }

    /// `row` 里第一个改动格的列（普通行；结构行或没有改动格时保持当前列）。
    fn first_changed_column(&self, row: usize) -> usize {
        let sheet = self.sheet();
        if row >= sheet.row_count() || sheet.rows[row].kind != RowKind::Normal {
            return self.selected.1;
        }
        (0..sheet.columns)
            .find(|&c| sheet.choice(row, c) != CellChoice::Center)
            .unwrap_or(self.selected.1)
    }

    /// 跳到改动区段的首行，并把选中格落在该行的第一个改动格上（列随格可见）。
    fn goto_change(&mut self, row: usize) {
        self.goto_row(row);
        self.selected.1 = self.first_changed_column(self.selected.0);
        self.ensure_visible();
        self.pending_ensure = true;
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, _prefs: &mut Prefs) -> Option<MergeAction> {
        let ctx = ui.ctx().clone();
        let blocked = self.prompt.is_some();
        if !blocked {
            self.keyboard(&ctx);
        }
        if blocked {
            ui.disable();
        }
        ui.spacing_mut().item_spacing.y = 0.0;
        self.toolbar(ui);
        if self.model.sheets().len() > 1 {
            self.sheet_tabs(ui);
        }
        let rows = self.sheet().row_count();
        let columns = self.sheet().columns;
        let available = ui.available_rect_before_wrap();
        let status = egui::Rect::from_min_max(
            egui::pos2(available.left(), available.bottom() - 25.0),
            available.max,
        );
        // 选中格详情条：状态条之上；网格区在它的上面。
        let details = egui::Rect::from_min_max(
            egui::pos2(available.left(), status.top() - DETAILS),
            egui::pos2(available.right() - 16.0, status.top()),
        );
        let body = egui::Rect::from_min_max(
            available.min,
            egui::pos2(available.right() - 16.0, details.top()),
        );
        let top_height = ((body.height() - HEADER) * 0.55).max(80.0);
        let inputs = egui::Rect::from_min_size(body.min, egui::vec2(body.width(), top_height));
        let output = egui::Rect::from_min_max(egui::pos2(body.left(), inputs.bottom()), body.max);
        let width = (inputs.width() - 2.0) / 3.0;
        for pane in 0..3 {
            self.pane_rects[pane] = egui::Rect::from_min_size(
                inputs.min + egui::vec2(COLUMN_OF[pane] as f32 * (width + 1.0), 0.0),
                egui::vec2(width, inputs.height()),
            );
        }
        self.pane_rects[OUT] = output;
        self.path_edit_focused = false;
        self.details_focused = false;
        // 每窗格的横向滚动范围（>0 显示横向滚动条）。
        for pane in 0..4 {
            self.max_x[pane] =
                (columns as f32 * COLUMN_WIDTH + GUTTER - self.pane_rects[pane].width() + 20.0)
                    .max(0.0);
        }
        if self.pending_ensure {
            self.pending_ensure = false;
            self.ensure_visible();
        }

        let shortest = self
            .pane_rects
            .iter()
            .enumerate()
            .map(|(pane, r)| {
                r.height()
                    - HEADER
                    - COLUMNS
                    - if self.max_x[pane] > 0.0 { 16.0 } else { 0.0 }
            })
            .fold(f32::INFINITY, f32::min)
            .max(ROW_HEIGHT);
        let max_y = (rows as f32 * ROW_HEIGHT - shortest + ROW_HEIGHT).max(0.0);
        if let Some(row) = self.pending_scroll.take() {
            self.scroll_y = row as f32 * ROW_HEIGHT;
        }
        if !blocked {
            let delta = chrome::wheel::delta(&ctx, &self.pane_rects);
            // 纵向四窗格共用；横向只滚指针下的那个窗格。
            self.scroll_y -= delta.y;
            if delta.x != 0.0 {
                if let Some(pointer) = ctx.input(|i| i.pointer.hover_pos()) {
                    if let Some(pane) = self.pane_rects.iter().position(|r| r.contains(pointer))
                    {
                        self.scroll_x[pane] -= delta.x;
                    }
                }
            }
        }
        self.scroll_y = self.scroll_y.clamp(0.0, max_y);
        for pane in 0..4 {
            self.scroll_x[pane] = self.scroll_x[pane].clamp(0.0, self.max_x[pane]);
        }

        for pane in 0..4 {
            let rect = self.pane_rects[pane];
            ui.scope_builder(
                egui::UiBuilder::new()
                    .id_salt(("table-merge-pane", pane))
                    .max_rect(rect),
                |ui| {
                    ui.set_clip_rect(rect.intersect(ui.clip_rect()));
                    self.pane(ui, pane, rect);
                },
            );
        }
        for x in [
            self.pane_rects[0].left() - 1.0,
            self.pane_rects[2].left() - 1.0,
        ] {
            ui.painter().line_segment(
                [egui::pos2(x, inputs.top()), egui::pos2(x, inputs.bottom())],
                egui::Stroke::new(1.0, theme::LINE_STRONG),
            );
        }
        self.edit_box(ui);

        let bar =
            egui::Rect::from_min_max(egui::pos2(body.right(), body.top()), body.right_bottom());
        let last = (max_y / ROW_HEIGHT).ceil() as usize;
        let mut top = (self.scroll_y / ROW_HEIGHT).round() as usize;
        ui.scope_builder(egui::UiBuilder::new().max_rect(bar), |ui| {
            if chrome::scrollbar::show(ui, 0..=last, &mut top).changed() {
                self.scroll_y = top as f32 * ROW_HEIGHT;
            }
        });
        self.details(ui, details);

        ui.scope_builder(egui::UiBuilder::new().max_rect(status), |ui| {
            use chrome::statusbar::{self, StatusField};
            let mut count = StatusField::fixed(
                format!("{} conflict section(s)", self.model.unresolved()),
                200.0,
            );
            count.icon = Some(chrome::icons::Icon::Diffs);
            ui.spacing_mut().item_spacing.x = 0.0;
            statusbar::show(
                ui,
                &[
                    count,
                    StatusField::fixed(format!("Keys: {}", self.sheet().keys.label()), 200.0),
                    StatusField::fixed(self.describe_cell(), 240.0),
                    StatusField::flexible(self.message.clone()),
                ],
            );
        });
        ui.allocate_rect(available, egui::Sense::hover());
        self.prompts(&ctx);
        self.action.take()
    }

    pub(crate) fn describe_cell(&self) -> String {
        let sheet = self.sheet();
        let (row, column) = self.selected;
        let Some(merge_row) = sheet.rows.get(row) else {
            return String::new();
        };
        let kind = match merge_row.kind {
            RowKind::Normal => match sheet.choice(row, column) {
                CellChoice::Center => "Unchanged",
                CellChoice::Left => "Left Change",
                CellChoice::Right => "Right Change",
                CellChoice::Both => "Same Change",
                CellChoice::Conflict => "Different Change",
            },
            RowKind::LeftAdded => "Added on Left",
            RowKind::RightAdded => "Added on Right",
            RowKind::BothAdded => "Added on Both Sides",
            RowKind::AddConflict => "Added Differently on Both Sides",
            RowKind::Deleted => "Deleted",
            RowKind::DeleteConflict => "Deleted vs Modified",
        };
        let place = format!("R{} C{}", row + 1, column + 1);
        if sheet.row_unresolved(row) {
            format!("{place}  Conflict: {kind}")
        } else {
            format!("{place}  {kind}")
        }
    }

    /// 详情条显示的文本：活动窗格选中格的完整内容。
    pub(crate) fn details_text(&self) -> String {
        let (row, column) = self.selected;
        let sheet = self.sheet();
        if self.active < 3 {
            sheet.input_text(SIDES[self.active], row, column)
        } else {
            sheet.output_cell(row, column).map(|(_, text, _)| text)
        }
        .unwrap_or_default()
    }

    /// 选中格详情条：活动窗格徽标 + 位置与类型 + 完整内容
    /// （换行显示、内部滚动、可选中复制）。
    fn details(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, 0.0, theme::PANEL_BG);
        painter.line_segment(
            [rect.left_top(), rect.right_top()],
            egui::Stroke::new(1.0, theme::BORDER),
        );
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(8.0, 4.0))),
            |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let (fill, ink) = match self.active {
                    0 => (theme::MERGE_LEFT_SOFT, theme::MERGE_LEFT),
                    2 => (theme::MERGE_RIGHT_SOFT, theme::MERGE_RIGHT),
                    _ => (theme::SURFACE, theme::TEXT_MUTED),
                };
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(BADGES[self.active])
                            .color(ink)
                            .background_color(fill)
                            .small(),
                    );
                    ui.label(
                        egui::RichText::new(self.describe_cell()).color(theme::TEXT_MUTED),
                    );
                });
                let text = self.details_text();
                let edit = ui.add_sized(
                    ui.available_size(),
                    egui::TextEdit::multiline(&mut text.as_str())
                        .id(ui.make_persistent_id("table-merge-details"))
                        .frame(egui::Frame::NONE)
                        .font(egui::FontId::proportional(theme::UI_SIZE - 1.0))
                        .text_color(theme::TEXT)
                        .hint_text("(empty cell)"),
                );
                self.details_focused |= edit.has_focus();
            },
        );
    }

    fn sheet_tabs(&mut self, ui: &mut egui::Ui) {
        let mut clicked = None;
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), SHEET_TABS),
            egui::Sense::hover(),
        );
        ui.painter().rect_filled(rect, 0.0, theme::PANEL_BG);
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(6.0, 2.0))),
            |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    for (i, sheet) in self.model.sheets().iter().enumerate() {
                        let mut label = sheet.name.clone();
                        let count = self.model.unresolved_in(i);
                        if count > 0 {
                            label.push_str(&format!(" ({count})"));
                        }
                        if sheet.is_dropped() {
                            label.push_str(" — not in output");
                        } else if let Some(by) = sheet.sheet_deleted_by() {
                            label.push_str(&format!(" — deleted on {}", side_name(by)));
                        }
                        let text = if count > 0 {
                            egui::RichText::new(label).color(theme::MERGE_CONFLICT)
                        } else if sheet.is_dropped() {
                            egui::RichText::new(label).strikethrough()
                        } else {
                            egui::RichText::new(label)
                        };
                        if ui
                            .add(chrome::widgets::Button::new(text).selected(i == self.model.sheet))
                            .clicked()
                        {
                            clicked = Some(i);
                        }
                    }
                });
            },
        );
        if let Some(i) = clicked {
            self.switch_sheet(i);
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        use chrome::{icons::Icon, menubar, toolbar};
        use toolbar::{ToolItem, ToolItemKind as Kind};
        let undo = self.model.can_undo();
        let redo = self.model.can_redo();
        for item in self.menus.iter_mut().flat_map(|m| m.items.iter_mut()) {
            item.enabled = match item.action_id.as_str() {
                "Undo" => undo,
                "Redo" => redo,
                _ => true,
            };
        }
        if let Some(action) = menubar::show(ui, &self.menus) {
            self.command(&action);
        }
        let has_rows = self.sheet().row_count() > 0;
        let on_section = self.model.section_at(self.selected.0).is_some();
        let changes = self.model.has_changes();
        let items: Vec<ToolItem> = [
            (Icon::Home, "Home", "Home", "home", true),
            (Icon::Save, "Save", "Save Output", "file", true),
            (Icon::ArrowLeft, "Left", "Take Left", "take", has_rows),
            (Icon::Select, "Center", "Take Center", "take", has_rows),
            (Icon::ArrowRight, "Right", "Take Right", "take", has_rows),
            (
                Icon::Status,
                "Accept",
                "Accept Current Content",
                "take",
                on_section,
            ),
            (
                Icon::Next,
                "Next Change",
                "Next Change Section",
                "nav",
                changes,
            ),
            (
                Icon::Prev,
                "Prev Change",
                "Previous Change Section",
                "nav",
                changes,
            ),
        ]
        .into_iter()
        .map(|(icon, label, action, group, enabled)| ToolItem {
            label: label.into(),
            action_id: action.into(),
            kind: Kind::Normal,
            enabled,
            selected: false,
            icon,
            group: Some(group.into()),
        })
        .collect();
        if let Some(action) = toolbar::show(ui, &items).clicked {
            self.command(&action);
        }
    }

    fn header(&mut self, ui: &mut egui::Ui, pane: usize, rect: egui::Rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, 0.0, theme::HEADER_FILL);
        painter.line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            egui::Stroke::new(1.0, theme::BORDER),
        );
        let (fill, ink) = match pane {
            0 => (theme::MERGE_LEFT_SOFT, theme::MERGE_LEFT),
            2 => (theme::MERGE_RIGHT_SOFT, theme::MERGE_RIGHT),
            _ => (theme::SURFACE, theme::TEXT_MUTED),
        };
        let badge =
            painter.layout_no_wrap(BADGES[pane].into(), egui::FontId::proportional(9.5), ink);
        let chip = egui::Rect::from_min_size(
            rect.left_center() + egui::vec2(6.0, -8.0),
            egui::vec2(badge.size().x + 10.0, 16.0),
        );
        painter.rect_filled(chip, theme::RADIUS_SM, fill);
        painter.galley(chip.min + egui::vec2(5.0, 2.0), badge, ink);
        let text_rect = egui::Rect::from_min_max(
            egui::pos2(chip.right() + 8.0, rect.top()),
            egui::pos2(rect.right() - 4.0, rect.bottom()),
        );
        if pane == OUT {
            // 输出不显示路径（保存目标即 LEFT 的文件），只显示状态。
            let mut status = Vec::new();
            if self.sheet().is_dropped() {
                status.push("sheet not in output");
            }
            if self.model.is_dirty() {
                status.push("not saved");
            }
            ui.painter().text(
                text_rect.left_center(),
                egui::Align2::LEFT_CENTER,
                status.join("  •  "),
                egui::FontId::proportional(12.0),
                theme::TEXT_MUTED,
            );
            return;
        }
        let spec = &self.model.launch.spec;
        let path = match pane {
            0 => &spec.left,
            1 => &spec.center,
            _ => &spec.right,
        };
        let text = self.model.launch.titles[pane]
            .clone()
            .unwrap_or_else(|| path.display().to_string());
        // 只读路径框：可选中/复制，右键 Copy / Select All。
        let edit = chrome::pathbar::show_readonly(ui, text_rect, &text);
        self.path_edit_focused |= edit.has_focus();
    }

    /// 输入格的底色：按与 base 的差异与冲突状态。
    fn input_fill(sheet: &SheetMerge, side: Side, row: usize, column: usize) -> egui::Color32 {
        let merge_row = &sheet.rows[row];
        let soft = |side: Side| match side {
            Side::Left => theme::MERGE_LEFT_SOFT,
            Side::Right => theme::MERGE_RIGHT_SOFT,
            _ => theme::SURFACE,
        };
        match merge_row.kind {
            RowKind::AddConflict | RowKind::DeleteConflict if sheet.row_unresolved(row) => {
                theme::MERGE_CONFLICT_SOFT
            }
            RowKind::LeftAdded
            | RowKind::RightAdded
            | RowKind::BothAdded
            | RowKind::AddConflict => soft(side),
            RowKind::Deleted => theme::PANEL_BG,
            RowKind::DeleteConflict => match (side, sheet.choice(row, column)) {
                (Side::Left, CellChoice::Left) | (Side::Right, CellChoice::Right) => soft(side),
                _ => theme::SURFACE,
            },
            RowKind::Normal => match (side, sheet.choice(row, column)) {
                (Side::Center, _) => theme::SURFACE,
                (_, CellChoice::Conflict) if sheet.cell_unresolved(row, column) => {
                    theme::MERGE_CONFLICT_SOFT
                }
                (_, CellChoice::Conflict) => soft(side),
                (Side::Left, CellChoice::Left | CellChoice::Both)
                | (Side::Right, CellChoice::Right | CellChoice::Both) => soft(side),
                _ => theme::SURFACE,
            },
        }
    }

    /// 输出格的底色：按来源；未解决冲突为冲突色。
    fn output_fill(
        sheet: &SheetMerge,
        row: usize,
        column: usize,
        source: CellSource,
    ) -> egui::Color32 {
        if sheet.rows[row].kind.is_structural_conflict() && sheet.row_unresolved(row) {
            return theme::MERGE_CONFLICT_SOFT;
        }
        if sheet.cell_unresolved(row, column) {
            return theme::MERGE_CONFLICT_SOFT;
        }
        match source {
            CellSource::Edited => theme::MERGE_MANUAL_SOFT,
            CellSource::Left => theme::MERGE_LEFT_SOFT,
            CellSource::Right => theme::MERGE_RIGHT_SOFT,
            CellSource::Center => theme::SURFACE,
        }
    }

    /// 一个窗格：标题条 + 列名行 + 可见的网格行 + （超宽时）横向滚动条。
    fn pane(&mut self, ui: &mut egui::Ui, pane: usize, rect: egui::Rect) {
        let header = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), HEADER));
        self.header(ui, pane, header);
        let names =
            egui::Rect::from_min_size(header.left_bottom(), egui::vec2(rect.width(), COLUMNS));
        let hbar_height = if self.max_x[pane] > 0.0 { 16.0 } else { 0.0 };
        let hbar = egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.bottom() - hbar_height),
            rect.max,
        );
        let area =
            egui::Rect::from_min_max(names.left_bottom(), egui::pos2(rect.right(), hbar.top()));
        ui.painter().rect_filled(area, 0.0, theme::CANVAS);
        let clip = ui.clip_rect();
        let painter = ui.painter().with_clip_rect(area.intersect(clip));
        let columns = self.sheet().columns;
        let scroll_x = self.scroll_x[pane];
        let x0 = rect.left() + GUTTER - scroll_x;
        let first_column = (scroll_x / COLUMN_WIDTH) as usize;
        let shown_columns = ((rect.width() - GUTTER) / COLUMN_WIDTH).ceil() as usize + 1;
        let visible_columns = first_column..(first_column + shown_columns).min(columns);
        let font = egui::FontId::proportional(theme::UI_SIZE - 1.0);

        // 列名行。
        {
            let names_painter = ui.painter().with_clip_rect(names.intersect(clip));
            names_painter.rect_filled(names, 0.0, theme::HEADER_FILL);
            let headers = self.sheet().headers.clone();
            for column in visible_columns.clone() {
                let cell = egui::Rect::from_min_size(
                    egui::pos2(x0 + column as f32 * COLUMN_WIDTH, names.top()),
                    egui::vec2(COLUMN_WIDTH, COLUMNS),
                );
                let name = headers
                    .as_ref()
                    .and_then(|h| h.get(column))
                    .filter(|n| !n.is_empty())
                    .cloned()
                    .unwrap_or_else(|| (column + 1).to_string());
                let key = self.sheet().keys.columns.contains(&column)
                    && self.sheet().keys.source != bc_engine_table::merge::KeySource::AllColumns;
                names_painter
                    .with_clip_rect(cell.intersect(names).intersect(clip))
                    .text(
                        cell.left_center() + egui::vec2(4.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        if key { format!("◆ {name}") } else { name },
                        font.clone(),
                        theme::TEXT_MUTED,
                    );
                names_painter.line_segment(
                    [cell.right_top(), cell.right_bottom()],
                    egui::Stroke::new(1.0, theme::BORDER),
                );
            }
            names_painter.rect_filled(
                egui::Rect::from_min_size(names.min, egui::vec2(GUTTER, COLUMNS)),
                0.0,
                theme::HEADER_FILL,
            );
            names_painter.line_segment(
                [names.left_bottom(), names.right_bottom()],
                egui::Stroke::new(1.0, theme::BORDER),
            );
        }

        let id = egui::Id::unique(("table-merge-grid", pane));
        let response = ui.interact(area, id, egui::Sense::click());
        let first = (self.scroll_y / ROW_HEIGHT) as usize;
        let offset = self.scroll_y - first as f32 * ROW_HEIGHT;
        let shown = (area.height() / ROW_HEIGHT).ceil() as usize + 1;
        let rows = self.sheet().row_count();
        let side = (pane < 3).then(|| SIDES[pane]);
        for row in first..(first + shown).min(rows) {
            let top = area.top() + (row - first) as f32 * ROW_HEIGHT - offset;
            let line = egui::Rect::from_min_size(
                egui::pos2(area.left(), top),
                egui::vec2(area.width(), ROW_HEIGHT),
            );
            let sheet = self.model.current();
            let present = match side {
                Some(side) => sheet.rows[row].index(side),
                None => sheet.row_base(row).map(|_| row),
            };
            if present.is_none() {
                painter.rect_filled(line, 0.0, theme::SURFACE);
                crate::text::render::hatch(ui, line.intersect(area), false);
            } else {
                for column in visible_columns.clone() {
                    let cell = egui::Rect::from_min_size(
                        egui::pos2(x0 + column as f32 * COLUMN_WIDTH, top),
                        egui::vec2(COLUMN_WIDTH, ROW_HEIGHT),
                    );
                    let (text, fill) = match side {
                        Some(side) => (
                            sheet.input_text(side, row, column).unwrap_or_default(),
                            Self::input_fill(sheet, side, row, column),
                        ),
                        None => match sheet.output_cell(row, column) {
                            Some((_, text, source)) => {
                                (text, Self::output_fill(sheet, row, column, source))
                            }
                            None => (String::new(), theme::SURFACE),
                        },
                    };
                    painter.rect_filled(cell, 0.0, fill);
                    let shown_text: String = text.chars().take(200).collect();
                    painter
                        .with_clip_rect(cell.shrink(1.0).intersect(area).intersect(clip))
                        .text(
                            cell.left_center() + egui::vec2(4.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            shown_text,
                            font.clone(),
                            theme::TEXT,
                        );
                    painter.line_segment(
                        [cell.right_top(), cell.right_bottom()],
                        egui::Stroke::new(1.0, theme::BORDER),
                    );
                    if (row, column) == self.selected {
                        painter.rect_stroke(
                            cell.shrink(1.0),
                            0.0,
                            egui::Stroke::new(
                                if pane == self.active { 2.0 } else { 1.0 },
                                theme::ACCENT,
                            ),
                            egui::StrokeKind::Inside,
                        );
                    }
                }
                painter.line_segment(
                    [line.left_bottom(), line.right_bottom()],
                    egui::Stroke::new(1.0, theme::BORDER),
                );
            }
            // 行号栏：本侧的源行号；未解决冲突为红色竖条。
            let gutter = egui::Rect::from_min_size(line.min, egui::vec2(GUTTER, ROW_HEIGHT));
            painter.rect_filled(gutter, 0.0, theme::SURFACE);
            if sheet.row_unresolved(row) {
                painter.rect_filled(
                    egui::Rect::from_min_size(gutter.min, egui::vec2(4.0, ROW_HEIGHT)),
                    0.0,
                    theme::MERGE_CONFLICT,
                );
            }
            let number = match side {
                Some(side) => sheet.rows[row].index(side).map(|i| i + 1),
                None => present.map(|r| r + 1),
            };
            if let Some(number) = number {
                painter.text(
                    gutter.right_center() - egui::vec2(6.0, 0.0),
                    egui::Align2::RIGHT_CENTER,
                    number.to_string(),
                    egui::FontId::proportional(9.5),
                    if row == self.selected.0 {
                        theme::TEXT_MUTED
                    } else {
                        theme::TEXT_FAINT
                    },
                );
            }
        }
        // 悬停显示完整格内容（网格里放不下时被截断的长文本）。
        if let Some(pos) = response.hover_pos() {
            if area.contains(pos) {
                let row = first + ((pos.y - area.top() + offset) / ROW_HEIGHT) as usize;
                let x = pos.x - rect.left() - GUTTER + scroll_x;
                if row < rows && x >= 0.0 {
                    let column = ((x / COLUMN_WIDTH) as usize).min(columns.saturating_sub(1));
                    let text = match side {
                        Some(side) => self.sheet().input_text(side, row, column),
                        None => self
                            .sheet()
                            .output_cell(row, column)
                            .map(|(_, text, _)| text),
                    };
                    if let Some(text) = text.filter(|t| !t.is_empty()) {
                        response.clone().on_hover_ui(|ui| {
                            ui.set_max_width(720.0);
                            let capped: String = text.chars().take(4000).collect();
                            ui.add(egui::Label::new(capped).wrap());
                        });
                    }
                }
            }
        }
        // 点击选格；输出网格双击编辑。
        if let Some(pos) = response.interact_pointer_pos() {
            if response.clicked() || response.double_clicked() {
                let row = first + ((pos.y - area.top() + offset) / ROW_HEIGHT) as usize;
                let x = pos.x - rect.left() - GUTTER + scroll_x;
                if row < rows && x >= 0.0 {
                    let column = ((x / COLUMN_WIDTH) as usize).min(columns.saturating_sub(1));
                    if self.editing.is_some() {
                        self.commit_edit();
                    }
                    self.active = pane;
                    self.selected = (row, column);
                    if pane == OUT && response.double_clicked() {
                        self.begin_edit();
                    }
                }
            }
        }
        // 窗格自己的横向滚动条（内容超宽时）。
        if hbar_height > 0.0 {
            let max_column = (self.max_x[pane] / COLUMN_WIDTH).ceil() as usize;
            let mut first_column = (self.scroll_x[pane] / COLUMN_WIDTH).round() as usize;
            ui.scope_builder(egui::UiBuilder::new().max_rect(hbar), |ui| {
                if chrome::scrollbar::show_horizontal(ui, 0..=max_column, &mut first_column)
                    .changed()
                {
                    self.scroll_x[pane] = first_column as f32 * COLUMN_WIDTH;
                }
            });
        }
        if self.active == pane {
            ui.painter().line_segment(
                [header.left_bottom(), header.right_bottom()],
                egui::Stroke::new(2.0, theme::ACCENT),
            );
        }
    }

    /// 正在编辑的输出格：盖在格上的单行编辑框；Enter 提交，Esc 取消，失焦提交。
    fn edit_box(&mut self, ui: &mut egui::Ui) {
        let Some(edit) = &mut self.editing else {
            return;
        };
        let area = self.pane_rects[OUT];
        let top = area.top() + HEADER + COLUMNS + edit.row as f32 * ROW_HEIGHT - self.scroll_y;
        let left = area.left() + GUTTER + edit.column as f32 * COLUMN_WIDTH - self.scroll_x[OUT];
        let rect =
            egui::Rect::from_min_size(egui::pos2(left, top), egui::vec2(COLUMN_WIDTH, ROW_HEIGHT));
        let id = egui::Id::unique("table-merge-edit");
        let response = ui.put(
            rect,
            egui::TextEdit::singleline(&mut edit.text)
                .id(id)
                .margin(egui::vec2(3.0, 1.0)),
        );
        if !edit.focused {
            response.request_focus();
            edit.focused = true;
            return;
        }
        let (enter, escape) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Escape),
            )
        });
        if escape {
            self.editing = None;
        } else if enter || response.lost_focus() {
            self.commit_edit();
        }
    }

    fn prompts(&mut self, ctx: &egui::Context) {
        use chrome::{
            controls::Role,
            modal::{Item, Modal, Tone},
        };
        let output_item = || match &self.model.target {
            Some(path) => chrome::modal::file_item("OUTPUT", path),
            None => Item {
                badge: "OUTPUT".into(),
                title: "No output file yet".into(),
                detail: String::new(),
            },
        };
        match self.prompt.take() {
            None => {}
            Some(Prompt::Unsaved(pending)) => {
                let title = match pending {
                    Pending::Close => "Save the merge output before closing?",
                    Pending::Exit => "Save the merge output before quitting?",
                };
                match Modal::new("table-merge-unsaved", Tone::Warning, title)
                    .message("The merge result has not been saved. It will be lost if you don't save it.")
                    .items([output_item()])
                    .action("Don't Save", Role::Destructive)
                    .action("Cancel", Role::Secondary)
                    .action("Save", Role::Primary)
                    .cancel(1)
                    .show(ctx)
                {
                    Some(0) => self.finish(pending),
                    Some(1) => {}
                    Some(2) => self.save(Some(pending)),
                    _ => self.prompt = Some(Prompt::Unsaved(pending)),
                }
            }
            Some(Prompt::Conflicts { then, path }) => {
                let count = self.model.unresolved();
                match Modal::new("table-merge-conflicts", Tone::Warning, "Save with unresolved conflicts?")
                    .message(format!(
                        "{count} conflict section(s) are still unresolved. Their cells are saved \
                         with the base content (rows deleted on one side and changed on the \
                         other keep the base row; rows added differently on both sides are left out)."
                    ))
                    .items([output_item()])
                    .action("Cancel", Role::Secondary)
                    .action("Save Anyway", Role::Primary)
                    .cancel(0)
                    .show(ctx)
                {
                    Some(0) => {}
                    Some(1) => match path {
                        Some(path) => {
                            self.save_to(path, then);
                        }
                        None => self.save_now(then),
                    },
                    _ => self.prompt = Some(Prompt::Conflicts { then, path }),
                }
            }
            Some(Prompt::SaveAs {
                mut path,
                then,
                confirmed,
            }) => {
                let (mut accept, mut cancel) = (false, false);
                chrome::window::Window::new("Save merge output as")
                    .show(ctx, |ui| {
                        ui.label("Output file path");
                        chrome::widgets::text_field_width(ui, &mut path, 420.0);
                        match chrome::widgets::dialog_buttons(
                            ui,
                            &[
                                ("Cancel", chrome::controls::Role::Secondary),
                                ("Save", chrome::controls::Role::Primary),
                            ],
                        ) {
                            Some(0) => cancel = true,
                            Some(_) => accept = true,
                            None => {}
                        }
                    });
                if accept && !path.trim().is_empty() {
                    self.save_as(path, then, confirmed);
                } else if !cancel {
                    self.prompt = Some(Prompt::SaveAs {
                        path,
                        then,
                        confirmed,
                    });
                }
            }
        }
    }
}
