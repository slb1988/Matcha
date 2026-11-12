//! 任务 3.2：hex-report 黄金对拍（期望文件由参考工具 5.0.4.30422 GUI 报告生成）。
//!
//! 语料生成器与 bc-core 共享（`#[path]` 引用，单一来源）。

#[path = "../../bc-core/tests/golden/hex/corpus.rs"]
#[allow(dead_code)] // 生成器与 bc-core 共享；本测试只用 primary()
mod corpus;

use bc_core::align::{ByteAlign, ByteAlignStrategy};
use bc_core::DiffKind;
use std::path::PathBuf;

// ---- 任务 3.2：hex-report 黄金对拍 ----
//
// 期望文件由参考工具 5.0.4.30422 的 GUI 报告（纯文本输出）生成，与本模块语料同字节。
// 布局参数：side-by-side + display-all/display-mismatches + line-numbers（报告默认 24 格/行）、
// summary + display-all、interleaved + display-all + line-numbers。

mod report {
    /// 一行 side-by-side 报告内容行。
    #[derive(Debug, PartialEq)]
    pub struct SbsRow {
        pub left_addr: Option<usize>,
        pub left_cells: [Option<u8>; 24],
        pub right_addr: Option<usize>,
        pub right_cells: [Option<u8>; 24],
        /// `<>` = 差异块首行；`=` = 相同块首行；其余行无标记。
        pub differs: bool,
    }

    fn is_hex(text: &str) -> bool {
        !text.is_empty() && text.bytes().all(|b| b.is_ascii_hexdigit())
    }

    fn addr(text: Option<&str>) -> Option<usize> {
        let text = text?.trim();
        if text.is_empty() {
            return None;
        }
        if !is_hex(text) {
            return None; // 表头/中文行
        }
        Some(usize::from_str_radix(text, 16).unwrap())
    }

    fn cell(line: &str, at: usize) -> Option<u8> {
        let text = line.get(at..at + 2)?.trim();
        if text.is_empty() || !is_hex(text) {
            return None;
        }
        Some(u8::from_str_radix(text, 16).unwrap())
    }

    /// 24 格：每格 3 字符，每 8 格一个分组空格。
    fn cells24(line: &str, base: usize) -> [Option<u8>; 24] {
        let mut out = [None; 24];
        for (k, slot) in out.iter_mut().enumerate() {
            *slot = cell(line, base + k * 3 + k / 8);
        }
        out
    }

    /// 解析 24 格 side-by-side 内容行（短行/全空侧照常）；表头/分隔线返回 None。
    pub fn parse_sbs_line(line: &str) -> Option<SbsRow> {
        if line.starts_with('-') {
            return None;
        }
        let left_addr = addr(line.get(0..8));
        let right_addr = addr(line.get(112..120));
        if left_addr.is_none() && right_addr.is_none() {
            return None;
        }
        let (left_cells, right_cells) = (cells24(line, 9), cells24(line, 121));
        if left_cells.iter().chain(&right_cells).all(Option::is_none) {
            return None;
        }
        Some(SbsRow {
            left_addr,
            left_cells,
            right_addr,
            right_cells,
            differs: line.contains(" <> "),
        })
    }

    pub fn parse_sbs(text: &str) -> Vec<SbsRow> {
        text.lines().filter_map(parse_sbs_line).collect()
    }
}

#[test]
fn golden_side_by_side_matches_reference_alignment() {
    use bc_engine_bin::hex::{pack_slice, HexCell};
    let (left, right) = corpus::primary();
    let expected = report::parse_sbs(
        &std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bc-core/tests/golden/hex/expected/sbs-all.txt"),
        )
        .unwrap(),
    );
    // 报告默认 24 格/行：与我们的打包模型逐行对照。
    let map = ByteAlign.align(&left, &right, ByteAlignStrategy::Complete);
    let packed = pack_slice(map.rows(), 24);
    assert_eq!(packed.len(), expected.len(), "行数不一致");
    for (i, (row, want)) in packed.iter().zip(&expected).enumerate() {
        assert_eq!(row.left_label, want.left_addr, "行 {i} 左标签");
        assert_eq!(row.right_label, want.right_addr, "行 {i} 右标签");
        for (k, cell) in row.cells.iter().enumerate() {
            let (wl, wr) = (want.left_cells[k], want.right_cells[k]);
            match *cell {
                HexCell::Pair { left: l, right: r, kind } => {
                    assert_eq!(wl, Some(left[l]), "行 {i} 格 {k} 左字节");
                    assert_eq!(wr, Some(right[r]), "行 {i} 格 {k} 右字节");
                    let expect_diff = wl != wr;
                    assert_eq!(kind == DiffKind::Changed, expect_diff, "行 {i} 格 {k} 差异判定");
                }
                HexCell::LeftOrphan(l) => {
                    assert_eq!(wl, Some(left[l]), "行 {i} 格 {k} 左孤儿");
                    assert_eq!(wr, None, "行 {i} 格 {k} 右侧应为占位");
                }
                HexCell::RightOrphan(r) => {
                    assert_eq!(wl, None, "行 {i} 格 {k} 左侧应为占位");
                    assert_eq!(wr, Some(right[r]), "行 {i} 格 {k} 右孤儿");
                }
            }
        }
    }
}

