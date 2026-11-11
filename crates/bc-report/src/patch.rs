//! `patch` 布局：普通 / 上下文 / 统一差异格式。
//!
//! 字节格式以参考工具样本为准（`tools/golden/README.md` 的“patch 报告协议”）：GNU diff 的
//! 三种格式，但上下文为 [`CONTEXT`] = 6 行；相邻差异之间不变行不超过 [`MERGE_GAP`] = 12 时并入
//! 同一差异块；末个差异之后剩余不超过 12 行时该块延伸到文件末尾（文件开头无此规则）。每行以
//! CRLF 结尾；任一输入带 BOM 时以 UTF-8 BOM 开头；两侧无差异时只输出文件头。
//!
//! 渲染分两步：[`hunks`] 从对齐行得到差异段与差异块，[`render`] 按格式写出。两步都只依赖
//! [`PatchRow`] 与取行文本的回调，便于脱离文本引擎单测。

use crate::{PatchFormat, ReportFile};
use bc_core::Side;
use bc_engine_text::{Importance, TextMap};
use std::fmt::Write as _;
use std::ops::Range;

/// 每个差异块在首个差异前、末个差异后带的不变行数。
pub const CONTEXT: usize = 6;
/// 两处差异之间不变行不超过此数时并入同一块；也用于文件末尾的延伸判定。
pub const MERGE_GAP: usize = 2 * CONTEXT;
/// 报告的换行符（参考工具在 Windows 上的输出）。
pub const EOL: &str = "\r\n";

/// 一条对齐行中 patch 需要的信息。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatchRow {
    /// 左侧行号（0 起），该行左侧缺失时为 `None`。
    pub left: Option<usize>,
    pub right: Option<usize>,
    /// 计为差异（开始或延续一个差异段）。
    pub diff: bool,
}

/// 一个差异块：覆盖的行区间与其中的差异段（均为行下标的半开区间）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub rows: Range<usize>,
    pub segments: Vec<Range<usize>>,
}

/// 文本引擎的对齐行 → [`PatchRow`]。`ignore_unimportant` 时只含不重要差异的行不算差异。
pub fn rows_of(map: &TextMap, ignore_unimportant: bool) -> Vec<PatchRow> {
    map.rows()
        .map(|row| PatchRow {
            left: row.left,
            right: row.right,
            diff: row.importance.with_ignore_unimportant(ignore_unimportant) != Importance::Same,
        })
        .collect()
}

/// 连续差异行组成的差异段。
pub fn segments(rows: &[PatchRow]) -> Vec<Range<usize>> {
    let mut segments: Vec<Range<usize>> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if !row.diff {
            continue;
        }
        match segments.last_mut() {
            Some(last) if last.end == index => last.end = index + 1,
            _ => segments.push(index..index + 1),
        }
    }
    segments
}

/// 按上下文与合并规则把差异段组成差异块。
pub fn hunks(rows: &[PatchRow]) -> Vec<Hunk> {
    let mut hunks: Vec<Hunk> = Vec::new();
    for segment in segments(rows) {
        match hunks.last_mut() {
            Some(hunk) if segment.start - hunk.rows.end <= MERGE_GAP => {
                hunk.rows.end = segment.end;
                hunk.segments.push(segment);
            }
            _ => hunks.push(Hunk {
                rows: segment.clone(),
                segments: vec![segment],
            }),
        }
    }
    let total = rows.len();
    let count = hunks.len();
    for (index, hunk) in hunks.iter_mut().enumerate() {
        let end = if index + 1 == count && total - hunk.rows.end <= MERGE_GAP {
            total
        } else {
            (hunk.rows.end + CONTEXT).min(total)
        };
        hunk.rows = hunk.rows.start.saturating_sub(CONTEXT)..end;
    }
    hunks
}

/// 每侧在行下标 `i` 之前已有的行数（前缀和），用于范围计算。
struct Counts {
    left: Vec<usize>,
    right: Vec<usize>,
}

