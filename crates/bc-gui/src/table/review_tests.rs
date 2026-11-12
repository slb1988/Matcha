//! 4.4 最终 review 的两个 blocking 回归（先复现后修）：
//! R1：单侧替换必须对"缓存 sheet 的目标侧脏编辑"确认（未替换侧缓存编辑保留）。
//! R2：替换后保留的 pending 缓存 sheet 不得让扫描无限重启；失败扫描不忙重试。
use super::*;
use bc_core::edit::EditStack;
use bc_engine_table::columns::Side;
use bc_engine_table::compare::Importance;

/// 活动对干净；缓存对（索引 1）目标侧与对侧都各有一处未保存编辑。
fn view_with_dirty_cached_pair() -> TableCompareView {
    let mut v = view();
    let mut cached = Pair {
        map: bc_engine_table::TableMap::new(
            sheet("id,t\n1,x\n"),
            sheet("id,t\n1,y\n"),
            TableRules::default(),
            &bc_core::cancel::NeverCancel,
        )
        .unwrap(),
        history: [EditStack::new(), EditStack::new()],
        cache: Default::default(),
        projection: Default::default(),
        pending_side: None,
    };
    let e0 = cached
        .map
        .modify_cell(Side::Left, 0, 1, Cell::text("lx"))
        .unwrap();
    cached.history[0].push(e0);
    let e1 = cached
        .map
        .modify_cell(Side::Right, 0, 1, Cell::text("ry"))
        .unwrap();
    cached.history[1].push(e1);
    v.cached.insert(1, cached);
    v.names = [vec!["a".into(), "b".into()], vec!["a".into(), "b".into()]];
    v.pairs.0.push(bc_engine_table::sheets::SheetPair {
        left: Some(1),
        right: Some(1),
    });
    v.states.push(None);
    v.summaries.push(None);
    v
}

fn dialog_click(ctx: &egui::Context, v: &mut TableCompareView, label: &str) {
    frame(ctx, v, vec![]);
    let out = frame(ctx, v, vec![]);
    click(ctx, v, button(&out, label));
}

#[test]
fn replacing_a_side_with_dirty_cached_target_edits_requires_confirmation() {
    let new_file = temp_file("r1-new-left.csv", "id,name\n9,kiwi\n");
    for use_open in [false, true] {
        let mut v = view_with_dirty_cached_pair();
        assert!(!v.pair.as_ref().unwrap().history[0].is_dirty());
        if use_open {
            v.request(Pending::Open(0, new_file.clone()));
        } else {
            v.request(Pending::Clipboard(0, "id,name\n9,kiwi\n".into()));
        }
        assert!(
            v.pending.is_some(),
            "use_open={use_open}: cached target-side edits must trigger confirmation"
        );
        // Cancel：全部保留（活动对与缓存对双侧历史）。
        let ctx = egui::Context::default();
        dialog_click(&ctx, &mut v, "Cancel");
        assert!(
            v.pair
                .as_ref()
                .unwrap()
                .map
                .left
                .borrow()
                .display_at(0, 1)
                .unwrap()
                == "apple"
        );
        let cached = v.cached.get(&1).unwrap();
        assert!(cached.history[0].can_undo() && cached.history[1].can_undo());
        // Discard：仅目标侧内容可替换；对侧缓存编辑与 Undo 仍在。
        if use_open {
            v.request(Pending::Open(0, new_file.clone()));
        } else {
            v.request(Pending::Clipboard(0, "id,name\n9,kiwi\n".into()));
        }
        dialog_click(&ctx, &mut v, "Don't Save");
        ready(&mut v);
        assert_eq!(
            v.pair
                .as_ref()
                .unwrap()
                .map
                .left
                .borrow()
                .display_at(1, 1)
                .unwrap(),
            "kiwi"
        );
        let (cached_idx, has_target, has_other) = {
            let (idx, p) = v.cached.iter().next().expect("cached pair re-hung");
            (*idx, p.history[0].can_undo(), p.history[1].can_undo())
        };
        assert!(
            !has_target,
            "replaced side's cached edits were confirmed away"
        );
        assert!(has_other, "untouched side's cached edits survive");
        // 访问该 sheet：对侧编辑内容在、可 Undo。
        v.select_sheet(cached_idx);
        ready(&mut v);
        let p = v.pair.as_ref().unwrap();
        assert_eq!(p.map.right.borrow().display_at(0, 1).unwrap(), "ry");
        assert_eq!(
            p.map.left.borrow().row_count(),
            0,
            "replaced side is an honest placeholder"
        );
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
                .display_at(0, 1)
                .unwrap(),
            "y"
        );
    }
    let _ = std::fs::remove_file(new_file);
}

