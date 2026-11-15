//! Hex 字节对齐 – `TBcByteAlign` / `THexMapByteAlign`。
//!
//! 产物是紧凑的 [`ByteAlignMap`]：只存连续段（配对 / 左孤儿 / 右孤儿）与段首行号，
//! 不再为每个对齐字节存一个 `AlignRow`（56 字节/行 + 两侧反查表 16 字节/字节——两个约 18 MB 的
//! exe 曾因此占约 1.5 GB）。配对字节的 same/difference 由两侧字节值比较得出（[`ByteRow::kind`]）。
//! [`ByteAlign::align`] 仍返回逐行 [`AlignMap`]（展开紧凑结果，供小数据与既有调用方）。

use std::ops::Range;

use super::{AlignMap, AlignRow};
use crate::cancel::{CancelCheck, NeverCancel};
use crate::diff::{DiffOp, MyersDiff};
use crate::hash::ContentHash;
use crate::DiffKind;

/// Byte-sequence aligner for Hex compare – `TBcByteAlign` / `THexMapByteAlign`.
///
/// 算法边界经 REA 反汇编校准（macOS 5.0.4.30422 arm64，`THEXMAPALIGN.RUN`，2026-11-11）：
/// 锚窗 50 字节、滚动哈希、Complete 档区间 < 5000 字节做字节级细化、超限降级。
#[derive(Debug, Default)]
pub struct ByteAlign;

/// Hex 三档字节对齐策略 – BC `THexAlignKind = (alignNone=0, alignFast=1, alignComplete=2)`
///（RTTI 枚举表逐字证实）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ByteAlignStrategy {
    /// 锚块骨架 + 区间字节级细化（参考工具默认）。
    #[default]
    Complete,
    /// 锚块骨架，区间不做字节级细化，降级为孤儿段（面向超大文件）。
    ///
    /// 注意：参考工具的 Fast 实际把 mismatch 区间逐位对齐（RUNNONE）；本实现按设计 D1
    /// 取更保守的孤儿段，杜绝跨位移的假 same。
    Fast,
    /// 逐字节位置对齐（offset 对 offset），长度差尾部为孤儿。
    None,
}

/// 锚窗宽度（字节）：参考工具实测值 50。
const BYTE_ANCHOR_WINDOW: usize = 50;
/// Complete 档字节级细化的区间长度上限（两侧均 < 该值才细化）：参考工具实测值 5000。
const BYTE_REFINE_LIMIT: usize = 5000;
/// Complete 档大区间重同步的偏移容差（字节）：参考工具 `RUNRANGE` 立即数 10。
const BYTE_RESYNC_SKEW: usize = 10;
/// 大区间重同步判定所需的最短连续相等字节数（防止随机数据里的单字节巧合把配对拉偏）。
const BYTE_RESYNC_MATCH: usize = 4;

impl ByteAlign {
    pub fn align(&self, left: &[u8], right: &[u8], strategy: ByteAlignStrategy) -> AlignMap {
        self.align_cancellable(left, right, strategy, &NeverCancel)
            .expect("NeverCancel never cancels")
    }

    /// 逐行结果（展开 [`Self::align_segments`]；大文件请直接用紧凑结果）。
    pub fn align_cancellable(
        &self,
        left: &[u8],
        right: &[u8],
        strategy: ByteAlignStrategy,
        cancel: &dyn CancelCheck,
    ) -> Result<AlignMap, crate::CoreError> {
        self.align_segments(left, right, strategy, cancel)?
            .to_align_map(left, right, cancel)
    }

    /// 紧凑对齐结果（Hex 视图用）。
    pub fn align_segments(
        &self,
        left: &[u8],
        right: &[u8],
        strategy: ByteAlignStrategy,
        cancel: &dyn CancelCheck,
    ) -> Result<ByteAlignMap, crate::CoreError> {
        let segments = match strategy {
            ByteAlignStrategy::None => {
                // 第 i 字节对第 i 字节；长度差尾部为孤儿。
                let n = left.len().min(right.len());
                let mut sink = SegSink::new(0, 0);
                sink.pair(n);
                sink.left_only(left.len() - n);
                sink.right_only(right.len() - n);
                sink.segments
            }
            ByteAlignStrategy::Fast => anchored_segments(left, right, false, cancel)?,
            ByteAlignStrategy::Complete => anchored_segments(left, right, true, cancel)?,
        };
        cancel.check()?;
        Ok(ByteAlignMap::from_segments(segments))
    }
}

