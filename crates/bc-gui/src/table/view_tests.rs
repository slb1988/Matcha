use super::super::{formats, grid::GUTTER};
use super::*;
use bc_formats::{
    model::{HeaderMode, TableType},
    registry::FormatRegistry,
    FormatKind,
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "matcha-table-view-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
/// Wave 8 moved the old toolbar rows into the classic menus/toolbar; the
/// legacy labels used by these tests resolve to their new entry paths.
fn legacy_path(label: &str) -> Vec<&str> {
    match label {
        "Differences" => vec!["Diffs"],
        "None" => vec!["View", "Show None"],
        "Ignore Unimportant" => vec!["Minor"],
        "Go" => vec!["Search", "Go To...", "Go"],
        "Hide Same Columns" => vec!["View", "Hide Same Columns"],
        "Over-Under" => vec!["View", "Over-Under Layout"],
        "+ Formulas" => vec!["View", "Compare Values + Formulas"],
        "+ Number formats" => vec!["View", "Compare Values + Formulas + Number Formats"],
        "Values" => vec!["View", "Compare Values"],
        other => vec![other],
    }
}
fn choose(ctx: &egui::Context, v: &mut TableCompareView, label: &str) {
    for label in legacy_path(label) {
        frame(ctx, v, vec![]);
        let out = frame(ctx, v, vec![]);
        click(ctx, v, button(&out, label));
    }
}
fn option(ctx: &egui::Context, v: &mut TableCompareView, label: &str) {
    choose(ctx, v, "View");
    choose(ctx, v, label);
    frame(ctx, v, vec![key(egui::Key::Escape, false)]);
}
fn has(out: &egui::FullOutput, label: &str) -> bool {
    out.shapes
        .iter()
        .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == label))
}
fn registry() -> FormatRegistry {
    let mut r = FormatRegistry::with_builtins();
    let mut f = r.get(FormatKind::Table, "CSV").unwrap().clone();
    f.name = "Fixed blocks".into();
    f.source_xml = None;
    f.masks = vec!["*.fix;report-?.dat".into()];
    let options = f.table.as_mut().unwrap();
    options.table_type = TableType::Fixed;
    options.column_widths = vec![2, 4];
    options.header = HeaderMode::Data;
    r.upsert(f).unwrap();
    r
}

#[test]
fn table_masks_use_registry_order_and_never_enable_external_converters() {
    let mut r = registry();
    for path in [
        "left.fix",
        "RIGHT.FIX",
        "report-1.dat",
        "data.tsv",
        "data.csv",
        "book.XLSX",
    ] {
        assert!(formats::table_path(Path::new(path), &r), "{path}");
    }
    assert!(!formats::table_path(Path::new("report-12.dat"), &r));
    assert!(!formats::table_path(Path::new("source.cpp"), &r));
    let mut f = r
        .formats()
        .iter()
        .find(|f| f.kind == FormatKind::Text)
        .unwrap()
        .clone();
    f.name = "Prefer text".into();
    f.masks = vec!["*.fix".into()];
    f.source_xml = None;
    r.upsert(f).unwrap();
    r.move_to(FormatKind::Text, "Prefer text", 0).unwrap();
    assert!(!formats::table_path(Path::new("left.fix"), &r));
    assert!(formats::options(Path::new("legacy.xls"), &r).is_err());
    let mut f = r.get(FormatKind::Table, "Fixed blocks").unwrap().clone();
    f.convert = Some(bc_formats::model::ConvertKind::Pdf);
    r.upsert(f).unwrap();
    assert!(formats::options(Path::new("left.fix"), &r).is_err());
}

