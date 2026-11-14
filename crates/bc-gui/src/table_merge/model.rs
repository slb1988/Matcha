//! 表格合并会话的数据模型（不依赖 egui）：引擎的 `TableMerge`、按 sheet 缓存的冲突区段，
//! 以及以 `Resolution` 快照实现的撤销栈。合并语义全部来自 `bc_engine_table::merge`，读入与
//! 写出来自 `bc_script::table_merge`（与 `/automerge` 同一条路径）。
//!
//! 引擎的 `conflicts()` / `sections()` 是 O(行×列)：只在打开与每次操作后按 sheet 重算，
//! 绘制时只读缓存。
use crate::merge::MergeLaunch;
use bc_core::Side;
use bc_engine_table::{
    merge::{Conflict, Resolution, SheetMerge},
    Cell,
};
use bc_script::table_merge::{load, options, write_output, LoadedTableMerge};
use std::{
    ops::Range,
    path::{Path, PathBuf},
};

/// 一个 sheet 的冲突缓存。
#[derive(Debug, Clone, Default)]
struct SheetCache {
    /// 全部冲突区段（含已解决）与是否仍未解决。
    sections: Vec<(Range<usize>, bool)>,
    /// 未解决的冲突区段。
    conflicts: Vec<Conflict>,
}

/// 撤销栈最多保留的步数。
const UNDO_LIMIT: usize = 200;

pub struct TableMergeModel {
    pub loaded: LoadedTableMerge,
    pub launch: MergeLaunch,
    /// 输出目标；`None` 时保存即另存。
    pub target: Option<PathBuf>,
    /// 当前显示的 sheet。
    pub sheet: usize,
    cache: Vec<SheetCache>,
    undo: Vec<(usize, Resolution)>,
    redo: Vec<(usize, Resolution)>,
    /// 自上次保存以来输出有改动。
    changed: bool,
    /// `Some(有未解决冲突)` = 已保存过，值为最后一次保存时的状态。
    saved: Option<bool>,
    /// 最近一次保存的非致命提示（宏、样式未保留）。
    pub save_warnings: Vec<String>,
}

impl TableMergeModel {
    /// 读入三方表格并建立合并。
    pub fn load(launch: &MergeLaunch) -> Result<Self, String> {
        let options = options(launch.keys.clone(), launch.ignore_unimportant, launch.favor);
        let loaded = load(&launch.spec, &options)?;
        Ok(Self::from_loaded(loaded, launch.clone()))
    }

    pub fn from_loaded(loaded: LoadedTableMerge, launch: MergeLaunch) -> Self {
        let mut model = Self {
            target: launch.spec.output.clone(),
            loaded,
            launch,
            sheet: 0,
            cache: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            changed: false,
            saved: None,
            save_warnings: Vec::new(),
        };
        model.cache = (0..model.sheets().len())
            .map(|i| Self::compute(&model.sheets()[i]))
            .collect();
        // 打开即停在第一个有冲突的 sheet。
        if let Some(first) = (0..model.cache.len()).find(|&i| !model.cache[i].conflicts.is_empty())
        {
            model.sheet = first;
        }
        model
    }

    fn compute(sheet: &SheetMerge) -> SheetCache {
        SheetCache {
            sections: sheet.sections(),
            conflicts: sheet.conflicts(),
        }
    }

    fn refresh(&mut self, sheet: usize) {
        self.cache[sheet] = Self::compute(&self.loaded.merge.sheets[sheet]);
    }

    pub fn sheets(&self) -> &[SheetMerge] {
        &self.loaded.merge.sheets
    }

    /// 当前 sheet。
    pub fn current(&self) -> &SheetMerge {
        &self.loaded.merge.sheets[self.sheet]
    }

    pub fn set_sheet(&mut self, sheet: usize) {
        if sheet < self.cache.len() {
            self.sheet = sheet;
        }
    }

    /// 全部 sheet 的未解决冲突区段数。
    pub fn unresolved(&self) -> usize {
        self.cache.iter().map(|c| c.conflicts.len()).sum()
    }

    /// 某个 sheet 的未解决冲突区段数。
    pub fn unresolved_in(&self, sheet: usize) -> usize {
        self.cache.get(sheet).map_or(0, |c| c.conflicts.len())
    }

    /// 当前 sheet 的未解决冲突区段。
    pub fn conflicts(&self) -> &[Conflict] {
        &self.cache[self.sheet].conflicts
    }

