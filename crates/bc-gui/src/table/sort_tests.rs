use super::*;
use bc_engine_table::columns::{ColumnMapping, ComparisonColumn};

fn make(left: &str, right: &str, rules: TableRules) -> TableCompareView {
    let mut v = TableCompareView::from_sheets(sheet(left), sheet(right), rules);
    ready(&mut v);
    v
}
fn positional() -> TableRules {
    TableRules {
        strategy: AlignStrategy::None,
        keys: KeySelection::Manual(vec![0]),
        ..Default::default()
    }
}
fn ids(v: &TableCompareView, side: usize) -> Vec<String> {
    let p = v.pair.as_ref().unwrap();
    let sheet = if side == 0 {
        p.map.left.borrow()
    } else {
        p.map.right.borrow()
    };
    (0..sheet.row_count())
        .map(|r| sheet.display_at(r, 0).unwrap())
        .collect()
}
fn choose(ctx: &egui::Context, v: &mut TableCompareView, label: &str) {
    frame(ctx, v, vec![]);
    let out = frame(ctx, v, vec![]);
    click(ctx, v, button(&out, label));
}
fn menu_sort(ctx: &egui::Context, v: &mut TableCompareView, descending: bool) {
    choose(ctx, v, "Edit");
    choose(
        ctx,
        v,
        if descending {
            "Sort Descending"
        } else {
            "Sort Ascending"
        },
    );
    ready(v);
}

#[test]
fn raw_menu_sorts_by_mapped_comparison_column_priority_stably_in_both_directions() {
    let csv =
        "id,minor,major,tag\n1,10,a,first\n2,2,b,second\n3,2,a,tie1\n4,2,a,tie2\n5,1,b,last\n";
    let mut rules = positional();
    rules.columns = ColumnMapping(
        [0, 2, 1, 3]
            .into_iter()
            .map(|c| ComparisonColumn::new(Some(c), Some(c)))
            .collect(),
    );
    let mut v = make(csv, csv, rules);
    v.grid.active = 1;
    v.grid.anchor = Some(Cursor { row: 0, column: 1 });
    v.grid.cursor = Cursor { row: 0, column: 2 };
    let ctx = egui::Context::default();
    menu_sort(&ctx, &mut v, false);
    assert_eq!(ids(&v, 1), ["3", "4", "1", "5", "2"]);
    assert_eq!(ids(&v, 0), ["1", "2", "3", "4", "5"]);
    assert_eq!(v.grid.active, 1);
    frame(&ctx, &mut v, vec![key(egui::Key::Z, true)]);
    ready(&mut v);
    assert_eq!(ids(&v, 1), ["1", "2", "3", "4", "5"]);
    assert!(!v.pair.as_ref().unwrap().history[1].can_undo());
    frame(&ctx, &mut v, vec![key(egui::Key::Y, true)]);
    ready(&mut v);
    assert_eq!(ids(&v, 1), ["3", "4", "1", "5", "2"]);
    v.undo(false);
    ready(&mut v);
    menu_sort(&ctx, &mut v, true);
    assert_eq!(ids(&v, 1), ["2", "5", "1", "3", "4"]);
}

#[test]
fn cross_row_filtered_selection_only_reorders_selected_source_slots() {
    let mut v = make(
        "id,n\n1,9\n2,-5\n3,3\n4,99\n5,1\n6,88\n",
        "id,n\n1,90\n2,-5\n3,30\n4,99\n5,10\n6,80\n",
        positional(),
    );
    v.filter = Filter::Differences;
    v.project();
    ready(&mut v);
    v.grid.anchor = Some(Cursor { row: 0, column: 1 });
    v.grid.cursor = Cursor { row: 4, column: 1 };
    let ctx = egui::Context::default();
    menu_sort(&ctx, &mut v, false);
    assert_eq!(ids(&v, 0), ["5", "2", "3", "4", "1", "6"]);
    assert_eq!(ids(&v, 1), ["1", "2", "3", "4", "5", "6"]);
    for row in [1, 3, 5] {
        assert!(!v.pair.as_ref().unwrap().map.left.borrow().is_edited(row));
    }
    v.undo(false);
    ready(&mut v);
    assert_eq!(ids(&v, 0), ["1", "2", "3", "4", "5", "6"]);
    v.undo(true);
    ready(&mut v);
    assert_eq!(ids(&v, 0), ["5", "2", "3", "4", "1", "6"]);
}