/// 段的种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteSpan {
    /// 两侧逐字节配对（每行 same 或 difference，按字节值判定）。
    Pair,
    /// 仅左侧字节（右侧占位）。
    LeftOnly,
    /// 仅右侧字节（左侧占位）。
    RightOnly,
}

/// 一个连续段：自对齐行 `row` 起 `len` 行；两侧自 `left` / `right` 偏移起连续
///（孤儿段不消耗对侧偏移，对侧字段为该处的插入点）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteSegment {
    pub row: usize,
    pub left: usize,
    pub right: usize,
    pub len: usize,
    pub span: ByteSpan,
}

impl ByteSegment {
    pub fn left_len(&self) -> usize {
        if self.span == ByteSpan::RightOnly { 0 } else { self.len }
    }

    pub fn right_len(&self) -> usize {
        if self.span == ByteSpan::LeftOnly { 0 } else { self.len }
    }

    pub fn rows(&self) -> Range<usize> {
        self.row..self.row + self.len
    }

    /// 段内第 k 行。
    pub fn row_at(&self, k: usize) -> ByteRow {
        ByteRow {
            left: (self.span != ByteSpan::RightOnly).then_some(self.left + k),
            right: (self.span != ByteSpan::LeftOnly).then_some(self.right + k),
        }
    }
}

/// 一个对齐行：两侧字节偏移（孤儿行对侧为 None）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRow {
    pub left: Option<usize>,
    pub right: Option<usize>,
}

impl ByteRow {
    /// 行的四分类：配对行按字节值判定 same / difference。
    pub fn kind(&self, left: &[u8], right: &[u8]) -> DiffKind {
        match (self.left, self.right) {
            (Some(l), Some(r)) => match (left.get(l), right.get(r)) {
                (Some(a), Some(b)) if a == b => DiffKind::Same,
                _ => DiffKind::Changed,
            },
            (Some(_), None) => DiffKind::LeftOnly,
            _ => DiffKind::RightOnly,
        }
    }
}

/// 紧凑字节对齐图：按行序排列的连续段。行号 / 偏移查找均为二分。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ByteAlignMap {
    segments: Vec<ByteSegment>,
    rows: usize,
    left_len: usize,
    right_len: usize,
}

impl ByteAlignMap {
    fn from_segments(segments: Vec<ByteSegment>) -> Self {
        let (mut rows, mut left_len, mut right_len) = (0usize, 0usize, 0usize);
        for seg in &segments {
            debug_assert_eq!((seg.row, seg.left, seg.right), (rows, left_len, right_len), "segments must be contiguous");
            rows += seg.len;
            left_len += seg.left_len();
            right_len += seg.right_len();
        }
        Self { segments, rows, left_len, right_len }
    }

    pub fn segments(&self) -> &[ByteSegment] {
        &self.segments
    }

    /// 对齐行数。
    pub fn len(&self) -> usize {
        self.rows
    }

    pub fn is_empty(&self) -> bool {
        self.rows == 0
    }

    /// 对齐时两侧的字节数。
    pub fn side_lens(&self) -> (usize, usize) {
        (self.left_len, self.right_len)
    }

    /// 包含对齐行 `row` 的段下标。
    pub fn segment_of_row(&self, row: usize) -> Option<usize> {
        (row < self.rows).then(|| self.segments.partition_point(|s| s.row + s.len <= row))
    }

    pub fn row(&self, row: usize) -> Option<ByteRow> {
        let seg = &self.segments[self.segment_of_row(row)?];
        Some(seg.row_at(row - seg.row))
    }

