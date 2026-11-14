use super::*;
use crate::{
    source::{read_text, ReadOptions},
    CellValue,
};
use bc_formats::model::HeaderMode;

/// Sheet from rows of cells: numbers when they parse, text otherwise.
fn sheet(rows: &[&[&str]]) -> Sheet {
    let mut sheet = Sheet::new("Sheet1");
    for row in rows {
        sheet.push_row(
            row.iter()
                .map(|v| match v.parse::<f64>() {
                    Ok(n) => Cell::number(n),
                    Err(_) if v.is_empty() => Cell::default(),
                    Err(_) => Cell::text(*v),
                })
                .collect(),
        );
    }
    sheet
}

fn keyed() -> TableMergeOptions {
    TableMergeOptions {
        keys: Some(vec![0]),
        ..Default::default()
    }
}

fn merge(l: &[&[&str]], c: &[&[&str]], r: &[&[&str]], options: &TableMergeOptions) -> SheetMerge {
    SheetMerge::new(
        "Sheet1",
        [Some(sheet(l)), Some(sheet(c)), Some(sheet(r))],
        options,
    )
    .unwrap()
}

fn texts(sheet: &Sheet) -> Vec<Vec<String>> {
    (0..sheet.row_count())
        .map(|r| {
            (0..sheet.column_count())
                .map(|c| sheet.display_at(r, c).unwrap_or_default())
                .collect()
        })
        .collect()
}

fn rows(expected: &[&[&str]]) -> Vec<Vec<String>> {
    expected
        .iter()
        .map(|r| r.iter().map(|s| s.to_string()).collect())
        .collect()
}

const BASE: &[&[&str]] = &[
    &["1", "apple", "10"],
    &["2", "pear", "20"],
    &["3", "plum", "30"],
];

#[test]
fn identical_inputs_merge_to_the_center() {
    let m = merge(BASE, BASE, BASE, &keyed());
    assert!(m.conflicts().is_empty());
    assert_eq!(texts(&m.output_sheet().unwrap()), rows(BASE));
}

#[test]
fn same_key_aligns_regardless_of_physical_position() {
    let left: &[&[&str]] = &[
        &["3", "plum", "30"],
        &["1", "apple", "10"],
        &["2", "pear", "20"],
    ];
    let m = merge(left, BASE, BASE, &keyed());
    assert!(m.rows.iter().all(|r| r.kind == RowKind::Normal));
    let row = m.rows.iter().find(|r| r.center == Some(2)).unwrap();
    assert_eq!(row.left, Some(0));
    // Order follows the center.
    assert_eq!(texts(&m.output_sheet().unwrap()), rows(BASE));
}

#[test]
fn different_columns_of_one_row_merge_without_conflict() {
    let left: &[&[&str]] = &[
        &["1", "apple", "11"],
        &["2", "pear", "20"],
        &["3", "plum", "30"],
    ];
    let right: &[&[&str]] = &[
        &["1", "APPLE", "10"],
        &["2", "pear", "20"],
        &["3", "plum", "30"],
    ];
    let m = merge(left, BASE, right, &keyed());
    assert_eq!(m.choice(0, 2), CellChoice::Left);
    assert_eq!(m.choice(0, 1), CellChoice::Right);
    assert!(m.conflicts().is_empty());
    assert_eq!(texts(&m.output_sheet().unwrap())[0], ["1", "APPLE", "11"]);
}

#[test]
fn same_change_on_both_sides_is_taken_once() {
    let both: &[&[&str]] = &[
        &["1", "apple", "99"],
        &["2", "pear", "20"],
        &["3", "plum", "30"],
    ];
    let m = merge(both, BASE, both, &keyed());
    assert_eq!(m.choice(0, 2), CellChoice::Both);
    assert!(m.conflicts().is_empty());
    assert_eq!(texts(&m.output_sheet().unwrap())[0], ["1", "apple", "99"]);
}