#[test]
fn sorted_alignment_selection_maps_back_to_source_slots_without_locking_automatic_keys() {
    let csv = "id,n\n3,30\n1,10\n2,20\n4,40\n";
    let mut v = make(
        csv,
        csv,
        TableRules {
            sort_by_key: true,
            ..Default::default()
        },
    );
    let rules = v.pair.as_ref().unwrap().map.rules().clone();
    assert!(matches!(rules.keys, KeySelection::Automatic { .. }));
    v.grid.anchor = Some(Cursor { row: 1, column: 1 });
    v.grid.cursor = Cursor { row: 2, column: 1 };
    v.structure(7).unwrap();
    ready(&mut v);
    assert_eq!(ids(&v, 0), ["2", "1", "3", "4"]);
    assert_eq!(*v.pair.as_ref().unwrap().map.rules(), rules);
}

#[test]
fn a_cross_row_selection_reduced_to_one_visible_row_does_not_expand_to_the_full_table() {
    let mut v = make(
        "id,n\n1,9\n2,3\n3,1\n",
        "id,n\n1,9\n2,30\n3,1\n",
        positional(),
    );
    v.filter = Filter::Differences;
    v.project();
    ready(&mut v);
    v.grid.anchor = Some(Cursor { row: 0, column: 1 });
    v.grid.cursor = Cursor { row: 2, column: 1 };
    let revision = v.pair.as_ref().unwrap().map.left.borrow().revision();
    v.structure(7).unwrap();
    assert_eq!(ids(&v, 0), ["1", "2", "3"]);
    assert_eq!(
        v.pair.as_ref().unwrap().map.left.borrow().revision(),
        revision
    );
    assert!(!v.pair.as_ref().unwrap().history[0].can_undo());
    // In contrast, a single-row/column selection deliberately sorts the whole
    // active side, even when the view is filtered (the user-approved rule).
    v.grid.anchor = None;
    v.grid.cursor = Cursor { row: 1, column: 1 };
    v.structure(7).unwrap();
    ready(&mut v);
    assert_eq!(ids(&v, 0), ["3", "2", "1"]);
}

#[test]
fn empty_hidden_and_already_sorted_selections_do_not_create_edits() {
    for mode in 0..4 {
        let csv = "id,n\n1,1\n2,2\n";
        let mut v = make(csv, csv, positional());
        v.grid.cursor = Cursor { row: 0, column: 1 };
        if mode == 1 {
            v.filter = Filter::None;
            v.project();
            ready(&mut v);
        } else if mode == 2 {
            v.hide_same = true;
            v.refresh_grid();
        } else if mode == 3 {
            v = TableCompareView::from_sheets(
                Sheet::default(),
                Sheet::default(),
                TableRules {
                    strategy: AlignStrategy::None,
                    keys: KeySelection::AllColumns,
                    ..Default::default()
                },
            );
            ready(&mut v);
        }
        let revision = v.pair.as_ref().unwrap().map.left.borrow().revision();
        v.structure(7).unwrap();
        assert_eq!(
            v.pair.as_ref().unwrap().map.left.borrow().revision(),
            revision,
            "mode {mode}"
        );
        assert!(
            !v.pair.as_ref().unwrap().history[0].can_undo(),
            "mode {mode}"
        );
        assert!(v.is_ready(), "mode {mode}");
    }
}

