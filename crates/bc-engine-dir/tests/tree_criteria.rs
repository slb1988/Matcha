use bc_engine_dir::{
    criteria::{TimestampCriteria, TimezoneRule},
    tree::{AlignmentRule, PairingOptions},
    CompareStatus::*,
    ContentCompare, Criteria, DirTree, DisplayFilter,
};
use bc_vfs::{EntryKind, FileAttrs, VfsEntry};
use std::time::{Duration, UNIX_EPOCH};

fn file(name: &str) -> VfsEntry {
    VfsEntry {
        name: name.into(),
        kind: EntryKind::File,
        size: Some(5),
        modified: Some(UNIX_EPOCH + Duration::from_secs(100_000)),
        attrs: FileAttrs::default(),
        version: None,
        crc32: None,
        error: None,
    }
}
fn folder(name: &str) -> VfsEntry {
    VfsEntry {
        kind: EntryKind::Dir,
        size: None,
        ..file(name)
    }
}

#[test]
fn quick_criteria_are_independent_and_default_archive_is_ignored() {
    let a = file("README.md");
    let mut b = file("readme.md");
    let mut c = Criteria::default();
    assert_eq!(c.quick(Some(&a), Some(&b)), Same);
    b.attrs.archive = true;
    assert_eq!(c.quick(Some(&a), Some(&b)), Same);
    c.attrib = Some(FileAttrs {
        archive: true,
        ..Default::default()
    });
    assert_eq!(c.quick(Some(&a), Some(&b)), Different);
    c.attrib = None;
    c.compare_filename_case = true;
    assert_eq!(c.quick(Some(&a), Some(&b)), Different);
    c.compare_filename_case = false;
    b.size = Some(6);
    assert_eq!(c.quick(Some(&a), Some(&b)), Different);
    c.compare_size = false;
    assert_eq!(c.quick(Some(&a), Some(&b)), Same);
    assert_eq!(c.quick(Some(&a), None), LeftOrphan);
    assert_eq!(c.quick(None, Some(&b)), RightOrphan);
    b.error = Some("access denied".into());
    assert_eq!(c.quick(Some(&a), Some(&b)), Error);
}
#[test]
fn timestamp_tolerance_dst_timezone_and_direction() {
    let a = file("a");
    let mut b = a.clone();
    let mut c = Criteria::default();
    for (delta, expected) in [(1, Same), (2, Same), (3, RightNewer), (3600, RightNewer)] {
        b.modified = a.modified.map(|t| t + Duration::from_secs(delta));
        assert_eq!(c.quick(Some(&a), Some(&b)), expected);
    }
    c.timestamp = Some(TimestampCriteria {
        tolerance_secs: 2,
        ignore_dst: true,
    });
    assert_eq!(c.quick(Some(&a), Some(&b)), Same);
    b.modified = a.modified.map(|t| t + Duration::from_secs(7200));
    assert_eq!(c.quick(Some(&a), Some(&b)), RightNewer);
    c.timezone = TimezoneRule::Ignore;
    assert_eq!(c.quick(Some(&a), Some(&b)), Same);
    b.modified = a.modified.map(|t| t + Duration::from_secs(7201));
    assert_eq!(c.quick(Some(&a), Some(&b)), RightNewer);
    c.timezone = TimezoneRule::Offset {
        left: true,
        hours: 2,
    };
    assert_eq!(c.quick(Some(&a), Some(&b)), Same);
    c.timezone = TimezoneRule::None;
    assert_eq!(c.quick(Some(&b), Some(&a)), LeftNewer);
    c.timestamp = None;
    assert_eq!(c.quick(Some(&b), Some(&a)), Same);
}
#[test]
fn content_override_and_version_pipeline() {
    let mut c = Criteria::default();
    assert_eq!(c.combine(LeftNewer, Some(Same), None), Same);
    c.content_overrides_quick = false;
    assert_eq!(c.combine(LeftNewer, Some(Same), None), LeftNewer);
    assert_eq!(c.combine(Same, Some(Different), None), Different);
    assert_eq!(
        c.combine(Same, Some(SimilarUnimportant), None),
        SimilarUnimportant
    );
    c.ignore_unimportant = true;
    assert_eq!(c.combine(Same, Some(SimilarUnimportant), None), Same);
    assert_eq!(c.combine(Same, Some(Same), Some(false)), Different);
}
#[test]
fn criteria_script_roundtrip_and_validation() {
    let input = "criteria attrib:sh timestamp:2sec;IgnoreDST rules-based timezone:left+6";
    let c = Criteria::parse_script(input).unwrap();
    assert_eq!(c.content, ContentCompare::RulesBased);
    assert_eq!(
        c.timezone,
        TimezoneRule::Offset {
            left: true,
            hours: 6
        }
    );
    assert!(c.timestamp.unwrap().ignore_dst);
    assert_eq!(Criteria::parse_script(&c.to_script()).unwrap(), c);
    for input in [
        "",
        "version CRC follow-symlinks ignore-unimportant",
        "timestamp:;IgnoreDST size",
        "timestamp binary timezone:ignore",
        "no-size filename-case skip-quick-same no-content-override",
        "rules-based comments-unimportant comments-unimportant",
    ] {
        let c = Criteria::parse_script(input).unwrap();
        assert_eq!(Criteria::parse_script(&c.to_script()).unwrap(), c);
    }
    for input in [
        "binary CRC",
        "attrib:z",
        "timezone:left+13",
        "timezone:right2",
        "timestamp:bad",
        "unknown",
    ] {
        assert!(Criteria::parse_script(input).is_err(), "{input}");
    }
}
#[test]
fn tree_pairing_summary_cache_refresh_and_expansion() {
    let mut tree = DirTree::new();
    tree.set_pairing(PairingOptions {
        case_sensitive: false,
        ..Default::default()
    })
    .unwrap();
    for side in 0..2 {
        tree.apply_listing(side, "", vec![folder("src")]).unwrap();
    }
    let src = tree.find("src").unwrap();
    assert!(tree.node(src).unwrap().hollow());
    assert!(tree.node(src).unwrap().yellow(false));
    tree.apply_listing(0, "src", vec![file("README.md"), file("orphan")])
        .unwrap();
    let mut newer = file("readme.md");
    newer.modified = newer.modified.map(|t| t + Duration::from_secs(5));
    tree.apply_listing(1, "src", vec![newer]).unwrap();
    let node = tree.node(src).unwrap();
    assert!(node.summary.red && node.summary.purple && !node.hollow());
    assert_eq!(tree.visible_rows(DisplayFilter::All).len(), 1);
    tree.set_expanded(None, true);
    assert_eq!(tree.visible_rows(DisplayFilter::All).len(), 3);
    assert_eq!(
        tree.visible_rows(DisplayFilter::All),
        tree.visible_rows(DisplayFilter::All)
    );
    assert_eq!(tree.count_status(RightNewer), 1);
    tree.apply_listing(0, "src", vec![]).unwrap();
    assert_eq!(tree.count_status(RightOrphan), 1);
    assert_eq!(tree.visible_rows(DisplayFilter::All).len(), 2);
    tree.set_expanded(None, false);
    assert_eq!(tree.visible_rows(DisplayFilter::All).len(), 1);
}
#[test]
fn extension_nfc_overrides_and_align_with_survive_rescan() {
    let mut tree = DirTree::new();
    tree.set_pairing(PairingOptions {
        ignore_extension: true,
        ..Default::default()
    })
    .unwrap();
    tree.apply_listing(0, "", vec![file("logo.gif")]).unwrap();
    tree.apply_listing(1, "", vec![file("logo.png")]).unwrap();
    assert_eq!(tree.root().children.len(), 1);
    for side in 0..2 {
        tree.apply_listing(side, "", vec![folder("images")])
            .unwrap();
    }
    tree.apply_listing(0, "images", vec![file("x1.jpg"), file("x2.jpg"), file("a")])
        .unwrap();
    tree.apply_listing(1, "images", vec![file("y1.jpg"), file("y2.jpg"), file("b")])
        .unwrap();
    tree.add_alignment(
        AlignmentRule::new("x*.jpg", "y*.jpg", false, Some("images".into())).unwrap(),
    )
    .unwrap();
    tree.align_with("images", "a", "b").unwrap();
    let images = tree.find("images").unwrap();
    assert_eq!(tree.node(images).unwrap().children.len(), 3);
    tree.apply_listing(1, "images", vec![file("y1.jpg"), file("y2.jpg"), file("b")])
        .unwrap();
    assert_eq!(tree.node(images).unwrap().children.len(), 3);
    for &id in &tree.node(images).unwrap().children {
        assert!(!tree.node(id).unwrap().is_orphan());
    }

    let mut tree = DirTree::new();
    tree.add_alignment(
        AlignmentRule::new(r"left-(\d+)\.txt", r"right-(\d+)\.txt", true, None).unwrap(),
    )
    .unwrap();
    tree.apply_listing(0, "", vec![file("left-42.txt")])
        .unwrap();
    tree.apply_listing(1, "", vec![file("right-42.txt")])
        .unwrap();
    assert_eq!(tree.root().children.len(), 1);
    #[cfg(windows)]
    {
        tree.set_pairing(PairingOptions {
            normalize_nfc: true,
            ..Default::default()
        })
        .unwrap();
        tree.apply_listing(0, "", vec![file("café")]).unwrap();
        tree.apply_listing(1, "", vec![file("cafe\u{301}")])
            .unwrap();
        assert_eq!(tree.root().children.len(), 1);
    }
}
#[test]
fn refreshed_case_only_name_and_removed_subtrees_do_not_leave_stale_nodes() {
    let mut tree = DirTree::new();
    tree.set_pairing(PairingOptions {
        case_sensitive: false,
        ..Default::default()
    })
    .unwrap();
    tree.apply_listing(0, "", vec![file("Foo")]).unwrap();
    tree.apply_listing(1, "", vec![file("Foo")]).unwrap();
    tree.apply_listing(0, "", vec![file("foo")]).unwrap();
    assert_eq!(tree.root().children.len(), 1);
    assert_eq!(tree.count_status(Same), 1);
    for side in 0..2 {
        tree.apply_listing(side, "", vec![folder("a")]).unwrap();
    }
    tree.apply_listing(0, "a", vec![file("deleted")]).unwrap();
    tree.apply_listing(0, "", vec![]).unwrap();
    assert!(tree.find("a/deleted").is_none());
    assert_eq!(tree.count_status(LeftOrphan), 0);
}

#[test]
fn revision_check_blocks_late_content_and_summary_can_clear() {
    let mut tree = DirTree::new();
    tree.apply_listing(0, "", vec![file("a")]).unwrap();
    tree.apply_listing(1, "", vec![file("a")]).unwrap();
    let id = tree.find("a").unwrap();
    let revision = tree.node(id).unwrap().revision;
    assert!(tree.apply_content(id, revision, Different, None));
    assert!(tree.root().summary.red);
    assert!(tree.apply_content(id, revision, Same, None));
    assert!(!tree.root().summary.red);
    tree.apply_listing(0, "", vec![file("a")]).unwrap();
    assert!(!tree.apply_content(id, revision, Different, None));
    let old = tree.find("a").unwrap();
    let revision = tree.node(old).unwrap().revision;
    tree.align_with("", "a", "a").unwrap();
    assert!(!tree.apply_content(old, revision, Different, None));
}
