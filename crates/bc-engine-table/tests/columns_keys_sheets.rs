use bc_core::cancel::NeverCancel;
use bc_engine_table::{
    columns::{align_columns, ColumnChange, ColumnMapping, ComparisonColumn, Side},
    inference::infer_keys,
    rules::{ColumnType, DateTolerance, KeySelection},
    sheets::SheetPairs,
    Cell, ColumnHandling, Sheet, TableRules,
};
fn table(headers: &[&str], rows: &[&[&str]]) -> Sheet {
    let mut s = Sheet::default();
    s.headers = Some(headers.iter().map(|s| s.to_string()).collect());
    for row in rows {
        s.push_row(row.iter().map(|s| Cell::text(*s)).collect());
    }
    s
}
#[test]
fn mapping_moves_each_side_whole_columns_tidy_and_roundtrips() {
    let mut m = ColumnMapping::by_number(3, 5);
    m.move_to(4, 2, Some(Side::Right)).unwrap();
    assert_eq!(m.0[2].left, Some(2));
    assert_eq!(m.0[2].right, Some(4));
    m.0[2].handling.is_key = true;
    m.move_to(2, 0, None).unwrap();
    assert!(m.0[0].handling.is_key);
    m.0.push(ComparisonColumn::new(None, None));
    m.tidy();
    assert_eq!(m.0.len(), 5);
    m.validate(3, 5).unwrap();
    let r = TableRules {
        columns: m,
        default_handling: ColumnHandling {
            ignore_case: true,
            ignore_whitespace: true,
            numeric_tolerance: 0.01,
            date_tolerance: DateTolerance::days(1),
            ..Default::default()
        },
        ..Default::default()
    };
    let json = serde_json::to_string(&r).unwrap();
    let restored: TableRules = serde_json::from_str(&json).unwrap();
    assert_eq!(r, restored);
    let inherited = r.handling(0);
    assert!(inherited.is_key);
    assert!(inherited.ignore_case);
    assert_eq!(inherited.normalize_text("  APPLE   Pie  "), "apple pie");
    assert_eq!(inherited.date_tolerance, DateTolerance::hours(24));
    let own = ColumnHandling {
        column_type: ColumnType::Text,
        ..Default::default()
    };
    assert!(!own.resolved(&inherited).ignore_case);
}
#[test]
fn keys_infer_unique_id_with_95_percent_overlap_and_lock_on_user_edit() {
    let mut l = Sheet::default();
    let mut r = Sheet::default();
    for i in 0..100 {
        l.push_row(vec![Cell::number(i as f64), Cell::text("same")]);
        r.push_row(vec![Cell::number((i + 5) as f64), Cell::text("same")]);
    }
    let mut rules = TableRules {
        columns: ColumnMapping::by_number(2, 2),
        ..Default::default()
    };
    let suggestion = infer_keys(&l, &r, &rules, &NeverCancel).unwrap();
    assert!(suggestion.apply(&mut rules));
    assert_eq!(rules.keys.label(), "自动");
    assert_eq!(rules.key_columns(), vec![0]);
    rules.set_manual_keys(vec![1]).unwrap();
    assert!(!suggestion.apply(&mut rules));
    assert_eq!(rules.key_columns(), vec![1]);
    assert_eq!(rules.keys.label(), "手动");
}
#[test]
fn adjacent_composite_key_and_no_suitable_key() {
    let mut l = Sheet::default();
    for a in ["2026-01-01", "2026-01-02"] {
        for b in ["a", "b", "c"] {
            l.push_row(vec![Cell::text(a), Cell::text(b), Cell::text("constant")]);
        }
    }
    let rules = TableRules {
        columns: ColumnMapping::by_number(3, 3),
        ..Default::default()
    };
    let suggestion = infer_keys(&l, &l, &rules, &NeverCancel).unwrap();
    assert!(
        matches!(suggestion.selection,KeySelection::Automatic{ref columns,..} if columns==&vec![0,1])
    );
    l.push_row(vec![
        Cell::text("2026-01-01"),
        Cell::text("a"),
        Cell::text("constant"),
    ]);
    // No suitable key: stay in empty-Automatic (content alignment) instead of
    // AllColumns, so similar rows pair as changed rather than orphaning.
    assert_eq!(
        infer_keys(&l, &l, &rules, &NeverCancel).unwrap().selection,
        KeySelection::Automatic {
            columns: Vec::new(),
            score: 0.0,
        }
    );
    assert!(infer_keys(&l, &l, &rules, &|| true).is_err());
}
#[test]
fn structural_columns_insert_delete_rename_move_and_positional_compatibility() {
    let l = table(
        &["id", "qty", "name"],
        &[&["1", "7", "apple"], &["2", "8", "pear"]],
    );
    let r = table(
        &[" name ", "ID", "note", "quantity"],
        &[&["apple", "1", "new", "7"], &["pear", "2", "new", "8"]],
    );
    let m = align_columns(&l, &r, true, &NeverCancel).unwrap();
    let id = m.0.iter().find(|c| c.left == Some(0)).unwrap();
    assert_eq!(id.right, Some(1));
    assert!(id.changes.contains(&ColumnChange::Moved));
    let qty = m.0.iter().find(|c| c.left == Some(1)).unwrap();
    assert_eq!(qty.right, Some(3));
    assert!(qty.changes.contains(&ColumnChange::Renamed));
    assert!(m.0.iter().any(|c| c.left.is_none()
        && c.right == Some(2)
        && c.changes == vec![ColumnChange::Inserted]));
    let by_number = ColumnMapping::by_number(3, 4);
    assert_eq!(by_number.0[1].right, Some(1));
    let reverse = align_columns(&r, &l, false, &NeverCancel).unwrap();
    assert!(reverse
        .0
        .iter()
        .any(|c| c.changes.contains(&ColumnChange::Deleted)));
}
#[test]
fn insertion_in_middle_does_not_shift_remaining_columns() {
    let l = table(&["id", "name", "price"], &[&["7", "apple", "1.0"]]);
    let r = table(
        &["id", "name", "note", "price"],
        &[&["7", "apple", "memo", "1.0"]],
    );
    let m = align_columns(&l, &r, true, &NeverCancel).unwrap();
    assert_eq!(
        m.0.iter().map(|c| (c.left, c.right)).collect::<Vec<_>>(),
        vec![
            (Some(0), Some(0)),
            (Some(1), Some(1)),
            (None, Some(2)),
            (Some(2), Some(3))
        ]
    );
}
#[test]
fn sheets_name_pairing_source_order_and_same_file_manual_pairing() {
    let l = vec!["Summary".into(), "Data".into(), "2024".into()];
    let r = vec!["data".into(), " summary ".into(), "2025".into()];
    let mut pairs = SheetPairs::by_name(&l, &r, true);
    assert_eq!(pairs.0[0].right, Some(1));
    assert_eq!(pairs.0[1].right, Some(0));
    assert_eq!(pairs.0[2].right, None);
    pairs.move_to(3, 2, Side::Right).unwrap();
    assert_eq!(pairs.0[2].right, Some(2));
    let mut same = SheetPairs::by_name(&l, &l, true);
    same.move_to(2, 0, Side::Right).unwrap();
    assert_eq!(same.0[0].left, Some(0));
    assert_eq!(same.0[0].right, Some(2));
    assert_eq!(SheetPairs::by_name(&l, &r, false).0[0].left, Some(1));
    let serialized = serde_json::to_string(&same).unwrap();
    assert_eq!(
        serde_json::from_str::<SheetPairs>(&serialized).unwrap(),
        same
    );
}
