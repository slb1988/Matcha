use super::HexCompareView;
use crate::hex::render;
use bc_core::DiffKind;
use eframe::egui;
use std::time::{Duration, Instant};

fn corpus(name: &str) -> Vec<u8> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/reference-parity/hex-compare/corpus/").to_owned() + name;
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

fn ready(v: &mut HexCompareView) {
    let start = Instant::now();
    while !v.is_ready() {
        v.poll();
        assert!(start.elapsed() < Duration::from_secs(5), "{}", v.message);
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// 跑一帧 egui（RawInput 注入），随后断言视图网格模型。
fn frame(ctx: &egui::Context, v: &mut HexCompareView) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0))),
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| { v.ui(ui); });
    out.textures_delta.clear();
}

/// 场景一：整行孤儿（16 字节插入）。
#[test]
fn full_row_orphan_rendering() {
    let ctx = egui::Context::default();
    let mut v = HexCompareView::from_bytes(corpus("left.bin"), corpus("right.bin"));
    ready(&mut v);
    // 固定 16 格行宽（关闭 Fit to window）。
    v.fit_to_window = false;
    v.map.rules.bytes_per_line = 16;
    frame(&ctx, &mut v);
    // 通过公开网格模型断言：行 64 为整行孤儿。
    let rows = v.grid_rows_for_test(16);
    let row = &rows[64];
    assert_eq!(row.left_label, None);
    assert_eq!(row.right_label, Some("00000400".to_owned()));
    assert_eq!(row.cells.len(), 16);
    for (i, cell) in row.cells.iter().enumerate() {
        assert_eq!(cell.kind, DiffKind::RightOnly);
        assert_eq!(cell.left, None); // 左侧斜纹占位格
        assert_eq!(cell.right, Some(i as u8)); // 插入内容 00..0F
    }
    // 下一行左 0x400 对右 0x410，内容相同不着色。
    let next = &rows[65];
    assert_eq!(next.left_label, Some("00000400".to_owned()));
    assert_eq!(next.right_label, Some("00000410".to_owned()));
    assert_eq!(next.cells[0].kind, DiffKind::Same);
    assert_eq!(next.cells[0].left, next.cells[0].right);
}

/// 场景二：行中孤儿（5 字节插入）与相位偏移。
#[test]
fn mid_row_orphan_rendering() {
    let ctx = egui::Context::default();
    let mut v = HexCompareView::from_bytes(corpus("l2.bin"), corpus("r2.bin"));
    ready(&mut v);
    v.fit_to_window = false;
    v.map.rules.bytes_per_line = 16;
    frame(&ctx, &mut v);
    let rows = v.grid_rows_for_test(16);
    let row = &rows[18];
    assert_eq!(row.left_label, Some("00000120".to_owned()));
    assert_eq!(row.right_label, Some("00000120".to_owned()));
    // 格 3..8：左侧斜纹占位，右侧红色孤儿（插入 AA..EE）。
    for (k, expect) in [0xAAu8, 0xBB, 0xCC, 0xDD, 0xEE].iter().enumerate() {
        let cell = row.cells[3 + k];
        assert_eq!(cell.kind, DiffKind::RightOnly);
        assert_eq!(cell.left, None);
        assert_eq!(cell.right, Some(*expect));
    }
    assert_eq!(rows[19].left_label, Some("0000012B".to_owned()));
    assert_eq!(rows[19].right_label, Some("00000130".to_owned()));
}

/// 场景三：行标签 = 该侧本行首个真实字节偏移（差异字节红底）。
#[test]
fn row_labels_track_first_real_byte_offsets() {
    let ctx = egui::Context::default();
    let mut v = HexCompareView::from_bytes(corpus("left.bin"), corpus("right.bin"));
    ready(&mut v);
    frame(&ctx, &mut v);
    let rows = v.grid_rows_for_test(16);
    // 第 16 行起始于 0x100：首格为差异字节（左 0x100 值 ^= 0xFF）。
    let row = &rows[16];
    assert_eq!(row.left_label, Some("00000100".to_owned()));
    assert_eq!(row.right_label, Some("00000100".to_owned()));
    assert_eq!(row.cells[0].kind, DiffKind::Changed);
    assert_ne!(row.cells[0].left, row.cells[0].right);
    // 删除段（左 0x5F0..0x610）行：左孤儿红、右占位。
    let del_row = rows.iter().find(|r| r.cells.iter().any(|c| c.kind == DiffKind::LeftOnly)).unwrap();
    assert!(del_row.cells.iter().filter(|c| c.kind == DiffKind::LeftOnly).count() > 0);
}

/// ASCII 列：不可打印字节显示为 `.`。
#[test]
fn ascii_preview_marks_unprintable_as_dot() {
    assert_eq!(render::ascii_char(b'A'), 'A');
    assert_eq!(render::ascii_char(0x00), '.');
    assert_eq!(render::ascii_char(0xFF), '.');
}

// ---- 任务 4.2：缩略图 / File Info / 工具栏 / 状态栏 ----

#[test]
fn toolbar_filter_group_and_actions() {
    let ctx = egui::Context::default();
    let mut v = HexCompareView::from_bytes(corpus("l2.bin"), corpus("r2.bin"));
    ready(&mut v);
    let items = v.toolbar_items();
    let selected: Vec<&str> = items.iter().filter(|i| i.selected).map(|i| i.action_id.as_str()).collect();
    assert_eq!(selected, ["filter.all"]);
    frame(&ctx, &mut v); // 工具栏渲染
    v.toolbar_action("filter.diffs");
    let items = v.toolbar_items();
    let selected: Vec<&str> = items.iter().filter(|i| i.selected).map(|i| i.action_id.as_str()).collect();
    assert_eq!(selected, ["filter.diffs"]);
}

