use super::*;
use bc_core::cancel::CancelCheck;
use std::io::Write;

fn map_of(left: &[u8], right: &[u8]) -> HexMap {
    let mut map = HexMap {
        left: HexContent::from_bytes(left.to_vec()),
        right: HexContent::from_bytes(right.to_vec()),
        ..Default::default()
    };
    map.align();
    map
}

fn corpus(name: &str) -> Vec<u8> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/reference-parity/hex-compare/corpus/").to_owned() + name;
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

/// 临时目录（沿用 bc-engine-dir 测试的 temp_dir + 唯一名约定）。
struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        let salt = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("matcha-hex-{}-{salt}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn write(&self, name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = self.0.join(name);
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(bytes).unwrap();
        path
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn identical_files_are_all_same() {
    let map = map_of(b"hello world", b"hello world");
    assert_eq!(map.stats(), HexStats { same: 11, ..Default::default() });
    assert_eq!(map.stats().summary(), "Binary same");
}

#[test]
fn mid_insert_becomes_right_orphans() {
    let left = corpus("left.bin");
    let mut right = left.clone();
    right.splice(0x400..0x400, 0..16);
    let map = map_of(&left, &right);
    assert_eq!(map.stats(), HexStats { same: 4160, right_orphan: 16, ..Default::default() });
}

#[test]
fn mid_delete_becomes_left_orphans() {
    // 用无重复内容的参考语料基底：周期内容会让锚窗在倍数相位误锚。
    let left = corpus("left.bin");
    let mut right = left.clone();
    right.splice(0x5F0..0x610, []);
    let map = map_of(&left, &right);
    assert_eq!(map.stats(), HexStats { same: 4128, left_orphan: 32, ..Default::default() });
}

#[test]
fn odd_insert_keeps_pairing() {
    // 周期 256 的内容会让 50 字节锚窗在错位 256 倍数处误锚——改用无重复内容。
    let left = corpus("l2.bin");
    let mut right = left.clone();
    right.splice(0x123..0x123, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE]);
    let map = map_of(&left, &right);
    let stats = map.stats();
    assert_eq!(stats.right_orphan, 5);
    assert_eq!(stats.same, 2048);
    assert_eq!(stats.difference + stats.left_orphan, 0);
}

#[test]
fn reference_corpus_stats_match() {
    let map = map_of(&corpus("left.bin"), &corpus("right.bin"));
    assert_eq!(
        map.stats(),
        HexStats { same: 4120, difference: 8, left_orphan: 32, right_orphan: 16 }
    );
    assert_eq!(map.stats().summary(), "Binary differences");
}

#[test]
fn default_rules_are_complete() {
    assert_eq!(HexRules::default().alignment, ByteAlignStrategy::Complete);
    assert_eq!(HexRules::default().lock_threshold, 2 * 1024 * 1024);
    assert_eq!(HexRules::default().context_count, 16);
}

#[test]
fn none_aligns_positionally() {
    let mut right = b"abcde".to_vec();
    right[2] = b'X';
    right.extend_from_slice(b"12345");
    let mut map = HexMap {
        left: HexContent::from_bytes(b"abcde".to_vec()),
        right: HexContent::from_bytes(right),
        rules: HexRules { alignment: ByteAlignStrategy::None, ..Default::default() },
        ..Default::default()
    };
    map.align();
    assert_eq!(map.stats(), HexStats { same: 4, difference: 1, right_orphan: 5, ..Default::default() });
}

#[test]
fn switching_strategy_realigns_without_touching_content() {
    let left = corpus("left.bin");
    let right = corpus("right.bin");
    let mut map = map_of(&left, &right);
    map.rules.alignment = ByteAlignStrategy::None;
    map.align();
    let none_stats = map.stats();
    assert!(none_stats.difference >= 8); // 错位放大差异，但内容不变
    assert_eq!(map.left.bytes(), left);
    assert_eq!(map.right.bytes(), right);
    map.rules.alignment = ByteAlignStrategy::Complete;
    map.align();
    assert_eq!(map.stats().same, 4120);
}

#[test]
fn edit_then_realign_updates_classification() {
    let left = corpus("l2.bin");
    let mut right = left.clone();
    right[0x100] ^= 0xFF;
    let mut map = map_of(&left, &right);
    assert_eq!(map.stats().difference, 1);
    // 编辑右侧使差异消失
    let byte = left[0x100];
    map.right.replace(0x100..0x101, &[byte]).unwrap();
    map.align();
    assert_eq!(map.stats(), HexStats { same: 2048, ..Default::default() });
}

#[test]
fn align_reports_cancellation() {
    struct Always;
    impl CancelCheck for Always {
        fn is_cancelled(&self) -> bool {
            true
        }
    }
    let mut map = HexMap {
        left: HexContent::from_bytes(vec![0; 4096]),
        right: HexContent::from_bytes(vec![1; 4096]),
        ..Default::default()
    };
    assert!(matches!(map.align_cancellable(&Always), Err(CoreError::Cancelled)));
}

