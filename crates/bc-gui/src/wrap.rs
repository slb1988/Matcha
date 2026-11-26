//! Word Wrap 共用部件（文本比较与文本合并）：
//!
//! - [`Glyphs`]：一种字体的逐字符字宽缓存；
//! - [`WrapSpec`] + [`count`] / [`breaks`]：按窗格文本区宽度计算一行的折行断点与视觉行数；
//! - [`RowHeights`]：按显示行记录视觉行数（整数单位）及其前缀和，配合 [`RowSource`]
//!   在大文件上分「精确 / 估算」两档建立。
//!
//! 断点由这里算、绘制按段排版（不用 egui 的 `wrap.max_width`），保证行高与实际绘制一致。
use eframe::egui;
use std::{collections::HashMap, ops::Range};

/// 一行在一个窗格中最多折出的视觉行数；超出部分不显示。
pub const WRAP_CAP: u32 = 10_000;
/// 断点判断时从文本区宽度里扣掉的余量（吸收字形像素取整误差）。
const MARGIN: f32 = 0.5;
/// 显示行建表时同步精确计算的字节预算；超出部分先估算。
pub const SYNC_BUDGET: usize = 16 * 1024 * 1024;
/// 只因内容 / 投影变化（宽度不变）而重建时的同步预算：编辑时每次按键都会重建，
/// 预算小保证输入跟手；其余行先估算，视口附近当帧精确，空闲帧补齐。
pub const EDIT_BUDGET: usize = 1024 * 1024;

/// 一种字体的逐字符字宽（点）。ASCII 用数组，其他字符按需查询后缓存。
pub struct Glyphs {
    ascii: [f32; 128],
    other: HashMap<char, f32>,
    measure: Box<dyn FnMut(char) -> f32 + Send>,
    /// 可打印 ASCII 的最大字宽（快路径上界）。
    ascii_max: f32,
}

impl std::fmt::Debug for Glyphs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Glyphs")
            .field("ascii_max", &self.ascii_max)
            .finish_non_exhaustive()
    }
}

impl Glyphs {
    /// 以任意测量函数建缓存（单测用定宽函数）。
    pub fn with(mut measure: impl FnMut(char) -> f32 + Send + 'static) -> Self {
        let mut ascii = [0.0; 128];
        for (i, w) in ascii.iter_mut().enumerate() {
            *w = measure(i as u8 as char);
        }
        let ascii_max = ascii[0x20..0x7f].iter().copied().fold(0.0, f32::max);
        Self {
            ascii,
            other: HashMap::new(),
            measure: Box::new(measure),
            ascii_max,
        }
    }
    /// egui 字体的实际字宽；字体里没有的字符按 egui 画「豆腐块」时的宽度（取 `M` 的宽度）计。
    pub fn egui(ctx: &egui::Context, font: egui::FontId) -> Self {
        let ctx = ctx.clone();
        let tofu = ctx.fonts_mut(|f| f.glyph_width(&font, 'M'));
        Self::with(move |c| {
            ctx.fonts_mut(|f| {
                if c.is_control() {
                    0.0
                } else if f.has_glyph(&font, c) {
                    f.glyph_width(&font, c)
                } else {
                    tofu
                }
            })
        })
    }
    pub fn width(&mut self, c: char) -> f32 {
        if c.is_ascii() {
            return self.ascii[c as usize];
        }
        if let Some(w) = self.other.get(&c) {
            return *w;
        }
        let w = (self.measure)(c);
        self.other.insert(c, w);
        w
    }
}

/// 一个窗格的折行参数。`width` 为文本区宽度（点）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WrapSpec {
    pub width: f32,
    /// 制表宽度（列）。
    pub tab: usize,
    /// Visible Whitespace：空格画成 `·`、制表符以 `→` 开头。
    pub whitespace: bool,
}

