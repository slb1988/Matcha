//! 4.4 session commands: Swap 与编辑历史/设置/配对的对应、单侧替换、Reload/设置
//! 的提交与失败恢复、保存与导出。Swap 的 Undo 验收使用不对称映射，
//! 使交换前朝向的快照在旧代码上产生可观察的错误。
use super::*;
use crate::chrome::native_dialog;
use bc_core::edit::EditStack;
use bc_engine_table::columns::{ColumnMapping, ComparisonColumn, Side};
use bc_engine_table::compare::Importance;

/// 左 id,name,price；右 id,price,name：比较列 (0↔0, 1↔2, 2↔1) 不对称但无空隙。
fn shifted_view() -> TableCompareView {
    let mut v = TableCompareView::from_sheets(
        sheet("id,name,price\n7,apple,1.0\n8,pear,2.0\n"),
        sheet("id,price,name\n7,1.0,apple\n8,2.0,PEAR\n"),
        TableRules {
            columns: ColumnMapping(vec![
                ComparisonColumn::new(Some(0), Some(0)),
                ComparisonColumn::new(Some(1), Some(2)),
                ComparisonColumn::new(Some(2), Some(1)),
            ]),
            keys: KeySelection::Manual(vec![0]),
            strategy: AlignStrategy::Quick,
            ..Default::default()
        },
    );
    ready(&mut v);
    v
}

fn statistics(v: &TableCompareView) -> bc_engine_table::map::TableStatistics {
    v.pair.as_ref().unwrap().cache.statistics
}

fn mapping(v: &TableCompareView) -> Vec<(Option<usize>, Option<usize>)> {
    v.pair
        .as_ref()
        .unwrap()
        .map
        .rules()
        .columns
        .0
        .iter()
        .map(|c| (c.left, c.right))
        .collect()
}

#[test]
fn swap_transposes_mapping_and_edit_history_so_undo_redo_restore_the_edited_file() {
    let mut v = shifted_view();
    assert_eq!(statistics(&v).same, 1);
    assert_eq!(statistics(&v).important, 1);
    // 编辑右侧 name 单元格（比较列 1 → 右源列 2），行使两行相同。
    v.begin_edit(1, Cursor { row: 1, column: 1 }, Some("pear".into()));
    v.commit_edit().unwrap();
    ready(&mut v);
    assert_eq!(statistics(&v).same, 2);
    assert!(v.pair.as_ref().unwrap().history[1].can_undo());
    // Swap：被编辑的文件换到左侧，映射/历史随之转置。
    v.swap();
    ready(&mut v);
    assert_eq!(
        mapping(&v),
        vec![(Some(0), Some(0)), (Some(2), Some(1)), (Some(1), Some(2))]
    );
    {
        let p = v.pair.as_ref().unwrap();
        assert!(
            p.history[0].can_undo(),
            "edited side history follows the file"
        );
        assert_eq!(p.map.left.borrow().display_at(1, 2).unwrap(), "pear");
        assert_eq!(p.map.right.borrow().display_at(1, 1).unwrap(), "pear");
    }
    assert_eq!(statistics(&v).same, 2);
    // Undo 必须还原内容并按交换后的朝向恢复规则/状态，而不是写回交换前朝向。
    v.grid.active = 0;
    v.undo(false);
    assert_eq!(
        mapping(&v),
        vec![(Some(0), Some(0)), (Some(2), Some(1)), (Some(1), Some(2))],
        "undo must not restore pre-swap orientation rules"
    );
    ready(&mut v);
    assert_eq!(statistics(&v).same, 1);
    assert_eq!(statistics(&v).important, 1);
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .left
            .borrow()
            .display_at(1, 2)
            .unwrap(),
        "PEAR"
    );
    // Redo 同样按当前朝向。
    v.undo(true);
    ready(&mut v);
    assert_eq!(statistics(&v).same, 2);
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .left
            .borrow()
            .display_at(1, 2)
            .unwrap(),
        "pear"
    );
}

#[test]
fn swap_twice_returns_orientation_and_oldest_undo_still_targets_original_side() {
    let mut v = shifted_view();
    v.begin_edit(1, Cursor { row: 1, column: 1 }, Some("pear".into()));
    v.commit_edit().unwrap();
    ready(&mut v);
    v.swap();
    ready(&mut v);
    v.swap();
    ready(&mut v);
    assert_eq!(
        mapping(&v),
        vec![(Some(0), Some(0)), (Some(1), Some(2)), (Some(2), Some(1))]
    );
    {
        let p = v.pair.as_ref().unwrap();
        assert!(p.history[1].can_undo());
        assert_eq!(p.map.right.borrow().display_at(1, 2).unwrap(), "pear");
    }
    assert_eq!(statistics(&v).same, 2);
    v.grid.active = 1;
    v.undo(false);
    ready(&mut v);
    assert_eq!(statistics(&v).same, 1);
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .right
            .borrow()
            .display_at(1, 2)
            .unwrap(),
        "PEAR"
    );
}