#[test]
fn pack_rows_full_row_orphan_block() {
    // 16 字节整行插入：右侧独占一行红色孤儿格，左侧整行占位且无标签。
    let map = map_of(&corpus("left.bin"), &corpus("right.bin"));
    let rows = map.pack_rows();
    // 0x400 之前的配对恰好 0x400/16 = 64 行。
    let row = &rows[64];
    assert!(row.cells.iter().all(|c| matches!(c, HexCell::RightOrphan(_))));
    assert_eq!(row.cells.len(), 16);
    assert_eq!(row.left_label, None);
    assert_eq!(row.right_label, Some(0x400));
    // 下一行：左 0x400 与右 0x410 配对同一内容。
    let next = &rows[65];
    assert_eq!(next.left_label, Some(0x400));
    assert_eq!(next.right_label, Some(0x410));
    match next.cells[0] {
        HexCell::Pair { left, right, kind } => {
            assert_eq!((left, right), (0x400, 0x410));
            assert_eq!(kind, DiffKind::Same);
        }
        other => panic!("expected pair, got {other:?}"),
    }
}

#[test]
fn pack_rows_mid_row_orphans_shift_phase() {
    // 5 字节行中插入：孤儿格在行中间，插入点后行标签相位偏移。
    let map = map_of(&corpus("l2.bin"), &corpus("r2.bin"));
    let rows = map.pack_rows();
    // 0x123 落在第 18 行（0x120..0x130）的第 3 格。
    let row = &rows[18];
    assert_eq!(row.left_label, Some(0x120));
    assert_eq!(row.right_label, Some(0x120));
    for k in 0..3 {
        assert!(matches!(row.cells[k], HexCell::Pair { kind: DiffKind::Same, .. }));
    }
    for k in 3..8 {
        assert_eq!(row.cells[k], HexCell::RightOrphan(0x123 + (k - 3)));
    }
    match row.cells[8] {
        HexCell::Pair { left, right, .. } => assert_eq!((left, right), (0x123, 0x128)),
        other => panic!("expected pair, got {other:?}"),
    }
    // 相位偏移：下一行左标签 0x12B（…B 相位），右标签 0x130（保持 16 对齐）。
    assert_eq!(rows[19].left_label, Some(0x12B));
    assert_eq!(rows[19].right_label, Some(0x130));
}

// ---- 任务 2.1：加载分层与锁语义 ----

#[test]
fn load_below_threshold_loads_writable_whole_file() {
    let temp = Temp::new();
    let path = temp.write("small.bin", &corpus("l2.bin"));
    let content = HexContent::load(&path, 2 * 1024 * 1024).unwrap();
    assert!(!content.is_large());
    assert_eq!(content.bytes(), corpus("l2.bin"));
    assert!(content.ensure_writable().is_ok());
    // 小文件不加锁：外部程序可同时修改，重载拿到新内容。
    std::fs::write(&path, b"external edit").unwrap();
    let reloaded = HexContent::load(&path, 2 * 1024 * 1024).unwrap();
    assert_eq!(reloaded.bytes(), b"external edit");
}

#[test]
fn load_at_threshold_maps_readonly_and_rejects_writes() {
    let temp = Temp::new();
    let bytes = corpus("left.bin");
    let path = temp.write("large.bin", &bytes);
    // 阈值设为文件大小，使该文件落入 mmap 路径（免去 2 MB 夹具）。
    let content = HexContent::load(&path, bytes.len() as u64).unwrap();
    assert!(content.is_large());
    assert_eq!(content.bytes(), bytes);
    // 大文件编辑/保存被拒绝（设计 D3 初版已知限制）。
    assert!(content.ensure_writable().is_err());
    let mut content = content;
    assert!(content.replace(0..1, &[0]).is_err());
    assert!(content.save().is_err());
    assert!(content.save_as(&temp.0.join("copy.bin")).is_err());
    // mmap 内容可直接对齐。
    let mut map = HexMap {
        left: HexContent::from_bytes(bytes.clone()),
        right: content,
        ..Default::default()
    };
    map.align();
    assert_eq!(map.stats().summary(), "Binary same");
}

// ---- 任务 2.2：后台对齐 job 与 revision 过期拒绝 ----

#[test]
fn stale_alignment_output_is_rejected() {
    let left = corpus("l2.bin");
    let right = corpus("r2.bin");
    let mut map = map_of(&left, &right);
    let job = HexAlignJob::from_map(&map);
    // 快照之后编辑右侧：revision 前进，旧 job 结果必须被拒。
    map.right.replace(0..0, &[0x00]).unwrap();
    let output = job.execute(&bc_core::cancel::NeverCancel).unwrap();
    assert!(map.set_alignment(output).is_err());
    // 新 job 基于当前 revision，回填成功。
    let fresh = HexAlignJob::from_map(&map);
    let output = fresh.execute(&bc_core::cancel::NeverCancel).unwrap();
    map.set_alignment(output).unwrap();
    let stats = map.stats();
    assert_eq!(stats.right_orphan, 6); // 原 5 字节插入 + 新插入 1 字节
}

