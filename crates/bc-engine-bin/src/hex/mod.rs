//! Hex Compare 引擎（P2）– `THexMap`/`THexRules`/`THexInfoDialog` 对应物。
//!
//! - [`HexRules`]：会话规则（设计 D6 定稿的五字段）。
//! - [`HexMap::align`] / [`HexMap::set_alignment`]：三档字节对齐与 revision 过期拒绝。
//! - [`pack_rows`]：cell packing 派生行模型（设计 D2，证据见参考文档）。
//! - [`content::HexContent`]：加载/保存与写入门禁（设计 D3/D4）。

pub mod content;
pub mod jobs;

pub use content::HexContent;
pub use jobs::{HexAlignJob, HexAlignOutput};

use std::ops::Range;

use bc_core::align::{AlignMap, AlignRow, ByteAlign, ByteAlignStrategy};
use bc_core::cancel::CancelCheck;
use bc_core::{CoreError, DiffKind};

/// Hex session rules – `THexRules`（设计 D6 定稿的字段集）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexRules {
    /// 三档对齐策略（默认 Complete）– BC `THexAlignKind`。
    pub alignment: ByteAlignStrategy,
    /// 每行单元格数（默认 16；Fit to window 开启时由视图按宽度重算）。
    pub bytes_per_line: u16,
    /// hex 区分组字节数（默认 8，即 8+8 分组显示）。
    pub bytes_per_group: u16,
    /// Show Differences 过滤的上下文字节数（默认 16）。
    pub context_count: usize,
    /// 小文件整载阈值（默认 2 MB）：低于阈值不加文件锁，高者 mmap 只读。
    pub lock_threshold: u64,
}

impl Default for HexRules {
    fn default() -> Self {
        Self {
            alignment: ByteAlignStrategy::Complete,
            bytes_per_line: 16,
            bytes_per_group: 8,
            context_count: 16,
            lock_threshold: 2 * 1024 * 1024,
        }
    }
}

/// 显示过滤（View > Show All/Differences/Same）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplayFilter {
    /// 显示全部（默认）。
    #[default]
    All,
    /// 只看差异（带 context_count 上下文，按区域起点重新打包行）。
    Differences,
    /// 只看相同。
    Same,
}

/// 四分类字节统计 – BC `THexInfoDialog` 的 same/difference/left orphan/right orphan。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HexStats {
    pub same: u64,
    pub difference: u64,
    pub left_orphan: u64,
    pub right_orphan: u64,
}

impl HexStats {
    /// 状态栏汇总文案（参考工具：`Binary same` / `Binary differences`）。
    pub fn summary(&self) -> &'static str {
        if self.difference == 0 && self.left_orphan == 0 && self.right_orphan == 0 {
            "Binary same"
        } else {
            "Binary differences"
        }
    }
}

/// 连续同种非 Same 对齐行（差异游程）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffRun {
    pub rows: Range<usize>,
    pub kind: DiffKind,
}

/// 对齐图摘要：四分类统计 + 差异游程。
///
/// 后台对齐时一并算出（[`HexAlignOutput::summary`]），视图逐帧复用而不再扫描每个对齐行
///（大文件对齐行数以千万计，逐帧扫描会把界面拖到个位数帧率）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HexSummary {
    pub stats: HexStats,
    pub runs: Vec<DiffRun>,
    /// 对齐行总数。
    pub rows: usize,
}

impl HexSummary {
    pub fn of(map: &AlignMap) -> Self {
        let mut summary = Self { rows: map.len(), ..Default::default() };
        for (i, row) in map.rows().iter().enumerate() {
            match row.kind {
                DiffKind::Same => {
                    summary.stats.same += 1;
                    continue;
                }
                DiffKind::Changed => summary.stats.difference += 1,
                DiffKind::LeftOnly => summary.stats.left_orphan += 1,
                DiffKind::RightOnly => summary.stats.right_orphan += 1,
            }
            match summary.runs.last_mut() {
                Some(run) if run.rows.end == i && run.kind == row.kind => run.rows.end = i + 1,
                _ => summary.runs.push(DiffRun { rows: i..i + 1, kind: row.kind }),
            }
        }
        summary
    }