#[test]
fn retained_pending_cached_sheets_do_not_retrigger_the_scan_forever() {
    let left = temp_file("scanloop-l.csv", "id,name\n7,apple\n");
    let right = temp_file("scanloop-r.csv", "id,name\n7,apple\n");
    let mut v = open_files(&left, &right);
    // 伪造第二个 sheet 对并让它被替换后保留在缓存里（pending）。
    let cached = Pair {
        map: bc_engine_table::TableMap::new(
            sheet("id,t\n1,x\n"),
            sheet("id,t\n1,y\n"),
            TableRules::default(),
            &bc_core::cancel::NeverCancel,
        )
        .unwrap(),
        history: [EditStack::new(), EditStack::new()],
        cache: Default::default(),
        projection: Default::default(),
        pending_side: None,
    };
    v.cached.insert(1, cached);
    v.names = [vec!["a".into(), "b".into()], vec!["a".into(), "b".into()]];
    v.pairs.0.push(bc_engine_table::sheets::SheetPair {
        left: Some(1),
        right: Some(1),
    });
    v.states.push(None);
    v.summaries.push(None);
    v.request(Pending::Clipboard(0, "id,name\n9,kiwi\n".into()));
    ready(&mut v);
    assert!(v.cached.contains_key(&1));
    assert_eq!(v.states[1], None, "cached pair stays pending until visited");
    assert!(v.scan.is_none());
    for _ in 0..5 {
        v.poll();
        std::thread::sleep(Duration::from_millis(20));
        assert!(
            v.scan.is_none(),
            "pending cached sheets must not respawn the scan"
        );
    }
    let _ = std::fs::remove_file(left);
    let _ = std::fs::remove_file(right);
}