/// 报告 display-mismatches：每个差异区域自带 16 字节上下文、**自区域起点重新打包**。
#[test]
fn golden_mismatches_only_lists_regions_repacked_from_their_starts() {
    use bc_engine_bin::hex::{filtered_regions, pack_slice, DisplayFilter};
    let (left, right) = corpus::primary();
    let map = ByteAlign.align(&left, &right, ByteAlignStrategy::Complete);
    let mut ours = Vec::new();
    for region in filtered_regions(&map, DisplayFilter::Differences, 16) {
        // 报告显示区域的全部行（含区域内纯上下文的行）。
        ours.extend(pack_slice(&map.rows()[region], 24));
    }
    let expected = report::parse_sbs(
        &std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bc-core/tests/golden/hex/expected/sbs-mis.txt"),
        )
        .unwrap(),
    );
    assert_eq!(ours.len(), expected.len(), "差异行数不一致（报告为每区域重打包）");
    for (i, (row, want)) in ours.iter().zip(&expected).enumerate() {
        assert_eq!(row.left_label, want.left_addr, "行 {i} 左标签");
        assert_eq!(row.right_label, want.right_addr, "行 {i} 右标签");
    }
}

#[test]
fn golden_summary_counts_match_stats() {
    let text = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bc-core/tests/golden/hex/expected/sum-all.txt"),
    )
    .unwrap();
    let num = |marker: &str| -> u64 {
        text.lines()
            .find(|l| l.contains(marker))
            .and_then(|l| l.split_whitespace().next()?.parse().ok())
            .unwrap_or_else(|| panic!("缺行: {marker}"))
    };
    assert_eq!(num("个相同字节"), 4120);
    assert_eq!(num("个左边独有字节"), 32);
    assert_eq!(num("个右边独有字节"), 16);
    assert_eq!(num("个差异字节"), 8);
}

#[test]
fn golden_interleaved_rebuilds_both_streams() {
    let (left, right) = corpus::primary();
    let text = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bc-core/tests/golden/hex/expected/int-all.txt"),
    )
    .unwrap();
    let hex_addr = |s: Option<&str>| -> Option<()> {
        let s = s?.trim();
        if s.is_empty() || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        Some(())
    };
    // 交错格式：LADDR RADDR 后跟该行字节；孤儿行仅一个地址列有值。
    let (mut left_stream, mut right_stream) = (Vec::new(), Vec::new());
    for line in text.lines() {
        if line.starts_with('-') {
            continue;
        }
        let has_left = hex_addr(line.get(0..8)).is_some();
        let has_right = hex_addr(line.get(9..17)).is_some();
        if !has_left && !has_right {
            continue;
        }
        // 字节区固定第 18 列起、24 格（分组空格在每 12 格后）。
        let mut bytes = Vec::new();
        for k in 0..24 {
            let off = 18 + k * 3 + k / 8;
            if let Some(two) = line.get(off..off + 2) {
                let two = two.trim();
                if !two.is_empty() && two.bytes().all(|b| b.is_ascii_hexdigit()) {
                    bytes.push(u8::from_str_radix(two, 16).unwrap());
                }
            }
        }
        if has_left {
            left_stream.extend_from_slice(&bytes);
        }
        if has_right {
            right_stream.extend_from_slice(&bytes);
        }
    }
    assert_eq!(left_stream, left);
    assert_eq!(right_stream, right);
}
