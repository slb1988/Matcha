use super::super::grid::{GUTTER, ROW_HEIGHT};
use super::*;
use bc_engine_table::columns::ColumnMapping;

fn choose(ctx: &egui::Context, v: &mut TableCompareView, label: &str) {
    frame(ctx, v, vec![]);
    let out = frame(ctx, v, vec![]);
    click(ctx, v, button(&out, label));
}
fn edit(ctx: &egui::Context, v: &mut TableCompareView, label: &str) {
    choose(ctx, v, "Edit");
    choose(ctx, v, label);
}
fn text(v: &TableCompareView, side: usize, row: usize, column: usize) -> String {
    let p = v.pair.as_ref().unwrap();
    let s = if side == 0 {
        p.map.left.borrow()
    } else {
        p.map.right.borrow()
    };
    s.display_at(row, column).unwrap()
}
fn point(v: &TableCompareView, side: usize, row: usize, column: usize) -> egui::Pos2 {
    v.grid.pane_rects[side].left_top()
        + egui::vec2(
            GUTTER + v.grid.offsets[column] - v.grid.scroll.x + 10.0,
            row as f32 * ROW_HEIGHT - v.grid.scroll.y + ROW_HEIGHT / 2.0,
        )
}

fn click_cell(
    ctx: &egui::Context,
    v: &mut TableCompareView,
    side: usize,
    row: usize,
    column: usize,
) {
    let pos = point(v, side, row, column);
    click(ctx, v, pos);
}

#[test]
fn every_insert_command_has_correct_source_and_comparison_position_and_one_undo_redo() {
    for (label, op, expected) in [
        ("Insert Row Before", 0, 1),
        ("Insert Row After", 1, 2),
        ("Insert Column Before", 3, 1),
        ("Insert Column After", 4, 2),
    ] {
        let mut v = view();
        let ctx = egui::Context::default();
        v.grid.active = 1;
        v.grid.cursor = Cursor { row: 1, column: 1 };
        edit(&ctx, &mut v, label);
        ready(&mut v);
        let p = v.pair.as_ref().unwrap();
        if op < 2 {
            assert_eq!(p.map.right.borrow().row_count(), 3);
            assert_eq!(text(&v, 1, expected, 0), "");
            assert_eq!(text(&v, 1, if expected == 1 { 2 } else { 1 }, 0), "8");
        } else {
            assert_eq!(p.map.right.borrow().column_count(), 4);
            assert_eq!(p.map.rules().columns.0[expected].right, Some(expected));
            assert_eq!(p.map.rules().columns.0[expected].left, None);
            assert_eq!(text(&v, 1, 0, expected), "");
        }
        assert_eq!(p.map.left.borrow().row_count(), 2);
        assert_eq!(p.map.left.borrow().column_count(), 3);
        v.undo(false);
        ready(&mut v);
        assert_eq!(v.pair.as_ref().unwrap().map.right.borrow().row_count(), 2);
        assert_eq!(
            v.pair.as_ref().unwrap().map.right.borrow().column_count(),
            3
        );
        assert_eq!(text(&v, 1, 1, 1), "PEAR");
        assert!(!v.pair.as_ref().unwrap().history[1].can_undo());
        v.undo(true);
        ready(&mut v);
        assert!(v.pair.as_ref().unwrap().history[1].can_undo());
    }
}

#[test]
fn insert_into_a_gap_uses_the_next_source_position_not_the_start_of_the_file() {
    for op in [0, 1] {
        let mut v = TableCompareView::from_sheets(
            sheet("id,n\n1,a\n2,b\n3,c\n"),
            sheet("id,n\n1,a\n3,c\n"),
            TableRules {
                keys: KeySelection::Manual(vec![0]),
                ..Default::default()
            },
        );
        ready(&mut v);
        v.grid.active = 1;
        v.grid.cursor = Cursor { row: 1, column: 1 };
        assert!(v.pair.as_ref().unwrap().map.row(1).unwrap().right.is_none());
        v.structure(op).unwrap();
        ready(&mut v);
        assert_eq!(text(&v, 1, 0, 0), "1");
        assert_eq!(text(&v, 1, 1, 0), "");
        assert_eq!(text(&v, 1, 2, 0), "3");
    }
}

#[test]
fn rapid_structural_commands_reject_stale_coordinates_and_empty_tables_can_be_built() {
    let mut v = view();
    v.structure(0).unwrap();
    let revision = v.pair.as_ref().unwrap().map.left.borrow().revision();
    assert!(v.structure(2).is_err());
    assert_eq!(
        v.pair.as_ref().unwrap().map.left.borrow().revision(),
        revision
    );
    ready(&mut v);
    v.undo(false);
    ready(&mut v);
    assert!(!v.pair.as_ref().unwrap().history[0].can_undo());
    let mut v =
        TableCompareView::from_sheets(Sheet::default(), Sheet::default(), TableRules::default());
    ready(&mut v);
    v.structure(3).unwrap();
    ready(&mut v);
    v.structure(0).unwrap();
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().map.left.borrow().row_count(), 1);
    assert_eq!(v.pair.as_ref().unwrap().map.left.borrow().column_count(), 1);
    v.undo(false);
    ready(&mut v);
    v.undo(false);
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().map.left.borrow().row_count(), 0);
    assert_eq!(v.pair.as_ref().unwrap().map.left.borrow().column_count(), 0);
}