#[test]
fn custom_mask_options_survive_registry_persistence_and_load_actual_fixed_width_sources() {
    let tmp = Temp::new();
    let r = registry();
    r.save_user_dir(&tmp.0.join("formats")).unwrap();
    let r = FormatRegistry::load_user_dir(&tmp.0.join("formats")).unwrap();
    let left = tmp.0.join("left.fix");
    let right = tmp.0.join("right.fix");
    std::fs::write(&left, "01Tea \n02Maca\n").unwrap();
    std::fs::write(&right, "01Tea \n02Moch\n").unwrap();
    let mut v = TableCompareView::open_with_registry(&left, &right, &r).unwrap();
    ready(&mut v);
    assert_eq!(v.row_count(), 2);
    let p = v.pair.as_ref().unwrap();
    assert_eq!(p.map.column_count(), 2);
    assert_eq!(p.map.left.borrow().display_at(0, 0).unwrap(), "01");
    assert_eq!(p.map.left.borrow().display_at(0, 1).unwrap(), "Tea ");
    assert_eq!(p.cache.statistics.important, 1);
    assert_eq!(v.options[0].format.table_type, TableType::Fixed);
    assert_eq!(v.options[0].format.header, HeaderMode::Data);
}

#[test]
fn toolbar_can_be_hidden_and_restored_without_losing_menu_access() {
    let mut v = view();
    let ctx = egui::Context::default();
    option(&ctx, &mut v, "Toolbar");
    let out = frame(&ctx, &mut v, vec![]);
    assert!(!v.show_toolbar);
    assert!(!has(&out, "Copy"));
    assert!(has(&out, "Session") && has(&out, "Edit") && has(&out, "Search") && has(&out, "View"));
    option(&ctx, &mut v, "Toolbar");
    let out = frame(&ctx, &mut v, vec![]);
    assert!(v.show_toolbar && has(&out, "Copy"));
    assert!(!v.is_dirty());
}

#[test]
fn visible_whitespace_details_file_info_and_row_numbers_are_independent_view_options() {
    let mut s = Sheet::default();
    s.push_row(vec![Cell::text("a b\tc\nd")]);
    let mut v = TableCompareView::from_sheets(s.clone(), s, TableRules::default());
    ready(&mut v);
    v.files[0].path = Some(PathBuf::from("synthetic-left.csv"));
    v.files[0].byte_len = 42;
    let ctx = egui::Context::default();
    option(&ctx, &mut v, "Visible Whitespace");
    let out = frame(&ctx, &mut v, vec![]);
    assert!(has(&out, "a·b→c↵d"));
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .left
            .borrow()
            .display_at(0, 0)
            .unwrap(),
        "a b\tc\nd"
    );
    // Text Details (on by default) shows the current row of both sides.
    assert_eq!(
        out.shapes
            .iter()
            .filter(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "a b	c ↵ d"))
            .count(),
        2
    );
    option(&ctx, &mut v, "Text Details");
    let out = frame(&ctx, &mut v, vec![]);
    assert!(!v.show_details && !has(&out, "a b	c ↵ d"));
    option(&ctx, &mut v, "File Info");
    let out = frame(&ctx, &mut v, vec![]);
    assert!(!out.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text().contains("synthetic-left.csv"))));
    option(&ctx, &mut v, "Row Numbers");
    let out = frame(&ctx, &mut v, vec![]);
    let pane = v.grid.pane_rects[0];
    assert!(!out
        .shapes
        .iter()
        .any(|s| matches!(&s.shape, egui::Shape::Text(t)
        if t.galley.text() == "1" && pane.contains(t.pos) && t.pos.x < pane.left()+GUTTER)));
    assert!(!v.grid.row_numbers && !v.show_info && !v.is_dirty());
    option(&ctx, &mut v, "Thumbnail");
    assert!(!v.show_thumbnail);
}

#[test]
fn hide_same_reports_hidden_count_and_resize_scans_beyond_the_initial_sample() {
    let mut left = Sheet::default();
    for i in 0..200 {
        left.push_row(vec![
            Cell::number(i as f64),
            Cell::text("short"),
            Cell::text("same"),
        ]);
    }
    left.set_cell(150, 1, Cell::text("x".repeat(1000))).unwrap();
    let mut right = left.clone();
    right.set_cell(150, 1, Cell::text("different")).unwrap();
    let mut v = TableCompareView::from_sheets(
        left,
        right,
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            ..Default::default()
        },
    );
    ready(&mut v);
    assert!(v.grid.widths[1] <= 420.0);
    let ctx = egui::Context::default();
    choose(&ctx, &mut v, "Hide Same Columns");
    let out = frame(&ctx, &mut v, vec![]);
    assert_eq!(v.grid.columns, [1]);
    assert_eq!(
        out.shapes
            .iter()
            .filter(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "2 hidden"))
            .count(),
        2
    );
    option(&ctx, &mut v, "Resize Columns to Fit");
    ready(&mut v);
    assert!(v.grid.widths[1] > 7000.0);
    assert!(v
        .pair
        .as_ref()
        .unwrap()
        .history
        .iter()
        .all(|h| !h.can_undo()));
}