    /// 左侧偏移 `offset` 所在的对齐行。
    pub fn row_of_left(&self, offset: usize) -> Option<usize> {
        if offset >= self.left_len {
            return None;
        }
        let seg = &self.segments[self.segments.partition_point(|s| s.left + s.left_len() <= offset)];
        Some(seg.row + (offset - seg.left))
    }

    /// 右侧偏移 `offset` 所在的对齐行。
    pub fn row_of_right(&self, offset: usize) -> Option<usize> {
        if offset >= self.right_len {
            return None;
        }
        let seg = &self.segments[self.segments.partition_point(|s| s.right + s.right_len() <= offset)];
        Some(seg.row + (offset - seg.right))
    }

    /// 逐行遍历（小数据 / 测试用）。
    pub fn rows(&self) -> impl Iterator<Item = ByteRow> + '_ {
        self.segments.iter().flat_map(|s| (0..s.len).map(move |k| s.row_at(k)))
    }

    /// 展开为逐行 [`AlignMap`]（每行 56 字节，只适合小数据）。
    pub fn to_align_map(&self, left: &[u8], right: &[u8], cancel: &dyn CancelCheck) -> Result<AlignMap, crate::CoreError> {
        let rows: Vec<AlignRow> = self
            .rows()
            .map(|r| AlignRow { left: r.left, right: r.right, kind: r.kind(left, right), ..Default::default() })
            .collect();
        AlignMap::from_rows_cancellable(rows, cancel)
    }
}

/// 段收集器：维护当前行号与两侧偏移，相邻同种段自动合并。
struct SegSink {
    segments: Vec<ByteSegment>,
    row: usize,
    left: usize,
    right: usize,
}

impl SegSink {
    fn new(left: usize, right: usize) -> Self {
        Self { segments: Vec::new(), row: 0, left, right }
    }

    fn push(&mut self, span: ByteSpan, len: usize) {
        if len == 0 {
            return;
        }
        let seg = ByteSegment { row: self.row, left: self.left, right: self.right, len, span };
        self.row += len;
        self.left += seg.left_len();
        self.right += seg.right_len();
        match self.segments.last_mut() {
            Some(last) if last.span == span => last.len += len,
            _ => self.segments.push(seg),
        }
    }

    fn pair(&mut self, len: usize) {
        self.push(ByteSpan::Pair, len);
    }

    fn left_only(&mut self, len: usize) {
        self.push(ByteSpan::LeftOnly, len);
    }

    fn right_only(&mut self, len: usize) {
        self.push(ByteSpan::RightOnly, len);
    }
}

/// Rabin 滚动哈希（base 257，模 2^64 自然回绕；命中后一律 memcmp 复核，碰撞只浪费一次比较）。
struct RollingBytes<'a> {
    bytes: &'a [u8],
    window: usize,
    out_factor: u64,
}

impl<'a> RollingBytes<'a> {
    const BASE: u64 = 257;

    fn new(bytes: &'a [u8], window: usize) -> Self {
        let mut out_factor = 1u64;
        for _ in 0..window {
            out_factor = out_factor.wrapping_mul(Self::BASE);
        }
        Self { bytes, window, out_factor }
    }

    fn window_hash(&self, start: usize) -> u64 {
        let mut h = 0u64;
        for &b in &self.bytes[start..start + self.window] {
            h = h.wrapping_mul(Self::BASE).wrapping_add(u64::from(b));
        }
        h
    }

    /// 返回 (起点, hash) 迭代所需的滚动推进：h' = h*B - out*B^W + in。
    fn roll(&self, start: usize, hash: u64) -> u64 {
        hash.wrapping_mul(Self::BASE)
            .wrapping_sub(u64::from(self.bytes[start]).wrapping_mul(self.out_factor))
            .wrapping_add(u64::from(self.bytes[start + self.window]))
    }
}

///// 大文件锚窗采样阈值：任一侧超过该字节数时，只对 `hash % BYTE_SAMPLE_MOD == 0` 的锚窗建索引。
/// 采样按哈希值决定，两侧同一内容的锚窗同进同出，不破坏相位语义；小文件逐字节入库。
const BYTE_SAMPLE_THRESHOLD: usize = 1024 * 1024;
const BYTE_SAMPLE_MOD: u64 = 16;
/// 区间物化并行化门槛（两侧合计字节）：低于该值单线程，避免小文件的线程开销。
const BYTE_PARALLEL_THRESHOLD: usize = 4 * 1024 * 1024;