#[test]
fn a_failing_sheet_scan_reports_error_and_waits_for_user_retry() {
    let left = temp_file(
        "scanfail-l.csv",
        "id,name
7,apple
",
    );
    let right = temp_file(
        "scanfail-r.csv",
        "id,name
7,apple
",
    );
    let mut v = open_files(&left, &right);
    // 第二个未访问 sheet 待扫描（非缓存）。
    v.names = [vec!["a".into(), "b".into()], vec!["a".into(), "b".into()]];
    v.pairs.0.push(bc_engine_table::sheets::SheetPair {
        left: Some(1),
        right: Some(1),
    });
    v.states.push(None);
    v.summaries.push(None);
    // 让扫描必然失败：右文件已消失。
    std::fs::remove_file(&right).unwrap();
    v.poll();
    assert!(
        v.scan.is_some(),
        "pending non-cached sheet triggers the scan"
    );
    let start = Instant::now();
    while v.scan.is_some() {
        v.poll();
        assert!(start.elapsed() < Duration::from_secs(15));
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        v.message.contains("Sheet scan failed"),
        "scan failure surfaces an error instead of faking Same"
    );
    assert_eq!(v.states[1], None, "failed sheet stays pending");
    for _ in 0..4 {
        v.poll();
        std::thread::sleep(Duration::from_millis(20));
        assert!(v.scan.is_none(), "failed scan must not busy-restart");
    }
    // 用户重试路径：恢复文件并切换/重载后允许扫描重新完成。
    std::fs::write(
        &right,
        "id,name
7,APPLE
",
    )
    .unwrap();
    v.cancel(); // 用户动作清除失败标记
    let start = Instant::now();
    while v.states[1].is_none() {
        v.poll();
        assert!(start.elapsed() < Duration::from_secs(15), "{}", v.message);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(v.states[1], Some(Importance::Important));
    let _ = std::fs::remove_file(left);
    let _ = std::fs::remove_file(right);
}

/// 多 sheet 场景制造一个 pending_side 对：访问 Summary 并编辑右侧 → 回 Data →
/// 用含 Summary 的新左文件替换左侧 → Summary 缓存对等待加载新左侧 sheet。
fn pending_pair_view(
    tag: &str,
) -> (
    TableCompareView,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let l0 = super::sheets::temp_xlsx(
        &format!("pl-{tag}-l0.xlsx"),
        &super::sheets::xlsx(&[("Data", "id,name\n7,apple"), ("Summary", "id,t\n1,x")]),
    );
    let r = super::sheets::temp_xlsx(
        &format!("pl-{tag}-r.xlsx"),
        &super::sheets::xlsx(&[("Data", "id,name\n7,apple"), ("Summary", "id,t\n1,y")]),
    );
    let mut v = open_files(&l0, &r);
    v.select_sheet(1);
    ready(&mut v);
    v.begin_edit(1, Cursor { row: 0, column: 1 }, Some("zed".into()));
    v.commit_edit().unwrap();
    ready(&mut v);
    v.select_sheet(0);
    ready(&mut v);
    let l1 = super::sheets::temp_xlsx(
        &format!("pl-{tag}-l1.xlsx"),
        &super::sheets::xlsx(&[
            ("Data", "id,name\n7,apple"),
            ("Other", "id,q\n3,z"),
            ("Summary", "id,t\n1,x2"),
        ]),
    );
    v.request(Pending::Open(0, l1.clone()));
    ready(&mut v);
    // pairs: Data↔Data, Other(左孤儿), Summary(2↔1，pending 加载新左侧)
    assert_eq!(v.pairs.0.len(), 3);
    let summary_idx = v
        .pairs
        .0
        .iter()
        .position(|p| p.right == Some(1) && p.left.is_some())
        .unwrap();
    assert_eq!(
        v.cached.get(&summary_idx).and_then(|p| p.pending_side),
        Some((0, 2)),
        "Summary pair waits for its replacement left sheet"
    );
    (v, l0, r, l1)
}

#[test]
fn pending_side_clears_after_load_and_edits_survive_a_round_trip() {
    let (mut v, l0, r, l1) = pending_pair_view("rt");
    v.select_sheet(2);
    ready(&mut v);
    assert_eq!(
        v.pair.as_ref().unwrap().pending_side,
        None,
        "successful publish clears pending"
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
        "x2"
    );
    // 编辑新目标侧 → 切走 → 再回：不得再从磁盘覆盖。
    v.begin_edit(0, Cursor { row: 0, column: 1 }, Some("edited".into()));
    v.commit_edit().unwrap();
    ready(&mut v);
    v.select_sheet(0);
    ready(&mut v);
    v.select_sheet(2);
    ready(&mut v);
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .left
            .borrow()
            .display_at(0, 1)
            .unwrap(),
        "edited",
        "revisiting must not reload over the new edit"
    );
    let _ = std::fs::remove_file(l0);
    let _ = std::fs::remove_file(r);
    let _ = std::fs::remove_file(l1);
}

#[test]
fn swap_flips_pending_side_and_the_visit_loads_the_correct_side() {
    let (mut v, l0, r, l1) = pending_pair_view("swap");
    v.swap();
    ready(&mut v);
    // Swap 后 pending 必须指向新右侧（原左文件换到右侧），保留侧编辑不动。
    v.select_sheet(2);
    ready(&mut v);
    let p = v.pair.as_ref().unwrap();
    assert_eq!(p.pending_side, None);
    assert_eq!(
        p.map.left.borrow().display_at(0, 1).unwrap(),
        "zed",
        "preserved edit now on the left"
    );
    assert_eq!(
        p.map.right.borrow().display_at(1, 1).unwrap(),
        "x2",
        "replacement sheet loaded for the new right side"
    );
    assert!(
        p.history[0].can_undo(),
        "history follows the preserved side"
    );
    let _ = std::fs::remove_file(l0);
    let _ = std::fs::remove_file(r);
    let _ = std::fs::remove_file(l1);
}