#[test]
fn conflicting_cell_keeps_base_and_counts() {
    let left: &[&[&str]] = &[
        &["1", "apple", "12"],
        &["2", "pear", "20"],
        &["3", "plum", "30"],
    ];
    let right: &[&[&str]] = &[
        &["1", "apple", "15"],
        &["2", "pear", "20"],
        &["3", "plum", "30"],
    ];
    let mut m = merge(left, BASE, right, &keyed());
    assert_eq!(m.choice(0, 2), CellChoice::Conflict);
    let conflicts = m.conflicts();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].rows, 0..1);
    assert_eq!(texts(&m.output_sheet().unwrap())[0], ["1", "apple", "10"]);
    m.take(0..1, Side::Right);
    assert!(m.conflicts().is_empty());
    assert_eq!(texts(&m.output_sheet().unwrap())[0], ["1", "apple", "15"]);
}

#[test]
fn adjacent_conflict_rows_form_one_section() {
    let left: &[&[&str]] = &[&["1", "a", "10"], &["2", "b", "20"], &["3", "plum", "30"]];
    let right: &[&[&str]] = &[&["1", "x", "10"], &["2", "y", "20"], &["3", "plum", "30"]];
    let m = merge(left, BASE, right, &keyed());
    let conflicts = m.conflicts();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].rows, 0..2);
}

#[test]
fn single_side_added_row_is_placed_after_its_predecessor() {
    let left: &[&[&str]] = &[
        &["1", "apple", "10"],
        &["9", "kiwi", "90"],
        &["2", "pear", "20"],
        &["3", "plum", "30"],
    ];
    let m = merge(left, BASE, BASE, &keyed());
    let added = m
        .rows
        .iter()
        .find(|r| r.kind == RowKind::LeftAdded)
        .unwrap();
    assert_eq!(
        (added.left, added.center, added.right),
        (Some(1), None, None)
    );
    assert_eq!(texts(&m.output_sheet().unwrap()), rows(left));
}

#[test]
fn both_sides_add_the_same_key() {
    let mut left = BASE.to_vec();
    left.push(&["9", "kiwi", "90"]);
    let mut right = BASE.to_vec();
    right.push(&["9", "kiwi", "90"]);
    let m = merge(&left, BASE, &right, &keyed());
    assert_eq!(m.rows.last().unwrap().kind, RowKind::BothAdded);
    assert_eq!(m.output_sheet().unwrap().row_count(), 4);

    let mut right = BASE.to_vec();
    right.push(&["9", "lime", "90"]);
    let mut m = merge(&left, BASE, &right, &keyed());
    assert_eq!(m.rows.last().unwrap().kind, RowKind::AddConflict);
    assert_eq!(m.conflicts().len(), 1);
    // Unresolved: neither new row is written.
    assert_eq!(m.output_sheet().unwrap().row_count(), 3);
    m.take(3..4, Side::Right);
    assert_eq!(texts(&m.output_sheet().unwrap())[3], ["9", "lime", "90"]);
}

#[test]
fn delete_against_unchanged_and_against_modified() {
    let deleted: &[&[&str]] = &[&["1", "apple", "10"], &["3", "plum", "30"]];
    let m = merge(deleted, BASE, BASE, &keyed());
    assert_eq!(m.rows[1].kind, RowKind::Deleted);
    assert!(m.conflicts().is_empty());
    assert_eq!(texts(&m.output_sheet().unwrap()), rows(deleted));

    let modified: &[&[&str]] = &[
        &["1", "apple", "10"],
        &["2", "pear", "25"],
        &["3", "plum", "30"],
    ];
    let mut m = merge(deleted, BASE, modified, &keyed());
    assert_eq!(m.rows[1].kind, RowKind::DeleteConflict);
    // Unresolved: the center row stays.
    assert_eq!(texts(&m.output_sheet().unwrap())[1], ["2", "pear", "20"]);
    m.take(1..2, Side::Left);
    assert!(m.conflicts().is_empty());
    assert_eq!(texts(&m.output_sheet().unwrap()), rows(deleted));
    m.take(1..2, Side::Right);
    assert_eq!(texts(&m.output_sheet().unwrap())[1], ["2", "pear", "25"]);
}