/// 采样后的锚窗：(位置, 哈希)，按位置升序。
fn sampled_windows(
    bytes: &[u8],
    window: usize,
    sample_mod: u64,
    cancel: &dyn CancelCheck,
) -> Result<Vec<(usize, u64)>, crate::CoreError> {
    let rolling = RollingBytes::new(bytes, window);
    let mut out = Vec::with_capacity(bytes.len() / sample_mod as usize + 1);
    let mut h = rolling.window_hash(0);
    let mut pos = 0usize;
    loop {
        if h.is_multiple_of(sample_mod) {
            out.push((pos, h));
        }
        if pos + window >= bytes.len() {
            break;
        }
        h = rolling.roll(pos, h);
        pos += 1;
        if pos.is_multiple_of(1 << 16) {
            cancel.check()?;
        }
    }
    Ok(out)
}

/// 锚窗索引：按 (混合哈希, 位置) 排序，同哈希候选是连续切片且位置升序；
/// 顶位分桶目录让查找为 O(1) 次访存（大文件千万级条目时二分查找的缓存缺失会主导耗时）。
struct WindowIndex {
    entries: Vec<(u64, usize)>,
    /// `dir[b]` = 首个桶号 ≥ b 的条目下标（长度 2^bits + 1）。
    dir: Vec<usize>,
    shift: u32,
}

impl WindowIndex {
    /// 奇数乘法是 2^64 上的双射：等值关系不变，顶位分布更均匀（滚动哈希高位偏差大）。
    fn mix(hash: u64) -> u64 {
        hash.wrapping_mul(0x9E37_79B9_7F4A_7C15)
    }

    fn new(samples: &[(usize, u64)]) -> Self {
        let mut entries: Vec<(u64, usize)> = samples.iter().map(|&(p, h)| (Self::mix(h), p)).collect();
        entries.sort_unstable();
        let bits = (usize::BITS - entries.len().max(2).leading_zeros()).saturating_sub(1).clamp(1, 26);
        let shift = 64 - bits;
        let buckets = 1usize << bits;
        let mut dir = Vec::with_capacity(buckets + 1);
        let mut at = 0usize;
        for b in 0..buckets {
            while at < entries.len() && ((entries[at].0 >> shift) as usize) < b {
                at += 1;
            }
            dir.push(at);
        }
        dir.push(entries.len());
        Self { entries, dir, shift }
    }

    fn candidates(&self, hash: u64) -> &[(u64, usize)] {
        let key = Self::mix(hash);
        let b = (key >> self.shift) as usize;
        let bucket = &self.entries[self.dir[b]..self.dir[b + 1]];
        let lo = bucket.partition_point(|&(k, _)| k < key);
        let hi = lo + bucket[lo..].partition_point(|&(k, _)| k == key);
        &bucket[lo..hi]
    }
}

/// 最长递增子序列（按 j 严格递增；输入已按 i 升序）。
fn longest_increasing_chain(pairs: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut tails: Vec<usize> = Vec::new(); // tails[k] = 长度 k+1 链尾在 pairs 中的下标
    let mut prev: Vec<usize> = vec![usize::MAX; pairs.len()];
    for (idx, &(_, j)) in pairs.iter().enumerate() {
        // 相近文件的唯一锚点几乎单调递增：先比链尾，省去千万级二分的缓存缺失。
        let k = match tails.last() {
            Some(&t) if pairs[t].1 < j => tails.len(),
            _ => tails.partition_point(|&t| pairs[t].1 < j),
        };
        if k > 0 {
            prev[idx] = tails[k - 1];
        }
        if k == tails.len() {
            tails.push(idx);
        } else {
            tails[k] = idx;
        }
    }
    let mut chain = Vec::with_capacity(tails.len());
    let mut cur = tails.last().copied().unwrap_or(usize::MAX);
    while cur != usize::MAX {
        chain.push(pairs[cur]);
        cur = prev[cur];
    }
    chain.reverse();
    chain
}

