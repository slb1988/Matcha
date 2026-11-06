mod common;
use bc_engine_dir::{filter::FilterClock, op_script::SelectMask, ops::*};
use bc_vfs::{LocalFs, Vfs, VfsPath};
use common::*;
use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
fn clock() -> FilterClock {
    FilterClock {
        today_midnight: UNIX_EPOCH + Duration::from_secs(1_700_000_000),
        utc_offset_seconds: 0,
    }
}
#[test]
fn all_documented_operation_spellings_parse_and_build_plans() {
    let temp = Temp::new();
    temp.write("left", "abc1.txt", b"left");
    temp.write("right", "abc1.txt", b"right");
    let vfs = manager(Arc::new(ProbeFs::default()));
    let tree = temp.tree(&vfs);
    let selection = Selection::both([tree.find("abc1.txt").unwrap()]);
    let dest = temp
        .0
        .join("new destination")
        .to_string_lossy()
        .into_owned();
    let examples = vec![
        "copy left->right".into(),
        "copy right->left".into(),
        "move left->right".into(),
        "move right->left".into(),
        format!("copyto left path:base \"{dest}\""),
        format!("copyto right path:relative \"{dest}\""),
        format!("moveto left path:none \"{dest}\""),
        format!("copyto \"{dest}\""),
        "delete recyclebin=yes left".into(),
        "delete recyclebin=no right".into(),
        "delete all".into(),
        "rename *.bak".into(),
        r"rename regexpr (...)(...)\.txt $2$1.txt".into(),
        "touch left->right".into(),
        "touch right->left".into(),
        "touch all:now".into(),
        r#"touch left:"12/31/2012 12:00pm""#.into(),
        "attrib +h".into(),
        "attrib +sh -a".into(),
    ];
    for example in examples {
        let op = DirOp::parse_script(&example, clock()).unwrap();
        let planned = OpPlan::build(
            &tree,
            &selection,
            &op,
            Default::default(),
            SystemTime::now(),
        );
        // Copying both same-named sides to one external directory is an explicit collision,
        // not accidental last-writer-wins.
        if example.starts_with("copyto \"") {
            assert!(matches!(planned, Err(PlanError::Collision(_))));
        } else {
            assert!(planned.is_ok(), "{example}: {planned:?}");
        }
    }
    for invalid in [
        "copy all",
        "move left->left",
        "copyto path:bad x",
        "delete recyclebin=maybe left",
        "touch left:bad",
        "attrib +z",
        "rename regexpr [ bad",
        "copy left->right extra",
    ] {
        assert!(DirOp::parse_script(invalid, clock()).is_err(), "{invalid}");
    }
}
#[test]
fn selection_masks_examples_and_folder_homogeneity() {
    let temp = Temp::new();
    temp.write("left", "same", b"x");
    temp.write("right", "same", b"x");
    temp.write("left", "changed", b"longer");
    temp.write("right", "changed", b"x");
    temp.write("left", "newer", b"x");
    temp.write("right", "newer", b"x");
    temp.write("left", "orphan", b"x");
    temp.write("left", "mixed/same", b"x");
    temp.write("right", "mixed/same", b"x");
    temp.write("left", "mixed/orphan", b"x");
    std::fs::create_dir(temp.path("left", "empty")).unwrap();
    let t = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    for name in ["same", "changed", "newer", "mixed/same"] {
        for side in ["left", "right"] {
            LocalFs::new()
                .set_modified(&VfsPath::local(&temp.path(side, name)), t)
                .unwrap();
        }
    }
    LocalFs::new()
        .set_modified(
            &VfsPath::local(&temp.path("left", "newer")),
            t + Duration::from_secs(10),
        )
        .unwrap();
    let vfs = manager(Arc::new(ProbeFs::default()));
    let tree = temp.tree(&vfs);
    for example in [
        "select all",
        "select left",
        "select exact",
        "select files",
        "select right.diff",
        "select left.folders",
        "select newer.files right.older.files",
        "select empty.folders",
    ] {
        let selected = Selection::parse_script(example, &tree, Default::default()).unwrap();
        assert!(!selected.items.is_empty(), "{example}");
    }
    let ids =
        Selection::parse_script("select left.newer.files", &tree, Default::default()).unwrap();
    assert_eq!(ids.items, [(tree.find("newer").unwrap(), 0)].into());
    let ids =
        Selection::parse_script("select left.orphan.folders", &tree, Default::default()).unwrap();
    assert!(!ids.items.contains(&(tree.find("mixed").unwrap(), 0)));
    assert!(
        Selection::parse_script("select empty.folders", &tree, Default::default())
            .unwrap()
            .items
            .contains(&(tree.find("empty").unwrap(), 0))
    );
    for invalid in [
        "left.nope",
        "files.left",
        "newer.files.extra",
        "empty.files",
        "",
    ] {
        assert!(SelectMask::parse(invalid).is_err(), "{invalid}");
    }
}