#[test]
fn rules_change_marks_output_stale() {
    let left = corpus("l2.bin");
    let right = corpus("r2.bin");
    let mut map = map_of(&left, &right);
    let job = HexAlignJob::from_map(&map);
    map.rules.alignment = ByteAlignStrategy::Fast;
    let output = job.execute(&bc_core::cancel::NeverCancel).unwrap();
    assert!(map.set_alignment(output).is_err());
}

// ---- 任务 2.3：保存/另存、Disable editing、Open Clipboard ----

#[test]
fn save_writes_only_the_target_side() {
    let temp = Temp::new();
    let left_path = temp.write("left.bin", &corpus("l2.bin"));
    let right_path = temp.write("right.bin", &corpus("r2.bin"));
    let mut left = HexContent::load(&left_path, u64::MAX).unwrap();
    let right = HexContent::load(&right_path, u64::MAX).unwrap();
    left.replace(0..0, &[0xEE]).unwrap();
    left.save().unwrap();
    // 仅左侧落盘变化；右侧文件不变。
    assert_eq!(std::fs::read(&left_path).unwrap()[0], 0xEE);
    assert_eq!(std::fs::read(&right_path).unwrap(), corpus("r2.bin"));
    drop(right);
}

#[test]
fn save_as_stages_then_renames_and_retargets_source() {
    let temp = Temp::new();
    let path = temp.write("a.bin", b"before");
    let mut content = HexContent::load(&path, u64::MAX).unwrap();
    content.replace(0..6, b"after!").unwrap();
    let other = temp.write("b.bin", b"untouched");
    content.save_as(&other).unwrap();
    assert_eq!(std::fs::read(&other).unwrap(), b"after!");
    assert_eq!(content.source(), Some(other.as_path()));
    // 暂存文件不残留。
    assert!(!temp.0.read_dir().unwrap().any(|e| e.unwrap().file_name().to_string_lossy().contains("matcha-tmp")));
}

#[test]
fn read_only_side_rejects_every_write_path() {
    let temp = Temp::new();
    let path = temp.write("locked.bin", &corpus("l2.bin"));
    let mut side = HexContent::load(&path, u64::MAX).unwrap();
    side.set_read_only(true);
    assert!(side.replace(0..1, &[0]).is_err());
    assert!(side.save().is_err());
    assert!(side.save_as(&temp.0.join("elsewhere.bin")).is_err());
    // 另一侧不受影响。
    let mut other = HexContent::from_bytes(b"editable".to_vec());
    other.replace(0..1, b"E").unwrap();
    assert_eq!(other.bytes(), b"Editable");
}

#[test]
fn open_clipboard_has_no_source_and_saves_via_save_as() {
    let temp = Temp::new();
    let mut clip = HexContent::from_bytes(b"clipboard bytes".to_vec());
    assert!(clip.source().is_none());
    assert!(clip.save().is_err()); // 无来源：须另存
    let target = temp.0.join("pasted.bin");
    clip.save_as(&target).unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"clipboard bytes");
}

/// 摘要（后台随对齐算出）与逐行扫描等价：统计同 `HexMap::stats`，过滤区间同 `filtered_regions`。
#[test]
fn summary_matches_row_scans() {
    let mut seed = 0x9E37_79B9u64;
    let mut rand = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let left: Vec<u8> = (0..20_000).map(|_| rand() as u8).collect();
    let mut right = left.clone();
    for k in 0..40 {
        let at = (rand() as usize) % (right.len() - 64);
        match k % 3 {
            0 => right[at] ^= 0xFF,
            1 => drop(right.splice(at..at, (0..(k % 7 + 1)).map(|b| b as u8))),
            _ => drop(right.drain(at..at + (k % 5 + 1))),
        }
    }
    let mut map = HexMap {
        left: HexContent::from_bytes(left),
        right: HexContent::from_bytes(right),
        ..Default::default()
    };
    map.align();
    let summary = map.summary();
    let rows = map.align_rows();
    let mut expect = HexStats::default();
    for row in rows.rows() {
        match row.kind {
            DiffKind::Same => expect.same += 1,
            DiffKind::Changed => expect.difference += 1,
            DiffKind::LeftOnly => expect.left_orphan += 1,
            DiffKind::RightOnly => expect.right_orphan += 1,
        }
    }
    assert_eq!(summary.stats, expect);
    // 差异导航与逐行对齐图一致。
    for row in (0..rows.len()).step_by(7) {
        assert_eq!(summary.next_difference(row), rows.next_difference(row), "next {row}");
        assert_eq!(summary.prev_difference(row), rows.prev_difference(row), "prev {row}");
    }
    assert!(!summary.runs.is_empty());
    for filter in [DisplayFilter::All, DisplayFilter::Differences, DisplayFilter::Same] {
        for context in [0, 3, 16] {
            assert_eq!(
                summary.regions(filter, context),
                super::filtered_regions(&rows, filter, context),
                "{filter:?} context {context}"
            );
        }
    }
}