impl Counts {
    fn new(rows: &[PatchRow]) -> Self {
        let mut counts = Counts {
            left: Vec::with_capacity(rows.len() + 1),
            right: Vec::with_capacity(rows.len() + 1),
        };
        let (mut left, mut right) = (0, 0);
        counts.left.push(0);
        counts.right.push(0);
        for row in rows {
            left += usize::from(row.left.is_some());
            right += usize::from(row.right.is_some());
            counts.left.push(left);
            counts.right.push(right);
        }
        counts
    }

    /// (区间之前的行数, 区间内的行数)。
    fn span(&self, side: Side, rows: &Range<usize>) -> (usize, usize) {
        let prefix = if side == Side::Left {
            &self.left
        } else {
            &self.right
        };
        (prefix[rows.start], prefix[rows.end] - prefix[rows.start])
    }
}

/// normal 与 context 的范围：`起,止`；单行只写行号；空范围写区间之前的行号。
fn plain_range((before, count): (usize, usize)) -> String {
    match count {
        0 => before.to_string(),
        1 => (before + 1).to_string(),
        _ => format!("{},{}", before + 1, before + count),
    }
}

/// normal 的范围：同 [`plain_range`]，唯一例外是空范围之前恰为 1 行时写 `0`——参考工具如此
/// （`patch-extra/106、107、109、110` 与 `2way/014`：`2d0`、`0a2`、`2,3d0`；之前为 0、2、3、5 行
/// 时与 GNU 相同，见 `2way/004、006、009` 与 `patch-extra/105、108`）。
fn normal_range((before, count): (usize, usize)) -> String {
    if count == 0 && before == 1 {
        "0".into()
    } else {
        plain_range((before, count))
    }
}

/// unified 的范围：`起,行数`；单行省略行数；空范围写 `<之前的行号>,0`。
fn unified_range((before, count): (usize, usize)) -> String {
    match count {
        0 => format!("{before},0"),
        1 => (before + 1).to_string(),
        _ => format!("{},{count}", before + 1),
    }
}

/// 行在差异块中的角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    /// 两侧都在、不属于差异段。
    Context,
    /// 差异段内且该段两侧都有行（context 格式的 `!`）。
    Changed,
    /// 只有被删除/插入的一侧（差异段内，或被忽略的单侧不重要行）。
    Removed,
    Added,
}

fn marks(rows: &[PatchRow], hunk: &Hunk) -> Vec<Mark> {
    let mut marks = Vec::with_capacity(hunk.rows.len());
    for index in hunk.rows.clone() {
        let row = rows[index];
        let segment = hunk.segments.iter().find(|s| s.contains(&index));
        let mark = match segment {
            Some(segment) => {
                let both = rows[segment.clone()].iter().any(|r| r.left.is_some())
                    && rows[segment.clone()].iter().any(|r| r.right.is_some());
                if both {
                    Mark::Changed
                } else if row.left.is_some() {
                    Mark::Removed
                } else {
                    Mark::Added
                }
            }
            // 被忽略的单侧行：仍按删除/插入写出，保持 patch 可应用（参考未采样）。
            None => match (row.left, row.right) {
                (Some(_), None) => Mark::Removed,
                (None, Some(_)) => Mark::Added,
                _ => Mark::Context,
            },
        };
        marks.push(mark);
    }
    marks
}

