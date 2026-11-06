use bc_core::cancel::NeverCancel;
use bc_engine_table::{
    rules::{ColumnHandling, ColumnType},
    source::{read_text, write_delimited, ReadOptions},
    Cell, CellValue, Sheet, Style,
};
use std::io::Cursor;

fn sheet(text: &str) -> Sheet {
    read_text(Cursor::new(text), &ReadOptions::default(), &NeverCancel)
        .unwrap()
        .sheet
}

#[test]
fn stable_multikey_sort_refills_only_unique_selected_source_slots() {
    let s = sheet("id,major,minor\n1,a,2\n2,z,99\n3,a,1\n4,a,1\n");
    let keys = [
        (1, ColumnHandling::default()),
        (2, ColumnHandling::default()),
    ];
    assert_eq!(
        s.sorted_row_order(&[3, 0, 2, 0], &keys, false).unwrap(),
        Some(vec![2, 1, 3, 0])
    );
    // Descending keeps the equal rows 3 and 4 in their original relative order.
    assert_eq!(s.sorted_row_order(&[3, 0, 2], &keys, true).unwrap(), None);
    let keys = [
        (2, ColumnHandling::default()),
        (1, ColumnHandling::default()),
    ];
    assert_eq!(
        s.sorted_row_order(&[0, 1, 2, 3], &keys, true).unwrap(),
        Some(vec![1, 0, 2, 3])
    );
}

#[test]
fn general_and_text_handling_use_numeric_values_and_lexical_spelling_respectively() {
    let s = sheet("id,n\n1,2\n2,10\n3,2\n");
    let rows = [0, 1, 2];
    assert_eq!(
        s.sorted_row_order(&rows, &[(1, ColumnHandling::default())], false)
            .unwrap(),
        Some(vec![0, 2, 1])
    );
    let text = ColumnHandling {
        column_type: ColumnType::Text,
        ..Default::default()
    };
    assert_eq!(
        s.sorted_row_order(&rows, &[(1, text)], false).unwrap(),
        Some(vec![1, 0, 2])
    );
}

#[test]
fn mixed_types_use_the_existing_alignment_total_order() {
    let mut s = Sheet::default();
    for value in [
        CellValue::Number(10.0),
        CellValue::Text("1a".into()),
        CellValue::Number(2.0),
        CellValue::Empty,
        CellValue::Date(5),
        CellValue::Bool(true),
        CellValue::Error("#N/A".into()),
    ] {
        s.push_row(vec![Cell::new(value)]);
    }
    let rows = (0..7).collect::<Vec<_>>();
    assert_eq!(
        s.sorted_row_order(&rows, &[(0, ColumnHandling::default())], false)
            .unwrap(),
        Some(vec![3, 2, 0, 4, 5, 1, 6])
    );
}

#[test]
fn normalized_ties_are_stable_but_tolerances_do_not_collapse_ordering() {
    let s = sheet("id,n\n1,a\n2,B\n3,A\n4,b\n");
    let handling = ColumnHandling {
        ignore_case: true,
        ..Default::default()
    };
    assert_eq!(
        s.sorted_row_order(&[0, 1, 2, 3], &[(1, handling.clone())], false)
            .unwrap(),
        Some(vec![0, 2, 1, 3])
    );
    assert_eq!(
        s.sorted_row_order(&[0, 1, 2, 3], &[(1, handling)], true)
            .unwrap(),
        Some(vec![1, 3, 0, 2])
    );
    let s = sheet("id,n\n1,1.005\n2,1.001\n3,1.003\n");
    let handling = ColumnHandling {
        numeric_tolerance: 0.1,
        ..Default::default()
    };
    assert_eq!(
        s.sorted_row_order(&[0, 1, 2], &[(1, handling)], false)
            .unwrap(),
        Some(vec![1, 2, 0])
    );
}

#[test]
fn invalid_inputs_and_identity_plans_never_mutate_the_sheet() {
    let s = sheet("id,n\n1,1\n2,2\n");
    let revision = s.revision();
    let keys = [(1, ColumnHandling::default())];
    assert!(s.sorted_row_order(&[0, 2], &keys, false).is_err());
    assert!(s
        .sorted_row_order(&[0, 1], &[(2, ColumnHandling::default())], false)
        .is_err());
    assert_eq!(s.sorted_row_order(&[], &keys, false).unwrap(), None);
    assert_eq!(s.sorted_row_order(&[0, 0], &keys, false).unwrap(), None);
    assert_eq!(s.sorted_row_order(&[0, 1], &[], false).unwrap(), None);
    assert_eq!(s.sorted_row_order(&[0, 1], &keys, false).unwrap(), None);
    assert_eq!(s.revision(), revision);
    assert!(!s.is_edited(0));
    assert!(!s.is_edited(1));
}

#[test]
fn reordering_moves_complete_cells_and_lexical_rows_without_marking_unselected_rows() {
    let mut s = sheet("id,key,note\r\n1,3.00,a\n2,2.00,b\r\n3,1.00,c");
    let style = s.intern_style(Style {
        number_format: Some("0.00".into()),
    });
    let mut cell = s.cell(0, 2).unwrap().clone();
    cell.formula = Some("=A1".into());
    cell.style = Some(style);
    s.set_cell(0, 2, cell).unwrap();
    let before = s.clone();
    let order = s
        .sorted_row_order(&[0, 2], &[(1, ColumnHandling::default())], false)
        .unwrap()
        .unwrap();
    assert_eq!(s.revision(), before.revision());
    s.reorder_rows(&order).unwrap();
    for (row, &old) in order.iter().enumerate() {
        for column in 0..s.column_count() {
            assert_eq!(s.cell(row, column), before.cell(old, column));
        }
    }
    assert_eq!(s.display_at(0, 1).unwrap(), "1.00");
    assert_eq!(s.display_at(2, 1).unwrap(), "3.00");
    assert!(!s.is_edited(1));
    assert!(s.is_edited(0));
    assert!(s.is_edited(2));
    let mut saved = Vec::new();
    write_delimited(&s, &mut saved).unwrap();
    let saved = String::from_utf8(saved).unwrap();
    assert!(saved.starts_with("id,key,note\r\n3,1.00,c"));
    assert!(saved.contains("2,2.00,b\r\n"));
    assert!(saved.contains("1,3.00,a"));
}