#[test]
fn section_take_resolves_structural_and_cell_conflicts() {
    let left: &[&[&str]] = &[&["1", "apple", "12"], &["3", "plum", "30"]];
    let right: &[&[&str]] = &[
        &["1", "apple", "15"],
        &["2", "pear", "21"],
        &["3", "plum", "30"],
    ];
    let mut m = merge(left, BASE, right, &keyed());
    let conflicts = m.conflicts();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].rows, 0..2);
    m.take(conflicts[0].rows.clone(), Side::Right);
    assert!(m.conflicts().is_empty());
    assert_eq!(texts(&m.output_sheet().unwrap()), rows(right));
}

#[test]
fn accept_and_edit_resolve_without_taking_a_side() {
    let left: &[&[&str]] = &[
        &["1", "apple", "12"],
        &["2", "pear", "20"],
        &["3", "plum", "30"],
    ];
    let right: &[&[&str]] = &[
        &["1", "apple", "15"],
        &["2", "pear", "20"],
        &["3", "plum", "30"],
    ];
    let mut m = merge(left, BASE, right, &keyed());
    m.accept(0..1);
    assert!(m.conflicts().is_empty());
    assert_eq!(texts(&m.output_sheet().unwrap())[0][2], "10");

    let mut m = merge(left, BASE, right, &keyed());
    assert!(m.edit(0, 2, Cell::number(13.0)));
    assert!(m.conflicts().is_empty());
    assert_eq!(m.cell_source(0, 2), Some(CellSource::Edited));
    assert_eq!(texts(&m.output_sheet().unwrap())[0][2], "13");
}

#[test]
fn tolerance_differences_are_not_changes() {
    let left: &[&[&str]] = &[
        &["1", "apple", "10.005"],
        &["2", "pear", "20"],
        &["3", "plum", "30"],
    ];
    let options = TableMergeOptions {
        keys: Some(vec![0]),
        handling: vec![
            ColumnHandling::default(),
            ColumnHandling::default(),
            ColumnHandling {
                numeric_tolerance: 0.01,
                ..Default::default()
            },
        ],
        ignore_unimportant: true,
        ..Default::default()
    };
    let m = merge(left, BASE, BASE, &options);
    assert_eq!(m.choice(0, 2), CellChoice::Center);
}

#[test]
fn unimportant_column_does_not_conflict_and_favor_picks_a_side() {
    let left: &[&[&str]] = &[
        &["1", "apple", "note-l"],
        &["2", "pear", "20"],
        &["3", "plum", "30"],
    ];
    let right: &[&[&str]] = &[
        &["1", "apple", "note-r"],
        &["2", "pear", "20"],
        &["3", "plum", "30"],
    ];
    let unimportant = ColumnHandling {
        unimportant: true,
        ..Default::default()
    };
    let mut options = TableMergeOptions {
        keys: Some(vec![0]),
        handling: vec![
            ColumnHandling::default(),
            ColumnHandling::default(),
            unimportant,
        ],
        ignore_unimportant: true,
        ..Default::default()
    };
    let m = merge(left, BASE, right, &options);
    assert!(m.conflicts().is_empty());
    assert_eq!(texts(&m.output_sheet().unwrap())[0][2], "10");
    options.favor = Favor::Right;
    let m = merge(left, BASE, right, &options);
    assert_eq!(texts(&m.output_sheet().unwrap())[0][2], "note-r");
    // Without ignore-unimportant the difference conflicts and favor changes nothing.
    options.ignore_unimportant = false;
    let m = merge(left, BASE, right, &options);
    assert_eq!(m.conflicts().len(), 1);
}

#[test]
fn keys_are_inferred_and_fall_back_to_whole_rows() {
    let base: &[&[&str]] = &[
        &["id", "name", "qty"],
        &["1", "apple", "10"],
        &["2", "pear", "20"],
        &["3", "plum", "30"],
        &["4", "fig", "40"],
        &["5", "kiwi", "50"],
    ];
    let mut left = base.to_vec();
    left[2] = &["2", "pear", "22"];
    let m = merge(&left, base, base, &TableMergeOptions::default());
    assert_eq!(m.keys.source, KeySource::Inferred);
    assert!(m.rows.iter().all(|r| r.kind == RowKind::Normal));

    // No unique column: whole rows are the key, a change is a delete plus an add.
    let dup: &[&[&str]] = &[&["a"], &["a"], &["b"], &["b"]];
    let changed: &[&[&str]] = &[&["a"], &["c"], &["b"], &["b"]];
    let m = merge(changed, dup, dup, &TableMergeOptions::default());
    assert_eq!(m.keys.source, KeySource::AllColumns);
    assert!(m.rows.iter().any(|r| r.kind == RowKind::LeftAdded));
    assert!(m.rows.iter().any(|r| r.kind == RowKind::Deleted));
    assert!(m.conflicts().is_empty());
    assert_eq!(texts(&m.output_sheet().unwrap()), rows(changed));
}

