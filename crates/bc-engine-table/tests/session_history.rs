//! 对抗回归：Swap Sides / replace_sheet 与编辑历史的组合边界。
//! 只使用公开 API，只断言用户可见规格（摘要方向、源值、规则/锚点、统计与
//! 对当前两侧全新比较的一致性），不发明语义；失败即为可复现的审查发现。
use bc_core::{cancel::NeverCancel, edit::EditStack};
use bc_engine_table::{
    columns::{ColumnMapping, ComparisonColumn, Side},
    jobs::TableRealignJob,
    rules::KeySelection,
    source::{read_text, ReadOptions},
    summary::SummaryFormat,
    Cell, Sheet, TableMap, TableRules,
};
use std::io::Cursor;

fn sheet(text: &str) -> Sheet {
    read_text(Cursor::new(text), &ReadOptions::default(), &NeverCancel)
        .unwrap()
        .sheet
}

fn mapping(m: &TableMap) -> Vec<(Option<usize>, Option<usize>)> {
    m.rules()
        .columns
        .0
        .iter()
        .map(|c| (c.left, c.right))
        .collect()
}

fn realign_if_stale(m: &TableMap) {
    if m.is_stale() {
        m.set_alignment(TableRealignJob::from_map(m).execute(&NeverCancel).unwrap())
            .unwrap();
    }
}

/// 对当前两侧内容与当前规则/锚点做全新比较，作为统计口径的 ground truth。
fn fresh_statistics(m: &TableMap) -> bc_engine_table::map::TableStatistics {
    let fresh = TableMap::new_stores(
        m.left.borrow().clone(),
        m.right.borrow().clone(),
        m.rules().clone(),
        &NeverCancel,
    )
    .unwrap();
    fresh.set_overrides(m.overrides());
    fresh.recompare(&NeverCancel).unwrap();
    fresh.statistics()
}

/// (1) 非对称列数产生的 Deleted 方向注释：Swap 后摘要方向必须与
/// 对当前两侧全新比较一致（左右已交换，右多列应是 Inserted 而非残留 Deleted）。
#[test]
fn swap_flips_directional_column_annotations_in_summary() {
    let left = "id,a,b\n1,x,1\n2,y,2\n";
    let right = "id,b\n1,1\n2,2\n";
    let rules = || TableRules {
        keys: KeySelection::Manual(vec![0]),
        ..Default::default()
    };
    let mut m = TableMap::new(sheet(left), sheet(right), rules(), &NeverCancel).unwrap();
    assert_eq!(
        mapping(&m),
        vec![(Some(0), Some(0)), (Some(1), Some(1)), (Some(2), None)]
    );
    let before = m.export_summary(SummaryFormat::Text).unwrap();
    assert!(
        before.contains("Deleted"),
        "pre-swap summary should mark the right-missing column Deleted: {before}"
    );

    m.swap_sides();
    assert_eq!(
        mapping(&m),
        vec![(Some(0), Some(0)), (Some(1), Some(1)), (None, Some(2))],
        "swap must exchange left/right column sources"
    );
    let swapped = m.export_summary(SummaryFormat::Text).unwrap();
    // Ground truth：对交换后的两侧按相同规则全新比较。注意 Swap 保持比较列顺序而
    // 自动配列可能按 left_source 换序，因此按比较列逐项核对方向与旧值→新值、
    // 行记录按集合核对，不强求整篇字节序一致。
    let fresh = TableMap::new(sheet(right), sheet(left), rules(), &NeverCancel).unwrap();
    let reference = fresh.export_summary(SummaryFormat::Text).unwrap();
    assert!(
        reference.contains("Inserted"),
        "fresh comparison of swapped sides marks the extra column Inserted: {reference}"
    );
    fn column_lines(s: &str) -> Vec<&str> {
        s.lines().filter(|l| l.starts_with("COLUMN")).collect()
    }
    fn row_lines(s: &str) -> Vec<&str> {
        let mut v = s
            .lines()
            .filter(|l| !l.starts_with("COLUMN") && !l.starts_with("MATCHA-TABLE-DIFF"))
            .collect::<Vec<_>>();
        v.sort_unstable();
        v
    }
    assert_eq!(
        column_lines(&swapped),
        column_lines(&reference),
        "per-column structural annotations (Inserted/Deleted direction, old->new) must follow the swap"
    );
    assert_eq!(
        row_lines(&swapped),
        row_lines(&reference),
        "row records after Swap must match a fresh comparison of the current sides"
    );
    assert_eq!(m.statistics(), fresh.statistics());
}

