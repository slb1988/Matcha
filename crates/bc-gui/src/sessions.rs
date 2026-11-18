//! 命名会话（`docs/features/saved-sessions.md`）：Session › Save Session / Save Session As…、
//! Home 会话树「Saved」、命令行 `matcha "<会话名>"`。
//!
//! 会话库是 `bc_session::SessionManager` 管理的 `%APPDATA%\Matcha\BCSessions.xml`；这里只做
//! 视图 ↔ 会话的转换：保存两侧路径、文本对齐规则与 Show Context，打开时按会话类型建视图。
//! 快照/基准模式（`settings::disable_persistence`）不读不写会话库；单元测试按线程用临时文件。
use crate::egui;
use bc_session::folder::{FolderItem, SessionFolder};
use bc_session::{Session, SessionKind, SessionManager, Stamp};
use std::path::PathBuf;

fn library_path() -> Option<PathBuf> {
    if cfg!(test) {
        // 测试并行跑在不同线程：每个线程一个会话库，互不干扰。
        let thread = format!("{:?}", std::thread::current().id());
        let thread: String = thread.chars().filter(char::is_ascii_digit).collect();
        return Some(std::env::temp_dir().join(format!(
            "matcha-test-sessions-{}-{thread}.xml",
            std::process::id()
        )));
    }
    if !crate::settings::persistence_enabled() {
        return None;
    }
    SessionManager::default_path().ok()
}

/// 读会话库；没有会话库（快照模式）或读失败时为空树。
pub fn load() -> SessionManager {
    library_path()
        .and_then(|path| SessionManager::load(&path).ok())
        .unwrap_or_default()
}

/// 按名称查找已保存会话。
pub fn find(name: &str) -> Option<Session> {
    load().root.find(name).cloned()
}

pub fn exists(name: &str) -> bool {
    load().root.find(name).is_some()
}

fn writable() -> Result<SessionManager, String> {
    let path = library_path().ok_or("Sessions are not saved in this mode")?;
    SessionManager::load(&path).map_err(|e| format!("Cannot read {}: {e}", path.display()))
}

/// 保存（同名则更新）：沿用同名同类型会话里本项目不建模的内容（规则其余键、State 其余键），
/// 只覆盖路径、对齐规则、Show Context 与修改时间。
pub fn store(mut session: Session) -> Result<(), String> {
    let mut library = writable()?;
    let name = session.name.clone().ok_or("A saved session needs a name")?;
    if let Some(old) = library.root.find(&name).filter(|s| s.kind == session.kind) {
        let mut merged = old.clone();
        let keep_extra = std::mem::take(&mut merged.specs.extra);
        merged.specs = std::mem::take(&mut session.specs);
        merged.specs.extra = keep_extra;
        if let (Some(target), Some(alignment)) =
            (merged.rules.alignment_mut(), session.rules.alignment())
        {
            *target = alignment.clone();
        }
        for value in session.state.values.drain(..) {
            if let Some(v) = value.value {
                merged.state.set(&value.name, v);
            }
        }
        session = merged;
    }
    session.last_modified = Some(now());
    // 原位更新所在文件夹；新会话放在根下。
    let folder = folder_of(&library.root, &name).unwrap_or_default();
    library.upsert(&folder, session);
    library
        .save()
        .map_err(|e| format!("Cannot save the session: {e}"))
}

/// 删除已保存会话。
pub fn remove(name: &str) -> Result<(), String> {
    let mut library = writable()?;
    if library.remove(name) {
        library
            .save()
            .map_err(|e| format!("Cannot save the session library: {e}"))?;
    }
    Ok(())
}

/// 会话所在文件夹的名称路径（根下为空）。
fn folder_of(root: &SessionFolder, name: &str) -> Option<Vec<String>> {
    for item in &root.items {
        match item {
            FolderItem::Session(s) if s.name.as_deref() == Some(name) => return Some(Vec::new()),
            FolderItem::Folder(f) => {
                if let Some(mut path) = folder_of(f, name) {
                    path.insert(0, f.name.clone()?);
                    return Some(path);
                }
            }
            _ => {}
        }
    }
    None
}

