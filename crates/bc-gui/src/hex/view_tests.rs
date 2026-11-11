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
    let mut out = ctx.run_ui(input, |ui| v.ui(ui));
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
    let mut out = ctx.run_ui(input, |ui| v.ui(ui));
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
    // 默认 context=16：首个差异区域从 0x100-16=0x0F0 开始（区域起点重打包）。
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
}

#[test]
fn same_filter_shows_only_same_cells() {
    let mut v = HexCompareView::from_bytes(corpus("left.bin"), corpus("right.bin"));
    ready(&mut v);
    v.filter = bc_engine_bin::hex::DisplayFilter::Same;
    let rows = v.grid_rows_for_test(16);
    assert!(rows.iter().flat_map(|r| &r.cells).all(|c| c.kind == DiffKind::Same));
}