#[test]
fn status_bar_fields_cover_conclusion_mode_and_cursors() {
    let mut v = HexCompareView::from_bytes(corpus("left.bin"), corpus("right.bin"));
    ready(&mut v);
    let fields = v.status_fields();
    assert!(fields[0].text.contains("Binary differences"));
    assert_eq!(fields[1].text, "Insert");
    assert!(fields.iter().any(|f| f.text == "00000000"));
    v.insert_mode = false;
    assert_eq!(v.status_fields()[1].text, "Overwrite");
}

#[test]
fn file_info_and_thumbnail_cover_the_corpus() {
    let mut v = HexCompareView::from_bytes(corpus("left.bin"), corpus("right.bin"));
    ready(&mut v);
    assert!(v.file_info_label(0).contains("4160 bytes"));
    assert!(v.file_info_label(1).contains("4144 bytes"));
    let marks = v.thumbnail_marks();
    assert!(!marks.is_empty());
    assert!(marks.iter().all(|m| (0.0..=1.0).contains(m)));
}

// ---- 任务 4.3：布局切换与显示开关 ----

#[test]
fn over_under_and_display_toggles_render() {
    let ctx = egui::Context::default();
    let mut v = HexCompareView::from_bytes(corpus("l2.bin"), corpus("r2.bin"));
    ready(&mut v);
    v.side_by_side = false; // Over-Under
    frame(&ctx, &mut v);
    v.show_thumbnail = false;
    v.show_file_info = false;
    v.show_toolbar = false;
    v.show_byte_addresses = false;
    frame(&ctx, &mut v);
    v.side_by_side = true;
    frame(&ctx, &mut v);
}

#[test]
fn layout_fixed_width_packs_eight_per_line() {
    let ctx = egui::Context::default();
    let mut v = HexCompareView::from_bytes(corpus("l2.bin"), corpus("r2.bin"));
    ready(&mut v);
    v.fit_to_window = false;
    v.map.rules.bytes_per_group = 8;
    v.groups_per_item = 1; // 每行 8 格
    frame(&ctx, &mut v);
    // 2053 个格（2048 配对 + 5 孤儿）按 8 格打包。
    let rows = v.grid_rows_for_test(8);
    assert_eq!(rows.len(), 2053usize.div_ceil(8));
    assert_eq!(rows[0].cells.len(), 8);
    // 行标签规则不变：5 字节插入行相位依旧偏移。
    let row = rows.iter().find(|r| r.cells.iter().any(|c| c.kind == DiffKind::RightOnly)).unwrap();
    assert_eq!(row.left_label, Some("00000120".to_owned()));
}

// ---- 任务 5.1：差异导航与定位 ----

fn key(ctx: &egui::Context, v: &mut HexCompareView, k: egui::Key, modifiers: egui::Modifiers) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0))),
        events: vec![egui::Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| { v.ui(ui); });
    out.textures_delta.clear();
}

#[test]
fn next_difference_cycles_and_stops_at_end() {
    let ctx = egui::Context::default();
    let mut v = HexCompareView::from_bytes(corpus("left.bin"), corpus("right.bin"));
    ready(&mut v);
    // Ctrl+N 依次落到 0x100 / 0x104 / 0x108 …
    key(&ctx, &mut v, egui::Key::N, egui::Modifiers::COMMAND);
    assert_eq!(v.cursor[0], 0x100);
    assert_eq!(v.cursor[1], 0x100); // 跨侧同步（插入点之前两侧同址）
    key(&ctx, &mut v, egui::Key::N, egui::Modifiers::COMMAND);
    assert_eq!(v.cursor[0], 0x104);
    key(&ctx, &mut v, egui::Key::P, egui::Modifiers::COMMAND);
    assert_eq!(v.cursor[0], 0x100);
    // 越过末端：提示且不回卷。
    for _ in 0..32 {
        v.next_difference(true);
    }
    let last = v.cursor[0];
    v.next_difference(true);
    assert_eq!(v.cursor[0], last);
    assert!(!v.message.is_empty());
}

#[test]
fn goto_hex_address_syncs_the_paired_side() {
    let mut v = HexCompareView::from_bytes(corpus("l2.bin"), corpus("r2.bin"));
    ready(&mut v);
    v.goto_open = true;
    v.goto_hex = true;
    v.goto_side = 1;
    v.goto_input = "128".into();
    v.goto_apply();
    assert_eq!(v.cursor[1], 0x128);
    assert_eq!(v.cursor[0], 0x123); // 左側同步到配对位置（5 字节插入前）
    assert!(v.pending_scroll.is_some());
    assert!(!v.goto_open);
}

// ---- 任务 5.2：查找与替换 ----

#[test]
fn find_hex_sequence_selects_and_reports_misses() {
    let mut v = HexCompareView::from_bytes(corpus("l2.bin"), corpus("r2.bin"));
    ready(&mut v);
    v.set_cursor(1, 0);
    v.find_open = true;
    v.find_hex = true;
    v.find_input = "AA BB CC DD EE".into();
    v.find_next(true);
    assert_eq!(v.cursor[1], 0x123);
    assert_eq!(v.selection[1], Some(0x123..0x128));
    // 再次查找：不命中 → 提示且内容不变
    v.find_next(true);
    assert!(v.message.contains("未找到"));
    assert_eq!(v.map.right.bytes().len(), 2053);
}

#[test]
fn find_text_mode_honors_case_switch() {
    let mut v = HexCompareView::from_bytes(b"Hello hELLO".to_vec(), b"x".to_vec());
    ready(&mut v);
    v.set_cursor(0, 0);
    v.find_hex = false;
    v.find_case = false;
    v.find_input = "hello".into();
    v.find_next(true);
    assert_eq!(v.cursor[0], 0); // 不区分大小写命中 0
    v.find_next(true);
    assert_eq!(v.cursor[0], 6); // 命中 hELLO
    v.find_case = true;
    v.set_cursor(0, 0);
    v.find_input = "hELLO".into();
    v.find_next(true);
    assert_eq!(v.cursor[0], 6); // 大小写敏感：只命中 hELLO
    assert_eq!(v.selection[0], Some(6..11));
}