/// 对齐计划的一段：先规划、后物化（物化可并行，区间细化是主要耗时）。
#[derive(Debug, Clone, Copy)]
enum ByteSeg {
    /// 两侧自 (l, r) 起连续 len 字节相同。
    Same { l: usize, r: usize, len: usize },
    /// 锚点间区间 `[l0, l1) × [r0, r1)`，交给 [`emit_gap`]。
    Gap { l0: usize, l1: usize, r0: usize, r1: usize },
}

impl ByteSeg {
    /// 物化工作量估计（并行分块用）。
    fn weight(&self) -> usize {
        match *self {
            Self::Same { len, .. } => len,
            Self::Gap { l0, l1, r0, r1 } => 4 * ((l1 - l0) + (r1 - r0)),
        }
    }
}

/// Fast/Complete 共享的锚块骨架。
///
/// 1. **唯一锚窗骨架**：两侧各只出现一次的锚窗配对，取按位置保序的最长链（patience 思路）。
///    偶然在远处重复出现的短片段（重编译 exe 里的常见指令序列、填充）因此不会把对齐拉到
///    文件远端——旧的纯贪心扫描曾因此把两个相近 exe 几乎全判为孤儿。
/// 2. **区间内贪心**：骨架锚点之间沿用原贪心扫描（取 ≥ 下界的最近锚点、向前扩展 same 游程），
///    候选限定在区间内；锚点间余量由 `refine` 决定孤儿段（Fast）还是字节级细化（Complete）。
/// 3. **物化**：计划按工作量切块，大文件多线程物化后按序拼接（结果与单线程逐字节一致）。
fn anchored_segments(
    left: &[u8],
    right: &[u8],
    refine: bool,
    cancel: &dyn CancelCheck,
) -> Result<Vec<ByteSegment>, crate::CoreError> {
    let plan = byte_plan(left, right, cancel)?;
    materialize_plan(&plan, left, right, refine, cancel)
}

fn byte_plan(left: &[u8], right: &[u8], cancel: &dyn CancelCheck) -> Result<Vec<ByteSeg>, crate::CoreError> {
    let window = BYTE_ANCHOR_WINDOW;
    let mut plan = Vec::new();
    // 任一侧短于锚窗时无锚可寻，整体作为一个区间处理。
    if left.len() < window || right.len() < window {
        plan.push(ByteSeg::Gap { l0: 0, l1: left.len(), r0: 0, r1: right.len() });
        return Ok(plan);
    }
    let sample_mod = if left.len().max(right.len()) > BYTE_SAMPLE_THRESHOLD { BYTE_SAMPLE_MOD } else { 1 };
    // 两侧滚动哈希与排序互不依赖：并行。
    let (left_side, right_index) = std::thread::scope(|scope| {
        let right_job = scope.spawn(|| -> Result<WindowIndex, crate::CoreError> {
            Ok(WindowIndex::new(&sampled_windows(right, window, sample_mod, cancel)?))
        });
        let left_side = sampled_windows(left, window, sample_mod, cancel).map(|samples| {
            let index = WindowIndex::new(&samples);
            (samples, index)
        });
        let right_index = right_job.join().unwrap_or_else(|_| Err(crate::CoreError::Invalid("anchor index panicked".into())));
        (left_side, right_index)
    });
    let (left_samples, left_index) = left_side?;
    let right_index = right_index?;
    cancel.check()?;

    // 1. 唯一锚窗骨架。
    let mut unique: Vec<(usize, usize)> = Vec::new();
    for (n, &(i, h)) in left_samples.iter().enumerate() {
        if n.is_multiple_of(1 << 16) {
            cancel.check()?;
        }
        if let [(_, j)] = right_index.candidates(h) {
            if left_index.candidates(h).len() == 1 && left[i..i + window] == right[*j..*j + window] {
                unique.push((i, *j));
            }
        }
    }
    drop(left_index);
    let chain = longest_increasing_chain(&unique);
    drop(unique);
    cancel.check()?;

    // 2. 沿骨架规划：锚点前的区间走区间内贪心，锚点处向前后扩展 same 游程。
    let ctx = GreedyCtx { left, right, window, left_samples: &left_samples, right_index: &right_index };
    let (mut gl, mut gr) = (0usize, 0usize);
    for (n, &(i, j)) in chain.iter().enumerate() {
        if n.is_multiple_of(4096) {
            cancel.check()?;
        }
        if i < gl || j < gr {
            continue; // 已被上一锚点的 same 游程吞并
        }
        let (mut a, mut b) = (i, j);
        while a > gl && b > gr && left[a - 1] == right[b - 1] {
            a -= 1;
            b -= 1;
        }
        ctx.gap(&mut plan, gl, a, gr, b, cancel)?;
        (gl, gr) = push_same_run(&mut plan, left, right, a, b, left.len(), right.len());
    }
    ctx.gap(&mut plan, gl, left.len(), gr, right.len(), cancel)?;
    Ok(plan)
}

