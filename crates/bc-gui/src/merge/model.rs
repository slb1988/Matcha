//! 合并会话的数据模型（不依赖 egui）：三方行表、合并状态与可编辑的输出缓冲。
//!
//! 输出缓冲是一个独立的 `TextContent`。每个输出行记着来源（左 / 右 / 中 / 手工）和所属的
//! 对齐行，四个窗格据此共用一张行网格：某对齐行在输出里有多行时输入窗格显示空隙，没有
//! 行时输出显示空隙。Take、接受与手工编辑都走同一个快照式撤销栈。
use crate::text::editor::{Caret, Editor};
use bc_core::{
    encoding::Detection,
    line::{LineEnding, LineList},
    CoreError, Side,
};
use bc_engine_text::{
    merge::{output_encoding, MergeState, Origin},
    rules::{Favor, TextMergeRules},
    TextContent, TextMap,
};
use bc_script::cli::MergeSpec;
use std::{
    ops::Range,
    path::{Path, PathBuf},
};

/// 启动一个合并会话所需的一切（命令行或单实例转交）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeLaunch {
    pub spec: MergeSpec,
    /// `/title1..4`：left、right、center、output。
    pub titles: [Option<String>; 4],
    /// 仅由 `/automerge /reviewconflicts` 转入时带入，使界面与自动合并的判定一致。
    pub ignore_unimportant: bool,
    pub favor: Favor,
    /// 表格合并（xlsx/xlsm，或 `/tablemerge` 的文本表格）：打开表格合并视图而非文本合并。
    pub table: bool,
    /// `/keys=`：表格合并的键列（0 起）。
    pub keys: Option<Vec<usize>>,
}

impl MergeLaunch {
    pub fn new(spec: MergeSpec) -> Self {
        Self {
            spec,
            titles: Default::default(),
            ignore_unimportant: false,
            favor: Favor::None,
            table: false,
            keys: None,
        }
    }
}

/// 输出行的来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutOrigin {
    Center,
    Left,
    Right,
    /// 用户在输出窗格里编辑或新增的行。
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutLine {
    pub origin: OutOrigin,
    /// 所属对齐行。
    pub row: usize,
}

/// 行网格的一行：对齐行 `row` 的一个显示行。输入窗格只在 `own` 的那一行显示自己的内容
/// （该对齐行的第一个非手工输出行；全是手工行或没有输出时为第一行），其余是空隙。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridRow {
    pub row: usize,
    pub own: bool,
    /// 这一显示行上的输出行号。
    pub out: Option<usize>,
}

struct Snapshot {
    lines: LineList,
    meta: Vec<OutLine>,
    resolved: Vec<bool>,
    caret: Caret,
}

/// 撤销栈最多保留的步数（每步是整份行元数据的拷贝）。
const UNDO_LIMIT: usize = 200;

pub struct MergeModel {
    pub map: TextMap,
    pub state: MergeState,
    pub output: TextContent,
    /// 输出目标；`None` 时保存即另存。
    pub target: Option<PathBuf>,
    meta: Vec<OutLine>,
    grid: Vec<GridRow>,
    /// 每个对齐行在网格里的首个显示行。
    grid_of_row: Vec<usize>,
    /// 每个输出行所在的网格显示行。
    grid_of_out: Vec<usize>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    /// 自上次保存以来输出有改动。
    changed: bool,
    /// `Some(有未解决冲突)` = 已保存过，值为最后一次保存时的状态。
    saved: Option<bool>,
}

fn origin_of(side: Side) -> OutOrigin {
    match side {
        Side::Left => OutOrigin::Left,
        Side::Right => OutOrigin::Right,
        _ => OutOrigin::Center,
    }
}