impl WrapSpec {
    /// 用于比较「是否需要重建」的键：宽度向下取整，避免亚像素抖动。
    pub fn key(&self) -> (i32, usize, bool) {
        (self.width.floor() as i32, self.tab.max(1), self.whitespace)
    }
}

/// 一行的折行结果：每个视觉行的起点（字节偏移、该处的逻辑列）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Wrapped {
    /// 视觉行起点 `(byte, column)`；第一项恒为 `(0, 0)`。
    pub starts: Vec<(usize, usize)>,
    /// 显示到此字节为止（未截断时等于行长）。
    pub end: usize,
    /// 因 [`WRAP_CAP`] 被截掉的字符数（未截断时为 0）。
    pub hidden_chars: usize,
}

impl Wrapped {
    pub fn len(&self) -> usize {
        self.starts.len()
    }
    pub fn is_empty(&self) -> bool {
        self.starts.is_empty()
    }
    /// 第 `k` 个视觉行的字节区间与起始列。
    pub fn segment(&self, k: usize) -> (Range<usize>, usize) {
        let (start, column) = self.starts[k];
        let end = self.starts.get(k + 1).map_or(self.end, |s| s.0);
        (start..end, column)
    }
    /// 字节 `byte` 所在的视觉行（行尾字节属于最后一个视觉行；截断部分归最后一个视觉行）。
    pub fn segment_of(&self, byte: usize) -> usize {
        self.starts
            .partition_point(|s| s.0 <= byte)
            .saturating_sub(1)
    }
}

/// CJK 表意文字、假名、谚文、全角形式：前后都可折行。
fn breakable_cjk(c: char) -> bool {
    matches!(c as u32,
        0x2E80..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFE30..=0xFE4F
        | 0xFF00..=0xFFEF | 0x20000..=0x3FFFF)
}

#[cfg(test)]
thread_local! {
    /// 逐字符扫描次数（单测断言快路径不扫描）。
    static SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// 逐字符折行。`on_break(byte, column)` 在每个新视觉行起点调用；返回 `(视觉行数, 截断处字节)`。
fn run(
    text: &str,
    spec: &WrapSpec,
    glyphs: &mut Glyphs,
    mut on_break: impl FnMut(usize, usize),
) -> (u32, Option<usize>) {
    #[cfg(test)]
    SCANS.with(|s| s.set(s.get() + 1));
    let limit = (spec.width - MARGIN).max(0.0);
    let tab = spec.tab.max(1);
    let space_w = glyphs.width(' ');
    let dot_w = if spec.whitespace {
        glyphs.width('·')
    } else {
        space_w
    };
    let arrow_w = if spec.whitespace {
        glyphs.width('→')
    } else {
        space_w
    };
    let mut count = 1u32;
    let mut seg_start = 0usize;
    let mut x = 0.0f32;
    let mut column = 0usize;
    // 最近的可断点：(字节, 该处 x, 该处列)。
    let mut breakpoint: Option<(usize, f32, usize)> = None;
    for (i, c) in text.char_indices() {
        let (w, n) = match c {
            '\t' => {
                let n = tab - column % tab;
                (arrow_w + space_w * (n - 1) as f32, n)
            }
            ' ' => (dot_w, 1),
            _ => (glyphs.width(c), 1),
        };
        let space = c == ' ' || c == '\t';
        let cjk = breakable_cjk(c);
        // 空白不触发折行（挂在上一视觉行末尾）；每个视觉行至少一个字符。
        while !space && i > seg_start && x + w > limit {
            let (b, bx, bc) = match breakpoint {
                Some(p) if p.0 > seg_start => p,
                _ => (i, x, column),
            };
            if count == WRAP_CAP {
                return (count, Some(b));
            }
            count += 1;
            on_break(b, bc);
            seg_start = b;
            x -= bx;
            breakpoint = None;
        }
        if cjk && i > seg_start {
            breakpoint = Some((i, x, column));
        }
        x += w;
        column += n;
        if space || cjk {
            breakpoint = Some((i + c.len_utf8(), x, column));
        }
    }
    (count, None)
}

/// 一行的视觉行数（≤ [`WRAP_CAP`]）。纯 ASCII、无制表符且整行放得下时不逐字符扫描。
pub fn count(text: &str, spec: &WrapSpec, glyphs: &mut Glyphs) -> u32 {
    if fits_quickly(text, spec, glyphs) {
        return 1;
    }
    run(text, spec, glyphs, |_, _| {}).0
}

/// 不扫描就能确定只占一个视觉行时返回 true。
pub fn fits_quickly(text: &str, spec: &WrapSpec, glyphs: &mut Glyphs) -> bool {
    if text.is_empty() {
        return true;
    }
    if !text.is_ascii() || text.as_bytes().contains(&b'\t') {
        return false;
    }
    let per_byte = if spec.whitespace {
        glyphs.ascii_max.max(glyphs.width('·'))
    } else {
        glyphs.ascii_max
    };
    text.len() as f32 * per_byte <= (spec.width - MARGIN).max(0.0)
}

/// 一行的全部视觉行起点。
pub fn breaks(text: &str, spec: &WrapSpec, glyphs: &mut Glyphs) -> Wrapped {
    let mut starts = vec![(0, 0)];
    if fits_quickly(text, spec, glyphs) {
        return Wrapped {
            starts,
            end: text.len(),
            hidden_chars: 0,
        };
    }
    let (_, cut) = run(text, spec, glyphs, |b, c| starts.push((b, c)));
    let end = cut.unwrap_or(text.len());
    Wrapped {
        starts,
        end,
        hidden_chars: text[end..].chars().count(),
    }
}

/// 建表时对单个显示行的快速判断。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quick {
    /// 不扫描即可确定（快路径、空行、细折叠行等）。
    Known(u32),
    /// 需要逐字符计算；`bytes` 为工作量，`estimate` 为不扫描时的估算值。
    Measure { bytes: usize, estimate: u32 },
}

