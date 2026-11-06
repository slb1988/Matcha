use bc_core::{cancel::NeverCancel, edit::EditStack};
use bc_engine_table::{
    columns::Side,
    jobs::TableRealignJob,
    rules::{KeySelection, Layers},
    source::{read_text, write_delimited, ReadOptions},
    Cell, TableMap, TableRules,
};
use std::io::Cursor;
fn sheet(text: &str) -> bc_engine_table::Sheet {
    read_text(Cursor::new(text), &ReadOptions::default(), &NeverCancel)
        .unwrap()
        .sheet
}
#[test]
fn deferred_modify_stays_stale_until_matching_job_and_undo_preserves_projection() {
    let mut m = TableMap::new(
        sheet("id,n\n1,a\n"),
        sheet("id,n\n1,b\n"),
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            ..Default::default()
        },
        &NeverCancel,
    )
    .unwrap();
    m.set_deferred_edits(true);
    let mut history = EditStack::new();
    history.push(m.modify_cell(Side::Right, 0, 1, Cell::text("a")).unwrap());
    assert!(m.is_stale());
    let old = TableRealignJob::from_map(&m).execute(&NeverCancel).unwrap();
    history.push(m.modify_cell(Side::Right, 0, 1, Cell::text("c")).unwrap());
    assert!(m.set_alignment(old).is_err());
    history.undo();
    assert!(m.is_stale());
    m.set_alignment(TableRealignJob::from_map(&m).execute(&NeverCancel).unwrap())
        .unwrap();
    assert_eq!(m.statistics().same, 1);
    m.rules_mut().layers = Layers::All;
    m.rules_mut().ignore_unimportant = true;
    history.undo();
    assert_eq!(m.rules().layers, Layers::All);
    assert!(m.rules().ignore_unimportant);
    history.redo();
    assert!(m.is_stale());
}
#[test]
fn structural_rows_columns_keep_values_and_roundtrip() {
    let mut s = sheet("id,n,price\r\n1,\"a\",1.00\n2,b,2.00");
    s.insert_column(1, "new".into()).unwrap();
    assert_eq!(s.display_at(0, 3).unwrap(), "1.00");
    s.set_cell(0, 1, Cell::text("x")).unwrap();
    s.remove_column(2).unwrap();
    assert_eq!(s.display_at(0, 2).unwrap(), "1.00");
    s.reorder_rows(&[1, 0]).unwrap();
    assert_eq!(s.display_at(0, 2).unwrap(), "2.00");
    let mut out = Vec::new();
    write_delimited(&s, &mut out).unwrap();
    let parsed = sheet(std::str::from_utf8(&out).unwrap());
    assert_eq!(parsed.row_count(), 2);
    assert_eq!(parsed.display_at(0, 2).unwrap(), "2.00");
    s.remove_rows(0..1).unwrap();
    assert_eq!(s.row_count(), 1);
    assert_eq!(s.display_at(0, 2).unwrap(), "1.00");
}