/// 写出完整报告。`text(side, line)` 返回某侧第 `line` 行（0 起）的正文，不含换行符。
pub fn render(
    format: PatchFormat,
    rows: &[PatchRow],
    text: &dyn Fn(Side, usize) -> String,
    files: &[ReportFile; 2],
    bom: bool,
) -> String {
    let mut out = String::new();
    if bom {
        out.push('\u{feff}');
    }
    let [left, right] = files;
    let counts = Counts::new(rows);
    let line = |out: &mut String, prefix: &str, body: &str| {
        let _ = write!(out, "{prefix}{body}{EOL}");
    };
    match format {
        PatchFormat::Normal => {
            line(
                &mut out,
                "",
                &format!("diff -r {} {}", left.path, right.path),
            );
            for segment in segments(rows) {
                let (l, r) = (
                    counts.span(Side::Left, &segment),
                    counts.span(Side::Right, &segment),
                );
                let op = match (l.1, r.1) {
                    (0, _) => 'a',
                    (_, 0) => 'd',
                    _ => 'c',
                };
                line(
                    &mut out,
                    "",
                    &format!("{}{op}{}", normal_range(l), normal_range(r)),
                );
                for row in &rows[segment.clone()] {
                    if let Some(index) = row.left {
                        line(&mut out, "< ", &text(Side::Left, index));
                    }
                }
                if op == 'c' {
                    line(&mut out, "", "---");
                }
                for row in &rows[segment] {
                    if let Some(index) = row.right {
                        line(&mut out, "> ", &text(Side::Right, index));
                    }
                }
            }
        }
        PatchFormat::Context => {
            line(
                &mut out,
                "*** ",
                &format!("{}\t{}", left.path, left.modified),
            );
            line(
                &mut out,
                "--- ",
                &format!("{}\t{}", right.path, right.modified),
            );
            for hunk in hunks(rows) {
                let marks = marks(rows, &hunk);
                line(&mut out, "", "***************");
                for (side, open, close) in [
                    (Side::Left, "*** ", " ****"),
                    (Side::Right, "--- ", " ----"),
                ] {
                    let span = counts.span(side, &hunk.rows);
                    line(&mut out, open, &format!("{}{close}", plain_range(span)));
                    let changed = |mark: Mark| match side {
                        Side::Left => matches!(mark, Mark::Changed | Mark::Removed),
                        _ => matches!(mark, Mark::Changed | Mark::Added),
                    };
                    if !marks.iter().any(|&mark| changed(mark)) {
                        continue;
                    }
                    for (offset, &mark) in marks.iter().enumerate() {
                        let row = rows[hunk.rows.start + offset];
                        let Some(index) = (if side == Side::Left {
                            row.left
                        } else {
                            row.right
                        }) else {
                            continue;
                        };
                        let prefix = match mark {
                            Mark::Context => "  ",
                            Mark::Changed => "! ",
                            Mark::Removed => "- ",
                            Mark::Added => "+ ",
                        };
                        line(&mut out, prefix, &text(side, index));
                    }
                }
            }
        }
        PatchFormat::Unified => {
            line(
                &mut out,
                "--- ",
                &format!("{}\t{}", left.path, left.modified),
            );
            line(
                &mut out,
                "+++ ",
                &format!("{}\t{}", right.path, right.modified),
            );
            for hunk in hunks(rows) {
                let marks = marks(rows, &hunk);
                line(
                    &mut out,
                    "",
                    &format!(
                        "@@ -{} +{} @@",
                        unified_range(counts.span(Side::Left, &hunk.rows)),
                        unified_range(counts.span(Side::Right, &hunk.rows))
                    ),
                );
                let mut index = hunk.rows.start;
                while index < hunk.rows.end {
                    let mark = marks[index - hunk.rows.start];
                    if let Some(segment) = hunk.segments.iter().find(|s| s.start == index) {
                        // 一个差异段：先全部删除行，再全部插入行。
                        for row in &rows[segment.clone()] {
                            if let Some(line_index) = row.left {
                                line(&mut out, "-", &text(Side::Left, line_index));
                            }
                        }
                        for row in &rows[segment.clone()] {
                            if let Some(line_index) = row.right {
                                line(&mut out, "+", &text(Side::Right, line_index));
                            }
                        }
                        index = segment.end;
                        continue;
                    }
                    let row = rows[index];
                    match (mark, row.left, row.right) {
                        (Mark::Removed, Some(l), _) => line(&mut out, "-", &text(Side::Left, l)),
                        (Mark::Added, _, Some(r)) => line(&mut out, "+", &text(Side::Right, r)),
                        (_, Some(l), _) => line(&mut out, " ", &text(Side::Left, l)),
                        (_, None, Some(r)) => line(&mut out, " ", &text(Side::Right, r)),
                        _ => {}
                    }
                    index += 1;
                }
            }
        }
    }
    out
}