#[test]
fn save_and_writes_are_refused_while_the_target_side_is_pending() {
    let (mut v, l0, r, l1) = pending_pair_view("save");
    let before = std::fs::read(&l1).unwrap();
    v.select_sheet(2);
    // 加载进行中或失败后：保存 pending 侧被拒绝，且不取消加载/不写占位内容。
    let err = v.save_to(0, &l1, &no_backup_prefs(), false);
    assert!(err.is_err(), "saving a pending placeholder side is refused");
    assert!(v.editor.is_none());
    ready(&mut v);
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .left
            .borrow()
            .display_at(1, 1)
            .unwrap(),
        "x2",
        "the in-flight load was not cancelled by the save attempt"
    );
    assert_eq!(std::fs::read(&l1).unwrap(), before, "target file untouched");
    // 完成后导出恢复可用（workbook 来源走 CSV 出口）。
    let out = temp_file("pending-save.csv", "");
    std::fs::remove_file(&out).unwrap();
    v.save_to(0, &out, &no_backup_prefs(), true).unwrap();
    assert!(std::fs::read_to_string(&out).unwrap().contains("x2"));
    let _ = std::fs::remove_file(out);
    let _ = std::fs::remove_file(l0);
    let _ = std::fs::remove_file(r);
    let _ = std::fs::remove_file(l1);
}

#[test]
fn failed_pending_load_keeps_write_protection_and_recompare_restarts_the_load() {
    let (mut v, l0, r, l1) = pending_pair_view("fail");
    // 让 PairSide 加载失败：新左文件已删除。
    std::fs::remove_file(&l1).unwrap();
    v.select_sheet(2);
    let start = Instant::now();
    while v.worker.is_some() {
        v.poll();
        assert!(start.elapsed() < Duration::from_secs(15));
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!v.message.is_empty());
    assert_eq!(
        v.pair.as_ref().unwrap().pending_side,
        Some((0, 2)),
        "failed load keeps pending for retry"
    );
    assert!(!v.is_ready(), "placeholder must never report ready");
    // 写保护：不能往占位表写（失败后 staged 已清，只靠 pending 守卫）。
    v.grid.active = 0;
    v.grid.cursor = Cursor { row: 0, column: 0 };
    v.begin_edit(0, Cursor { row: 0, column: 0 }, Some("x".into()));
    assert!(v.editor.is_none(), "no editing into a placeholder");
    // 恢复文件后 Recompare 重启正确侧的加载，而不是把占位表当比较结果。
    let l1b = super::sheets::temp_xlsx(
        "pl-fail-l1b.xlsx",
        &super::sheets::xlsx(&[
            ("Data", "id,name\n7,apple"),
            ("Other", "id,q\n3,z"),
            ("Summary", "id,t\n1,BACK"),
        ]),
    );
    v.files[0].path = Some(l1b.clone());
    v.recompare();
    ready(&mut v);
    let p = v.pair.as_ref().unwrap();
    assert_eq!(p.pending_side, None);
    assert_eq!(p.map.left.borrow().display_at(1, 1).unwrap(), "BACK");
    assert_eq!(p.map.right.borrow().display_at(0, 1).unwrap(), "zed");
    let _ = std::fs::remove_file(l0);
    let _ = std::fs::remove_file(r);
    let _ = std::fs::remove_file(l1b);
}

#[test]
fn format_importance_checkbox_in_settings_toggles_the_format_layer() {
    // 值相同、数字格式不同：All 档默认 U，设置里可配为 I。
    let mut l = sheet("id,v\n1,6\n");
    let mut r = sheet("id,v\n1,6\n");
    let sa = l.intern_style(bc_engine_table::model::Style {
        number_format: Some("0.00".into()),
    });
    let sb = r.intern_style(bc_engine_table::model::Style {
        number_format: Some("0.000".into()),
    });
    let mut cell_l = Cell::number(6.0);
    cell_l.style = Some(sa);
    let mut cell_r = Cell::number(6.0);
    cell_r.style = Some(sb);
    l.set_cell(0, 1, cell_l).unwrap();
    r.set_cell(0, 1, cell_r).unwrap();
    let mut v = TableCompareView::from_sheets(
        l,
        r,
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            layers: Layers::All,
            ..Default::default()
        },
    );
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.unimportant, 1);
    let ctx = egui::Context::default();
    let click_columns_checkbox = |ctx: &egui::Context, v: &mut TableCompareView, apply: bool| {
        v.settings_open();
        frame(ctx, v, vec![]);
        let out = frame(ctx, v, vec![]);
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
        click(ctx, v, tab);
        let out = frame(ctx, v, vec![]);
        click(ctx, v, button(&out, "Format differences are important"));
        if apply {
            let out = frame(ctx, v, vec![]);
            click(ctx, v, button(&out, "Apply to current comparison"));
        }
    };
    // 取消路径：勾选但不 Apply → 关闭窗口 → 不变。
    click_columns_checkbox(&ctx, &mut v, false);
    v.settings.open = false;
    frame(&ctx, &mut v, vec![]);
    assert_eq!(
        v.pair.as_ref().unwrap().cache.statistics.unimportant,
        1,
        "cancel applies nothing"
    );
    // Apply：U → I。
    click_columns_checkbox(&ctx, &mut v, true);
    ready(&mut v);
    assert_eq!(
        v.pair.as_ref().unwrap().cache.statistics.important,
        1,
        "format layer now important"
    );
    // Ignore Unimportant 不隐藏重要差异。
    {
        let p = v.pair.as_mut().unwrap();
        p.map.rules_mut().ignore_unimportant = true;
    }
    v.project();
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.important, 1);
    // 值/公式档不受影响（格式层不参与）。
    {
        let p = v.pair.as_mut().unwrap();
        p.map.rules_mut().layers = Layers::Values;
    }
    v.project();
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.same, 1);
}

