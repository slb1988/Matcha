//! Session › Compare Files Using：文本 / 表格 / 十六进制视图共用的子菜单、启用规则与切换动作。
//!
//! 参考语义：用另一类文件视图打开当前这对文件。可用性只由文件决定（见
//! [`bc_formats::route::can_open`]），各视图不再各自维护白名单：
//!
//! * 当前视图自身置灰；
//! * 两侧都要是磁盘文件（剪贴板 / 未保存的一侧不能重开）；
//! * 十六进制总可用；表格要求两侧都是表格或文本内容；文本要求两侧都是文本内容
//!   （xlsx 等工作簿不能用文本打开，csv 可以）；
//! * Media / Picture / Version Compare 尚未实现，恒灰。
use crate::chrome::menubar::{MenuItem, MenuItemKind};
use bc_formats::route::FileView;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const LABEL: &str = "Compare Files Using";
const PREFIX: &str = "Using:";

/// 子菜单项（参考顺序）与对应的已实现视图。
const VIEWS: [(&str, char, Option<FileView>); 6] = [
    ("Text Compare", 't', Some(FileView::Text)),
    ("Hex Compare", 'h', Some(FileView::Hex)),
    ("Media Compare", 'm', None),
    ("Picture Compare", 'p', None),
    ("Table Compare", 'b', Some(FileView::Table)),
    ("Version Compare", 'v', None),
];

/// 切换请求：以 `view` 原位重开这对文件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reopen {
    pub view: FileView,
    pub left: PathBuf,
    pub right: PathBuf,
}

/// Session 菜单里的 Compare Files Using 子菜单（命令 id 为 `Using:<label>`，
/// 与 New Session 下的同名项区分）。启用状态由 [`refresh`] 填。
pub fn submenu(mnemonic: Option<char>) -> MenuItem {
    let children = VIEWS
        .iter()
        .map(|(label, key, _)| MenuItem {
            mnemonic: Some(*key),
            enabled: false,
            ..MenuItem::action(*label, format!("{PREFIX}{label}"))
        })
        .collect();
    MenuItem {
        label: LABEL.into(),
        mnemonic,
        shortcut: None,
        kind: MenuItemKind::Submenu(children),
        enabled: false,
        action_id: LABEL.into(),
    }
}

/// 命令 id 对应的已实现视图；不是 Compare Files Using 的命令为 `None`。
pub fn view_of(action_id: &str) -> Option<FileView> {
    let label = action_id.strip_prefix(PREFIX)?;
    VIEWS
        .iter()
        .find(|(l, ..)| *l == label)
        .and_then(|(.., v)| *v)
}

/// 命令 id → 切换请求；不是 Compare Files Using 的命令为 `None`。
pub fn request(action_id: &str, paths: [Option<PathBuf>; 2]) -> Option<Result<Reopen, String>> {
    Some(reopen(view_of(action_id)?, paths))
}

/// 以 `view` 重开这对文件；两侧不全是磁盘文件时为 `Err`（状态栏提示）。
pub fn reopen(view: FileView, paths: [Option<PathBuf>; 2]) -> Result<Reopen, String> {
    match paths {
        [Some(left), Some(right)] => Ok(Reopen { view, left, right }),
        _ => Err("Save clipboard sides before using another file view".into()),
    }
}

/// 刷新菜单树中的 Compare Files Using 子菜单；返回是否找到并处理了它。
pub fn refresh(items: &mut [MenuItem], current: FileView, paths: &[Option<PathBuf>; 2]) -> bool {
    for item in items {
        if item.label == LABEL {
            refresh_item(item, current, paths);
            return true;
        }
        if let MenuItemKind::Submenu(children) = &mut item.kind {
            if refresh(children, current, paths) {
                return true;
            }
        }
    }
    false
}

/// 按规则设置子菜单各项与子菜单本身的启用状态。
pub fn refresh_item(item: &mut MenuItem, current: FileView, paths: &[Option<PathBuf>; 2]) {
    let MenuItemKind::Submenu(children) = &mut item.kind else {
        return;
    };
    let allowed = allowed(paths);
    for child in children.iter_mut() {
        child.enabled =
            view_of(&child.action_id).is_some_and(|v| v != current && allowed[index(v)]);
    }
    item.enabled = children.iter().any(|c| c.enabled);
}

