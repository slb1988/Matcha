//! 菜单库存由离线 tokens.json 转录；业务可用性在控制器中绑定。
use crate::chrome::menubar::{MenuItem, MenuItemKind, MenuSpec};
fn item(
    label: &str,
    mnemonic: Option<char>,
    shortcut: Option<&str>,
    children: Vec<MenuItem>,
) -> MenuItem {
    MenuItem {
        label: label.into(),
        mnemonic,
        shortcut: shortcut.map(str::to_owned),
        kind: if label.is_empty() {
            MenuItemKind::Separator
        } else if children.is_empty() {
            MenuItemKind::Action
        } else {
            MenuItemKind::Submenu(children)
        },
        enabled: false,
        action_id: label.into(),
    }
}
/// Toggle / Go to Bookmark 子菜单：标签为 `Bookmark N`，action_id 带父菜单前缀
/// （`Toggle Bookmark N` / `Go to Bookmark N`）以区分两类命令。
fn bookmarks(verb: &str, keys: &str) -> Vec<MenuItem> {
    (0..super::bookmarks::COUNT)
        .map(|n| {
            let digit = char::from_digit(n as u32, 10);
            MenuItem {
                action_id: format!("{verb} Bookmark {n}"),
                ..item(
                    &format!("Bookmark {n}"),
                    digit,
                    Some(&format!("{keys}{n}")),
                    vec![],
                )
            }
        })
        .collect()
}
pub(super) fn inventory() -> Vec<MenuSpec> {
    vec![
        MenuSpec {
            title: "Session".into(),
            mnemonic: Some('s'),
            items: vec![
                item(
                    "New Session",
                    Some('n'),
                    None,
                    vec![
                        item("Folder Compare", Some('f'), None, vec![]),
                        item("Folder Sync", Some('o'), None, vec![]),
                        item("Text Compare", Some('t'), None, vec![]),
                        item("Hex Compare", Some('h'), None, vec![]),
                        item("Media Compare", Some('m'), None, vec![]),
                        item("Picture Compare", Some('p'), None, vec![]),
                        item("Table Compare", Some('b'), None, vec![]),
                        item("Version Compare", Some('v'), None, vec![]),
                    ],
                ),
                item("New Tab", Some('t'), Some("Ctrl+T"), vec![]),
                item("New Window", Some('w'), None, vec![]),
                item("Open Session", Some('o'), Some("Shift+Ctrl+O"), vec![]),
                item("", None, None, vec![]),
                item(
                    "Load Workspace",
                    Some('k'),
                    None,
                    vec![item("Manage Workspaces...", None, None, vec![])],
                ),
                item("Save Workspace As...", Some('v'), None, vec![]),
                item("", None, None, vec![]),
                item("Save Session", Some('s'), Some("Shift+Ctrl+S"), vec![]),
                item("Save Session As...", Some('a'), None, vec![]),
                item("Session Settings...", Some('e'), None, vec![]),
                item("Locked", Some('l'), None, vec![]),
                item("Clear Session", Some('c'), Some("Shift+Ctrl+C"), vec![]),
                item("Close Tab", None, Some("Ctrl+W"), vec![]),
                item("", None, None, vec![]),
                item("Swap Sides", Some('d'), None, vec![]),
                item("Reload Files", Some('r'), Some("F5"), vec![]),
                item("Recompare Files", Some('m'), Some("Ctrl+F5"), vec![]),
                item("", None, None, vec![]),
                item("Text Compare Report...", Some('p'), None, vec![]),
                item("Text Compare Info", Some('i'), Some("Ctrl+I"), vec![]),
                item("", None, None, vec![]),
                crate::using::submenu(Some('u')),
                item("Compare Parent Folders", Some('f'), None, vec![]),
                item("", None, None, vec![]),
                item("Exit", Some('x'), Some("Ctrl+Q"), vec![]),
            ],
        },
        MenuSpec {
            title: "File".into(),
            mnemonic: Some('f'),
            items: vec![
                item("Open File...", Some('o'), Some("Ctrl+O"), vec![]),
                item("Open Clipboard", Some('c'), Some("Shift+Ctrl+V"), vec![]),
                item(
                    "Open With",
                    Some('w'),
                    None,
                    vec![
                        item("Associated Application", None, None, vec![]),
                        item("", None, None, vec![]),
                        item("Text Edit", Some('d'), None, vec![]),
                    ],
                ),
                item("Save File", Some('s'), Some("Ctrl+S"), vec![]),
                item("Save File As...", Some('a'), Some("F12"), vec![]),
                item("Explorer", Some('x'), None, vec![]),
            ],
        },
        MenuSpec {
            title: "Edit".into(),
            mnemonic: Some('e'),
            items: vec![
                item("Undo", Some('u'), Some("Ctrl+Z"), vec![]),
                item("Redo", Some('r'), Some("Ctrl+Y"), vec![]),
                item("", None, None, vec![]),
                item("Align With...", Some('w'), Some("F7"), vec![]),
                item("Isolate", Some('s'), None, vec![]),
                item("Copy to Right", Some('y'), Some("Ctrl+R"), vec![]),
                item(
                    "Copy Line to Right",
                    Some('y'),
                    Some("Shift+Ctrl+R"),
                    vec![],
                ),
                item("Increase Indent", Some('n'), Some("Shift+Ctrl+I"), vec![]),
                item("Decrease Indent", Some('e'), Some("Shift+Ctrl+U"), vec![]),
                item("", None, None, vec![]),
                item("Cut", Some('t'), Some("Ctrl+X"), vec![]),
                item("Copy", Some('c'), Some("Ctrl+C"), vec![]),
                item("Paste", Some('p'), Some("Ctrl+V"), vec![]),
                item("Delete", Some('d'), None, vec![]),
                item("", None, None, vec![]),
                item("Select All", Some('a'), Some("Ctrl+A"), vec![]),
                item("Select Section", Some('o'), Some("Ctrl+D"), vec![]),
                item("Compare Selection to Clipboard", Some('l'), None, vec![]),
                item("", None, None, vec![]),
                item(
                    "Convert File",
                    Some('v'),
                    None,
                    vec![
                        item("Trim Trailing Whitespace", Some('t'), None, vec![]),
                        item("Leading Spaces to Tabs", Some('l'), None, vec![]),
                        item("Tabs to Spaces", Some('s'), None, vec![]),
                        item(
                            "Line Endings",
                            Some('e'),
                            None,
                            vec![
                                item("To Windows/DOS", Some('w'), None, vec![]),
                                item("To Unix", Some('u'), None, vec![]),
                                item("To Macintosh", Some('m'), None, vec![]),
                            ],
                        ),
                    ],
                ),
            ],
        },
        MenuSpec {
            title: "Search".into(),
            mnemonic: Some('s'),
            items: vec![
                item("Next Difference", Some('t'), Some("Shift+Ctrl+N"), vec![]),
                item(
                    "Previous Difference",
                    Some('u'),
                    Some("Shift+Ctrl+P"),
                    vec![],
                ),
                item("Next Difference Section", Some('n'), Some("Ctrl+N"), vec![]),
                item(
                    "Previous Difference Section",
                    Some('p'),
                    Some("Ctrl+P"),
                    vec![],
                ),
                item("", None, None, vec![]),
                item("Next Edit", Some('e'), None, vec![]),
                item("Previous Edit", Some('d'), None, vec![]),
                item("", None, None, vec![]),
                item("Find...", Some('f'), Some("Ctrl+F"), vec![]),
                item("Replace...", Some('r'), Some("Ctrl+H"), vec![]),
                item("Find Next", Some('x'), Some("F3"), vec![]),
                item("Find Previous", Some('v'), Some("Shift+F3"), vec![]),
                item("", None, None, vec![]),
                item("Go To...", Some('g'), Some("Ctrl+G"), vec![]),
                item("", None, None, vec![]),
                item(
                    "Toggle Bookmark",
                    Some('b'),
                    None,
                    bookmarks("Toggle", "Shift+Ctrl+"),
                ),
                item(
                    "Go to Bookmark",
                    Some('k'),
                    None,
                    bookmarks("Go to", "Ctrl+"),
                ),
                item("Clear Bookmarks", Some('l'), None, vec![]),
            ],
        },
        MenuSpec {
            title: "View".into(),
            mnemonic: Some('v'),
            items: vec![
                item("Show All", None, None, vec![]),
                item("Show Differences", None, None, vec![]),
                item("Show Same", None, None, vec![]),
                item("", None, None, vec![]),
                item("Show Context", Some('c'), None, vec![]),
                item("Ignore Unimportant Differences", Some('u'), None, vec![]),
                item("", None, None, vec![]),
                item("Ignored", Some('g'), None, vec![]),
                item("", None, None, vec![]),
                item("Visible Whitespace", Some('w'), None, vec![]),
                item("Line Numbers", Some('n'), None, vec![]),
                item("Syntax Highlighting", Some('h'), None, vec![]),
                item("Word Wrap", Some('p'), None, vec![]),
                item("", None, None, vec![]),
                item("Side-by-Side Layout", Some('s'), None, vec![]),
                item("Over-Under Layout", Some('o'), None, vec![]),
                item("Webpages", Some('b'), None, vec![]),
                item("Thumbnail", Some('m'), None, vec![]),
                item("Text Details", Some('d'), None, vec![]),
                item("Hex Details", Some('x'), None, vec![]),
                item("Alignment Details", Some('a'), None, vec![]),
                item("Ruler", Some('r'), None, vec![]),
                item("File Info", Some('i'), None, vec![]),
                item("Toolbar", Some('t'), None, vec![]),
            ],
        },
        MenuSpec {
            title: "Tools".into(),
            mnemonic: Some('t'),
            items: vec![
                item("Options...", Some('o'), None, vec![]),
                item("File Formats...", Some('f'), None, vec![]),
                item("Profiles...", Some('p'), None, vec![]),
                item("", None, None, vec![]),
                item("Export Settings...", Some('e'), None, vec![]),
                item("Import Settings...", Some('i'), None, vec![]),
                item("Restore Factory Defaults...", Some('r'), None, vec![]),
                item("", None, None, vec![]),
                item("Save Snapshot...", Some('h'), None, vec![]),
                item("Edit Text File", Some('d'), None, vec![]),
                item("View Patch...", Some('a'), None, vec![]),
            ],
        },
        MenuSpec {
            title: "Help".into(),
            mnemonic: Some('h'),
            items: vec![
                item("Contents", Some('c'), None, vec![]),
                item("Context Sensitive Help", Some('h'), Some("F1"), vec![]),
                item("", None, None, vec![]),
                item("Project Website", Some('w'), None, vec![]),
                item("Check for Updates", Some('u'), None, vec![]),
                item("Support", Some('s'), None, vec![]),
                item("Enter Key...", Some('e'), None, vec![]),
                item("", None, None, vec![]),
                item("About Matcha", Some('a'), None, vec![]),
            ],
        },
    ]
}
/// 文本窗格右键菜单（`text-context.tree.json`）；`s` 为被点击的窗格，
/// 复制方向指向另一侧。全部初始禁用，由控制器按状态启用。
pub(super) fn context(s: usize) -> Vec<MenuItem> {
    let (copy, copy_key, line, line_key) = if s == 0 {
        (
            "Copy to Right",
            "Ctrl+R",
            "Copy Line to Right",
            "Shift+Ctrl+R",
        )
    } else {
        (
            "Copy to Left",
            "Ctrl+L",
            "Copy Line to Left",
            "Shift+Ctrl+L",
        )
    };
    vec![
        item("Align With...", Some('w'), None, vec![]),
        item("Isolate", Some('s'), None, vec![]),
        item(copy, Some('y'), Some(copy_key), vec![]),
        item(line, Some('y'), Some(line_key), vec![]),
        item("Increase Indent", Some('n'), Some("Shift+Ctrl+I"), vec![]),
        item("Decrease Indent", Some('e'), Some("Shift+Ctrl+U"), vec![]),
        item("", None, None, vec![]),
        item("Cut", Some('t'), Some("Ctrl+X"), vec![]),
        item("Copy", Some('c'), Some("Ctrl+C"), vec![]),
        item("Paste", Some('p'), Some("Ctrl+V"), vec![]),
        item("Delete", Some('d'), None, vec![]),
        item("", None, None, vec![]),
        item("Select Section", Some('o'), Some("Ctrl+D"), vec![]),
        item("Compare Selection to Clipboard", Some('l'), None, vec![]),
        item("", None, None, vec![]),
        item("Ignored", Some('g'), None, vec![]),
        item("", None, None, vec![]),
        item("Open File...", Some('o'), Some("Ctrl+O"), vec![]),
        item("Open Clipboard", Some('c'), Some("Shift+Ctrl+V"), vec![]),
        item(
            "Open With",
            Some('w'),
            None,
            vec![
                item("Associated Application", None, None, vec![]),
                item("", None, None, vec![]),
                item("Text Edit", Some('d'), None, vec![]),
            ],
        ),
        item("Save File", Some('s'), Some("Ctrl+S"), vec![]),
        item("Save File As...", Some('a'), Some("F12"), vec![]),
        item("Explorer", Some('x'), None, vec![]),
    ]
}
