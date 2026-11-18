//! `BCSessions.xml` read/write against a fixture shaped like files written by the
//! reference tool (structure copied from real files, paths anonymised).

use bc_core::align::AlignStrategy;
use bc_session::folder::FolderItem;
use bc_session::xml::{read_sessions, write_sessions};
use bc_session::{Session, SessionKind, SessionManager, SessionRules, Stamp};
use bc_vfs::VfsPath;

const FIXTURE: &str = include_str!("fixtures/BCSessions.xml");

/// Reference banner and CRLF are the only differences allowed on round trip.
fn normalise(xml: &str) -> String {
    xml.replace("\r\n", "\n")
        .lines()
        .filter(|l| !l.starts_with("<!--"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn reference_file_round_trips_byte_for_byte() {
    let tree = read_sessions(FIXTURE).unwrap();
    let written = write_sessions(&tree);
    assert_eq!(normalise(&written), normalise(FIXTURE));
    assert!(written.contains("<!-- Produced by Matcha -->"));
    assert_eq!(read_sessions(&written).unwrap(), tree);
}

#[test]
fn reads_tree_kinds_specs_rules_and_state() {
    let tree = read_sessions(FIXTURE).unwrap();
    let all = tree.all_sessions();
    assert_eq!(all.len(), 9, "unknown class is kept raw, not as a session");
    assert_eq!(tree.sessions().count(), 2);
    let outer = tree.folders().next().unwrap();
    assert_eq!(outer.name, None);
    let inner = outer.folders().next().unwrap();
    assert!(matches!(inner.items.last(), Some(FolderItem::Raw(n)) if n.name == "TExternalSession"));

    let quick = tree.find("quick-align").unwrap();
    assert_eq!(quick.kind, SessionKind::TextCompare);
    assert_eq!(
        quick.last_modified,
        Some(Stamp {
            year: 2026,
            month: 11,
            day: 6,
            hour: 1,
            minute: 0,
            second: 0
        })
    );
    let alignment = quick.rules.alignment().unwrap();
    assert_eq!(alignment.strategy, AlignStrategy::Quick);
    assert!(!alignment.use_closeness);
    assert_eq!(alignment.skew_tolerance, 2000);

    let chinese = tree.find("报告 - 副本.md <--> 报告.md").unwrap();
    assert_eq!(
        chinese.specs.left.as_ref().unwrap().as_str(),
        r"D:\docs\报告 - 副本.md"
    );
    assert_eq!(chinese.state.get_bool("ShowContext"), Some(true));

    let dir = tree.find("source <--> source").unwrap();
    assert_eq!(dir.kind, SessionKind::FolderCompare);
    assert!(matches!(dir.rules, SessionRules::Dir(_)));
    assert_eq!(dir.state.get("SortSide"), Some("dsRight"));
    assert_eq!(dir.specs.right.as_ref().unwrap().as_str(), r"e:\b\source\");

    let kinds: Vec<_> = inner.sessions().map(|s| s.kind).collect();
    assert_eq!(
        kinds,
        [
            SessionKind::TextCompare,
            SessionKind::FolderCompare,
            SessionKind::TableCompare,
            SessionKind::HexCompare,
            SessionKind::PictureCompare
        ]
    );
    let defaults: Vec<_> = outer.sessions().map(|s| (s.kind, s.name.clone())).collect();
    assert_eq!(
        defaults,
        [
            (SessionKind::FolderCompare, None),
            (SessionKind::TextCompare, None)
        ]
    );
}

#[test]
fn changed_alignment_rewrites_only_modelled_keys() {
    let mut tree = read_sessions(FIXTURE).unwrap();
    let FolderItem::Session(standard) = &mut tree.items[1] else {
        panic!("second root item is a session")
    };
    standard.rules.alignment_mut().unwrap().strategy = AlignStrategy::Patience;
    let written = write_sessions(&tree);
    assert!(written.contains("<AlignKind Value=\"alignPatience\"/>"));
    assert!(
        written.contains("<FutureKey Value=\"kept\"/>"),
        "unmodelled rule key kept"
    );
    assert!(written.contains("<Bookmarks Value=\"unmodelled element\"/>"));
    let back = read_sessions(&written).unwrap();
    let strategy = back
        .find("standard-align")
        .unwrap()
        .rules
        .alignment()
        .unwrap()
        .strategy;
    assert_eq!(strategy, AlignStrategy::Patience);
}

#[test]
fn new_session_writes_minimal_element() {
    let mut session = Session::pair(
        SessionKind::TableCompare,
        "a & b",
        VfsPath::parse(r"C:\a.csv").unwrap(),
        VfsPath::parse(r"C:\b.csv").unwrap(),
    );
    session.last_modified = Stamp::parse("2026-11-18 09:05:00");
    let mut root = bc_session::SessionFolder::default();
    root.items
        .push(FolderItem::Session(Box::new(session.clone())));
    let written = write_sessions(&root);
    assert_eq!(
        normalise(&written),
        normalise(concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<BCSessions Version=\"2\" MinVersion=\"2\">\n",
            "\t<TTableCompareSession Value=\"a &amp; b\">\n",
            "\t\t<LastModified Value=\"2026-11-18 09:05:00\"/>\n",
            "\t\t<Specs>\n",
            "\t\t\t<Left Value=\"C:\\a.csv\"/>\n",
            "\t\t\t<Right Value=\"C:\\b.csv\"/>\n",
            "\t\t</Specs>\n",
            "\t</TTableCompareSession>\n",
            "</BCSessions>\n",
        ))
    );
    assert_eq!(
        read_sessions(&written).unwrap().find("a & b"),
        Some(&session)
    );

    // Non-default alignment on a fresh session creates the `<Rules>` element.
    let mut text = Session::new(SessionKind::TextCompare);
    text.rules.alignment_mut().unwrap().never_align_differences = true;
    let node = bc_session::xml::session_to_node(&text);
    let rules = node.child("Rules").unwrap();
    assert_eq!(rules.value_str(), Some("TTextRules"));
    assert_eq!(rules.child_value("NeverAlignDiffs"), Some("True"));
}

#[test]
fn stamp_parse_rejects_malformed() {
    assert!(Stamp::parse("2026-11-18 09:05:00").is_some());
    for bad in [
        "",
        "2026-11-18",
        "2026-13-01 00:00:00",
        "2026-11-18 24:00:00",
        "x-1-1 0:0:0",
    ] {
        assert_eq!(Stamp::parse(bad), None, "{bad:?}");
    }
}

#[test]
fn rejects_other_root_and_keeps_unreadable_children() {
    assert!(read_sessions("<BCPreferences/>").is_err());
    let xml = "<BCSessions><TTextCompareSession Value=\"x\">\
        <LastModified Value=\"yesterday\"/>\
        <Rules Value=\"TTextRules\"><AlignKind Value=\"alignWeird\"/></Rules>\
        </TTextCompareSession></BCSessions>";
    let tree = read_sessions(xml).unwrap();
    let session = tree.find("x").unwrap();
    assert_eq!(session.last_modified, None);
    assert_eq!(session.unknown[0].value_str(), Some("yesterday"));
    // Unreadable rules: typed defaults, element written back untouched.
    assert_eq!(
        session.rules.alignment().unwrap().strategy,
        AlignStrategy::Smart
    );
    assert!(write_sessions(&tree).contains("alignWeird"));
}

#[test]
fn manager_upsert_remove_and_save() {
    let dir = std::env::temp_dir().join(format!("bc-session-mgr-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("nested").join("BCSessions.xml");
    let mut manager = SessionManager::load(&path).unwrap();
    assert!(manager.root.items.is_empty(), "missing file = empty tree");
    let pair = |name: &str, right: &str| {
        Session::pair(
            SessionKind::TextCompare,
            name,
            VfsPath::parse(r"C:\l.txt").unwrap(),
            VfsPath::parse(right).unwrap(),
        )
    };
    manager.upsert(&[], pair("one", r"C:\r1.txt"));
    manager.upsert(&["Work".into(), "Sub".into()], pair("two", r"C:\r2.txt"));
    // Same name in another folder moves it (names stay unique).
    manager.upsert(&["Work".into()], pair("one", r"C:\r3.txt"));
    assert!(manager.is_dirty());
    manager.save().unwrap();
    assert!(!manager.is_dirty());

    let mut loaded = SessionManager::load(&path).unwrap();
    assert_eq!(loaded.root.all_sessions().len(), 2);
    assert_eq!(loaded.root.sessions().count(), 0);
    let work = loaded.root.folders().next().unwrap();
    assert_eq!(work.name.as_deref(), Some("Work"));
    assert_eq!(
        work.find("one")
            .unwrap()
            .specs
            .right
            .as_ref()
            .unwrap()
            .as_str(),
        r"C:\r3.txt"
    );
    assert!(loaded.remove("two"));
    assert!(!loaded.remove("two"));
    loaded.save().unwrap();
    assert_eq!(
        SessionManager::load(&path)
            .unwrap()
            .root
            .all_sessions()
            .len(),
        1
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Real reference files (git-ignored, machine local): set `MATCHA_SESSION_SAMPLES`
/// to a folder tree containing `BCSessions.xml` files to round-trip them all.
#[test]
fn local_reference_samples_round_trip() {
    let Some(root) = std::env::var_os("MATCHA_SESSION_SAMPLES") else {
        return;
    };
    let mut stack = vec![std::path::PathBuf::from(root)];
    let mut checked = 0;
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().is_some_and(|n| n == "BCSessions.xml") {
                let text = std::fs::read_to_string(&path).unwrap();
                let tree = read_sessions(&text).unwrap();
                assert_eq!(
                    normalise(&write_sessions(&tree)),
                    normalise(&text),
                    "{path:?}"
                );
                checked += 1;
            }
        }
    }
    assert!(
        checked > 0,
        "no BCSessions.xml under MATCHA_SESSION_SAMPLES"
    );
}