#[test]
fn replace_overwrites_match_and_undoes() {
    let mut v = HexCompareView::from_bytes(corpus("l2.bin"), corpus("r2.bin"));
    ready(&mut v);
    v.set_cursor(1, 0);
    v.find_hex = true;
    v.find_input = "AA BB".into();
    v.find_next(true);
    assert_eq!(v.cursor[1], 0x123);
    v.replace_open = true;
    v.replace_input = "00 00".into();
    v.replace_current();
    ready(&mut v);
    assert_eq!(&v.map.right.bytes()[0x123..0x125], &[0, 0]);
    assert!(v.dirty[1]);
    v.undo(1).unwrap();
    ready(&mut v);
    assert_eq!(&v.map.right.bytes()[0x123..0x125], &[0xAA, 0xBB]);
}

// ---- 任务 5.3：字节编辑 ----

#[test]
fn insert_overwrite_delete_and_undo_redo() {
    let mut v = HexCompareView::from_bytes(b"abcd".to_vec(), b"abcd".to_vec());
    ready(&mut v);
    // Insert 模式：光标处插入，长度 +2
    v.set_cursor(0, 1);
    v.type_bytes(0, &[0xAA, 0xBB]).unwrap();
    ready(&mut v);
    assert_eq!(v.map.left.bytes(), b"a\xAA\xBBbcd");
    // Overwrite 模式：覆盖
    v.insert_mode = false;
    v.set_cursor(0, 0);
    v.type_bytes(0, &[0x41]).unwrap();
    ready(&mut v);
    assert_eq!(v.map.left.bytes(), b"A\xAA\xBBbcd");
    // Delete 选区
    v.select_all(0);
    v.delete(0).unwrap();
    ready(&mut v);
    assert!(v.map.left.is_empty());
    // Undo ×3 还原，Redo 重放一步
    v.undo(0).unwrap();
    v.undo(0).unwrap();
    ready(&mut v);
    assert_eq!(v.map.left.bytes(), b"a\xAA\xBBbcd");
    v.redo(0).unwrap();
    ready(&mut v);
    assert_eq!(v.map.left.bytes(), b"A\xAA\xBBbcd");
}

#[test]
fn copy_to_other_side_clears_difference_with_single_undo() {
    let mut v = HexCompareView::from_bytes(corpus("left.bin"), corpus("right.bin"));
    ready(&mut v);
    assert_eq!(v.stats().difference, 8);
    // 选中左侧 0x100 差异字节，Ctrl+R 复制到右侧。
    v.selection[0] = Some(0x100..0x101);
    v.set_cursor(0, 0x100);
    v.copy_to_other_side().unwrap();
    ready(&mut v);
    assert_eq!(v.stats().difference, 7);
    assert_eq!(v.map.right.bytes()[0x100], v.map.left.bytes()[0x100]);
    v.undo(1).unwrap();
    ready(&mut v);
    assert_eq!(v.stats().difference, 8);
}

#[test]
fn disabled_editing_side_rejects_writes() {
    let mut v = HexCompareView::from_bytes(b"ab".to_vec(), b"cd".to_vec());
    ready(&mut v);
    v.map.left.set_read_only(true);
    assert!(v.type_bytes(0, &[0]).is_err());
    assert!(v.delete(0).is_err());
    // 右侧不受影响（Insert 模式在光标处插入）
    v.set_cursor(1, 2);
    v.type_bytes(1, &[0x5A]).unwrap();
    assert_eq!(v.map.right.bytes(), b"cdZ");
}

// ---- 任务 5.4：Swap / Reload / Recompare / Info / 过滤 ----

#[test]
fn swap_sides_exchanges_contents() {
    let mut v = HexCompareView::from_bytes(corpus("l2.bin"), corpus("r2.bin"));
    ready(&mut v);
    v.swap();
    ready(&mut v);
    assert_eq!(v.map.left.len(), 2053);
    assert_eq!(v.map.right.len(), 2048);
    assert_eq!(v.stats().left_orphan, 5); // 原右侧孤儿换为左侧孤儿
}

#[test]
fn recompare_keeps_edits_and_refreshes_stats() {
    let mut v = HexCompareView::from_bytes(corpus("l2.bin"), corpus("r2.bin"));
    ready(&mut v);
    v.set_cursor(1, 0x200);
    v.type_bytes(1, &[0x00]).unwrap(); // Insert 模式插入 1 字节
    ready(&mut v);
    assert_eq!(v.stats().right_orphan, 6);
    // Recompare：保留编辑内容，丢弃可撤销性，重新对齐。
    let edited = v.map.right.bytes().to_vec();
    v.recompare();
    ready(&mut v);
    assert_eq!(v.map.right.bytes(), edited.as_slice());
    assert_eq!(v.stats().right_orphan, 6);
}

