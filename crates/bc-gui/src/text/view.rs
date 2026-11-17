//! 双窗格控制器：后台比较、命令、保存协议和弹窗；绘制只访问可见行。
#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
use super::{
    editor::{Caret, Editor},
    find_panel::{FindAction, FindPanel},
    gutter::{ArrowKind, GutterArrow, GutterMark},
    nav_bar::NavBar,
    projection::{DisplayFilter, DisplayRow, Projection},
    session_dialog::{Draft as SessionDraft, FormatChoice, SessionDialog},
    status_bar,
    worker::Worker,
};
use crate::chrome::{widgets, window::Window};
use bc_core::{
    encoding::{BomPolicy, Encoding},
    line::{LineEdit, MaterializeContext},
    CoreError, Side,
};
use bc_engine_text::{
    importance::{CompiledComparison, Importance},
    inline::InlineDiff,
    jobs::{Align2Job, Realign2Job},
    map::{DifferenceCounts, Section, TextMapRow},
    rules::TextRules,
    TextContent, TextMap,
};
use bc_formats::{registry::FormatRegistry, FormatKind};
use bc_session::prefs::Prefs;
use eframe::egui;
use std::{
    collections::HashMap,
    ops::Range,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// Left / right pane split (side-by-side) and top / bottom split (stacked), persisted.
const TEXT_SPLIT_X: crate::chrome::splitter::Split =
    crate::chrome::splitter::Split::ratio("text.split.x", 0.5);
const TEXT_SPLIT_Y: crate::chrome::splitter::Split =
    crate::chrome::splitter::Split::ratio("text.split.y", 0.5).min(60.0, 60.0);

#[derive(Default, Debug, Clone, Copy)]
pub struct PaneStats {
    pub rows_drawn: usize,
}
#[derive(Debug, Clone)]
pub enum ViewAction {
    /// 切到 Home 标签页（不关闭本会话）。
    Home,
    /// 关闭本标签页（未保存编辑已确认）。
    Close,
    /// Session > New Tab。
    NewTab,
    Exit,
    CompareParentFolders {
        left: PathBuf,
        right: PathBuf,
    },
    /// Edit > Compare Selection to Clipboard：选中文本（左）对剪贴板（右）开新比较。
    CompareTexts {
        left: String,
        right: String,
    },
    /// Session > Compare Files Using > Hex Compare：同一对文件原位以 Hex Compare 重开。
    CompareAsHex {
        left: std::path::PathBuf,
        right: std::path::PathBuf,
    },
}
#[derive(Clone)]
enum Pending {
    Close,
    Exit,
    Reload(usize),
    ReloadBoth,
    Open(usize, PathBuf),
    Clipboard(usize, String),
    Encoding(usize, Encoding),
}
/// Encodings offered by the file header combo and the File properties window.
const ENCODINGS: [(&str, Encoding); 5] = [
    ("UTF-8", Encoding::Utf8),
    ("UTF-16 LE", Encoding::Utf16Le),
    ("UTF-16 BE", Encoding::Utf16Be),
    ("GBK (936)", Encoding::CodePage(936)),
    ("Windows-1252", Encoding::CodePage(1252)),
];
#[derive(Clone, Copy)]
enum FileMode {
    Open,
    SaveAs,
    Target,
}
struct FileDialog {
    side: usize,
    mode: FileMode,
    path: String,
}
#[derive(Default)]
pub struct FileInfo {
    pub path: Option<PathBuf>,
    pub target: Option<PathBuf>,
    pub byte_len: usize,
    pub load_time: Duration,
    pub metadata_dirty: bool,
    pub modified_label: super::status_bar::ModifiedLabel,
}
pub struct TextCompareView {
    pub map: TextMap,
    pub files: [FileInfo; 2],
    pub editors: [Editor; 2],
    pub last_stats: PaneStats,
    pub(crate) active: usize,
    pub(crate) current_row: usize,
    pub(crate) scroll: egui::Vec2,
    pub(crate) stacked: bool,
    pub(crate) projection: Projection,
    pub(crate) inline: HashMap<usize, InlineDiff>,
    pub(crate) comparison: CompiledComparison,
    pub(crate) dragging: Option<usize>,
    pub(crate) pane_rects: [egui::Rect; 2],
    pub(crate) visible: Range<usize>,
    pub(crate) align_with: Option<(usize, usize)>,
    pub(crate) show_whitespace: bool,
    pub(crate) show_syntax: bool,
    pub(crate) show_numbers: bool,
    pub(crate) font_size: f32,
    pub(crate) content_width: f32,
    pending_scroll: Option<usize>,
    pub(crate) filter: DisplayFilter,
    context: usize,
    expanded: Vec<Range<usize>>,
    nav: NavBar,
    pub(crate) sections: Vec<Section>,
    counts: DifferenceCounts,
    registry: FormatRegistry,
    worker: Option<Worker>,
    dirty: Option<Range<usize>>,
    edited_at: Option<Instant>,
    pub message: String,
    find: FindPanel,
    session: SessionDialog,
    options: crate::dialogs::options::OptionsDialog,
    formats: super::format_panel::FormatPanel,
    pending: Option<Pending>,
    file_dialog: Option<FileDialog>,
    clipboard_side: Option<usize>,
    /// Compare Selection to Clipboard 等待 RequestPaste 回送的剪贴板；附发起时的 pass 号。
    compare_selection: Option<(String, u64)>,
    action: Option<ViewAction>,
    goto_line: usize,
    goto_column: usize,
    recent: Vec<PathBuf>,
    menus: Vec<crate::chrome::menubar::MenuSpec>,
    context_enabled: bool,
    show_info: bool,
    show_goto: bool,
    show_view_options: bool,
    show_file_options: bool,
    path_edits: [String; 2],
    path_sources: [Option<PathBuf>; 2],
    snapshot_select_paths: bool,
    displayed_sections: usize,
    /// 本帧点中的行号栏箭头（窗格、行、种类），行循环结束后执行。
    pub(crate) pending_gutter_copy: Option<(usize, usize, ArrowKind)>,
}
impl TextCompareView {
    fn registry() -> Result<FormatRegistry, CoreError> {
        match bc_formats::registry::user_formats_dir() {
            Ok(path) => FormatRegistry::load_user_dir(&path)
                .map_err(|e| CoreError::Invalid(format!("user formats: {e}"))),
            Err(_) => Ok(FormatRegistry::with_builtins()),
        }
    }
    /// 空会话：两侧无文件、路径栏为空；路径栏回车/浏览经 `open_side`
    /// 逐侧加载并重新比较（与已打开会话换文件同一路径）。
    pub fn empty() -> Result<Self, CoreError> {
        let blank = || TextContent::from_bytes(Vec::new(), None);
        Ok(Self::from_contents(blank()?, blank()?, Self::registry()?))
    }
    /// 加载一侧文件（空会话预填一侧时使用）。
    pub fn open_path(&mut self, side: usize, path: &Path) -> Result<(), CoreError> {
        self.open_side(side, path)
    }
    /// 两段内存文本的比较（Compare Selection to Clipboard 的新会话）。
    pub fn from_texts(left: &str, right: &str) -> Result<Self, CoreError> {
        let content = |t: &str| TextContent::from_bytes(t.as_bytes().to_vec(), None);
        Ok(Self::from_contents(
            content(left)?,
            content(right)?,
            Self::registry()?,
        ))
    }
    pub fn open(left: &Path, right: &Path) -> Result<Self, CoreError> {
        let registry = Self::registry()?;
        let t = std::time::Instant::now();
        let (l, lf) = load(left, &registry)?;
        let (r, rf) = load(right, &registry)?;
        crate::perf::span("text:load-files", t);
        let mut view = Self::from_contents(l, r, registry);
        view.files = [lf, rf];
        view.recent = vec![left.to_path_buf(), right.to_path_buf()];
        Ok(view)
    }
    pub fn from_contents(left: TextContent, right: TextContent, registry: FormatRegistry) -> Self {
        let rules = TextRules::default();
        let mut view = Self {
            comparison: CompiledComparison::new(&rules.comparison).expect("default rules"),
            map: TextMap::new(left, right, None, rules),
            files: Default::default(),
            editors: Default::default(),
            last_stats: Default::default(),
            active: 0,
            current_row: 0,
            scroll: egui::Vec2::ZERO,
            stacked: false,
            projection: Default::default(),
            inline: HashMap::new(),
            dragging: None,
            pane_rects: [egui::Rect::NOTHING; 2],
            visible: 0..0,
            align_with: None,
            show_whitespace: false,
            show_syntax: true,
            show_numbers: true,
            font_size: crate::chrome::theme::EDITOR_SIZE,
            content_width: 0.0,
            pending_scroll: None,
            filter: DisplayFilter::All,
            context: 3,
            expanded: Vec::new(),
            nav: Default::default(),
            sections: Vec::new(),
            counts: Default::default(),
            registry,
            worker: None,
            dirty: None,
            edited_at: None,
            message: String::new(),
            find: Default::default(),
            session: Default::default(),
            options: Default::default(),
            formats: Default::default(),
            pending: None,
            file_dialog: None,
            clipboard_side: None,
            compare_selection: None,
            action: None,
            goto_line: 1,
            goto_column: 1,
            recent: Vec::new(),
            menus: super::menu::inventory(),
            context_enabled: true,
            show_info: false,
            show_goto: false,
            show_view_options: false,
            show_file_options: false,
            path_edits: Default::default(),
            snapshot_select_paths: false,
            path_sources: Default::default(),
            displayed_sections: 0,
            pending_gutter_copy: None,
        };
        view.rebuild_projection();
        view.recompare();
        view
    }
    pub fn content(&self, side: usize) -> &TextContent {
        if side == 0 {
            &self.map.left
        } else {
            &self.map.right
        }
    }
    pub fn content_mut(&mut self, side: usize) -> &mut TextContent {
        if side == 0 {
            &mut self.map.left
        } else {
            &mut self.map.right
        }
    }
    pub fn is_ready(&self) -> bool {
        self.worker.is_none() && !self.map.is_stale() && self.edited_at.is_none()
    }
    pub fn is_dirty(&self) -> bool {
        (0..2).any(|s| self.side_dirty(s))
    }
    fn side_dirty(&self, s: usize) -> bool {
        self.content(s).is_dirty() || self.files[s].metadata_dirty
    }
    pub fn row_count(&self) -> usize {
        if self.map.is_stale() {
            self.map
                .left
                .lines
                .borrow()
                .len()
                .max(self.map.right.lines.borrow().len())
                + 1
        } else {
            self.map.row_count() + 1
        }
    }
    pub fn top_row(&self, row_height: f32) -> usize {
        self.projection.display_at(
            self.scroll.y,
            super::projection::RowGeometry::for_row_height(row_height),
        )
    }
    pub fn scroll_to_row(&mut self, row: usize) {
        self.pending_scroll = Some(row);
    }
    /// 关闭标签页：有未保存编辑时先弹保存确认，确认后产出 [`ViewAction::Close`]。
    pub fn request_close(&mut self) {
        self.request(Pending::Close);
    }
    /// 保存确认框是否打开（标签页关闭/退出流程据此识别"取消"）。
    pub fn prompt_open(&self) -> bool {
        self.pending.is_some()
    }
    /// 查找条是否打开（Esc 语义：先关查找条，再退出进程）。
    pub fn find_open(&self) -> bool {
        self.find.open
    }
    pub fn request_exit(&mut self) {
        self.request(Pending::Exit);
    }
    pub fn paths(&self) -> [Option<PathBuf>; 2] {
        [self.files[0].path.clone(), self.files[1].path.clone()]
    }
    /// Folders holding both source files, for Compare Parent Folders.
    pub fn parent_folders(&self) -> Option<[PathBuf; 2]> {
        let parent = |s: usize| {
            let path = std::path::absolute(self.files[s].path.as_ref()?).ok()?;
            Some(path.parent()?.to_path_buf())
        };
        Some([parent(0)?, parent(1)?])
    }
    pub fn index_bytes(&self) -> usize {
        (self.map.left.lines.borrow().len() + self.map.right.lines.borrow().len())
            * std::mem::size_of::<bc_core::line::Line>()
    }
    /// True while a file's modification label is still being resolved.
    pub fn labels_pending(&mut self) -> bool {
        let left = self.files[0].modified_label.poll();
        self.files[1].modified_label.poll() || left
    }
    pub fn poll(&mut self) {
        if let Some(result) = self.worker.as_mut().and_then(Worker::poll) {
            self.worker = None;
            match result {
                Ok(output) => {
                    if self.map.set_alignment(output).is_ok() {
                        self.edited_at = None;
                        self.dirty = None;
                        self.after_alignment();
                        self.message = "Comparison complete".into();
                    }
                }
                Err(bc_jobs::JobError::Cancelled) => {}
                Err(e) => {
                    self.message = e.to_string();
                    self.edited_at = None;
                }
            }
        }
        if self.worker.is_none()
            && self
                .edited_at
                .is_some_and(|t| t.elapsed() >= Duration::from_millis(350))
        {
            let job = Realign2Job::from_map(
                &self.map,
                self.dirty.clone().unwrap_or(0..self.map.row_count()),
            );
            self.worker = Some(Worker::spawn(Box::new(job)));
        }
    }
    pub fn recompare(&mut self) {
        self.cancel_join();
        self.edited_at = None;
        self.dirty = None;
        self.inline.clear();
        self.worker = Some(Worker::spawn(Box::new(Align2Job::from_map(&self.map))));
        self.message = "Comparing…".into();
        self.rebuild_projection();
    }
    fn cancel_join(&mut self) {
        drop(self.worker.take());
    }
    fn after_alignment(&mut self) {
        self.inline.clear();
        self.expanded.clear();
        self.comparison =
            CompiledComparison::new(&self.map.rules.comparison).expect("validated session rules");
        self.sections = self.map.sections();
        self.counts = self.map.statistics();
        self.displayed_sections = self
            .sections
            .iter()
            .filter(|s| {
                s.importance
                    .with_ignore_unimportant(self.map.rules.ignore_unimportant)
                    != Importance::Same
            })
            .count();
        self.nav.rebuild(&self.map);
        self.rebuild_projection();
        self.current_row = self.row_for_line(self.active, self.editors[self.active].caret.line);
    }
    pub(crate) fn edited(&mut self, old_line_count: usize) {
        if let Some(worker) = &self.worker {
            worker.cancel();
        }
        let new_count = self.content(self.active).lines.borrow().len();
        let range = if old_line_count != new_count || self.dirty.is_some() {
            0..self.map.row_count()
        } else {
            self.current_row.saturating_sub(1)..(self.current_row + 2).min(self.map.row_count())
        };
        self.dirty = Some(range);
        self.edited_at = Some(Instant::now());
        self.inline.clear();
        self.find.last_hit = None;
        self.rebuild_projection();
    }
    fn rebuild_projection(&mut self) {
        let stale = self.map.is_stale();
        let ranges: Vec<_> = self
            .sections
            .iter()
            .filter(|s| {
                s.importance
                    .with_ignore_unimportant(self.map.rules.ignore_unimportant)
                    != Importance::Same
            })
            .map(|s| s.rows.clone())
            .collect();
        self.projection = Projection::build_with_options(
            self.row_count(),
            &ranges,
            if stale {
                DisplayFilter::All
            } else {
                self.filter
            },
            self.context,
            &self.expanded,
            true,
        );
    }
    /// Character-level difference segments of the current row for the line-detail pane
    /// (reference: `he{asd}lo` / `he{l}lo`, not a whole red line).
    fn detail_segments(&mut self, prefs: &Prefs) -> [Option<status_bar::DetailSegments>; 2] {
        let row = self.row(self.current_row);
        let ignore = self.map.rules.ignore_unimportant;
        let (Some(left), Some(right)) = (row.left, row.right) else {
            return [None, None];
        };
        if row.importance.with_ignore_unimportant(ignore) == Importance::Same {
            return [None, None];
        }
        self.ensure_inline(self.current_row);
        let Some(diff) = self.inline.get(&self.current_row) else {
            return [None, None];
        };
        let segments = |spans: &[bc_engine_text::inline::InlineSpan]| {
            spans
                .iter()
                .filter(|s| s.importance.with_ignore_unimportant(ignore) != Importance::Same)
                .map(|s| {
                    (
                        s.range.clone(),
                        super::render::diff_ink(&prefs.colors, s.importance),
                    )
                })
                .collect::<Vec<_>>()
        };
        [
            Some((left, segments(&diff.left))),
            Some((right, segments(&diff.right))),
        ]
    }
    pub(crate) fn row(&self, row: usize) -> TextMapRow {
        if !self.map.is_stale() {
            if let Some(r) = self.map.row(row) {
                return r;
            }
        }
        let left = self.map.left.lines.borrow().len();
        let right = self.map.right.lines.borrow().len();
        // A virtual EOF row allows editing empty files and typing after a final newline.
        let eof = !self.map.is_stale() && row >= self.map.row_count();
        TextMapRow {
            left: if eof {
                Some(left)
            } else {
                (row <= left).then_some(row)
            },
            right: if eof {
                Some(right)
            } else {
                (row <= right).then_some(row)
            },
            ..Default::default()
        }
    }
    /// 点在本侧空隙行（只有对侧有内容）上：仍激活该窗格，光标落到空隙之后本侧第一行行首
    /// （之后没有本侧行时落在文件末尾）。否则对侧内容很长时整个窗格都是空隙，鼠标永远
    /// 无法把焦点切过来，粘贴/输入都会落空。
    pub(crate) fn place_in_gap(&mut self, s: usize, row: usize) {
        let line = (row..self.row_count())
            .find_map(|r| index(self.row(r), s))
            .unwrap_or_else(|| self.content(s).lines.borrow().len());
        self.active = s;
        self.current_row = row;
        let content = if s == 0 {
            &self.map.left
        } else {
            &self.map.right
        };
        self.editors[s].place(Caret { line, byte: 0 }, false);
        self.editors[s].clamp(content);
        if let Some(other) = index(self.row(row), 1 - s) {
            self.editors[1 - s].place(
                Caret {
                    line: other,
                    byte: 0,
                },
                false,
            );
        }
    }
    pub(crate) fn row_for_line(&self, side: usize, line: usize) -> usize {
        if self.map.is_stale() {
            return line;
        }
        let mut low = 0;
        let mut high = self.map.row_count();
        while low < high {
            let mid = (low + high) / 2;
            let candidate =
                (mid..high).find_map(|r| index(self.map.row(r).unwrap(), side).map(|i| (r, i)));
            match candidate {
                Some((r, i)) if i < line => low = r + 1,
                Some((r, i)) if i == line => return r,
                _ => high = mid,
            }
        }
        low.min(self.map.row_count())
    }
    fn navigate(&mut self, next: bool, edge: bool) {
        if self.map.is_stale() {
            self.message = "Wait for comparison before navigating differences".into();
            return;
        }
        let target = if edge {
            let mut iter = self.sections.iter().filter(|s| {
                s.importance
                    .with_ignore_unimportant(self.map.rules.ignore_unimportant)
                    != Importance::Same
            });
            if next { iter.next() } else { iter.next_back() }.map(|s| s.rows.start)
        } else if next {
            self.map
                .next_difference(self.current_row, Importance::Unimportant)
        } else {
            self.map
                .prev_difference(self.current_row, Importance::Unimportant)
        };
        if let Some(row) = target {
            self.goto_row(row);
        } else {
            self.message = "No more differences".into();
        }
    }
    pub(crate) fn goto_row(&mut self, row: usize) {
        self.current_row = row;
        for s in 0..2 {
            if let Some(line) = index(self.row(row), s) {
                self.editors[s].place(Caret { line, byte: 0 }, false);
            }
        }
        if self.filter == DisplayFilter::Same {
            self.filter = DisplayFilter::All;
            self.rebuild_projection();
        }
        self.scroll_to_row(self.projection.display_for(row));
    }
    fn copy_to(&mut self, destination: usize) -> Result<(), CoreError> {
        if self.map.is_stale() {
            return Err(CoreError::Invalid(
                "wait for comparison before copying aligned sections".into(),
            ));
        }
        self.cancel_join();
        let source = 1 - destination;
        self.content(destination).ensure_writable()?;
        if self.editors[source].selection().is_some() {
            let lines = self.editors[source].selected_lines(self.content(source));
            self.copy_lines(destination, lines);
        } else {
            self.copy_section_at(destination, self.current_row)?;
        }
        Ok(())
    }
    /// 把 `row` 所属差异区块整块覆盖到 `destination`，一次撤销，同步得到新对齐。
    fn copy_section_at(&mut self, destination: usize, row: usize) -> Result<(), CoreError> {
        let edit = self.map.copy_section(row, side(1 - destination))?;
        self.content_mut(destination).edits.push(edit);
        self.after_alignment();
        Ok(())
    }
    /// `s` 侧选区覆盖的整行；没有选区时为 `None`（光标行不算）。
    pub(crate) fn selected_line_range(&self, s: usize) -> Option<Range<usize>> {
        self.editors[s].selection()?;
        Some(self.editors[s].selected_lines(self.content(s))).filter(|r| !r.is_empty())
    }
    /// 行号栏箭头判定：`selected` 为 `s` 侧选区整行（每帧算一次后传入）。
    /// 选区所跨的行（含相同行、本侧空隙行）为选区标记；否则非相同差异区块的各行为区块标记
    /// ——本侧是空隙也画（与参考一致：点击即把空白覆盖到对侧，即删除对侧这些行）。
    /// 首行为箭头，其余行为括号线（[`GutterMark`]）。
    /// 对齐过期时差异信息不可用，只剩（禁用的）选区标记。
    pub(crate) fn gutter_arrow(
        &self,
        s: usize,
        row: usize,
        selected: Option<&Range<usize>>,
    ) -> Option<GutterArrow> {
        let selection = selected
            .filter(|r| !r.is_empty())
            .map(|r| {
                (
                    self.row_for_line(s, r.start),
                    self.row_for_line(s, r.end - 1),
                )
            })
            .filter(|(first, last)| (*first..=*last).contains(&row));
        let (kind, mark) = if let Some((first, last)) = selection {
            (ArrowKind::Selection, GutterMark::at(row, first, last))
        } else {
            if self.map.is_stale() {
                return None;
            }
            let section = self.sections.get(self.row(row).section?)?;
            if section
                .importance
                .with_ignore_unimportant(self.map.rules.ignore_unimportant)
                == Importance::Same
            {
                return None;
            }
            let rows = &section.rows;
            (
                ArrowKind::Section,
                GutterMark::at(row, rows.start, rows.end.saturating_sub(1)),
            )
        };
        Some(GutterArrow {
            kind,
            mark,
            disabled: self.gutter_disabled(s),
        })
    }
    fn gutter_disabled(&self, s: usize) -> Option<String> {
        if self.map.is_stale() {
            Some("Comparison is updating".into())
        } else if self.content(1 - s).read_only {
            Some(format!(
                "{} side is read-only",
                if s == 0 { "Right" } else { "Left" }
            ))
        } else {
            None
        }
    }
    /// 执行行号栏箭头点击：区块整块覆盖，或选中的行覆盖到另一侧（之后清除选区）。
    pub(crate) fn apply_gutter_copy(&mut self, s: usize, row: usize, kind: ArrowKind) {
        let destination = 1 - s;
        let to = if destination == 1 { "Right" } else { "Left" };
        if let Some(reason) = self.gutter_disabled(s) {
            self.message = reason;
            return;
        }
        self.cancel_join();
        match kind {
            ArrowKind::Section => match self.copy_section_at(destination, row) {
                Ok(()) => self.message = format!("Copied section to {to}"),
                Err(e) => self.message = e.to_string(),
            },
            ArrowKind::Selection => {
                let Some(lines) = self.selected_line_range(s) else {
                    self.message = "No lines selected".into();
                    return;
                };
                let count = lines.len();
                self.copy_lines(destination, lines);
                self.editors[s].anchor = None;
                self.message = format!("Copied {count} line(s) to {to}");
            }
        }
    }
    /// Copy Line to Left/Right：选中各行（无选区时为光标行）覆盖目标侧对齐行。
    fn copy_line_to(&mut self, destination: usize) -> Result<(), CoreError> {
        if self.map.is_stale() {
            return Err(CoreError::Invalid(
                "wait for comparison before copying aligned lines".into(),
            ));
        }
        self.cancel_join();
        let source = 1 - destination;
        self.content(destination).ensure_writable()?;
        let lines = self.editors[source].selected_lines(self.content(source));
        if lines.is_empty() {
            return Err(CoreError::Invalid("no line to copy".into()));
        }
        self.copy_lines(destination, lines);
        Ok(())
    }
    /// 来源行区间替换目标侧与之对齐的行（无对齐行时插入到对应位置），一次撤销。
    fn copy_lines(&mut self, destination: usize, lines: Range<usize>) {
        let source = 1 - destination;
        let first = self.row_for_line(source, lines.start);
        let end = self.row_for_line(source, lines.end);
        let target_start = (0..first)
            .rev()
            .find_map(|r| index(self.row(r), destination))
            .map_or(0, |i| i + 1);
        let target_end = (first..end)
            .filter_map(|r| index(self.row(r), destination))
            .last()
            .map_or(target_start, |i| i + 1);
        let replacement = self.content(source).lines.borrow().slice(lines);
        let target = self.content_mut(destination);
        target.edits.push(Box::new(LineEdit::replace_range(
            &target.lines,
            target_start..target_end,
            &replacement,
        )));
        self.recompare();
    }
    /// Isolate：选中各行（无选区时为光标行）不再与另一侧配对。
    fn isolate(&mut self, s: usize) -> Result<(), CoreError> {
        let lines = self.editors[s].selected_lines(self.content(s));
        if lines.is_empty() {
            return Err(CoreError::Invalid("no line to isolate".into()));
        }
        for line in lines {
            self.map.overrides.isolate(side(s), line)?;
        }
        self.align_with = None;
        self.recompare();
        Ok(())
    }
    /// Select Section：选中当前区块在 `s` 侧的整行。
    fn select_section(&mut self, s: usize) {
        let row = self.current_row;
        let rows = self
            .sections
            .iter()
            .find(|sec| sec.rows.contains(&row))
            .map_or(row..row + 1, |sec| sec.rows.clone());
        let mut lines = rows.filter_map(|r| index(self.row(r), s));
        let Some(first) = lines.next() else {
            self.message = "Section has no lines on this side".into();
            return;
        };
        let last = lines.last().unwrap_or(first);
        let end = {
            let lines = self.content(s).lines.borrow();
            if last < super::editor::end(&lines).line {
                Caret {
                    line: last + 1,
                    byte: 0,
                }
            } else {
                Caret {
                    line: last,
                    byte: lines.line_bytes(last).map_or(0, <[u8]>::len),
                }
            }
        };
        self.editors[s].anchor = Some(Caret {
            line: first,
            byte: 0,
        });
        self.editors[s].caret = end;
    }
    pub fn swap(&mut self) {
        self.cancel_join();
        std::mem::swap(&mut self.map.left, &mut self.map.right);
        self.files.swap(0, 1);
        self.editors.swap(0, 1);
        self.active = 1 - self.active;
        self.map.overrides = self.map.overrides.swapped();
        self.find.last_hit = None;
        self.align_with = None;
        self.recompare();
        self.rebuild_projection();
    }
    pub fn save_to(
        &mut self,
        s: usize,
        path: &Path,
        prefs: &Prefs,
        keep_source: bool,
    ) -> Result<(), CoreError> {
        self.content(s).ensure_writable()?;
        // Drop/join the worker before materialization; it owns the only external snapshots.
        self.cancel_join();
        if prefs.backups.enabled && path.exists() {
            let backup = backup_path(path, prefs.backups.extension.as_deref())?;
            std::fs::copy(path, &backup)?;
        }
        {
            let mut context = MaterializeContext::default();
            // A copied section can share the OTHER pane's original mmap, too.
            self.map.left.materialize_owned(&mut context);
            self.map.right.materialize_owned(&mut context);
        }
        let original_source = self.content(s).source.clone();
        self.content_mut(s).materialize_and_save(path)?;
        if keep_source {
            self.content_mut(s).source = original_source;
        } else {
            self.files[s].path = Some(path.to_path_buf());
            self.files[s].target = None;
        }
        self.files[s].metadata_dirty = false;
        self.message = format!("Saved {}", path.display());
        if self.map.is_stale() {
            self.recompare();
        }
        Ok(())
    }
    fn save(&mut self, s: usize, prefs: &Prefs) -> Result<bool, CoreError> {
        let target = self.files[s].target.clone();
        if let Some(path) = target.as_ref().or(self.files[s].path.as_ref()).cloned() {
            self.save_to(s, &path, prefs, target.is_some())?;
            Ok(true)
        } else {
            self.file_dialog = Some(FileDialog {
                side: s,
                mode: FileMode::SaveAs,
                path: String::new(),
            });
            Ok(false)
        }
    }
    fn request(&mut self, pending: Pending) {
        if self
            .affected(&pending)
            .into_iter()
            .any(|s| self.side_dirty(s))
        {
            self.pending = Some(pending);
        } else {
            self.perform(pending);
        }
    }
    fn affected(&self, p: &Pending) -> Vec<usize> {
        match p {
            Pending::Close | Pending::Exit | Pending::ReloadBoth => vec![0, 1],
            Pending::Reload(s)
            | Pending::Open(s, _)
            | Pending::Clipboard(s, _)
            | Pending::Encoding(s, _) => vec![*s],
        }
    }
    fn perform(&mut self, p: Pending) {
        let result = match p {
            Pending::Close => {
                self.action = Some(ViewAction::Close);
                Ok(())
            }
            Pending::Exit => {
                self.action = Some(ViewAction::Exit);
                Ok(())
            }
            Pending::ReloadBoth => {
                let paths = self.paths();
                (0..2).try_for_each(|s| paths[s].as_ref().map_or(Ok(()), |p| self.open_side(s, p)))
            }
            Pending::Reload(s) => self.files[s].path.clone().map_or_else(
                || {
                    Err(CoreError::Invalid(
                        "clipboard content has no source file".into(),
                    ))
                },
                |p| self.open_side(s, &p),
            ),
            Pending::Open(s, path) => self.open_side(s, &path),
            Pending::Clipboard(s, text) => {
                self.cancel_join();
                match TextContent::from_bytes(text.into_bytes(), None) {
                    Ok(mut c) => {
                        c.edits.clear();
                        *self.content_mut(s) = c;
                        self.files[s] = FileInfo::default();
                        self.editors[s] = Editor::default();
                        self.recompare();
                        self.rebuild_projection();
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            }
            Pending::Encoding(s, encoding) => {
                self.cancel_join();
                let result = self.content_mut(s).reload_encoding(encoding);
                if result.is_ok() {
                    self.editors[s] = Editor::default();
                    self.recompare();
                    self.rebuild_projection();
                }
                result
            }
        };
        if let Err(e) = result {
            self.message = e.to_string();
        }
    }
    fn open_side(&mut self, s: usize, path: &Path) -> Result<(), CoreError> {
        // Load transactionally before dropping the old editable content.
        // 二进制输入给出可操作的指引（引擎只报 "use Hex comparison"）。
        let (content, file) = load(path, &self.registry).map_err(|e| {
            if e.to_string().contains("binary input") {
                CoreError::Invalid(
                    "binary file: use Session > Compare Files Using > Hex Compare".into(),
                )
            } else {
                e
            }
        })?;
        self.cancel_join();
        *self.content_mut(s) = content;
        self.files[s] = file;
        self.editors[s] = Editor::default();
        self.map.overrides.clear();
        self.align_with = None;
        self.find.last_hit = None;
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_path_buf());
        self.recent.truncate(12);
        self.recompare();
        self.rebuild_projection();
        Ok(())
    }
    fn find_action(&mut self, action: FindAction) -> Result<(), CoreError> {
        if self.map.is_stale() {
            return Err(CoreError::Invalid(
                "wait for comparison before searching".into(),
            ));
        }
        let s = self.active;
        if matches!(action, FindAction::All) {
            let ranges = self.map.difference_line_ranges(side(s));
            let options = self.find.options.clone();
            let replacement = self.find.replacement.clone();
            let count = bc_engine_text::search::replace_all(
                self.content_mut(s),
                &options,
                &replacement,
                Some(&ranges),
            )?;
            self.message = format!("Replaced {count} occurrences");
            if count > 0 {
                self.recompare();
                self.rebuild_projection();
            }
            self.find.last_hit = None;
            return Ok(());
        }
        if matches!(action, FindAction::Replace) {
            let hit = self
                .find
                .last_hit
                .clone()
                .ok_or_else(|| CoreError::Invalid("Find a match before replacing".into()))?;
            if hit.side != side(s) {
                return Err(CoreError::Invalid("match belongs to the other pane".into()));
            }
            let text = self
                .content(s)
                .lines
                .borrow()
                .line_bytes(hit.line)
                .unwrap()
                .to_vec();
            let text = std::str::from_utf8(&text).unwrap();
            let replacement = if self.find.options.is_regexp {
                let p = bc_core::pattern::Pattern::new(
                    &self.find.options.pattern,
                    self.find.options.case_sensitive,
                )?;
                let changed =
                    p.replace_all_if(text, &self.find.replacement, |range| range == hit.range);
                changed[hit.range.start..changed.len() - (text.len() - hit.range.end)].to_string()
            } else {
                self.find.replacement.clone()
            };
            let old = self.content(s).lines.borrow().len();
            self.editors[s].caret = Caret {
                line: hit.line,
                byte: hit.range.end,
            };
            self.editors[s].anchor = Some(Caret {
                line: hit.line,
                byte: hit.range.start,
            });
            let content = if s == 0 {
                &mut self.map.left
            } else {
                &mut self.map.right
            };
            self.editors[s].insert(content, &replacement)?;
            self.edited(old);
            return Ok(());
        }
        let backwards = matches!(action, FindAction::Previous) || self.find.options.backwards;
        let mut options = self.find.options.clone();
        options.backwards = backwards;
        let mut from = (self.editors[s].caret.line, self.editors[s].caret.byte);
        if let Some(hit) = self.find.last_hit.as_ref().filter(|h| h.side == side(s)) {
            from = (
                hit.line,
                if backwards {
                    hit.range.start
                } else {
                    hit.range.end
                },
            );
            if hit.range.is_empty() {
                let mut editor = self.editors[s].clone();
                editor.place(
                    Caret {
                        line: from.0,
                        byte: from.1,
                    },
                    false,
                );
                editor.move_horizontal(self.content(s), !backwards, false, false);
                if editor.caret
                    == (Caret {
                        line: from.0,
                        byte: from.1,
                    })
                {
                    self.message = "No more matches".into();
                    return Ok(());
                }
                from = (editor.caret.line, editor.caret.byte);
            }
        }
        if let Some(hit) = self.map.find_next(side(s), from, &options)? {
            self.current_row = self.row_for_line(s, hit.line);
            self.editors[s].anchor = Some(Caret {
                line: hit.line,
                byte: hit.range.start,
            });
            self.editors[s].caret = Caret {
                line: hit.line,
                byte: hit.range.end,
            };
            if matches!(
                self.projection
                    .row(self.projection.display_for(self.current_row)),
                Some(DisplayRow::Fold(_))
            ) || self.filter == DisplayFilter::Same
            {
                self.filter = DisplayFilter::All;
                self.rebuild_projection();
            }
            self.scroll_to_row(self.projection.display_for(self.current_row));
            self.find.last_hit = Some(hit);
        } else {
            self.message = "No more matches".into();
        }
        Ok(())
    }
    /// Current session state as a Session Settings draft.
    pub(crate) fn session_draft(&self) -> SessionDraft {
        let side = |s: usize| self.content(s);
        SessionDraft {
            rules: self.map.rules.clone(),
            context: self.context,
            paths: std::array::from_fn(|s| {
                self.files[s]
                    .path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default()
            }),
            read_only: std::array::from_fn(|s| side(s).read_only),
            format: [FormatChoice::Keep, FormatChoice::Keep],
            format_names: std::array::from_fn(|s| {
                side(s)
                    .format()
                    .map_or("Everything Else", |f| f.name.as_str())
                    .to_string()
            }),
            encoding: std::array::from_fn(|s| side(s).encoding),
            bom: std::array::from_fn(|s| side(s).bom_policy),
        }
    }
    /// Apply an accepted Session Settings draft through the view's normal
    /// paths (recompare / projection / reload requests).
    pub(crate) fn apply_session(&mut self, draft: SessionDraft) {
        let before = self.map.rules.clone();
        if before != draft.rules {
            self.map.rules = draft.rules;
            if !before.same_comparison(&self.map.rules) {
                self.recompare();
            } else {
                self.after_alignment();
            }
        }
        if draft.context != self.context {
            self.context = draft.context;
            self.expanded.clear();
            self.rebuild_projection();
        }
        for s in 0..2 {
            self.content_mut(s).read_only = draft.read_only[s];
            if self.content(s).bom_policy != draft.bom[s] {
                self.content_mut(s).bom_policy = draft.bom[s];
                self.files[s].metadata_dirty = true;
            }
            let format = match &draft.format[s] {
                FormatChoice::Keep => None,
                FormatChoice::Detected => Some(
                    self.files[s]
                        .path
                        .as_ref()
                        .and_then(|p| self.registry.resolve(p, Some(FormatKind::Text)))
                        .cloned(),
                ),
                FormatChoice::Plain => Some(None),
                FormatChoice::Named(name) => Some(
                    self.registry
                        .formats()
                        .iter()
                        .find(|f| f.kind == FormatKind::Text && &f.name == name)
                        .cloned(),
                ),
            };
            if let Some(format) = format {
                match self.content_mut(s).set_format(format) {
                    Ok(()) => self.recompare(),
                    Err(e) => self.message = e.to_string(),
                }
            }
            if self.content(s).encoding != draft.encoding[s] {
                self.request(Pending::Encoding(s, draft.encoding[s]));
            }
        }
        for s in 0..2 {
            let current = self.files[s]
                .path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let wanted = draft.paths[s].trim();
            if !wanted.is_empty() && wanted != current {
                self.request(Pending::Open(s, PathBuf::from(wanted)));
            }
        }
    }
    /// 离线参考采用大小写不重要规则；只在明确的快照初始化路径调用。
    pub(crate) fn snapshot_rules(&mut self) {
        self.map.rules.comparison.importance.case_unimportant = true;
        self.recompare();
    }
    pub(crate) fn snapshot_preset(&mut self, scene: &str) {
        self.scroll_to_row(0);
        self.snapshot_select_paths = true;
        if scene == "text-diffs-filter" {
            self.context_enabled = false;
            self.filter = DisplayFilter::Differences;
            self.rebuild_projection();
        }
        if scene == "dlg-text-session-settings" {
            let draft = self.session_draft();
            self.session
                .show(draft, crate::chrome::dialog::Placement::ClientOnly);
        }
        if scene == "dlg-options" {
            self.options.show(
                &bc_session::prefs::Prefs::default(),
                crate::chrome::dialog::Placement::ClientOnly,
            );
        }
        if scene == "text-unsaved-modal" {
            self.files[0].metadata_dirty = true;
            self.files[1].metadata_dirty = true;
            self.request(Pending::Close);
        }
        if scene == "text-find-bar" {
            self.find.options.pattern = "alpha".into();
            self.find.show();
        }
        if scene == "text-long-scrolled" {
            // Reference: left pane, PageDown x3 → top row 147, caret row 158.
            self.goto_row(158);
            self.scroll_to_row(147);
        }
        if scene == "text-cursor-on-diff" {
            self.goto_row(self.row_for_line(0, 1));
            // 参考光标停在第 2 行第 5 列。
            for editor in &mut self.editors {
                editor.caret.byte = 4;
                editor.anchor = None;
            }
            self.scroll_to_row(0);
        }
    }
    pub fn title(&self) -> String {
        let name = |s: usize| {
            self.files[s]
                .path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Untitled".into())
        };
        format!("{} <--> {} - Text Compare - Matcha", name(0), name(1))
    }
    pub fn ui(&mut self, ui: &mut egui::Ui, prefs: &mut Prefs) -> Option<ViewAction> {
        self.poll();
        let ctx = ui.ctx().clone();
        if self.worker.is_some() || self.edited_at.is_some() || self.labels_pending() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        // Modal dialogs own the keyboard while they are open.
        let modal = self.session.open || self.options.open;
        if !modal {
            self.keyboard(&ctx, prefs);
        }
        // Commands (replace-all/reload/undo) may invalidate byte caret offsets.
        self.editors[0].clamp(&self.map.left);
        self.editors[1].clamp(&self.map.right);
        let blocked = self.pending.is_some() || self.file_dialog.is_some() || modal;
        if blocked {
            ui.disable();
        }
        ui.spacing_mut().item_spacing.y = 0.0;
        self.toolbar(ui, prefs);
        if let Some(action) = self.find.ui(ui) {
            if let Err(e) = self.find_action(action) {
                self.message = e.to_string();
            }
        }
        if self.session.open {
            let names: Vec<String> = self
                .registry
                .formats()
                .iter()
                .filter(|f| f.kind == FormatKind::Text)
                .map(|f| f.name.clone())
                .collect();
            let recent: Vec<String> = self
                .recent
                .iter()
                .map(|p| p.display().to_string())
                .collect();
            if let Some(draft) = self.session.ui(&ctx, &names, &recent) {
                self.apply_session(draft);
            }
        }
        self.options.ui(&ctx, prefs);
        if !blocked {
            self.formats.ui(&ctx, &mut self.registry);
        }
        self.dialogs(&ctx, prefs);
        self.last_stats.rows_drawn = 0;
        let row_height = self.row_height();
        if let Some(row) = self.pending_scroll.take() {
            self.scroll.y = self.projection.y_of(
                row.min(self.projection.len().saturating_sub(1)),
                super::projection::RowGeometry::for_row_height(row_height),
            );
        }
        if !blocked {
            // Raw wheel over either pane, 1:1 this frame; upper bounds are
            // clamped in `pane` before anything is drawn.
            let delta = crate::chrome::wheel::delta(&ctx, &self.pane_rects);
            self.scroll.x = (self.scroll.x - delta.x).max(0.0);
            self.scroll.y = (self.scroll.y - delta.y).max(0.0);
        }
        let available = ui.available_rect_before_wrap();
        let body = egui::Rect::from_min_max(
            available.min,
            egui::pos2(
                available.right(),
                (available.bottom() - 74.0).max(available.top() + 120.0),
            ),
        );
        let nav_rect = egui::Rect::from_min_size(body.min, egui::vec2(52.0, body.height()));
        ui.scope_builder(egui::UiBuilder::new().max_rect(nav_rect), |ui| {
            if let Some(row) =
                self.nav
                    .ui(ui, &prefs.colors, self.visible.clone(), self.current_row)
            {
                self.goto_row(row);
            }
        });
        let panes = egui::Rect::from_min_max(body.min + egui::vec2(52.0, 0.0), body.max);
        use crate::chrome::splitter;
        let width = TEXT_SPLIT_X.first(panes.width());
        let height = TEXT_SPLIT_Y.first(panes.height());
        for s in 0..2 {
            let rect = if self.stacked {
                if s == 0 {
                    egui::Rect::from_min_size(panes.min, egui::vec2(panes.width(), height))
                } else {
                    egui::Rect::from_min_max(panes.min + egui::vec2(0.0, height), panes.max)
                }
            } else if s == 0 {
                egui::Rect::from_min_size(panes.min, egui::vec2(width, panes.height()))
            } else {
                egui::Rect::from_min_max(panes.min + egui::vec2(width + 4.0, 0.0), panes.max)
            };
            ui.scope_builder(
                egui::UiBuilder::new()
                    .id_salt(("text-pane", s))
                    .max_rect(rect),
                |ui| {
                    ui.set_clip_rect(rect.intersect(ui.clip_rect()));
                    self.pane(ui, s, rect.height(), prefs);
                },
            );
        }
        // 窗格分隔线：并排时左右拖、上下布局时上下拖（位置持久化，双击复位）。
        if self.stacked {
            splitter::horizontal_line(
                ui,
                &TEXT_SPLIT_Y,
                "text",
                panes.top() + height,
                panes.x_range(),
                panes.y_range(),
            );
        } else {
            splitter::vertical_line(
                ui,
                &TEXT_SPLIT_X,
                "text",
                panes.left() + width + 2.0,
                panes.y_range(),
                panes.x_range(),
            );
        }
        let details = egui::Rect::from_min_size(
            egui::pos2(panes.left(), body.bottom()),
            egui::vec2(panes.width(), 49.0),
        );
        let style = {
            let row = self.row(self.current_row);
            let importance = row
                .importance
                .with_ignore_unimportant(self.map.rules.ignore_unimportant);
            let orphan = matches!(
                row.kind,
                bc_core::DiffKind::LeftOnly | bc_core::DiffKind::RightOnly
            );
            (importance != Importance::Same).then(|| {
                (
                    super::render::row_fill(&prefs.colors, importance, orphan),
                    super::render::diff_ink(&prefs.colors, importance),
                )
            })
        };
        let inline = self.detail_segments(prefs);
        status_bar::details(
            ui,
            details,
            [&self.map.left, &self.map.right],
            &self.editors,
            style,
            inline,
            self.active,
        );
        let status = egui::Rect::from_min_size(
            egui::pos2(available.left(), details.bottom()),
            egui::vec2(available.width(), 25.0),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(status), |ui| {
            use crate::chrome::statusbar::{self, StatusField};
            let current = if self.map.is_stale() {
                "Comparing"
            } else {
                match self
                    .row(self.current_row)
                    .importance
                    .with_ignore_unimportant(self.map.rules.ignore_unimportant)
                {
                    Importance::Same => "Same",
                    Importance::Unimportant => "Unimportant Difference",
                    _ => "Important Difference",
                }
            };
            let mut count = StatusField::fixed(
                format!("{} difference section(s)", self.displayed_sections),
                293.0,
            );
            count.icon = Some(crate::chrome::icons::Icon::Diffs);
            ui.spacing_mut().item_spacing.x = 0.0;
            statusbar::show(
                ui,
                &[
                    count,
                    StatusField::fixed(current, 392.0),
                    StatusField::fixed("Insert", 92.0),
                    StatusField::flexible(format!(
                        "Load time: {} seconds",
                        (self.files[0].load_time + self.files[1].load_time).as_secs()
                    )),
                ],
            );
        });
        // Bottom slots are reserved independently of document length and filtering.
        ui.allocate_rect(available, egui::Sense::hover());
        if !ctx.input(|i| i.pointer.primary_down()) {
            self.dragging = None;
        }
        if let Some((s, path)) = crate::chrome::drop::dropped_on(
            &ctx,
            crate::chrome::drop::panes(&self.pane_rects),
            self.active,
        )
        .filter(|_| !blocked)
        {
            self.request(Pending::Open(s, path));
        }
        self.action.take()
    }
    fn toolbar(&mut self, ui: &mut egui::Ui, prefs: &Prefs) {
        use crate::chrome::{icons::Icon, menubar, toolbar};
        use toolbar::{ToolItem, ToolItemKind as Kind};
        self.update_menus();
        if let Some(action) = menubar::show(ui, &self.menus) {
            self.command(&action, ui.ctx(), prefs);
        }
        let items: Vec<_> = [
            (Icon::Home, "Home", "Home", Kind::Normal, "home"),
            (
                Icon::Sessions,
                "Sessions",
                "Sessions",
                Kind::Dropdown,
                "sessions",
            ),
            (Icon::All, "All", "Show All", Kind::Toggle, "filter"),
            (
                Icon::Diffs,
                "Diffs",
                "Show Differences",
                Kind::Toggle,
                "filter",
            ),
            (Icon::Same, "Same", "Show Same", Kind::Toggle, "filter"),
            (
                Icon::Context,
                "Context",
                "Show Context",
                Kind::Toggle,
                "context",
            ),
            (
                Icon::Minor,
                "Minor",
                "Ignore Unimportant Differences",
                Kind::Toggle,
                "context",
            ),
            (
                Icon::Rules,
                "Rules",
                "Session Settings...",
                Kind::Normal,
                "rules",
            ),
            (
                Icon::Format,
                "Format",
                "File Formats...",
                Kind::Split,
                "rules",
            ),
            (Icon::Copy, "Copy", "Copy to Other", Kind::Normal, "copy"),
            (
                Icon::Next,
                "Next Section",
                "Next Difference Section",
                Kind::Normal,
                "nav",
            ),
            (
                Icon::Prev,
                "Prev Section",
                "Previous Difference Section",
                Kind::Normal,
                "nav",
            ),
            (Icon::Swap, "Swap", "Swap Sides", Kind::Normal, "files"),
            (
                Icon::Reload,
                "Reload",
                "Reload Files",
                Kind::Normal,
                "files",
            ),
        ]
        .into_iter()
        .map(|(icon, label, id, kind, group)| ToolItem {
            label: label.into(),
            action_id: id.into(),
            kind,
            enabled: true,
            selected: self.checked(id).unwrap_or(false),
            icon,
            group: Some(group.into()),
        })
        .collect();
        let response = toolbar::show(ui, &items);
        if let Some(action) = response.clicked.clone() {
            self.command(&action, ui.ctx(), prefs);
        }
        let current_format = self.content(self.active).format().map(|f| f.name.clone());
        let formats: Vec<String> = self
            .registry
            .formats()
            .iter()
            .filter(|f| f.kind == FormatKind::Text)
            .map(|f| f.name.clone())
            .collect();
        let menu = toolbar::dropdown_menu(
            ui.ctx(),
            ui.make_persistent_id("text-toolbar-dropdown"),
            &response,
            |name| {
                use crate::chrome::popup::item;
                if name == "Sessions" {
                    vec![
                        item::action("Home", "Home", true),
                        item::action("Session Settings...", "Session Settings...", true),
                        item::action("Text Compare Info", "Text Compare Info", true),
                        item::separator(),
                        item::action("Open Session", "Open Session", false),
                        item::action("Save Session", "Save Session", false),
                    ]
                } else {
                    let mut items = vec![
                        item::action("Detected", "format:detected", true),
                        item::radio("Plain text", "format:plain", current_format.is_none()),
                        item::separator(),
                    ];
                    items.extend(formats.iter().enumerate().map(|(i, name)| {
                        item::radio(
                            name.clone(),
                            format!("format:{i}"),
                            current_format.as_ref() == Some(name),
                        )
                    }));
                    items
                }
            },
        );
        if let Some(action) = menu {
            let format = match action.strip_prefix("format:") {
                None => {
                    self.command(&action, ui.ctx(), prefs);
                    None
                }
                Some("detected") => Some(
                    self.files[self.active]
                        .path
                        .as_ref()
                        .and_then(|p| self.registry.resolve(p, Some(FormatKind::Text)))
                        .cloned(),
                ),
                Some("plain") => Some(None),
                Some(i) => i
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| {
                        self.registry
                            .formats()
                            .iter()
                            .filter(|f| f.kind == FormatKind::Text)
                            .nth(i)
                            .cloned()
                    })
                    .map(Some),
            };
            if let Some(format) = format {
                match self.content_mut(self.active).set_format(format) {
                    Ok(()) => self.recompare(),
                    Err(e) => self.message = e.to_string(),
                }
            }
        }
        if self.show_file_options {
            let mut open = true;
            Window::new("File properties")
                .open(&mut open)
                .show(ui.ctx(), |ui| self.file_options(ui, self.active, prefs));
            self.show_file_options &= open;
        }
        if self.show_goto {
            let mut open = true;
            Window::new("Go To").open(&mut open).show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.label("Line:");
                    ui.add(egui::DragValue::new(&mut self.goto_line).range(1..=usize::MAX));
                    ui.label("Column:");
                    ui.add(egui::DragValue::new(&mut self.goto_column).range(1..=usize::MAX));
                    if widgets::primary_button(ui, "Go").clicked() {
                        let content = self.content(self.active);
                        let lines = content.lines.borrow();
                        let line = (self.goto_line - 1).min(lines.len().saturating_sub(1));
                        let text = std::str::from_utf8(lines.line_bytes(line).unwrap_or_default())
                            .unwrap();
                        let byte = super::editor::byte_at_char(text, self.goto_column - 1);
                        drop(lines);
                        self.current_row = self.row_for_line(self.active, line);
                        self.editors[self.active].place(Caret { line, byte }, false);
                        self.filter = DisplayFilter::All;
                        self.rebuild_projection();
                        self.scroll_to_row(self.current_row);
                        self.show_goto = false;
                    }
                });
            });
            self.show_goto &= open;
        }
        if self.show_view_options {
            Window::new("Text display")
                .open(&mut self.show_view_options)
                .show(ui.ctx(), |ui| {
                    ui.add(egui::Slider::new(&mut self.font_size, 10.0..=24.0).text("Font size"));
                });
        }
        if self.show_info {
            Window::new("Text Compare Info")
                .open(&mut self.show_info)
                .show(ui.ctx(), |ui| {
                    ui.label(status_bar::side_text(&self.map.left, &self.editors[0]));
                    ui.label(status_bar::side_text(&self.map.right, &self.editors[1]));
                    ui.label(format!(
                        "{} important · {} unimportant · {} left-only · {} right-only",
                        self.counts.important,
                        self.counts.unimportant,
                        self.counts.left_only,
                        self.counts.right_only
                    ));
                    ui.label(
                        self.worker
                            .as_ref()
                            .map(Worker::progress)
                            .unwrap_or_else(|| self.message.clone()),
                    );
                    if self.map.left.lines.borrow().ending_style()
                        != self.map.right.lines.borrow().ending_style()
                    {
                        ui.label("Line ending styles differ");
                    }
                    let final_ending = |c: &TextContent| {
                        let lines = c.lines.borrow();
                        lines.len().checked_sub(1).and_then(|i| lines.ending(i))
                    };
                    if final_ending(&self.map.left) != final_ending(&self.map.right) {
                        ui.label("Final line endings differ");
                    }
                    for file in &self.files {
                        if let Some(target) = &file.target {
                            ui.label(format!("Save target: {}", target.display()));
                        }
                    }
                });
        }
    }
    fn checked(&self, id: &str) -> Option<bool> {
        Some(match id {
            "Show All" => self.filter == DisplayFilter::All,
            "Show Differences" => matches!(
                self.filter,
                DisplayFilter::Differences | DisplayFilter::Context
            ),
            "Show Same" => self.filter == DisplayFilter::Same,
            "Show Context" => self.context_enabled,
            "Ignore Unimportant Differences" => self.map.rules.ignore_unimportant,
            "Visible Whitespace" => self.show_whitespace,
            "Line Numbers" => self.show_numbers,
            "Syntax Highlighting" => self.show_syntax,
            "Over-Under Layout" => self.stacked,
            "Side-by-Side Layout" => !self.stacked,
            _ => return None,
        })
    }
    fn update_menus(&mut self) {
        use crate::chrome::menubar::{MenuItem, MenuItemKind};
        fn update(view: &TextCompareView, items: &mut [MenuItem]) {
            for item in items {
                if item.action_id == "Align With..." {
                    item.shortcut = None;
                }
                if let MenuItemKind::Submenu(children) = &mut item.kind {
                    update(view, children);
                    item.enabled = true;
                } else {
                    item.enabled = matches!(
                        item.action_id.as_str(),
                        "Home"
                            | "New Tab"
                            | "Close Tab"
                            | "Exit"
                            | "Swap Sides"
                            | "Reload Files"
                            | "Recompare Files"
                            | "Session Settings..."
                            | "Text Compare Info"
                            | "Open File..."
                            | "Save File"
                            | "Save File As..."
                            | "Open Clipboard"
                            | "Undo"
                            | "Redo"
                            | "Cut"
                            | "Copy"
                            | "Paste"
                            | "Delete"
                            | "Select All"
                            | "Copy to Left"
                            | "Copy to Right"
                            | "Copy Line to Left"
                            | "Copy Line to Right"
                            | "Isolate"
                            | "Increase Indent"
                            | "Decrease Indent"
                            | "Select Section"
                            | "Compare Selection to Clipboard"
                            | "Associated Application"
                            | "Explorer"
                            | "Align With..."
                            | "Clear manual alignment"
                            | "Next Difference"
                            | "Previous Difference"
                            | "Next Difference Section"
                            | "Previous Difference Section"
                            | "First difference"
                            | "Last difference"
                            | "Find..."
                            | "Replace..."
                            | "Find Next"
                            | "Find Previous"
                            | "Go To..."
                            | "File Formats..."
                            | "Options..."
                            | "Text display..."
                            | "Save target..."
                            | "File properties..."
                    );
                    if item.action_id == "Compare Parent Folders" {
                        item.enabled = view.parent_folders().is_some() && !view.is_dirty();
                    }
                    if let Some(checked) = view.checked(&item.action_id) {
                        item.kind = MenuItemKind::Check { checked };
                        item.enabled = true;
                    }
                }
            }
        }
        let mut menus = std::mem::take(&mut self.menus);
        if !menus[1]
            .items
            .iter()
            .any(|i| i.action_id == "File properties...")
        {
            for (index, names) in [
                (1, &["Save target...", "File properties..."][..]),
                (
                    2,
                    &[
                        "Copy to Left",
                        "Copy Line to Left",
                        "Clear manual alignment",
                    ][..],
                ),
                (3, &["First difference", "Last difference"][..]),
                (4, &["Text display..."][..]),
            ] {
                menus[index]
                    .items
                    .extend(names.iter().map(|n| MenuItem::action(*n, *n)));
            }
        }
        for menu in &mut menus {
            update(self, &mut menu.items);
        }
        self.menus = menus;
    }
    /// 窗格 `s` 的右键菜单：库存来自参考树，启用状态按该侧选区、写保护与文件路径计算。
    pub(crate) fn context_items(&self, s: usize) -> Vec<crate::chrome::menubar::MenuItem> {
        use crate::chrome::menubar::MenuItemKind;
        let selection = self.editors[s].selection().is_some();
        let writable = !self.content(s).read_only;
        let other_writable = !self.content(1 - s).read_only;
        let has_path = self.files[s].path.is_some();
        let mut items = super::menu::context(s);
        for item in &mut items {
            item.enabled = match item.action_id.as_str() {
                "Align With..." | "Isolate" | "Select Section" | "Open File..."
                | "Open Clipboard" | "Save File As..." | "Open With" => true,
                "Copy to Left" | "Copy to Right" | "Copy Line to Left" | "Copy Line to Right" => {
                    other_writable
                }
                "Increase Indent" | "Decrease Indent" | "Paste" => writable,
                "Cut" | "Delete" => writable && selection,
                "Copy" | "Compare Selection to Clipboard" => selection,
                "Save File" => self.side_dirty(s),
                "Explorer" => has_path,
                _ => false,
            };
            if let MenuItemKind::Submenu(children) = &mut item.kind {
                for child in children {
                    child.enabled = child.action_id == "Associated Application" && has_path;
                }
            }
        }
        items
    }
    /// 右键菜单选择：命令作用于被点击的窗格，并把键盘焦点还给该窗格的编辑器。
    pub(crate) fn context_command(
        &mut self,
        s: usize,
        id: &str,
        ctx: &egui::Context,
        prefs: &Prefs,
    ) {
        self.active = s;
        ctx.memory_mut(|m| m.request_focus(editor_id(s)));
        self.command(id, ctx, prefs);
    }
    fn command(&mut self, id: &str, ctx: &egui::Context, prefs: &Prefs) {
        let old_filter = self.filter;
        match id {
            "To Windows/DOS" | "To Unix" | "To Macintosh" => {
                let ending = match id {
                    "To Windows/DOS" => bc_core::line::LineEnding::CrLf,
                    "To Unix" => bc_core::line::LineEnding::Lf,
                    _ => bc_core::line::LineEnding::Cr,
                };
                match self.content_mut(self.active).convert_endings(ending) {
                    Ok(()) => self.recompare(),
                    Err(e) => self.message = e.to_string(),
                }
            }
            // Home 只切换标签页、不丢弃会话，故无需保存确认。
            "Home" => self.action = Some(ViewAction::Home),
            "Close Tab" => self.request_close(),
            "New Tab" => self.action = Some(ViewAction::NewTab),
            "Exit" => self.request_exit(),
            "First difference" => self.navigate(true, true),
            "Last difference" => self.navigate(false, true),
            "Next Difference" | "Next Difference Section" => self.navigate(true, false),
            "Previous Difference" | "Previous Difference Section" => self.navigate(false, false),
            "Copy to Left" | "Copy to Right" | "Copy to Other" => {
                let s = if id == "Copy to Left" {
                    0
                } else if id == "Copy to Right" {
                    1
                } else {
                    1 - self.active
                };
                if let Err(e) = self.copy_to(s) {
                    self.message = e.to_string();
                }
            }
            "Copy Line to Left" | "Copy Line to Right" => {
                if let Err(e) = self.copy_line_to(usize::from(id == "Copy Line to Right")) {
                    self.message = e.to_string();
                }
            }
            "Isolate" => {
                if let Err(e) = self.isolate(self.active) {
                    self.message = e.to_string();
                }
            }
            "Increase Indent" | "Decrease Indent" => {
                let s = self.active;
                let old = self.content(s).lines.borrow().len();
                let revision = self.content(s).revision();
                let content = if s == 0 {
                    &mut self.map.left
                } else {
                    &mut self.map.right
                };
                match self.editors[s].indent(content, id == "Increase Indent") {
                    Ok(()) if self.content(s).revision() != revision => self.edited(old),
                    Ok(()) => {}
                    Err(e) => self.message = e.to_string(),
                }
            }
            "Select Section" => self.select_section(self.active),
            "Compare Selection to Clipboard" => {
                let s = self.active;
                let text = self.editors[s].selected_text(self.content(s));
                if text.is_empty() {
                    self.message = "Select text to compare with the clipboard".into();
                } else {
                    self.compare_selection = Some((text, ctx.cumulative_pass_nr()));
                    ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                }
            }
            "Associated Application" | "Explorer" => match self.files[self.active].path.clone() {
                Some(path) => {
                    if let Err(e) = shell_open(&path, id == "Explorer") {
                        self.message = format!("Cannot open {}: {e}", path.display());
                    }
                }
                None => self.message = "This side has no file on disk".into(),
            },
            "Swap Sides" => self.swap(),
            "Hex Compare" => {
                let [left, right] = self.paths();
                match (left, right) {
                    (Some(left), Some(right)) => {
                        self.action = Some(ViewAction::CompareAsHex { left, right })
                    }
                    _ => self.message = "Save clipboard sides before opening Hex Compare".into(),
                }
            }
            "Recompare Files" => self.recompare(),
            "Reload Files" => self.request(Pending::ReloadBoth),
            "Find..." => self.find.show(),
            "Replace..." => {
                self.find.show();
                self.find.replace_open = true;
            }
            "Find Next" | "Find Previous" => {
                if let Err(e) = self.find_action(if id == "Find Next" {
                    FindAction::Next
                } else {
                    FindAction::Previous
                }) {
                    self.message = e.to_string();
                }
            }
            "Session Settings..." => {
                let draft = self.session_draft();
                self.session
                    .show(draft, crate::chrome::dialog::Placement::Window);
            }
            "Options..." => self
                .options
                .show(prefs, crate::chrome::dialog::Placement::Window),
            "File Formats..." => self.formats.open = true,
            "Save File" => {
                if let Err(e) = self.save(self.active, prefs) {
                    self.message = e.to_string();
                }
            }
            "Open File..." | "Save File As..." | "Save target..." => {
                self.file_dialog = Some(FileDialog {
                    side: self.active,
                    mode: match id {
                        "Open File..." => FileMode::Open,
                        "Save File As..." => FileMode::SaveAs,
                        _ => FileMode::Target,
                    },
                    path: self.files[self.active]
                        .path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default(),
                })
            }
            "Open Clipboard" => {
                self.clipboard_side = Some(self.active);
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
            }
            "Show All" => self.filter = DisplayFilter::All,
            "Show Differences" => {
                self.filter = if self.context_enabled {
                    DisplayFilter::Context
                } else {
                    DisplayFilter::Differences
                }
            }
            "Show Same" => self.filter = DisplayFilter::Same,
            "Show Context" => {
                self.context_enabled = !self.context_enabled;
                if matches!(
                    self.filter,
                    DisplayFilter::Context | DisplayFilter::Differences
                ) {
                    self.filter = if self.context_enabled {
                        DisplayFilter::Context
                    } else {
                        DisplayFilter::Differences
                    };
                }
            }
            "Ignore Unimportant Differences" => {
                self.map.rules.ignore_unimportant = !self.map.rules.ignore_unimportant;
                self.after_alignment();
            }
            "Over-Under Layout" => self.stacked = true,
            "Side-by-Side Layout" => self.stacked = false,
            "Line Numbers" => self.show_numbers = !self.show_numbers,
            "Visible Whitespace" => self.show_whitespace = !self.show_whitespace,
            "Syntax Highlighting" => self.show_syntax = !self.show_syntax,
            "Align With..." => {
                self.align_with = Some((self.active, self.editors[self.active].caret.line))
            }
            "Clear manual alignment" => {
                self.map.overrides.clear();
                self.recompare();
            }
            "Compare Parent Folders" => {
                if !self.is_dirty() {
                    if let Some([left, right]) = self.parent_folders() {
                        self.action = Some(ViewAction::CompareParentFolders { left, right });
                    }
                }
            }
            "Go To..." => self.show_goto = true,
            "Text display..." => self.show_view_options = true,
            "Text Compare Info" => self.show_info = true,
            "File properties..." => self.show_file_options = true,
            "Undo" | "Redo" | "Select All" | "Delete" | "Cut" | "Copy" | "Paste" => {
                if id == "Paste" {
                    ctx.memory_mut(|m| m.request_focus(editor_id(self.active)));
                    ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                } else {
                    let event = match id {
                        "Cut" => egui::Event::Cut,
                        "Copy" => egui::Event::Copy,
                        _ => egui::Event::Key {
                            key: match id {
                                "Undo" => egui::Key::Z,
                                "Redo" => egui::Key::Y,
                                "Select All" => egui::Key::A,
                                _ => egui::Key::Delete,
                            },
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: egui::Modifiers {
                                command: id != "Delete",
                                ctrl: id != "Delete",
                                ..Default::default()
                            },
                        },
                    };
                    let s = self.active;
                    let old = self.content(s).lines.borrow().len();
                    let content = if s == 0 {
                        &mut self.map.left
                    } else {
                        &mut self.map.right
                    };
                    match self.editors[s].event(content, &event, ctx) {
                        Ok(true) => self.edited(old),
                        Ok(false) => {}
                        Err(e) => self.message = e.to_string(),
                    }
                }
            }
            _ => {}
        }
        if self.filter != old_filter {
            self.expanded.clear();
            self.rebuild_projection();
            self.scroll.y = 0.0;
        }
    }
    pub(crate) fn row_height(&self) -> f32 {
        self.font_size + (15.0 - crate::chrome::theme::EDITOR_SIZE)
    }
    pub(crate) fn header(&mut self, ui: &mut egui::Ui, s: usize, prefs: &Prefs) {
        use crate::chrome::pathbar;
        let top = ui.cursor().min;
        let width = ui.available_width();
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
        let history: Vec<_> = self
            .recent
            .iter()
            .map(|p| p.display().to_string())
            .collect();
        let path_rect = egui::Rect::from_min_size(top, egui::vec2(width, 26.0));
        // 参考路径栏顶端有一条全宽白色高光线，按钮区保持面板灰。
        ui.painter().line_segment(
            [path_rect.left_top(), path_rect.right_top()],
            egui::Stroke::new(1.0, egui::Color32::WHITE),
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .id_salt(("path", s))
                .max_rect(egui::Rect::from_min_max(
                    path_rect.min + egui::vec2(0.0, 1.0),
                    path_rect.max,
                )),
            |ui| {
                let response = pathbar::show_with_history(
                    ui,
                    &mut self.path_edits[s],
                    &history,
                    self.snapshot_select_paths && s == 0,
                );
                if response.submitted {
                    self.request(Pending::Open(s, PathBuf::from(self.path_edits[s].trim())));
                } else {
                    match response.action.as_deref() {
                        Some("save") => {
                            if let Err(e) = self.save(s, prefs) {
                                self.message = e.to_string();
                            }
                        }
                        Some("browse" | "history") => {
                            self.active = s;
                            self.command("Open File...", ui.ctx(), prefs);
                        }
                        _ => {}
                    }
                }
            },
        );
        let info_rect =
            egui::Rect::from_min_size(top + egui::vec2(0.0, 26.0), egui::vec2(width, 27.0));
        ui.scope_builder(
            egui::UiBuilder::new()
                .id_salt(("info", s))
                .max_rect(info_rect.shrink2(egui::vec2(10.0, 3.0))),
            |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    ui.visuals_mut().widgets.inactive.bg_fill = crate::chrome::theme::PANEL_BG;
                    ui.visuals_mut().widgets.inactive.weak_bg_fill = crate::chrome::theme::PANEL_BG;
                    ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::NONE;
                    ui.visuals_mut().override_text_color = Some(crate::chrome::theme::TEXT_MUTED);
                    // Flowing items (no fixed slots): long dates or sizes never overlap.
                    ui.add(egui::Label::new(self.files[s].modified_label.text()).extend());
                    ui.add(egui::Label::new(format!("{} bytes", self.files[s].byte_len)).extend());
                    // 参考在信息栏标出只读文件（如 Perforce 未 checkout 的文件带只读属性）；
                    // 点击打开文件属性，可取消只读以允许在 Matcha 中编辑/覆盖。
                    if self.content(s).read_only
                        && ui
                            .add(
                                egui::Label::new(egui::RichText::new("Read-only").underline())
                                    .sense(egui::Sense::click()),
                            )
                            .on_hover_text(
                                "This side is read-only (the file has the read-only attribute), \
                                 so nothing can be copied or typed into it. \
                                 Click to open File properties and clear Read-only.",
                            )
                            .clicked()
                    {
                        self.active = s;
                        self.show_file_options = true;
                    }
                    let selected = self
                        .content(s)
                        .format()
                        .map_or("Everything Else", |f| f.name.as_str())
                        .to_string();
                    let mut choice = None;
                    let formats: Vec<_> = self
                        .registry
                        .formats()
                        .iter()
                        .filter(|f| f.kind == FormatKind::Text)
                        .cloned()
                        .collect();
                    widgets::Combo::new(("format", s), selected.as_str())
                        .width(110.0)
                        .flat()
                        .show(ui, |list| {
                            if list.selectable_label(false, "Detected") {
                                choice = Some(
                                    self.files[s]
                                        .path
                                        .as_ref()
                                        .and_then(|p| {
                                            self.registry.resolve(p, Some(FormatKind::Text))
                                        })
                                        .cloned(),
                                );
                            }
                            if list.selectable_label(selected == "Everything Else", "Plain text") {
                                choice = Some(None);
                            }
                            list.separator();
                            for f in &formats {
                                if list.selectable_label(selected == f.name, f.name.as_str()) {
                                    choice = Some(Some(f.clone()));
                                }
                            }
                        });
                    if let Some(format) = choice {
                        match self.content_mut(s).set_format(format) {
                            Ok(()) => self.recompare(),
                            Err(e) => self.message = e.to_string(),
                        }
                    }

                    let encoding = super::status_bar::encoding_label(self.content(s));
                    let current = self.content(s).encoding;
                    let mut pick = None;
                    widgets::Combo::new(("encoding", s), encoding)
                        .width(96.0)
                        .flat()
                        .show(ui, |list| {
                            for (label, value) in ENCODINGS {
                                if list.selectable_label(current == value, label) {
                                    pick = Some(value);
                                }
                            }
                        });
                    if let Some(value) = pick {
                        self.request(Pending::Encoding(s, value));
                    }
                    ui.label(super::status_bar::ending_label(self.content(s)));
                });
            },
        );
        ui.allocate_rect(
            egui::Rect::from_min_size(top, egui::vec2(width, 53.0)),
            egui::Sense::hover(),
        );
    }
    fn file_options(&mut self, ui: &mut egui::Ui, s: usize, prefs: &Prefs) {
        ui.horizontal_wrapped(|ui| {
            for (label, mode) in [
                ("Open…", FileMode::Open),
                ("Save as…", FileMode::SaveAs),
                ("Save target…", FileMode::Target),
            ] {
                if widgets::button(ui, label).clicked() {
                    self.file_dialog = Some(FileDialog {
                        side: s,
                        mode,
                        path: self.files[s]
                            .path
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default(),
                    });
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            if widgets::button(ui, "Save").clicked() {
                if let Err(e) = self.save(s, prefs) {
                    self.message = e.to_string();
                }
            }
            if widgets::button(ui, "Reload").clicked() {
                self.request(Pending::Reload(s));
            }
            if widgets::button(ui, "Open Clipboard").clicked() {
                self.clipboard_side = Some(s);
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::RequestPaste);
            }
        });
        widgets::checkbox(ui, &mut self.content_mut(s).read_only, "Read-only");
        widgets::section(ui, "Encoding");
        for (label, encoding) in ENCODINGS {
            if widgets::list_row(ui, self.content(s).encoding == encoding, label).clicked() {
                self.request(Pending::Encoding(s, encoding));
            }
        }
        widgets::section(ui, "Save BOM policy");
        let old = self.content(s).bom_policy;
        ui.horizontal(|ui| {
            for (name, policy) in [
                ("Preserve", BomPolicy::Preserve),
                ("Always", BomPolicy::Always),
                ("Never", BomPolicy::Never),
            ] {
                widgets::radio_value(ui, &mut self.content_mut(s).bom_policy, policy, name);
            }
        });
        if old != self.content(s).bom_policy {
            self.files[s].metadata_dirty = true;
        }
        ui.add_space(4.0);
        let endings = [
            bc_core::line::LineEnding::Lf,
            bc_core::line::LineEnding::CrLf,
            bc_core::line::LineEnding::Cr,
        ];
        let chosen = widgets::menu_button(ui, "Convert line endings", || {
            endings
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    crate::chrome::popup::item::action(format!("{e:?}"), i.to_string(), true)
                })
                .collect()
        });
        if let Some(e) = chosen
            .and_then(|i| i.parse::<usize>().ok())
            .map(|i| endings[i])
        {
            match self.content_mut(s).convert_endings(e) {
                Ok(()) => self.recompare(),
                Err(e) => self.message = e.to_string(),
            }
        }
    }
    fn keyboard(&mut self, ctx: &egui::Context, prefs: &Prefs) {
        let events = ctx.input(|i| i.events.clone());
        // RequestPaste 回送只在随后几帧内有效；剪贴板为空时不会回送，丢弃挂起请求，
        // 以免下一次普通 Ctrl+V 被误当作比较。
        if self
            .compare_selection
            .as_ref()
            .is_some_and(|(_, pass)| ctx.cumulative_pass_nr() > pass + 3)
        {
            self.compare_selection = None;
        }
        for event in events {
            if let egui::Event::Paste(text) = &event {
                if let Some((left, _)) = self.compare_selection.take() {
                    self.action = Some(ViewAction::CompareTexts {
                        left,
                        right: text.clone(),
                    });
                    continue;
                }
                if let Some(s) = self.clipboard_side.take() {
                    self.request(Pending::Clipboard(s, text.clone()));
                    continue;
                }
            }
            if self.pending.is_some() || self.file_dialog.is_some() {
                continue;
            }
            if let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = &event
            {
                let action = match (*key, modifiers.command, modifiers.shift) {
                    (egui::Key::N, true, _) => Some("Next Difference Section"),
                    (egui::Key::P, true, _) => Some("Previous Difference Section"),
                    (egui::Key::G, true, _) => Some("Go To..."),
                    (egui::Key::I, true, false) => Some("Text Compare Info"),
                    (egui::Key::O, true, false) => Some("Open File..."),
                    (egui::Key::H, true, _) => Some("Replace..."),
                    (egui::Key::R, true, false) => Some("Copy to Right"),
                    (egui::Key::R, true, true) => Some("Copy Line to Right"),
                    (egui::Key::L, true, false) => Some("Copy to Left"),
                    (egui::Key::L, true, true) => Some("Copy Line to Left"),
                    (egui::Key::I, true, true) => Some("Increase Indent"),
                    (egui::Key::U, true, true) => Some("Decrease Indent"),
                    (egui::Key::D, true, false) => Some("Select Section"),
                    (egui::Key::V, true, true) => Some("Open Clipboard"),
                    (egui::Key::W, true, _) => Some("Close Tab"),
                    (egui::Key::Q, true, _) => Some("Exit"),
                    (egui::Key::F5, true, _) => Some("Recompare Files"),
                    (egui::Key::F5, false, _) => Some("Reload Files"),
                    (egui::Key::F12, _, _) => Some("Save File As..."),
                    _ => None,
                };
                if let Some(action) = action {
                    self.command(action, ctx, prefs);
                    continue;
                }
                if modifiers.command && *key == egui::Key::F {
                    self.find.show();
                    continue;
                }
                if modifiers.command && *key == egui::Key::S {
                    if let Err(e) = self.save(self.active, prefs) {
                        self.message = e.to_string();
                    }
                    continue;
                }
                if *key == egui::Key::F3 {
                    if let Err(e) = self.find_action(if modifiers.shift {
                        FindAction::Previous
                    } else {
                        FindAction::Next
                    }) {
                        self.message = e.to_string();
                    }
                    continue;
                }
                if *key == egui::Key::F7 {
                    self.navigate(!modifiers.shift, false);
                    continue;
                }
            }
            let s = self.active;
            // 复制是只读操作：焦点被菜单等临时夺走（无任何控件持有焦点）时仍复制活动窗格选区。
            let focused = ctx.memory(|m| m.focused());
            let copy_without_focus = focused.is_none() && matches!(event, egui::Event::Copy);
            if focused != Some(editor_id(s)) && !copy_without_focus {
                continue;
            }
            let old = self.content(s).lines.borrow().len();
            let before = self.editors[s].caret;
            let content = if s == 0 {
                &mut self.map.left
            } else {
                &mut self.map.right
            };
            match self.editors[s].event(content, &event, ctx) {
                Ok(true) => self.edited(old),
                Ok(false) => {}
                Err(e) => self.message = e.to_string(),
            }
            if before != self.editors[s].caret {
                self.current_row = self.row_for_line(s, self.editors[s].caret.line);
                if let Some(line) = index(self.row(self.current_row), 1 - s) {
                    self.editors[1 - s].place(Caret { line, byte: 0 }, false);
                }
                let row = self.projection.display_for(self.current_row);
                let top = self.top_row(self.row_height());
                if row < top || row > top + 30 {
                    self.scroll_to_row(row.saturating_sub(8));
                }
            }
        }
    }
    fn dialogs(&mut self, ctx: &egui::Context, prefs: &Prefs) {
        if let Some(dialog) = self.file_dialog.take() {
            use crate::chrome::native_dialog::{side_title, Kind, Request};
            let (kind, action) = match dialog.mode {
                FileMode::Open => (Kind::OpenFile, "Open File on File System"),
                FileMode::SaveAs => (Kind::SaveFile, "Save File As"),
                FileMode::Target => (Kind::SaveFile, "Save Target"),
            };
            let initial = match dialog.mode {
                FileMode::Target => self.files[dialog.side].target.clone(),
                _ => None,
            }
            .or_else(|| self.files[dialog.side].path.clone())
            .unwrap_or_else(|| PathBuf::from(dialog.path.trim()));
            let picked = Request::new(kind, side_title(action, dialog.side))
                .initial(&initial)
                .show();
            if let Some(path) = picked {
                match dialog.mode {
                    FileMode::Open => self.request(Pending::Open(dialog.side, path)),
                    FileMode::SaveAs | FileMode::Target => {
                        let target = matches!(dialog.mode, FileMode::Target);
                        match self.save_to(dialog.side, &path, prefs, target) {
                            Ok(()) => {
                                if target {
                                    self.files[dialog.side].target = Some(path);
                                }
                            }
                            Err(e) => self.message = e.to_string(),
                        }
                    }
                }
            }
        }
        if let Some(pending) = self.pending.clone() {
            use crate::chrome::{
                controls::Role,
                modal::{Modal, Tone},
            };
            let dirty: Vec<usize> = self
                .affected(&pending)
                .into_iter()
                .filter(|&s| self.side_dirty(s))
                .collect();
            let items = dirty.iter().map(|&s| {
                let badge = if s == 0 { "LEFT" } else { "RIGHT" };
                match &self.files[s].path {
                    Some(path) => crate::chrome::modal::file_item(badge, path),
                    None => crate::chrome::modal::Item {
                        badge: badge.into(),
                        title: "Untitled".into(),
                        detail: String::new(),
                    },
                }
            });
            let title = match pending {
                Pending::Close => "Save changes before closing?",
                Pending::Exit => "Save changes before quitting?",
                Pending::Reload(_) | Pending::ReloadBoth => "Save changes before reloading?",
                Pending::Open(..) => "Save changes before opening another file?",
                Pending::Clipboard(..) => "Save changes before loading the clipboard?",
                Pending::Encoding(..) => "Save changes before changing the encoding?",
            };
            let message = if dirty.len() > 1 {
                "Both sides have unsaved edits. They will be lost if you don't save them."
            } else {
                "This file has unsaved edits. They will be lost if you don't save them."
            };
            let decision = match Modal::new("text-unsaved", Tone::Warning, title)
                .message(message)
                .items(items)
                .action("Don't Save", Role::Destructive)
                .action("Cancel", Role::Secondary)
                .action("Save", Role::Primary)
                .cancel(1)
                .show(ctx)
            {
                Some(0) => 2,
                Some(1) => 3,
                Some(2) => 1,
                _ => 0,
            };
            if decision == 1 {
                let mut saved = true;
                for s in self.affected(&pending) {
                    if self.side_dirty(s) {
                        match self.save(s, prefs) {
                            Ok(true) => {}
                            Ok(false) => {
                                saved = false;
                                break;
                            }
                            Err(e) => {
                                self.message = e.to_string();
                                saved = false;
                                break;
                            }
                        }
                    }
                }
                if saved {
                    self.pending = None;
                    self.perform(pending);
                }
            } else if decision == 2 {
                self.pending = None;
                self.perform(pending);
            } else if decision == 3 {
                self.pending = None;
            }
        }
    }
    pub(crate) fn expand(&mut self, range: Range<usize>) {
        self.expanded.push(range);
        self.rebuild_projection();
    }
}
pub(crate) fn side(s: usize) -> Side {
    if s == 0 {
        Side::Left
    } else {
        Side::Right
    }
}
pub(crate) fn index(row: TextMapRow, s: usize) -> Option<usize> {
    if s == 0 {
        row.left
    } else {
        row.right
    }
}
/// Open With > Associated Application（系统默认程序）或 Explorer（在文件管理器中定位）。
pub(crate) fn shell_open(path: &Path, reveal: bool) -> std::io::Result<()> {
    let path = std::path::absolute(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // explorer 自行解析参数：/select, 后的路径须原样加引号（Windows 路径不含引号）。
        let arg = if reveal {
            format!("/select,\"{}\"", path.display())
        } else {
            format!("\"{}\"", path.display())
        };
        std::process::Command::new("explorer")
            .raw_arg(arg)
            .spawn()
            .map(drop)
    }
    #[cfg(target_os = "macos")]
    {
        let mut command = std::process::Command::new("open");
        if reveal {
            command.arg("-R");
        }
        command.arg(&path).spawn().map(drop)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let target = if reveal {
            path.parent().map(Path::to_path_buf).unwrap_or(path)
        } else {
            path
        };
        std::process::Command::new("xdg-open")
            .arg(target)
            .spawn()
            .map(drop)
    }
}
pub(crate) fn editor_id(s: usize) -> egui::Id {
    egui::Id::unique(("text_editor", s))
}
fn load(path: &Path, registry: &FormatRegistry) -> Result<(TextContent, FileInfo), CoreError> {
    let start = Instant::now();
    let metadata = std::fs::metadata(path)?;
    let mut content = TextContent::open(
        path,
        registry.resolve(path, Some(FormatKind::Text)).cloned(),
        1252,
    )?;
    content.read_only = metadata.permissions().readonly();
    let load_time = start.elapsed();
    let modified_label = super::status_bar::ModifiedLabel::resolve(metadata.modified().ok());
    Ok((
        content,
        FileInfo {
            path: Some(path.to_path_buf()),
            byte_len: metadata.len() as usize,
            modified_label,
            load_time,
            ..Default::default()
        },
    ))
}
fn backup_path(path: &Path, suffix: Option<&str>) -> Result<PathBuf, CoreError> {
    let name = path
        .file_name()
        .ok_or_else(|| CoreError::Invalid("save path has no file name".into()))?;
    let mut backup = std::ffi::OsString::new();
    if let Some(suffix) = suffix {
        if suffix.is_empty() || suffix.contains(['/', '\\']) {
            return Err(CoreError::Invalid(
                "backup suffix must be nonempty and contain no separators".into(),
            ));
        }
        backup.push(name);
        backup.push(suffix);
    } else {
        backup.push("~");
        backup.push(name);
    }
    Ok(path.with_file_name(backup))
}