impl MergeModel {
    /// 读入三份输入并建立合并。输入整读进内存（不映射），所以输出目标可以是某个输入。
    pub fn load(launch: &MergeLaunch) -> Result<Self, String> {
        let formats = bc_script::runner::default_formats();
        let format = launch
            .spec
            .center
            .file_name()
            .and_then(|name| formats.resolve(Path::new(name), Some(bc_formats::FormatKind::Text)))
            .cloned();
        let load = |path: &Path| -> Result<TextContent, String> {
            if path.is_dir() {
                return Err(format!("{} is a folder; Text Merge needs three files", path.display()));
            }
            if !path.is_file() {
                return Err(format!("{} does not exist", path.display()));
            }
            let bytes = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            let mut content = TextContent::from_bytes(bytes, format.clone())
                .map_err(|_| format!("{} is not a text file", path.display()))?;
            content.source = Some(bc_vfs::VfsPath::local(path));
            content.read_only = true;
            Ok(content)
        };
        let (left, right, center) = (
            load(&launch.spec.left)?,
            load(&launch.spec.right)?,
            load(&launch.spec.center)?,
        );
        let rules = TextMergeRules {
            ignore_unimportant: launch.ignore_unimportant,
            favor: launch.favor,
            ..Default::default()
        };
        let mut model = Self::from_contents(left, right, center, rules).map_err(|e| e.to_string())?;
        model.target = launch.spec.output.clone();
        Ok(model)
    }

    pub fn from_contents(
        left: TextContent,
        right: TextContent,
        center: TextContent,
        rules: TextMergeRules,
    ) -> Result<Self, CoreError> {
        let format = center.format().cloned();
        let map = TextMap::merge(left, right, center, rules.text.clone())?;
        let state = MergeState::classify(&map, &rules)?;
        let lines = state.output_lines(&map, None);
        let mut bytes = Vec::new();
        for line in &lines {
            bytes.extend_from_slice(&line.text);
            bytes.extend_from_slice(line.ending.as_bytes());
        }
        // 内部文本恒为 UTF-8；保存时再按三方取舍得到的编码与 BOM 写出。
        let mut output = TextContent::from_bytes(bytes, format)?;
        let ((encoding, has_bom), _) = output_encoding(&map);
        output.encoding = encoding;
        output.detection = Some(Detection {
            encoding,
            has_bom,
            confidence: 100,
        });
        output.edits.clear();
        output.edits.set_limit(1);
        let meta = lines
            .iter()
            .map(|line| OutLine {
                origin: match line.origin {
                    Origin::Left => OutOrigin::Left,
                    Origin::Right => OutOrigin::Right,
                    Origin::Center | Origin::Marker => OutOrigin::Center,
                },
                row: line.row,
            })
            .collect();
        let mut model = Self {
            map,
            state,
            output,
            target: None,
            meta,
            grid: Vec::new(),
            grid_of_row: Vec::new(),
            grid_of_out: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            changed: false,
            saved: None,
        };
        model.rebuild_grid();
        Ok(model)
    }

    fn rebuild_grid(&mut self) {
        debug_assert_eq!(self.meta.len(), self.output.lines.borrow().len());
        self.grid.clear();
        self.grid_of_row.clear();
        let mut next = 0;
        for row in 0..self.map.row_count() {
            self.grid_of_row.push(self.grid.len());
            let start = next;
            while next < self.meta.len() && self.meta[next].row == row {
                next += 1;
            }
            // 手工行插在本行内容之前时，输入窗格仍与本行原有的那一行对齐。
            let own = (start..next)
                .find(|&i| self.meta[i].origin != OutOrigin::Manual)
                .unwrap_or(start);
            for line in start..next {
                self.grid.push(GridRow {
                    row,
                    own: line == own,
                    out: Some(line),
                });
            }
            if next == start {
                self.grid.push(GridRow {
                    row,
                    own: true,
                    out: None,
                });
            }
        }
        // 空的三方输入：输出里手工加的行仍要有地方显示。
        while next < self.meta.len() {
            self.grid.push(GridRow {
                row: self.meta[next].row,
                own: false,
                out: Some(next),
            });
            next += 1;
        }
        self.grid_of_out = vec![0; self.meta.len()];
        for (index, cell) in self.grid.iter().enumerate() {
            if let Some(line) = cell.out {
                self.grid_of_out[line] = index;
            }
        }
    }