/// [`render`] on a compared text map: line text comes from both contents (stored as UTF-8),
/// and the BOM follows the reference rule (any input had one).
pub fn render_map(
    map: &TextMap,
    files: &[ReportFile; 2],
    format: PatchFormat,
    ignore_unimportant: bool,
) -> String {
    let rows = rows_of(map, ignore_unimportant);
    let left = map.left.lines.borrow();
    let right = map.right.lines.borrow();
    let text = |side: Side, index: usize| {
        let lines = if side == Side::Left { &left } else { &right };
        String::from_utf8_lossy(lines.line_bytes(index).unwrap_or_default()).into_owned()
    };
    let bom = [&map.left, &map.right]
        .iter()
        .any(|content| content.detection.as_ref().is_some_and(|d| d.has_bom));
    render(format, &rows, &text, files, bom)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `=` 相同、`-` 仅左、`+` 仅右、`~` 两侧改动、`h` 被忽略的仅左行（非差异）。
    fn rows(spec: &str) -> Vec<PatchRow> {
        let (mut l, mut r) = (0, 0);
        let mut next = |left: bool, right: bool, diff: bool| {
            let row = PatchRow {
                left: left.then_some(l),
                right: right.then_some(r),
                diff,
            };
            l += usize::from(left);
            r += usize::from(right);
            row
        };
        spec.chars()
            .map(|c| match c {
                '=' => next(true, true, false),
                '-' => next(true, false, true),
                '+' => next(false, true, true),
                '~' => next(true, true, true),
                'h' => next(true, false, false),
                other => panic!("bad spec {other}"),
            })
            .collect()
    }

    fn text(side: Side, index: usize) -> String {
        format!(
            "{}{}",
            if side == Side::Left { "L" } else { "R" },
            index + 1
        )
    }

    fn files() -> [ReportFile; 2] {
        let file = |path: &str| ReportFile {
            path: path.into(),
            modified: "2026-01-02 03:04:05.000000000 +0800".into(),
        };
        [file("C:\\l.txt"), file("C:\\r.txt")]
    }

    fn patch(format: PatchFormat, spec: &str) -> String {
        render(format, &rows(spec), &text, &files(), false)
    }

    fn body(text: &str, header_lines: usize) -> Vec<&str> {
        text.split(EOL)
            .skip(header_lines)
            .filter(|l| !l.is_empty())
            .collect()
    }

    #[test]
    fn segments_are_runs_of_difference_rows() {
        assert_eq!(segments(&rows("==~-==+")), vec![2..4, 6..7]);
        assert!(segments(&rows("====")).is_empty());
        assert!(segments(&rows("=h=")).is_empty());
    }

    #[test]
    fn hunks_merge_up_to_twelve_unchanged_rows() {
        let gap = |n: usize| format!("{}~{}~{}", "=".repeat(20), "=".repeat(n), "=".repeat(20));
        assert_eq!(hunks(&rows(&gap(12))).len(), 1);
        assert_eq!(hunks(&rows(&gap(13))).len(), 2);
    }

    #[test]
    fn context_is_six_rows_and_the_tail_extends_only_up_to_twelve() {
        // 首差异前 10 行：从第 4 行（下标）开始，不延伸到文件开头。
        let start = hunks(&rows(&format!("{}~{}", "=".repeat(10), "=".repeat(20))));
        assert_eq!(start[0].rows, 4..17);
        let tail = |n: usize| hunks(&rows(&format!("~{}", "=".repeat(n))))[0].rows.clone();
        assert_eq!(tail(12), 0..13);
        assert_eq!(tail(13), 0..7);
    }

    #[test]
    fn identical_inputs_print_headers_only() {
        for format in [
            PatchFormat::Normal,
            PatchFormat::Context,
            PatchFormat::Unified,
        ] {
            let out = patch(format, "===");
            let headers = if format == PatchFormat::Normal { 1 } else { 2 };
            assert_eq!(out.matches(EOL).count(), headers, "{format:?}: {out}");
        }
        assert_eq!(
            patch(PatchFormat::Normal, ""),
            "diff -r C:\\l.txt C:\\r.txt\r\n"
        );
    }

    #[test]
    fn unified_header_and_hunk() {
        let out = patch(PatchFormat::Unified, "==++==");
        assert!(out.starts_with(
            "--- C:\\l.txt\t2026-01-02 03:04:05.000000000 +0800\r\n\
             +++ C:\\r.txt\t2026-01-02 03:04:05.000000000 +0800\r\n"
        ));
        assert_eq!(
            body(&out, 2),
            ["@@ -1,4 +1,6 @@", " L1", " L2", "+R3", "+R4", " L3", " L4"]
        );
    }

    #[test]
    fn unified_ranges_for_single_and_empty_sides() {
        assert_eq!(
            body(&patch(PatchFormat::Unified, "~"), 2),
            ["@@ -1 +1 @@", "-L1", "+R1"]
        );
        assert_eq!(
            body(&patch(PatchFormat::Unified, "++"), 2),
            ["@@ -0,0 +1,2 @@", "+R1", "+R2"]
        );
        assert_eq!(
            body(&patch(PatchFormat::Unified, "--"), 2),
            ["@@ -1,2 +0,0 @@", "-L1", "-L2"]
        );
    }

    #[test]
    fn unified_lists_removals_before_additions_within_a_segment() {
        assert_eq!(
            body(&patch(PatchFormat::Unified, "=-+-+="), 2),
            ["@@ -1,4 +1,4 @@", " L1", "-L2", "-L3", "+R2", "+R3", " L4"]
        );
    }

    #[test]
    fn normal_operations_and_ranges() {
        assert_eq!(
            body(&patch(PatchFormat::Normal, "++"), 1),
            ["0a1,2", "> R1", "> R2"]
        );
        assert_eq!(
            body(&patch(PatchFormat::Normal, "--"), 1),
            ["1,2d0", "< L1", "< L2"]
        );
        assert_eq!(
            body(&patch(PatchFormat::Normal, "=~~="), 1),
            ["2,3c2,3", "< L2", "< L3", "---", "> R2", "> R3"]
        );
        // 参考的例外：空范围之前恰为 1 行时写 0。
        assert_eq!(
            body(&patch(PatchFormat::Normal, "=-=="), 1),
            ["2d0", "< L2"]
        );
        assert_eq!(
            body(&patch(PatchFormat::Normal, "=+=="), 1),
            ["0a2", "> R2"]
        );
        assert_eq!(
            body(&patch(PatchFormat::Normal, "=--="), 1),
            ["2,3d0", "< L2", "< L3"]
        );
        assert_eq!(
            body(&patch(PatchFormat::Normal, "==-="), 1),
            ["3d2", "< L3"]
        );
        // 不带上下文：相隔一行的两处差异各成一块。
        assert_eq!(
            body(&patch(PatchFormat::Normal, "~=~"), 1),
            ["1c1", "< L1", "---", "> R1", "3c3", "< L3", "---", "> R3"]
        );
    }

    #[test]
    fn context_sections_and_marks() {
        assert_eq!(
            body(&patch(PatchFormat::Context, "==+="), 2),
            // 新文件一节的上下文行取右侧正文（真实输入里与左侧相同）。
            [
                "***************",
                "*** 1,3 ****",
                "--- 1,4 ----",
                "  R1",
                "  R2",
                "+ R3",
                "  R4"
            ]
        );
        assert_eq!(
            body(&patch(PatchFormat::Context, "=-="), 2),
            [
                "***************",
                "*** 1,3 ****",
                "  L1",
                "- L2",
                "  L3",
                "--- 1,2 ----"
            ]
        );
        assert_eq!(
            body(&patch(PatchFormat::Context, "~"), 2),
            [
                "***************",
                "*** 1 ****",
                "! L1",
                "--- 1 ----",
                "! R1"
            ]
        );
        assert_eq!(
            body(&patch(PatchFormat::Context, "++"), 2),
            [
                "***************",
                "*** 0 ****",
                "--- 1,2 ----",
                "+ R1",
                "+ R2"
            ]
        );
    }

    #[test]
    fn ignored_one_sided_rows_alone_produce_no_hunk() {
        for format in [
            PatchFormat::Normal,
            PatchFormat::Context,
            PatchFormat::Unified,
        ] {
            let out = patch(format, "=h=");
            assert!(
                !out.contains("@@") && !out.contains("*******") && !out.contains("1d"),
                "{out}"
            );
        }
    }

    #[test]
    fn bom_prefix() {
        let out = render(PatchFormat::Normal, &rows("="), &text, &files(), true);
        assert!(out.starts_with("\u{feff}diff -r "));
        assert!(out.as_bytes().starts_with(b"\xef\xbb\xbf"));
    }
}