#[test]
fn reload_guard_blocks_when_dirty() {
    let temp = {
        let dir = std::env::temp_dir().join(format!("matcha-hex-reload-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    };
    let lp = temp.join("l.bin");
    let rp = temp.join("r.bin");
    std::fs::write(&lp, b"aa").unwrap();
    std::fs::write(&rp, b"bb").unwrap();
    let mut v = HexCompareView::open(&lp, &rp).unwrap();
    ready(&mut v);
    v.type_bytes(0, &[0xFF]).unwrap();
    ready(&mut v);
    assert!(!v.reload_guarded(false)); // 脏：拒绝并要求确认
    std::fs::write(&lp, b"zz").unwrap();
    assert!(v.reload_guarded(true)); // 确认后重载
    ready(&mut v);
    assert_eq!(v.map.left.bytes(), b"zz");
    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn info_dialog_lines_match_corpus_stats() {
    let mut v = HexCompareView::from_bytes(corpus("left.bin"), corpus("right.bin"));
    ready(&mut v);
    let lines = v.info_lines();
    assert_eq!(lines[0], "4120 same byte(s)");
    assert_eq!(lines[1], "32 left orphan byte(s)");
    assert_eq!(lines[2], "16 right orphan byte(s)");
    assert_eq!(lines[3], "8 difference byte(s)");
}

#[test]
fn differences_filter_packs_regions_with_context() {
    let mut v = HexCompareView::from_bytes(corpus("left.bin"), corpus("right.bin"));
    ready(&mut v);
    v.filter = bc_engine_bin::hex::DisplayFilter::Differences;
    // 默认 context=16：首行从 0x100-16=0x0F0 开始。
    let rows = v.grid_rows_for_test(16);
    assert_eq!(rows[0].left_label, Some("000000F0".to_owned()));
    assert!(rows.len() < 260); // 远小于全量 260+ 行，中间大段相同被隐藏
    // 每个差异字节都在模型中。
    let diff_cells = rows
        .iter()
        .flat_map(|r| &r.cells)
        .filter(|c| c.kind == DiffKind::Changed)
        .count();
    assert_eq!(diff_cells, 8);
    // 区域连续流入同一打包序列（参考 22-diffs-only.png）：第一簇（0xF0 起 41 格）
    // 余 9 格与第二簇前 7 格同行 → 第 3 行标签 0x110、第 4 行 0x1F7。
    assert_eq!(rows[2].left_label, Some("00000110".to_owned()));
    assert_eq!(rows[3].left_label, Some("000001F7".to_owned()));
}

#[test]
fn same_filter_shows_only_same_cells() {
    let mut v = HexCompareView::from_bytes(corpus("left.bin"), corpus("right.bin"));
    ready(&mut v);
    v.filter = bc_engine_bin::hex::DisplayFilter::Same;
    let rows = v.grid_rows_for_test(16);
    assert!(rows.iter().flat_map(|r| &r.cells).all(|c| c.kind == DiffKind::Same));
}

// ---- 任务 6.1：会话设置三页 ----

#[test]
fn session_settings_apply_alignment_and_disable_editing() {
    use bc_core::align::ByteAlignStrategy;
    let mut v = HexCompareView::from_bytes(corpus("l2.bin"), corpus("r2.bin"));
    ready(&mut v);
    assert_eq!(v.stats().same, 2048);
    let mut draft = v.session_draft();
    assert_eq!(draft.alignment, ByteAlignStrategy::Complete); // 默认 Complete
    draft.alignment = ByteAlignStrategy::None;
    draft.disable_editing[0] = true;
    v.settings_open = true;
    v.apply_session_draft(draft);
    ready(&mut v);
    assert!(!v.settings_open);
    assert!(v.map.left.read_only());
    assert!(v.type_bytes(0, &[0]).is_err());
    assert_eq!(v.map.rules.alignment, ByteAlignStrategy::None);
    // None 档逐位对齐：插入点后错位放大为 difference。
    assert!(v.stats().difference > 5);
    // "Use for this view only"：其它会话不受影响。
    let mut other = HexCompareView::from_bytes(corpus("l2.bin"), corpus("r2.bin"));
    ready(&mut other);
    assert_eq!(other.map.rules.alignment, ByteAlignStrategy::Complete);
    // 切回 Complete 恢复精确对齐。
    let mut draft = v.session_draft();
    draft.alignment = ByteAlignStrategy::Complete;
    draft.disable_editing[0] = false;
    v.apply_session_draft(draft);
    ready(&mut v);
    assert_eq!(v.stats().same, 2048);
    assert_eq!(v.stats().right_orphan, 5);
}

// ---- 任务 6.3：空会话流程 ----

#[test]
fn empty_session_opens_sides_independently() {
    let dir = std::env::temp_dir().join(format!("matcha-hex-empty-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (lp, rp) = (dir.join("l.bin"), dir.join("r.bin"));
    std::fs::write(&lp, corpus("l2.bin")).unwrap();
    std::fs::write(&rp, corpus("r2.bin")).unwrap();
    // 先建空视图，再逐侧打开。
    let mut v = HexCompareView::empty();
    ready(&mut v);
    assert_eq!(v.stats(), bc_engine_bin::hex::HexStats::default());
    v.open_path(0, &lp).unwrap();
    ready(&mut v);
    assert_eq!(v.map.left.len(), 2048);
    assert_eq!(v.map.right.len(), 0);
    v.open_path(1, &rp).unwrap();
    ready(&mut v);
    assert_eq!(v.stats().same, 2048);
    assert_eq!(v.stats().right_orphan, 5);
    // 标题与路径反映两侧。
    let paths = v.paths();
    assert_eq!(paths[0].as_deref(), Some(lp.as_path()));
    assert_eq!(paths[1].as_deref(), Some(rp.as_path()));
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- 任务 6.2：互切动作与菜单命令 ----

#[test]
fn compare_files_using_emits_hex_switch_action() {
    let dir = std::env::temp_dir().join(format!("matcha-hex-switch-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (lp, rp) = (dir.join("l.bin"), dir.join("r.bin"));
    std::fs::write(&lp, corpus("l2.bin")).unwrap();
    std::fs::write(&rp, corpus("r2.bin")).unwrap();
    let mut v = HexCompareView::open(&lp, &rp).unwrap();
    ready(&mut v);
    v.command("Using:Text Compare");
    assert!(matches!(
        v.action,
        Some(super::HexAction::CompareAsText { .. })
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn menu_commands_drive_view_state() {
    let ctx = egui::Context::default();
    let mut v = HexCompareView::from_bytes(corpus("left.bin"), corpus("right.bin"));
    ready(&mut v);
    v.command("Show Differences");
    assert_eq!(v.filter, bc_engine_bin::hex::DisplayFilter::Differences);
    v.command("Show All");
    v.command("Thumbnail");
    assert!(!v.show_thumbnail);
    v.command("Over-Under Layout");
    assert!(!v.side_by_side);
    v.command("Side-by-Side Layout");
    assert!(v.side_by_side);
    v.command("Hex Compare Info");
    assert!(v.info_open);
    frame(&ctx, &mut v); // Info 对话框渲染
    v.info_open = false;
    // 菜单栏渲染（menubar::show）。
    frame(&ctx, &mut v);
}

#[test]
fn snapshot_preset_insertion_scrolls_to_the_insert_region() {
    let ctx = egui::Context::default();
    let mut v = HexCompareView::from_bytes(corpus("left.bin"), corpus("right.bin"));
    ready(&mut v);
    v.snapshot_preset("hex-insertion");
    assert!(v.pending_scroll.is_some());
    frame(&ctx, &mut v);
    assert!(v.pending_scroll.is_none()); // 被 ui() 消费
    frame(&ctx, &mut v);
}

#[test]
fn snapshot_preset_diffs_filter_switches_filter() {
    let mut v = HexCompareView::from_bytes(corpus("left.bin"), corpus("right.bin"));
    ready(&mut v);
    v.snapshot_preset("hex-diffs-filter");
    assert_eq!(v.filter, bc_engine_bin::hex::DisplayFilter::Differences);
}

#[test]
fn scroll_offset_shifts_the_visible_window() {
    let ctx = egui::Context::default();
    let mut v = HexCompareView::from_bytes(corpus("left.bin"), corpus("right.bin"));
    ready(&mut v);
    frame(&ctx, &mut v);
    assert_eq!(v.visible_first_row, 0);
    v.pending_scroll = Some(0x400);
    frame(&ctx, &mut v);
    assert!(v.visible_first_row > 0, "滚动后首可见行应离开 0");
    let at = v.visible_first_row;
    frame(&ctx, &mut v); // 下一帧不得回弹（快照在该状态截图）
    assert_eq!(v.visible_first_row, at, "滚动状态未保持");
}

/// 地址栏：路径编辑态跟随来源（构造/open_path 后刷新）。
#[test]
fn path_edits_track_sources() {
    let dir = std::env::temp_dir().join(format!("matcha-hex-pathbar-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.bin");
    let b = dir.join("b.bin");
    std::fs::write(&a, b"aaaa").unwrap();
    std::fs::write(&b, b"bbbb").unwrap();
    let mut v = HexCompareView::open(&a, &b).unwrap();
    ready(&mut v);
    assert!(v.path_edits[0].ends_with("a.bin"), "{}", v.path_edits[0]);
    assert!(v.path_edits[1].ends_with("b.bin"), "{}", v.path_edits[1]);
    // 单侧换文件后同步刷新。
    v.open_path(1, &a).unwrap();
    ready(&mut v);
    v.sync_path_edits();
    assert!(v.path_edits[1].ends_with("a.bin"), "{}", v.path_edits[1]);
    // 无来源（剪贴板内容）地址栏留空。
    v.open_clipboard(0, vec![1, 2, 3]);
    ready(&mut v);
    v.sync_path_edits();
    assert!(v.path_edits[0].is_empty(), "{}", v.path_edits[0]);
    std::fs::remove_dir_all(&dir).ok();
}

/// 缩略图 bins：红色像素横线的行位置与水平区间对齐参考工具实测
///（01-main.png：行 16/32/48 满宽，行 64 右半=RightOnly，行 96/97 左半=LeftOnly）。
#[test]
fn thumbnail_bins_match_reference_marks() {
    let mut v = HexCompareView::from_bytes(corpus("left.bin"), corpus("right.bin"));
    ready(&mut v);
    let rows = v.grid_rows_for_test(16);
    let bins = render::thumb_bins(&rows, 16, render::THUMB_BIN_MAX);
    // 语料行数未超分辨率上限：bin 与行一一对应。
    assert_eq!(bins.len(), rows.len());
    let red: std::collections::BTreeMap<usize, (u16, u16)> = bins
        .iter()
        .enumerate()
        .filter_map(|(i, &b)| render::thumb_bin_range(b).map(|r| (i, r)))
        .collect();
    let rows_set: Vec<usize> = red.keys().copied().collect();
    assert_eq!(rows_set, vec![16, 32, 48, 64, 96, 97]);
    assert_eq!(red[&16], (0, 15), "Changed 行满宽");
    assert_eq!(red[&32], (0, 15));
    assert_eq!(red[&48], (0, 15));
    assert_eq!(red[&64], (8, 15), "RightOnly 行右半");
    assert_eq!(red[&96], (0, 7), "LeftOnly 行左半");
    assert_eq!(red[&97], (0, 7));
}

/// 缩略图 bins 压缩：行数超分辨率上限时聚合并保持并集。
#[test]
fn thumbnail_bins_compress_when_rows_exceed_resolution() {
    let mut v = HexCompareView::from_bytes(corpus("left.bin"), corpus("right.bin"));
    ready(&mut v);
    let rows = v.grid_rows_for_test(16);
    let bins = render::thumb_bins(&rows, 16, 8);
    assert_eq!(bins.len(), 8);
    // bin 0 聚合行 0–32（含满宽行 16/32），bin 1 聚合行 33–65（满宽行 48 ∪ 右半行 64 = 满宽），
    // bin 2 聚合行 66–98（左半行 96/97），其余 bin 无差异。
    assert_eq!(render::thumb_bin_range(bins[0]), Some((0, 15)));
    assert_eq!(render::thumb_bin_range(bins[1]), Some((0, 15)));
    assert_eq!(render::thumb_bin_range(bins[2]), Some((0, 7)));
    assert!(bins[3..].iter().all(|&b| b == 0));
}

/// 缩略图几何：内框每行 1 px（超出列高压压缩），视口框反映可见占比。
#[test]
fn thumbnail_geometry_maps_rows_and_viewport() {
    let column = egui::Rect::from_min_size(egui::pos2(0.0, 100.0), egui::vec2(render::THUMB_COL_W, 600.0));
    // 行数少于列高：1 px/行。
    let g = render::thumbnail_geometry(column, 261, 0.0, 261.0 * 15.0, 600.0, None);
    assert_eq!(g.inner.height(), 261.0);
    assert_eq!(g.inner.left(), render::THUMB_INNER_X);
    assert_eq!(g.pointer_y, None);
    // 行数超出列高：压缩到列高 - 2。
    let g = render::thumbnail_geometry(column, 5000, 0.0, 5000.0 * 15.0, 600.0, Some(0.5));
    assert_eq!(g.inner.height(), 598.0);
    // 当前位置指针：内容比例映射到内框纵坐标。
    assert_eq!(g.pointer_y, Some(g.inner.top() + 0.5 * g.inner.height()));
    // 视口框：顶端起点 + 占比高度；滚到底时贴内框底。
    let content_h = 500.0 * 15.0;
    let g = render::thumbnail_geometry(column, 500, 0.0, content_h, 600.0, None);
    assert_eq!(g.viewport.top(), g.inner.top());
    assert!((g.viewport.height() - 600.0 / content_h * g.inner.height()).abs() < 1.0);
    let max_scroll = content_h - 600.0;
    let g = render::thumbnail_geometry(column, 500, max_scroll, content_h, 600.0, None);
    assert!((g.viewport.bottom() - g.inner.bottom()).abs() < 1.0, "滚到底视口框贴底");
    // 可见占比极小时保持最小框高（大文件下不退化成横线），位置仍随滚动移动。
    let content_h = 5000.0 * 15.0;
    let g = render::thumbnail_geometry(column, 5000, 0.0, content_h, 600.0, None);
    assert_eq!(g.viewport.height(), render::THUMB_VIEW_MIN_H);
    let g = render::thumbnail_geometry(column, 5000, content_h - 600.0, content_h, 600.0, None);
    assert!((g.viewport.bottom() - g.inner.bottom()).abs() < 1.0, "最小框高时滚到底仍贴底");
}

/// 缩略图与滚动条的点击/拖动 → 滚动偏移映射。
#[test]
fn thumbnail_and_scrollbar_scroll_mapping() {
    let (content_h, view_h) = (4000.0, 600.0);
    // 缩略图：点顶部 = 0；点中部 = 视口中心对齐；点底部 = 到底。
    assert_eq!(render::thumbnail_scroll_at(0.0, content_h, view_h), 0.0);
    assert_eq!(render::thumbnail_scroll_at(0.5, content_h, view_h), 1700.0);
    assert_eq!(render::thumbnail_scroll_at(1.0, content_h, view_h), content_h - view_h);
    // 内容放得下时恒 0。
    assert_eq!(render::thumbnail_scroll_at(0.7, 500.0, 600.0), 0.0);
    // 滚动条：滑块比例高、定位往返一致。
    let track = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(16.0, 600.0));
    assert!(render::scrollbar_thumb(track, 500.0, 600.0, 0.0).is_none(), "放得下不画滑块");
    let thumb = render::scrollbar_thumb(track, content_h, view_h, 0.0).unwrap();
    assert_eq!(thumb.top(), track.top());
    assert!((thumb.height() - 90.0).abs() < 1.0, "滑块高 = 可见占比 × 轨道高");
    let max_scroll = content_h - view_h;
    let thumb = render::scrollbar_thumb(track, content_h, view_h, max_scroll).unwrap();
    assert!((thumb.bottom() - track.bottom()).abs() < 1.0);
    // 拖动映射：指针在滑块中心 ⇄ 原偏移（往返）。
    let thumb_h = thumb.height();
    let back = render::scrollbar_scroll_at(thumb.center().y, track, thumb_h, content_h, view_h);
    assert!((back - max_scroll).abs() < 1.0);
    assert_eq!(render::scrollbar_scroll_at(track.top(), track, thumb_h, content_h, view_h), 0.0);
}

/// 状态栏加载时间格式对齐参考（`Load time: 0.05 seconds`）。
#[test]
fn status_bar_load_time_in_seconds() {
    let dir = std::env::temp_dir().join(format!("matcha-hex-loadtime-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (lp, rp) = (dir.join("l.bin"), dir.join("r.bin"));
    std::fs::write(&lp, corpus("left.bin")).unwrap();
    std::fs::write(&rp, corpus("right.bin")).unwrap();
    let mut v = HexCompareView::open(&lp, &rp).unwrap();
    ready(&mut v);
    let fields = v.status_fields();
    let load = fields.iter().find(|f| f.text.starts_with("Load time: ")).expect("加载时间字段");
    assert!(load.text.ends_with(" seconds"), "{}", load.text);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 注入指针事件的帧（RawInput 回归：缩略图/滚动条拖动）。
fn pointer_frame(ctx: &egui::Context, v: &mut HexCompareView, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0))),
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| { v.ui(ui); });
    out.textures_delta.clear();
}

fn primary(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::default() }
}

/// 缩略图列：按下并拖动迷你图 → 视口中心跟随指针（“图形框拖动页面”）。
#[test]
fn thumbnail_drag_scrolls_the_page() {
    let ctx = egui::Context::default();
    let mut v = HexCompareView::from_bytes(corpus("left.bin"), corpus("right.bin"));
    ready(&mut v);
    frame(&ctx, &mut v);
    let layout = v.last_layout;
    let geom = layout.thumb.expect("缩略图几何");
    assert!(geom.inner.height() > 100.0, "语料在测试窗口中应可滚动");
    assert!(geom.hit.contains(geom.inner.center()));
    let start = geom.inner.center();
    pointer_frame(&ctx, &mut v, vec![egui::Event::PointerMoved(start)]);
    pointer_frame(&ctx, &mut v, vec![primary(start, true)]);
    assert_eq!(v.scroll, 0.0, "按下未拖动不滚动");
    let target = egui::pos2(start.x, geom.inner.top() + geom.inner.height() * 0.75);
    pointer_frame(&ctx, &mut v, vec![egui::Event::PointerMoved(target)]);
    let expect = render::thumbnail_scroll_at(0.75, layout.pane_h, layout.view_h);
    assert!((v.scroll - expect).abs() < 2.0, "拖动后 scroll={} expect={expect}", v.scroll);
    assert!(v.visible_first_row > 0, "首可见行随滚动离开 0");
    pointer_frame(&ctx, &mut v, vec![primary(target, false)]);
    // 点击内框顶部 → 回到起点。
    let top = egui::pos2(start.x, geom.inner.top() + 1.0);
    pointer_frame(&ctx, &mut v, vec![primary(top, true)]);
    pointer_frame(&ctx, &mut v, vec![primary(top, false)]);
    assert!(v.scroll.abs() < 2.0, "点击顶部回滚到 0，scroll={}", v.scroll);
}

/// 每窗格右缘滚动条：点击轨道定位、拖动滑块跟随。
#[test]
fn scrollbar_click_and_drag_scroll() {
    let ctx = egui::Context::default();
    let mut v = HexCompareView::from_bytes(corpus("left.bin"), corpus("right.bin"));
    ready(&mut v);
    frame(&ctx, &mut v);
    let layout = v.last_layout;
    for (side, bar) in layout.bars.iter().enumerate() {
        assert_eq!(bar.width(), render::SCROLLBAR_W, "滚动条宽 16（side={side}）");
    }
    let bar = layout.bars[1];
    assert!(render::scrollbar_thumb(bar, layout.pane_h, layout.view_h, 0.0).is_some(), "可滚动时有滑块");
    // 点击轨道 60% 处 → 滑块中心对齐指针。
    let click = egui::pos2(bar.center().x, bar.top() + bar.height() * 0.6);
    pointer_frame(&ctx, &mut v, vec![egui::Event::PointerMoved(click)]);
    pointer_frame(&ctx, &mut v, vec![primary(click, true)]);
    pointer_frame(&ctx, &mut v, vec![primary(click, false)]);
    let thumb0 = render::scrollbar_thumb(bar, layout.pane_h, layout.view_h, 0.0).unwrap();
    let expect = render::scrollbar_scroll_at(click.y, bar, thumb0.height(), layout.pane_h, layout.view_h);
    assert!((v.scroll - expect).abs() < 2.0, "点击定位 scroll={} expect={expect}", v.scroll);
    // 拖动滑块再下移 20% 轨道高。
    let thumb = render::scrollbar_thumb(bar, layout.pane_h, layout.view_h, v.scroll).unwrap();
    let grab = thumb.center();
    pointer_frame(&ctx, &mut v, vec![primary(grab, true)]);
    let drop_y = grab.y + bar.height() * 0.2;
    pointer_frame(&ctx, &mut v, vec![egui::Event::PointerMoved(egui::pos2(grab.x, drop_y))]);
    let expect2 = render::scrollbar_scroll_at(drop_y, bar, thumb.height(), layout.pane_h, layout.view_h);
    assert!((v.scroll - expect2).abs() < 2.0, "拖动跟随 scroll={} expect={expect2}", v.scroll);
    pointer_frame(&ctx, &mut v, vec![primary(egui::pos2(grab.x, drop_y), false)]);
}

/// RawInput 注入用的拖放文件桩。
#[derive(Debug)]
struct DropPath(std::path::PathBuf);
impl egui::DroppedFile for DropPath {
    fn path(&self) -> &std::path::Path {
        &self.0
    }
    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|e| e.to_string())
    }
}

/// 系统拖放：按落点替换该侧（不是活动侧）；该侧有未保存编辑时拒绝。
#[test]
fn os_drop_replaces_side_under_pointer_and_keeps_dirty_edits() {
    let dir = std::env::temp_dir().join(format!("matcha-hex-drop-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("dropped.bin");
    std::fs::write(&file, [9u8, 8, 7]).unwrap();
    let drop_at = |ctx: &egui::Context, v: &mut HexCompareView, pos: egui::Pos2| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0))),
            events: vec![egui::Event::PointerMoved(pos)],
            dropped_files: vec![std::sync::Arc::new(DropPath(file.clone()))],
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| { v.ui(ui); });
        out.textures_delta.clear();
    };
    let ctx = egui::Context::default();
    let mut v = HexCompareView::from_bytes(vec![1, 2, 3], vec![4, 5, 6]);
    ready(&mut v);
    frame(&ctx, &mut v);
    v.active_side = 0;
    v.dirty[1] = true;
    drop_at(&ctx, &mut v, egui::pos2(1100.0, 500.0));
    assert!(v.content(1).source().is_none(), "dirty right side must not be replaced");
    v.dirty[1] = false;
    drop_at(&ctx, &mut v, egui::pos2(1100.0, 500.0));
    ready(&mut v);
    assert_eq!(v.content(1).source(), Some(file.as_path()));
    assert!(v.content(0).source().is_none());
    assert_eq!(v.active_side, 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 系统拖放两个文件：成对载入左右（不看落点）；任一侧脏则整体拒绝。
#[test]
fn os_drop_two_files_sets_left_and_right() {
    let dir = std::env::temp_dir().join(format!("matcha-hex-drop-pair-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let left = dir.join("pair-left.bin");
    let right = dir.join("pair-right.bin");
    let right2 = dir.join("pair-right2.bin");
    std::fs::write(&left, [1u8, 2, 3]).unwrap();
    std::fs::write(&right, [4u8, 5, 6]).unwrap();
    std::fs::write(&right2, [7u8, 8]).unwrap();
    let drop_at = |ctx: &egui::Context, v: &mut HexCompareView, pos: egui::Pos2, paths: [&std::path::Path; 2]| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0))),
            events: vec![egui::Event::PointerMoved(pos)],
            dropped_files: vec![
                std::sync::Arc::new(DropPath(paths[0].to_path_buf())),
                std::sync::Arc::new(DropPath(paths[1].to_path_buf())),
            ],
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| { v.ui(ui); });
        out.textures_delta.clear();
    };
    let ctx = egui::Context::default();
    let mut v = HexCompareView::from_bytes(vec![0], vec![9]);
    ready(&mut v);
    frame(&ctx, &mut v);
    // 即使落点在右窗格，两个文件也成对进左右。
    drop_at(&ctx, &mut v, egui::pos2(1100.0, 500.0), [&left, &right]);
    ready(&mut v);
    assert_eq!(v.content(0).source(), Some(left.as_path()));
    assert_eq!(v.content(1).source(), Some(right.as_path()));
    // 左侧有未保存编辑：成对拖入整体拒绝，两侧保留原内容。
    v.dirty[0] = true;
    drop_at(&ctx, &mut v, egui::pos2(300.0, 500.0), [&left, &right2]);
    ready(&mut v);
    assert_eq!(v.content(0).source(), Some(left.as_path()));
    assert_eq!(v.content(1).source(), Some(right.as_path()));
    assert!(v.message.contains("左"), "dirty side refusal message, got {}", v.message);
    // 清除脏标记后再拖入：两侧一起更新。
    v.dirty[0] = false;
    drop_at(&ctx, &mut v, egui::pos2(300.0, 500.0), [&left, &right2]);
    ready(&mut v);
    assert_eq!(v.content(0).source(), Some(left.as_path()));
    assert_eq!(v.content(1).source(), Some(right2.as_path()));
    let _ = std::fs::remove_dir_all(&dir);
}

fn frame_with(ctx: &egui::Context, v: &mut HexCompareView, events: Vec<egui::Event>) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0))),
        events,
        ..Default::default()
    };
    crate::chrome::test_frame(ctx, input, |ui| {
        v.ui(ui);
    })
}