    pub fn grid(&self) -> &[GridRow] {
        &self.grid
    }
    /// 对齐行在网格里的首个显示行。
    pub fn grid_row_of(&self, row: usize) -> usize {
        self.grid_of_row
            .get(row)
            .copied()
            .unwrap_or_else(|| self.grid.len().saturating_sub(1))
    }
    pub fn out_line(&self, index: usize) -> Option<OutLine> {
        self.meta.get(index).copied()
    }
    /// 输出行所在的网格显示行。
    pub fn grid_row_of_out(&self, line: usize) -> usize {
        self.grid_of_out
            .get(line)
            .copied()
            .unwrap_or_else(|| self.grid.len().saturating_sub(1))
    }
    /// 对齐行的首个输出行；该行没有输出时取其后最近的输出行。
    pub fn out_line_of_row(&self, row: usize) -> usize {
        self.meta.partition_point(|m| m.row < row)
    }
    pub fn unresolved(&self) -> usize {
        self.state.conflicts().len()
    }
    /// `row` 所在的区段（冲突区段优先，否则改动段）。
    pub fn section_at(&self, row: usize) -> Option<Range<usize>> {
        self.state.section_at(row)
    }
    /// 从对齐行 `from` 出发的下一个 / 上一个未解决冲突区段的首行。
    pub fn next_conflict(&self, from: usize, forward: bool) -> Option<usize> {
        let conflicts = self.state.conflicts();
        if forward {
            conflicts
                .iter()
                .map(|c| c.rows.start)
                .find(|&start| start > from)
        } else {
            conflicts
                .iter()
                .rev()
                .find(|c| c.rows.end <= from)
                .map(|c| c.rows.start)
        }
    }