/// 缓存对（索引 1）只让指定侧脏。
fn push_dirty_cached(v: &mut TableCompareView, target_side: usize) {
    let mut cached = Pair {
        map: bc_engine_table::TableMap::new(
            sheet("id,t\n1,x\n"),
            sheet("id,t\n1,y\n"),
            TableRules::default(),
            &bc_core::cancel::NeverCancel,
        )
        .unwrap(),
        history: [EditStack::new(), EditStack::new()],
        cache: Default::default(),
        projection: Default::default(),
        pending_side: None,
    };
    let e = cached
        .map
        .modify_cell(
            if target_side == 0 {
                Side::Left
            } else {
                Side::Right
            },
            0,
            1,
            Cell::text("cached-target-edit"),
        )
        .unwrap();
    cached.history[target_side].push(e);
    v.cached.insert(1, cached);
    v.names = [vec!["a".into(), "b".into()], vec!["a".into(), "b".into()]];
    v.pairs.0.push(bc_engine_table::sheets::SheetPair {
        left: Some(1),
        right: Some(1),
    });
    v.states.push(None);
    v.summaries.push(None);
}

#[test]
fn save_decision_blocks_when_a_cached_target_side_is_dirty() {
    // 单侧替换确认框的 "Save"：缓存目标侧脏编辑不能伪装保存成功
    // （workbook sheet 无法写回），必须保持 pending 且不丢内容/历史。
    let mut v = view();
    push_dirty_cached(&mut v, 0); // 缓存对左侧（目标侧）脏
    v.request(Pending::Clipboard(0, "id,name\n9,kiwi\n".into()));
    assert!(v.pending.is_some());
    let ctx = egui::Context::default();
    dialog_click(&ctx, &mut v, "Save");
    assert!(
        v.pending.is_some(),
        "cached target-side dirty sheet cannot be saved in place: stay pending"
    );
    assert!(
        !v.message.is_empty(),
        "explicit export/cancel guidance shown"
    );
    let cached = v.cached.get(&1).unwrap();
    assert_eq!(
        cached.map.left.borrow().display_at(0, 1).unwrap(),
        "cached-target-edit"
    );
    assert!(cached.history[0].can_undo());
    // Cancel 后一切保留。
    dialog_click(&ctx, &mut v, "Cancel");
    assert!(v.pending.is_none());
    assert!(v.cached.get(&1).unwrap().history[0].can_undo());
}