/// 显示行的视觉行数来源（视图实现：对齐行各窗格取最大值）。
pub trait RowSource {
    fn quick(&mut self, row: usize) -> Quick;
    fn measure(&mut self, row: usize) -> u32;
}

/// 按显示行的视觉行数（整数单位）与前缀和。细折叠行等非文本行记 0，由调用方另加高度。
#[derive(Debug, Clone, Default)]
pub struct RowHeights {
    units: Vec<u32>,
    /// `prefix[i]` = `units[..i]` 之和；长度 `len + 1`。
    prefix: Vec<u64>,
    /// 仍是估算值的行。
    inexact: Vec<bool>,
    pending: usize,
    /// 后台补齐的下一个候选行。
    cursor: usize,
}

impl RowHeights {
    /// 建表：先走快判断；需要扫描的行在 `budget` 字节内精确计算，超出部分记估算值。
    pub fn build(len: usize, src: &mut impl RowSource, budget: usize) -> Self {
        let mut units = Vec::with_capacity(len);
        let mut inexact = vec![false; len];
        let mut spent = 0usize;
        let mut pending = 0;
        for (row, flag) in inexact.iter_mut().enumerate() {
            units.push(match src.quick(row) {
                Quick::Known(u) => u,
                Quick::Measure { bytes, estimate } => {
                    if spent < budget {
                        spent += bytes;
                        src.measure(row)
                    } else {
                        *flag = true;
                        pending += 1;
                        estimate.clamp(1, WRAP_CAP)
                    }
                }
            });
        }
        let mut table = Self {
            units,
            prefix: Vec::new(),
            inexact,
            pending,
            cursor: 0,
        };
        table.reprefix(0);
        table
    }
    pub fn len(&self) -> usize {
        self.units.len()
    }
    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }
    /// 是否所有行都已精确计算。
    pub fn is_complete(&self) -> bool {
        self.pending == 0
    }
    pub fn units(&self, row: usize) -> u32 {
        self.units.get(row).copied().unwrap_or(1)
    }
    /// `row` 之前所有行的单位和（越界钳制到总和）。
    pub fn before(&self, row: usize) -> u64 {
        self.prefix[row.min(self.units.len())]
    }
    pub fn total(&self) -> u64 {
        *self.prefix.last().unwrap_or(&0)
    }
    /// 精确重算 `rows` 中仍为估算值的行。返回是否有行的高度改变。
    pub fn refine(&mut self, rows: Range<usize>, src: &mut impl RowSource) -> bool {
        if self.pending == 0 {
            return false;
        }
        let rows = rows.start.min(self.len())..rows.end.min(self.len());
        let mut first = None;
        for row in rows {
            if self.inexact[row] {
                self.inexact[row] = false;
                self.pending -= 1;
                let u = src.measure(row);
                if u != self.units[row] {
                    self.units[row] = u;
                    first.get_or_insert(row);
                }
            }
        }
        if let Some(first) = first {
            self.reprefix(first);
        }
        first.is_some()
    }
    /// 后台补齐：从游标起精确计算约 `budget` 字节的估算行。返回是否有行的高度改变。
    pub fn advance(&mut self, src: &mut impl RowSource, budget: usize) -> bool {
        let mut spent = 0usize;
        let mut first = None;
        while self.pending > 0 && spent < budget && self.cursor < self.len() {
            let row = self.cursor;
            self.cursor += 1;
            if !self.inexact[row] {
                continue;
            }
            if let Quick::Measure { bytes, .. } = src.quick(row) {
                spent += bytes;
            }
            self.inexact[row] = false;
            self.pending -= 1;
            let u = src.measure(row);
            if u != self.units[row] {
                self.units[row] = u;
                first.get_or_insert(row);
            }
        }
        if let Some(first) = first {
            self.reprefix(first);
        }
        first.is_some()
    }
    /// 编辑后：重算 `rows` 的高度（精确），更新前缀和。
    pub fn remeasure(&mut self, rows: Range<usize>, src: &mut impl RowSource) {
        let rows = rows.start.min(self.len())..rows.end.min(self.len());
        let start = rows.start;
        for row in rows {
            if self.inexact[row] {
                self.inexact[row] = false;
                self.pending -= 1;
            }
            self.units[row] = match src.quick(row) {
                Quick::Known(u) => u,
                Quick::Measure { .. } => src.measure(row),
            };
        }
        self.reprefix(start);
    }
    /// 前缀和中最后一个满足 `before(row) <= y` 且 `units(row) > 0` 的行（单位坐标）。
    pub fn row_at(&self, y: u64) -> usize {
        // prefix 单调不减；求 prefix[i+1] > y 的最小 i。
        let i = self.prefix[1..].partition_point(|p| *p <= y);
        i.min(self.len().saturating_sub(1))
    }
    fn reprefix(&mut self, from: usize) {
        let from = from.min(self.units.len());
        self.prefix.truncate(from + 1);
        if self.prefix.is_empty() {
            self.prefix.push(0);
        }
        let mut acc = self.prefix[from];
        for u in &self.units[from..] {
            acc += u64::from(*u);
            self.prefix.push(acc);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: f32 = 7.0;
    /// 定宽字体：ASCII 宽 7，CJK 宽 14。
    fn glyphs() -> Glyphs {
        Glyphs::with(|c| {
            if c.is_control() {
                0.0
            } else if breakable_cjk(c) {
                2.0 * W
            } else {
                W
            }
        })
    }
    fn spec(chars: usize) -> WrapSpec {
        WrapSpec {
            width: chars as f32 * W + MARGIN,
            tab: 4,
            whitespace: false,
        }
    }
    fn lines(text: &str, s: &WrapSpec) -> Vec<String> {
        let w = breaks(text, s, &mut glyphs());
        (0..w.len())
            .map(|k| text[w.segment(k).0].to_string())
            .collect()
    }

    #[test]
    fn breaks_at_whitespace_and_keeps_it_on_the_previous_line() {
        assert_eq!(
            lines("alpha beta gamma", &spec(10)),
            ["alpha beta ", "gamma"]
        );
        assert_eq!(count("alpha beta gamma", &spec(10), &mut glyphs()), 2);
    }

    #[test]
    fn overlong_word_is_hard_broken() {
        let word = "abcdefghijklmnopqrstuvwxy";
        assert_eq!(
            lines(word, &spec(10)),
            ["abcdefghij", "klmnopqrst", "uvwxy"]
        );
    }

    #[test]
    fn cjk_uses_real_width_and_breaks_between_ideographs() {
        let text = "一二三四五六七八九十甲乙丙丁戊";
        let got = lines(text, &spec(20));
        assert_eq!(got, ["一二三四五六七八九十", "甲乙丙丁戊"]);
    }

    #[test]
    fn cjk_mixed_with_ascii_breaks_before_ideograph() {
        // "abc" + 5 个汉字 = 3 + 10 列；宽 8 列：abc + 2 个汉字 = 7 列。
        assert_eq!(lines("abc一二三四五", &spec(8)), ["abc一二", "三四五"]);
    }

    #[test]
    fn empty_line_and_too_wide_single_char_take_one_line_each() {
        assert_eq!(count("", &spec(10), &mut glyphs()), 1);
        // 宽度只够半个汉字：每个视觉行仍放一个字。
        assert_eq!(lines("一二", &spec(1)), ["一", "二"]);
    }

    #[test]
    fn tabs_expand_to_tab_stops_with_columns_continuing_across_lines() {
        // tab=4：「ab\tcd」= ab + 2 空格 + cd = 6 列。
        assert_eq!(count("ab\tcd", &spec(6), &mut glyphs()), 1);
        assert_eq!(count("ab\tcd", &spec(5), &mut glyphs()), 2);
        let w = breaks("abcdef\tg", &spec(4), &mut glyphs());
        // abcd | ef\t（\t 在列 6，展开 2 列，挂在行尾）| g（列 8）
        assert_eq!(w.starts, [(0, 0), (4, 4), (7, 8)]);
    }

    #[test]
    fn visible_whitespace_uses_marker_widths() {
        let mut g = Glyphs::with(|c| match c {
            '·' => 2.0 * W,
            _ => W,
        });
        let mut s = spec(6);
        s.whitespace = true;
        // "a b c"：空格画成 2 宽的 ·，a·b = 4 列宽 + c… 空格不触发折行。
        assert_eq!(count("a b c d", &s, &mut g), 2);
        s.whitespace = false;
        assert_eq!(count("a b c", &s, &mut g), 1);
    }

    #[test]
    fn short_ascii_lines_skip_the_scan() {
        SCANS.with(|s| s.set(0));
        let mut g = glyphs();
        assert_eq!(count("short line", &spec(80), &mut g), 1);
        assert_eq!(SCANS.with(|s| s.get()), 0);
        // 含制表符的短行、非 ASCII 的短行走精确路径。
        assert_eq!(count("a\tb", &spec(80), &mut g), 1);
        assert_eq!(count("一二", &spec(80), &mut g), 1);
        assert_eq!(SCANS.with(|s| s.get()), 2);
    }

    #[test]
    fn cjk_is_not_mistaken_for_a_single_line_by_the_fast_path() {
        // 6 个汉字 18 字节、12 列宽；宽 10 列。
        assert!(!fits_quickly("一二三四五六", &spec(10), &mut glyphs()));
        assert_eq!(count("一二三四五六", &spec(10), &mut glyphs()), 2);
    }

    #[test]
    fn overlong_line_is_capped_and_reports_hidden_chars() {
        let text = "x".repeat(WRAP_CAP as usize * 10 + 25);
        let s = spec(10);
        assert_eq!(count(&text, &s, &mut glyphs()), WRAP_CAP);
        let w = breaks(&text, &s, &mut glyphs());
        assert_eq!(w.len(), WRAP_CAP as usize);
        assert_eq!(w.end, WRAP_CAP as usize * 10);
        assert_eq!(w.hidden_chars, 25);
        assert_eq!(w.segment(w.len() - 1).0, w.end - 10..w.end);
    }

    #[test]
    fn segment_of_locates_bytes() {
        let w = breaks("alpha beta gamma", &spec(10), &mut glyphs());
        assert_eq!(w.segment_of(0), 0);
        assert_eq!(w.segment_of(10), 0);
        assert_eq!(w.segment_of(11), 1);
        assert_eq!(w.segment_of(16), 1);
    }

    /// 合成行：`units[i]` 为精确值，所有行都需扫描 100 字节，估算值恒为 1。
    struct Synthetic {
        units: Vec<u32>,
        measured: usize,
    }
    impl RowSource for Synthetic {
        fn quick(&mut self, row: usize) -> Quick {
            if self.units[row] == 0 {
                Quick::Known(0)
            } else {
                Quick::Measure {
                    bytes: 100,
                    estimate: 1,
                }
            }
        }
        fn measure(&mut self, row: usize) -> u32 {
            self.measured += 1;
            self.units[row]
        }
    }

    #[test]
    fn row_heights_prefix_and_lookup() {
        let mut src = Synthetic {
            units: vec![1, 4, 0, 2, 1],
            measured: 0,
        };
        let t = RowHeights::build(5, &mut src, usize::MAX);
        assert!(t.is_complete());
        assert_eq!(
            (0..=5).map(|r| t.before(r)).collect::<Vec<_>>(),
            [0, 1, 5, 5, 7, 8]
        );
        assert_eq!(t.total(), 8);
        for row in [0, 1, 3, 4] {
            assert_eq!(t.row_at(t.before(row)), row);
            assert_eq!(t.row_at(t.before(row + 1) - 1), row);
        }
        assert_eq!(t.row_at(100), 4);
    }

    #[test]
    fn budget_estimates_then_refine_and_advance_converge() {
        let n = 1_000_000;
        let units: Vec<u32> = (0..n).map(|i| (i % 3 + 1) as u32).collect();
        let mut src = Synthetic {
            units: units.clone(),
            measured: 0,
        };
        let mut t = RowHeights::build(n, &mut src, SYNC_BUDGET);
        let exact = SYNC_BUDGET.div_ceil(100);
        assert_eq!(src.measured, exact);
        assert!(!t.is_complete());
        // 估算部分为 1。
        assert_eq!(t.units(n - 1), 1);
        // 视口附近精确。
        assert!(t.refine(n - 50..n, &mut src));
        assert_eq!(t.units(n - 1), units[n - 1]);
        let mut rounds = 0;
        while !t.is_complete() {
            t.advance(&mut src, 2 * 1024 * 1024);
            rounds += 1;
        }
        assert!(rounds > 1);
        let full = RowHeights::build(
            n,
            &mut Synthetic {
                units,
                measured: 0,
            },
            usize::MAX,
        );
        assert_eq!(t.total(), full.total());
        assert_eq!(t.before(n / 2), full.before(n / 2));
    }

    #[test]
    fn remeasure_updates_prefix() {
        let mut src = Synthetic {
            units: vec![1, 1, 1],
            measured: 0,
        };
        let mut t = RowHeights::build(3, &mut src, usize::MAX);
        src.units[1] = 3;
        t.remeasure(1..2, &mut src);
        assert_eq!(t.before(2), 4);
        assert_eq!(t.total(), 5);
    }
}
