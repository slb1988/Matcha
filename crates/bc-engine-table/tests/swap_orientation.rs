//! Swap Sides keeps the map aligned by mirroring the installed snapshot, and
//! captured edits transpose their snapshots across the swap so undo/redo never
//! restore the pre-swap orientation onto exchanged sheets.
use bc_core::{cancel::NeverCancel, edit::EditStack};
use bc_engine_table::{
    columns::{ColumnMapping, ComparisonColumn, Side},
    jobs::TableRealignJob,
    rules::KeySelection,
    source::{read_text, ReadOptions},
    Cell, TableMap, TableRules,
};
use std::io::Cursor;

fn sheet(text: &str) -> bc_engine_table::Sheet {
    read_text(Cursor::new(text), &ReadOptions::default(), &NeverCancel)
        .unwrap()
        .sheet
}

fn shifted_map() -> TableMap {
    TableMap::new(
        sheet("id,name,price\n7,apple,1.0\n8,pear,2.0\n"),
        sheet("id,price,name\n7,1.0,apple\n8,2.0,PEAR\n"),
        TableRules {
            columns: ColumnMapping(vec![
                ComparisonColumn::new(Some(0), Some(0)),
                ComparisonColumn::new(Some(1), Some(2)),
                ComparisonColumn::new(Some(2), Some(1)),
            ]),
            keys: KeySelection::Manual(vec![0]),
            ..Default::default()
        },
        &NeverCancel,
    )
    .unwrap()
}

fn mapping(m: &TableMap) -> Vec<(Option<usize>, Option<usize>)> {
    m.rules()
        .columns
        .0
        .iter()
        .map(|c| (c.left, c.right))
        .collect()
}

#[test]
fn swap_mirrors_statistics_without_recompare_and_undo_transposes_snapshots() {
    let mut m = shifted_map();
    m.set_deferred_edits(true);
    let mut history = EditStack::new();
    // 编辑右侧 name（源列 2）使两行相同。
    history.push(
        m.modify_cell(Side::Right, 1, 2, Cell::text("pear"))
            .unwrap(),
    );
    m.set_alignment(TableRealignJob::from_map(&m).execute(&NeverCancel).unwrap())
        .unwrap();
    assert_eq!(m.statistics().same, 2);
    m.swap_sides();
    assert!(!m.is_stale(), "mirrored snapshot stays aligned");
    assert_eq!(
        mapping(&m),
        vec![(Some(0), Some(0)), (Some(2), Some(1)), (Some(1), Some(2))]
    );
    assert_eq!(m.statistics().same, 2);
    assert_eq!(m.left.borrow().display_at(1, 2).unwrap(), "pear");
    // Undo across the swap: content returns, rules stay in current orientation.
    history.undo();
    assert_eq!(
        mapping(&m),
        vec![(Some(0), Some(0)), (Some(2), Some(1)), (Some(1), Some(2))]
    );
    assert!(m.is_stale() || m.statistics().same == 1);
    m.set_alignment(TableRealignJob::from_map(&m).execute(&NeverCancel).unwrap())
        .unwrap();
    assert_eq!(m.statistics().same, 1);
    assert_eq!(m.left.borrow().display_at(1, 2).unwrap(), "PEAR");
    history.redo();
    m.set_alignment(TableRealignJob::from_map(&m).execute(&NeverCancel).unwrap())
        .unwrap();
    assert_eq!(m.statistics().same, 2);
}

#[test]
fn undo_after_double_swap_restores_original_orientation() {
    let mut m = shifted_map();
    m.set_deferred_edits(true);
    let mut history = EditStack::new();
    history.push(
        m.modify_cell(Side::Right, 1, 2, Cell::text("pear"))
            .unwrap(),
    );
    m.set_alignment(TableRealignJob::from_map(&m).execute(&NeverCancel).unwrap())
        .unwrap();
    m.swap_sides();
    m.swap_sides();
    assert!(!m.is_stale());
    assert_eq!(
        mapping(&m),
        vec![(Some(0), Some(0)), (Some(1), Some(2)), (Some(2), Some(1))]
    );
    history.undo();
    let _ = m.set_alignment(TableRealignJob::from_map(&m).execute(&NeverCancel).unwrap());
    assert_eq!(m.statistics().same, 1);
    assert_eq!(m.right.borrow().display_at(1, 2).unwrap(), "PEAR");
}