#[test]
fn hidden_comparison_columns_are_not_sort_keys() {
    let mut v = make(
        "id,major,minor\n1,b,1\n2,a,9\n",
        "id,major,minor\n1,a,1\n2,b,9\n",
        positional(),
    );
    v.hide_same = true;
    v.refresh_grid();
    assert_eq!(v.grid.columns, [1]);
    v.grid.anchor = Some(Cursor { row: 0, column: 1 });
    v.grid.cursor = Cursor { row: 0, column: 2 };
    v.structure(7).unwrap();
    ready(&mut v);
    assert_eq!(ids(&v, 0), ["2", "1"]);
}

#[test]
fn readonly_draft_stale_comparisons_and_source_gaps_cannot_trigger_sorting() {
    let csv = "id,n\n1,9\n2,1\n";
    let mut v = make(csv, csv, positional());
    v.grid.cursor = Cursor { row: 0, column: 1 };
    v.files[0].readonly = true;
    assert!(v.structure(7).is_err());
    v.files[0].readonly = false;
    v.begin_edit(0, v.grid.cursor, Some("5".into()));
    assert!(v.structure(7).is_err());
    assert_eq!(v.editor.as_ref().unwrap().text, "5");
    v.commit_edit().unwrap();
    let revision = v.pair.as_ref().unwrap().map.left.borrow().revision();
    assert!(v.structure(7).is_err());
    assert_eq!(
        v.pair.as_ref().unwrap().map.left.borrow().revision(),
        revision
    );
    v.undo(false);
    ready(&mut v);
    assert!(!v.pair.as_ref().unwrap().history[0].can_undo());
    assert_eq!(ids(&v, 0), ["1", "2"]);

    let mut gap = make(
        "id,n\n2,9\n3,1\n",
        "id,n\n1,0\n2,9\n3,1\n",
        TableRules {
            strategy: AlignStrategy::Quick,
            ..positional()
        },
    );
    gap.grid.cursor = Cursor { row: 0, column: 1 };
    assert!(gap
        .pair
        .as_ref()
        .unwrap()
        .map
        .row(0)
        .unwrap()
        .left
        .is_none());
    gap.structure(7).unwrap();
    assert!(!gap.pair.as_ref().unwrap().history[0].can_undo());
    assert_eq!(ids(&gap, 0), ["2", "3"]);
    let mut gap = make(
        "id,n\n1,9\n2,1\n",
        "id,n,extra\n1,9,a\n2,1,b\n",
        positional(),
    );
    gap.grid.cursor = Cursor { row: 0, column: 2 };
    gap.structure(7).unwrap();
    assert!(!gap.pair.as_ref().unwrap().history[0].can_undo());
    assert_eq!(ids(&gap, 0), ["1", "2"]);
}

#[test]
fn sorting_clears_anchors_transactionally_but_noop_sort_preserves_anchors_and_redo() {
    let csv = "id,n\n1,2\n2,1\n";
    let mut v = make(csv, csv, positional());
    let mut anchors = bc_core::align::AlignmentOverrides::default();
    anchors.insert(0, 0).unwrap();
    v.pair.as_ref().unwrap().map.set_overrides(anchors.clone());
    v.recompare();
    ready(&mut v);
    v.grid.cursor = Cursor { row: 0, column: 1 };
    v.structure(7).unwrap();
    ready(&mut v);
    assert!(v.pair.as_ref().unwrap().map.overrides().pairs().is_empty());
    v.undo(false);
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().map.overrides(), anchors);
    let revision = v.pair.as_ref().unwrap().map.left.borrow().revision();
    v.structure(6).unwrap();
    assert_eq!(
        v.pair.as_ref().unwrap().map.left.borrow().revision(),
        revision
    );
    assert_eq!(v.pair.as_ref().unwrap().map.overrides(), anchors);
    assert!(v.pair.as_ref().unwrap().history[0].can_redo());
}