#[test]
fn raw_scroll_on_either_pane_is_synchronized_in_both_layouts_and_row_dot_ignores_column_viewport() {
    let mut left = Sheet::default();
    for row in 0..300 {
        left.push_row((0..40).map(|c| Cell::text(format!("R{row}C{c}"))).collect());
    }
    let mut right = left.clone();
    right.set_cell(0, 35, Cell::text("different")).unwrap();
    let mut v = TableCompareView::from_sheets(
        left,
        right,
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            ..Default::default()
        },
    );
    ready(&mut v);
    // Pane synchronization only: the details strip repeats the cursor row.
    v.show_details = false;
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    let out = frame(&ctx, &mut v, vec![]);
    // Row difference markers: 9 px white-rimmed squares in each pane's gutter.
    assert_eq!(
        out.shapes
            .iter()
            .filter(|s| matches!(&s.shape, egui::Shape::Rect(r)
                if r.rect.size() == egui::vec2(9.0, 9.0)
                    && r.fill == egui::Color32::WHITE
                    && v.grid.pane_rects.iter().any(|p| p.contains(r.rect.min)
                        && r.rect.min.x < p.left() + GUTTER)))
            .count(),
        2
    );
    for stacked in [false, true] {
        if v.grid.stacked != stacked {
            choose(&ctx, &mut v, "Over-Under");
        }
        for side in 0..2 {
            let pos = v.grid.pane_rects[side].center();
            frame(
                &ctx,
                &mut v,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        phase: egui::TouchPhase::Move,
                        source: egui::MouseWheelSource::Unknown,
                        delta: egui::vec2(-300.0, -300.0),
                        modifiers: Default::default(),
                    },
                ],
            );
            for _ in 0..12 {
                frame(&ctx, &mut v, vec![]);
            }
            let out = frame(&ctx, &mut v, vec![]);
            assert!(v.grid.scroll.x > 0.0 && v.grid.scroll.y > 0.0);
            let mut seen = HashMap::<String, usize>::new();
            for shape in &out.shapes {
                if let egui::Shape::Text(t) = &shape.shape {
                    let text = t.galley.text();
                    if text
                        .strip_prefix('R')
                        .and_then(|s| s.split_once('C'))
                        .is_some_and(|(r, c)| {
                            r.parse::<usize>().is_ok() && c.parse::<usize>().is_ok()
                        })
                    {
                        *seen.entry(text.into()).or_default() += 1;
                    }
                }
            }
            assert!(seen.len() > 10);
            assert!(
                seen.values().all(|&count| count == 2),
                "panes show different source cells: stacked={stacked} side={side} scroll={:?} panes={:?} unmatched={:?}",
                v.grid.scroll, v.grid.pane_rects,
                seen.iter().filter(|(_, count)| **count != 2).take(20).collect::<Vec<_>>()
            );
            assert!(v.cells_drawn() < 1000 && v.rows_drawn() < 100);
        }
    }
}

#[test]
fn thumbnail_click_maps_to_the_comparison_row() {
    let ctx = egui::Context::default();
    let mut result = None;
    let mut draw = |events| {
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(300.0, 500.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                result = super::super::details::thumbnail(
                    ui,
                    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(52.0, 500.0)),
                    &[bc_engine_table::compare::Importance::Same; 512],
                    1000,
                    0..20,
                    &Prefs::default().colors,
                );
            },
        );
        out.textures_delta.clear();
        out
    };
    draw(vec![]);
    let out = draw(vec![]);
    let rect = out
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            // The miniature frame (38 px wide, one line per row up to the column).
            egui::Shape::Rect(r) if r.rect.width() == 38.0 && r.rect.height() > 400.0 => {
                Some(r.rect.shrink(1.0))
            }
            _ => None,
        })
        .unwrap();
    let pos = egui::pos2(rect.center().x, rect.top() + rect.height() * 0.7);
    for pressed in [true, false] {
        draw(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            },
        ]);
    }
    assert!(result.is_some_and(|r| (699..=701).contains(&r)));
}

