//! 文本合并视图：上排三个只读输入窗格（左 / base / 右），底部可编辑的输出窗格。
//! 四个窗格共用模型的行网格与同一个滚动位置；绘制只访问可见行。
#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
use super::model::{GridRow, MergeLaunch, MergeModel, OutOrigin};
use crate::chrome::{
    self,
    menubar::{MenuItem, MenuItemKind, MenuSpec},
    theme,
};
use crate::text::editor::{Caret, Editor};
use bc_core::Side;
use bc_engine_text::merge::MergeChoice;
use bc_session::prefs::Prefs;
use eframe::egui::{self, text::CCursor};
use std::path::PathBuf;

/// 输出窗格的窗格号；0..3 为 left、center、right。
const OUT: usize = 3;
const SIDES: [Side; 3] = [Side::Left, Side::Center, Side::Right];
const BADGES: [&str; 4] = ["LEFT", "BASE", "RIGHT", "OUTPUT"];
/// 输入窗格（left、center、right）在上排的列位：BASE | LEFT | RIGHT。
const COLUMN_OF: [usize; 3] = [1, 0, 2];
/// Input columns (two dividers) and inputs / output split, persisted.
const SPLIT_COLUMNS: [chrome::splitter::Split; 2] = [
    chrome::splitter::Split::ratio("merge.col.a", 1.0 / 3.0),
    chrome::splitter::Split::ratio("merge.col.b", 2.0 / 3.0),
];
const SPLIT_Y: chrome::splitter::Split =
    chrome::splitter::Split::ratio("merge.split.y", 0.55).min(80.0, 40.0);
