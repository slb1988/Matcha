use bc_core::{
    align::{AlignStrategy, AlignmentOverrides},
    cancel::NeverCancel,
    edit::EditStack,
};
use bc_engine_table::{
    columns::{ColumnMapping, ComparisonColumn, Side},
    jobs::TableRealignJob,
    rules::KeySelection,
    source::{read_text, ReadOptions},
    CellValue, Sheet, Style, TableMap, TableRules,
};
use std::io::Cursor;

fn sheet(text: &str) -> Sheet {
    read_text(Cursor::new(text), &ReadOptions::default(), &NeverCancel)
        .unwrap()
        .sheet
}
fn rules() -> TableRules {
    TableRules {
        keys: KeySelection::Manual(vec![0]),
        strategy: AlignStrategy::Quick,
        ..Default::default()
    }
}

#[test]
fn mapped_right_to_left_cell_batch_keeps_types_formulas_styles_spelling_and_one_undo() {
    let mut left = sheet("id,name,price\n1,old,2\n2,old,3\n");
    left.intern_style(Style {
        number_format: Some("0%".into()),
    });
    let mut right = sheet("price,id,name\n1.00,1,new\n4.00,2,newer\n");
    let style = right.intern_style(Style {
        number_format: Some("0.00".into()),
    });
    let mut cell = right.cell(0, 2).unwrap().clone();
    cell.formula = Some("=A1".into());
    cell.style = Some(style);
    right.set_cell(0, 2, cell).unwrap();
    let mut rules = rules();
    rules.columns = ColumnMapping(vec![
        ComparisonColumn::new(Some(0), Some(1)),
        ComparisonColumn::new(Some(1), Some(2)),
        ComparisonColumn::new(Some(2), Some(0)),
    ]);
    let map = TableMap::new(left, right, rules, &NeverCancel).unwrap();
    let mut history = EditStack::new();
    history.push(
        map.copy_cells(&[(0, 1), (0, 2), (1, 1), (1, 2)], Side::Right)
            .unwrap(),
    );
    assert_eq!(map.statistics().same, 2);
    {
        let left = map.left.borrow();
        assert_eq!(left.cell_snapshot(0, 2).unwrap().value, CellValue::Number(1.0));
        assert_eq!(left.display_at(0, 2).unwrap(), "1.00");
        assert_eq!(left.display_at(1, 2).unwrap(), "4.00");
        let copied = left.cell_snapshot(0, 1).unwrap();
        assert_eq!(copied.formula.as_deref(), Some("=A1"));
        assert_eq!(
            left.style(copied.style).unwrap().number_format.as_deref(),
            Some("0.00")
        );
    }
    history.undo();
    assert!(!history.can_undo());
    assert_eq!(map.left.borrow().display_at(0, 1).unwrap(), "old");
    assert_eq!(map.left.borrow().display_at(1, 2).unwrap(), "3");
    history.redo();
    assert!(!history.can_redo());
    assert_eq!(map.statistics().same, 2);
}

#[test]
fn invalid_batch_and_stale_deferred_copy_leave_content_rules_and_revisions_unchanged() {
    let mut map = TableMap::new(
        sheet("id,n\n1,a\n2,b\n"),
        sheet("id,n\n1,x\n2,y\n"),
        rules(),
        &NeverCancel,
    )
    .unwrap();
    map.set_deferred_edits(true);
    let before = map.right.borrow().revision();
    let rules = map.rules().clone();
    assert!(map.copy_cells(&[(0, 1), (1, 50)], Side::Left).is_err());
    assert!(map.copy_rows(&[0, 50], Side::Left).is_err());
    assert!(map.copy_cells(&[], Side::Left).is_err());
    assert!(map.copy_rows(&[], Side::Left).is_err());
    assert_eq!(map.right.borrow().revision(), before);
    assert_eq!(*map.rules(), rules);
    assert_eq!(map.right.borrow().display_at(0, 1).unwrap(), "x");
    let old = TableRealignJob::from_map(&map)
        .execute(&NeverCancel)
        .unwrap();
    let mut history = EditStack::new();
    history.push(map.copy_cells(&[(0, 1), (1, 1)], Side::Left).unwrap());
    assert!(map.is_stale());
    let revision = map.right.borrow().revision();
    assert!(map.copy_row(0, Side::Left).is_err());
    assert!(map.copy_cell(0, 1, Side::Left).is_err());
    assert_eq!(map.right.borrow().revision(), revision);
    assert!(map.set_alignment(old).is_err());
    map.set_alignment(
        TableRealignJob::from_map(&map)
            .execute(&NeverCancel)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(map.statistics().same, 2);
    history.undo();
    assert!(!history.can_undo());
    assert_eq!(map.right.borrow().display_at(1, 1).unwrap(), "y");
}

#[test]
fn rows_copied_into_empty_side_add_columns_once_and_ignore_duplicate_selection_indices() {
    let map = TableMap::new(
        Sheet::default(),
        sheet("id,n\n1,a\n2,b\n"),
        rules(),
        &NeverCancel,
    )
    .unwrap();
    let mut history = EditStack::new();
    history.push(map.copy_rows(&[1, 0, 1], Side::Right).unwrap());
    assert_eq!(map.left.borrow().row_count(), 2);
    assert_eq!(map.left.borrow().column_count(), 2);
    assert_eq!(map.statistics().same, 2);
    assert_eq!(map.left.borrow().display_at(0, 0).unwrap(), "1");
    assert_eq!(map.left.borrow().display_at(1, 0).unwrap(), "2");
    history.undo();
    assert_eq!(map.left.borrow().column_count(), 0);
    assert_eq!(map.left.borrow().row_count(), 0);
    history.redo();
    assert_eq!(map.statistics().same, 2);
}

#[test]
fn consecutive_orphans_insert_before_the_same_anchor_and_shift_it_once_per_row() {
    let map = TableMap::new(
        sheet("id,n\n1,a\n2,b\n3,c\n4,d\n"),
        sheet("id,n\n1,a\n4,d\n"),
        rules(),
        &NeverCancel,
    )
    .unwrap();
    let mut anchors = AlignmentOverrides::default();
    anchors.insert(3, 1).unwrap();
    map.set_overrides(anchors.clone());
    map.recompare(&NeverCancel).unwrap();
    let mut history = EditStack::new();
    history.push(map.copy_rows(&[1, 2], Side::Left).unwrap());
    assert_eq!(map.statistics().same, 4);
    assert_eq!(map.right.borrow().display_at(1, 1).unwrap(), "b");
    assert_eq!(map.right.borrow().display_at(2, 1).unwrap(), "c");
    assert_eq!(map.overrides().pairs(), &[(3, 3)]);
    history.undo();
    assert_eq!(map.overrides(), anchors);
    assert_eq!(map.right.borrow().row_count(), 2);
    history.redo();
    assert_eq!(map.overrides().pairs(), &[(3, 3)]);
}