#[test]
fn reference_layout_geometry_title_and_menu_inventory() {
    let mut v = view();
    v.files[0].path = Some(PathBuf::from("table-left.csv"));
    v.files[1].path = Some(PathBuf::from("table-right.csv"));
    let ctx = egui::Context::default();
    crate::chrome::theme::apply(&ctx);
    for _ in 0..2 {
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1384.0, 961.0),
                )),
                ..Default::default()
            },
            |ui| {
                v.ui(ui, &mut Prefs::default());
            },
        );
        out.textures_delta.clear();
    }
    // tokens `table-basic` regions: grid rows below the 28 px header at
    // y151, pane status at y836, details 81 px, status bar 25 px.
    assert_eq!(
        v.grid.pane_rects[0],
        egui::Rect::from_min_max(egui::pos2(52.0, 151.0), egui::pos2(718.0, 836.0))
    );
    assert_eq!(
        v.grid.pane_rects[1],
        egui::Rect::from_min_max(egui::pos2(722.0, 151.0), egui::pos2(1384.0, 836.0))
    );
    assert_eq!(
        v.title(),
        "table-left.csv <--> table-right.csv - Table Compare - Matcha"
    );
    // Menu titles and leaf labels follow `inventories.table.menus`; product
    // names never appear in Help.
    let tokens: serde_json::Value = serde_json::from_str(
        std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../docs/visual-parity/tokens.json"),
        )
        .unwrap()
        .trim_start_matches('\u{feff}'),
    )
    .unwrap();
    let menus = super::super::menu::inventory();
    let reference = tokens["inventories"]["table"]["menus"].as_array().unwrap();
    assert_eq!(menus.len(), reference.len());
    for (menu, expected) in menus.iter().zip(reference) {
        assert_eq!(menu.title, expected["name"].as_str().unwrap());
        let items = expected["items"].as_array().unwrap();
        assert_eq!(menu.items.len(), items.len(), "{}", menu.title);
        for (item, expected) in menu.items.iter().zip(items) {
            let name = expected["name"].as_str().unwrap_or_default();
            if menu.title != "Help" {
                assert_eq!(item.label, name);
            }
        }
    }
    assert!(menus[6]
        .items
        .iter()
        .any(|i| i.label == "About Matcha" || i.label == "Project Website"));
}

#[test]
fn column_resize_hit_test_and_offset_updates() {
    let mut state = super::super::grid::GridState::default();
    state.widths = vec![33.0, 47.0, 33.0];
    state.rebuild_columns(3, &[], false);
    assert_eq!(state.offsets, vec![0.0, 33.0, 80.0, 113.0]);
    let pane_left = 52.0;
    let boundary0 = pane_left + GUTTER + 33.0;
    // ±3 px grab around a boundary; mid-column is not grabbable; the gutter
    // line itself never is.
    assert_eq!(state.resize_target(pane_left, boundary0 + 2.0), Some(0));
    assert_eq!(state.resize_target(pane_left, boundary0 + 6.0), None);
    assert_eq!(state.resize_target(pane_left, pane_left + GUTTER), None);
    // Drag column A 30 px wider: later columns shift by the same delta.
    state.resize_column(0, 63.0);
    assert_eq!(state.widths[0], 63.0);
    assert_eq!(state.offsets, vec![0.0, 63.0, 110.0, 143.0]);
    // Floor: a collapsed drag clamps to the minimum width.
    state.resize_column(0, 1.0);
    assert_eq!(state.widths[0], super::super::grid::MIN_COLUMN_WIDTH);
    assert_eq!(state.offsets[1], super::super::grid::MIN_COLUMN_WIDTH);
    // Horizontal scroll hides a boundary under the gutter: not grabbable.
    let mut scrolled = super::super::grid::GridState::default();
    scrolled.widths = vec![33.0, 47.0];
    scrolled.rebuild_columns(2, &[], false);
    scrolled.scroll.x = 30.0;
    assert_eq!(scrolled.resize_target(pane_left, pane_left + GUTTER + 3.0), None);
    assert_eq!(
        scrolled.resize_target(pane_left, pane_left + GUTTER + 50.0),
        Some(1)
    );
}