/// 本地时钟的 `LastModified`。
pub fn now() -> Stamp {
    let t = bc_script::platform::local(std::time::SystemTime::now());
    Stamp {
        year: t.year,
        month: t.month as u8,
        day: t.day as u8,
        hour: t.hour as u8,
        minute: t.minute as u8,
        second: t.second as u8,
    }
}

/// 能在界面中打开的会话类型。
pub fn openable(kind: SessionKind) -> bool {
    matches!(
        kind,
        SessionKind::TextCompare
            | SessionKind::TableCompare
            | SessionKind::HexCompare
            | SessionKind::FolderCompare
            | SessionKind::FolderSync
    )
}

/// 会话两侧的本地路径（缺一侧或非本地路径为 `None`）。
pub fn pair(session: &Session) -> Option<(PathBuf, PathBuf)> {
    let local = |p: &Option<bc_vfs::VfsPath>| p.as_ref().and_then(|p| p.to_local_path());
    Some((local(&session.specs.left)?, local(&session.specs.right)?))
}

/// 新会话：`kind` + 两侧路径（空路径不写）。
pub fn session(kind: SessionKind, name: &str, paths: [Option<PathBuf>; 2]) -> Session {
    let mut session = Session::new(kind);
    session.name = Some(name.to_string());
    let [left, right] = paths.map(|p| {
        p.filter(|p| !p.as_os_str().is_empty())
            .map(|p| bc_vfs::VfsPath::local(&p))
    });
    session.specs.left = left;
    session.specs.right = right;
    session
}

/// 当前视图的会话（不含名称以外的界面状态）；Home 与合并视图没有可保存的会话。
pub(crate) fn from_view(view: &crate::app::View, name: &str) -> Option<Session> {
    use crate::app::View;
    let texts = |t: [String; 2]| t.map(|t| (!t.trim().is_empty()).then(|| PathBuf::from(t.trim())));
    Some(match view {
        View::TextCompare(v) => {
            let mut s = session(SessionKind::TextCompare, name, v.paths());
            let (alignment, show_context) = v.saved_settings();
            if let Some(target) = s.rules.alignment_mut() {
                *target = alignment;
            }
            s.state
                .set_bool(bc_session::state::SHOW_CONTEXT, show_context);
            s
        }
        View::TableCompare(v) => session(SessionKind::TableCompare, name, v.paths()),
        View::HexCompare(v) => session(SessionKind::HexCompare, name, v.paths()),
        View::FolderCompare(v) => session(SessionKind::FolderCompare, name, texts(v.path_texts())),
        View::FolderSync(v) => session(SessionKind::FolderSync, name, texts(v.folder.path_texts())),
        View::Home(_) | View::TextMerge(_) | View::TableMerge(_) => return None,
    })
}

/// 打开已保存会话对应的视图。
pub(crate) fn open_view(
    ctx: &egui::Context,
    session: &Session,
) -> Result<crate::app::View, String> {
    use crate::app::View;
    if !openable(session.kind) {
        return Err(format!(
            "{} sessions cannot be opened yet",
            session.kind.force_view_name()
        ));
    }
    let (left, right) = pair(session).ok_or("the saved session does not name two local paths")?;
    let err = |e: bc_core::CoreError| e.to_string();
    Ok(match session.kind {
        SessionKind::TableCompare => View::TableCompare(Box::new(
            crate::table::TableCompareView::open(&left, &right).map_err(err)?,
        )),
        SessionKind::HexCompare => View::HexCompare(Box::new(
            crate::hex::HexCompareView::open(&left, &right).map_err(err)?,
        )),
        SessionKind::FolderCompare => View::FolderCompare(Box::new(
            crate::dir_view::DirCompareView::open(ctx, &left, &right, false, false),
        )),
        SessionKind::FolderSync => View::FolderSync(Box::new(crate::sync_view::SyncView::open(
            ctx, &left, &right,
        ))),
        _ => {
            let mut view = crate::text::TextCompareView::open(&left, &right).map_err(err)?;
            let alignment = session.rules.alignment().cloned().unwrap_or_default();
            let show_context = session.state.get_bool(bc_session::state::SHOW_CONTEXT);
            view.apply_saved(alignment, show_context);
            View::TextCompare(Box::new(view))
        }
    })
}