#[test]
fn swap_also_transposes_default_rules_and_keeps_manual_anchors_consistent() {
    let mut v = shifted_view();
    v.default_rules.columns = ColumnMapping(vec![
        ComparisonColumn::new(Some(0), Some(0)),
        ComparisonColumn::new(Some(1), Some(2)),
        ComparisonColumn::new(Some(2), Some(1)),
    ]);
    // 手动锚点也随 Swap 转置，仍绑定同一对源行。
    v.grid.active = 0;
    v.grid.cursor = Cursor { row: 0, column: 0 };
    let edit = {
        let pair = v.pair.as_mut().unwrap();
        pair.map
            .edit_sheet(Side::Left, "Align With", |_, _, anchors| {
                anchors.insert(0, 0)
            })
            .unwrap()
    };
    v.pair.as_mut().unwrap().history[0].push(edit);
    v.edited();
    ready(&mut v);
    v.swap();
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().map.overrides().pairs(), &[(0, 0)]);
    assert_eq!(
        mapping(&v),
        vec![(Some(0), Some(0)), (Some(2), Some(1)), (Some(1), Some(2))]
    );
    // default_rules 是后续加载/扫描使用的草稿，也必须转置。
    assert_eq!(
        v.default_rules
            .columns
            .0
            .iter()
            .map(|c| (c.left, c.right))
            .collect::<Vec<_>>(),
        vec![(Some(0), Some(0)), (Some(2), Some(1)), (Some(1), Some(2))]
    );
    // Undo 锚点编辑：转置后的 before_settings 恢复，覆盖被清除。
    v.grid.active = 1; // Align With 编辑属于原左侧文件，Swap 后在 history[1]
    v.undo(false);
    ready(&mut v);
    assert!(v.pair.as_ref().unwrap().map.overrides().pairs().is_empty());
}

#[test]
fn open_file_replaces_only_that_side_and_a_failed_open_keeps_the_whole_session() {
    let left = temp_file("open-left.csv", "id,name,price\n7,apple,1.0\n");
    let right = temp_file("open-right.csv", "id,name,price\n7,apple,1.2\n");
    let newer = temp_file("open-new-left.csv", "id,name,price\n9,kiwi,3.0\n");
    let mut v = open_files(&left, &right);
    // 右侧有未保存编辑：替换左侧不得触碰它。
    v.begin_edit(1, Cursor { row: 1, column: 2 }, Some("4.5".into()));
    v.commit_edit().unwrap();
    ready(&mut v);
    assert!(v.pair.as_ref().unwrap().history[1].is_dirty());
    v.request(Pending::Open(0, newer.clone()));
    assert!(
        v.pending.is_none(),
        "clean target side needs no confirmation"
    );
    ready(&mut v);
    {
        let p = v.pair.as_ref().unwrap();
        assert_eq!(p.map.left.borrow().display_at(1, 0).unwrap(), "9");
        assert_eq!(p.map.right.borrow().display_at(1, 2).unwrap(), "4.5");
        assert!(p.history[1].can_undo(), "other side history preserved");
    }
    assert_eq!(v.files[0].path.as_ref().unwrap(), &newer);
    v.undo(false); // active side 0 history cleared; undo right side explicitly
    v.grid.active = 1;
    v.undo(false);
    ready(&mut v);
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .right
            .borrow()
            .display_at(1, 2)
            .unwrap(),
        "1.2"
    );
    // 失败：不存在的路径不得丢弃当前比较、路径或编辑状态。
    let missing = left.with_file_name("definitely-missing-wave7.csv");
    v.request(Pending::Open(0, missing));
    ready(&mut v);
    assert!(v.pair.is_some(), "failed open must keep the comparison");
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .left
            .borrow()
            .display_at(1, 0)
            .unwrap(),
        "9"
    );
    assert_eq!(v.files[0].path.as_ref().unwrap(), &newer);
    assert!(!v.message.is_empty());
    for f in [left, right, newer] {
        let _ = std::fs::remove_file(f);
    }
}

#[test]
fn reload_failure_keeps_the_previous_comparison_and_reload_success_rereads() {
    let left = temp_file("reload-left.csv", "id,name\n7,apple\n");
    let right = temp_file("reload-right.csv", "id,name\n7,apple\n");
    let mut v = open_files(&left, &right);
    std::fs::remove_file(&right).unwrap();
    v.request(Pending::Reload);
    assert!(v.pending.is_none());
    ready(&mut v);
    assert!(
        v.pair.is_some(),
        "failed reload keeps the previous comparison"
    );
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .left
            .borrow()
            .display_at(1, 1)
            .unwrap(),
        "apple"
    );
    assert!(!v.message.is_empty());
    // 恢复文件并修改内容：成功 Reload 后读取新内容。
    std::fs::write(&right, "id,name\n7,APPLE\n").unwrap();
    v.request(Pending::Reload);
    ready(&mut v);
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .right
            .borrow()
            .display_at(1, 1)
            .unwrap(),
        "APPLE"
    );
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.important, 1);
    let _ = std::fs::remove_file(left);
    let _ = std::fs::remove_file(right);
}

#[test]
fn open_clipboard_replaces_only_that_side_and_preserves_the_other_history() {
    let mut v = view();
    v.begin_edit(1, Cursor { row: 0, column: 2 }, Some("4.5".into()));
    v.commit_edit().unwrap();
    ready(&mut v);
    v.request(Pending::Clipboard(0, "id,name,price\n9,kiwi,3.0\n".into()));
    assert!(v.pending.is_none());
    ready(&mut v);
    {
        let p = v.pair.as_ref().unwrap();
        assert_eq!(p.map.left.borrow().display_at(1, 1).unwrap(), "kiwi");
        assert_eq!(p.map.right.borrow().display_at(0, 2).unwrap(), "4.5");
        assert!(
            p.history[1].can_undo(),
            "other side history survives clipboard replace"
        );
        assert!(!p.history[0].can_undo(), "replaced side stack is cleared");
    }
    assert!(v.files[0].path.is_none());
    assert!(v.files[0].clipboard_dirty);
    // 剪贴板侧保留表头行：id 行与 9 行都无键配对 → 两个仅左。
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.left_only, 2);
    v.grid.active = 1;
    v.undo(false);
    ready(&mut v);
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .right
            .borrow()
            .display_at(0, 2)
            .unwrap(),
        "1.2"
    );
}