/// 从 (p, q) 起向前扩展 same 游程（不越过上界），记入计划并返回游程终点。
fn push_same_run(
    plan: &mut Vec<ByteSeg>,
    left: &[u8],
    right: &[u8],
    p: usize,
    q: usize,
    left_end: usize,
    right_end: usize,
) -> (usize, usize) {
    let len = left[p..left_end]
        .iter()
        .zip(&right[q..right_end])
        .take_while(|(a, b)| a == b)
        .count();
    if len > 0 {
        plan.push(ByteSeg::Same { l: p, r: q, len });
    }
    (p + len, q + len)
}

fn push_gap(plan: &mut Vec<ByteSeg>, l0: usize, l1: usize, r0: usize, r1: usize) {
    if l1 > l0 || r1 > r0 {
        plan.push(ByteSeg::Gap { l0, l1, r0, r1 });
    }
}

/// 区间内贪心扫描的共享上下文。
struct GreedyCtx<'a> {
    left: &'a [u8],
    right: &'a [u8],
    window: usize,
    left_samples: &'a [(usize, u64)],
    right_index: &'a WindowIndex,
}

impl GreedyCtx<'_> {
    /// 区间 `[l0, l1) × [r0, r1)` 内的贪心锚点扫描：左侧按位置取采样锚窗，配对右侧
    /// 区间内、≥ 保序下界的第一个相同锚窗；锚点间余量记为区间段。
    fn gap(
        &self,
        plan: &mut Vec<ByteSeg>,
        l0: usize,
        l1: usize,
        r0: usize,
        r1: usize,
        cancel: &dyn CancelCheck,
    ) -> Result<(), crate::CoreError> {
        let (left, right, window) = (self.left, self.right, self.window);
        let (mut gap_left, mut gap_right) = (l0, r0);
        if l1 - l0 >= window && r1 - r0 >= window {
            let mut k = self.left_samples.partition_point(|&(p, _)| p < l0);
            let mut steps = 0usize;
            while let Some(&(i, h)) = self.left_samples.get(k) {
                if i + window > l1 {
                    break;
                }
                steps += 1;
                if steps.is_multiple_of(4096) {
                    cancel.check()?;
                }
                let cands = self.right_index.candidates(h);
                let from = cands.partition_point(|&(_, j)| j < gap_right);
                let anchor = cands[from..]
                    .iter()
                    .map(|&(_, j)| j)
                    .take_while(|&j| j + window <= r1)
                    .find(|&j| right[j..j + window] == left[i..i + window]);
                let Some(j) = anchor else {
                    k += 1;
                    continue;
                };
                let (mut a, mut b) = (i, j);
                while a > gap_left && b > gap_right && left[a - 1] == right[b - 1] {
                    a -= 1;
                    b -= 1;
                }
                push_gap(plan, gap_left, a, gap_right, b);
                (gap_left, gap_right) = push_same_run(plan, left, right, a, b, l1, r1);
                k += self.left_samples[k..].partition_point(|&(pos, _)| pos < gap_left);
            }
        }
        push_gap(plan, gap_left, l1, gap_right, r1);
        Ok(())
    }
}