/// (2) 已应用的结构编辑（右側插入列）跨一次 Swap 撤销、再跨第二次 Swap 重做：
/// 源值、规则方向、手动锚点与统计都必须正确。
#[test]
fn structural_edit_undo_redo_across_swaps_keeps_rules_anchors_statistics() {
    // 非对称映射 + 非对称锚点，方向错误一定能被观察到。
    let left = "id,name,price\n1,a,1.0\n2,b,2.0\n";
    let right = "id,price\n9,1.0\n2,2.0\n";
    let mut m = TableMap::new(
        sheet(left),
        sheet(right),
        TableRules {
            columns: ColumnMapping(vec![
                ComparisonColumn::new(Some(0), Some(0)),
                ComparisonColumn::new(Some(1), None),
                ComparisonColumn::new(Some(2), Some(1)),
            ]),
            keys: KeySelection::Manual(vec![0]),
            ..Default::default()
        },
        &NeverCancel,
    )
    .unwrap();
    m.set_deferred_edits(true);
    let mut history = EditStack::new();
    // 手动锚点：左行 0（id=1）强配右行 1（id=2），键不同但允许。
    history.push(
        m.edit_sheet(Side::Left, "Align With", |_, _, anchors| {
            anchors.insert(0, 1)?;
            Ok(())
        })
        .unwrap(),
    );
    realign_if_stale(&m);
    // 结构编辑：右側第 0 列前插入 note 列，规则同步右移。
    history.push(
        m.edit_sheet(Side::Right, "Insert column", |sheet, rules, _| {
            sheet.insert_column(0, "note".into())?;
            for c in &mut rules.columns.0 {
                if let Some(r) = &mut c.right {
                    *r += 1;
                }
            }
            rules
                .columns
                .0
                .insert(0, ComparisonColumn::new(None, Some(0)));
            Ok(())
        })
        .unwrap(),
    );
    realign_if_stale(&m);
    assert_eq!(m.right.borrow().column_count(), 3);
    assert_eq!(
        mapping(&m),
        vec![
            (None, Some(0)),
            (Some(0), Some(1)),
            (Some(1), None),
            (Some(2), Some(2))
        ]
    );
    let stats_after_insert = m.statistics();

    // 跨一次 Swap 撤销插入。
    m.swap_sides();
    assert!(!m.is_stale(), "swap of an aligned map stays aligned");
    assert!(history.undo().is_some());
    realign_if_stale(&m);
    assert_eq!(
        mapping(&m),
        vec![(Some(0), Some(0)), (None, Some(1)), (Some(1), Some(2))],
        "undo across swap must restore rules transposed into the current orientation"
    );
    assert_eq!(
        m.overrides().pairs(),
        &[(1, 0)],
        "anchor must be transposed by the swap and survive the undo"
    );
    assert_eq!(m.left.borrow().column_count(), 2, "inserted column removed");
    assert_eq!(m.left.borrow().column_name(0), "id");
    assert_eq!(m.right.borrow().display_at(0, 1).unwrap(), "a");
    assert_eq!(
        m.statistics(),
        fresh_statistics(&m),
        "statistics after cross-swap undo must equal a fresh comparison"
    );

    // 再 Swap（回到原方向）后重做插入。
    m.swap_sides();
    assert!(history.redo().is_some());
    realign_if_stale(&m);
    assert_eq!(
        mapping(&m),
        vec![
            (None, Some(0)),
            (Some(0), Some(1)),
            (Some(1), None),
            (Some(2), Some(2))
        ]
    );
    assert_eq!(m.overrides().pairs(), &[(0, 1)]);
    assert_eq!(m.right.borrow().column_count(), 3);
    assert_eq!(m.right.borrow().column_name(0), "note");
    assert_eq!(m.statistics(), stats_after_insert);
    assert_eq!(m.statistics(), fresh_statistics(&m));
}

