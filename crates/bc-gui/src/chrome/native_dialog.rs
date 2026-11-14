//! 系统原生文件/文件夹选择对话框（Windows 上为 IFileOpenDialog / IFileSaveDialog，
//! 与资源管理器同款）。所有「Browse / Open File... / Save File As...」入口统一走这里，
//! 标题统一为「<动作> - Left Side / Right Side」，初始目录与文件名取自当前路径。
//!
//! 对话框同步阻塞当前帧（系统模态：属主为本进程的前台窗口，句柄 FFI 在 `bc_shell::dialog_owner`）；测试构建不弹窗，
//! 由 [`mock`] 注入返回值并记录请求。
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    OpenFile,
    SaveFile,
    PickFolder,
}

/// 一次对话框请求。`filters` 为 (显示名, 扩展名列表)；空列表时只有「All files (*.*)」。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub kind: Kind,
    pub title: String,
    pub initial: Option<PathBuf>,
    pub filters: Vec<(String, Vec<String>)>,
}

impl Request {
    pub fn new(kind: Kind, title: impl Into<String>) -> Self {
        Self {
            kind,
            title: title.into(),
            initial: None,
            filters: Vec::new(),
        }
    }
    /// 初始位置：文件路径 → 其所在目录 + 预填文件名；目录路径 → 该目录；
    /// 空串忽略。不存在的路径退到最近的已存在上级目录。
    pub fn initial(mut self, path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        if !path.as_os_str().is_empty() {
            self.initial = Some(path.to_path_buf());
        }
        self
    }
    pub fn initial_opt(self, path: Option<&Path>) -> Self {
        match path {
            Some(p) => self.initial(p),
            None => self,
        }
    }
    pub fn filter(mut self, name: &str, extensions: &[&str]) -> Self {
        self.filters.push((
            name.into(),
            extensions.iter().map(|e| (*e).to_owned()).collect(),
        ));
        self
    }
    /// 弹出对话框；用户取消返回 None。
    pub fn show(self) -> Option<PathBuf> {
        #[cfg(test)]
        {
            mock::take(self)
        }
        #[cfg(not(test))]
        {
            show_native(self)
        }
    }
}

/// 「Left Side / Right Side」标题后缀（与参考的 Open File on File System - Left Side 一致）。
pub fn side_title(action: &str, side: usize) -> String {
    format!(
        "{action} - {}",
        if side == 0 { "Left Side" } else { "Right Side" }
    )
}

/// 拆出 (起始目录, 预填文件名)。
fn split_initial(kind: Kind, path: &Path) -> (Option<PathBuf>, Option<String>) {
    if path.is_dir() {
        return (Some(path.to_path_buf()), None);
    }
    let name = (kind != Kind::PickFolder)
        .then(|| path.file_name().map(|n| n.to_string_lossy().into_owned()))
        .flatten();
    let dir = path
        .ancestors()
        .skip(1)
        .find(|p| p.is_dir())
        .map(Path::to_path_buf);
    (dir, name)
}

#[cfg(not(test))]
fn show_native(request: Request) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new().set_title(&request.title);
    if let Some(initial) = &request.initial {
        let (dir, name) = split_initial(request.kind, initial);
        if let Some(dir) = dir {
            dialog = dialog.set_directory(dir);
        }
        if let Some(name) = name {
            dialog = dialog.set_file_name(name);
        }
    }
    if request.kind != Kind::PickFolder {
        for (name, extensions) in &request.filters {
            dialog = dialog.add_filter(name, extensions);
        }
        dialog = dialog.add_filter("All files (*.*)", &["*"]);
    }
    #[cfg(windows)]
    if let Some(owner) = bc_shell::dialog_owner() {
        dialog = dialog.set_parent(&owner);
    }
    match request.kind {
        Kind::OpenFile => dialog.pick_file(),
        Kind::SaveFile => dialog.save_file(),
        Kind::PickFolder => dialog.pick_folder(),
    }
}

/// 测试替身：不弹系统窗口。`respond` 预置下一次返回值，`last` 读最近一次请求。
#[cfg(test)]
pub mod mock {
    use super::Request;
    use std::cell::RefCell;
    use std::path::PathBuf;
    thread_local! {
        static RESPONSE: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
        static LAST: RefCell<Option<Request>> = const { RefCell::new(None) };
    }
    /// 下一次对话框返回 `path`（None = 用户取消）。
    pub fn respond(path: Option<PathBuf>) {
        RESPONSE.with(|r| *r.borrow_mut() = path);
    }
    /// 取走最近一次对话框请求。
    pub fn last() -> Option<Request> {
        LAST.with(|l| l.borrow_mut().take())
    }
    pub(super) fn take(request: Request) -> Option<PathBuf> {
        LAST.with(|l| *l.borrow_mut() = Some(request));
        RESPONSE.with(|r| r.borrow_mut().take())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn initial_file_splits_into_folder_and_name() {
        let dir = std::env::temp_dir();
        let file = dir.join("matcha-native-dialog-missing.txt");
        assert_eq!(
            split_initial(Kind::OpenFile, &file),
            (
                Some(dir.clone()),
                Some("matcha-native-dialog-missing.txt".into())
            )
        );
        assert_eq!(
            split_initial(Kind::PickFolder, &dir),
            (Some(dir.clone()), None)
        );
        // 文件夹选择不预填文件名，退到存在的上级。
        let missing = dir.join("no-such-dir-xyz").join("deeper");
        assert_eq!(split_initial(Kind::PickFolder, &missing), (Some(dir), None));
    }
    #[test]
    fn titles_and_mock_round_trip() {
        assert_eq!(side_title("Open File", 0), "Open File - Left Side");
        assert_eq!(side_title("Open File", 1), "Open File - Right Side");
        mock::respond(Some(PathBuf::from("x.txt")));
        let got = Request::new(Kind::OpenFile, "t").initial("").show();
        assert_eq!(got, Some(PathBuf::from("x.txt")));
        let last = mock::last().unwrap();
        assert_eq!(last.initial, None);
        assert_eq!(Request::new(Kind::SaveFile, "t").show(), None);
    }
}
