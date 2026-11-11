use super::*;

fn csv_view(left: &str, right: &str, sorted: bool) -> TableCompareView {
    let mut v = TableCompareView::from_sheets(
        sheet(left),
        sheet(right),
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            strategy: AlignStrategy::Quick,
            sort_by_key: sorted,
            ..Default::default()
        },
    );
    ready(&mut v);
    v
}
fn right_values(v: &TableCompareView, column: usize) -> Vec<String> {
    let sheet = v.pair.as_ref().unwrap().map.right.borrow();
    (0..sheet.row_count())
        .map(|r| sheet.display_at(r, column).unwrap())
        .collect()
}

#[test]
fn copy_cells_uses_visible_projection_and_does_not_touch_hidden_formulas() {
    let mut left = sheet("id,value,note\n1,A,keep\n2,same,keep\n3,C,keep\n");
    let mut right = sheet("id,value,note\n1,B,keep\n2,same,keep\n3,D,keep\n");
    for (s, formula) in [(&mut left, "=1"), (&mut right, "=9")] {
        for (r, c) in [(1, 1), (0, 2)] {
            let mut cell = s.cell(r, c).unwrap().clone();
            cell.formula = Some(formula.into());
            s.set_cell(r, c, cell).unwrap();
        }
    }
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
    v.hide_same = true;
    v.project();
    ready(&mut v);
    assert_eq!(v.row_count(), 2);
    assert_eq!(v.grid.columns, vec![1]);
    v.grid.active = 0;
    v.grid.anchor = Some(Cursor { row: 0, column: 0 });
    v.grid.cursor = Cursor { row: 2, column: 2 };
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]); // Settle the initial panel geometry before targeting a button.
    let out = frame(&ctx, &mut v, vec![]);
    click(&ctx, &mut v, button(&out, "Copy"));
    ready(&mut v);
    assert_eq!(right_values(&v, 1), ["A", "same", "C"]);
    {
        let s = v.pair.as_ref().unwrap().map.right.borrow();
        assert_eq!(
            s.cell_snapshot(1, 1).unwrap().formula.as_deref(),
            Some("=9")
        );
        assert_eq!(
            s.cell_snapshot(0, 2).unwrap().formula.as_deref(),
            Some("=9")
        );
    }
    v.undo(false);
    ready(&mut v);
    assert_eq!(right_values(&v, 1), ["B", "same", "D"]);
    assert!(!v.pair.as_ref().unwrap().history[1].can_undo());
    v.undo(true);
    ready(&mut v);
    assert_eq!(right_values(&v, 1), ["A", "same", "C"]);
}

#[test]
fn a_gap_rejects_the_entire_copy_without_history_or_partial_writes() {
    for whole_row in [false, true] {
        let mut v = csv_view("id,value\n2,new\n", "id,value\n1,keep\n2,old\n", false);
        let revision = v.pair.as_ref().unwrap().map.right.borrow().revision();
        v.grid.active = 0;
        v.grid.anchor = Some(Cursor { row: 0, column: 1 });
        v.grid.cursor = Cursor { row: 1, column: 1 };
        v.copy(whole_row);
        assert_eq!(right_values(&v, 1), ["keep", "old"]);
        assert_eq!(
            v.pair.as_ref().unwrap().map.right.borrow().revision(),
            revision
        );
        assert!(!v.pair.as_ref().unwrap().history[1].can_undo());
        assert_eq!(v.grid.active, 0);
        assert!(v.is_ready());
        assert!(v.message.contains("gap"));
    }
}

#[test]
fn show_none_copy_is_not_an_edit() {
    for whole_row in [false, true] {
        let mut v = csv_view("id,value\n1,new\n", "id,value\n1,old\n", false);
        v.filter = Filter::None;
        v.pair.as_mut().unwrap().projection = v.pair.as_ref().unwrap().cache.project(v.filter);
        v.grid.cursor = Cursor { row: 0, column: 1 };
        v.copy(whole_row);
        assert_eq!(right_values(&v, 1), ["old"]);
        assert!(!v.pair.as_ref().unwrap().history[1].can_undo());
        assert_eq!(v.grid.active, 0);
        assert!(v.is_ready());
    }
}

#[test]
fn copy_rows_in_sorted_filtered_view_plans_destination_insertions_before_editing() {
    let mut v = csv_view(
        "id,value\n7,new7\n5,keep5\n6,new6\n1,keep1\n3,new3\n2,new2\n",
        "id,value\n7,old7\n1,keep1\n3,old3\n5,keep5\n",
        true,
    );
    v.filter = Filter::Differences;
    v.project();
    ready(&mut v);
    assert_eq!(v.row_count(), 4);
    v.grid.anchor = Some(Cursor { row: 0, column: 0 });
    v.grid.cursor = Cursor { row: 5, column: 1 };
    v.copy(true);
    ready(&mut v);
    assert_eq!(right_values(&v, 0), ["6", "7", "1", "2", "3", "5"]);
    assert_eq!(
        right_values(&v, 1),
        ["new6", "new7", "keep1", "new2", "new3", "keep5"]
    );
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.same, 6);
    v.undo(false);
    ready(&mut v);
    assert_eq!(right_values(&v, 0), ["7", "1", "3", "5"]);
    assert_eq!(right_values(&v, 1), ["old7", "keep1", "old3", "keep5"]);
    assert!(!v.pair.as_ref().unwrap().history[1].can_undo());
    v.undo(true);
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.same, 6);
}