#[test]
fn settings_with_changed_paths_repair_sheets_and_do_not_reuse_stale_indices() {
    // 停在第 2 页的双 sheet 文件，Specs 换成单 sheet 不同名的新文件：
    // Apply 后标签/内容/active 对应新文件，而不是旧索引越界得到的空表。
    let l0 = super::sheets::temp_xlsx(
        "sp-l0.xlsx",
        &super::sheets::xlsx(&[("Data", "id,name\n7,apple"), ("Summary", "id,t\n1,x")]),
    );
    let r0 = super::sheets::temp_xlsx(
        "sp-r0.xlsx",
        &super::sheets::xlsx(&[("Data", "id,name\n7,apple"), ("Summary", "id,t\n1,x")]),
    );
    let mut v = open_files(&l0, &r0);
    v.select_sheet(1);
    ready(&mut v);
    assert_eq!(v.active_pair, 1);
    let l1 = super::sheets::temp_xlsx(
        "sp-l1.xlsx",
        &super::sheets::xlsx(&[("Fresh", "id,q\n3,z")]),
    );
    let r1 = super::sheets::temp_xlsx(
        "sp-r1.xlsx",
        &super::sheets::xlsx(&[("Other", "id,q\n3,z2")]),
    );
    v.settings_open();
    let mut draft = v.settings.draft.clone().unwrap();
    draft.paths = [l1.display().to_string(), r1.display().to_string()];
    v.request(Pending::Settings(Box::new(draft)));
    ready(&mut v);
    assert_eq!(v.names[0], vec!["Fresh".to_string()]);
    assert_eq!(v.names[1], vec!["Other".to_string()]);
    assert_eq!(v.pairs.0.len(), 1);
    let p = v.pair.as_ref().unwrap();
    assert_eq!(p.map.left.borrow().display_at(1, 1).unwrap(), "z");
    assert_eq!(p.map.right.borrow().display_at(1, 1).unwrap(), "z2");
    assert_eq!(p.cache.statistics.important, 1);
    let _ = std::fs::remove_file(l0);
    let _ = std::fs::remove_file(r0);
    let _ = std::fs::remove_file(l1);
    let _ = std::fs::remove_file(r1);
}

#[test]
fn swap_transposes_an_uncommitted_draft_to_its_physical_side() {
    // 两侧同值单元格：草稿进行中 Swap，Commit 必须写回原物理文件。
    let mut v = view(); // 两侧 (0,0) 都是 "7"
    v.begin_edit(0, Cursor { row: 0, column: 0 }, Some("42".into()));
    assert!(v.editor.as_ref().is_some_and(|e| e.side == 0));
    v.find.last = Some((0, 0, 0..1));
    v.swap();
    assert!(
        v.editor.as_ref().is_some_and(|e| e.side == 1),
        "the draft follows its physical sheet"
    );
    assert!(
        v.find.last.is_none(),
        "find hits computed on the old side are invalidated"
    );
    v.commit_edit().unwrap();
    ready(&mut v);
    let p = v.pair.as_ref().unwrap();
    assert_eq!(
        p.map.right.borrow().display_at(0, 0).unwrap(),
        "42",
        "committed into the original left file"
    );
    assert_eq!(p.map.left.borrow().display_at(0, 0).unwrap(), "7");
    // Undo 作用于原编辑文件（现在在右侧）。
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
            .display_at(0, 0)
            .unwrap(),
        "7"
    );
}

#[test]
fn rules_only_settings_apply_does_not_silently_drop_an_open_draft() {
    let mut v = view();
    v.begin_edit(0, Cursor { row: 0, column: 1 }, Some("draft-text".into()));
    assert!(v.editor.as_ref().is_some_and(|e| e.changed()));
    // 只改规则（Handling），不触发 reload：草稿必须走确认而不是被静默丢弃。
    v.settings_open();
    v.settings.draft.as_mut().unwrap().rules.columns.0[1]
        .handling
        .unimportant = true;
    v.request(Pending::Settings(Box::new(
        v.settings.draft.clone().unwrap(),
    )));
    assert!(
        v.pending.is_some(),
        "open draft requires confirmation before Apply"
    );
    let ctx = egui::Context::default();
    dialog_click(&ctx, &mut v, "Cancel");
    assert_eq!(
        v.editor.as_ref().map(|e| e.text.as_str()),
        Some("draft-text"),
        "cancel keeps the draft"
    );
    // Discard 路径：明确丢弃后再应用。
    v.request(Pending::Settings(Box::new(
        v.settings.draft.clone().unwrap(),
    )));
    dialog_click(&ctx, &mut v, "Don't Save");

    ready(&mut v);
    assert!(v.editor.is_none());
    assert!(
        v.pair.as_ref().unwrap().map.rules().columns.0[1]
            .handling
            .unimportant
    );
}