#[test]
fn settings_invalid_mapping_is_rejected_without_polluting_rules_or_defaults() {
    let mut v = view();
    v.settings_open();
    let mut draft = v.settings.draft.clone().unwrap();
    draft.rules.columns.0[0].left = Some(99);
    let before = v.default_rules.clone();
    v.request(Pending::Settings(Box::new(draft)));
    assert!(v.pending.is_none(), "rules-only change is not destructive");
    assert!(!v.message.is_empty(), "invalid mapping reports an error");
    assert_eq!(v.default_rules, before, "defaults stay unpolluted");
    assert_eq!(
        v.pair.as_ref().unwrap().map.rules().columns.0[0].left,
        Some(0)
    );
    assert!(v.is_ready());
}

#[test]
fn settings_reload_failure_keeps_paths_options_and_the_running_comparison() {
    let left = temp_file("settings-left.csv", "id,name\n7,apple\n");
    let right = temp_file("settings-right.csv", "id,name\n7,APPLE\n");
    let mut v = open_files(&left, &right);
    v.settings_open();
    let mut draft = v.settings.draft.clone().unwrap();
    draft.paths[1] = right
        .with_file_name("missing-wave7.csv")
        .display()
        .to_string();
    let before_options = v.options.clone();
    let before_rules = v.default_rules.clone();
    v.request(Pending::Settings(Box::new(draft)));
    ready(&mut v);
    assert!(
        v.pair.is_some(),
        "failed settings reload keeps the comparison"
    );
    assert_eq!(v.files[1].path.as_ref().unwrap(), &right);
    assert_eq!(v.options, before_options);
    assert_eq!(v.default_rules, before_rules);
    assert!(!v.message.is_empty());
    let _ = std::fs::remove_file(left);
    let _ = std::fs::remove_file(right);
}

#[test]
fn save_failure_rearms_realign_and_does_not_cancel_the_sheet_scan() {
    let mut v = view();
    // 伪造第二个 sheet 对，让扫描 worker 启动（空路径加载空 sheet）。
    v.names = [vec!["a".into(), "b".into()], vec!["a".into(), "b".into()]];
    v.pairs = bc_engine_table::sheets::SheetPairs(vec![
        bc_engine_table::sheets::SheetPair {
            left: Some(0),
            right: Some(0),
        },
        bc_engine_table::sheets::SheetPair {
            left: Some(1),
            right: Some(1),
        },
    ]);
    v.states = vec![Some(Importance::Same), None];
    v.summaries = vec![None, None];
    v.poll();
    assert!(v.scan.is_some(), "scan spawned for the pending sheet");
    // 编辑使 map stale，然后保存到目录路径必然失败。
    v.begin_edit(0, Cursor { row: 0, column: 1 }, Some("changed".into()));
    v.commit_edit().unwrap();
    assert!(v.pair.as_ref().unwrap().map.is_stale());
    let dir = std::env::temp_dir();
    let err = v.save_to(0, &dir, &no_backup_prefs(), false);
    assert!(err.is_err());
    assert!(v.scan.is_some(), "a save must not cancel the sheet scan");
    assert!(
        v.edited_at.is_some(),
        "a failed save must re-arm the pending realignment"
    );
    ready(&mut v);
    assert!(!v.pair.as_ref().unwrap().map.is_stale());
}