    /// 与 [`filtered_regions`] 等价的过滤区间，但只遍历差异游程（O(游程数)）。
    pub fn regions(&self, filter: DisplayFilter, context: usize) -> Vec<Range<usize>> {
        let total = self.rows;
        let mut regions: Vec<Range<usize>> = Vec::new();
        match filter {
            DisplayFilter::All => {
                if total > 0 {
                    regions.push(0..total);
                }
            }
            DisplayFilter::Differences => {
                for run in &self.runs {
                    let start = run.rows.start.saturating_sub(context);
                    let end = (run.rows.end + context).min(total);
                    match regions.last_mut() {
                        Some(last) if start <= last.end => last.end = last.end.max(end),
                        _ => regions.push(start..end),
                    }
                }
            }
            DisplayFilter::Same => {
                let mut at = 0usize;
                for run in &self.runs {
                    if run.rows.start > at {
                        regions.push(at..run.rows.start);
                    }
                    at = run.rows.end;
                }
                if at < total {
                    regions.push(at..total);
                }
            }
        }
        regions
    }
}

/// Aligned byte map – `THexMap`.
#[derive(Debug, Default)]
pub struct HexMap {
    pub left: HexContent,
    pub right: HexContent,
    pub rules: HexRules,
    pub align: AlignMap,
}

impl HexMap {
    /// Run `bc_core::align::ByteAlign` with the configured strategy and store the result.
    pub fn align(&mut self) {
        self.align = ByteAlign.align(self.left.bytes(), self.right.bytes(), self.rules.alignment);
    }

    /// Cancellable variant for background workers（过期结果由调用方按 revision 丢弃）。
    pub fn align_cancellable(&mut self, cancel: &dyn CancelCheck) -> Result<(), CoreError> {
        self.align = ByteAlign.align_cancellable(
            self.left.bytes(),
            self.right.bytes(),
            self.rules.alignment,
            cancel,
        )?;
        Ok(())
    }

    /// 回填后台对齐结果；revision 或规则不一致即过期，拒绝并保留现有对齐（设计 D4）。
    pub fn set_alignment(&mut self, output: HexAlignOutput) -> Result<(), CoreError> {
        if output.revisions != (self.left.revision(), self.right.revision()) || output.rules != self.rules {
            return Err(CoreError::Invalid("stale alignment result".into()));
        }
        self.align = output.map;
        Ok(())
    }

    /// 当前视图模型是否与最新内容同步（不同步则应投递重对齐 job）。
    pub fn is_stale(&self, revisions: (u64, u64)) -> bool {
        revisions != (self.left.revision(), self.right.revision())
    }

    /// 从对齐图汇总四分类统计；每侧合计等于该侧字节数。
    pub fn stats(&self) -> HexStats {
        let mut stats = HexStats::default();
        for row in self.align.rows() {
            match row.kind {
                DiffKind::Same => stats.same += 1,
                DiffKind::Changed => stats.difference += 1,
                DiffKind::LeftOnly => stats.left_orphan += 1,
                DiffKind::RightOnly => stats.right_orphan += 1,
            }
        }
        stats
    }

    /// 派生 cell packing 行模型（设计 D2）：对齐图按每行 N 格打包，孤儿字节占格、
    /// 对侧为斜纹占位格；行标签 = 该侧本行首个真实字节偏移（全占位行无标签）。
    /// 奇数长度孤儿段导致的行相位偏移由该模型自然产生（证据：`24-odd-insert-all.png`）。
    pub fn pack_rows(&self) -> Vec<HexRow> {
        pack_rows(&self.align, self.rules.bytes_per_line)
    }
}

/// 一个单元格：配对字节同格相邻；孤儿字节占格时另一侧画斜纹占位格。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HexCell {
    /// 配对字节（kind 为 Same 或 Changed）。
    Pair { left: usize, right: usize, kind: DiffKind },
    /// 左孤儿字节（右侧占位）。
    LeftOrphan(usize),
    /// 右孤儿字节（左侧占位）。
    RightOrphan(usize),
}

/// 一行打包格。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexRow {
    pub cells: Vec<HexCell>,
    /// 左侧行标签（本行首个真实字节的文件偏移；整行占位时为 None）。
    pub left_label: Option<usize>,
    /// 右侧行标签。
    pub right_label: Option<usize>,
}