    fn snapshot(&self, caret: Caret) -> Snapshot {
        Snapshot {
            lines: self.output.lines.borrow().clone(),
            meta: self.meta.clone(),
            resolved: self.state.conflict_sections().map(|(_, r)| r).collect(),
            caret,
        }
    }
    fn restore(&mut self, snapshot: Snapshot) -> Caret {
        *self.output.lines.borrow_mut() = snapshot.lines;
        self.meta = snapshot.meta;
        let sections: Vec<Range<usize>> = self.state.conflict_sections().map(|(r, _)| r).collect();
        for (rows, resolved) in sections.iter().zip(snapshot.resolved) {
            self.state.set_resolved(rows, resolved);
        }
        self.rebuild_grid();
        self.changed = true;
        snapshot.caret
    }
    fn record(&mut self, before: Snapshot) {
        self.undo.push(before);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.changed = true;
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn undo(&mut self, editor: &mut Editor) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo.push(self.snapshot(editor.caret));
        editor.place(self.restore(previous), false);
        editor.clamp(&self.output);
        true
    }
    pub fn redo(&mut self, editor: &mut Editor) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(self.snapshot(editor.caret));
        editor.place(self.restore(next), false);
        editor.clamp(&self.output);
        true
    }

    fn side_line(&self, side: Side, row: usize) -> Option<(Vec<u8>, LineEnding)> {
        let mapped = self.map.row(row)?;
        let (content, index) = match side {
            Side::Left => (&self.map.left, mapped.left?),
            Side::Right => (&self.map.right, mapped.right?),
            Side::Center => (self.map.center.as_ref()?, mapped.center?),
            Side::Output => return None,
        };
        let lines = content.lines.borrow();
        Some((lines.line_bytes(index)?.to_vec(), lines.ending(index)?))
    }

    /// Take：把 `rows` 的输出整体换成 `side`（left / right / center）在这些行上的内容，
    /// 覆盖手工行，并清除涉及的冲突标记。
    pub fn take(&mut self, rows: Range<usize>, side: Side, editor: &mut Editor) {
        let before = self.snapshot(editor.caret);
        let fallback = self.output.lines.borrow().dominant_ending();
        let mut bytes = Vec::new();
        let mut meta = Vec::new();
        for row in rows.clone() {
            if let Some((text, ending)) = self.side_line(side, row) {
                bytes.extend_from_slice(&text);
                // 来源行可能是其文件的无行尾末行；替换块内部不能留下无行尾的行。
                bytes.extend_from_slice(if ending == LineEnding::None {
                    fallback.as_bytes()
                } else {
                    ending.as_bytes()
                });
                meta.push(OutLine {
                    origin: origin_of(side),
                    row,
                });
            }
        }
        let lo = self.meta.partition_point(|m| m.row < rows.start);
        let hi = self.meta.partition_point(|m| m.row < rows.end);
        self.output
            .lines
            .borrow_mut()
            .replace_range(lo..hi, &LineList::from_bytes(bytes));
        self.meta.splice(lo..hi, meta);
        self.state.take(rows, side);
        self.rebuild_grid();
        self.record(before);
        editor.place(
            Caret {
                line: lo.min(self.meta.len().saturating_sub(1)),
                byte: 0,
            },
            false,
        );
        editor.clamp(&self.output);
    }

    /// 接受当前内容：不改输出，只把涉及的冲突区段标为已解决。
    pub fn accept(&mut self, rows: &Range<usize>, editor: &Editor) {
        let before = self.snapshot(editor.caret);
        self.state.accept(rows);
        self.record(before);
    }

    /// 在输出缓冲上执行一次编辑（`apply` 返回是否改了文本）。改动的行标为手工，涉及的冲突
    /// 区段视为已解决；整个编辑是一个撤销步。
    pub fn edit(
        &mut self,
        editor: &mut Editor,
        apply: impl FnOnce(&mut Editor, &mut TextContent) -> Result<bool, CoreError>,
    ) -> Result<bool, CoreError> {
        editor.clamp(&self.output);
        let before = self.snapshot(editor.caret);
        let (first, last) = editor
            .selection()
            .map_or((editor.caret.line, editor.caret.line), |(a, b)| (a.line, b.line));
        if !apply(editor, &mut self.output)? {
            return Ok(false);
        }
        let new = self.output.lines.borrow().clone();
        let old = &before.lines;
        let same = |o: usize, n: usize| {
            old.line_bytes(o) == new.line_bytes(n) && old.ending(o) == new.ending(n)
        };
        // 编辑器只动选区所在的行，外加与上一行 / 下一行的合并；其余行原样平移。
        let shared = old.len().min(new.len());
        let mut prefix = first.min(editor.caret.line).saturating_sub(1).min(shared);
        while prefix < shared && same(prefix, prefix) {
            prefix += 1;
        }
        let mut suffix = old.len().saturating_sub(last + 2).min(shared - prefix);
        while suffix < shared - prefix && same(old.len() - 1 - suffix, new.len() - 1 - suffix) {
            suffix += 1;
        }
        let old_window = prefix..old.len() - suffix;
        let new_window = prefix..new.len() - suffix;
        let row_at = |i: usize| -> usize {
            if before.meta.is_empty() {
                0
            } else if old_window.is_empty() {
                // 纯插入：附着在它前面那一行所属的对齐行上。
                before.meta[prefix.saturating_sub(1).min(before.meta.len() - 1)].row
            } else {
                before.meta[(prefix + i).min(old_window.end - 1)].row
            }
        };
        let window: Vec<OutLine> = (0..new_window.len())
            .map(|i| OutLine {
                origin: OutOrigin::Manual,
                row: row_at(i),
            })
            .collect();
        let mut touched: Vec<usize> = before.meta[old_window.clone()].iter().map(|m| m.row).collect();
        touched.extend(window.iter().map(|m| m.row));
        self.meta.splice(old_window, window);
        for row in touched {
            self.state.accept(&(row..row + 1));
        }
        self.rebuild_grid();
        self.record(before);
        Ok(true)
    }

    /// 输出窗格当前内容按输出编码得到的字节。
    pub fn output_bytes(&self) -> Result<Vec<u8>, CoreError> {
        self.output.to_bytes()
    }

    /// 写出到 `path`（临时文件 + 重命名），并把它记为输出目标。不建备份，不读既有内容。
    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        let bytes = self.output_bytes().map_err(|e| e.to_string())?;
        bc_script::runner::replace_file(path, &bytes)?;
        self.target = Some(path.to_path_buf());
        self.saved = Some(self.unresolved() > 0);
        self.changed = false;
        Ok(())
    }

    /// 有输出目标的会话自打开起就算未保存；没有目标时改过才算。
    pub fn is_dirty(&self) -> bool {
        self.changed || (self.target.is_some() && self.saved.is_none())
    }

    /// 标签页关闭时回给调用方的结果码：0 已保存且无冲突，14 带未解决冲突保存，101 从未保存。
    pub fn result_code(&self) -> i32 {
        match self.saved {
            Some(false) => bc_script::ExitCode::Success.code(),
            Some(true) => bc_script::ExitCode::ConflictsDetected.code(),
            None => bc_script::ExitCode::ConflictsMergeNotWritten.code(),
        }
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
pub(crate) mod tests;
