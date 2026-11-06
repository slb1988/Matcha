use bc_engine_dir::{filter::*, filter_script::*, view::ViewOptions, CompareStatus::*, DirTree};
use bc_vfs::{EntryKind, FileAttrs, VfsEntry};
use std::time::{Duration, UNIX_EPOCH};

fn file(name: &str) -> VfsEntry {
    VfsEntry {
        name: name.into(),
        kind: EntryKind::File,
        size: Some(10),
        modified: Some(UNIX_EPOCH),
        attrs: FileAttrs::default(),
        version: None,
        crc32: None,
        error: None,
    }
}
fn folder(name: &str) -> VfsEntry {
    VfsEntry {
        kind: EntryKind::Dir,
        ..file(name)
    }
}
fn clock() -> FilterClock {
    FilterClock {
        today_midnight: parse_date("2026-11-05", 8 * 3600).unwrap(),
        utc_offset_seconds: 8 * 3600,
    }
}
fn yes(mask: &str, included: &[&str], excluded: &[&str]) {
    let filter = NameFilter::parse(mask).unwrap();
    for path in included {
        assert!(
            filter.matches(path, false),
            "{mask:?} should include {path:?}"
        );
    }
    for path in excluded {
        assert!(
            !filter.matches(path, false),
            "{mask:?} should exclude {path:?}"
        );
    }
}
#[test]
fn every_file_masks_help_example_and_spec_syntax() {
    yes("*.*", &["Makefile", "a.txt"], &[]);
    yes("*.html", &["index.html"], &["a.txt"]);
    yes("A?", &["Ab", "A猫"], &["A", "Abc"]);
    yes("A*", &["A", "Abc"], &["b"]);
    yes("[az]", &["a", "z"], &["b"]);
    yes("[a-z0-9]", &["a", "9"], &["@", "ab"]);
    yes("[!ac-g]", &["b", "h"], &["a", "c", "g"]);
    yes("[[]*.txt", &["[x.txt"], &["x.txt"]);
    yes("f", &["f", "p/f"], &["ff"]);
    yes("-f", &["g", "p/g"], &["f", "p/f"]);
    yes(r"p\", &["p/f", "other/p/f", "p/sub/a"], &["other/a"]);
    yes(r"-p\", &["other/a"], &["p/a", "other/p/sub/a"]);
    yes(
        "*.pas;*.dfm;*.dpr",
        &["x.pas", "x.dfm", "x.dpr"],
        &["x.cpp"],
    );
    yes(r"Source\", &["Source/a", "q/Source/sub/a"], &["a", "q/a"]);
    yes("-*.dcu;-*.ddp", &["a", "x.pas"], &["x.dcu", "q/x.ddp"]);
    yes(
        r"-Backup\",
        &["a", "Backup.txt"],
        &["Backup/a", "q/Backup/sub/a"],
    );
    yes("*.", &["Makefile", "x"], &["main.c", "a.b.c"]);
    yes("a?.", &["ab"], &["a.", "a.x"]);
    yes("[a-c]*.txt", &["apple.txt", "cat.txt"], &["dog.txt"]);
    yes(r"p\f", &["p/f", "q/p/f"], &["q/f", "p/sub/f"]);
    yes(r".\f", &["f"], &["q/f"]);
    yes(r"...\f", &["f", "q/f", "q/x/f"], &["other"]);
    yes(
        r"Windows\*.txt",
        &["Windows/a.txt", "q/Windows/a.txt"],
        &["Windows/sub/a.txt"],
    );
    yes(
        r"...\Windows\*.txt",
        &["Windows/a.txt", "q/Windows/a.txt"],
        &["Windows/sub/a.txt"],
    );
    yes(r".\Windows\*.txt", &["Windows/a.txt"], &["q/Windows/a.txt"]);
    for input in ["[", "[]", "[z-a]", "../x"] {
        assert!(NameFilter::parse(input).is_err());
    }
    assert!(NameFilter::parse_case("A", true)
        .unwrap()
        .matches("A", false));
    assert!(!NameFilter::parse_case("A", true)
        .unwrap()
        .matches("a", false));
}
#[test]
fn four_groups_intersect_exclusions_win_and_explicit_any_parent_escapes_include_gate() {
    yes(
        r"*.txt;Source\;-*.bak.txt;-Backup\",
        &["Source/a.txt", "x/Source/sub/a.txt"],
        &[
            "a.txt",
            "Source/a.bin",
            "Source/a.bak.txt",
            "Source/Backup/a.txt",
        ],
    );
    yes(
        r"Source\;...\Windows\*.txt;-Backup\",
        &["Windows/a.txt", "x/Windows/a.txt"],
        &["Source/a.bin", "Backup/Windows/a.txt"],
    );
    let names = NameFilter::parse(r"Source\;*.txt").unwrap();
    assert!(names.matches("", true));
    assert_eq!(NameFilter::parse(&names.to_spec()).unwrap(), names);
}
#[test]
fn metadata_content_filters_clock_protected_and_boundary_text() {
    let mut entry = file("a");
    let mut filters = OtherFilters::default();
    entry.attrs.hidden = true;
    entry.attrs.system = true;
    assert!(!filters.matches_metadata(&entry, clock()));
    filters.exclude_protected = false;
    assert!(filters.matches_metadata(&entry, clock()));
    filters.attrib_exclude = FileAttrs::parse_set("h");
    assert!(!filters.matches_metadata(&entry, clock()));
    filters.attrib_exclude = None;
    filters.attrib_include = FileAttrs::parse_set("r");
    assert!(!filters.matches_metadata(&entry, clock()));
    entry.attrs.readonly = true;
    assert!(filters.matches_metadata(&entry, clock()));
    filters.attrib_include = None;
    filters.cutoff = Some(Cutoff::OlderThanDays(7));
    entry.modified = Some(clock().today_midnight - Duration::from_secs(7 * 86400));
    assert!(filters.matches_metadata(&entry, clock()));
    entry.modified = entry.modified.map(|t| t - Duration::from_secs(1));
    assert!(!filters.matches_metadata(&entry, clock()));
    filters.cutoff = None;
    filters.size = Some(SizeLimit::LargerThan(9));
    assert!(!filters.matches_metadata(&entry, clock()));
    filters.content.push(ContentFilter {
        text: "标记".into(),
        contains: true,
        case_sensitive: false,
    });
    let text = format!("{}标记 end", "a".repeat(65535));
    assert!(!filters.matches_text(&text).unwrap()); // UTF-8 split across 64 KiB boundary
    assert!(filters.matches_text("ordinary text").unwrap());
    filters.content[0].contains = false;
    assert!(filters.matches_text(&text).unwrap());
    assert!(!filters.matches_text("ordinary text").unwrap());
    assert!(matches!(
        filters.matches_content(&mut std::io::Cursor::new(text), &|| true),
        Err(bc_vfs::VfsError::Cancelled)
    ));
    assert!(filters.has_items());
}
#[test]
fn thirteen_display_filters_truth_table() {
    // Unknown, Same, Different, LeftNewer, RightNewer, LeftOrphan, RightOrphan, Similar.
    let states = [
        Unknown,
        Same,
        Different,
        LeftNewer,
        RightNewer,
        LeftOrphan,
        RightOrphan,
        SimilarUnimportant,
    ];
    let expected = [
        "11111111", "00111111", "01000000", "11111001", "00111001", "00000110", "00010000",
        "00001000", "00010100", "00001010", "00000100", "00000010", "00000000",
    ];
    for (filter, expected) in DisplayFilter::ALL.into_iter().zip(expected) {
        for (state, bit) in states.into_iter().zip(expected.bytes()) {
            assert_eq!(filter.shows(state), bit == b'1', "{filter:?} {state:?}");
        }
        assert!(filter.script_name().starts_with("display-"));
    }
    assert!(DisplayFilter::Mismatches.shows(Error));
    assert!(!DisplayFilter::Mismatches.shows_with_importance(SimilarUnimportant, true));
    assert!(DisplayFilter::Matches.shows_with_importance(SimilarUnimportant, true));
}
#[test]
fn one_side_teal_suppress_and_no_comparison_mutation() {
    let mut tree = DirTree::new();
    let mut left = file("big.bin");
    left.size = Some(100);
    tree.apply_listing(0, "", vec![left]).unwrap();
    tree.apply_listing(1, "", vec![file("big.bin")]).unwrap();
    let other = OtherFilters {
        size: Some(SizeLimit::LargerThan(50)),
        ..Default::default()
    };
    tree.apply_filters(&NameFilter::default(), &other, clock(), |_, _| true);
    let rows = tree.view_rows(ViewOptions::default());
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].teal, [true, false]);
    assert_eq!(rows[0].status, Different);
    let options = ViewOptions {
        display: DisplayFilter::Matches,
        ..Default::default()
    };
    assert!(tree.view_rows(options).is_empty());
    let suppressed = tree.view_rows(ViewOptions {
        suppress_filters: true,
        ..options
    });
    assert_eq!(suppressed[0].teal, [true, true]);
    assert_eq!(
        tree.node(tree.find("big.bin").unwrap()).unwrap().status,
        Different
    );
}
#[test]
fn four_folder_modes_ancestors_and_deterministic_flatten_duplicates() {
    let mut tree = DirTree::new();
    for side in 0..2 {
        tree.apply_listing(side, "", vec![folder("a"), folder("b"), folder("empty")])
            .unwrap();
    }
    tree.apply_listing(0, "a", vec![file("same.txt")]).unwrap();
    tree.apply_listing(0, "b", vec![file("same.txt")]).unwrap();
    tree.apply_listing(1, "b", vec![file("same.txt")]).unwrap();
    for side in 0..2 {tree.apply_listing(side,"empty",vec![]).unwrap();}
    tree.set_expanded(None, true);
    tree.apply_filters(
        &NameFilter::parse("*.txt").unwrap(),
        &OtherFilters::default(),
        clock(),
        |_, _| true,
    );
    let files = tree.view_rows(ViewOptions::default());
    assert_eq!(files.len(), 4); // a/file, b/file; no empty folder
    assert_eq!(
        tree.view_rows(ViewOptions {
            folders: FolderDisplay::AlwaysShow,
            ..Default::default()
        })
        .len(),
        5
    );
    assert_eq!(
        tree.view_rows(ViewOptions {
            folders: FolderDisplay::FilesAndStructure,
            ..Default::default()
        })
        .len(),
        5
    );
    let flat = tree.view_rows(ViewOptions {
        folders: FolderDisplay::Flatten,
        ..Default::default()
    });
    assert_eq!(flat.len(), 2);
    assert_eq!(flat[0].status, Same);
    assert_eq!(flat[1].status, LeftOrphan);
    assert!(flat.iter().all(|r| r.node.is_none() && r.depth == 0));
    assert_eq!(
        tree.side_relative(flat[0].sides[0].unwrap(), 0),
        "a/same.txt"
    );
    assert_eq!(
        tree.side_relative(flat[0].sides[1].unwrap(), 1),
        "b/same.txt"
    );
    tree.apply_filters(
        &NameFilter::parse(r"Other\;...\b\*.txt").unwrap(),
        &OtherFilters::default(),
        clock(),
        |_, _| true,
    );
    assert_eq!(tree.view_rows(ViewOptions::default()).len(), 2); // b is promoted as a structural ancestor
    assert!(tree
        .view_rows(ViewOptions {
            display: DisplayFilter::None,
            ..Default::default()
        })
        .is_empty());
}
#[test]
fn seven_filter_script_examples_and_atomic_updates() {
    let examples = [
        r#"filter "*.pas;*.dpr;*.dfm""#,
        "filter cutoff:<7days",
        r#"filter cutoff:>"Dec 31, 2012""#,
        r#"filter cutoff:"12/31/2012 12:00pm""#,
        "filter attrib:-sh",
        r#"filter "*.txt;-My Folder\" attrib:+r"#,
        "filter size:>1MB",
    ];
    let mut settings = FilterSettings::default();
    for example in examples {
        settings.apply_script(example, clock()).unwrap();
    }
    assert_eq!(settings.other.size, Some(SizeLimit::LargerThan(1 << 20)));
    assert!(settings.names.matches("readme.txt", false));
    assert!(!settings.names.matches("My Folder/readme.txt", false));
    assert_eq!(parse_size("1.5GB").unwrap(), 3 << 29);
    assert_eq!(parse_size("1TB").unwrap(), 1 << 40);
    assert!(parse_size("18446744073709551615TB").is_err());
    let before = settings.clone();
    assert!(settings.apply_script("size:garbage", clock()).is_err());
    assert_eq!(before, settings);
    assert!(settings.apply_script("cutoff:2026-02-30", clock()).is_err());
    assert_eq!(before, settings);
    settings
        .apply_script(
            "cutoff:none attrib:none size:none include-protected",
            clock(),
        )
        .unwrap();
    assert!(!settings.other.has_items());
    assert!(!settings.other.exclude_protected);
    assert_eq!(parse_date("1970-01-01", 0).unwrap(), UNIX_EPOCH);
    assert_eq!(
        parse_date("2026-11-05", 8 * 3600).unwrap(),
        clock().today_midnight
    );
}
#[test]
fn presets_xml_roundtrip_validates_before_publication() {
    let mut presets = FilterPresets::default();
    presets.upsert("源码<&>", "*.rs;*.toml").unwrap();
    presets.upsert("No backup", r"-Backup\").unwrap();
    assert_eq!(FilterPresets::from_xml(&presets.to_xml()).unwrap(), presets);
    presets.upsert("源码<&>", "*.c;*.h").unwrap();
    assert_eq!(presets.items.len(), 2);
    assert!(presets.upsert("", "*.c").is_err());
    assert!(FilterPresets::from_xml("<Wrong/>").is_err());
}
