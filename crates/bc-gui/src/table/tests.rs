use super::{
    cache::Filter,
    find::Action,
    grid::{Cursor, GridState},
    view::*,
};
use bc_core::{align::AlignStrategy, cancel::NeverCancel};
use bc_engine_table::{
    rules::{ColumnHandling, KeySelection, Layers},
    source::{read_text, ReadOptions},
    Cell, Sheet, TableRules,
};
use bc_session::prefs::Prefs;
use eframe::egui;
use std::{
    io::Cursor as IoCursor,
    path::Path,
    time::{Duration, Instant},
};
#[path = "copy_tests.rs"]
mod copying;
#[path = "edit_tests.rs"]
mod editing;
#[path = "navigation_tests.rs"]
mod navigation;
#[path = "projection_tests.rs"]
mod projections;
#[path = "sort_tests.rs"]
mod sorting;

fn sheet(text: &str) -> Sheet {
    read_text(IoCursor::new(text), &ReadOptions::default(), &NeverCancel)
        .unwrap()
        .sheet
}
fn view() -> TableCompareView {
    let mut v = TableCompareView::from_sheets(
        sheet("id,name,price\n7,apple,1.0\n8,pear,2.0\n"),
        sheet("id,name,price\n7,apple,1.2\n8,PEAR,2.1\n"),
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            strategy: AlignStrategy::Quick,
            ..Default::default()
        },
    );
    ready(&mut v);
    v
}
fn ready(v: &mut TableCompareView) {
    let start = Instant::now();
    while !v.is_ready() {
        v.poll();
        assert!(start.elapsed() < Duration::from_secs(15), "{}", v.message);
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn frame(
    ctx: &egui::Context,
    v: &mut TableCompareView,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            v.ui(ui, &mut Prefs::default());
        },
    );
    output.textures_delta.clear();
    output
}
fn key(key: egui::Key, command: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers {
            ctrl: command,
            command,
            ..Default::default()
        },
    }
}
fn click(ctx: &egui::Context, v: &mut TableCompareView, p: egui::Pos2) {
    for pressed in [true, false] {
        frame(
            ctx,
            v,
            vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
}
fn button(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
    output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == label => {
                Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing button {label}"))
}
#[test]
fn grid_tessellation_cell_colours_two_dimensional_virtualization_and_stacked_sync() {
    let mut l = Sheet::default();
    for r in 0..1000 {
        l.push_row((0..100).map(|c| Cell::text(format!("R{r}C{c}"))).collect());
    }
    let mut r = l.clone();
    r.set_cell(700, 70, Cell::text("changed")).unwrap();
    let mut v = TableCompareView::from_sheets(
        l,
        r,
        TableRules {
            strategy: AlignStrategy::None,
            ..Default::default()
        },
    );
    ready(&mut v);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    v.scroll_to_row(700);
    v.scroll_to_column(70);
    let out = frame(&ctx, &mut v, vec![]);
    assert!(v.grid.cells_drawn < 600);
    assert!(v.grid.rows_drawn < 100);
    assert!(!ctx.tessellate(out.shapes, out.pixels_per_point).is_empty());
    assert_eq!(v.grid.visible.start, 700);
    v.grid.stacked = true;
    frame(&ctx, &mut v, vec![]);
    assert!(v.grid.pane_rects[0].bottom() <= v.grid.pane_rects[1].top());
    assert_eq!(
        v.pair.as_ref().unwrap().map.row(700).unwrap().left,
        Some(700)
    );
}
#[test]
fn navigation_filter_ignore_and_cell_copy_are_background_then_undoable() {
    let mut v = view();
    v.navigate(true, 0);
    assert_eq!(v.grid.cursor, Cursor { row: 0, column: 2 });
    v.copy(false);
    assert!(v.pair.as_ref().unwrap().map.is_stale());
    let start = Instant::now();
    ready(&mut v);
    assert!(start.elapsed() < Duration::from_millis(700));
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.same, 1);
    v.undo(false);
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.important, 2);
    {
        let p = v.pair.as_ref().unwrap();
        let mut rules = p.map.rules_mut();
        for c in &mut rules.columns.0 {
            c.handling = ColumnHandling {
                unimportant: true,
                ..Default::default()
            };
        }
    }
    v.recompare();
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.unimportant, 2);
    v.pair.as_ref().unwrap().map.rules_mut().ignore_unimportant = true;
    v.project();
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.same, 2);
    v.filter = Filter::Differences;
    v.pair.as_mut().unwrap().projection = v.pair.as_ref().unwrap().cache.project(v.filter);
    assert_eq!(v.row_count(), 0);
}
#[test]
fn raw_pointer_modify_commit_undo_and_find_replace() {
    let ctx = egui::Context::default();
    let mut v = view();
    frame(&ctx, &mut v, vec![]);
    let pos = v.grid.pane_rects[1].left_top() + egui::vec2(80.0, 10.0);
    click(&ctx, &mut v, pos);
    assert_eq!(v.grid.active, 1);
    frame(&ctx, &mut v, vec![key(egui::Key::F2, false)]);
    assert!(v.editor.is_some());
    frame(
        &ctx,
        &mut v,
        vec![key(egui::Key::A, true), egui::Event::Text("7".into())],
    );
    let out = frame(&ctx, &mut v, vec![]);
    let commit = button(&out, "Commit (Ctrl+Enter)");
    click(&ctx, &mut v, commit);
    ready(&mut v);
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .right
            .borrow()
            .display_at(0, 0)
            .unwrap(),
        "7"
    );
    v.find.query = "pear".into();
    v.find.side = 1;
    v.find.case = false;
    v.find_action(Action::Next).unwrap();
    assert_eq!(v.grid.cursor, Cursor { row: 1, column: 1 });
    v.find.replacement = "梨".into();
    v.find_action(Action::All).unwrap();
    ready(&mut v);
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .right
            .borrow()
            .display_at(1, 1)
            .unwrap(),
        "梨"
    );
    v.undo(false);
    ready(&mut v);
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .right
            .borrow()
            .display_at(1, 1)
            .unwrap(),
        "PEAR"
    );
}
#[test]
fn structures_saved_fields_and_independent_history() {
    let mut v = view();
    v.grid.active = 1;
    v.grid.cursor = Cursor { row: 1, column: 1 };
    v.structure(0).unwrap();
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().map.right.borrow().row_count(), 3);
    v.undo(false);
    ready(&mut v);
    v.structure(3).unwrap();
    ready(&mut v);
    assert_eq!(
        v.pair.as_ref().unwrap().map.right.borrow().column_count(),
        4
    );
    v.undo(false);
    ready(&mut v);
    assert_eq!(
        v.pair.as_ref().unwrap().map.right.borrow().column_count(),
        3
    );
    let path =
        std::env::temp_dir().join(format!("matcha-table-gui-save-{}.csv", std::process::id()));
    v.begin_edit(1, Cursor { row: 0, column: 2 }, Some("4.00".into()));
    v.commit_edit().unwrap();
    v.save_to(
        1,
        &path,
        &Prefs {
            backups: bc_session::prefs::BackupPrefs {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        },
        false,
    )
    .unwrap();
    ready(&mut v);
    assert!(std::fs::read_to_string(&path)
        .unwrap()
        .contains("7,apple,4.00"));
    assert!(!v.pair.as_ref().unwrap().history[1].is_dirty());
    std::fs::remove_file(path).unwrap();
}
#[test]
fn layer_controls_preserve_raw_states_details_and_readonly_commands() {
    let mut l = sheet("id,value\n1,6\n");
    let mut r = l.clone();
    let mut cell = Cell::number(6.0);
    cell.formula = Some("=1+5".into());
    l.set_cell(0, 1, cell.clone()).unwrap();
    cell.formula = Some("=SUM(1,5)".into());
    r.set_cell(0, 1, cell).unwrap();
    let mut v = TableCompareView::from_sheets(
        l,
        r,
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            ..Default::default()
        },
    );
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.same, 1);
    v.pair.as_ref().unwrap().map.rules_mut().layers = Layers::ValuesAndFormulas;
    v.project();
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.important, 1);
    v.grid.cursor = Cursor { row: 0, column: 1 };
    v.show_details = true;
    let ctx = egui::Context::default();
    let out = frame(&ctx, &mut v, vec![]);
    assert!(out
        .shapes
        .iter()
        .any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text().contains("=SUM(1,5)"))));
    v.files[1].readonly = true;
    v.begin_edit(1, v.grid.cursor, None);
    assert!(v.editor.is_none());
    v.grid.active = 0;
    v.copy(false);
    assert!(v.message.contains("disabled"));
}
#[test]
fn home_masks_and_unsaved_cancel_do_not_discard() {
    for ext in ["csv", "TSV", "xlsx", "XLSM"] {
        assert!(crate::home::table_paths(
            Path::new(&format!("a.{ext}")),
            Path::new("b.txt")
        ));
    }
    assert!(!crate::home::table_paths(
        Path::new("a.txt"),
        Path::new("b.cpp")
    ));
    let mut v = view();
    v.begin_edit(1, Cursor { row: 0, column: 1 }, Some("dirty".into()));
    v.commit_edit().unwrap();
    v.request_home();
    assert!(v.pending.is_some());
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    let out = frame(&ctx, &mut v, vec![]);
    let cancel = button(&out, "Cancel");
    click(&ctx, &mut v, cancel);
    assert!(v.pending.is_none());
    assert!(v.is_dirty());
    assert!(v.action.is_none());
    let _ = GridState::default();
}