fn index(view: FileView) -> usize {
    match view {
        FileView::Text => 0,
        FileView::Table => 1,
        FileView::Hex => 2,
    }
}

type Key = [Option<(PathBuf, Option<SystemTime>, u64)>; 2];

fn key_of(path: &Path) -> (PathBuf, Option<SystemTime>, u64) {
    let meta = std::fs::metadata(path).ok();
    (
        path.to_path_buf(),
        meta.as_ref().and_then(|m| m.modified().ok()),
        meta.map_or(0, |m| m.len()),
    )
}

/// 两侧都能以 Text / Table / Hex 打开吗。菜单逐帧刷新，结果按路径 + 修改时间 + 大小缓存
/// （内容嗅探要读文件头）。
fn allowed(paths: &[Option<PathBuf>; 2]) -> [bool; 3] {
    let [Some(left), Some(right)] = paths else {
        return [false; 3];
    };
    let key: Key = [Some(key_of(left)), Some(key_of(right))];
    thread_local! {
        static CACHE: RefCell<Option<(Key, [bool; 3])>> = const { RefCell::new(None) };
    }
    if let Some(hit) = CACHE.with(|c| {
        c.borrow()
            .as_ref()
            .filter(|(k, _)| *k == key)
            .map(|(_, v)| *v)
    }) {
        return hit;
    }
    let registry = crate::table::formats::load_registry()
        .unwrap_or_else(|_| bc_formats::registry::FormatRegistry::with_builtins());
    let can = |view| {
        [left, right]
            .iter()
            .all(|p| bc_formats::route::can_open(view, p, &registry))
    };
    let value = [
        can(FileView::Text),
        can(FileView::Table),
        can(FileView::Hex),
    ];
    CACHE.with(|c| *c.borrow_mut() = Some((key, value)));
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enabled(item: &MenuItem) -> Vec<&str> {
        let MenuItemKind::Submenu(children) = &item.kind else {
            panic!("submenu")
        };
        children
            .iter()
            .filter(|c| c.enabled)
            .map(|c| c.label.as_str())
            .collect()
    }

    #[test]
    fn availability_follows_the_files_not_the_view() {
        let book = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/reference-parity/table-report/corpus/key-none-rows-changed/"
        );
        let workbooks = [
            Some(PathBuf::from(format!("{book}left.xlsx"))),
            Some(PathBuf::from(format!("{book}right.xlsx"))),
        ];
        let mut item = submenu(Some('u'));
        // 表格视图里的 xlsx：可切十六进制，不能文本（工作簿不是文本），自身置灰。
        refresh_item(&mut item, FileView::Table, &workbooks);
        assert_eq!(enabled(&item), ["Hex Compare"]);
        assert!(item.enabled);
        // 十六进制视图里的 xlsx：可切表格。
        refresh_item(&mut item, FileView::Hex, &workbooks);
        assert_eq!(enabled(&item), ["Table Compare"]);

        let dir = std::env::temp_dir().join(format!("matcha-using-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a.csv"), dir.join("b.csv"));
        std::fs::write(&a, b"id,name\n1,x\n").unwrap();
        std::fs::write(&b, b"id,name\n1,y\n").unwrap();
        let csv = [Some(a), Some(b)];
        // csv：表格视图可切文本 / 十六进制；文本视图可切十六进制 / 表格。
        refresh_item(&mut item, FileView::Table, &csv);
        assert_eq!(enabled(&item), ["Text Compare", "Hex Compare"]);
        refresh_item(&mut item, FileView::Text, &csv);
        assert_eq!(enabled(&item), ["Hex Compare", "Table Compare"]);
        // 一侧不是磁盘文件：全部置灰，子菜单也灰。
        refresh_item(&mut item, FileView::Text, &[csv[0].clone(), None]);
        assert!(enabled(&item).is_empty() && !item.enabled);
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(view_of("Using:Hex Compare"), Some(FileView::Hex));
        assert_eq!(view_of("Using:Version Compare"), None);
        assert_eq!(view_of("Hex Compare"), None);
    }
}
