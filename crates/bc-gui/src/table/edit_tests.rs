use super::*;
use bc_engine_table::{CellValue, Style};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static ID: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "matcha-table-edit-{}-{}",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn action_frame(
    ctx: &egui::Context,
    v: &mut TableCompareView,
    events: Vec<egui::Event>,
) -> (egui::FullOutput, Option<TableAction>) {
    let mut action = None;
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
            action = v.ui(ui, &mut Prefs::default());
        },
    );
    output.textures_delta.clear();
    (output, action)
}
fn choose(ctx: &egui::Context, v: &mut TableCompareView, label: &str) -> Option<TableAction> {
    action_frame(ctx, v, vec![]);
    let (out, _) = action_frame(ctx, v, vec![]);
    let pos = button(&out, label);
    let mut action = None;
    for pressed in [true, false] {
        let (_, a) = action_frame(
            ctx,
            v,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
        action = action.or(a);
    }
    action
}
fn text(v: &TableCompareView, s: usize, r: usize, c: usize) -> String {
    let p = v.pair.as_ref().unwrap();
    if s == 0 {
        p.map.left.borrow().display_at(r, c).unwrap()
    } else {
        p.map.right.borrow().display_at(r, c).unwrap()
    }
}

#[test]
fn uncommitted_draft_cancel_and_discard_protect_home_and_exit() {
    let ctx = egui::Context::default();
    let mut v = view();
    frame(&ctx, &mut v, vec![]);
    v.begin_edit(1, Cursor { row: 0, column: 2 }, Some("3.00".into()));
    assert!(v.is_dirty());
    assert!(!v.pair.as_ref().unwrap().history[1].is_dirty());
    v.request_home();
    assert!(v.pending.is_some());
    assert!(choose(&ctx, &mut v, "Cancel").is_none());
    assert_eq!(v.editor.as_ref().unwrap().text, "3.00");
    assert_eq!(text(&v, 1, 0, 2), "1.2");
    assert!(v.is_dirty());
    v.request_exit();
    assert!(matches!(
        choose(&ctx, &mut v, "Discard"),
        Some(TableAction::Exit)
    ));
    assert!(v.editor.is_none());
    assert_eq!(text(&v, 1, 0, 2), "1.2");
    assert!(!v.is_dirty());
}
#[test]
fn save_before_exit_commits_the_draft_and_does_not_save_the_other_side() {
    let temp = Temp::new();
    let left = temp.0.join("left.csv");
    let right = temp.0.join("right.csv");
    std::fs::write(&left, "left sentinel").unwrap();
    std::fs::write(&right, "id,name,price\n7,apple,1.2\n8,PEAR,2.1\n").unwrap();
    let mut v = view();
    v.files[0].path = Some(left.clone());
    v.files[1].path = Some(right.clone());
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    v.begin_edit(1, Cursor { row: 0, column: 2 }, Some("12.50".into()));
    v.request_exit();
    assert!(matches!(
        choose(&ctx, &mut v, "Save changed files"),
        Some(TableAction::Exit)
    ));
    assert!(std::fs::read_to_string(right)
        .unwrap()
        .contains("7,apple,12.50"));
    assert_eq!(std::fs::read_to_string(left).unwrap(), "left sentinel");
    assert!(v.editor.is_none());
    assert!(!v.pair.as_ref().unwrap().history[1].is_dirty());
}
#[test]
fn clean_commit_keeps_formula_style_type_and_commit_error_keeps_the_draft() {
    let left = sheet("id,n\n1,6\n");
    let mut right = left.clone();
    let style = right.intern_style(Style {
        number_format: Some("0.00".into()),
    });
    let original = Cell {
        value: CellValue::Number(6.0),
        formula: Some("=1+5".into()),
        style: Some(style),
    };
    right.set_cell(0, 1, original.clone()).unwrap();
    let mut v = TableCompareView::from_sheets(
        left,
        right,
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            ..Default::default()
        },
    );
    ready(&mut v);
    v.begin_edit(1, Cursor { row: 0, column: 1 }, None);
    assert!(!v.is_dirty());
    v.commit_edit().unwrap();
    assert_eq!(
        v.pair.as_ref().unwrap().map.right.borrow().cell_snapshot(0, 1),
        Some(original)
    );
    assert!(!v.pair.as_ref().unwrap().history[1].can_undo());
    v.begin_edit(1, Cursor { row: 0, column: 1 }, Some("7".into()));
    v.files[1].readonly = true;
    assert!(v.commit_edit().is_err());
    assert_eq!(v.editor.as_ref().unwrap().text, "7");
    assert!(v.is_dirty());
    v.files[1].readonly = false;
    v.commit_edit().unwrap();
    ready(&mut v);
    let p = v.pair.as_ref().unwrap();
    let s = p.map.right.borrow();
    let cell = s.cell_snapshot(0, 1).unwrap();
    assert_eq!(cell.style, Some(style));
    assert_eq!(cell.formula, None);
}
#[test]
fn alignment_result_waits_for_draft_and_stale_held_result_cannot_overwrite_commit() {
    let mut v = view();
    v.begin_edit(1, Cursor { row: 1, column: 1 }, Some("PEARX".into()));
    v.commit_edit().unwrap();
    v.edited_at = Some(Instant::now() - Duration::from_secs(1));
    v.poll();
    v.begin_edit(1, Cursor { row: 0, column: 2 }, Some("7.00".into()));
    let started = Instant::now();
    while v.held_output.is_none() {
        v.poll();
        assert!(started.elapsed() < Duration::from_secs(5));
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!v.is_ready());
    assert_eq!(text(&v, 1, 0, 2), "1.2");
    v.commit_edit().unwrap();
    ready(&mut v);
    assert_eq!(text(&v, 1, 0, 2), "7.00");
    assert_eq!(text(&v, 1, 1, 1), "PEARX");
}
#[test]
fn inline_editor_is_anchored_to_cell_and_escape_cancels_without_history() {
    let ctx = egui::Context::default();
    let mut v = view();
    frame(&ctx, &mut v, vec![]);
    let pane = v.grid.pane_rects[1];
    let pos = pane.left_top() + egui::vec2(80.0, 10.0);
    click(&ctx, &mut v, pos);
    frame(&ctx, &mut v, vec![key(egui::Key::F2, false)]);
    let out = frame(&ctx, &mut v, vec![]);
    let rect = v.grid.edit_rect.unwrap();
    assert!(pane.contains(rect.center()));
    let commit = button(&out, "Commit (Ctrl+Enter)");
    assert!(
        commit.y >= rect.top() && commit.y < rect.top() + 150.0,
        "inline editor is not a detached window"
    );
    frame(
        &ctx,
        &mut v,
        vec![key(egui::Key::A, true), egui::Event::Text("edited".into())],
    );
    assert!(v.is_dirty());
    frame(&ctx, &mut v, vec![key(egui::Key::Escape, false)]);
    assert!(v.editor.is_none());
    assert!(!v.is_dirty());
    assert_eq!(text(&v, 1, 0, 0), "7");
}
#[test]
fn clipboard_quotes_tabs_newlines_quotes_and_empty_cells_roundtrip_via_raw_paste() {
    let mut left = Sheet::default();
    left.push_row(vec![
        Cell::text("tab\tquote\""),
        Cell::text("line1\r\nline2"),
    ]);
    left.push_row(vec![Cell::text(""), Cell::text("last")]);
    let mut right = Sheet::default();
    right.push_row(vec![Cell::text("x"), Cell::text("y")]);
    right.push_row(vec![Cell::text("z"), Cell::text("w")]);
    let mut v = TableCompareView::from_sheets(
        left,
        right,
        TableRules {
            strategy: AlignStrategy::None,
            ..Default::default()
        },
    );
    ready(&mut v);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    let pos = v.grid.pane_rects[0].left_top() + egui::vec2(80.0, 10.0);
    click(&ctx, &mut v, pos);
    frame(&ctx, &mut v, vec![key(egui::Key::A, true)]);
    let out = frame(&ctx, &mut v, vec![egui::Event::Copy]);
    let payload = out
        .platform_output
        .commands
        .iter()
        .find_map(|c| {
            if let egui::OutputCommand::CopyText(t) = c {
                Some(t.clone())
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(
        payload,
        "\"tab\tquote\"\"\"\t\"line1\r\nline2\"\r\n\"\"\tlast"
    );
    v.grid.active = 1;
    v.grid.cursor = Cursor::default();
    v.grid.anchor = None;
    v.paste_requested = true;
    v.focus_grid = Some(1);
    frame(&ctx, &mut v, vec![egui::Event::Paste(payload)]);
    ready(&mut v);
    for r in 0..2 {
        for c in 0..2 {
            assert_eq!(text(&v, 0, r, c), text(&v, 1, r, c));
        }
    }
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.same, 2);
    v.undo(false);
    ready(&mut v);
    assert_eq!(text(&v, 1, 0, 0), "x");
}
#[test]
fn find_textbox_paste_does_not_modify_table_and_menu_paste_has_an_explicit_target() {
    let ctx = egui::Context::default();
    let mut v = view();
    v.find.open = true;
    v.find.query = "query".into();
    frame(&ctx, &mut v, vec![]);
    let out = frame(&ctx, &mut v, vec![]);
    let pos = button(&out, "query");
    click(&ctx, &mut v, pos);
    frame(
        &ctx,
        &mut v,
        vec![
            key(egui::Key::A, true),
            egui::Event::Paste("changed-query".into()),
        ],
    );
    assert_eq!(v.find.query, "changed-query");
    assert!(!v.is_dirty());
    assert_eq!(text(&v, 0, 0, 0), "7");
    let out = frame(&ctx, &mut v, vec![]);
    click(&ctx, &mut v, button(&out, "Edit"));
    let out = frame(&ctx, &mut v, vec![]);
    click(&ctx, &mut v, button(&out, "Paste"));
    assert!(v.paste_requested);
    frame(&ctx, &mut v, vec![egui::Event::Paste("9".into())]);
    ready(&mut v);
    assert_eq!(text(&v, 0, 0, 0), "9");
    assert_eq!(v.find.query, "changed-query");
}
#[test]
fn selected_deletion_uses_filtered_rows_and_remapped_source_columns_and_undo() {
    let left = sheet("id,n\n1,a\n2,b\n3,c\n");
    let right = sheet("id,n\n1,x\n2,b\n3,y\n");
    let mut v = TableCompareView::from_sheets(
        left,
        right,
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            ..Default::default()
        },
    );
    ready(&mut v);
    v.filter = Filter::Differences;
    let p = v.pair.as_mut().unwrap();
    p.projection = p.cache.project(v.filter);
    v.grid.anchor = Some(Cursor { row: 0, column: 0 });
    v.grid.cursor = Cursor { row: 2, column: 1 };
    v.structure(2).unwrap();
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().map.left.borrow().row_count(), 1);
    assert_eq!(text(&v, 0, 0, 0), "2");
    v.undo(false);
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().map.left.borrow().row_count(), 3);
    let s = sheet("a,b,c,d\n10,11,12,13\n");
    let mut rules = TableRules {
        columns: bc_engine_table::columns::ColumnMapping::by_number(4, 4),
        strategy: AlignStrategy::None,
        ..Default::default()
    };
    rules
        .columns
        .move_to(3, 1, Some(bc_engine_table::columns::Side::Right))
        .unwrap();
    let mut v = TableCompareView::from_sheets(s.clone(), s, rules);
    ready(&mut v);
    v.grid.active = 1;
    v.grid.anchor = Some(Cursor { row: 0, column: 1 });
    v.grid.cursor = Cursor { row: 0, column: 2 };
    v.structure(5).unwrap();
    ready(&mut v);
    assert_eq!(
        v.pair.as_ref().unwrap().map.right.borrow().column_count(),
        2
    );
    assert_eq!(text(&v, 1, 0, 0), "10");
    assert_eq!(text(&v, 1, 0, 1), "12");
    v.undo(false);
    ready(&mut v);
    assert_eq!(
        v.pair.as_ref().unwrap().map.right.borrow().column_count(),
        4
    );
}