#[test]
fn dragging_a_header_boundary_resizes_the_shared_column() {
    let mut v = view();
    let ctx = egui::Context::default();
    crate::chrome::theme::apply(&ctx);
    frame(&ctx, &mut v, vec![]);
    frame(&ctx, &mut v, vec![]);
    let pane = v.grid.pane_rects[0];
    let header_y = pane.top() - super::super::grid::HEADER_HEIGHT / 2.0;
    let before = v.grid.offsets[1];
    let start = egui::pos2(pane.left() + GUTTER + before, header_y);
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(start)]);
    frame(
        &ctx,
        &mut v,
        vec![egui::Event::PointerButton {
            pos: start,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    );
    let mid = start + egui::vec2(15.0, 0.0);
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(mid)]);
    let end = start + egui::vec2(30.0, 0.0);
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(end)]);
    frame(
        &ctx,
        &mut v,
        vec![egui::Event::PointerButton {
            pos: end,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
    frame(&ctx, &mut v, vec![]);
    assert_eq!(
        v.grid.offsets[1],
        before + 30.0,
        "header boundary drag widens column A"
    );
    assert!(v.grid.resizing.is_none(), "resize ends with the drag");
    // A click without drag keeps the width.
    let stable = v.grid.offsets[1];
    click(&ctx, &mut v, egui::pos2(pane.left() + GUTTER + stable, header_y));
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.grid.offsets[1], stable);
}

#[test]
fn header_paints_excel_style_letters_even_with_a_header_row() {
    // 真实文件走 worker 路径：可检测的表头行保留为第 1 行数据，
    // 表头显示 Excel 风格字母（参考工具默认外观）。
    let left = temp_file("hdr-left.csv", "id,name,price\n7,apple,1.0\n8,pear,2.0\n");
    let right = temp_file("hdr-right.csv", "id,name,price\n7,apple,1.2\n8,PEAR,2.1\n");
    let mut v = open_files(&left, &right);
    let ctx = egui::Context::default();
    crate::chrome::theme::apply(&ctx);
    frame(&ctx, &mut v, vec![]);
    let out = frame(&ctx, &mut v, vec![]);
    let pane = v.grid.pane_rects[0];
    let header = egui::Rect::from_min_max(
        egui::pos2(pane.left(), pane.top() - super::super::grid::HEADER_HEIGHT),
        egui::pos2(pane.left() + 200.0, pane.top()),
    );
    let texts: Vec<&str> = out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t)
                if header.contains(egui::Rect::from_min_size(t.pos, t.galley.size()).center()) =>
            {
                Some(t.galley.text())
            }
            _ => None,
        })
        .collect();
    for letter in ["A", "B", "C"] {
        // 键列带 ◆ 前缀（如 "◆ A"）。
        assert!(
            texts.iter().any(|t| t.ends_with(letter)),
            "header shows {letter}: {texts:?}"
        );
    }
    for name in ["id", "name", "price"] {
        assert!(
            !texts.iter().any(|t| t.contains(name)),
            "header never shows the source column name {name}: {texts:?}"
        );
    }
    // 表头行保留为第 1 行数据。
    let first_row = egui::Rect::from_min_max(
        egui::pos2(pane.left(), pane.top()),
        egui::pos2(pane.left() + 200.0, pane.top() + super::super::grid::ROW_HEIGHT),
    );
    let row_texts: Vec<&str> = out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t)
                if first_row
                    .contains(egui::Rect::from_min_size(t.pos, t.galley.size()).center()) =>
            {
                Some(t.galley.text())
            }
            _ => None,
        })
        .collect();
    for name in ["id", "name", "price"] {
        assert!(
            row_texts.contains(&name),
            "header row stays as data row 1 ({name}): {row_texts:?}"
        );
    }
    let _ = std::fs::remove_file(left);
    let _ = std::fs::remove_file(right);
}
