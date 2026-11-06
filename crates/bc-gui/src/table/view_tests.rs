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
fn choose(ctx: &egui::Context, v: &mut TableCompareView, label: &str) {
    frame(ctx, v, vec![]);
    let out = frame(ctx, v, vec![]);
    click(ctx, v, button(&out, label));
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
    assert!(!has(&out, "Copy cell → other"));
    assert!(has(&out, "Session") && has(&out, "Edit") && has(&out, "Search") && has(&out, "View"));
    option(&ctx, &mut v, "Toolbar");
    let out = frame(&ctx, &mut v, vec![]);
    assert!(v.show_toolbar && has(&out, "Copy cell → other"));
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
    option(&ctx, &mut v, "Text Details");
    let out = frame(&ctx, &mut v, vec![]);
    assert!(has(
        &out,
        "Text Details — full cell text / cached formula / number format"
    ));
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
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    let out = frame(&ctx, &mut v, vec![]);
    let dot = super::super::grid::color(
        &Prefs::default().colors,
        bc_engine_table::compare::Importance::Important,
        false,
        255,
    );
    assert_eq!(
        out.shapes
            .iter()
            .filter(
                |s| matches!(&s.shape, egui::Shape::Circle(c) if c.radius == 3.0 && c.fill == dot)
            )
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
            egui::Shape::Rect(r) if r.rect.width() == 20.0 && r.rect.height() > 400.0 => {
                Some(r.rect)
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