    /// 当前 sheet 上 `row` 所在的冲突区段（已解决的也算）；整张 sheet 的删改冲突为全部行。
    pub fn section_at(&self, row: usize) -> Option<Range<usize>> {
        let sheet = self.current();
        if sheet.sheet_deleted_by().is_some() {
            return Some(0..sheet.row_count());
        }
        let sections = &self.cache[self.sheet].sections;
        let i = sections.partition_point(|(rows, _)| rows.end <= row);
        sections
            .get(i)
            .filter(|(rows, _)| rows.contains(&row))
            .map(|(rows, _)| rows.clone())
    }

    /// `row` 位于未解决的冲突区段内。
    pub fn in_conflict(&self, row: usize) -> bool {
        self.cache[self.sheet]
            .conflicts
            .iter()
            .any(|c| c.sheet || c.rows.contains(&row))
    }

    /// 当前 sheet 从 `from` 出发的下一个 / 上一个未解决冲突区段的首行。
    pub fn next_conflict(&self, from: usize, forward: bool) -> Option<usize> {
        let conflicts = self.conflicts();
        if forward {
            conflicts.iter().map(|c| c.rows.start).find(|&s| s > from)
        } else {
            conflicts
                .iter()
                .rev()
                .find(|c| c.rows.end <= from)
                .map(|c| c.rows.start)
        }
    }

    /// 当前 sheet 之后 / 之前第一个还有未解决冲突的 sheet。
    pub fn next_conflict_sheet(&self, forward: bool) -> Option<usize> {
        let n = self.cache.len();
        if forward {
            (self.sheet + 1..n).find(|&i| self.unresolved_in(i) > 0)
        } else {
            (0..self.sheet).rev().find(|&i| self.unresolved_in(i) > 0)
        }
    }

    fn act(&mut self, f: impl FnOnce(&mut SheetMerge) -> bool) -> bool {
        let index = self.sheet;
        let before = self.loaded.merge.sheets[index].resolution.clone();
        if !f(&mut self.loaded.merge.sheets[index]) {
            return false;
        }
        if self.loaded.merge.sheets[index].resolution == before {
            return false;
        }
        self.undo.push((index, before));
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.changed = true;
        self.refresh(index);
        true
    }

    /// 取 `side` 解决 `rows` 内的冲突（结构冲突决定行去留，其余替换冲突格）。
    pub fn take(&mut self, rows: Range<usize>, side: Side) -> bool {
        self.act(|s| {
            s.take(rows, side);
            true
        })
    }

    /// 单个格取 `side`。
    pub fn take_cell(&mut self, row: usize, column: usize, side: Side) -> bool {
        self.act(|s| {
            s.take_cell(row, column, side);
            true
        })
    }

    /// 接受 `rows` 内冲突的当前内容。
    pub fn accept(&mut self, rows: Range<usize>) -> bool {
        self.act(|s| {
            s.accept(rows);
            true
        })
    }

    /// 在输出格里输入 `text`（按 center 的区域设置解析为数字 / 日期等）。`false` = 该行不在输出里。
    pub fn edit(&mut self, row: usize, column: usize, text: &str) -> bool {
        let regional = self.current().center.regional.clone();
        let value = bc_engine_table::regional::interpret(text, &regional);
        self.act(|s| s.edit(row, column, Cell::new(value)))
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// 撤销上一步；返回被改动的 sheet。
    pub fn undo(&mut self) -> Option<usize> {
        let (sheet, resolution) = self.undo.pop()?;
        let current =
            std::mem::replace(&mut self.loaded.merge.sheets[sheet].resolution, resolution);
        self.redo.push((sheet, current));
        self.changed = true;
        self.refresh(sheet);
        self.sheet = sheet;
        Some(sheet)
    }

    pub fn redo(&mut self) -> Option<usize> {
        let (sheet, resolution) = self.redo.pop()?;
        let current =
            std::mem::replace(&mut self.loaded.merge.sheets[sheet].resolution, resolution);
        self.undo.push((sheet, current));
        self.changed = true;
        self.refresh(sheet);
        self.sheet = sheet;
        Some(sheet)
    }

    /// 写出到 `path`（临时文件 + 重命名），并把它记为输出目标。未解决冲突格按默认内容写出。
    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        let sheets = self.loaded.merge.output();
        let spec = &self.launch.spec;
        self.save_warnings = write_output(
            &sheets,
            &self.loaded,
            path,
            &[&spec.left, &spec.right, &spec.center],
        )?;
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