#[test]
fn summary_export_writes_keyed_rows_and_stale_results_are_rejected() {
    let mut v = view();
    let path = temp_file("summary.txt", "");
    let ctx = egui::Context::default();
    // 编辑后结果陈旧：导出必须被拒绝且不写文件。
    v.begin_edit(0, Cursor { row: 0, column: 1 }, Some("changed".into()));
    v.commit_edit().unwrap();
    native_dialog::mock::respond(Some(path.clone()));
    v.dialog = Some(PathDialog {
        mode: FileMode::Summary(false),
        path: String::new(),
    });
    frame(&ctx, &mut v, vec![]);
    assert!(
        !v.message.is_empty(),
        "stale summary is rejected with a message"
    );
    assert!(std::fs::read_to_string(&path).unwrap().is_empty());
    ready(&mut v);
    // 就绪后导出：按键列出变化行与列结构。
    native_dialog::mock::respond(Some(path.clone()));
    v.dialog = Some(PathDialog {
        mode: FileMode::Summary(false),
        path: String::new(),
    });
    frame(&ctx, &mut v, vec![]);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with("MATCHA-TABLE-DIFF"));
    assert!(text.contains("ROW\t7\tChanged"));
    assert!(text.contains("price") && text.contains("1.0") && text.contains("1.2"));
    // CSV 变体
    native_dialog::mock::respond(Some(path.clone()));
    v.dialog = Some(PathDialog {
        mode: FileMode::Summary(true),
        path: String::new(),
    });
    frame(&ctx, &mut v, vec![]);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with("record,key,status,column,layer,old,new"));
    assert!(text.contains("7,Changed,price,value"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn inferred_keys_are_annotated_with_column_names_in_status_and_settings() {
    let mut v = TableCompareView::from_sheets(
        sheet("id,name\n7,apple\n8,pear\n"),
        sheet("id,name\n7,apple\n8,PEAR\n"),
        TableRules::default(),
    );
    ready(&mut v);
    let ctx = egui::Context::default();
    // Wave 8: the old statistics line moved to Session › Table Compare Info (Ctrl+I).
    frame(&ctx, &mut v, vec![key(egui::Key::I, true)]);
    frame(&ctx, &mut v, vec![]); // 窗口首帧布局
    let out = frame(&ctx, &mut v, vec![]);
    assert!(
        out.shapes.iter().any(
            |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text().contains("id（自动）"))
        ),
        "Table Compare Info names the inferred key columns"
    );
    v.show_statistics = false;
    v.settings_open();
    frame(&ctx, &mut v, vec![]); // 窗口首帧布局
    let out = frame(&ctx, &mut v, vec![]);
    let columns_tab = out
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == "Columns" => {
                Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center())
            }
            _ => None,
        })
        .expect("settings Columns tab");
    click(&ctx, &mut v, columns_tab);
    let out = frame(&ctx, &mut v, vec![]);
    assert!(
        out.shapes.iter().any(
            |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text().contains("id（自动）"))
        ),
        "Columns settings annotate the inferred keys"
    );
}

fn menu_click(ctx: &egui::Context, v: &mut TableCompareView, label: &str) {
    frame(ctx, v, vec![]);
    let out = frame(ctx, v, vec![]);
    click(ctx, v, button(&out, label));
}

#[test]
fn table_compare_info_lists_per_sheet_counts_through_the_menu() {
    // 键推断友好：5/6 键重合 → id 列自动为键；1 行变化 + 1 行仅左。
    let left = temp_file(
        "info-left.csv",
        "id,name,price
1,a,1.0
2,b,2.0
3,c,3.0
4,d,4.0
5,e,5.0
6,f,6.0
",
    );
    let right = temp_file(
        "info-right.csv",
        "id,name,price
1,a,1.5
2,b,2.0
3,c,3.0
4,d,4.0
5,e,5.0
",
    );
    let mut v = open_files(&left, &right);
    let ctx = egui::Context::default();
    menu_click(&ctx, &mut v, "Session");
    menu_click(&ctx, &mut v, "Table Compare Info");
    frame(&ctx, &mut v, vec![]); // 窗口首帧布局
    let out = frame(&ctx, &mut v, vec![]);
    assert!(
        out.shapes.iter().any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text().contains("Sheet 1: 5 same / 0 unimportant / 1 important / 1 left-only / 0 right-only / 3 columns"))),
        "Info lists same/unimportant/important/left-only/right-only/columns"
    );
    let _ = std::fs::remove_file(left);
    let _ = std::fs::remove_file(right);
}

#[test]
fn save_as_updates_only_that_side_target_and_marks_it_clean() {
    let mut v = view();
    v.begin_edit(0, Cursor { row: 0, column: 1 }, Some("apple2".into()));
    v.commit_edit().unwrap();
    v.begin_edit(1, Cursor { row: 1, column: 2 }, Some("9.9".into()));
    v.commit_edit().unwrap();
    ready(&mut v);
    assert!(v.pair.as_ref().unwrap().history[0].is_dirty());
    assert!(v.pair.as_ref().unwrap().history[1].is_dirty());
    let left_out = temp_file("save-as-left.csv", "");
    std::fs::remove_file(&left_out).unwrap();
    v.save_to(0, &left_out, &no_backup_prefs(), false).unwrap();
    ready(&mut v);
    assert!(!v.pair.as_ref().unwrap().history[0].is_dirty());
    assert!(
        v.pair.as_ref().unwrap().history[1].is_dirty(),
        "other side stays dirty"
    );
    assert_eq!(v.files[0].target.as_ref().unwrap(), &left_out);
    assert!(std::fs::read_to_string(&left_out)
        .unwrap()
        .contains("apple2"));
    // Ctrl+S 保存活动侧（右侧仍有未保存编辑 → 使用其目标或路径；无路径则弹路径框）。
    v.files[1].path = None;
    v.files[1].target = None;
    v.grid.active = 1;
    let ctx = egui::Context::default();
    let right_out = temp_file("save-as-right.csv", "");
    std::fs::remove_file(&right_out).unwrap();
    // 无路径侧 Ctrl+S 弹系统「另存为」对话框（测试替身返回 right_out）。
    native_dialog::mock::respond(Some(right_out.clone()));
    frame(&ctx, &mut v, vec![key(egui::Key::S, true)]);
    let request = native_dialog::mock::last().expect("save dialog shown");
    assert_eq!(request.kind, native_dialog::Kind::SaveFile);
    assert_eq!(request.title, "Save File As - Right Side");
    assert!(v.dialog.is_none());
    ready(&mut v);
    assert!(!v.pair.as_ref().unwrap().history[1].is_dirty());
    assert!(std::fs::read_to_string(&right_out).unwrap().contains("9.9"));
    let _ = std::fs::remove_file(left_out);
    let _ = std::fs::remove_file(right_out);
}

