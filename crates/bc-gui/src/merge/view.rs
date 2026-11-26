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
use crate::text::{
    bookmarks::{self, Bookmarks, Toggled},
    editor::{Caret, Editor},
    find_panel::{FindAction, FindPanel},
    render::{bookmark_badge, layout, LineInk},
    syntax_style::SyntaxStyle,
};
use bc_core::Side;
use bc_engine_text::{
    content::ContentSnapshot,
    importance::{CompiledComparison, LineView},
    inline::{inline_diff_classified, InlineOptions, InlineSpan},
    merge::MergeChoice,
    search::{self, FindHit},
    TextContent,
};
use bc_session::prefs::{DiffColors, Prefs};
use eframe::egui::{self, text::CCursor, text::LayoutJob};
use std::{collections::HashMap, path::PathBuf};

/// 输出窗格的窗格号；0..3 为 left、center、right。
const OUT: usize = 3;
const SIDES: [Side; 3] = [Side::Left, Side::Center, Side::Right];
/// 窗格在查找命中（[`FindHit::side`]）里的标记。
fn side_of(pane: usize) -> Side {
    SIDES.get(pane).copied().unwrap_or(Side::Output)
}
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
/// 排版与行内比较只看每行开头这么多字节（合并窗格不做横向窗口化）。
const LINE_LIMIT: usize = 4096;
/// 行内差异缓存的上限：超过即整体清空（与文本比较相同）。
pub(crate) const INLINE_LIMIT: usize = 512;

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
    /// 三个输入窗格（left、center、right）的光标与选区：只定位、选择、取选区文本，从不编辑。
    pub(crate) input_editors: [Editor; 3],
    /// 每个输入侧：行号 → 对齐行（载入后不变）。
    side_rows: [Vec<usize>; 3],
    /// 活动窗格（0..=3）。
    active: usize,
    /// 光标所在的对齐行；Take / 接受作用于它所在的区段。
    pub(crate) current_row: usize,
    /// 纵向以像素计（整张行网格），横向四个窗格共用。
    scroll: egui::Vec2,
    pending_scroll: Option<usize>,
    pane_rects: [egui::Rect; 4],
    /// 每个窗格本帧可见行的最宽排版宽度（下一帧据此决定横向滚动条与上限）。
    content_widths: [f32; 4],
    /// 本帧的横向滚动条（内容放得下时为 `None`）与横向偏移上限。
    pub(crate) hbar: Option<egui::Rect>,
    pub(crate) max_x: f32,
    font_size: f32,
    /// 正在拖选的窗格。
    dragging: Option<usize>,
    prompt: Option<Prompt>,
    action: Option<MergeAction>,
    pub message: String,
    /// 「Difference not found.」面板（状态栏上方）。
    pub(crate) not_found: chrome::message_panel::NotFoundPanel,
    /// Options › Next Difference（每帧 `ui()` 开头从偏好同步）。
    pub(crate) next_diff: bc_session::prefs::NextDiffPrefs,
    menus: Vec<MenuSpec>,
    /// View › Syntax Highlighting。
    pub(crate) show_syntax: bool,
    /// 左 / 右输入窗格改动行与 base 的行内差异（本侧的区间），键为 (窗格, 对齐行)。
    /// 输入只读，缓存从不失效，只在达到 [`INLINE_LIMIT`] 时清空。
    pub(crate) inline: HashMap<(usize, usize), Vec<InlineSpan>>,
    /// 合并会话的比较规则（载入时确定）编译结果，供行内差异使用。
    comparison: CompiledComparison,
    /// 差异配色（不重要差异色），每帧取自偏好。
    colors: DiffColors,
    /// 工具栏下方的查找条（与文本比较同一组件）。
    pub(crate) find: FindPanel,
    /// 编号书签 0–9，锚点为 (窗格, 对齐行)；对齐行编号载入后不变，编辑与 Take 不移动书签。
    pub(crate) bookmarks: Bookmarks<(usize, usize)>,
    /// 下一帧把键盘焦点交给这个窗格（Go to Bookmark 切换了活动窗格）。
    focus_request: Option<usize>,
}

/// 一个窗格本帧的排版资料：内容快照（语法 token）、语法风格与粗体字体。
pub(crate) struct PaneInk {
    snapshot: ContentSnapshot,
    syntax: SyntaxStyle,
    bold: egui::FontId,
    normal: egui::Color32,
}

/// 由某行在合并中的类型判断 `side` 侧是否改了这一行。
fn side_changed(choice: Option<MergeChoice>, side: Side) -> bool {
    match side {
        Side::Left => matches!(
            choice,
            Some(MergeChoice::Left | MergeChoice::Both | MergeChoice::Conflict)
        ),
        Side::Right => matches!(
            choice,
            Some(MergeChoice::Right | MergeChoice::Both | MergeChoice::Conflict)
        ),
        _ => false,
    }
}

/// 指针 → 光标（四个窗格通用）：`galley` 是第 `line` 行从 `origin` 处排出的文字，
/// `bytes` 是其字符 → 字节映射（末项为行尾）。
fn hit(
    galley: &egui::Galley,
    bytes: &[usize],
    origin: egui::Pos2,
    pointer: egui::Pos2,
    line: usize,
) -> Caret {
    let cursor = galley.cursor_from_pos(pointer - origin);
    let byte = bytes
        .get(cursor.index.0)
        .or(bytes.last())
        .copied()
        .unwrap_or_default();
    Caret { line, byte }
}