#[test]
fn explicit_key_out_of_range_is_an_error() {
    let options = TableMergeOptions {
        keys: Some(vec![98]),
        ..Default::default()
    };
    let error = SheetMerge::new(
        "s",
        [Some(sheet(BASE)), Some(sheet(BASE)), Some(sheet(BASE))],
        &options,
    )
    .unwrap_err();
    assert!(error.to_string().contains("key column 99"), "{error}");
}

#[test]
fn duplicate_keys_pair_by_occurrence() {
    let base: &[&[&str]] = &[&["1", "a"], &["1", "b"]];
    let left: &[&[&str]] = &[&["1", "a"], &["1", "B"]];
    let m = merge(left, base, base, &keyed());
    assert_eq!(m.duplicates, [1, 1, 1]);
    assert_eq!(m.choice(1, 1), CellChoice::Left);
}

#[test]
fn appended_column_on_one_side_merges() {
    let left: &[&[&str]] = &[
        &["1", "apple", "10", "red"],
        &["2", "pear", "20", "green"],
        &["3", "plum", "30", "blue"],
    ];
    let m = merge(left, BASE, BASE, &keyed());
    assert_eq!(m.columns, 4);
    assert!(m.conflicts().is_empty());
    assert_eq!(texts(&m.output_sheet().unwrap()), rows(left));
}

#[test]
fn workbook_sheets_pair_by_name() {
    let named = |name: &str, rows: &[&[&str]]| {
        let mut s = sheet(rows);
        s.name = name.into();
        s
    };
    let changed: &[&[&str]] = &[
        &["1", "apple", "11"],
        &["2", "pear", "20"],
        &["3", "plum", "30"],
    ];
    let inputs = MergeInputs {
        left: vec![named("A", BASE), named("B", BASE)],
        center: vec![named("A", BASE), named("B", BASE), named("C", BASE)],
        right: vec![named("A", BASE), named("C", BASE), named("New", BASE)],
    };
    let merge = TableMerge::new(inputs, &keyed()).unwrap();
    // B deleted on the right and unchanged on the left; C deleted on the left and
    // unchanged on the right: both are dropped. New is appended.
    let names: Vec<_> = merge.output().into_iter().map(|s| s.name).collect();
    assert_eq!(names, ["A", "New"]);
    assert_eq!(merge.unresolved(), 0);

    let inputs = MergeInputs {
        left: vec![named("A", BASE)],
        center: vec![named("A", BASE), named("B", BASE)],
        right: vec![named("A", BASE), named("B", changed)],
    };
    let mut merge = TableMerge::new(inputs, &keyed()).unwrap();
    let outcome = auto_merge(&merge);
    assert!(outcome.withheld);
    assert!(outcome.conflicts[0].1.sheet);
    // Unresolved: the center version is kept.
    assert_eq!(texts(&merge.output()[1]), rows(BASE));
    merge.sheets[1].take(0..0, Side::Right);
    assert_eq!(texts(&merge.output()[1]), rows(changed));
    merge.sheets[1].take(0..0, Side::Left);
    assert_eq!(merge.output().len(), 1);
}

fn csv(text: &str) -> Sheet {
    let options = ReadOptions {
        header: Some(HeaderMode::Names),
        ..Default::default()
    };
    read_text(
        std::io::Cursor::new(text.as_bytes().to_vec()),
        &options,
        &NeverCancel,
    )
    .unwrap()
    .sheet
}