#[test]
fn stale_menu_paste_request_cannot_steal_a_find_textbox_paste() {
    let ctx = egui::Context::default();
    let mut v = view();
    v.find.open = true;
    v.find.query = "query".into();
    frame(&ctx, &mut v, vec![]);
    let out = frame(&ctx, &mut v, vec![]);
    click(&ctx, &mut v, button(&out, "query"));
    v.paste_requested = true; // A clipboard request returned no data before focus moved.
    frame(
        &ctx,
        &mut v,
        vec![
            key(egui::Key::A, true),
            egui::Event::Paste("replacement".into()),
        ],
    );
    assert_eq!(v.find.query, "replacement");
    assert!(!v.paste_requested);
    assert!(!v.is_dirty());
    assert_eq!(text(&v, 0, 0, 0), "7");
}
#[test]
fn empty_paste_is_not_an_edit_and_plain_text_paste_keeps_destination_style() {
    let left = sheet("id,n\n1,6\n");
    let mut right = left.clone();
    let style = right.intern_style(Style {
        number_format: Some("0.00".into()),
    });
    right
        .set_cell(
            0,
            1,
            Cell {
                value: CellValue::Number(6.0),
                formula: None,
                style: Some(style),
            },
        )
        .unwrap();
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
    v.grid.active = 1;
    v.grid.cursor = Cursor { row: 0, column: 1 };
    v.focus_grid = Some(1);
    frame(&ctx, &mut v, vec![]);
    frame(&ctx, &mut v, vec![egui::Event::Paste(String::new())]);
    assert!(!v.is_dirty());
    assert_eq!(text(&v, 1, 0, 1), "6");
    frame(&ctx, &mut v, vec![egui::Event::Paste("12.00".into())]);
    ready(&mut v);
    assert_eq!(text(&v, 1, 0, 1), "12.00");
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .right
            .borrow()
            .cell_snapshot(0, 1)
            .unwrap()
            .style,
        Some(style)
    );
}
#[test]
fn copy_button_restores_destination_keyboard_undo_and_redo() {
    let ctx = egui::Context::default();
    let mut v = view();
    frame(&ctx, &mut v, vec![]);
    let pos = v.grid.pane_rects[0].left_top()
        + egui::vec2(super::super::grid::GUTTER + v.grid.offsets[2] + 10.0, 10.0);
    click(&ctx, &mut v, pos);
    let out = frame(&ctx, &mut v, vec![]);
    click(&ctx, &mut v, button(&out, "Copy cell → other"));
    ready(&mut v);
    assert_eq!(v.grid.active, 1);
    assert_eq!(text(&v, 1, 0, 2), "1.0");
    frame(&ctx, &mut v, vec![key(egui::Key::Z, true)]);
    ready(&mut v);
    assert_eq!(text(&v, 1, 0, 2), "1.2");
    frame(&ctx, &mut v, vec![key(egui::Key::Y, true)]);
    ready(&mut v);
    assert_eq!(text(&v, 1, 0, 2), "1.0");
}
#[test]
fn raw_key_edit_commits_without_newline_and_realigns_then_undo_restores_orphans() {
    let mut v = TableCompareView::from_sheets(
        sheet("id,n\n7,a\n"),
        sheet("id,n\n8,a\n"),
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            strategy: AlignStrategy::Quick,
            ..Default::default()
        },
    );
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().map.row_count(), 2);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    let pos =
        v.grid.pane_rects[1].left_top() + egui::vec2(80.0, super::super::grid::ROW_HEIGHT + 10.0);
    click(&ctx, &mut v, pos);
    frame(&ctx, &mut v, vec![key(egui::Key::F2, false)]);
    frame(
        &ctx,
        &mut v,
        vec![key(egui::Key::A, true), egui::Event::Text("7".into())],
    );
    frame(&ctx, &mut v, vec![key(egui::Key::Enter, true)]);
    ready(&mut v);
    assert!(v.editor.is_none());
    assert_eq!(text(&v, 1, 0, 0), "7");
    assert_eq!(v.pair.as_ref().unwrap().map.statistics().same, 1);
    frame(&ctx, &mut v, vec![key(egui::Key::Z, true)]);
    ready(&mut v);
    assert_eq!(text(&v, 1, 0, 0), "8");
    assert_eq!(v.pair.as_ref().unwrap().map.row_count(), 2);
}
#[test]
fn delete_selected_display_rows_in_a_sorted_view_uses_source_indices() {
    let s = sheet("id,n\n3,c\n1,a\n2,b\n4,d\n");
    let mut v = TableCompareView::from_sheets(
        s.clone(),
        s,
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            sort_by_key: true,
            ..Default::default()
        },
    );
    ready(&mut v);
    v.grid.active = 1;
    v.grid.anchor = Some(Cursor::default());
    v.grid.cursor = Cursor { row: 1, column: 1 };
    v.structure(2).unwrap();
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().map.right.borrow().row_count(), 2);
    assert_eq!(text(&v, 1, 0, 0), "3");
    assert_eq!(text(&v, 1, 1, 0), "4");
    v.undo(false);
    ready(&mut v);
    assert_eq!(
        (0..4).map(|r| text(&v, 1, r, 0)).collect::<Vec<_>>(),
        ["3", "1", "2", "4"]
    );
}