#[test]
fn settings_with_changed_paths_publish_the_loaded_active_pair() {
    // 旧 active=1（Summary）；Specs 换成 3 个全新名 sheet 的文件：
    // Apply 后 active 必须跟随实际加载的第 0 对，标签与内容一致。
    let l0 = super::sheets::temp_xlsx(
        "ap-l0.xlsx",
        &super::sheets::xlsx(&[("Data", "id,name\n7,apple"), ("Summary", "id,t\n1,x")]),
    );
    let r0 = super::sheets::temp_xlsx(
        "ap-r0.xlsx",
        &super::sheets::xlsx(&[("Data", "id,name\n7,apple"), ("Summary", "id,t\n1,x")]),
    );
    let mut v = open_files(&l0, &r0);
    v.select_sheet(1);
    ready(&mut v);
    assert_eq!(v.active_pair, 1);
    let l1 = super::sheets::temp_xlsx(
        "ap-l1.xlsx",
        &super::sheets::xlsx(&[
            ("Alpha", "id,q\n3,aL"),
            ("Beta", "id,q\n4,bL"),
            ("Gamma", "id,q\n5,cL"),
        ]),
    );
    let r1 = super::sheets::temp_xlsx(
        "ap-r1.xlsx",
        &super::sheets::xlsx(&[
            ("Alpha", "id,q\n3,aR"),
            ("Beta", "id,q\n4,bR"),
            ("Gamma", "id,q\n5,cR"),
        ]),
    );
    v.settings_open();
    let mut draft = v.settings.draft.clone().unwrap();
    draft.paths = [l1.display().to_string(), r1.display().to_string()];
    v.request(Pending::Settings(Box::new(draft)));
    ready(&mut v);
    assert_eq!(
        v.active_pair, 0,
        "active follows the loaded pair, not the stale index"
    );
    assert_eq!(v.names[0], vec!["Alpha", "Beta", "Gamma"]);
    let p = v.pair.as_ref().unwrap();
    // 实际显示的是 Alpha 的内容（键 3），而不是旧 active=1 对应的 Beta（键 4）。
    assert_eq!(p.map.left.borrow().display_at(1, 1).unwrap(), "aL");
    assert_eq!(p.map.right.borrow().display_at(1, 1).unwrap(), "aR");
    assert_eq!(p.cache.statistics.important, 1);
    // 标签栏第一项与内容一致。
    let ctx = egui::Context::default();
    let out = frame(&ctx, &mut v, vec![]);
    let first_tab = out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text().starts_with('●') => {
                Some(t.galley.text().to_string())
            }
            _ => None,
        })
        .next();
    assert_eq!(first_tab.as_deref(), Some("● Alpha"));
    let _ = std::fs::remove_file(l0);
    let _ = std::fs::remove_file(r0);
    let _ = std::fs::remove_file(l1);
    let _ = std::fs::remove_file(r1);
}

#[test]
fn staged_transactions_do_not_spawn_scans_and_scan_resumes_afterwards() {
    let left = temp_file("staged-scan-l.csv", "id,name\n7,apple\n");
    let right = temp_file("staged-scan-r.csv", "id,name\n7,APPLE\n");
    let mut v = open_files(&left, &right);
    // 一个非缓存待扫描 sheet（第二对）。
    v.names = [vec!["a".into(), "b".into()], vec!["a".into(), "b".into()]];
    v.pairs.0.push(bc_engine_table::sheets::SheetPair {
        left: Some(1),
        right: Some(1),
    });
    v.states.push(None);
    v.summaries.push(None);
    // staged 事务进行中：即便有待扫描项也不得启动扫描（旧 paths/pairs 的结果
    // 没有 generation，可能污染新标签）。
    v.staged = Some(Staged::Reload);
    v.poll();
    assert!(
        v.scan.is_none(),
        "no scan while a staged transaction is in flight"
    );
    v.poll();
    assert!(v.scan.is_none());
    // 事务结束（取消/失败恢复稳定旧状态）：一次扫描正常完成。
    v.staged = None;
    v.poll();
    assert!(v.scan.is_some(), "stable pending sheet resumes scanning");
    let start = Instant::now();
    while v.states[1].is_none() {
        v.poll();
        assert!(start.elapsed() < Duration::from_secs(15));
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(v.states[1], Some(Importance::Important));
    // 完成后不重启。
    v.poll();
    assert!(v.scan.is_none());
    let _ = std::fs::remove_file(left);
    let _ = std::fs::remove_file(right);
}