/// (3) deferred 编辑使 map 处于 stale 时 Swap： Swap 前完成的对齐结果必须被拒绝，
/// 之后按当前方向重对齐与撤销都不能卡死或接受错误快照。
#[test]
fn swap_while_stale_rejects_preswap_snapshot_and_realigns() {
    let left = "id,name,price\n7,apple,1.0\n8,pear,2.0\n";
    let right = "id,price,name\n7,1.0,apple\n8,2.0,PEAR\n";
    let mut m = TableMap::new(
        sheet(left),
        sheet(right),
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
    .unwrap();
    m.set_deferred_edits(true);
    let mut history = EditStack::new();
    history.push(
        m.modify_cell(Side::Right, 1, 2, Cell::text("pear"))
            .unwrap(),
    );
    assert!(m.is_stale(), "deferred edit leaves the map stale");
    // 竞态：worker 在 Swap 前完成的重对齐结果，Swap 后才送达。
    let preswap = TableRealignJob::from_map(&m).execute(&NeverCancel).unwrap();
    m.swap_sides();
    assert!(m.is_stale(), "transposed stale state must stay stale");
    let rejected = m.set_alignment(preswap);
    assert!(
        rejected.is_err(),
        "a snapshot computed in the pre-swap orientation must be rejected after Swap"
    );
    assert!(m.is_stale(), "rejected snapshot must not be installed");
    // 按当前方向重对齐并撤销。
    m.set_alignment(TableRealignJob::from_map(&m).execute(&NeverCancel).unwrap())
        .unwrap();
    assert!(!m.is_stale());
    assert_eq!(m.statistics().same, 2);
    assert!(history.undo().is_some());
    realign_if_stale(&m);
    assert_eq!(m.left.borrow().display_at(1, 2).unwrap(), "PEAR");
    assert_eq!(m.statistics().same, 1);
    assert_eq!(m.statistics(), fresh_statistics(&m));
}

/// (4a) replace_sheet 换成不同列布局后，撤销对侧旧编辑不得把已替换侧的
/// 旧锚点写回新内容；重对齐应成功且与全新比较一致。
#[test]
fn replace_sheet_then_other_side_undo_must_not_restore_stale_anchors() {
    let left = "id,name,price\n7,apple,1.0\n8,pear,2.0\n";
    let right = "id,name,price\n7,apple,1.2\n8,PEAR,2.1\n";
    let rules = || TableRules {
        keys: KeySelection::Manual(vec![0]),
        ..Default::default()
    };
    let mut m = TableMap::new(sheet(left), sheet(right), rules(), &NeverCancel).unwrap();
    m.set_deferred_edits(true);
    // 每一侧独立历史（GUI 结构）；锚点编辑属于被替换侧，且指向替换后不存在的左行 1。
    let mut left_history = EditStack::new();
    let mut right_history = EditStack::new();
    left_history.push(
        m.edit_sheet(Side::Left, "Align With", |_, _, anchors| {
            anchors.insert(1, 0)?;
            Ok(())
        })
        .unwrap(),
    );
    right_history.push(m.modify_cell(Side::Right, 0, 2, Cell::text("9.9")).unwrap());
    realign_if_stale(&m);

    // 换成只有 1 行的新左侧内容；调用方按约定清空被替换侧历史。
    m.replace_sheet(Side::Left, sheet("id,name,price\n7,apple,9.9\n"));
    left_history.clear();
    assert!(m.is_stale());

    // 撤销对侧（右侧）的单元格编辑：其快照包含针对旧左侧行 1 的锚点。
    assert!(right_history.undo().is_some());
    assert_eq!(
        m.right.borrow().display_at(0, 2).unwrap(),
        "1.2",
        "undo restores the other side's own content"
    );
    assert_eq!(
        m.left.borrow().display_at(0, 2).unwrap(),
        "9.9",
        "replaced side's new content must not be touched by the undo"
    );
    assert!(
        m.overrides().pairs().is_empty(),
        "anchors addressing replaced content must not be written back: {:?}",
        m.overrides().pairs()
    );
    let realigned = TableRealignJob::from_map(&m).execute(&NeverCancel);
    assert!(
        realigned.is_ok(),
        "recompare after cross-content undo must not fail on anchors of replaced content: {:?}",
        realigned.err()
    );
    m.set_alignment(realigned.unwrap()).unwrap();
    // Ground truth：当前内容、当前规则、无幽灵锚点的全新比较。
    let fresh = TableMap::new_stores(
        m.left.borrow().clone(),
        m.right.borrow().clone(),
        m.rules().clone(),
        &NeverCancel,
    )
    .unwrap();
    assert_eq!(
        m.statistics(),
        fresh.statistics(),
        "comparison must reflect current contents, not anchors into replaced rows"
    );
}

/// (4b) replace_sheet 换成列数更少的新内容：即使不撤销任何编辑，
/// 重对齐也不应因旧列映射越界而永久失败（调用方已按约定清空被替换侧历史）。
#[test]
fn replace_sheet_with_fewer_columns_must_stay_recomparable() {
    let left = "id,name,price\n7,apple,1.0\n8,pear,2.0\n";
    let right = "id,name,price\n7,apple,1.2\n8,PEAR,2.1\n";
    let mut m = TableMap::new(
        sheet(left),
        sheet(right),
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            ..Default::default()
        },
        &NeverCancel,
    )
    .unwrap();
    m.set_deferred_edits(true);
    let mut right_history = EditStack::new();
    right_history.push(m.modify_cell(Side::Right, 0, 2, Cell::text("9.9")).unwrap());
    realign_if_stale(&m);

    m.replace_sheet(Side::Left, sheet("id,note\n7,apple\n"));
    let realigned = TableRealignJob::from_map(&m).execute(&NeverCancel);
    assert!(
        realigned.is_ok(),
        "recompare after replacing a side with fewer columns must not fail on the old \
         column mapping: {:?}",
        realigned.err()
    );
    m.set_alignment(realigned.unwrap()).unwrap();
    // 对侧编辑仍应可撤销，且结果与全新比较一致。
    assert!(right_history.undo().is_some());
    realign_if_stale(&m);
    assert_eq!(m.right.borrow().display_at(0, 2).unwrap(), "1.2");
    assert_eq!(m.left.borrow().display_at(0, 1).unwrap(), "apple");
    assert_eq!(m.statistics(), fresh_statistics(&m));
}

/// 评审附带：rules.validate 必须拒绝非法默认 Handling（本轮 diff 删除了
/// default_handling.validate() 调用，默认负容差会静默通过并扩散到所有
/// Use Default 列）。
#[test]
fn validate_rejects_invalid_default_handling() {
    let rules = TableRules {
        default_handling: bc_engine_table::ColumnHandling {
            numeric_tolerance: -1.0,
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        rules.validate(1, 1).is_err(),
        "negative default numeric tolerance must be rejected"
    );
}