#[test]
fn workbook_side_reports_readonly_on_save_and_export_still_works() {
    let book = temp_file(
        "readonly.xlsx",
        "PK", // placeholder overwritten below
    );
    std::fs::remove_file(&book).unwrap();
    // 用引擎测试同款最小 xlsx 由 sheet_tests 覆盖；此处用 from_sheets + workbook 标记直接构造。
    let mut left = sheet("id,name\n7,apple\n");
    left.source = bc_engine_table::model::SheetSource::Workbook;
    let mut v =
        TableCompareView::from_sheets(left, sheet("id,name\n7,apple\n"), TableRules::default());
    ready(&mut v);
    let out_path = temp_file("readonly-out.csv", "");
    std::fs::remove_file(&out_path).unwrap();
    let err = v
        .save_to(0, &out_path, &no_backup_prefs(), false)
        .unwrap_err();
    assert!(err.to_string().contains("read-only"));
    assert!(!out_path.exists());
    // CSV 导出作为出口仍然可用。
    v.save_to(0, &out_path, &no_backup_prefs(), true).unwrap();
    assert!(std::fs::read_to_string(&out_path)
        .unwrap()
        .contains("apple"));
    let _ = std::fs::remove_file(out_path);
    let _ = book;
}

#[test]
fn closing_settings_without_apply_changes_nothing() {
    let mut v = view();
    let before_rules = v.pair.as_ref().unwrap().map.rules().clone();
    v.settings_open();
    {
        let draft = v.settings.draft.as_mut().unwrap();
        draft.rules.ignore_unimportant = true;
        draft.rules.columns.0[1].handling.unimportant = true;
    }
    v.settings.open = false; // 窗口关闭按钮：丢弃草稿
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.pair.as_ref().unwrap().map.rules().clone(), before_rules);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.important, 2);
    // 重新打开：草稿从当前规则重建，不残留上次未应用修改。
    v.settings_open();
    assert!(!v.settings.draft.as_ref().unwrap().rules.ignore_unimportant);
}

#[test]
fn settings_apply_through_the_real_window_takes_effect_and_accept_structure_clears_annotations() {
    // 右侧多一列 note → 结构注释 Inserted；Apply 后统计真正变化。
    let mut v = TableCompareView::from_sheets(
        sheet("id,name\n7,apple\n8,pear\n"),
        sheet("id,note,name\n7,x,apple\n8,y,PEAR\n"),
        TableRules {
            column_mode: bc_engine_table::rules::ColumnMode::Automatic,
            ..Default::default()
        },
    );
    ready(&mut v);
    // note=Inserted 且 name 位置不同=Moved，共两条结构注释。
    assert_eq!(
        v.pair.as_ref().unwrap().cache.statistics.structural_columns,
        2
    );
    // 仅右列的空隙单元格使两行均为重要差异。
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.important, 2);
    let ctx = egui::Context::default();
    v.settings_open();
    frame(&ctx, &mut v, vec![]);
    let out = frame(&ctx, &mut v, vec![]);
    let tab = out
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == "Columns" => {
                Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center())
            }
            _ => None,
        })
        .unwrap();
    click(&ctx, &mut v, tab);
    // Accept structure 清除注释；勾选第 2 比较列 Unimportant。
    let out = frame(&ctx, &mut v, vec![]);
    click(&ctx, &mut v, button(&out, "Accept structure"));
    let out = frame(&ctx, &mut v, vec![]);
    // 列顺序为 [id, note(仅右), name]：选中 name（第 3 行）。
    let row = out
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text().starts_with("3: L") => {
                Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center())
            }
            _ => None,
        })
        .expect("comparison column row 3");
    click(&ctx, &mut v, row);
    assert_eq!(v.settings.selected, 2);
    // 自动派生列跟随 Use Default：先解除继承才能逐列设置。
    let out = frame(&ctx, &mut v, vec![]);
    click(&ctx, &mut v, button(&out, "Use Default"));
    let out = frame(&ctx, &mut v, vec![]);
    click(&ctx, &mut v, button(&out, "Unimportant"));
    let out = frame(&ctx, &mut v, vec![]);
    click(&ctx, &mut v, button(&out, "Apply"));
    ready(&mut v);
    let p = v.pair.as_ref().unwrap();
    assert_eq!(
        p.cache.statistics.structural_columns, 0,
        "accepted annotations cleared"
    );
    assert!(p.map.rules().columns.0[2].handling.unimportant);
    // 单元格级生效证据：name 列 pear/PEAR 变为不重要；仅右列仍为重要。
    assert_eq!(
        p.map.cell_importance(1, 2),
        Some(bc_engine_table::compare::Importance::Unimportant),
        "column handling actually applied"
    );
    assert_eq!(
        p.map.cell_importance(0, 1),
        Some(bc_engine_table::compare::Importance::Important)
    );
}

#[test]
fn swap_during_the_initial_load_restarts_safely() {
    // 不等待首个 LoadJob 完成就 Swap：不得停在永久 spinner。
    let left = temp_file("n1-left.csv", "id,name\n7,apple\n");
    let right = temp_file("n1-right.csv", "id,name\n7,APPLE\n");
    let mut v = TableCompareView::open_with_registry(
        &left,
        &right,
        &bc_formats::registry::FormatRegistry::with_builtins(),
    )
    .unwrap();
    v.swap();
    ready(&mut v);
    assert!(v.pair.is_some());
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .left
            .borrow()
            .display_at(1, 1)
            .unwrap(),
        "APPLE"
    );
    assert_eq!(v.files[0].path.as_ref().unwrap(), &right);
    let _ = std::fs::remove_file(left);
    let _ = std::fs::remove_file(right);
}