/// `text` 截到 `limit` 字节以内的字符边界。
fn clip(text: &str, limit: usize) -> &str {
    let mut finish = text.len().min(limit);
    while !text.is_char_boundary(finish) {
        finish -= 1;
    }
    &text[..finish]
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
                menu_item("Take Left Then Right", None),
                menu_item("Take Right Then Left", None),
                menu_item("Take Left Line", Some("Shift+Ctrl+L")),
                menu_item("Take Center Line", Some("Shift+Ctrl+B")),
                menu_item("Take Right Line", Some("Shift+Ctrl+R")),
                menu_item("Accept Current Content", Some("Ctrl+K")),
            ],
        ),
        menu(
            "View",
            'v',
            vec![MenuItem {
                kind: MenuItemKind::Check { checked: true },
                ..menu_item("Syntax Highlighting", None)
            }],
        ),
        menu(
            "Search",
            'r',
            vec![
                menu_item("Find...", Some("Ctrl+F")),
                menu_item("Find Next", Some("F3")),
                menu_item("Find Previous", Some("Shift+F3")),
                menu_item("Replace...", Some("Ctrl+H")),
                menu_item("", None),
                menu_item("Next Change Section", Some("Ctrl+N")),
                menu_item("Previous Change Section", Some("Ctrl+P")),
                menu_item("", None),
                bookmark_menu("Toggle Bookmark", "Toggle", "Shift+Ctrl+"),
                bookmark_menu("Go to Bookmark", "Go to", "Ctrl+"),
                menu_item("Clear Bookmarks", None),
            ],
        ),
    ]
}

/// Toggle / Go to Bookmark 子菜单：子项标签 `Bookmark N`，action_id `Toggle Bookmark N` /
/// `Go to Bookmark N`（与文本比较相同）。
fn bookmark_menu(label: &str, verb: &str, keys: &str) -> MenuItem {
    MenuItem {
        kind: MenuItemKind::Submenu(
            (0..bookmarks::COUNT)
                .map(|n| MenuItem {
                    action_id: format!("{verb} Bookmark {n}"),
                    ..menu_item(&format!("Bookmark {n}"), Some(&format!("{keys}{n}")))
                })
                .collect(),
        ),
        ..menu_item(label, None)
    }
}

/// 书签命令的编号：`Toggle Bookmark N` / `Go to Bookmark N`。
fn bookmark_command(id: &str, prefix: &str) -> Option<usize> {
    let n: usize = id.strip_prefix(prefix)?.parse().ok()?;
    (n < bookmarks::COUNT).then_some(n)
}

impl MergeView {
    pub fn open(launch: MergeLaunch) -> Result<Self, String> {
        Ok(Self::from_model(MergeModel::load(&launch)?, launch))
    }