#[test]
fn align_with_opposite_row_clear_and_keyboard_undo_redo_target_the_correct_history() {
    let mut v = TableCompareView::from_sheets(
        sheet("id,n\n1,a\n2,b\n"),
        sheet("id,n\n3,c\n4,d\n"),
        TableRules {
            columns: ColumnMapping::by_number(2, 2),
            keys: KeySelection::Manual(vec![0]),
            ..Default::default()
        },
    );
    ready(&mut v);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    click_cell(&ctx, &mut v, 0, 0, 0);
    edit(&ctx, &mut v, "Align With...");
    click_cell(&ctx, &mut v, 0, 1, 0);
    assert_eq!(v.align_with, Some((0, 0)));
    // Unmatched keys are tie-paired by the shared Smart aligner and then split by key,
    // so the orphans interleave (L "1", R "3", L "2", R "4"): right source row 0 is
    // display row 1.
    click_cell(&ctx, &mut v, 1, 1, 0);
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().map.overrides().pairs(), &[(0, 0)]);
    assert_eq!(v.grid.active, 1);
    assert_eq!(v.pair.as_ref().unwrap().map.row_count(), 3);
    frame(&ctx, &mut v, vec![key(egui::Key::Z, true)]);
    ready(&mut v);
    assert!(v.pair.as_ref().unwrap().map.overrides().pairs().is_empty());
    frame(&ctx, &mut v, vec![key(egui::Key::Y, true)]);
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().map.overrides().pairs(), &[(0, 0)]);
    edit(&ctx, &mut v, "Clear Alignment Overrides");
    ready(&mut v);
    assert!(v.pair.as_ref().unwrap().map.overrides().pairs().is_empty());
    v.undo(false);
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().map.overrides().pairs(), &[(0, 0)]);
}

#[test]
fn select_all_cut_delete_and_readonly_protection_work_through_raw_events() {
    let mut v = view();
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    click_cell(&ctx, &mut v, 0, 0, 0);
    edit(&ctx, &mut v, "Select All");
    let out = frame(&ctx, &mut v, vec![egui::Event::Cut]);
    assert!(out
        .platform_output
        .commands
        .iter()
        .any(|c| matches!(c, egui::OutputCommand::CopyText(t) if t.contains("7\tapple\t1.0"))));
    ready(&mut v);
    assert_eq!(text(&v, 0, 0, 0), "");
    assert_eq!(text(&v, 0, 1, 2), "");
    frame(&ctx, &mut v, vec![key(egui::Key::Z, true)]);
    ready(&mut v);
    assert_eq!(text(&v, 0, 0, 0), "7");
    assert_eq!(text(&v, 0, 1, 2), "2.0");
    frame(&ctx, &mut v, vec![key(egui::Key::Delete, false)]);
    ready(&mut v);
    assert_eq!(text(&v, 0, 1, 2), "");
    v.undo(false);
    ready(&mut v);
    v.files[0].readonly = true;
    let revision = v.pair.as_ref().unwrap().map.left.borrow().revision();
    frame(
        &ctx,
        &mut v,
        vec![egui::Event::Cut, key(egui::Key::Delete, false)],
    );
    assert_eq!(
        v.pair.as_ref().unwrap().map.left.borrow().revision(),
        revision
    );
}

#[test]
fn ime_preedit_stays_in_the_inline_draft_and_commit_is_one_undoable_edit() {
    let mut v = view();
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    click_cell(&ctx, &mut v, 1, 0, 1);
    frame(&ctx, &mut v, vec![key(egui::Key::F2, false)]);
    frame(&ctx, &mut v, vec![]);
    frame(&ctx, &mut v, vec![key(egui::Key::A, true)]);
    let out = frame(
        &ctx,
        &mut v,
        vec![egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "中文".into(),
            active_range_chars: Some(0..2),
        })],
    );
    assert_eq!(text(&v, 1, 0, 1), "apple");
    assert!(out.platform_output.ime.is_some());
    frame(
        &ctx,
        &mut v,
        vec![egui::Event::Ime(egui::ImeEvent::Commit("中文".into()))],
    );
    assert_eq!(v.editor.as_ref().unwrap().text, "中文");
    assert_eq!(text(&v, 1, 0, 1), "apple");
    frame(&ctx, &mut v, vec![key(egui::Key::Enter, true)]);
    ready(&mut v);
    assert_eq!(text(&v, 1, 0, 1), "中文");
    frame(&ctx, &mut v, vec![key(egui::Key::Z, true)]);
    ready(&mut v);
    assert_eq!(text(&v, 1, 0, 1), "apple");
    assert!(!v.pair.as_ref().unwrap().history[1].can_undo());
    v.files[1].readonly = true;
    frame(
        &ctx,
        &mut v,
        vec![
            key(egui::Key::F2, false),
            egui::Event::Ime(egui::ImeEvent::Commit("blocked".into())),
        ],
    );
    assert!(v.editor.is_none());
    assert_eq!(text(&v, 1, 0, 1), "apple");
}