#[test]
fn recompare_keeps_history_and_recomputes() {
    let mut v = view();
    v.begin_edit(0, Cursor { row: 0, column: 1 }, Some("apple2".into()));
    v.commit_edit().unwrap();
    ready(&mut v);
    v.recompare();
    ready(&mut v);
    assert!(v.pair.as_ref().unwrap().history[0].can_undo());
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .left
            .borrow()
            .display_at(0, 1)
            .unwrap(),
        "apple2"
    );
    v.undo(false);
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.important, 2);
}

#[test]
fn replacing_a_dirty_side_asks_confirmation_and_discard_keeps_the_other_side() {
    let left = temp_file("dirty-left.csv", "id,name\n7,apple\n");
    let right = temp_file("dirty-right.csv", "id,name\n7,apple\n");
    let newer = temp_file("dirty-new-left.csv", "id,name\n8,pear\n");
    let mut v = open_files(&left, &right);
    // 两侧都编辑：替换左侧需要确认；Discard 后右侧编辑/历史仍在。
    v.begin_edit(0, Cursor { row: 1, column: 1 }, Some("APPLE".into()));
    v.commit_edit().unwrap();
    v.begin_edit(1, Cursor { row: 1, column: 1 }, Some("apricot".into()));
    v.commit_edit().unwrap();
    ready(&mut v);
    v.request(Pending::Open(0, newer.clone()));
    assert!(
        v.pending.is_some(),
        "dirty target side requires confirmation"
    );
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    let out = frame(&ctx, &mut v, vec![]);
    click(&ctx, &mut v, button(&out, "Don't Save"));
    ready(&mut v);
    {
        let p = v.pair.as_ref().unwrap();
        assert_eq!(p.map.left.borrow().display_at(1, 1).unwrap(), "pear");
        assert_eq!(p.map.right.borrow().display_at(1, 1).unwrap(), "apricot");
        assert!(
            p.history[1].can_undo(),
            "other side edits survive the discard"
        );
        assert!(!p.history[0].can_undo(), "replaced side history cleared");
    }
    let _ = std::fs::remove_file(left);
    let _ = std::fs::remove_file(right);
    let _ = std::fs::remove_file(newer);
}

#[test]
fn format_settings_change_actually_reparses_the_file() {
    let left = temp_file("fmt-left.dat", "id;name\n7;apple\n");
    let right = temp_file("fmt-right.dat", "id;name\n7;apple\n");
    let mut v = open_files(&left, &right);
    // 未知扩展名 → 默认探测：分号被探测为两列。
    ready(&mut v);
    let cols = v.pair.as_ref().unwrap().map.left.borrow().column_count();
    assert!(cols >= 1);
    // 改成显式逗号：应用后按一列解析，比较列随之重建。
    v.settings_open();
    let mut draft = v.settings.draft.clone().unwrap();
    for s in 0..2 {
        draft.options[s].format.detect_delimiter = false;
        draft.options[s].format.delimiters = vec![','];
    }
    v.request(Pending::Settings(Box::new(draft)));
    ready(&mut v);
    let p = v.pair.as_ref().unwrap();
    assert!(
        !v.options[0].format.detect_delimiter,
        "new options committed"
    );
    assert_eq!(
        p.map.left.borrow().column_count(),
        1,
        "format change reparses"
    );
    assert_eq!(p.map.column_count(), 1);
    // 单列无法检测表头，两行均为数据行且两侧一致。
    assert_eq!(p.cache.statistics.same, 2);
    let _ = std::fs::remove_file(left);
    let _ = std::fs::remove_file(right);
}

#[test]
fn open_file_routes_the_new_source_through_the_format_registry() {
    // 左侧先为 TSV（显式 tab、不探测），再 Open 一个 CSV：新来源必须按掩码
    // 重新匹配选项，而不是沿用 TSV 选项把逗号文件读成一列。
    let left = temp_file("route-left.tsv", "id\tname\n7\tapple\n");
    let right = temp_file("route-right.tsv", "id\tname\n7\tapple\n");
    let mut v = open_files(&left, &right);
    assert!(!v.options[0].format.detect_delimiter);
    let csv = temp_file("route-new-left.csv", "id,name\n7,apple\n8,pear\n");
    v.request(Pending::Open(0, csv.clone()));
    ready(&mut v);
    assert_ne!(
        v.options[0].format.delimiters,
        vec!['\t'],
        "options re-routed for the new source"
    );
    assert_eq!(
        v.pair.as_ref().unwrap().map.left.borrow().column_count(),
        2,
        "comma source parses as two columns"
    );
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.left_only, 1);
    let _ = std::fs::remove_file(left);
    let _ = std::fs::remove_file(right);
    let _ = std::fs::remove_file(csv);
}