/// 计划 → 紧凑段：按工作量切成连续块，大文件多线程物化后按序拼接（结果与单线程一致）。
fn materialize_plan(
    plan: &[ByteSeg],
    left: &[u8],
    right: &[u8],
    refine: bool,
    cancel: &dyn CancelCheck,
) -> Result<Vec<ByteSegment>, crate::CoreError> {
    let run = |segs: &[ByteSeg]| -> Result<SegSink, crate::CoreError> {
        let (l, r) = match segs.first() {
            Some(ByteSeg::Same { l, r, .. }) => (*l, *r),
            Some(ByteSeg::Gap { l0, r0, .. }) => (*l0, *r0),
            None => (0, 0),
        };
        let mut sink = SegSink::new(l, r);
        for seg in segs {
            match *seg {
                ByteSeg::Same { len, .. } => sink.pair(len),
                ByteSeg::Gap { l0, l1, r0, r1 } => emit_gap(&mut sink, left, right, l0, l1, r0, r1, refine, cancel)?,
            }
        }
        Ok(sink)
    };
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(16);
    if left.len() + right.len() < BYTE_PARALLEL_THRESHOLD || threads < 2 || plan.len() < 2 {
        return Ok(run(plan)?.segments);
    }
    // 按累计工作量切 threads×4 块（块内顺序不变），线程轮询取块。
    let total: usize = plan.iter().map(|s| s.weight()).sum();
    let target = total.div_ceil(threads * 4).max(1);
    let mut chunks: Vec<&[ByteSeg]> = Vec::new();
    let (mut start, mut acc) = (0usize, 0usize);
    for (k, seg) in plan.iter().enumerate() {
        acc += seg.weight();
        if acc >= target {
            chunks.push(&plan[start..=k]);
            start = k + 1;
            acc = 0;
        }
    }
    if start < plan.len() {
        chunks.push(&plan[start..]);
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut parts: Vec<Option<Result<SegSink, crate::CoreError>>> = (0..chunks.len()).map(|_| None).collect();
    let results = std::sync::Mutex::new(&mut parts);
    std::thread::scope(|scope| {
        for _ in 0..threads.min(chunks.len()) {
            scope.spawn(|| loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(chunk) = chunks.get(k) else { break };
                let out = run(chunk);
                let failed = out.is_err();
                if let Ok(mut guard) = results.lock() {
                    guard[k] = Some(out);
                }
                if failed {
                    break;
                }
            });
        }
    });
    // 拼接：各块行号自 0 起，按前序块行数平移；块边界上的同种段合并。
    let mut out: Vec<ByteSegment> = Vec::new();
    let mut row_base = 0usize;
    for part in parts {
        let part = part.ok_or_else(|| crate::CoreError::Invalid("alignment worker failed".into()))??;
        for mut seg in part.segments {
            seg.row += row_base;
            match out.last_mut() {
                Some(last) if last.span == seg.span => last.len += seg.len,
                _ => out.push(seg),
            }
        }
        row_base += part.row;
    }
    Ok(out)
}

/// 输出一个锚点间区间：Fast 降级为双侧孤儿段；Complete 小区间字节级细化、
/// 超限走 [`resync_gap`] 对角线重同步（参考 `RUNRANGE`，不再整段降级孤儿）。
#[allow(clippy::too_many_arguments)]
fn emit_gap(
    sink: &mut SegSink,
    left: &[u8],
    right: &[u8],
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
    refine: bool,
    cancel: &dyn CancelCheck,
) -> Result<(), crate::CoreError> {
    debug_assert_eq!((sink.left, sink.right), (left_start, right_start));
    let left_len = left_end - left_start;
    let right_len = right_end - right_start;
    if left_len == 0 && right_len == 0 {
        return Ok(());
    }
    if !refine {
        sink.left_only(left_len);
        sink.right_only(right_len);
        return Ok(());
    }
    if left_len < BYTE_REFINE_LIMIT && right_len < BYTE_REFINE_LIMIT {
        return refine_gap(sink, left, right, left_start, left_end, right_start, right_end, cancel);
    }
    resync_gap(sink, left, right, left_start, left_end, right_start, right_end, cancel)
}