/// Home 会话树里的一行已保存条目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub depth: usize,
    pub label: String,
    /// 会话名；文件夹为 `None`。
    pub session: Option<String>,
    /// 过滤用的全文（名称与两侧路径）。
    pub haystack: String,
    /// 悬停提示：会话类型与两侧完整路径；文件夹为 `None`。
    pub tip: Option<String>,
}

/// 树中的命名文件夹与命名会话（深度优先、文件内顺序）。无名文件夹（参考工具内部的
/// 自动保存区）与无名的按类型默认会话不列出。
pub fn entries(root: &SessionFolder) -> Vec<Entry> {
    fn walk(folder: &SessionFolder, depth: usize, out: &mut Vec<Entry>) {
        for item in &folder.items {
            match item {
                FolderItem::Folder(f) => {
                    if let Some(name) = &f.name {
                        out.push(Entry {
                            depth,
                            label: name.clone(),
                            session: None,
                            haystack: name.to_lowercase(),
                            tip: None,
                        });
                        walk(f, depth + 1, out);
                    }
                }
                FolderItem::Session(s) => {
                    if let Some(name) = &s.name {
                        let side = |p: &Option<bc_vfs::VfsPath>| {
                            p.as_ref().map(ToString::to_string).unwrap_or_default()
                        };
                        let (left, right) = (side(&s.specs.left), side(&s.specs.right));
                        out.push(Entry {
                            depth,
                            label: name.clone(),
                            session: Some(name.clone()),
                            haystack: format!("{name} {left} {right}").to_lowercase(),
                            tip: Some(crate::home::path_tip(
                                s.kind.force_view_name(),
                                &left,
                                &right,
                                None,
                            )),
                        });
                    }
                }
                FolderItem::Raw(_) => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(root, 0, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reset() {
        if let Some(path) = library_path() {
            let _ = std::fs::remove_file(path);
        }
    }

    #[test]
    fn store_find_update_and_remove() {
        reset();
        let mut first = session(
            SessionKind::TextCompare,
            "pair",
            [
                Some(PathBuf::from(r"C:\a.txt")),
                Some(PathBuf::from(r"C:\b.txt")),
            ],
        );
        first.state.set("FutureKey", "kept");
        store(first).unwrap();
        let saved = find("pair").unwrap();
        assert!(saved.last_modified.is_some());
        assert_eq!(pair(&saved).unwrap().1, PathBuf::from(r"C:\b.txt"));

        // Re-saving keeps unmodelled state and replaces the paths.
        let mut again = session(
            SessionKind::TextCompare,
            "pair",
            [
                Some(PathBuf::from(r"C:\a.txt")),
                Some(PathBuf::from(r"C:\c.txt")),
            ],
        );
        again.state.set_bool("ShowContext", false);
        store(again).unwrap();
        let saved = find("pair").unwrap();
        assert_eq!(saved.state.get("FutureKey"), Some("kept"));
        assert_eq!(saved.state.get_bool("ShowContext"), Some(false));
        assert_eq!(pair(&saved).unwrap().1, PathBuf::from(r"C:\c.txt"));
        assert_eq!(load().root.all_sessions().len(), 1);

        remove("pair").unwrap();
        assert!(!exists("pair"));
        reset();
    }

    #[test]
    fn entries_list_named_folders_and_sessions_only() {
        let xml = r#"<BCSessions>
            <TTextCompareSession Value="top"><Specs><Left Value="C:\L.txt"/></Specs></TTextCompareSession>
            <TSessionFolder Value="Work"><Items>
                <THexCompareSession Value="inner"/>
            </Items></TSessionFolder>
            <TSessionFolder><Items><TTextCompareSession Value="auto"/></Items></TSessionFolder>
            <TTextCompareSession/>
        </BCSessions>"#;
        let root = bc_session::xml::read_sessions(xml).unwrap();
        let rows: Vec<_> = entries(&root)
            .into_iter()
            .map(|e| (e.depth, e.label, e.session.is_some()))
            .collect();
        assert_eq!(
            rows,
            [
                (0, "top".to_string(), true),
                (0, "Work".to_string(), false),
                (1, "inner".to_string(), true)
            ]
        );
        assert!(entries(&root)[0].haystack.contains(r"c:\l.txt"));
    }
}