#[test]
fn failed_initial_load_shows_error_and_retry_instead_of_a_forever_spinner() {
    let bad = temp_file("broken.xlsx", "not a zip at all");
    let ok = temp_file("retry-right.csv", "id,name\n7,apple\n");
    let mut v = TableCompareView::open_with_registry(
        &bad,
        &ok,
        &bc_formats::registry::FormatRegistry::with_builtins(),
    )
    .unwrap();
    let start = Instant::now();
    while v.worker.is_some() {
        v.poll();
        assert!(start.elapsed() < Duration::from_secs(15));
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(v.pair.is_none());
    assert!(!v.message.is_empty());
    let ctx = egui::Context::default();
    let out = frame(&ctx, &mut v, vec![]);
    assert!(out.shapes.iter().any(
        |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text().starts_with("Load failed:"))
    ));
    // 修复文件后点击 Retry 真正恢复。
    std::fs::write(
        &bad,
        super::sheets::xlsx(&[(
            "Data",
            "id,name
7,apple",
        )]),
    )
    .unwrap();
    click(&ctx, &mut v, button(&out, "Retry load"));
    ready(&mut v);
    assert!(v.pair.is_some());
    let _ = std::fs::remove_file(bad);
    let _ = std::fs::remove_file(ok);
}

#[test]
fn clipboard_replace_preserves_cached_sheets_of_the_other_side_without_confirmation() {
    let left = temp_file(
        "cached-left.csv",
        "id,name
7,apple
",
    );
    let right = temp_file(
        "cached-right.csv",
        "id,name
7,APPLE
",
    );
    let mut v = open_files(&left, &right);
    // 活动对右侧编辑 + 伪造一个带编辑的缓存 sheet 对（等价于访问过第二个 sheet）。
    v.begin_edit(1, Cursor { row: 1, column: 1 }, Some("apricot".into()));
    v.commit_edit().unwrap();
    ready(&mut v);
    let mut cached = Pair {
        map: bc_engine_table::TableMap::new(
            sheet(
                "id,t
1,x
",
            ),
            sheet(
                "id,t
1,y
",
            ),
            TableRules::default(),
            &bc_core::cancel::NeverCancel,
        )
        .unwrap(),
        history: [EditStack::new(), EditStack::new()],
        cache: Default::default(),
        projection: Default::default(),
        pending_side: None,
    };
    let edit = cached
        .map
        .modify_cell(bc_engine_table::columns::Side::Right, 0, 1, Cell::text("z"))
        .unwrap();
    cached.history[1].push(edit);
    v.cached.insert(1, cached);
    v.names = [vec!["a".into(), "b".into()], vec!["a".into(), "b".into()]];
    v.pairs.0.push(bc_engine_table::sheets::SheetPair {
        left: Some(1),
        right: Some(1),
    });
    v.states.push(None);
    v.summaries.push(None);
    // 替换干净的左侧：不确认、不丢任何未替换侧的 sheet。
    v.request(Pending::Clipboard(
        0,
        "id,name
9,kiwi
"
        .into(),
    ));
    assert!(v.pending.is_none(), "preserved sheets need no confirmation");
    ready(&mut v);
    {
        let p = v.pair.as_ref().unwrap();
        assert_eq!(p.map.left.borrow().display_at(1, 1).unwrap(), "kiwi");
        assert_eq!(p.map.right.borrow().display_at(1, 1).unwrap(), "apricot");
        assert!(p.history[1].can_undo());
    }
    // 另一侧的第二个 sheet 保留标签、编辑与历史；其被替换侧显示为诚实空位。
    assert_eq!(v.names[1], vec!["a".to_string(), "b".to_string()]);
    assert!(
        v.cached.contains_key(&1),
        "cached pair re-hung, not dropped"
    );
    v.select_sheet(1);
    ready(&mut v);
    let p = v.pair.as_ref().unwrap();
    assert_eq!(p.map.right.borrow().display_at(0, 1).unwrap(), "z");
    assert!(p.history[1].can_undo(), "cached sheet history survives");
    assert_eq!(p.map.left.borrow().row_count(), 0);
    assert_eq!(p.cache.statistics.right_only, 1);
    let _ = std::fs::remove_file(left);
    let _ = std::fs::remove_file(right);
}

#[test]
fn search_panel_buttons_and_keyboard_undo_flow_through_raw_input() {
    let mut v = view();
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![key(egui::Key::F, true)]);
    assert!(v.find.open, "Ctrl+F opens the find panel");
    v.find.query = "pear".into();
    v.find.side = 1;
    v.find.case = false;
    frame(&ctx, &mut v, vec![]);
    let out = frame(&ctx, &mut v, vec![]);
    click(&ctx, &mut v, button(&out, "Next"));
    assert_eq!(v.grid.cursor, Cursor { row: 1, column: 1 });
    v.find.replacement = "梨".into();
    let out = frame(&ctx, &mut v, vec![]);
    click(&ctx, &mut v, button(&out, "Replace all"));
    ready(&mut v);
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .right
            .borrow()
            .display_at(1, 1)
            .unwrap(),
        "梨"
    );
    frame(&ctx, &mut v, vec![key(egui::Key::Z, true)]);
    ready(&mut v);
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .right
            .borrow()
            .display_at(1, 1)
            .unwrap(),
        "PEAR"
    );
}

#[test]
fn swap_mirrors_orphan_counts_and_statistics() {
    let mut v = TableCompareView::from_sheets(
        sheet("id,name\n7,a\n8,b\n10,c\n"),
        sheet("id,name\n7,a\n9,d\n"),
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            ..Default::default()
        },
    );
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.left_only, 2);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.right_only, 1);
    v.swap();
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.left_only, 1);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.right_only, 2);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.same, 1);
}