#[test]
fn replace_sheet_keeps_the_other_side_object_and_marks_stale() {
    let mut m = shifted_map();
    m.set_deferred_edits(true);
    let mut history = EditStack::new();
    history.push(
        m.modify_cell(Side::Right, 1, 2, Cell::text("pear"))
            .unwrap(),
    );
    m.set_alignment(TableRealignJob::from_map(&m).execute(&NeverCancel).unwrap())
        .unwrap();
    let kept = m.left.clone();
    m.replace_sheet(Side::Left, sheet("id,name,price\n7,apple,9.9\n"));
    assert!(std::rc::Rc::ptr_eq(&m.left, &kept));
    assert!(m.is_stale());
    m.set_alignment(TableRealignJob::from_map(&m).execute(&NeverCancel).unwrap())
        .unwrap();
    // 对侧历史仍然可撤销且作用于同一对象；恢复后内容回到编辑前。
    history.undo();
    assert_eq!(m.right.borrow().display_at(1, 2).unwrap(), "PEAR");
    let _ = m.set_alignment(TableRealignJob::from_map(&m).execute(&NeverCancel).unwrap());
    assert_eq!(m.left.borrow().display_at(0, 2).unwrap(), "9.9");
}

#[test]
fn stale_output_is_rejected_without_touching_unresolved_settings() {
    let mut m = TableMap::new(
        sheet("id,name,price\n7,apple,1.0\n"),
        sheet("id,name,price\n7,apple,1.2\n"),
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            ..Default::default()
        },
        &NeverCancel,
    )
    .unwrap();
    m.set_deferred_edits(true);
    // 旧朝向/旧内容的成功快照（revision 属于被替换前的 sheet）。
    let stale = TableRealignJob::from_map(&m).execute(&NeverCancel).unwrap();
    m.replace_sheet(Side::Left, sheet("id,note\n7,apple\n"));
    assert!(
        m.rules().columns.0.is_empty(),
        "incompatible replacement leaves an unresolved mapping"
    );
    let rules_before = m.rules().clone();
    let revisions_before = m.output().revisions;
    assert!(
        m.set_alignment(stale).is_err(),
        "pre-replacement snapshot must be rejected"
    );
    assert_eq!(
        m.rules().clone(),
        rules_before,
        "rejected output must not touch rules"
    );
    assert!(m.overrides().pairs().is_empty());
    assert_eq!(m.output().revisions, revisions_before, "state untouched");
    // 当前内容重新对齐成功并解析出新映射。
    m.set_alignment(TableRealignJob::from_map(&m).execute(&NeverCancel).unwrap())
        .unwrap();
    assert_eq!(m.rules().columns.0.len(), 3);
    assert_eq!(m.statistics().important, 1);
}

#[test]
fn replace_sheet_with_reordered_headers_rematches_and_undo_keeps_current_rules() {
    let mut m = TableMap::new(
        sheet("id,name,price\n7,apple,1.0\n"),
        sheet("id,name,price\n7,apple,1.2\n"),
        TableRules {
            column_mode: bc_engine_table::rules::ColumnMode::Automatic,
            keys: KeySelection::Manual(vec![0]),
            ..Default::default()
        },
        &NeverCancel,
    )
    .unwrap();
    m.set_deferred_edits(true);
    let mut right_history = EditStack::new();
    right_history.push(m.modify_cell(Side::Right, 0, 2, Cell::text("9.9")).unwrap());
    m.set_alignment(TableRealignJob::from_map(&m).execute(&NeverCancel).unwrap())
        .unwrap();
    // 相同列数但表头重排：Automatic 模式必须按名重新匹配，而不是沿用位置映射。
    m.replace_sheet(Side::Left, sheet("price,id,name\n1.0,7,apple\n"));
    assert!(
        m.rules().columns.0.is_empty(),
        "changed headers force a re-match"
    );
    m.set_alignment(TableRealignJob::from_map(&m).execute(&NeverCancel).unwrap())
        .unwrap();
    assert_eq!(m.statistics().left_only, 0, "key 7 pairs by name");
    assert_eq!(m.statistics().important, 1);
    let current_rules = m.rules().clone();
    // 对侧 Undo 只恢复其单元格内容，不把替换前的映射写回当前会话。
    right_history.undo();
    realign_if_stale_helper(&m);
    assert_eq!(
        m.rules().clone(),
        current_rules,
        "undo must not clobber session rules"
    );
    assert_eq!(m.right.borrow().display_at(0, 2).unwrap(), "1.2");
}

fn realign_if_stale_helper(m: &TableMap) {
    if m.is_stale() {
        m.set_alignment(TableRealignJob::from_map(m).execute(&NeverCancel).unwrap())
            .unwrap();
    }
}