fn written(sheet: &Sheet) -> String {
    let mut out = Vec::new();
    crate::source::write_delimited(sheet, &mut out).unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn csv_output_keeps_untouched_spelling_and_line_endings() {
    let center = csv("id,name,price\r\n1,apple,1.50\r\n2,pear,2.00\r\n3,\"plum, red\",3.00\r\n");
    let left = csv("id,name,price\r\n1,apple,1.50\r\n2,pear,2.50\r\n3,\"plum, red\",3.00\r\n");
    let right = csv(
        "id,name,price\r\n1,Apple,1.50\r\n2,pear,2.00\r\n3,\"plum, red\",3.00\r\n4,fig,4.00\r\n",
    );
    let m = SheetMerge::new("s", [Some(left), Some(center), Some(right)], &keyed()).unwrap();
    assert!(m.conflicts().is_empty());
    assert_eq!(
        written(&m.output_sheet().unwrap()),
        "id,name,price\r\n1,Apple,1.50\r\n2,pear,2.50\r\n3,\"plum, red\",3.00\r\n4,fig,4.00\r\n"
    );
}

#[test]
fn xlsx_round_trip_keeps_values_formulas_and_formats() {
    let mut s = Sheet::new("Data");
    let pct = s.intern_style(crate::Style {
        number_format: Some("0.00%".into()),
    });
    s.push_row(vec![
        Cell::text("id"),
        Cell::text("ratio"),
        Cell::text("sum"),
    ]);
    s.push_row(vec![
        Cell::number(1.0),
        Cell {
            value: CellValue::Number(0.25),
            formula: None,
            style: Some(pct),
        },
        Cell {
            value: CellValue::Number(3.0),
            formula: Some("=1+2".into()),
            style: None,
        },
    ]);
    s.push_row(vec![
        Cell::number(2.0),
        Cell::new(CellValue::Bool(true)),
        Cell::new(CellValue::Date(0)),
    ]);
    let bytes = crate::xlsx_write::workbook_bytes(&[s], None).unwrap();
    let mut book = crate::workbook::Workbook::from_reader(
        std::io::Cursor::new(bytes),
        std::sync::Arc::new(NeverCancel),
    )
    .unwrap();
    assert_eq!(book.sheet_names(), ["Data"]);
    let back = book.load_sheet("Data").unwrap();
    assert_eq!(back.cell(1, 1).unwrap().value, CellValue::Number(0.25));
    assert_eq!(
        back.style(back.cell(1, 1).unwrap().style)
            .and_then(|s| s.number_format.as_deref()),
        Some("0.00%")
    );
    assert_eq!(back.cell(1, 2).unwrap().formula.as_deref(), Some("=1+2"));
    assert_eq!(back.cell(1, 2).unwrap().value, CellValue::Number(3.0));
    assert_eq!(back.cell(2, 1).unwrap().value, CellValue::Bool(true));
    assert_eq!(back.cell(2, 2).unwrap().value, CellValue::Date(0));
}

/// `cargo test --release -p bc-engine-table --lib merge_scale -- --ignored --nocapture`
#[test]
#[ignore]
fn merge_scale_100k_rows() {
    let rows = 100_000;
    let build = |change: usize, salt: &str| {
        let mut s = Sheet::new("big");
        for r in 0..rows {
            let mut cells = vec![Cell::number(r as f64)];
            for c in 1..10 {
                cells.push(if r % change == 0 && c == 3 {
                    Cell::text(format!("{salt}{r}"))
                } else {
                    Cell::text(format!("v{r}-{c}"))
                });
            }
            s.push_row(cells);
        }
        s
    };
    let (center, left, right) = (build(usize::MAX, ""), build(97, "L"), build(101, "R"));
    let t = std::time::Instant::now();
    let m = SheetMerge::new(
        "big",
        [Some(left), Some(center), Some(right)],
        &TableMergeOptions::default(),
    )
    .unwrap();
    let classify = t.elapsed();
    let conflicts = m.conflicts().len();
    let t2 = std::time::Instant::now();
    let out = m.output_sheet().unwrap();
    eprintln!(
        "100k x 10: merge {classify:?}, conflicts {conflicts} ({:?}), output {:?}, keys {}",
        t2.duration_since(t + classify),
        t2.elapsed(),
        m.keys.label()
    );
    assert_eq!(out.row_count(), rows);
}
