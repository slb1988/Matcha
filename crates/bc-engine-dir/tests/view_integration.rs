mod common;
use bc_core::Side;
use bc_engine_dir::{
    criteria::ContentCompare, filter::FolderDisplay, jobs::ContentCompareJob,
    op_exec::ConfirmPolicy, ops::*, refresh, view::ViewOptions, CompareStatus, Criteria,
};
use bc_vfs::{LocalFs, Vfs, VfsPath};
use common::*;
use std::{
    fs,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
fn flat() -> ViewOptions {
    ViewOptions {
        folders: FolderDisplay::Flatten,
        ..Default::default()
    }
}
#[test]
fn content_difference_keeps_newer_direction_and_skipped_tests_have_no_content_icon() {
    let t = Temp::new();
    t.write("left", "a", b"AAAA");
    t.write("right", "a", b"BBBB");
    for (side, time) in [("left", 1005), ("right", 1000)] {
        LocalFs::new()
            .set_modified(
                &VfsPath::local(&t.path(side, "a")),
                UNIX_EPOCH + Duration::from_secs(time),
            )
            .unwrap();
    }
    let vfs = manager(Arc::new(ProbeFs::default()));
    let mut tree = t.tree(&vfs);
    let id = tree.find("a").unwrap();
    let n = tree.node(id).unwrap();
    let job = ContentCompareJob {
        vfs: vfs.clone(),
        left: VfsPath::local(&t.path("left", "a")),
        right: VfsPath::local(&t.path("right", "a")),
        criteria: Criteria {
            content: ContentCompare::Binary,
            ..Default::default()
        },
        quick_status: n.quick_status,
        node: id,
        revision: n.revision,
    };
    let result = job.compare(&|| false).unwrap();
    assert!(result.apply(&mut tree));
    assert_eq!(tree.node(id).unwrap().status, CompareStatus::LeftNewer);
    assert_eq!(
        tree.node(id).unwrap().content_status,
        Some(CompareStatus::Different)
    );
    let criteria = Criteria {
        timestamp: None,
        content: ContentCompare::Binary,
        skip_content_if_quick_same: true,
        ..Default::default()
    };
    tree.evaluate_quick(&criteria);
    let n = tree.node(id).unwrap();
    let job = ContentCompareJob {
        criteria,
        quick_status: n.quick_status,
        revision: n.revision,
        ..job
    };
    let result = job.compare(&|| false).unwrap();
    assert!(result.skipped);
    result.apply(&mut tree);
    assert!(tree.node(id).unwrap().content_status.is_none());
}
#[test]
fn flattened_copy_exchange_and_move_use_the_displayed_partner_path() {
    let t = Temp::new();
    t.write("left", "alpha/same.txt", b"left");
    t.write("right", "beta/same.txt", b"right");
    let vfs = manager(Arc::new(ProbeFs::default()));
    let mut tree = t.tree(&vfs);
    let left = tree.find_side("alpha/same.txt", 0).unwrap();
    let mut selected = Selection::default();
    selected.insert(left, 0);
    let options = PlanOptions {
        view: flat(),
        ..Default::default()
    };
    let p = OpPlan::build(
        &tree,
        &selected,
        &DirOp::Copy {
            from: Side::Left,
            to: Side::Right,
        },
        options.clone(),
        SystemTime::now(),
    )
    .unwrap();
    assert!(p.items[0]
        .destination
        .as_ref()
        .unwrap()
        .path
        .to_local_path()
        .unwrap()
        .ends_with("beta/same.txt"));
    let (outcome, _) = execute(&p, &vfs, &context(None), ConfirmPolicy::YesToAll);
    assert_eq!(outcome.failed, 0);
    assert_eq!(fs::read(t.path("right", "beta/same.txt")).unwrap(), b"left");
    assert!(!t.path("right", "alpha/same.txt").exists());
    t.write("left", "alpha/same.txt", b"LEFT-NEW");
    t.write("right", "beta/same.txt", b"RIGHTNEW");
    refresh::full_refresh(&mut tree, &vfs, &|| false).unwrap();
    let p = OpPlan::build(
        &tree,
        &selected,
        &DirOp::Exchange,
        options.clone(),
        SystemTime::now(),
    )
    .unwrap();
    assert_eq!(p.items.len(), 1);
    assert_eq!(p.items[0].action, OpAction::Exchange);
    let (outcome, _) = execute(&p, &vfs, &context(None), ConfirmPolicy::YesToAll);
    assert_eq!(outcome.failed, 0);
    assert_eq!(
        fs::read(t.path("left", "alpha/same.txt")).unwrap(),
        b"RIGHTNEW"
    );
    assert_eq!(
        fs::read(t.path("right", "beta/same.txt")).unwrap(),
        b"LEFT-NEW"
    );
    refresh::full_refresh(&mut tree, &vfs, &|| false).unwrap();
    let p = OpPlan::build(
        &tree,
        &selected,
        &DirOp::Move {
            from: Side::Left,
            to: Side::Right,
        },
        options,
        SystemTime::now(),
    )
    .unwrap();
    let (outcome, _) = execute(&p, &vfs, &context(None), ConfirmPolicy::YesToAll);
    assert_eq!(outcome.failed, 0);
    assert!(!t.path("left", "alpha/same.txt").exists());
    assert_eq!(
        fs::read(t.path("right", "beta/same.txt")).unwrap(),
        b"RIGHTNEW"
    );
}
#[test]
fn virtual_content_is_revision_checked_without_rewriting_the_physical_tree() {
    let t = Temp::new();
    t.write("left", "alpha/f", b"AAAA");
    t.write("right", "beta/f", b"BBBB");
    let vfs = manager(Arc::new(ProbeFs::default()));
    let mut tree = t.tree(&vfs);
    let pair = [
        tree.find_side("alpha/f", 0).unwrap(),
        tree.find_side("beta/f", 1).unwrap(),
    ];
    let revisions = pair.map(|id| tree.node(id).unwrap().revision);
    assert!(tree.apply_virtual_content(pair, revisions, Some(CompareStatus::Different), None));
    assert_eq!(tree.view_rows(flat())[0].status, CompareStatus::Different);
    assert_eq!(
        tree.node(pair[0]).unwrap().status,
        CompareStatus::LeftOrphan
    );
    assert_eq!(
        tree.node(pair[1]).unwrap().status,
        CompareStatus::RightOrphan
    );
    let entry = tree.node(pair[1]).unwrap().sides[1].clone();
    tree.update_entry(1, "beta/f", entry).unwrap();
    assert!(!tree.apply_virtual_content(pair, revisions, Some(CompareStatus::Same), None));
    assert_eq!(tree.virtual_content_status(pair[0], pair[1]), None);
}
#[test]
fn aligned_folder_names_remain_side_qualified_during_replay_and_refresh() {
    let t = Temp::new();
    for (side, path, bytes) in [
        ("left", "A/item", b"LA".as_slice()),
        ("left", "Z/item", b"LEFT-Z".as_slice()),
        ("right", "A/item", b"RIGHT-A-LONG".as_slice()),
        ("right", "Z/item", b"RZ".as_slice()),
    ] {
        t.write(side, path, bytes);
    }
    let vfs = manager(Arc::new(ProbeFs::default()));
    let mut tree = t.tree(&vfs);
    tree.align_with("", "A", "Z").unwrap();
    tree.align_with("", "Z", "A").unwrap();
    let changed = tree.find_side("A/item", 1).unwrap();
    let other = tree.find_side("Z/item", 1).unwrap();
    assert_eq!(tree.side_relative(changed, 0), "Z/item");
    assert_eq!(tree.find("Z/item"), Some(changed));
    assert_eq!(tree.side_relative(other, 0), "A/item");
    let revision = tree.node(other).unwrap().revision;
    t.write("right", "A/item", b"UPDATED RIGHT A");
    let place = location(&tree, changed, 1).unwrap();
    refresh::refresh_locations(&mut tree, &vfs, &[place], &|| false).unwrap();
    assert_eq!(
        tree.node(changed).unwrap().sides[1].as_ref().unwrap().size,
        Some(15)
    );
    assert_eq!(tree.node(other).unwrap().revision, revision);
    refresh::full_refresh(&mut tree, &vfs, &|| false).unwrap();
    let changed = tree.find_side("A/item", 1).unwrap();
    assert_eq!(tree.side_relative(changed, 0), "Z/item");
}