fn pointer(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }
}

/// 找到左侧地址栏编辑框：自上而下单击，首个让 egui 编辑框取得键盘焦点的 y。
fn focus_left_path(ctx: &egui::Context, v: &mut HexCompareView) -> f32 {
    for y in (0..200).step_by(4) {
        let pos = egui::pos2(120.0, y as f32);
        frame_with(ctx, v, vec![egui::Event::PointerMoved(pos)]);
        frame_with(ctx, v, vec![pointer(pos, true)]);
        frame_with(ctx, v, vec![pointer(pos, false)]);
        frame_with(ctx, v, vec![]);
        if ctx.egui_wants_keyboard_input() {
            return y as f32;
        }
        // 点到别处（菜单等）后复位。
        frame_with(ctx, v, vec![egui::Event::Key {
            key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default(),
        }]);
    }
    panic!("path bar edit box not found");
}

fn path_selection(ctx: &egui::Context) -> Option<std::ops::Range<usize>> {
    let id = ctx.memory(|m| m.focused())?;
    let state = egui::widgets::text_edit::TextEditState::load(ctx, id)?;
    let r = state.cursor.char_range()?.as_sorted_char_range();
    Some(r.start.0..r.end.0)
}

/// 地址栏可用鼠标拖选；Ctrl+A/Ctrl+C 作用于路径文本而非十六进制字节。
#[test]
fn path_bar_supports_mouse_selection_and_copy() {
    let dir = std::env::temp_dir().join(format!("matcha-hex-pathsel-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.bin");
    let b = dir.join("b.bin");
    std::fs::write(&a, b"aaaa").unwrap();
    std::fs::write(&b, b"bbbb").unwrap();
    let ctx = egui::Context::default();
    let mut v = HexCompareView::open(&a, &b).unwrap();
    ready(&mut v);
    frame_with(&ctx, &mut v, vec![]);
    frame_with(&ctx, &mut v, vec![]);
    let y = focus_left_path(&ctx, &mut v);
    // 拖选：从路径框左端拖到中部。
    let (p0, p1) = (egui::pos2(14.0, y), egui::pos2(200.0, y));
    frame_with(&ctx, &mut v, vec![egui::Event::PointerMoved(p0), pointer(p0, true)]);
    frame_with(&ctx, &mut v, vec![egui::Event::PointerMoved(egui::pos2(100.0, y))]);
    frame_with(&ctx, &mut v, vec![egui::Event::PointerMoved(p1)]);
    frame_with(&ctx, &mut v, vec![pointer(p1, false)]);
    let sel = path_selection(&ctx).expect("path edit focused");
    assert!(sel.end > sel.start + 5, "drag should select text, got {sel:?}");
    // Ctrl+A 全选路径；Ctrl+C 复制路径文本，不得改选十六进制字节或输出 hex dump。
    let ctrl = egui::Modifiers::COMMAND;
    let key = |k| egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: ctrl };
    frame_with(&ctx, &mut v, vec![key(egui::Key::A)]);
    let out = frame_with(&ctx, &mut v, vec![key(egui::Key::C), egui::Event::Copy]);
    let copied: Vec<String> = out
        .platform_output
        .commands
        .iter()
        .filter_map(|c| match c {
            egui::OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(copied, vec![v.path_edits[0].clone()]);
    assert_eq!(v.selection[0], None, "hex selection must stay untouched");
    // 粘贴进路径框不得写入字节。
    frame_with(&ctx, &mut v, vec![egui::Event::Paste("x".into())]);
    ready(&mut v);
    assert_eq!(v.map.left.bytes(), b"aaaa");
    std::fs::remove_dir_all(&dir).ok();
}