const GUTTER: f32 = 46.0;
const HEADER: f32 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeAction {
    Home,
    /// 关闭本标签页（保存确认已完成）。
    Close,
    NewTab,
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pending {
    Close,
    Exit,
}

enum Prompt {
    /// 关闭 / 退出前输出尚未保存。
    Unsaved(Pending),
    /// 仍有未解决冲突时保存；`path` 是另存为已选好的路径（`None` = 存到输出目标）。
    Conflicts {
        then: Option<Pending>,
        path: Option<PathBuf>,
    },
    /// 问输出路径。`confirmed` = 未解决冲突已经确认过，不再问第二次。
    SaveAs {
        path: String,
        then: Option<Pending>,
        confirmed: bool,
    },
}

pub struct MergeView {
    pub model: MergeModel,
    launch: MergeLaunch,
    pub editor: Editor,
    /// 活动窗格（0..=3）。
    active: usize,
    /// 光标所在的对齐行；Take / 接受作用于它所在的区段。
    pub(crate) current_row: usize,
    /// 纵向以像素计（整张行网格），横向四个窗格共用。
    scroll: egui::Vec2,
    pending_scroll: Option<usize>,
    pane_rects: [egui::Rect; 4],
    content_width: f32,
    font_size: f32,
    dragging: bool,
    prompt: Option<Prompt>,
    action: Option<MergeAction>,
    pub message: String,
    menus: Vec<MenuSpec>,
}

fn pane_id(pane: usize) -> egui::Id {
    egui::Id::unique(("merge_pane", pane))
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

impl MergeView {
    pub fn open(launch: MergeLaunch) -> Result<Self, String> {
        Ok(Self::from_model(MergeModel::load(&launch)?, launch))
    }

    pub fn from_model(model: MergeModel, launch: MergeLaunch) -> Self {
        let mut view = Self {
            model,
            launch,
            editor: Editor::default(),
            active: OUT,
            current_row: 0,
            scroll: egui::Vec2::ZERO,
            pending_scroll: None,
            pane_rects: [egui::Rect::NOTHING; 4],
            content_width: 0.0,
            font_size: theme::EDITOR_SIZE,
            dragging: false,
            prompt: None,
            action: None,
            message: String::new(),
            menus: menus(),
        };
        // 打开即定位到第一个冲突（没有冲突则第一个改动）。
        if let Some(row) = view.model.first_conflict_or_change() {
            view.goto_row(row);
        }
        view
    }

    /// 快照场景：`merge-basic` 停在文件开头；`merge-conflict` 解决第一个冲突（取右侧）并在
    /// 输出里手工改一行，使四种着色与已解决 / 未解决的冲突同时出现。
    pub(crate) fn snapshot_preset(&mut self, scene: &str) {
        if scene == "merge-conflict" {
            self.command("Take Right");
            self.editor.place(Caret { line: 0, byte: 0 }, false);
            let _ = self.model.edit(&mut self.editor, |editor, content| {
                editor.insert(content, "# merged by hand\n")?;
                Ok(true)
            });
            self.command("Next Change Section");
        } else {
            self.goto_row(0);
        }
        self.message.clear();
    }
    /// left、right 两个输入的路径（标签页标题用）。
    pub fn paths(&self) -> [Option<PathBuf>; 2] {
        [
            Some(self.launch.spec.left.clone()),
            Some(self.launch.spec.right.clone()),
        ]
    }
    pub fn is_dirty(&self) -> bool {
        self.model.is_dirty()
    }
    pub fn prompt_open(&self) -> bool {
        self.prompt.is_some()
    }
    /// 关闭标签页：输出未保存时先确认，确认后产出 [`MergeAction::Close`]。
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

    /// 保存输出；仍有未解决冲突时先确认。`then` 是保存成功后要继续的关闭 / 退出。
    fn save(&mut self, then: Option<Pending>) {
        if self.model.unresolved() > 0 {
            self.prompt = Some(Prompt::Conflicts { then, path: None });
        } else {
            self.save_now(then);
        }
    }
    /// 冲突确认之后（或本来就没有冲突）：存到输出目标，没有目标就问路径。
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

    /// 另存为对话框点了 Save：仍有未解决冲突且还没确认过时先确认；写出失败则留在对话框里
    /// （错误显示在状态栏）。
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

    fn row_height(&self) -> f32 {
        (self.font_size * 1.125).ceil().max(theme::EDITOR_LINE_HEIGHT)
    }
    /// 把对齐行滚到视野内靠上的位置，并把光标放到它的第一行输出上。
    pub(crate) fn goto_row(&mut self, row: usize) {
        self.current_row = row;
        let line = self.model.out_line_of_row(row);
        self.editor.place(Caret { line, byte: 0 }, false);
        self.editor.clamp(&self.model.output);
        self.pending_scroll = Some(self.model.grid_row_of(row).saturating_sub(3));
    }
    fn caret_moved(&mut self) {
        let line = self.editor.caret.line;
        if let Some(meta) = self.model.out_line(line) {
            self.current_row = meta.row;
        }
        let grid = self.model.grid_row_of_out(line);
        let row_height = self.row_height();
        let top = (self.scroll.y / row_height) as usize;
        let shown = ((self.pane_rects[OUT].height() - HEADER) / row_height).max(1.0) as usize;
        if grid < top || grid + 1 >= top + shown {
            self.pending_scroll = Some(grid.saturating_sub(shown / 2));
        }
    }

    /// 当前区段：光标所在的冲突区段，否则改动段。
    fn section(&mut self) -> Option<std::ops::Range<usize>> {
        let section = self.model.section_at(self.current_row);
        if section.is_none() {
            self.message = "The cursor is not on a change".into();
        }
        section
    }

    pub(crate) fn command(&mut self, id: &str) {
        self.message.clear();
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
                if self.model.undo(&mut self.editor) {
                    self.caret_moved();
                }
            }
            "Redo" => {
                if self.model.redo(&mut self.editor) {
                    self.caret_moved();
                }
            }
            "Take Left" | "Take Center" | "Take Right" => {
                let side = match id {
                    "Take Left" => Side::Left,
                    "Take Center" => Side::Center,
                    _ => Side::Right,
                };
                if let Some(rows) = self.section() {
                    let start = rows.start;
                    self.model.take(rows, side, &mut self.editor);
                    self.goto_row(start);
                }
            }
            "Accept Current Content" => {
                if let Some(rows) = self.section() {
                    let from = rows.start;
                    self.model.accept(&rows, &self.editor);
                    // 参考的 "Clear Conflict Section, Next"：接受后顺势走到下一个冲突。
                    if let Some(next) = self.model.next_conflict(from, true) {
                        self.goto_row(next);
                    }
                }
            }
            "Next Change Section" | "Previous Change Section" => {
                let forward = id == "Next Change Section";
                match self.model.next_change(self.current_row, forward) {
                    Some(row) => self.goto_row(row),
                    None => {
                        self.message = if forward {
                            "No more changes below".into()
                        } else {
                            "No more changes above".into()
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn keyboard(&mut self, ctx: &egui::Context) {
        for event in ctx.input(|i| i.events.clone()) {
            if let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = &event
            {
                let command = match (*key, modifiers.command, modifiers.shift) {
                    (egui::Key::S, true, _) => Some("Save Output"),
                    (egui::Key::F12, _, _) => Some("Save Output As..."),
                    (egui::Key::Z, true, false) => Some("Undo"),
                    (egui::Key::Z, true, true) | (egui::Key::Y, true, _) => Some("Redo"),
                    (egui::Key::L, true, _) => Some("Take Left"),
                    (egui::Key::B, true, _) => Some("Take Center"),
                    (egui::Key::R, true, _) => Some("Take Right"),
                    (egui::Key::K, true, _) => Some("Accept Current Content"),
                    (egui::Key::N, true, _) => Some("Next Change Section"),
                    (egui::Key::P, true, _) => Some("Previous Change Section"),
                    (egui::Key::T, true, _) => Some("New Tab"),
                    (egui::Key::W, true, _) => Some("Close Tab"),
                    (egui::Key::Q, true, _) => Some("Exit"),
                    _ => None,
                };
                if let Some(command) = command {
                    self.command(command);
                    continue;
                }
            }
            // 只有输出窗格接受编辑；输入窗格恒只读。
            if self.active != OUT || ctx.memory(|m| m.focused()) != Some(pane_id(OUT)) {
                continue;
            }
            let mutating = match &event {
                egui::Event::Text(_) | egui::Event::Paste(_) | egui::Event::Cut => true,
                egui::Event::Ime(
                    egui::ImeEvent::Commit(_) | egui::ImeEvent::DeleteSurrounding { .. },
                ) => true,
                egui::Event::Key {
                    key, pressed: true, ..
                } => matches!(
                    key,
                    egui::Key::Backspace | egui::Key::Delete | egui::Key::Enter | egui::Key::Tab
                ),
                _ => false,
            };
            let before = self.editor.caret;
            let result = if mutating {
                self.model
                    .edit(&mut self.editor, |editor, content| editor.event(content, &event, ctx))
            } else {
                self.editor.event(&mut self.model.output, &event, ctx)
            };
            if let Err(e) = result {
                self.message = e.to_string();
            }
            if before != self.editor.caret {
                self.caret_moved();
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, _prefs: &mut Prefs) -> Option<MergeAction> {
        let ctx = ui.ctx().clone();
        let blocked = self.prompt.is_some();
        if !blocked {
            self.keyboard(&ctx);
        }
        self.editor.clamp(&self.model.output);
        if blocked {
            ui.disable();
        }
        ui.spacing_mut().item_spacing.y = 0.0;
        self.toolbar(ui);
        let row_height = self.row_height();
        let available = ui.available_rect_before_wrap();
        let status = egui::Rect::from_min_max(
            egui::pos2(available.left(), available.bottom() - 25.0),
            available.max,
        );
        let body = egui::Rect::from_min_max(
            available.min,
            egui::pos2(available.right() - 16.0, status.top()),
        );
        // 输入区 / 输出区与三列之间的分隔线可拖动（持久化，双击复位）。
        let top_height = SPLIT_Y.first(body.height() - HEADER);
        let inputs = egui::Rect::from_min_size(body.min, egui::vec2(body.width(), top_height));
        let output = egui::Rect::from_min_max(egui::pos2(body.left(), inputs.bottom()), body.max);
        let columns = chrome::splitter::Columns3::new(inputs, SPLIT_COLUMNS);
        for pane in 0..3 {
            self.pane_rects[pane] = columns.rects[COLUMN_OF[pane]];
        }
        // 横向滚动共用一个偏移：按最窄的输入列算上限，保证每列都能滚到行尾。
        let column = columns
            .rects
            .iter()
            .map(|r| r.width())
            .fold(f32::INFINITY, f32::min);
        self.pane_rects[OUT] = output;

        // 滚动：滚轮原样作用于四个窗格共用的位置；纵向上限取最矮的窗格。
        let rows = self.model.grid().len();
        let shortest = self
            .pane_rects
            .iter()
            .map(|r| r.height() - HEADER)
            .fold(f32::INFINITY, f32::min)
            .max(row_height);
        let max_y = (rows as f32 * row_height - shortest + row_height).max(0.0);
        if let Some(row) = self.pending_scroll.take() {
            self.scroll.y = row as f32 * row_height;
        }
        if !blocked {
            let delta = chrome::wheel::delta(&ctx, &self.pane_rects);
            self.scroll.x -= delta.x;
            self.scroll.y -= delta.y;
        }
        self.scroll.y = self.scroll.y.clamp(0.0, max_y);
        self.scroll.x = self
            .scroll
            .x
            .clamp(0.0, (self.content_width - column + GUTTER + 40.0).max(0.0));

        for pane in 0..4 {
            let rect = self.pane_rects[pane];
            ui.scope_builder(
                egui::UiBuilder::new()
                    .id_salt(("merge-pane", pane))
                    .max_rect(rect),
                |ui| {
                    ui.set_clip_rect(rect.intersect(ui.clip_rect()));
                    self.pane(ui, pane, rect);
                },
            );
        }
        for x in [self.pane_rects[0].left() - 1.0, self.pane_rects[2].left() - 1.0] {
            ui.painter().line_segment(
                [egui::pos2(x, inputs.top()), egui::pos2(x, inputs.bottom())],
                egui::Stroke::new(1.0, theme::LINE_STRONG),
            );
        }
        columns.handles(ui, "merge-columns");
        chrome::splitter::horizontal_line(
            ui,
            &SPLIT_Y,
            "merge-output",
            inputs.bottom(),
            body.x_range(),
            egui::Rangef::new(body.top(), body.bottom() - HEADER),
        );

        // 整个视图一条纵向滚动条。
        let bar = egui::Rect::from_min_max(egui::pos2(body.right(), body.top()), status.right_top());
        let last = (max_y / row_height).ceil() as usize;
        let mut top = (self.scroll.y / row_height).round() as usize;
        ui.scope_builder(egui::UiBuilder::new().max_rect(bar), |ui| {
            if chrome::scrollbar::show(ui, 0..=last, &mut top).changed() {
                self.scroll.y = top as f32 * row_height;
            }
        });

        ui.scope_builder(egui::UiBuilder::new().max_rect(status), |ui| {
            use chrome::statusbar::{self, StatusField};
            let mut count = StatusField::fixed(
                format!("{} conflict section(s)", self.model.unresolved()),
                220.0,
            );
            count.icon = Some(chrome::icons::Icon::Diffs);
            ui.spacing_mut().item_spacing.x = 0.0;
            statusbar::show(
                ui,
                &[
                    count,
                    StatusField::fixed(self.describe_row(), 260.0),
                    StatusField::fixed("Insert", 92.0),
                    StatusField::flexible(self.message.clone()),
                ],
            );
        });
        ui.allocate_rect(available, egui::Sense::hover());
        if !ctx.input(|i| i.pointer.primary_down()) {
            self.dragging = false;
        }
        self.prompts(&ctx);
        self.action.take()
    }

    fn describe_row(&self) -> String {
        let Some(row) = self.model.state.rows.get(self.current_row) else {
            return String::new();
        };
        let kind = match row.choice {
            MergeChoice::Center => "Unchanged",
            MergeChoice::Left => "Left Change",
            MergeChoice::Right => "Right Change",
            MergeChoice::Both => "Same Change",
            MergeChoice::Conflict => "Different Change",
        };
        if self.model.state.in_conflict(self.current_row) {
            format!("Conflict: {kind}")
        } else {
            kind.into()
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
        let on_section = self.model.section_at(self.current_row).is_some();
        let changes = self.model.has_changes();
        let items: Vec<ToolItem> = [
            (Icon::Home, "Home", "Home", "home", true),
            (Icon::Save, "Save", "Save Output", "file", true),
            (Icon::ArrowLeft, "Left", "Take Left", "take", on_section),
            (Icon::Select, "Center", "Take Center", "take", on_section),
            (Icon::ArrowRight, "Right", "Take Right", "take", on_section),
            (
                Icon::Status,
                "Accept",
                "Accept Current Content",
                "take",
                self.model.state.in_conflict(self.current_row),
            ),
            (Icon::Next, "Next Change", "Next Change Section", "nav", changes),
            (Icon::Prev, "Prev Change", "Previous Change Section", "nav", changes),
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

    fn header(&self, ui: &mut egui::Ui, pane: usize, rect: egui::Rect) {
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
        let badge = painter.layout_no_wrap(
            BADGES[pane].into(),
            egui::FontId::proportional(9.5),
            ink,
        );
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
            if self.model.is_dirty() {
                ui.painter().text(
                    text_rect.left_center(),
                    egui::Align2::LEFT_CENTER,
                    "not saved",
                    egui::FontId::proportional(12.0),
                    theme::TEXT_MUTED,
                );
            }
            return;
        }
        let path = match pane {
            0 => &self.launch.spec.left,
            1 => &self.launch.spec.center,
            _ => &self.launch.spec.right,
        };
        let text = self.launch.titles[pane]
            .clone()
            .unwrap_or_else(|| path.display().to_string());
        // 只读路径框：可选中/复制，右键 Copy / Select All。
        chrome::pathbar::show_readonly(ui, text_rect, &text);
    }

    /// 一个窗格：标题条 + 可见的网格行。
    fn pane(&mut self, ui: &mut egui::Ui, pane: usize, rect: egui::Rect) {
        let header = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), HEADER));
        self.header(ui, pane, header);
        let area = egui::Rect::from_min_max(header.left_bottom(), rect.max);
        ui.painter().rect_filled(area, 0.0, theme::CANVAS);
        let id = pane_id(pane);
        let response = ui.interact(area, id, egui::Sense::click_and_drag());
        if response.clicked() || response.drag_started() {
            self.active = pane;
            response.request_focus();
        }
        // 输出窗格可编辑：文本区（行号栏右侧）悬停或拖选时显示 I 形插入光标。
        if pane == OUT
            && ((response.hovered()
                && response
                    .hover_pos()
                    .is_some_and(|p| p.x >= area.left() + GUTTER))
                || (self.dragging && ui.input(|i| i.pointer.primary_down())))
        {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }
        if pane == OUT {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    id,
                    egui::EventFilter {
                        tab: true,
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        ..Default::default()
                    },
                )
            });
        }
        let row_height = self.row_height();
        let first = (self.scroll.y / row_height) as usize;
        let offset = self.scroll.y - first as f32 * row_height;
        let shown = (area.height() / row_height).ceil() as usize + 1;
        let painter = ui.painter().with_clip_rect(area.intersect(ui.clip_rect()));
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let pressed = ui.input(|i| i.pointer.primary_pressed());
        let held = ui.input(|i| i.pointer.primary_down());
        let shift = ui.input(|i| i.modifiers.shift);
        let grid: Vec<GridRow> = self
            .model
            .grid()
            .iter()
            .skip(first)
            .take(shown)
            .copied()
            .collect();
        for (i, cell) in grid.into_iter().enumerate() {
            let top = area.top() + i as f32 * row_height - offset;
            let line_rect = egui::Rect::from_min_size(
                egui::pos2(area.left(), top),
                egui::vec2(area.width(), row_height),
            );
            let conflict = self.model.state.in_conflict(cell.row);
            // 这一显示行上本窗格的内容：行号、文本、底色。
            let (number, text, fill): (Option<usize>, Option<String>, egui::Color32) =
                if pane == OUT {
                    match cell.out {
                        Some(line) => {
                            let origin = self.model.out_line(line).map(|m| m.origin);
                            let fill = match origin {
                                Some(OutOrigin::Left) => theme::MERGE_LEFT_SOFT,
                                Some(OutOrigin::Right) => theme::MERGE_RIGHT_SOFT,
                                Some(OutOrigin::Manual) => theme::MERGE_MANUAL_SOFT,
                                _ if conflict => theme::MERGE_CONFLICT_SOFT,
                                _ => theme::SURFACE,
                            };
                            let text = self
                                .model
                                .output
                                .lines
                                .borrow()
                                .line_bytes(line)
                                .map(|b| String::from_utf8_lossy(b).into_owned());
                            (Some(line), text, fill)
                        }
                        None => (None, None, theme::SURFACE),
                    }
                } else {
                    let side = SIDES[pane];
                    let mapped = self.model.map.row(cell.row);
                    let index = mapped.and_then(|r| match side {
                        Side::Left => r.left,
                        Side::Right => r.right,
                        _ => r.center,
                    });
                    let own = cell.own.then_some(index).flatten();
                    let choice = self.model.state.rows.get(cell.row).map(|r| r.choice);
                    let changed = match side {
                        Side::Left => matches!(
                            choice,
                            Some(MergeChoice::Left | MergeChoice::Both | MergeChoice::Conflict)
                        ),
                        Side::Right => matches!(
                            choice,
                            Some(MergeChoice::Right | MergeChoice::Both | MergeChoice::Conflict)
                        ),
                        _ => false,
                    };
                    let fill = if conflict {
                        theme::MERGE_CONFLICT_SOFT
                    } else if changed && side == Side::Left {
                        theme::MERGE_LEFT_SOFT
                    } else if changed {
                        theme::MERGE_RIGHT_SOFT
                    } else {
                        theme::SURFACE
                    };
                    let text = own.and_then(|_| {
                        self.model
                            .map
                            .text_at(side, cell.row)
                            .map(|b| String::from_utf8_lossy(&b).into_owned())
                    });
                    (own, text, fill)
                };
            painter.rect_filled(line_rect, 0.0, fill);
            let Some(text) = text else {
                // 空隙行：本侧在这一显示行上没有内容。
                crate::text::render::hatch(ui, line_rect.intersect(area), false);
                self.paint_gutter(&painter, line_rect, None, conflict, cell.row);
                if pressed
                    && ui.is_enabled()
                    && pointer.is_some_and(|p| line_rect.contains(p) && area.contains(p))
                {
                    self.active = pane;
                    self.current_row = cell.row;
                }
                continue;
            };
            if cell.row == self.current_row {
                painter.line_segment(
                    [line_rect.left_bottom(), line_rect.right_bottom()],
                    egui::Stroke::new(1.0, egui::Color32::from_gray(190)),
                );
            }
            // 超长行只排版开头一段；合并窗格不做横向窗口化。
            let mut finish = text.len().min(4096);
            while !text.is_char_boundary(finish) {
                finish -= 1;
            }
            let (job, bytes) = crate::text::render::layout(
                &text[..finish],
                0,
                &[],
                &[],
                &Default::default(),
                self.font_size,
                false,
                false,
                4,
                ui.visuals().text_color(),
                &egui::FontId::monospace(self.font_size),
            );
            let galley = painter.layout_job(job);
            self.content_width = self.content_width.max(galley.size().x);
            let origin = line_rect.min + egui::vec2(GUTTER + 4.0 - self.scroll.x, 1.0);
            let text_clip = egui::Rect::from_min_max(
                egui::pos2(line_rect.left() + GUTTER, line_rect.top()),
                line_rect.max,
            )
            .intersect(area);
            let text_painter = painter.with_clip_rect(text_clip.intersect(ui.clip_rect()));
            let char_for_byte = |byte: usize| {
                bytes
                    .partition_point(|b| *b < byte)
                    .min(galley.text().chars().count())
            };
            if pane == OUT {
                let line = number.unwrap_or_default();
                if let Some((a, b)) = self.editor.selection() {
                    if (a.line..=b.line).contains(&line) {
                        let from = if line == a.line { a.byte } else { 0 };
                        let to = if line == b.line { b.byte } else { text.len() };
                        let x1 = galley.pos_from_cursor(CCursor::new(char_for_byte(from))).left();
                        let x2 = galley.pos_from_cursor(CCursor::new(char_for_byte(to))).left();
                        text_painter.rect_filled(
                            egui::Rect::from_min_max(
                                origin + egui::vec2(x1, 0.0),
                                origin + egui::vec2(x2.max(x1 + 2.0), row_height - 1.0),
                            ),
                            0.0,
                            theme::palette(ui.ctx()).text_selection,
                        );
                    }
                }
            }
            text_painter.galley(origin, galley.clone(), ui.visuals().text_color());
            if pane == OUT && number == Some(self.editor.caret.line) {
                let cursor = galley
                    .pos_from_cursor(CCursor::new(char_for_byte(self.editor.caret.byte)))
                    .translate(origin.to_vec2());
                text_painter.line_segment(
                    [cursor.left_top(), cursor.left_bottom()],
                    egui::Stroke::new(1.5, ui.visuals().text_color()),
                );
                if !self.editor.preedit.is_empty() {
                    text_painter.text(
                        cursor.left_top(),
                        egui::Align2::LEFT_TOP,
                        &self.editor.preedit,
                        egui::FontId::monospace(self.font_size),
                        theme::MERGE_MANUAL,
                    );
                }
                if ui.memory(|m| m.owns_ime_events(id)) {
                    ui.output_mut(|o| {
                        o.ime = Some(egui::output::IMEOutput {
                            purpose: egui::IMEPurpose::Normal,
                            rect: area,
                            cursor_rect: cursor,
                            should_interrupt_composition: false,
                        });
                        o.mutable_text_under_cursor = true;
                    });
                }
            }
            self.paint_gutter(&painter, line_rect, number, conflict, cell.row);
            // 弹出层盖在窗格上时，按下属于弹出层。
            let covered = pointer
                .is_some_and(|p| pressed && ui.ctx().layer_id_at(p).is_some_and(|l| l != ui.layer_id()));
            if let Some(p) = pointer
                .filter(|p| line_rect.contains(*p) && area.contains(*p) && !covered && ui.is_enabled())
            {
                if pressed {
                    self.active = pane;
                    self.current_row = cell.row;
                    ui.memory_mut(|m| m.request_focus(id));
                }
                if pane == OUT && (pressed || (held && self.dragging)) {
                    let cursor = galley.cursor_from_pos(p - origin);
                    let byte = bytes.get(cursor.index.0).copied().unwrap_or(finish);
                    self.editor.place(
                        Caret {
                            line: number.unwrap_or_default(),
                            byte,
                        },
                        !pressed || shift,
                    );
                    self.current_row = cell.row;
                    self.dragging = true;
                }
            }
        }
        if pane == OUT && self.dragging && ui.input(|i| i.pointer.primary_released()) {
            ui.memory_mut(|m| m.request_focus(id));
        }
        if self.active == pane {
            ui.painter().line_segment(
                [header.left_bottom(), header.right_bottom()],
                egui::Stroke::new(2.0, theme::ACCENT),
            );
        }
    }

    fn paint_gutter(
        &self,
        painter: &egui::Painter,
        line: egui::Rect,
        number: Option<usize>,
        conflict: bool,
        row: usize,
    ) {
        let gutter = egui::Rect::from_min_size(line.min, egui::vec2(GUTTER, line.height()));
        painter.rect_filled(gutter, 0.0, theme::SURFACE);
        if conflict {
            // 未解决冲突：行号栏左缘的红色竖条。
            painter.rect_filled(
                egui::Rect::from_min_size(gutter.min, egui::vec2(4.0, gutter.height())),
                0.0,
                theme::MERGE_CONFLICT,
            );
        }
        if let Some(number) = number {
            painter.text(
                gutter.right_center() - egui::vec2(6.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                (number + 1).to_string(),
                egui::FontId::proportional(9.5),
                if row == self.current_row {
                    theme::TEXT_MUTED
                } else {
                    theme::TEXT_FAINT
                },
            );
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
                match Modal::new("merge-unsaved", Tone::Warning, title)
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
                match Modal::new("merge-conflicts", Tone::Warning, "Save with unresolved conflicts?")
                    .message(format!(
                        "{count} conflict section(s) are still unresolved. The output will be \
                         saved exactly as shown; no conflict markers are added."
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