/// 把对齐图按每行 `bytes_per_line` 格打包（0 视为 16）。
pub fn pack_rows(map: &AlignMap, bytes_per_line: u16) -> Vec<HexRow> {
    pack_slice(map.rows(), bytes_per_line)
}

/// 打包任意对齐行切片（过滤区域重打包用；相位从切片起点重新起算）。
pub fn pack_slice(rows: &[AlignRow], bytes_per_line: u16) -> Vec<HexRow> {
    let per_line = usize::from(if bytes_per_line == 0 { 16 } else { bytes_per_line });
    let mut out = Vec::with_capacity(rows.len() / per_line + 1);
    let mut cells = Vec::with_capacity(per_line);
    let flush = |cells: &mut Vec<HexCell>, rows: &mut Vec<HexRow>| {
        if cells.is_empty() {
            return;
        }
        let label = |pick: fn(&HexCell) -> Option<usize>| cells.iter().find_map(pick);
        rows.push(HexRow {
            left_label: label(|c| match c {
                HexCell::Pair { left, .. } => Some(*left),
                HexCell::LeftOrphan(i) => Some(*i),
                HexCell::RightOrphan(_) => None,
            }),
            right_label: label(|c| match c {
                HexCell::Pair { right, .. } => Some(*right),
                HexCell::RightOrphan(i) => Some(*i),
                HexCell::LeftOrphan(_) => None,
            }),
            cells: std::mem::take(cells),
        });
    };
    for row in rows {
        cells.push(match row.kind {
            DiffKind::Same | DiffKind::Changed => HexCell::Pair {
                left: row.left.expect("paired row has left"),
                right: row.right.expect("paired row has right"),
                kind: row.kind,
            },
            DiffKind::LeftOnly => HexCell::LeftOrphan(row.left.expect("orphan row has its side")),
            DiffKind::RightOnly => HexCell::RightOrphan(row.right.expect("orphan row has its side")),
        });
        if cells.len() == per_line {
            flush(&mut cells, &mut out);
        }
    }
    flush(&mut cells, &mut out);
    out
}

/// 显示过滤后的对齐行（连续拼接）。
///
/// Diffs：每个差异游程向两侧各扩 `context` 行 Same（重叠合并）；Same：仅 Same 游程。
/// **区域不重置打包相位**（证据：`22-diffs-only.png` 左侧标签 0x1F7 = 前一区域 9 格余量
/// 与后一区域前 7 格同行）；调用方对拼接结果用 [`pack_rows`] 一次性打包。
pub fn filtered_rows(map: &AlignMap, filter: DisplayFilter, context: usize) -> Vec<AlignRow> {
    let regions = filtered_regions(map, filter, context);
    let mut out = Vec::new();
    for region in regions {
        out.extend_from_slice(&map.rows()[region]);
    }
    out
}

/// 显示过滤后的对齐行区间集合（align 行号区间）。
///
/// Diffs：每个差异游程向两侧各扩 `context` 行 Same（重叠合并）；Same：仅 Same 游程。
/// 各区间由调用方用 [`pack_slice`] 自区间起点重新打包（证据：`22-diffs-only.png` 行标签非 16 对齐）。
pub fn filtered_regions(map: &AlignMap, filter: DisplayFilter, context: usize) -> Vec<std::ops::Range<usize>> {
    let rows = map.rows();
    if rows.is_empty() {
        return Vec::new();
    }
    let wanted = |kind: DiffKind| match filter {
        DisplayFilter::All => true,
        DisplayFilter::Differences => kind != DiffKind::Same,
        DisplayFilter::Same => kind == DiffKind::Same,
    };
    let mut regions: Vec<std::ops::Range<usize>> = Vec::new();
    let mut i = 0;
    while i < rows.len() {
        if !wanted(rows[i].kind) {
            i += 1;
            continue;
        }
        let mut end = i + 1;
        while end < rows.len() && wanted(rows[end].kind) {
            end += 1;
        }
        let (mut start_c, mut end_c) = (i, end);
        if filter == DisplayFilter::Differences {
            start_c = start_c.saturating_sub(context);
            end_c = (end + context).min(rows.len());
        }
        if let Some(last) = regions.last_mut() {
            if start_c <= last.end {
                last.end = end_c;
                i = end;
                continue;
            }
        }
        regions.push(start_c..end_c);
        i = end;
    }
    regions
}

#[cfg(test)]
mod tests;