#[test]
fn open_two_real_xlsx_files_compares_via_parallel_load() {
    // 真实 xlsx 走 LoadJob 的并行双侧/三段解析路径；统计与内容必须与串行一致。
    let left =
        std::env::temp_dir().join(format!("matcha-xlsx-par-left-{}.xlsx", std::process::id()));
    let right =
        std::env::temp_dir().join(format!("matcha-xlsx-par-right-{}.xlsx", std::process::id()));
    std::fs::write(
        &left,
        super::sheets::xlsx(&[("Data", "id,name\n1,apple\n2,banana\n3,cherry")]),
    )
    .unwrap();
    std::fs::write(
        &right,
        super::sheets::xlsx(&[("Data", "id,name\n1,apple\n2,BLUEberry\n4,damson")]),
    )
    .unwrap();
    let mut v = open_files(&left, &right);
    ready(&mut v);
    // 表头行保留为数据（字母列头）：5 行 = 表头 + 3 数据 + 1 仅左。
    assert_eq!(v.row_count(), 5);
    let p = v.pair.as_ref().unwrap();
    let s = &p.cache.statistics;
    assert_eq!(s.same, 2);
    assert_eq!(s.important, 1);
    assert_eq!(s.left_only, 1);
    assert_eq!(s.right_only, 1);
    assert_eq!(p.map.left.borrow().row_count(), 4);
    assert_eq!(p.map.right.borrow().row_count(), 4);
    assert_eq!(p.map.left.borrow().display_at(2, 1).unwrap(), "banana");
    assert_eq!(p.map.right.borrow().display_at(2, 1).unwrap(), "BLUEberry");
    let _ = std::fs::remove_file(left);
    let _ = std::fs::remove_file(right);
}

fn cell(v: &TableCompareView, s: usize, r: usize, c: usize) -> String {
    let p = v.pair.as_ref().unwrap();
    let sheet = if s == 0 {
        p.map.left.borrow()
    } else {
        p.map.right.borrow()
    };
    sheet.display_at(r, c).unwrap()
}

fn rows(n: usize, tag: &str) -> String {
    let mut text = String::from("id,name\n");
    for i in 1..=n {
        text += &format!("{i},{tag}{i}\n");
    }
    text
}

#[test]
fn disk_change_reload_keeps_the_cursor_row_and_reads_the_new_value() {
    let left = temp_file("disk-left.csv", &rows(30, "v"));
    let right = temp_file("disk-right.csv", &rows(30, "v"));
    let mut v = open_files(&left, &right);
    v.goto_cell(Cursor { row: 20, column: 1 });
    let ask = bc_session::prefs::DiskChangePrefs {
        check: true,
        auto_reload: false,
    };
    crate::disk_watch::check(&mut v, &ask);
    assert!(v.disk_prompt.is_none(), "nothing changed yet");
    std::fs::write(&right, rows(30, "v").replace("v21", "CHANGED21")).unwrap();
    crate::disk_watch::check(&mut v, &ask);
    assert_eq!(v.disk_prompt.as_ref().unwrap().changed[0].side, 1);
    crate::disk_watch::answer(&mut v, true);
    let start = std::time::Instant::now();
    while v.disk_keep.is_some() || !v.is_ready() {
        v.poll();
        assert!(
            start.elapsed() < std::time::Duration::from_secs(15),
            "{}",
            v.message
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert_eq!(cell(&v, 1, 21, 1), "CHANGED21");
    assert_eq!(v.grid.cursor, Cursor { row: 20, column: 1 });
    // The reload is the new baseline: no prompt on the next check.
    crate::disk_watch::check(&mut v, &ask);
    assert!(v.disk_prompt.is_none());
    let _ = std::fs::remove_file(left);
    let _ = std::fs::remove_file(right);
}

#[test]
fn auto_reload_of_one_side_keeps_the_other_sides_unsaved_edit() {
    let left = temp_file("disk-edit-left.csv", "id,name\n1,a\n2,b\n");
    let right = temp_file("disk-edit-right.csv", "id,name\n1,a\n2,b\n");
    let mut v = open_files(&left, &right);
    v.begin_edit(0, Cursor { row: 1, column: 1 }, Some("edited".into()));
    v.commit_edit().unwrap();
    assert!(v.pair.as_ref().unwrap().history[0].is_dirty());
    let edited = (0..=2)
        .find(|&r| cell(&v, 0, r, 1) == "edited")
        .expect("edited cell");
    std::fs::write(&right, "id,name\n1,a\n2,outside\n").unwrap();
    crate::disk_watch::check(
        &mut v,
        &bc_session::prefs::DiskChangePrefs {
            check: true,
            auto_reload: true,
        },
    );
    assert!(
        v.disk_prompt.is_none(),
        "the changed side is clean: no prompt"
    );
    let start = std::time::Instant::now();
    while v.disk_keep.is_some() || !v.is_ready() {
        v.poll();
        assert!(
            start.elapsed() < std::time::Duration::from_secs(15),
            "{}",
            v.message
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert_eq!(cell(&v, 1, 2, 1), "outside");
    assert_eq!(cell(&v, 0, edited, 1), "edited");
    assert!(v.pair.as_ref().unwrap().history[0].is_dirty());
    let _ = std::fs::remove_file(left);
    let _ = std::fs::remove_file(right);
}
