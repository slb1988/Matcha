use bc_core::{align::AlignStrategy, cancel::NeverCancel, edit::EditStack, DiffKind};
use bc_engine_table::{
    columns::{ColumnMapping, Side},
    compare::Importance,
    jobs::{TableAlignJob, TableRealignJob},
    map::ShowRows,
    rules::{ColumnType, KeySelection},
    source::{read_text, ReadOptions},
    summary::SummaryFormat,
    Cell, Sheet, TableMap, TableRules,
};
use std::{io::Cursor, sync::Arc};
fn csv(text: &str) -> Sheet {
    read_text(Cursor::new(text), &ReadOptions::default(), &NeverCancel)
        .unwrap()
        .sheet
}
fn settings() -> TableRules {
    TableRules {
        columns: ColumnMapping::by_number(3, 3),
        keys: KeySelection::Manual(vec![0]),
        strategy: AlignStrategy::Quick,
        ..Default::default()
    }
}
fn map() -> TableMap {
    TableMap::new(
        csv("id,name,price\n7,apple,1.0\n8,pear,2.0\n"),
        csv("id,name,price\n7,apple,1.2\n8,PEAR,2.1\n"),
        settings(),
        &NeverCancel,
    )
    .unwrap()
}
#[test]
fn navigation_filters_statistics_and_copy_undo_redo() {
    let m = map();
    assert_eq!(m.next_difference_cell(None), Some((0, 2)));
    assert_eq!(m.next_difference_cell(Some((0, 2))), Some((1, 1)));
    assert_eq!(m.previous_difference_cell(None), Some((1, 2)));
    assert_eq!(m.previous_difference_cell(Some((0, 2))), None);
    assert_eq!(m.next_difference_row(Some(0)), Some(1));
    assert_eq!(m.previous_difference_row(Some(1)), Some(0));
    assert_eq!(m.goto(5, 2), None);
    assert!(m.visible_rows(ShowRows::None).is_empty());
    let mut edits = EditStack::new();
    edits.push(m.copy_cell(0, 2, Side::Left).unwrap());
    assert_eq!(m.cell_importance(0, 2), Some(Importance::Same));
    assert_eq!(m.next_edit(None), Some(0));
    assert!(!m.is_stale());
    edits.undo();
    assert!(!m.is_stale());
    assert_eq!(m.cell_importance(0, 2), Some(Importance::Important));
    assert_eq!(m.next_edit(None), None);
    edits.redo();
    assert!(!m.is_stale());
    assert_eq!(m.cell_importance(0, 2), Some(Importance::Same));
    edits.push(m.copy_row(1, Side::Left).unwrap());
    assert_eq!(m.statistics().same, 2);
    assert!(m.visible_rows(ShowRows::Differences).is_empty());
    assert_eq!(m.previous_edit(None), Some(1));
}
#[test]
fn copying_orphan_inserts_in_display_position_and_preserves_save_and_undo() {
    let l = csv("id,name,price\n1,a,1.0\n2,b,2.0\n3,c,3.0\n");
    let r = csv("id,name,price\n1,a,1.0\n3,c,3.0");
    let m = TableMap::new(l, r, settings(), &NeverCancel).unwrap();
    assert_eq!(m.row(1).unwrap().kind, DiffKind::LeftOnly);
    let mut edits = EditStack::new();
    edits.push(m.copy_row(1, Side::Left).unwrap());
    assert_eq!(m.statistics().same, 3);
    let mut out = Vec::new();
    bc_engine_table::store::write_store_delimited(&m.right.borrow(), &mut out).unwrap();
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "id,name,price\n1,a,1.0\n2,b,2.0\n3,c,3.0"
    );
    edits.undo();
    assert!(!m.is_stale());
    assert_eq!(m.statistics().left_only, 1);
    edits.redo();
    assert!(!m.is_stale());
    assert_eq!(m.statistics().same, 3);
}
#[test]
fn key_edit_realigns_and_stale_worker_results_are_rejected() {
    let m = TableMap::new(
        csv("id,name,price\n7,a,1.0\n"),
        csv("id,name,price\n8,a,1.0\n"),
        settings(),
        &NeverCancel,
    )
    .unwrap();
    assert_eq!(m.row_count(), 2);
    let old = TableAlignJob::from_map(&m).execute(&NeverCancel).unwrap();
    let mut edits = EditStack::new();
    edits.push(m.modify_cell(Side::Right, 0, 0, Cell::number(7.0)).unwrap());
    assert_eq!(m.row_count(), 1);
    assert_eq!(m.statistics().same, 1);
    assert!(m.set_alignment(old).is_err());
    let old = TableAlignJob::from_map(&m).execute(&NeverCancel).unwrap();
    edits.undo();
    assert_eq!(m.row_count(), 2);
    edits.redo();
    assert!(m.set_alignment(old).is_err()); // no revision ABA
    let job = TableRealignJob::from_map(&m);
    let out = std::thread::spawn(move || job.execute(&NeverCancel))
        .join()
        .unwrap()
        .unwrap();
    m.set_alignment(out).unwrap();
    let job = TableAlignJob::from_map(&m);
    m.rules_mut().default_handling.ignore_case = true;
    assert!(m.set_alignment(job.execute(&NeverCancel).unwrap()).is_err());
}
#[test]
fn initial_job_resolves_defaults_and_installs_without_synchronous_recompare() {
    let left = csv("id,name,price\n7,a,1.0\n8,b,2.0\n");
    let right = csv("id,name,price\n7,a,1.1\n8,b,2.0\n");
    let job = TableAlignJob {
        left: bc_engine_table::SheetStore::Dense(left.clone()),
        right: bc_engine_table::SheetStore::Dense(right.clone()),
        rules: TableRules::default(),
        overrides: Default::default(),
    };
    let output = std::thread::spawn(move || job.execute(&NeverCancel))
        .join()
        .unwrap()
        .unwrap();
    assert_eq!(output.rules.keys.label(), "自动");
    let m = TableMap::from_output(left.clone(), right.clone(), output.clone()).unwrap();
    assert_eq!(m.statistics().important, 1);
    let mut changed = right;
    changed.set_cell(0, 0, Cell::number(99.0)).unwrap();
    assert!(TableMap::from_output(left, changed, output).is_err());
}
#[test]
fn jobs_obey_job_trait_and_cancel_contract() {
    struct Sink;
    impl bc_jobs::ProgressSink for Sink {
        fn report(&self, _: bc_jobs::JobId, _: &bc_jobs::Progress) {}
    }
    let m = map();
    let context = bc_jobs::JobContext {
        id: bc_jobs::JobId(1),
        cancel: bc_jobs::CancelToken::new(),
        progress: Arc::new(Sink),
        interaction: None,
    };
    let output = bc_jobs::Job::run(Box::new(TableAlignJob::from_map(&m)), &context).unwrap();
    m.set_alignment(
        *output
            .downcast::<bc_engine_table::map::TableAlignOutput>()
            .unwrap(),
    )
    .unwrap();
    context.cancel.cancel();
    assert!(matches!(
        bc_jobs::Job::run(Box::new(TableRealignJob::from_map(&m)), &context),
        Err(bc_jobs::JobError::Cancelled)
    ));
}
#[test]
fn summary_protocol_has_key_and_lexical_old_new_values_and_projection() {
    let m = TableMap::new(
        csv("id,name,price\n7,apple,1.0\n"),
        csv("id,name,price\n7,apple,1.2\n"),
        settings(),
        &NeverCancel,
    )
    .unwrap();
    let text = m.export_summary(SummaryFormat::Text).unwrap();
    assert!(text.contains("7 | price | 1.0 -> 1.2\n"));
    assert_eq!(text.lines().count(), 3);
    let csv = m.export_summary(SummaryFormat::Csv).unwrap();
    assert!(csv.contains("cell,7,Changed,price,value,1.0,1.2\r\n"));
    {
        let mut r = m.rules_mut();
        r.columns.0[2].handling.use_default = false;
        r.columns.0[2].handling.unimportant = true;
    }
    m.recompare(&NeverCancel).unwrap();
    m.rules_mut().ignore_unimportant = true;
    assert_eq!(
        m.export_summary(SummaryFormat::Text).unwrap(),
        "MATCHA-TABLE-DIFF\t1\n"
    );
    assert_eq!(m.statistics().same, 1);
    assert_eq!(m.cell_diff(0, 2).unwrap().value, Importance::Unimportant);
}
#[test]
fn text_type_keeps_numeric_spelling_in_comparison_and_key_hashes() {
    let mut r = settings();
    r.columns.0[2].handling.use_default = false;
    r.columns.0[2].handling.column_type = ColumnType::Text;
    let m = TableMap::new(
        csv("id,name,price\n7,a,1.0\n"),
        csv("id,name,price\n7,a,1.00\n"),
        r,
        &NeverCancel,
    )
    .unwrap();
    assert_eq!(m.cell_diff(0, 2).unwrap().value, Importance::Important);
    let _edit = m.copy_cell(0, 2, Side::Left).unwrap();
    assert_eq!(m.cell_diff(0, 2).unwrap().value, Importance::Same);
    let mut out = Vec::new();
    bc_engine_table::store::write_store_delimited(&m.right.borrow(), &mut out).unwrap();
    assert_eq!(String::from_utf8(out).unwrap(), "id,name,price\n7,a,1.0\n");
}
