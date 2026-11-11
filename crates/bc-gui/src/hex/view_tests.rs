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