/// Complete 档大区间（任一侧 ≥ 5000）– 参考工具 `RUNRANGE` 的对应物：沿对角线逐字节配对
///（相等为 same、不等为 difference）；失配处在 ±`BYTE_RESYNC_SKEW` 字节内寻找连续
/// `BYTE_RESYNC_MATCH` 字节相等的重同步点，找到则把跳过的字节记为该侧孤儿。
/// 区间尾部余量为孤儿。线性时间，适合整段内容都不同的大文件（如两张不同的 PNG）。
#[allow(clippy::too_many_arguments)]
fn resync_gap(
    sink: &mut SegSink,
    left: &[u8],
    right: &[u8],
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
    cancel: &dyn CancelCheck,
) -> Result<(), crate::CoreError> {
    let k = BYTE_RESYNC_MATCH;
    let matches = |i: usize, j: usize| {
        i + k <= left_end && j + k <= right_end && left[i..i + k] == right[j..j + k]
    };
    let (mut i, mut j) = (left_start, right_start);
    let mut paired = 0usize; // 尚未写入的连续配对行
    let mut steps = 0usize;
    while i < left_end && j < right_end {
        steps += 1;
        if steps.is_multiple_of(4096) {
            cancel.check()?;
        }
        if left[i] != right[j] && !matches(i + 1, j + 1) {
            // 当前对角线接下来对不上：在偏移容差内找重同步点（单字节不同 = difference，不跳）。
            let skip = (1..=BYTE_RESYNC_SKEW).find_map(|d| {
                if matches(i + d, j) {
                    Some((d, 0))
                } else if matches(i, j + d) {
                    Some((0, d))
                } else {
                    None
                }
            });
            if let Some((dl, dr)) = skip {
                sink.pair(paired);
                paired = 0;
                sink.left_only(dl);
                sink.right_only(dr);
                i += dl;
                j += dr;
                continue;
            }
        }
        paired += 1;
        i += 1;
        j += 1;
    }
    sink.pair(paired);
    sink.left_only(left_end - i);
    sink.right_only(right_end - j);
    Ok(())
}

/// Complete 档区间细化：字节级 Myers diff，改动块内按位置配对为 difference、余量为孤儿
///（参考工具把单侧改动配对成 difference 而非双侧孤儿，四分类统计依赖此行为）。
/// 两个 Equal 之间的删除 / 插入在各自一侧都是连续区间，因此改动块 = 配对段 + 左孤儿段 + 右孤儿段。
#[allow(clippy::too_many_arguments)]
fn refine_gap(
    sink: &mut SegSink,
    left: &[u8],
    right: &[u8],
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
    cancel: &dyn CancelCheck,
) -> Result<(), crate::CoreError> {
    let hash = |bytes: &[u8]| -> Vec<ContentHash> { bytes.iter().map(|&b| ContentHash(u64::from(b))).collect() };
    let a = hash(&left[left_start..left_end]);
    let b = hash(&right[right_start..right_end]);
    let ops = MyersDiff::default().diff_cancellable(&a, &b, cancel)?;
    let (mut dels, mut inss) = (0usize, 0usize); // 当前改动块两侧字节数
    let flush = |sink: &mut SegSink, dels: &mut usize, inss: &mut usize| {
        let paired = (*dels).min(*inss);
        sink.pair(paired);
        sink.left_only(*dels - paired);
        sink.right_only(*inss - paired);
        *dels = 0;
        *inss = 0;
    };
    for op in ops {
        match op {
            DiffOp::Equal { len, .. } => {
                flush(sink, &mut dels, &mut inss);
                sink.pair(len);
            }
            DiffOp::Delete { len, .. } => dels += len,
            DiffOp::Insert { len, .. } => inss += len,
        }
    }
    flush(sink, &mut dels, &mut inss);
    debug_assert_eq!((sink.left, sink.right), (left_end, right_end));
    Ok(())
}
