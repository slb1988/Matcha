//! Session/File/Edit/Search/View 菜单（命令逐项对照 `docs/reference-parity/hex-compare.md` H01–H34）。
//!
//! 数据驱动（chrome::menubar）；动作语义在 [`super::view::HexCompareView::command`]。
//! 已批准后置项（会话持久化、报告、父文件夹联动、外部程序、全局字体）置灰展示。

use crate::chrome::menubar::{MenuItem, MenuItemKind, MenuSpec};

fn item(label: &str, shortcut: Option<&str>, children: Vec<MenuItem>) -> MenuItem {
    MenuItem {
        label: label.into(),
        mnemonic: None,
        shortcut: shortcut.map(str::to_owned),
        kind: if children.is_empty() {
            MenuItemKind::Action
        } else {
            MenuItemKind::Submenu(children)
        },
        enabled: true,
        action_id: label.into(),
    }
}

fn off(label: &str) -> MenuItem {
    MenuItem {
        enabled: false,
        ..item(label, None, vec![])
    }
}

fn sep() -> MenuItem {
    MenuItem {
        label: String::new(),
        mnemonic: None,
        shortcut: None,
        kind: MenuItemKind::Separator,
        enabled: false,
        action_id: String::new(),
    }
}

fn check(label: &str, checked: bool) -> MenuItem {
    MenuItem {
        kind: MenuItemKind::Check { checked },
        ..item(label, None, vec![])
    }
}

fn radio(label: &str, selected: bool) -> MenuItem {
    MenuItem {
        kind: MenuItemKind::Radio { selected },
        ..item(label, None, vec![])
    }
}

/// H29：光标处字节的小/大端数值解释（菜单标签即内容，禁用展示）。
pub fn endian_items(bytes: &[u8], big: bool) -> Vec<MenuItem> {
    let read = |n: usize| -> Option<u64> {
        let b = bytes.get(..n)?;
        Some(if big {
            b.iter().fold(0u64, |a, &x| (a << 8) | u64::from(x))
        } else {
            b.iter().rev().fold(0u64, |a, &x| (a << 8) | u64::from(x))
        })
    };
    let signed = |v: u64, n: usize| {
        let bits = (n * 8) as u32;
        let s = v as i64;
        // 最高位为 1 时给出有符号值
        if bits < 64 && (v >> (bits - 1)) & 1 == 1 {
            Some(s - (1i64 << bits))
        } else {
            None
        }
    };
    let mut out = Vec::new();
    for n in [1usize, 2, 4] {
        let label = match read(n) {
            Some(v) => match signed(v, n) {
                Some(s) => format!("{n} 字节 = {v} (或 {s})"),
                None => format!("{n} 字节 = {v}"),
            },
            None => format!("{n} 字节 = —"),
        };
        out.push(off(&label));
    }
    let floats = match (read(4), read(8)) {
        (Some(a), Some(b)) => {
            let (f4, f8) = (f32::from_bits(a as u32), f64::from_bits(b));
            [
                format!("浮点单精度 = {f4:.6E}"),
                format!("浮点双精度 = {f8:.14E}"),
            ]
        }
        _ => ["浮点单精度 = —".into(), "浮点双精度 = —".into()],
    };
    out.extend(floats.iter().map(|f| off(f)));
    out
}