    pub fn from_model(model: MergeModel, launch: MergeLaunch) -> Self {
        let comparison = CompiledComparison::new(&model.map.rules.comparison)
            .or_else(|_| CompiledComparison::new(&Default::default()))
            .expect("default comparison rules compile");
        let mut side_rows: [Vec<usize>; 3] = Default::default();
        for (row, mapped) in model.map.rows().enumerate() {
            for (pane, index) in [mapped.left, mapped.center, mapped.right]
                .into_iter()
                .enumerate()
            {
                if let Some(index) = index {
                    let rows = &mut side_rows[pane];
                    rows.resize(rows.len().max(index + 1), row);
                    rows[index] = row;
                }
            }
        }
        let mut view = Self {
            model,
            launch,
            editor: Editor::default(),
            input_editors: Default::default(),
            side_rows,
            active: OUT,
            current_row: 0,
            scroll: egui::Vec2::ZERO,
            pending_scroll: None,
            pane_rects: [egui::Rect::NOTHING; 4],
            content_widths: [0.0; 4],
            hbar: None,
            max_x: 0.0,
            font_size: theme::EDITOR_SIZE,
            dragging: None,
            prompt: None,
            action: None,
            message: String::new(),
            not_found: Default::default(),
            next_diff: Default::default(),
            menus: menus(),
            show_syntax: true,
            inline: HashMap::new(),
            comparison,
            colors: DiffColors::default(),
            find: FindPanel::default(),
            bookmarks: Bookmarks::default(),
            focus_request: None,
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
        (self.font_size * 1.125)
            .ceil()
            .max(theme::EDITOR_LINE_HEIGHT)
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
    /// 输入窗格光标移动后：光标行所在的对齐行成为当前行，并滚到视野内。
    fn input_caret_moved(&mut self, pane: usize) {
        let line = self.input_editors[pane].caret.line;
        let Some(&row) = self.side_rows[pane].get(line) else {
            return;
        };
        self.current_row = row;
        let grid = self.model.grid_row_of(row);
        let row_height = self.row_height();
        let top = (self.scroll.y / row_height) as usize;
        let shown = ((self.pane_rects[pane].height() - HEADER) / row_height).max(1.0) as usize;
        if grid < top || grid + 1 >= top + shown {
            self.pending_scroll = Some(grid.saturating_sub(shown / 2));
        }
    }
    fn editor_of(&self, pane: usize) -> &Editor {
        if pane == OUT {
            &self.editor
        } else {
            &self.input_editors[pane]
        }
    }
    fn editor_of_mut(&mut self, pane: usize) -> &mut Editor {
        if pane == OUT {
            &mut self.editor
        } else {
            &mut self.input_editors[pane]
        }
    }

    /// Take 的作用范围：输出窗格有选区时为选中行（选区止于行首时不含该行），
    /// 否则为光标所在区段（参考：Takes selected lines or current section）。
    fn take_rows(&mut self) -> Option<std::ops::Range<usize>> {
        if let Some((start, end)) = self.editor.selection() {
            let last = if end.byte == 0 && end.line > start.line {
                end.line - 1
            } else {
                end.line
            };
            let row = |line: usize| self.model.out_line(line).map(|m| m.row);
            if let (Some(first), Some(last)) = (row(start.line), row(last)) {
                return Some(first..last + 1);
            }
        }
        self.section()
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
            "Take Left"
            | "Take Center"
            | "Take Right"
            | "Take Left Then Right"
            | "Take Right Then Left" => {
                let sides: &[Side] = match id {
                    "Take Left" => &[Side::Left],
                    "Take Center" => &[Side::Center],
                    "Take Right" => &[Side::Right],
                    "Take Left Then Right" => &[Side::Left, Side::Right],
                    _ => &[Side::Right, Side::Left],
                };
                if let Some(rows) = self.take_rows() {
                    let start = rows.start;
                    self.model.take_sides(rows, sides, &mut self.editor);
                    self.goto_row(start);
                }
            }
            // 只取光标所在的一行（对齐行），光标留在原处便于逐行处理。
            "Take Left Line" | "Take Center Line" | "Take Right Line" => {
                let side = match id {
                    "Take Left Line" => Side::Left,
                    "Take Center Line" => Side::Center,
                    _ => Side::Right,
                };
                let row = self.current_row;
                if row < self.model.map.row_count() {
                    self.model.take_line(row, side, &mut self.editor);
                    self.goto_row(row);
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
            "Syntax Highlighting" => self.show_syntax = !self.show_syntax,
            "Find..." => self.find.show(),
            "Replace..." => {
                if self.active == OUT {
                    self.find.show();
                    self.find.replace_open = true;
                } else {
                    self.message = "Replace works in the output pane only".into();
                }
            }
            "Find Next" => self.find_action(FindAction::Next),
            "Find Previous" => self.find_action(FindAction::Previous),
            "Clear Bookmarks" => {
                self.bookmarks.clear();
                self.message = "Bookmarks cleared".into();
            }
            id if bookmark_command(id, "Toggle Bookmark ").is_some() => {
                self.toggle_bookmark(bookmark_command(id, "Toggle Bookmark ").unwrap())
            }
            id if bookmark_command(id, "Go to Bookmark ").is_some() => {
                self.goto_bookmark(bookmark_command(id, "Go to Bookmark ").unwrap())
            }
            "Next Change Section" | "Previous Change Section" => {
                let forward = id == "Next Change Section";
                let target = self.model.next_change(self.current_row, forward).or_else(|| {
                    // Wrap around：越过末端回到第一个（向上：最后一个）改动区段。
                    self.next_diff
                        .wrap_around
                        .then(|| self.model.edge_change(forward))
                        .flatten()
                });
                match target {
                    Some(row) => self.goto_row(row),
                    None => self.not_found.not_found(self.next_diff),
                }
            }
            _ => {}
        }
    }

    /// 活动窗格光标所在的对齐行：输出窗格取光标行所属的对齐行，输入窗格取光标行的对齐行。
    fn caret_row(&self) -> usize {
        let line = self.editor_of(self.active).caret.line;
        let row = if self.active == OUT {
            self.model.out_line(line).map(|m| m.row)
        } else {
            self.side_rows[self.active].get(line).copied()
        };
        row.unwrap_or(self.current_row)
    }
    /// Toggle Bookmark N：书签挂在活动窗格光标所在的对齐行上。
    fn toggle_bookmark(&mut self, n: usize) {
        let row = self.caret_row();
        self.message = match self.bookmarks.toggle(n, (self.active, row)) {
            Toggled::Placed => format!("Bookmark {n} set"),
            Toggled::Removed => format!("Bookmark {n} cleared"),
        };
    }
    /// Go To Bookmark N：激活书签所在窗格，四个窗格滚到该对齐行。
    fn goto_bookmark(&mut self, n: usize) {
        let Some((pane, row)) = self.bookmarks.get(n) else {
            self.not_found
                .show(chrome::message_panel::bookmark_not_set(n));
            return;
        };
        self.goto_row(row);
        if pane != OUT {
            let line = self
                .model
                .map
                .row(row)
                .and_then(|r| [r.left, r.center, r.right][pane]);
            if let Some(line) = line {
                self.input_editors[pane].place(Caret { line, byte: 0 }, false);
            }
        }
        self.active = pane;
        self.focus_request = Some(pane);
        self.message = format!("Bookmark {n}");
    }

    /// 窗格 `pane` 中位于改动或冲突区段内的行（「仅在改动中」查找的范围）。
    pub(crate) fn change_lines(&self, pane: usize) -> Vec<std::ops::Range<usize>> {
        let mut rows: Vec<std::ops::Range<usize>> = self.model.state.changes().to_vec();
        rows.extend(self.model.state.conflict_sections().map(|(r, _)| r));
        rows.sort_by_key(|r| r.start);
        let mut lines: Vec<std::ops::Range<usize>> = Vec::new();
        let mut push = |range: std::ops::Range<usize>| {
            if range.is_empty() {
                return;
            }
            match lines.last_mut() {
                Some(last) if last.end >= range.start => last.end = last.end.max(range.end),
                _ => lines.push(range),
            }
        };
        for range in rows {
            if pane == OUT {
                push(
                    self.model.out_line_of_row(range.start)..self.model.out_line_of_row(range.end),
                );
                continue;
            }
            for row in range {
                let mapped = self.model.map.row(row);
                let index = mapped.and_then(|r| [r.left, r.center, r.right][pane]);
                if let Some(line) = index {
                    push(line..line + 1);
                }
            }
        }
        lines
    }

    /// 查找条与 Search 菜单的查找 / 替换。
    pub(crate) fn find_action(&mut self, action: FindAction) {
        self.message.clear();
        match action {
            FindAction::Next => self.find_step(true),
            FindAction::Previous => self.find_step(false),
            FindAction::Replace | FindAction::All if self.active != OUT => {
                self.message = "Replace works in the output pane only".into();
            }
            FindAction::All => {
                let allowed = self.change_lines(OUT);
                match self.model.replace_all(
                    &mut self.editor,
                    &self.find.options,
                    &self.find.replacement,
                    Some(&allowed),
                ) {
                    Ok(count) => {
                        self.message = format!("Replaced {count} occurrences");
                        if count > 0 {
                            self.caret_moved();
                        }
                    }
                    Err(e) => self.message = e.to_string(),
                }
                self.find.last_hit = None;
            }
            FindAction::Replace => {
                if let Err(e) = self.replace_current() {
                    self.message = e.to_string();
                    return;
                }
                self.find_step(true);
            }
        }
    }

    /// 上次命中仍是输出窗格的选区时，把它替换掉（正则替换展开捕获组）。
    fn replace_current(&mut self) -> Result<(), bc_core::CoreError> {
        let Some(hit) = self.current_hit(OUT) else {
            return Ok(());
        };
        let text = self
            .model
            .output
            .lines
            .borrow()
            .line_bytes(hit.line)
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .unwrap_or_default();
        let options = &self.find.options;
        let replacement = if options.is_regexp {
            let pattern = bc_core::pattern::Pattern::new(&options.pattern, options.case_sensitive)?;
            let changed =
                pattern.replace_all_if(&text, &self.find.replacement, |range| range == hit.range);
            changed[hit.range.start..changed.len() - (text.len() - hit.range.end)].to_string()
        } else {
            self.find.replacement.clone()
        };
        self.model.edit(&mut self.editor, |editor, content| {
            editor.insert(content, &replacement)?;
            Ok(true)
        })?;
        // 继续查找从替换后的位置开始。
        self.find.last_hit = None;
        self.caret_moved();
        Ok(())
    }

    /// 上次命中：属于窗格 `pane` 且仍是它的选区（用户没有移动光标）。
    fn current_hit(&self, pane: usize) -> Option<FindHit> {
        let hit = self.find.last_hit.clone()?;
        let selection = self.editor_of(pane).selection()?;
        let expected = (
            Caret {
                line: hit.line,
                byte: hit.range.start,
            },
            Caret {
                line: hit.line,
                byte: hit.range.end,
            },
        );
        (hit.side == side_of(pane) && selection == expected).then_some(hit)
    }

    /// 在活动窗格里查找下一个 / 上一个匹配；不回绕。
    fn find_step(&mut self, forward: bool) {
        let pane = self.active;
        if self.find.options.pattern.is_empty() {
            self.find.show();
            return;
        }
        let backwards = !forward || self.find.options.backwards;
        let mut options = self.find.options.clone();
        options.backwards = backwards;
        let editor = self.editor_of(pane);
        let mut from = match (self.current_hit(pane), editor.selection()) {
            (Some(hit), _) => (
                hit.line,
                if backwards {
                    hit.range.start
                } else {
                    hit.range.end
                },
            ),
            (None, Some((a, b))) => {
                let at = if backwards { a } else { b };
                (at.line, at.byte)
            }
            (None, None) => (editor.caret.line, editor.caret.byte),
        };
        // 空匹配：先越过一个字符，否则会原地再命中。
        if self
            .current_hit(pane)
            .is_some_and(|hit| hit.range.is_empty())
        {
            let mut probe = editor.clone();
            let at = Caret {
                line: from.0,
                byte: from.1,
            };
            probe.place(at, false);
            probe.move_horizontal(self.content(pane), !backwards, false, false);
            if probe.caret == at {
                self.message = "No more matches".into();
                return;
            }
            from = (probe.caret.line, probe.caret.byte);
        }
        let allowed = self.change_lines(pane);
        match search::find_next(
            self.content(pane),
            side_of(pane),
            from,
            &options,
            Some(&allowed),
        ) {
            Ok(Some(hit)) => {
                let editor = self.editor_of_mut(pane);
                editor.anchor = Some(Caret {
                    line: hit.line,
                    byte: hit.range.start,
                });
                editor.caret = Caret {
                    line: hit.line,
                    byte: hit.range.end,
                };
                self.find.last_hit = Some(hit);
                if pane == OUT {
                    self.caret_moved();
                } else {
                    self.input_caret_moved(pane);
                }
            }
            Ok(None) => self.message = "No more matches".into(),
            Err(e) => self.message = e.to_string(),
        }
    }

    fn keyboard(&mut self, ctx: &egui::Context) {
        for event in ctx.input(|i| i.events.clone()) {
            if let egui::Event::Key {
                key,
                physical_key,
                pressed: true,
                modifiers,
                ..
            } = &event
            {
                // Shift+Ctrl+0–9 放置 / 移除书签，Ctrl+0–9 跳到书签（AltGr = Ctrl+Alt 留给输入字符）。
                if let Some(n) = crate::text::view::digit(*key, *physical_key)
                    .filter(|_| modifiers.command && !modifiers.alt)
                {
                    self.command(&if modifiers.shift {
                        format!("Toggle Bookmark {n}")
                    } else {
                        format!("Go to Bookmark {n}")
                    });
                    continue;
                }
                let command = match (*key, modifiers.command, modifiers.shift) {
                    (egui::Key::F3, false, true) => Some("Find Previous"),
                    (egui::Key::F3, false, false) => Some("Find Next"),
                    (egui::Key::F, true, _) => Some("Find..."),
                    (egui::Key::H, true, _) => Some("Replace..."),
                    (egui::Key::L, true, true) => Some("Take Left Line"),
                    (egui::Key::B, true, true) => Some("Take Center Line"),
                    (egui::Key::R, true, true) => Some("Take Right Line"),
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
            // 只有输出窗格接受编辑；输入窗格恒只读，只能移动光标、选择与复制。
            if ctx.memory(|m| m.focused()) != Some(pane_id(self.active)) {
                continue;
            }
            if self.active != OUT {
                self.input_event(&event, ctx);
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
                self.model.edit(&mut self.editor, |editor, content| {
                    editor.event(content, &event, ctx)
                })
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

    /// 输入窗格的键盘事件：方向键 / Home / End / 翻页（可带 Shift）、`Ctrl+A`、复制；
    /// 字符、粘贴、剪切、删除、回车、Tab、输入法一律丢弃，内容从不改变。
    fn input_event(&mut self, event: &egui::Event, ctx: &egui::Context) {
        use egui::Key;
        let pane = self.active;
        let allowed = match event {
            egui::Event::Copy => true,
            egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } => {
                matches!(
                    key,
                    Key::ArrowLeft
                        | Key::ArrowRight
                        | Key::ArrowUp
                        | Key::ArrowDown
                        | Key::Home
                        | Key::End
                        | Key::PageUp
                        | Key::PageDown
                ) || (*key == Key::A && modifiers.command)
            }
            _ => false,
        };
        if !allowed {
            return;
        }
        let before = self.input_editors[pane].caret;
        let content = match pane {
            0 => &mut self.model.map.left,
            1 => self
                .model
                .map
                .center
                .as_mut()
                .expect("a merge always has a center"),
            _ => &mut self.model.map.right,
        };
        if let Err(e) = self.input_editors[pane].event(content, event, ctx) {
            self.message = e.to_string();
        }
        if before != self.input_editors[pane].caret {
            self.input_caret_moved(pane);
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, prefs: &mut Prefs) -> Option<MergeAction> {
        let ctx = ui.ctx().clone();
        self.font_size = prefs.editor_font.size_px();
        self.colors.clone_from(&prefs.colors);
        let blocked = self.prompt.is_some();
        self.next_diff = prefs.next_diff;
        self.not_found.begin_frame(&ctx);
        if !blocked {
            self.keyboard(&ctx);
        }
        if let Some(pane) = self.focus_request.take() {
            ctx.memory_mut(|m| m.request_focus(pane_id(pane)));
        }
        self.editor.clamp(&self.model.output);
        if blocked {
            ui.disable();
        }
        ui.spacing_mut().item_spacing.y = 0.0;
        self.toolbar(ui);
        // 替换只对输出窗格：输入窗格活动时查找条不显示替换行。
        if self.active != OUT {
            self.find.replace_open = false;
        }
        if let Some(action) = self.find.ui(ui) {
            self.find_action(action);
        }
        let row_height = self.row_height();
        let available = ui.available_rect_before_wrap();
        let status = egui::Rect::from_min_max(
            egui::pos2(available.left(), available.bottom() - 25.0),
            available.max,
        );
        // 「Difference not found.」面板紧贴状态栏上方，窗格区上移让出高度。
        let notice = egui::Rect::from_min_max(
            egui::pos2(available.left(), status.top() - self.not_found.height()),
            egui::pos2(available.right(), status.top()),
        );
        let full = egui::Rect::from_min_max(
            available.min,
            egui::pos2(available.right() - 16.0, notice.top()),
        );
        // 输入区 / 输出区与三列之间的分隔线可拖动（持久化，双击复位）。
        let layout = |body: egui::Rect| {
            let top_height = SPLIT_Y.first(body.height() - HEADER);
            let inputs = egui::Rect::from_min_size(body.min, egui::vec2(body.width(), top_height));
            let output =
                egui::Rect::from_min_max(egui::pos2(body.left(), inputs.bottom()), body.max);
            let columns = chrome::splitter::Columns3::new(inputs, SPLIT_COLUMNS);
            let mut rects = [egui::Rect::NOTHING; 4];
            for pane in 0..3 {
                rects[pane] = columns.rects[COLUMN_OF[pane]];
            }
            rects[OUT] = output;
            (rects, inputs, columns)
        };
        // 横向滚动四个窗格共用一个偏移：上限取各窗格（上一帧）可见内容超出本窗格的最大量，
        // 保证每个窗格都能滚到行尾；都放得下时不显示横向滚动条，偏移归零。
        let (rects, _, _) = layout(full);
        self.max_x = (0..4)
            .map(|pane| self.content_widths[pane] + GUTTER + 40.0 - rects[pane].width())
            .fold(0.0, f32::max);
        let body = if self.max_x > 0.0 {
            egui::Rect::from_min_max(full.min, full.max - egui::vec2(0.0, 16.0))
        } else {
            full
        };
        self.hbar = (self.max_x > 0.0).then(|| {
            egui::Rect::from_min_max(body.left_bottom(), egui::pos2(body.right(), notice.top()))
        });
        let (rects, inputs, columns) = layout(body);
        self.pane_rects = rects;
        self.content_widths = [0.0; 4];

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
            // 横向滚动条上的滚轮（含竖向滚轮）只横移。
            if let Some(hbar) = self.hbar {
                let delta = chrome::wheel::delta(&ctx, &[hbar]);
                self.scroll.x -= delta.x + delta.y;
            }
        }
        self.scroll.y = self.scroll.y.clamp(0.0, max_y);
        self.scroll.x = self.scroll.x.clamp(0.0, self.max_x);

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
        for x in [
            self.pane_rects[0].left() - 1.0,
            self.pane_rects[2].left() - 1.0,
        ] {
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

        // 窗格区底部一条四窗格共用的横向滚动条，右端与纵向滚动条之间留一个角块。
        if let Some(hbar) = self.hbar {
            chrome::scrollbar::show_horizontal_px(
                ui,
                hbar,
                egui::Id::unique("merge-hscroll"),
                hbar.width() + self.max_x,
                hbar.width(),
                &mut self.scroll.x,
            );
            ui.painter().rect_filled(
                egui::Rect::from_min_max(hbar.right_top(), status.right_top()),
                0.0,
                theme::PANEL_BG,
            );
        }
        // 整个视图一条纵向滚动条（横向滚动条出现时随窗格区底边上移）。
        let bar = egui::Rect::from_min_max(
            egui::pos2(body.right(), body.top()),
            egui::pos2(status.right(), body.bottom()),
        );
        let last = (max_y / row_height).ceil() as usize;
        let mut top = (self.scroll.y / row_height).round() as usize;
        ui.scope_builder(egui::UiBuilder::new().max_rect(bar), |ui| {
            if chrome::scrollbar::show(ui, 0..=last, &mut top).changed() {
                self.scroll.y = top as f32 * row_height;
            }
        });

        self.not_found.paint(ui, notice);
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
            self.dragging = None;
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
                "Replace..." => self.active == OUT,
                "Clear Bookmarks" => !self.bookmarks.is_empty(),
                _ => true,
            };
            // Go to Bookmark 子菜单：只有已设置的编号可用。
            if let MenuItemKind::Submenu(children) = &mut item.kind {
                for child in children {
                    child.enabled = bookmark_command(&child.action_id, "Go to Bookmark ")
                        .is_none_or(|n| self.bookmarks.get(n).is_some());
                }
            }
            if item.action_id == "Syntax Highlighting" {
                item.kind = MenuItemKind::Check {
                    checked: self.show_syntax,
                };
            }
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
    /// 输出窗格右键菜单：Edit 菜单的 Take 系列与剪贴板命令。
    pub(crate) fn context_items(&self) -> Vec<MenuItem> {
        use crate::chrome::popup::item;
        let on_change = self.model.section_at(self.current_row).is_some();
        let selection = self.editor.selection().is_some();
        let takes = on_change || selection;
        vec![
            item::action("Take Left", "Take Left", takes),
            item::action("Take Center", "Take Center", takes),
            item::action("Take Right", "Take Right", takes),
            item::action("Take Left Then Right", "Take Left Then Right", takes),
            item::action("Take Right Then Left", "Take Right Then Left", takes),
            item::separator(),
            item::action("Take Left Line", "Take Left Line", true),
            item::action("Take Center Line", "Take Center Line", true),
            item::action("Take Right Line", "Take Right Line", true),
            item::separator(),
            item::action(
                "Accept Current Content",
                "Accept Current Content",
                on_change,
            ),
            item::separator(),
            item::action("Cut", "Cut", selection),
            item::action("Copy", "Copy", selection),
            item::action("Paste", "Paste", true),
            item::separator(),
            item::action("Select All", "Select All", true),
        ]
    }

    /// 输入窗格右键菜单：Take Left / Center / Right 作用于所点行所在区段，Copy 复制本窗格
    /// 选区，Select All 全选本窗格。
    pub(crate) fn input_context_items(&self, pane: usize) -> Vec<MenuItem> {
        use crate::chrome::popup::item;
        let on_change = self.model.section_at(self.current_row).is_some();
        vec![
            item::action("Take Left", "Take Left", on_change),
            item::action("Take Center", "Take Center", on_change),
            item::action("Take Right", "Take Right", on_change),
            item::separator(),
            item::action(
                "Copy",
                "Copy",
                self.input_editors[pane].selection().is_some(),
            ),
            item::action("Select All", "Select All", true),
        ]
    }

    /// 执行输入窗格右键菜单的命令。
    pub(crate) fn input_command(&mut self, pane: usize, id: &str, ctx: &egui::Context) {
        self.message.clear();
        match id {
            "Take Left" | "Take Center" | "Take Right" => {
                let side = match id {
                    "Take Left" => Side::Left,
                    "Take Center" => Side::Center,
                    _ => Side::Right,
                };
                if let Some(rows) = self.section() {
                    let start = rows.start;
                    self.model.take_sides(rows, &[side], &mut self.editor);
                    self.goto_row(start);
                }
            }
            "Copy" => ctx.copy_text(self.input_editors[pane].selected_text(self.content(pane))),
            "Select All" => {
                let content = self.content(pane);
                let mut editor = self.input_editors[pane].clone();
                editor.select_all(content);
                self.input_editors[pane] = editor;
            }
            _ => {}
        }
    }

    /// 右键菜单的剪贴板命令：合成与快捷键相同的编辑事件交给输出编辑器。
    fn clipboard(&mut self, id: &str, ctx: &egui::Context) {
        if id == "Paste" {
            ctx.memory_mut(|m| m.request_focus(pane_id(OUT)));
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
            return;
        }
        let event = match id {
            "Cut" => egui::Event::Cut,
            "Copy" => egui::Event::Copy,
            _ => egui::Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::COMMAND,
            },
        };
        let result = if id == "Cut" {
            self.model.edit(&mut self.editor, |editor, content| {
                editor.event(content, &event, ctx)
            })
        } else {
            self.editor.event(&mut self.model.output, &event, ctx)
        };
        if let Err(e) = result {
            self.message = e.to_string();
        }
    }

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
        let row_height = self.row_height();
        let first = (self.scroll.y / row_height) as usize;
        let offset = self.scroll.y - first as f32 * row_height;
        if pane != OUT {
            // 输入窗格右键：先把当前行与活动窗格移到所点处，再弹菜单。
            if response.secondary_clicked() {
                let grid = response.interact_pointer_pos().and_then(|p| {
                    let index = first + ((p.y - area.top() + offset) / row_height) as usize;
                    self.model.grid().get(index).copied()
                });
                if let Some(cell) = grid {
                    self.current_row = cell.row;
                }
                self.active = pane;
                response.request_focus();
            }
            if let Some(command) =
                crate::chrome::popup::context_menu(&response, || self.input_context_items(pane))
            {
                self.input_command(pane, &command, ui.ctx());
            }
        }
        if pane == OUT {
            if response.secondary_clicked() {
                self.active = OUT;
                response.request_focus();
            }
            if let Some(command) =
                crate::chrome::popup::context_menu(&response, || self.context_items())
            {
                match command.as_str() {
                    "Cut" | "Copy" | "Paste" | "Select All" => self.clipboard(&command, ui.ctx()),
                    other => self.command(other),
                }
            }
        }
        // 四个窗格都可选择：文本区（行号栏右侧）悬停或拖选时显示 I 形插入光标。
        if (response.hovered()
            && response
                .hover_pos()
                .is_some_and(|p| p.x >= area.left() + GUTTER))
            || (self.dragging == Some(pane) && ui.input(|i| i.pointer.primary_down()))
        {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
        }
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
        let shown = (area.height() / row_height).ceil() as usize + 1;
        let painter = ui.painter().with_clip_rect(area.intersect(ui.clip_rect()));
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let pressed = ui.input(|i| i.pointer.primary_pressed());
        let held = ui.input(|i| i.pointer.primary_down());
        let shift = ui.input(|i| i.modifiers.shift);
        let ink = self.pane_ink(pane, ui.ctx(), ui.visuals().text_color());
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
                    let changed = side_changed(choice, side);
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
            // 书签徽标画在锚点窗格、该对齐行的第一个显示行，文本区右缘。
            let bookmark = (self.model.grid_row_of(cell.row) == first + i)
                .then(|| self.bookmarks.at((pane, cell.row)))
                .flatten();
            let Some(text) = text else {
                // 空隙行：本侧在这一显示行上没有内容。
                crate::text::render::hatch(ui, line_rect.intersect(area), false);
                self.paint_gutter(&painter, line_rect, None, conflict, cell.row);
                if let Some(n) = bookmark {
                    bookmark_badge(&painter, line_rect, n);
                }
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
            let finish = clip(&text, LINE_LIMIT).len();
            let (job, bytes) = self.line_job(
                pane,
                cell.row,
                number.unwrap_or_default(),
                &text[..finish],
                &ink,
            );
            let galley = painter.layout_job(job);
            self.content_widths[pane] = self.content_widths[pane].max(galley.size().x);
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
            {
                let line = number.unwrap_or_default();
                if let Some((a, b)) = self.editor_of(pane).selection() {
                    if (a.line..=b.line).contains(&line) {
                        let from = if line == a.line { a.byte } else { 0 };
                        let to = if line == b.line { b.byte } else { text.len() };
                        let x1 = galley
                            .pos_from_cursor(CCursor::new(char_for_byte(from)))
                            .left();
                        let x2 = galley
                            .pos_from_cursor(CCursor::new(char_for_byte(to)))
                            .left();
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
            // 输出窗格总显示光标；输入窗格只在活动时显示。
            let caret = self.editor_of(pane).caret;
            if (pane == OUT || pane == self.active) && number == Some(caret.line) {
                let cursor = galley
                    .pos_from_cursor(CCursor::new(char_for_byte(caret.byte)))
                    .translate(origin.to_vec2());
                text_painter.line_segment(
                    [cursor.left_top(), cursor.left_bottom()],
                    egui::Stroke::new(1.5, ui.visuals().text_color()),
                );
            }
            if pane == OUT && number == Some(self.editor.caret.line) {
                let cursor = galley
                    .pos_from_cursor(CCursor::new(char_for_byte(self.editor.caret.byte)))
                    .translate(origin.to_vec2());
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
            if let Some(n) = bookmark {
                bookmark_badge(&painter, line_rect, n);
            }
            // 弹出层盖在窗格上时，按下属于弹出层。
            let covered = pointer.is_some_and(|p| {
                pressed && ui.ctx().layer_id_at(p).is_some_and(|l| l != ui.layer_id())
            });
            if let Some(p) = pointer.filter(|p| {
                line_rect.contains(*p) && area.contains(*p) && !covered && ui.is_enabled()
            }) {
                if pressed {
                    self.active = pane;
                    self.current_row = cell.row;
                    ui.memory_mut(|m| m.request_focus(id));
                }
                if pressed || (held && self.dragging == Some(pane)) {
                    let caret = hit(&galley, &bytes, origin, p, number.unwrap_or_default());
                    self.editor_of_mut(pane).place(caret, !pressed || shift);
                    self.current_row = cell.row;
                    self.dragging = Some(pane);
                }
            }
        }
        // 松开时取回焦点；已有焦点时不再请求（重新请求会让焦点锁定过滤失效一帧，Tab 会跳走）。
        if self.dragging == Some(pane)
            && ui.input(|i| i.pointer.primary_released())
            && !ui.memory(|m| m.has_focus(id))
        {
            ui.memory_mut(|m| m.request_focus(id));
        }
        if self.active == pane {
            ui.painter().line_segment(
                [header.left_bottom(), header.right_bottom()],
                egui::Stroke::new(2.0, theme::ACCENT),
            );
        }
    }

    /// 窗格的内容：0..3 为 left、center、right 输入，3 为输出。
    pub(crate) fn content(&self, pane: usize) -> &TextContent {
        match pane {
            0 => &self.model.map.left,
            1 => self
                .model
                .map
                .center
                .as_ref()
                .expect("a merge always has a center"),
            2 => &self.model.map.right,
            _ => &self.model.output,
        }
    }

    pub(crate) fn pane_ink(
        &self,
        pane: usize,
        ctx: &egui::Context,
        normal: egui::Color32,
    ) -> PaneInk {
        let snapshot = self.content(pane).snapshot();
        PaneInk {
            syntax: SyntaxStyle::for_format(snapshot.format.as_deref()),
            snapshot,
            bold: theme::editor_bold(ctx, self.font_size),
            normal,
        }
    }

    /// 窗格 `pane` 中第 `line` 行（属于对齐行 `row`）的排版：语法 token（开关打开时）、
    /// left / right 窗格改动行的行内差异着墨。返回排版与字符 → 字节映射。
    pub(crate) fn line_job(
        &mut self,
        pane: usize,
        row: usize,
        line: usize,
        text: &str,
        ink: &PaneInk,
    ) -> (LayoutJob, Vec<usize>) {
        let tokens = if self.show_syntax && line < ink.snapshot.lines.len() {
            ink.snapshot.tokens(line)
        } else {
            Default::default()
        };
        self.ensure_inline(pane, row);
        let spans = self.inline.get(&(pane, row)).cloned().unwrap_or_default();
        let important = if self.model.state.in_conflict(row) {
            theme::MERGE_CONFLICT
        } else if pane == 0 {
            theme::MERGE_LEFT
        } else {
            theme::MERGE_RIGHT
        };
        let tab = ink
            .snapshot
            .format
            .as_ref()
            .and_then(|f| f.tab_size)
            .unwrap_or(4)
            .clamp(1, 32) as usize;
        layout(
            text,
            0,
            0,
            &tokens,
            &spans,
            &self.colors,
            self.font_size,
            self.model.state.rules.ignore_unimportant,
            false,
            tab,
            &LineInk {
                normal: ink.normal,
                diff: None,
                syntax: &ink.syntax,
                bold: &ink.bold,
                unimportant: &self.model.map.rules.comparison.importance.unimportant,
                important: Some(important),
            },
        )
    }

    /// 行内差异只算 left / right 输入窗格：本侧改了该对齐行（或它在冲突区段里），且本侧与
    /// base 在这一行上都有内容。base 与输出窗格从不做。
    pub(crate) fn ensure_inline(&mut self, pane: usize, row: usize) {
        if !matches!(pane, 0 | 2) || self.inline.contains_key(&(pane, row)) {
            return;
        }
        let side = SIDES[pane];
        let choice = self.model.state.rows.get(row).map(|r| r.choice);
        if !side_changed(choice, side) && !self.model.state.in_conflict(row) {
            return;
        }
        let Some(mapped) = self.model.map.row(row) else {
            return;
        };
        let own = if pane == 0 { mapped.left } else { mapped.right };
        let (Some(own), Some(base)) = (own, mapped.center) else {
            return;
        };
        if self.inline.len() >= INLINE_LIMIT {
            self.inline.clear();
        }
        let mine = self.content(pane).snapshot();
        let center = self.content(1).snapshot();
        let view = |snapshot: &ContentSnapshot, index: usize| {
            let text = snapshot
                .lines
                .line_bytes(index)
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .unwrap_or_default();
            let ending = snapshot.lines.ending(index).unwrap_or_default();
            (
                clip(&text, LINE_LIMIT).to_owned(),
                snapshot.tokens(index),
                ending,
            )
        };
        let (own_text, own_tokens, own_ending) = view(&mine, own);
        let (base_text, base_tokens, base_ending) = view(&center, base);
        let diff = inline_diff_classified(
            LineView {
                text: &own_text,
                tokens: &own_tokens,
                format: mine.format.as_deref(),
                ending: own_ending,
            },
            LineView {
                text: &base_text,
                tokens: &base_tokens,
                format: center.format.as_deref(),
                ending: base_ending,
            },
            &self.comparison,
            &InlineOptions::default(),
        );
        self.inline.insert((pane, row), diff.left);
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
                match Modal::new(
                    "merge-conflicts",
                    Tone::Warning,
                    "Save with unresolved conflicts?",
                )
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
                chrome::window::Window::new("Save merge output as").show(ctx, |ui| {
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
