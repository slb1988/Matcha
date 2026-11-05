use bc_core::{
    align::{AlignStrategy, AlignmentOverrides},
    cancel::NeverCancel,
    DiffKind,
};
use bc_engine_table::{
    columns::{ColumnMapping, Side},
    compare::{CellComparator, Importance},
    regional::DAY_MS,
    rules::{ColumnType, DateTolerance, KeySelection, Layers},
    Cell, CellValue, ColumnHandling, KeyAlign, Sheet, Style, TableMap, TableRules,
};
fn table(rows: &[&[&str]]) -> Sheet {
    let mut sheet = Sheet::default();
    for row in rows {
        sheet.push_row(row.iter().map(|s| Cell::text(*s)).collect());
    }
    sheet
}
fn rules(cols: usize) -> TableRules {
    let mut r = TableRules {
        columns: ColumnMapping::by_number(cols, cols),
        keys: KeySelection::Manual(vec![0]),
        ..Default::default()
    };
    r.set_manual_keys(vec![0]).unwrap();
    r
}
#[test]
fn all_strategies_match_keys_only_and_unaligned_is_explicit_exception() {
    let left = table(&[&["7", "apple", "1.0"], &["8", "pear", "2.0"]]);
    let right = table(&[&["7", "apple", "1.2"], &["9", "pear", "2.0"]]);
    for strategy in [
        AlignStrategy::Quick,
        AlignStrategy::Smart,
        AlignStrategy::Patience,
        AlignStrategy::None,
    ] {
        for closeness in [false, true] {
            let mut r = rules(3);
            r.strategy = strategy;
            r.use_closeness = closeness;
            let map = TableMap::new(left.clone(), right.clone(), r, &NeverCancel).unwrap();
            assert_eq!(map.row(0).unwrap().left, Some(0));
            assert_eq!(map.row(0).unwrap().right, Some(0));
            assert_eq!(map.cell_diff(0, 2).unwrap().value, Importance::Important);
            if strategy == AlignStrategy::None {
                assert_eq!(map.row_count(), 2);
            } else {
                assert_eq!(map.row_count(), 3);
                assert_eq!(map.statistics().left_only, 1);
                assert_eq!(map.statistics().right_only, 1);
            }
        }
    }
}
#[test]
fn duplicate_occurrence_sorting_composite_and_all_columns() {
    let l = table(&[&["3", "first"], &["3", "second"]]);
    let r = table(&[&["3", "first"]]);
    let map = TableMap::new(l, r, rules(2), &NeverCancel).unwrap();
    assert_eq!(map.row(0).unwrap().left, Some(0));
    assert_eq!(map.row(0).unwrap().right, Some(0));
    assert_eq!(map.row(1).unwrap().kind, DiffKind::LeftOnly);
    assert_eq!(map.duplicate_keys().left, 1);
    let l = table(&[&["10", "b"], &["2", "a"], &["1", "c"]]);
    let r = table(&[&["1", "c"], &["10", "b"], &["2", "a"]]);
    let mut settings = rules(2);
    settings.sort_by_key = true;
    let map = TableMap::new(l, r, settings, &NeverCancel).unwrap();
    assert_eq!(map.statistics().same, 3);
    assert_eq!(map.row(0).unwrap().left, Some(2));
    assert_eq!(map.row(1).unwrap().left, Some(1));
    let l = table(&[&["a", "1", "old"], &["a", "2", "same"]]);
    let r = table(&[&["a", "1", "new"], &["a", "2", "same"]]);
    let mut settings = rules(3);
    settings.set_manual_keys(vec![0, 1]).unwrap();
    assert_eq!(
        TableMap::new(l.clone(), r.clone(), settings, &NeverCancel)
            .unwrap()
            .row_count(),
        2
    );
    let settings = TableRules {
        columns: ColumnMapping::by_number(3, 3),
        keys: KeySelection::AllColumns,
        ..Default::default()
    };
    assert_eq!(
        TableMap::new(l, r, settings, &NeverCancel)
            .unwrap()
            .row_count(),
        3
    );
}
#[test]
fn manual_anchors_override_keys_and_clear_restores_automatic() {
    for strategy in [
        AlignStrategy::Quick,
        AlignStrategy::Smart,
        AlignStrategy::Patience,
        AlignStrategy::None,
    ] {
        let mut settings = rules(1);
        settings.strategy = strategy;
        let l = table(&[&["1"], &["2"], &["3"]]);
        let r = table(&[&["1"], &["4"], &["3"]]);
        let map = TableMap::new(l, r, settings, &NeverCancel).unwrap();
        let mut anchors = AlignmentOverrides::default();
        anchors.insert(1, 1).unwrap();
        map.set_overrides(anchors);
        map.recompare(&NeverCancel).unwrap();
        assert_eq!(map.row_count(), 3);
        assert_eq!(map.row(1).unwrap().importance, Importance::Important);
        map.set_overrides(AlignmentOverrides::default());
        map.recompare(&NeverCancel).unwrap();
        assert_eq!(
            map.row_count(),
            if strategy == AlignStrategy::None {
                3
            } else {
                4
            }
        );
    }
}
#[test]
fn key_handling_normalization_and_tolerance_cross_bucket_boundary() {
    let l = table(&[&["  APPLE  pie ", "old"]]);
    let r = table(&[&["apple pie", "new"]]);
    let mut settings = rules(2);
    settings.columns.0[0].handling = ColumnHandling {
        is_key: true,
        ignore_case: true,
        ignore_whitespace: true,
        column_type: ColumnType::Text,
        ..Default::default()
    };
    assert_eq!(
        TableMap::new(l, r, settings, &NeverCancel)
            .unwrap()
            .row_count(),
        1
    );
    let l = table(&[&["0.099", "x"], &["1.005", "y"], &["9", "z"]]);
    let r = table(&[&["0.101", "x"], &["1.001", "y"], &["10", "z"]]);
    let mut settings = rules(2);
    settings.columns.0[0].handling = ColumnHandling {
        is_key: true,
        numeric_tolerance: 0.01,
        ..Default::default()
    };
    let map = TableMap::new(l, r, settings, &NeverCancel).unwrap();
    assert_eq!(map.statistics().unimportant, 2);
    assert_eq!(map.row_count(), 4);
    let l = table(&[&["2026-01-01", "a"]]);
    let r = table(&[&["2026-01-02", "a"]]);
    let mut settings = rules(2);
    settings.columns.0[0].handling = ColumnHandling {
        is_key: true,
        date_tolerance: DateTolerance::days(1),
        ..Default::default()
    };
    assert_eq!(
        TableMap::new(l, r, settings, &NeverCancel)
            .unwrap()
            .statistics()
            .unimportant,
        1
    );
}
#[test]
fn cell_types_tolerances_text_case_whitespace_and_column_importance() {
    let l = table(&[&["Apple", "a  b", "1.001", "2026-01-01", "x"]]);
    let r = table(&[&["apple", "a b", "1.005", "2026-01-02", "y"]]);
    let mut h = ColumnHandling {
        ignore_case: true,
        ignore_whitespace: true,
        numeric_tolerance: 0.01,
        date_tolerance: DateTolerance::days(1),
        ..Default::default()
    };
    for col in 0..4 {
        assert_eq!(
            CellComparator {
                handling: &h,
                format_important: false
            }
            .compare(&l, &r, (Some(0), Some(0)), (Some(col), Some(col)))
            .value,
            Importance::Unimportant
        );
    }
    h.unimportant = true;
    assert_eq!(
        CellComparator {
            handling: &h,
            format_important: false
        }
        .compare(&l, &r, (Some(0), Some(0)), (Some(4), Some(4)))
        .value,
        Importance::Unimportant
    );
    let mut dated = Sheet::default();
    dated.push_row(vec![Cell::new(CellValue::Date(2 * DAY_MS))]);
    let mut zero = Sheet::default();
    zero.push_row(vec![Cell::new(CellValue::Date(0))]);
    h.unimportant = false;
    assert_eq!(
        CellComparator {
            handling: &h,
            format_important: false
        }
        .compare(&zero, &dated, (Some(0), Some(0)), (Some(0), Some(0)))
        .value,
        Importance::Important
    );
}
#[test]
fn three_layers_resolve_style_attributes_and_ignore_is_only_projection() {
    let mut l = Sheet::default();
    let mut r = Sheet::default();
    let ls = l.intern_style(Style {
        number_format: Some("0.00".into()),
    });
    r.intern_style(Style {
        number_format: Some("irrelevant".into()),
    });
    let rs = r.intern_style(Style {
        number_format: Some("0.00".into()),
    });
    l.push_row(vec![
        Cell::number(7.0),
        Cell {
            value: CellValue::Number(6.0),
            formula: Some("=A1+A2+A3".into()),
            style: Some(ls),
        },
    ]);
    r.push_row(vec![
        Cell::number(7.0),
        Cell {
            value: CellValue::Number(6.0),
            formula: Some("=SUM(A1:A3)".into()),
            style: Some(rs),
        },
    ]);
    let map = TableMap::new(l, r, rules(2), &NeverCancel).unwrap();
    assert_eq!(map.statistics().same, 1);
    assert_eq!(map.cell_diff(0, 1).unwrap().format, Importance::Same);
    map.rules_mut().layers = Layers::ValuesAndFormulas;
    assert!(!map.is_stale());
    assert_eq!(map.statistics().important, 1);
    // Copy formula too; style IDs must be remapped by attributes, not numbers.
    let _edit = map.copy_cell(0, 1, Side::Left).unwrap();
    assert_eq!(map.statistics().same, 1);
    let new_style = map.right.borrow_mut().intern_style(Style {
        number_format: Some("0.000".into()),
    });
    let mut cell = map.right.borrow().cell(0, 1).unwrap().clone();
    cell.style = Some(new_style);
    map.right.borrow_mut().set_cell(0, 1, cell).unwrap();
    map.recompare(&NeverCancel).unwrap();
    map.rules_mut().layers = Layers::All;
    assert_eq!(map.statistics().unimportant, 1);
    assert_eq!(map.statistics().importance(), Importance::Unimportant);
    map.rules_mut().ignore_unimportant = true;
    assert_eq!(map.statistics().same, 1);
    assert_eq!(map.next_difference_cell(None), None);
    assert_eq!(map.cell_diff(0, 1).unwrap().format, Importance::Unimportant);
}
#[test]
fn mixed_rows_orphans_and_packed_state_storage() {
    let l = table(&[&["1", "old", "old"], &["2", "orphan", "same"]]);
    let r = table(&[&["1", "new", "new"]]);
    let mut settings = rules(3);
    settings.columns.0[1].handling = ColumnHandling {
        unimportant: true,
        ..Default::default()
    };
    let map = TableMap::new(l, r, settings, &NeverCancel).unwrap();
    assert_eq!(map.cell_diff(0, 1).unwrap().value, Importance::Unimportant);
    assert_eq!(map.row(0).unwrap().importance, Importance::Important);
    map.rules_mut().ignore_unimportant = true;
    assert_eq!(map.row(1).unwrap().importance, Importance::Important);
    let job = bc_engine_table::jobs::TableAlignJob::from_map(&map);
    let out = job.execute(&NeverCancel).unwrap();
    assert_eq!(out.data.state_bytes(), (2usize * 3 * 6).div_ceil(8));
    assert_eq!(out.data.compact_row_bytes(), 2 * 12);
}
#[test]
fn cancellation_returns_no_map() {
    let s = table(&[&["1"]]);
    assert!(matches!(
        KeyAlign.align(&s, &s, &rules(1), &AlignmentOverrides::default(), &|| true),
        Err(bc_core::CoreError::Cancelled)
    ));
}