/// 视图状态驱动的菜单树。
pub fn menus(view: &crate::hex::view::HexCompareView) -> Vec<MenuSpec> {
    let cursor_bytes = {
        let side = view.active_side;
        let at = view.cursor[side];
        let bytes = if side == 0 {
            view.map.left.bytes()
        } else {
            view.map.right.bytes()
        };
        bytes.get(at..).unwrap_or(&[]).to_vec()
    };
    vec![
        MenuSpec {
            title: "Session".into(),
            mnemonic: Some('s'),
            items: vec![
                off("Save Session"),
                item("Session Settings...", None, vec![]),
                off("Locked"),
                off("Clear Session"),
                sep(),
                item("Swap Sides", None, vec![]),
                item("Reload Files", Some("F5"), vec![]),
                item("Recompare Files", Some("Ctrl+F5"), vec![]),
                sep(),
                off("Hex Compare Report..."), // 报告输出后置（H08）
                item("Hex Compare Info", Some("Ctrl+I"), vec![]),
                sep(),
                {
                    let mut using = crate::using::submenu(None);
                    crate::using::refresh_item(
                        &mut using,
                        bc_formats::route::FileView::Hex,
                        &view.paths(),
                    );
                    using
                },
                off("Compare Parent Folders"),
                sep(),
                item("Close Tab", Some("Ctrl+W"), vec![]),
            ],
        },
        MenuSpec {
            title: "File".into(),
            mnemonic: Some('f'),
            items: vec![
                item("Open File...", Some("Ctrl+O"), vec![]),
                item("Open Clipboard", Some("Shift+Ctrl+V"), vec![]),
                off("Open With"),
                sep(),
                item("Save File", Some("Ctrl+S"), vec![]),
                item("Save File As...", None, vec![]),
                off("Explorer"),
            ],
        },
        MenuSpec {
            title: "Edit".into(),
            mnemonic: Some('e'),
            items: vec![
                item("Undo", Some("Ctrl+Z"), vec![]),
                item("Redo", Some("Ctrl+Y"), vec![]),
                sep(),
                item("Cut", Some("Ctrl+X"), vec![]),
                item("Copy", Some("Ctrl+C"), vec![]),
                item("Paste", Some("Ctrl+V"), vec![]),
                item("Delete", Some("Del"), vec![]),
                sep(),
                item("Select All", Some("Ctrl+A"), vec![]),
                item("Copy to Other Side", Some("Ctrl+R"), vec![]),
            ],
        },
        MenuSpec {
            title: "Search".into(),
            mnemonic: Some('s'),
            items: vec![
                item("Next Difference Bytes", Some("Ctrl+N"), vec![]),
                item("Previous Difference Bytes", Some("Ctrl+P"), vec![]),
                sep(),
                item("Find...", Some("Ctrl+F"), vec![]),
                item("Replace...", Some("Ctrl+H"), vec![]),
                item("Go To...", Some("Ctrl+G"), vec![]),
            ],
        },
        MenuSpec {
            title: "View".into(),
            mnemonic: Some('v'),
            items: vec![
                radio(
                    "Show All",
                    view.filter == bc_engine_bin::hex::DisplayFilter::All,
                ),
                radio(
                    "Show Differences",
                    view.filter == bc_engine_bin::hex::DisplayFilter::Differences,
                ),
                radio(
                    "Show Same",
                    view.filter == bc_engine_bin::hex::DisplayFilter::Same,
                ),
                sep(),
                off(&format!("字节地址 = {:08X}", view.cursor[view.active_side])), // H28 动态标签
                MenuItem {
                    label: "Little Endian Values".into(),
                    mnemonic: None,
                    shortcut: None,
                    kind: MenuItemKind::Submenu(endian_items(&cursor_bytes, false)),
                    enabled: true,
                    action_id: "endian.little".into(),
                },
                MenuItem {
                    label: "Big Endian Values".into(),
                    mnemonic: None,
                    shortcut: None,
                    kind: MenuItemKind::Submenu(endian_items(&cursor_bytes, true)),
                    enabled: true,
                    action_id: "endian.big".into(),
                },
                sep(),
                check("Byte Addresses", view.show_byte_addresses),
                item("Layout...", None, vec![]),
                sep(),
                radio("Side-by-Side Layout", view.side_by_side),
                radio("Over-Under Layout", !view.side_by_side),
                sep(),
                check("Thumbnail", view.show_thumbnail),
                check("File Info", view.show_file_info),
                check("Toolbar", view.show_toolbar),
            ],
        },
    ]
}
