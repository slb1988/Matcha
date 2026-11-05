mod common;
use bc_engine_dir::{
    filter::{DisplayFilter, FilterClock},
    op_exec::{ConfirmPolicy, OpLog},
    refresh,
    sync::*,
    CompareStatus, NameFilter,
};
use bc_jobs::Confirmation;
use bc_vfs::{LocalFs, Vfs, VfsPath};
use common::*;
use std::{
    fs,
    sync::Arc,
    time::{Duration, UNIX_EPOCH},
};
fn timestamp(temp: &Temp, side: &str, name: &str, seconds: u64) {
    LocalFs::new()
        .set_modified(
            &VfsPath::local(&temp.path(side, name)),
            UNIX_EPOCH + Duration::from_secs(seconds),
        )
        .unwrap();
}
fn sample() -> (Temp, Arc<bc_vfs::VfsManager>, bc_engine_dir::DirTree) {
    let t = Temp::new();
    t.write("left", "A", b"new");
    t.write("right", "A", b"old");
    timestamp(&t, "left", "A", 1000);
    timestamp(&t, "right", "A", 900);
    t.write("left", "B", b"left orphan");
    t.write("right", "C", b"right orphan");
    let vfs = manager(Arc::new(ProbeFs::default()));
    let tree = t.tree(&vfs);
    (t, vfs, tree)
}
fn choice(plan: &SyncPlan, path: &str) -> SyncChoice {
    plan.items
        .iter()
        .find(|i| i.relative == path)
        .unwrap()
        .effective()
}
#[test]
fn all_five_presets_custom_mapping_and_invalid_direction() {
    let (_t, _vfs, tree) = sample();
    use SyncChoice::*;
    use SyncDirection::*;
    for (mode, expected) in [
        (
            SyncMode::Update(LeftToRight),
            [CopyLeftToRight, CopyLeftToRight, LeaveAlone],
        ),
        (
            SyncMode::Update(RightToLeft),
            [LeaveAlone, LeaveAlone, CopyRightToLeft],
        ),
        (
            SyncMode::Update(Both),
            [CopyLeftToRight, CopyLeftToRight, CopyRightToLeft],
        ),
        (
            SyncMode::Mirror(LeftToRight),
            [CopyLeftToRight, CopyLeftToRight, DeleteRight],
        ),
        (
            SyncMode::Mirror(RightToLeft),
            [CopyRightToLeft, DeleteLeft, CopyRightToLeft],
        ),
    ] {
        let plan = SyncPlan::build(&tree, mode, false, false).unwrap();
        assert_eq!(["A", "B", "C"].map(|p| choice(&plan, p)), expected);
    }
    let mode = SyncMode::Custom(CustomSyncRules {
        left_newer: DeleteLeft,
        left_orphan: LeaveAlone,
        right_orphan: DeleteRight,
        ..Default::default()
    });
    let plan = SyncPlan::build(&tree, mode, false, false).unwrap();
    assert_eq!(
        ["A", "B", "C"].map(|p| choice(&plan, p)),
        [DeleteLeft, LeaveAlone, DeleteRight]
    );
    assert!(SyncPlan::build(&tree, SyncMode::Mirror(Both), false, false).is_err());
}
#[test]
fn update_right_executes_preview_then_local_refresh_reports_same() {
    let (t, vfs, mut tree) = sample();
    let plan = SyncPlan::build(&tree, Default::default(), false, false).unwrap();
    let result = plan
        .execute_and_refresh(
            &mut tree,
            &vfs,
            &context(None),
            ConfirmPolicy::YesToAll,
            &OpLog::default(),
        )
        .unwrap();
    assert_eq!(result.statistics.copies, 2);
    assert_eq!(result.statistics.failed, 0);
    assert_eq!(result.statistics.skipped, 1);
    assert!(result.refresh_error.is_none());
    assert_eq!(fs::read(t.path("right", "A")).unwrap(), b"new");
    assert!(t.path("right", "B").exists());
    assert!(t.path("right", "C").exists());
    for path in ["A", "B"] {
        assert_eq!(
            tree.node(tree.find(path).unwrap()).unwrap().status,
            CompareStatus::Same
        );
    }
}
#[test]
fn override_exclude_and_actual_delete_reuse_operation_log() {
    let (t, vfs, mut tree) = sample();
    let mut plan = SyncPlan::build(
        &tree,
        SyncMode::Mirror(SyncDirection::LeftToRight),
        false,
        false,
    )
    .unwrap();
    plan.set_override(tree.find("C").unwrap(), SyncChoice::LeaveAlone)
        .unwrap();
    plan.exclude(tree.find("B").unwrap()).unwrap();
    let log = OpLog::default();
    let result = plan
        .execute_and_refresh(
            &mut tree,
            &vfs,
            &context(None),
            ConfirmPolicy::YesToAll,
            &log,
        )
        .unwrap();
    assert_eq!(result.statistics.copies, 1);
    assert_eq!(result.statistics.skipped, 2);
    assert!(t.path("right", "C").exists());
    assert!(!t.path("right", "B").exists());
    assert!(!log.snapshot().is_empty());
    let mut plan = SyncPlan::build(
        &tree,
        SyncMode::Mirror(SyncDirection::LeftToRight),
        false,
        false,
    )
    .unwrap();
    plan.options.recycle_bin = false;
    let result = plan
        .execute_and_refresh(
            &mut tree,
            &vfs,
            &context(None),
            ConfirmPolicy::YesToAll,
            &OpLog::default(),
        )
        .unwrap();
    assert_eq!(result.statistics.deletes, 1);
    assert!(!t.path("right", "C").exists());
}
#[test]
fn baseline_conflicts_no_direction_navigation_and_user_resolution() {
    let t = Temp::new();
    for name in ["a", "b"] {
        for side in ["left", "right"] {
            t.write(side, name, b"base");
            timestamp(&t, side, name, 1000);
        }
    }
    let vfs = manager(Arc::new(ProbeFs::default()));
    let initial = t.tree(&vfs);
    let baseline = SyncBaseline::capture(&initial);
    t.write("left", "a", b"left changed");
    timestamp(&t, "left", "a", 1010);
    t.write("right", "a", b"right changed");
    timestamp(&t, "right", "a", 1020);
    t.write("left", "b", b"size differs");
    timestamp(&t, "left", "b", 1000);
    let tree = t.tree(&vfs);
    let ordinary =
        SyncPlan::build(&tree, SyncMode::Update(SyncDirection::Both), false, false).unwrap();
    assert_eq!(choice(&ordinary, "a"), SyncChoice::CopyRightToLeft);
    let mut plan = SyncPlan::build_with(
        &tree,
        SyncMode::Update(SyncDirection::Both),
        Default::default(),
        Some(&baseline),
    )
    .unwrap();
    assert_eq!(choice(&plan, "a"), SyncChoice::LeaveAlone);
    assert_eq!(choice(&plan, "b"), SyncChoice::LeaveAlone);
    assert_eq!(plan.statistics().conflicts, 2);
    let first = plan.next_conflict(None).unwrap();
    let second = plan.next_conflict(Some(first)).unwrap();
    assert!(plan.next_conflict(Some(second)).is_none());
    assert_eq!(plan.previous_conflict(None), Some(second));
    assert_eq!(plan.previous_conflict(Some(second)), Some(first));
    plan.set_override(tree.find("a").unwrap(), SyncChoice::CopyRightToLeft)
        .unwrap();
    assert_eq!(plan.statistics().conflicts, 1);
    plan.clear_override(tree.find("a").unwrap()).unwrap();
    assert_eq!(plan.statistics().conflicts, 2);
    assert!(plan
        .set_override(tree.find("a").unwrap(), SyncChoice::Exclude)
        .is_ok());
    assert_eq!(plan.statistics().conflicts, 1);
}
#[test]
fn folders_aggregate_copy_delete_create_empty_visible_and_override() {
    let t = Temp::new();
    t.write("left", "d/new", b"new");
    t.write("right", "d/extra", b"extra");
    fs::create_dir(t.path("left", "empty")).unwrap();
    t.write("left", "hidden.obj", b"ignored");
    let vfs = manager(Arc::new(ProbeFs::default()));
    let mut tree = t.tree(&vfs);
    tree.apply_filters(
        &NameFilter::parse("-*.obj").unwrap(),
        &Default::default(),
        FilterClock {
            today_midnight: UNIX_EPOCH,
            utc_offset_seconds: 0,
        },
        |_, _| true,
    );
    let mode = SyncMode::Mirror(SyncDirection::LeftToRight);
    let mut plan = SyncPlan::build(&tree, mode, true, false).unwrap();
    let d = tree.find("d").unwrap();
    assert!(plan.folder_actions[&d].copy_left_to_right);
    assert!(plan.folder_actions[&d].delete_right);
    assert_eq!(choice(&plan, "hidden.obj"), SyncChoice::LeaveAlone);
    assert_eq!(choice(&plan, "empty"), SyncChoice::CopyLeftToRight);
    plan.set_override(d, SyncChoice::LeaveAlone).unwrap();
    assert!(!plan.folder_actions[&d].copy_left_to_right);
    assert!(!plan.folder_actions[&d].delete_right);
    let result = plan
        .execute_and_refresh(
            &mut tree,
            &vfs,
            &context(None),
            ConfirmPolicy::YesToAll,
            &OpLog::default(),
        )
        .unwrap();
    assert_eq!(result.statistics.creates, 1);
    assert!(t.path("right", "empty").is_dir());
    assert!(t.path("right", "d/extra").exists());
    assert!(!t.path("right", "d/new").exists());
    let mut options = SyncOptions {
        visible_only: true,
        ..Default::default()
    };
    options.operation.view.display = DisplayFilter::RightOrphans;
    let visible = SyncPlan::build_with(&tree, mode, options, None).unwrap();
    assert_eq!(choice(&visible, "d/new"), SyncChoice::LeaveAlone);
    assert_eq!(choice(&visible, "d/extra"), SyncChoice::DeleteRight);
    let no_empty = SyncPlan::build(&tree, mode, false, false).unwrap();
    assert_eq!(choice(&no_empty, "empty"), SyncChoice::LeaveAlone);
}
#[test]
fn mirror_orphan_directory_preserves_hidden_or_unplanned_children() {
    let t = Temp::new();
    t.write("right", "extra/remove", b"remove");
    t.write("right", "extra/hidden.obj", b"hidden");
    let vfs = manager(Arc::new(ProbeFs::default()));
    let mut tree = t.tree(&vfs);
    tree.apply_filters(
        &NameFilter::parse("-*.obj").unwrap(),
        &Default::default(),
        FilterClock {
            today_midnight: UNIX_EPOCH,
            utc_offset_seconds: 0,
        },
        |_, _| true,
    );
    let mut plan = SyncPlan::build(
        &tree,
        SyncMode::Mirror(SyncDirection::LeftToRight),
        false,
        false,
    )
    .unwrap();
    plan.options.recycle_bin = false;
    let result = plan
        .execute_and_refresh(
            &mut tree,
            &vfs,
            &context(None),
            ConfirmPolicy::YesToAll,
            &OpLog::default(),
        )
        .unwrap();
    assert_eq!(result.statistics.deletes, 1);
    assert_eq!(result.statistics.failed, 0);
    assert!(t.path("right", "extra/hidden.obj").exists());
    assert!(!t.path("right", "extra/remove").exists());
}
#[test]
fn sync_script_examples_and_rejected_mirror_all() {
    for example in [
        "sync create-empty mirror:left->right",
        "sync visible update:all",
        "sync update:right->left",
        "sync mirror:right->left",
        "sync update:left->right",
    ] {
        let command = SyncCommand::parse_script(example).unwrap();
        assert_eq!(
            SyncCommand::parse_script(&command.to_script().unwrap()).unwrap(),
            command
        );
    }
    for bad in [
        "sync mirror:all",
        "sync update:wrong",
        "sync visible",
        "sync update:all mirror:left->right",
        "sync unknown",
    ] {
        assert!(SyncCommand::parse_script(bad).is_err());
    }
}
#[test]
fn sync_failure_and_cancelled_statistics_preserve_the_baseline() {
    let (t, _vfs, mut tree) = sample();
    let probe = Arc::new(ProbeFs {
        deny_delete: Some("C".into()),
        ..Default::default()
    });
    let vfs = manager(probe);
    let baseline = SyncBaseline::capture(&tree);
    let original = baseline.entries.clone();
    let mut plan = SyncPlan::build_with(
        &tree,
        SyncMode::Mirror(SyncDirection::LeftToRight),
        Default::default(),
        Some(&baseline),
    )
    .unwrap();
    plan.options.recycle_bin = false;
    let result = plan
        .execute_and_refresh(
            &mut tree,
            &vfs,
            &context(None),
            ConfirmPolicy::YesToAll,
            &OpLog::default(),
        )
        .unwrap();
    assert_eq!(result.statistics.failed, 1);
    assert_eq!(result.statistics.copies, 2);
    assert!(t.path("right", "C").exists());
    assert_eq!(baseline.entries, original);
    let ctx = context(None);
    ctx.cancel.cancel();
    let plan = SyncPlan::build(
        &tree,
        SyncMode::Mirror(SyncDirection::LeftToRight),
        false,
        false,
    )
    .unwrap();
    let result = plan
        .execute_and_refresh(
            &mut tree,
            &vfs,
            &ctx,
            ConfirmPolicy::YesToAll,
            &OpLog::default(),
        )
        .unwrap();
    assert!(result.operations.cancelled);
    assert_eq!(result.statistics.deletes, 0);
    assert_eq!(result.statistics.skipped, 3);
}

#[test]
fn silent_refusal_and_confirmation_failures_are_not_reported_as_copies() {
    let (t, vfs, mut tree) = sample();
    let plan = SyncPlan::build(
        &tree,
        SyncMode::Mirror(SyncDirection::LeftToRight),
        false,
        false,
    )
    .unwrap();
    let answers = Answers::new([Confirmation::NoToAll, Confirmation::NoToAll]);
    let result = plan
        .execute_and_refresh(
            &mut tree,
            &vfs,
            &context(Some(answers)),
            ConfirmPolicy::Prompt,
            &OpLog::default(),
        )
        .unwrap();
    assert_eq!(result.statistics.copies, 1); // new orphan B needs no destructive confirmation
    assert_eq!(result.statistics.deletes, 0);
    assert_eq!(result.statistics.skipped, 2);
    assert_eq!(fs::read(t.path("right", "A")).unwrap(), b"old");
    assert!(t.path("right", "C").exists());
    refresh::full_refresh(&mut tree, &vfs, &|| false).unwrap();
}
