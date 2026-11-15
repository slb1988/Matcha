//! Hex Compare 视图控制器（`THexCompareView` 对应物）。

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;

use std::path::{Path, PathBuf};

use bc_core::{CoreError, DiffKind};
use bc_engine_bin::hex::{DisplayFilter, HexAlignJob, HexContent, HexMap, HexStats};
use eframe::egui;

use crate::chrome::icons::Icon;
use crate::chrome::menubar;
use crate::chrome::statusbar::{self, StatusField};
use crate::chrome::toolbar::{self, ToolItem, ToolItemKind};
use crate::chrome::{controls::Role, widgets, window::Window};

use super::menu;
use super::render::{self, GridRowModel, GridStyle};
use super::worker::Worker;

/// 视图上报给应用外壳的动作。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HexAction {
    Home,
    Close,
    NewTab,
    Exit,
    /// Compare Files Using 互切（H10）：同一对文件以 Text/Table Compare 原位重开。
    CompareAsText { left: PathBuf, right: PathBuf },
    CompareAsTable { left: PathBuf, right: PathBuf },
}

/// 退出/关闭确认对话框的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitConfirm {
    Close,
    Exit,
}

/// 待弹出的打开/另存对话框请求（本帧末弹系统原生对话框，见 chrome::native_dialog）。
#[derive(Debug, Clone)]
pub struct HexFileDialog {
    pub side: usize,
    pub save_as: bool,
    pub path: String,
}

/// 网格行模型缓存：revision + 行宽 + 过滤变化时重算。
#[derive(Debug)]
struct GridCache {
    revisions: (u64, u64),
    per_line: usize,
    filter: DisplayFilter,
    context: usize,
    rows: Vec<GridRowModel>,
    /// 缩略图列 bins（与 rows 同批重算；差异行红色像素横线的水平区间）。
    thumb_bins: Vec<u32>,
}

/// 一次字节编辑的逆操作记录（Undo 栈，按侧分栈存放）。
#[derive(Debug)]
struct EditRecord {
    at: usize,
    removed: Vec<u8>,   // 编辑时被替换掉的原字节
    inserted_len: usize, // 编辑后新字节长度
}

/// 最近一帧的布局几何（RawInput 回归的命中与期望值计算用）。
#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct LastLayout {
    pub thumb: Option<render::ThumbnailGeometry>,
    pub bars: [egui::Rect; 2],
    pub pane_h: f32,
    pub view_h: f32,
}

#[cfg(test)]
impl Default for LastLayout {
    fn default() -> Self {
        Self { thumb: None, bars: [egui::Rect::NOTHING; 2], pane_h: 0.0, view_h: 0.0 }
    }
}

/// Hex Compare 双栏视图。
pub struct HexCompareView {
    pub map: HexMap,
    worker: Option<Worker>,
    /// 状态栏消息（加载/对齐/错误提示）。
    pub message: String,
    /// Fit to window（默认开）：按窗格宽度自动确定每行格数。
    pub fit_to_window: bool,
    /// 字节地址列开关（View > Byte Addresses）。
    pub show_byte_addresses: bool,
    /// Side-by-Side（false = Over-Under）。
    pub side_by_side: bool,
    /// 工具栏 / File Info 头 / 缩略图条开关（View 菜单）。
    pub show_toolbar: bool,
    pub show_file_info: bool,
    pub show_thumbnail: bool,
    /// 显示过滤（Show All/Diffs/Same）。
    pub filter: DisplayFilter,
    /// 插入/Overwrite 模式（状态栏指示，Insert 键切换）。
    pub insert_mode: bool,
    /// 每侧光标的字节地址（状态栏；跨侧同步到同一逻辑字节）。
    pub cursor: [usize; 2],
    /// 活动侧（0 左 1 右）。
    pub active_side: usize,
    /// Layout 对话框开关。
    pub layout_open: bool,
    /// Session Settings 对话框开关（任务 6.1 填充三页内容）。
    pub settings_open: bool,
    /// Session Settings 草稿与页签。
    pub(crate) settings_draft: Option<super::settings::SessionDraft>,
    pub(crate) settings_page: super::settings::Page,
    /// Groups per item（Layout 对话框；非 Fit 时每行格数 = bytes_per_group × groups_per_item）。
    pub groups_per_item: u16,
    /// 每侧选区（字节偏移半开区间）。
    pub selection: [Option<std::ops::Range<usize>>; 2],
    /// 每侧脏标记（有未保存编辑）。
    pub dirty: [bool; 2],
    undo: [Vec<EditRecord>; 2],
    redo: [Vec<EditRecord>; 2],
    /// 内联查找栏。
    pub find_open: bool,
    pub find_hex: bool,
    pub find_case: bool,
    pub find_input: String,
    pub replace_open: bool,
    pub replace_input: String,
    /// Go To / Info 对话框。
    pub goto_open: bool,
    pub goto_input: String,
    pub goto_hex: bool,
    pub goto_side: usize,
    pub info_open: bool,
    /// 待滚动的纵向像素位置（导航目标滚入视野）。
    pub pending_scroll: Option<usize>,  // 目标对齐行
    /// 待写入系统剪贴板的文本（ui() 帧内 flush）。
    pub pending_copy_text: Option<String>,
    /// 上报动作（互切/Home/Close/Exit 等）。
    pub action: Option<HexAction>,
    /// 退出/关闭确认。
    exit_confirm: Option<ExitConfirm>,
    /// Reload 脏确认。
    reload_confirm: bool,
    /// 打开/另存对话框。
    pub file_dialog: Option<HexFileDialog>,
    /// 地址栏编辑态（每侧可编辑路径；对齐 text 视图 header）。
    pub path_edits: [String; 2],
    /// 地址栏已同步的来源路径（换文件/Swap/另存后刷新编辑态）。
    path_sources: [Option<PathBuf>; 2],
    /// 最近打开/保存的路径（地址栏历史下拉，最新在前）。
    recent: Vec<PathBuf>,
    /// Open Clipboard 目标侧（等待 Paste 回送）。
    clipboard_side: Option<usize>,
    /// Ctrl+V/菜单 Paste：等待系统剪贴板回送。
    paste_pending: bool,
    cache: Option<GridCache>,
    /// 纵向滚动偏移（视图状态的一部分；pending_scroll 换算后写入，ScrollArea 回读保持）。
    scroll: f32,
    /// 当前首可见网格行（RawInput 回归断言滚动到位）。
    pub visible_first_row: usize,
    /// 最近一帧布局几何（见 LastLayout）。
    #[cfg(test)]
    pub(crate) last_layout: LastLayout,
}

impl HexCompareView {
    /// 空会话（任务 6.3）：先建视图，两侧待打开。
    pub fn empty() -> Self {
        Self::from_contents(HexContent::default(), HexContent::default())
    }

    pub fn from_contents(left: HexContent, right: HexContent) -> Self {
        let mut map = HexMap {
            left,
            right,
            ..Default::default()
        };
        let worker = if map.left.is_empty() && map.right.is_empty() {
            map.align();
            None
        } else {
            Some(Worker::spawn(HexAlignJob::from_map(&map)))
        };
        let mut view = Self {
            map,
            worker,
            message: "Comparing…".into(),
            fit_to_window: true,
            show_byte_addresses: true,
            side_by_side: true,
            show_toolbar: true,
            show_file_info: true,
            show_thumbnail: true,
            filter: DisplayFilter::All,
            insert_mode: true,
            cursor: [0, 0],
            active_side: 0,
            layout_open: false,
            settings_open: false,
            settings_draft: None,
            settings_page: super::settings::Page::default(),
            groups_per_item: 2,
            selection: [None, None],
            dirty: [false, false],
            undo: [Vec::new(), Vec::new()],
            redo: [Vec::new(), Vec::new()],
            find_open: false,
            find_hex: true,
            find_case: false,
            find_input: String::new(),
            replace_open: false,
            replace_input: String::new(),
            goto_open: false,
            goto_input: String::new(),
            goto_hex: true,
            goto_side: 0,
            info_open: false,
            pending_scroll: None,
            pending_copy_text: None,
            action: None,
            exit_confirm: None,
            reload_confirm: false,
            file_dialog: None,
            path_edits: [String::new(), String::new()],
            path_sources: [None, None],
            recent: Vec::new(),
            clipboard_side: None,
            paste_pending: false,
            cache: None,
            scroll: 0.0,
            visible_first_row: 0,
            #[cfg(test)]
            last_layout: LastLayout::default(),
        };
        view.sync_path_edits();
        for path in view.paths().into_iter().rev().flatten() {
            view.remember(&path);
        }
        view
    }

    pub fn from_bytes(left: Vec<u8>, right: Vec<u8>) -> Self {
        Self::from_contents(HexContent::from_bytes(left), HexContent::from_bytes(right))
    }

    /// 按规则阈值加载两侧文件（设计 D3 分层）。
    pub fn open(left: &Path, right: &Path) -> Result<Self, CoreError> {
        let threshold = crate::hex::default_lock_threshold();
        let left = HexContent::load(left, threshold).map_err(CoreError::Io)?;
        let right = HexContent::load(right, threshold).map_err(CoreError::Io)?;
        Ok(Self::from_contents(left, right))
    }

    /// 单侧换文件（H12）。
    pub fn open_path(&mut self, side: usize, path: &Path) -> Result<(), CoreError> {
        let threshold = self.map.rules.lock_threshold;
        let content = HexContent::load(path, threshold).map_err(CoreError::Io)?;
        if side == 0 {
            self.map.left = content;
        } else {
            self.map.right = content;
        }
        // 旧内容（含其编辑）已被替换，该侧不再有未保存修改。
        self.dirty[side] = false;
        self.recompare();
        self.remember(path);
        Ok(())
    }

    /// 系统拖放到某侧：误拖不能静默丢掉未保存的编辑，该侧有修改时拒绝并提示。
    pub fn drop_path(&mut self, side: usize, path: &Path) {
        self.active_side = side;
        if self.dirty[side] {
            self.message = format!(
                "{}侧有未保存的编辑，先保存或撤销后再拖入",
                if side == 0 { "左" } else { "右" }
            );
            return;
        }
        if let Err(e) = self.open_path(side, path) {
            self.message = e.to_string();
        }
    }

    /// 系统拖放两个文件：左右成对替换（单次重对齐）。成对替换是原子的——任一侧
    /// 有未保存编辑或加载失败都整体拒绝并提示，不留半新半旧的比较。
    pub fn drop_pair(&mut self, left: &Path, right: &Path) {
        if self.dirty[0] || self.dirty[1] {
            let side = if self.dirty[0] { "左" } else { "右" };
            self.message = format!("{side}侧有未保存的编辑，先保存或撤销后再拖入");
            return;
        }
        let threshold = self.map.rules.lock_threshold;
        let mut load = |path: &Path| -> Option<HexContent> {
            match HexContent::load(path, threshold) {
                Ok(content) => Some(content),
                Err(e) => {
                    self.message = e.to_string();
                    None
                }
            }
        };
        let (Some(left_content), Some(right_content)) = (load(left), load(right)) else {
            return;
        };
        self.map.left = left_content;
        self.map.right = right_content;
        self.dirty = [false, false];
        self.remember(left);
        self.remember(right);
        self.recompare();
    }

    /// 统计（四分类）。
    pub fn stats(&self) -> HexStats {
        self.map.stats()
    }

    pub fn is_ready(&self) -> bool {
        self.worker.is_none()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty[0] || self.dirty[1]
    }

    /// 视图标题（`left <--> right - Hex Compare`）。
    pub fn title(&self) -> String {
        let name = |c: &HexContent| {
            c.source()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                .unwrap_or_else(|| "<clipboard>".into())
        };
        format!("{} <--> {} - Hex Compare", name(&self.map.left), name(&self.map.right))
    }

    pub fn poll(&mut self) {
        if let Some(result) = self.worker.as_mut().and_then(Worker::poll) {
            self.worker = None;
            match result {
                Ok(output) => {
                    if self.map.set_alignment(output).is_ok() {
                        self.cache = None;
                        // 状态栏常态显示四分类结论（参考工具：`Binary same`/`Binary differences`）。
                        self.message = self.map.stats().summary().to_owned();
                    }
                }
                Err(bc_jobs::JobError::Cancelled) => {}
                Err(e) => self.message = e.to_string(),
            }
        }
    }

    /// 丢弃对齐结果并按当前内容重算（H07 Recompare / 编辑后重对齐）。
    pub fn recompare(&mut self) {
        drop(self.worker.take()); // Drop 先 cancel 再 join
        self.worker = Some(Worker::spawn(HexAlignJob::from_map(&self.map)));
        self.message = "Comparing…".into();
    }

    /// H05 Swap Sides：左右互换并重比。
    pub fn swap(&mut self) {
        std::mem::swap(&mut self.map.left, &mut self.map.right);
        self.cursor.swap(0, 1);
        self.active_side = 1 - self.active_side;
        self.recompare();
    }

    /// 当前网格行模型（Fit to window 时按窗格宽度重算每行格数；过滤模式按区域起点重打包）。
    pub fn grid_rows(&mut self, pane_width: f32, ui: &egui::Ui) -> &[GridRowModel] {
        let style = GridStyle::new(ui, self.map.rules.bytes_per_group);
        let per_line = self.per_line(&style, pane_width);
        let revisions = (self.map.left.revision(), self.map.right.revision());
        let (filter, context) = (self.filter, self.map.rules.context_count);
        let stale = self.cache.as_ref().is_none_or(|c| {
            c.revisions != revisions || c.per_line != per_line || c.filter != filter || c.context != context
        });
        if stale {
            let rows = self.build_grid_rows(per_line);
            let thumb_bins = render::thumb_bins(&rows, per_line, render::THUMB_BIN_MAX);
            self.cache = Some(GridCache { revisions, per_line, filter, context, rows, thumb_bins });
        }
        &self.cache.as_ref().unwrap().rows
    }

    /// 按过滤模式构建网格行模型（Diffs 带 context_count 上下文；区域连续流入同一打包序列）。
    fn build_grid_rows(&self, per_line: usize) -> Vec<GridRowModel> {
        let per_line = per_line as u16;
        if self.filter == DisplayFilter::All {
            return render::grid_rows(&self.map, usize::from(per_line));
        }
        let rows = bc_engine_bin::hex::filtered_rows(&self.map.align, self.filter, self.map.rules.context_count);
        let packed = bc_engine_bin::hex::pack_slice(&rows, per_line);
        render::models_from_packed(&self.map.left, &self.map.right, &packed)
    }

    /// 每行格数：Fit 开启时按宽度算；否则 bytes_per_group × groups_per_item。
    fn per_line(&self, style: &GridStyle, pane_width: f32) -> usize {
        if self.fit_to_window {
            style.fit_per_line(pane_width)
        } else {
            usize::from(self.map.rules.bytes_per_group.max(1)) * usize::from(self.groups_per_item.max(1))
        }
    }

    /// 快照场景预设（任务 7.3）：hex-basic 默认首屏；hex-insertion 滚到 0x400 插入区；
    /// hex-diffs-filter 切 Diffs 过滤。
    pub fn snapshot_preset(&mut self, scene: &str) {
        match scene {
            "hex-diffs-filter" => {
                self.filter = DisplayFilter::Differences;
                self.cache = None;
            }
            "hex-insertion" => {
                self.set_cursor(0, 0x400);
                if let Some(row) = self.align_row_of(0, 0x400) {
                    self.pending_scroll = Some(row);
                }
            }
            _ => {}
        }
    }

    // ---- 任务 6.2：应用外壳契约与命令分派 ----

    /// 两侧路径（空会话/剪贴板侧为 None）。
    pub fn paths(&self) -> [Option<PathBuf>; 2] {
        [
            self.map.left.source().map(|p| p.to_path_buf()),
            self.map.right.source().map(|p| p.to_path_buf()),
        ]
    }

    /// 退出/关闭确认框是否开着（app.rs step_exit 契约）。
    pub fn prompt_open(&self) -> bool {
        self.exit_confirm.is_some()
    }

    pub fn request_exit(&mut self) {
        if self.is_dirty() {
            self.exit_confirm = Some(ExitConfirm::Exit);
        }
    }

    pub fn request_close(&mut self) {
        if self.is_dirty() {
            self.exit_confirm = Some(ExitConfirm::Close);
        } else {
            self.action = Some(HexAction::Close);
        }
    }

    /// Open Clipboard（H14）：把剪贴板字节载入一侧（无来源路径，保存走另存）。
    pub fn open_clipboard(&mut self, side: usize, bytes: Vec<u8>) {
        *self.content_mut(side) = HexContent::from_bytes(bytes);
        self.dirty[side] = false;
        self.recompare();
    }

    /// 单侧另存（H16）。
    pub fn save_side_as(&mut self, side: usize, path: &Path) {
        match self.content_mut(side).save_as(path) {
            Ok(()) => {
                self.dirty[side] = false;
                self.message = "已保存".into();
                self.remember(path);
            }
            Err(e) => self.message = e.to_string(),
        }
    }

    /// 记住路径（地址栏历史下拉，最新在前，上限 12；对齐 text 视图）。
    fn remember(&mut self, path: &Path) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_path_buf());
        self.recent.truncate(12);
    }

    /// 地址栏编辑态与当前来源同步（换文件/Swap/另存后重置文本）。
    pub fn sync_path_edits(&mut self) {
        let paths = self.paths();
        for (s, path) in paths.iter().enumerate() {
            if self.path_sources[s] != *path {
                self.path_sources[s] = path.clone();
                self.path_edits[s] = path
                    .as_ref()
                    .map(|p| {
                        std::path::absolute(p)
                            .unwrap_or_else(|_| p.clone())
                            .display()
                            .to_string()
                    })
                    .unwrap_or_default();
            }
        }
    }

    /// 单侧地址栏：Enter/历史选择打开路径，浏览按钮进内建文件对话框，保存按钮同 Save File。
    fn path_header(&mut self, ui: &mut egui::Ui, side: usize, history: &[String]) {
        let response = crate::chrome::pathbar::show_with_history(
            ui,
            &mut self.path_edits[side],
            history,
            false,
        );
        if response.submitted {
            let path = PathBuf::from(self.path_edits[side].trim());
            self.active_side = side;
            if !path.as_os_str().is_empty() {
                if let Err(e) = self.open_path(side, &path) {
                    self.message = e.to_string();
                }
            }
            return;
        }
        match response.action.as_deref() {
            Some("save") => {
                self.active_side = side;
                if self.content(side).source().is_some() {
                    self.save_side(side);
                } else {
                    self.file_dialog = Some(HexFileDialog { side, save_as: true, path: String::new() });
                }
            }
            Some("browse" | "history") => {
                self.active_side = side;
                self.file_dialog = Some(HexFileDialog { side, save_as: false, path: String::new() });
            }
            _ => {}
        }
    }

    /// 菜单/命令统一分派（命令 id = 菜单项 label 或显式 `Using:*`）。
    pub fn command(&mut self, id: &str) {
        match id {
            "Session Settings..." => self.settings_open = true,
            "Swap Sides" => self.swap(),
            "Reload Files" => {
                if self.is_dirty() {
                    self.reload_confirm = true;
                } else {
                    self.reload();
                }
            }
            "Recompare Files" => self.recompare(),
            "Hex Compare Info" => self.info_open = true,
            "Using:Text Compare" | "Using:Table Compare" => {
                let [left, right] = self.paths();
                match (left, right) {
                    (Some(left), Some(right)) => {
                        self.action = Some(if id.ends_with("Text Compare") {
                            HexAction::CompareAsText { left, right }
                        } else {
                            HexAction::CompareAsTable { left, right }
                        });
                    }
                    _ => self.message = "两侧均为文件路径才能互切".into(),
                }
            }
            "Close Tab" => self.request_close(),
            "Open File..." => {
                self.file_dialog = Some(HexFileDialog { side: self.active_side, save_as: false, path: String::new() });
            }
            "Open Clipboard" => self.clipboard_side = Some(self.active_side),
            "Save File" => {
                let side = self.active_side;
                if self.content(side).source().is_some() {
                    self.save_side(side);
                } else {
                    self.file_dialog = Some(HexFileDialog { side, save_as: true, path: String::new() });
                }
            }
            "Save File As..." => {
                self.file_dialog = Some(HexFileDialog { side: self.active_side, save_as: true, path: String::new() });
            }
            "Undo" => {
                let _ = self.undo(self.active_side);
            }
            "Redo" => {
                let _ = self.redo(self.active_side);
            }
            "Cut" => {
                if let Some(bytes) = self.cut(self.active_side) {
                    self.pending_copy_text = Some(bytes.iter().map(|b| format!("{b:02X} ")).collect());
                }
            }
            "Copy" => {
                if let Some(bytes) = self.copy_selection(self.active_side) {
                    self.pending_copy_text = Some(bytes.iter().map(|b| format!("{b:02X} ")).collect());
                }
            }
            "Paste" => self.paste_pending = true,
            "Delete" => {
                let _ = self.delete(self.active_side);
            }
            "Select All" => self.select_all(self.active_side),
            "Copy to Other Side" => {
                let _ = self.copy_to_other_side();
            }
            "Next Difference Bytes" => self.next_difference(true),
            "Previous Difference Bytes" => self.next_difference(false),
            "Find..." => self.find_open = true,
            "Replace..." => {
                self.find_open = true;
                self.replace_open = true;
            }
            "Go To..." => self.goto_open = true,
            "Show All" => self.filter = DisplayFilter::All,
            "Show Differences" => self.filter = DisplayFilter::Differences,
            "Show Same" => self.filter = DisplayFilter::Same,
            "Byte Addresses" => self.show_byte_addresses = !self.show_byte_addresses,
            "Layout..." => self.layout_open = true,
            "Side-by-Side Layout" => self.side_by_side = true,
            "Over-Under Layout" => self.side_by_side = false,
            "Thumbnail" => self.show_thumbnail = !self.show_thumbnail,
            "File Info" => self.show_file_info = !self.show_file_info,
            "Toolbar" => self.show_toolbar = !self.show_toolbar,
            _ => self.message = format!("{id}: 不可用"),
        }
        self.cache = None;
    }


    // ---- 任务 5.1：差异导航与定位（H23/H26） ----

    fn content(&self, side: usize) -> &HexContent {
        if side == 0 { &self.map.left } else { &self.map.right }
    }

    fn content_mut(&mut self, side: usize) -> &mut HexContent {
        if side == 0 { &mut self.map.left } else { &mut self.map.right }
    }

    /// 该侧字节偏移所在的对齐行。
    fn align_row_of(&self, side: usize, offset: usize) -> Option<usize> {
        if side == 0 {
            self.map.align.row_of_left(offset)
        } else {
            self.map.align.row_of_right(offset)
        }
    }

    /// 与指定侧字节配对的对侧字节偏移（若配对存在）。
    fn paired_offset(&self, side: usize, offset: usize) -> Option<usize> {
        let row = self.map.align.row(self.align_row_of(side, offset)?)?;
        if side == 0 { row.right } else { row.left }
    }

    /// 定位光标并跨侧同步到同一逻辑字节（spec：光标跨侧同步）。
    pub fn set_cursor(&mut self, side: usize, offset: usize) {
        let len = self.content(side).len();
        // 光标允许停在 EOF（Insert 模式追加）；跨侧同步仅在配对存在时发生。
        let offset = offset.min(len);
        self.cursor[side] = offset;
        if let Some(paired) = self.paired_offset(side, offset) {
            self.cursor[1 - side] = paired;
        }
        self.active_side = side;
    }

    /// Next/Previous Difference Bytes（Ctrl+N / Ctrl+P；越过末端给提示不回卷）。
    pub fn next_difference(&mut self, forward: bool) {
        let side = self.active_side;
        let Some(row) = self.align_row_of(side, self.cursor[side]) else {
            return;
        };
        let target = if forward {
            self.map.align.next_difference(row)
        } else {
            self.map.align.prev_difference(row)
        };
        match target {
            Some(row) => {
                // 块首行在任一侧可能为孤儿：两侧各自落到本行字节（无则保持）。
                let row_ref = *self.map.align.row(row).unwrap();
                if let Some(left) = row_ref.left {
                    self.cursor[0] = left;
                }
                if let Some(right) = row_ref.right {
                    self.cursor[1] = right;
                }
                self.pending_scroll = Some(row);
            }
            None => {
                self.message = if forward { "最后一个差异之后".into() } else { "第一个差异之前".into() };
            }
        }
    }

    /// Go To 对话框确认（H26）：十进制/十六进制地址、选侧、跨侧同步。
    pub fn goto_apply(&mut self) {
        let text = self.goto_input.trim();
        let parsed = if self.goto_hex {
            usize::from_str_radix(text, 16)
        } else {
            text.parse::<usize>()
        };
        match parsed {
            Ok(addr) => {
                let side = self.goto_side;
                self.set_cursor(side, addr);
                if let Some(row) = self.align_row_of(side, self.cursor[side]) {
                    self.pending_scroll = Some(row);
                }
                self.goto_open = false;
            }
            Err(_) => self.message = format!("无效地址: {text}"),
        }
    }

    // ---- 任务 5.2：查找与替换（H25） ----

    /// 解析查找输入：hex 模式接受 `DE AD BE EF`/`DEADBEEF`；文本模式按字节（可选大小写敏感）。
    pub fn find_needle(&self) -> Result<Vec<u8>, String> {
        let text = self.find_input.trim();
        if text.is_empty() {
            return Err("查找内容为空".into());
        }
        if self.find_hex {
            let hex: String = text.chars().filter(|c| !c.is_whitespace()).collect();
            if !hex.len().is_multiple_of(2) || hex.is_empty() {
                return Err("hex 字节序列须为偶数个十六进制字符".into());
            }
            (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| format!("无效 hex: {}&{}", &hex[i..i + 1], &hex[i + 1..i + 2])))
                .collect()
        } else if self.find_case {
            Ok(text.as_bytes().to_vec())
        } else {
            Ok(text.to_lowercase().into_bytes())
        }
    }

    /// 在活动侧内容中查找；返回命中起点。文本模式大小写不敏感时按 ASCII 小写比较。
    fn find_from(&self, needle: &[u8], from: usize, forward: bool) -> Option<usize> {
        let hay = self.content(self.active_side).bytes();
        if needle.is_empty() || hay.len() < needle.len() {
            return None;
        }
        let matches = |at: usize| {
            let seg = &hay[at..at + needle.len()];
            if self.find_hex || self.find_case {
                seg == needle
            } else {
                seg.iter().zip(needle).all(|(a, b)| a.to_ascii_lowercase() == *b)
            }
        };
        if forward {
            (from..=hay.len() - needle.len()).find(|&at| matches(at))
        } else {
            (0..=from.min(hay.len() - needle.len())).rev().find(|&at| matches(at))
        }
    }

    /// Find Next/Previous：命中选中并滚入视野；不命中给提示，不修改内容。
    pub fn find_next(&mut self, forward: bool) {
        let needle = match self.find_needle() {
            Ok(n) => n,
            Err(e) => {
                self.message = e;
                return;
            }
        };
        let side = self.active_side;
        // 光标位于上次命中起点时从其邻位继续，否则从光标处（含）开始。
        let at_hit = self.selection[side].as_ref().is_some_and(|r| r.start == self.cursor[side]);
        let from = if at_hit {
            if forward {
                self.cursor[side].saturating_add(1)
            } else {
                self.cursor[side].saturating_sub(1)
            }
        } else {
            self.cursor[side]
        };
        match self.find_from(&needle, from, forward) {
            Some(at) => {
                self.selection = [None, None];
                self.selection[side] = Some(at..at + needle.len());
                self.set_cursor(side, at);
                if let Some(row) = self.align_row_of(side, at) {
                    self.pending_scroll = Some(row);
                }
            }
            None => self.message = "未找到匹配项".into(),
        }
    }

    /// Replace：在当前命中处用替换串覆盖（等长覆盖语义），可一次 Undo。
    pub fn replace_current(&mut self) {
        let side = self.active_side;
        let Some(range) = self.selection[side].clone() else {
            self.message = "没有选中的匹配项".into();
            return;
        };
        let replacement = if self.replace_input.trim().is_empty() {
            Vec::new()
        } else if self.find_hex {
            let hex: String = self.replace_input.chars().filter(|c| !c.is_whitespace()).collect();
            match (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16)).collect::<Result<Vec<_>, _>>() {
                Ok(bytes) => bytes,
                Err(_) => {
                    self.message = "替换内容不是有效 hex".into();
                    return;
                }
            }
        } else {
            self.replace_input.clone().into_bytes()
        };
        if let Err(e) = self.apply_edit(side, range, &replacement) {
            self.message = e;
        }
    }

    // ---- 任务 5.3：字节编辑（H19–H22） ----

    /// 统一的编辑写入口：记录 Undo 逆操作、置脏、触发重对齐。
    fn apply_edit(&mut self, side: usize, range: std::ops::Range<usize>, with: &[u8]) -> Result<(), String> {
        let removed = self.content(side).bytes().get(range.clone()).unwrap_or(&[]).to_vec();
        self.content_mut(side)
            .replace(range.clone(), with)
            .map_err(|e| e.to_string())?;
        self.undo[side].push(EditRecord { at: range.start, removed, inserted_len: with.len() });
        self.redo[side].clear();
        self.dirty[side] = true;
        self.selection[side] = None;
        self.recompare();
        Ok(())
    }

    /// 光标处键入字节：Insert 模式插入，Overwrite 模式覆盖。
    pub fn type_bytes(&mut self, side: usize, bytes: &[u8]) -> Result<(), String> {
        let at = self.cursor[side];
        if self.insert_mode {
            self.apply_edit(side, at..at, bytes)
        } else {
            self.apply_edit(side, at..(at + bytes.len()), bytes)
        }
    }

    /// Delete：删选区，无选区删光标字节。
    pub fn delete(&mut self, side: usize) -> Result<(), String> {
        let range = self.selection[side].clone().unwrap_or_else(|| {
            let at = self.cursor[side];
            at..(at + 1).min(self.content(side).len())
        });
        self.apply_edit(side, range, &[])
    }

    pub fn cut(&mut self, side: usize) -> Option<Vec<u8>> {
        let bytes = self.copy_selection(side);
        if bytes.is_some() {
            let _ = self.delete(side);
        }
        bytes
    }

    pub fn copy_selection(&self, side: usize) -> Option<Vec<u8>> {
        let range = self.selection[side].clone()?;
        Some(self.content(side).bytes()[range].to_vec())
    }

    /// Paste：Insert 模式在光标处插入；Overwrite 模式覆盖。
    pub fn paste(&mut self, side: usize, bytes: &[u8]) -> Result<(), String> {
        self.type_bytes(side, bytes)
    }

    /// Select All：当前窗格全部字节（对齐参考工具选择全部可见字节）。
    pub fn select_all(&mut self, side: usize) {
        self.selection[side] = Some(0..self.content(side).len());
    }

    pub fn undo(&mut self, side: usize) -> Result<(), String> {
        let Some(rec) = self.undo[side].pop() else {
            self.message = "无可撤销操作".into();
            return Ok(());
        };
        let current = self.content(side).bytes()[rec.at..rec.at + rec.inserted_len].to_vec();
        let reinsert = rec.removed.clone();
        self.content_mut(side)
            .replace(rec.at..rec.at + rec.inserted_len, &reinsert)
            .map_err(|e| e.to_string())?;
        self.redo[side].push(EditRecord { at: rec.at, removed: current, inserted_len: rec.removed.len() });
        self.dirty[side] = true;
        self.recompare();
        Ok(())
    }

    pub fn redo(&mut self, side: usize) -> Result<(), String> {
        let Some(rec) = self.redo[side].pop() else {
            self.message = "无可重做操作".into();
            return Ok(());
        };
        let current = self.content(side).bytes()[rec.at..rec.at + rec.inserted_len].to_vec();
        let redo_bytes = rec.removed.clone();
        self.content_mut(side)
            .replace(rec.at..rec.at + rec.inserted_len, &redo_bytes)
            .map_err(|e| e.to_string())?;
        self.undo[side].push(EditRecord { at: rec.at, removed: current, inserted_len: rec.removed.len() });
        self.dirty[side] = true;
        self.recompare();
        Ok(())
    }

    /// Copy to Other Side（Ctrl+R）：把选中区段复制到对侧的对齐区段（一次 Undo 可还原）。
    pub fn copy_to_other_side(&mut self) -> Result<(), String> {
        let from = self.active_side;
        let Some(range) = self.selection[from].clone() else {
            self.message = "先选择一段字节".into();
            return Ok(());
        };
        // 选中区段在对齐图上覆盖的对侧跨度。
        let (mut start, mut end) = (usize::MAX, 0usize);
        for offset in range.clone() {
            if let Some(row) = self.align_row_of(from, offset) {
                if let Some(paired) = self.map.align.row(row).and_then(|r| if from == 0 { r.right } else { r.left }) {
                    start = start.min(paired);
                    end = end.max(paired + 1);
                }
            }
        }
        if start > end {
            self.message = "选区在对侧没有对齐内容".into();
            return Ok(());
        }
        let bytes = self.content(from).bytes()[range].to_vec();
        self.apply_edit(1 - from, start..end, &bytes)
    }

    /// 单侧保存（H16）：成功后清除该侧脏标记。
    pub fn save_side(&mut self, side: usize) {
        match self.content(side).save() {
            Ok(()) => {
                self.dirty[side] = false;
                self.message = "已保存".into();
            }
            Err(e) => self.message = e.to_string(),
        }
    }

    // ---- 任务 5.4：Swap/Reload/Recompare/Info/过滤 ----

    /// Hex Compare Info 对话框的四行文案（H09）。
    pub fn info_lines(&self) -> [String; 4] {
        let s = self.stats();
        [
            format!("{} same byte(s)", s.same),
            format!("{} left orphan byte(s)", s.left_orphan),
            format!("{} right orphan byte(s)", s.right_orphan),
            format!("{} difference byte(s)", s.difference),
        ]
    }

    /// 重载入口：有未保存编辑时先提示（调用方确认后走 force）。
    pub fn reload_guarded(&mut self, force: bool) -> bool {
        if self.is_dirty() && !force {
            self.message = "有未保存的编辑，确认后重载".into();
            return false;
        }
        self.reload();
        self.dirty = [false, false];
        true
    }

    // ---- 任务 4.2：工具栏 / File Info / 状态栏 / 缩略图 ----

    /// 工具栏项（All/Diffs/Same 为单选组）。
    pub fn toolbar_items(&self) -> Vec<ToolItem> {
        let toggle = |label: &str, id: &str, icon: Icon, selected: bool| ToolItem {
            label: label.into(),
            action_id: id.into(),
            kind: ToolItemKind::Toggle,
            enabled: true,
            selected,
            icon,
            group: Some("filter".into()),
        };
        let normal = |label: &str, id: &str, icon: Icon| ToolItem {
            label: label.into(),
            action_id: id.into(),
            kind: ToolItemKind::Normal,
            enabled: true,
            selected: false,
            icon,
            group: None,
        };
        vec![
            toggle("All", "filter.all", Icon::All, self.filter == DisplayFilter::All),
            toggle("Diffs", "filter.diffs", Icon::Diffs, self.filter == DisplayFilter::Differences),
            toggle("Same", "filter.same", Icon::Same, self.filter == DisplayFilter::Same),
            normal("Rules", "session.rules", Icon::Rules),
            normal("Format", "session.format", Icon::Format),
            normal("Copy", "edit.copy", Icon::Copy),
            normal("Next", "nav.next", Icon::Next),
            normal("Prev", "nav.prev", Icon::Prev),
            normal("Swap", "session.swap", Icon::Swap),
            normal("Reload", "file.reload", Icon::Reload),
        ]
    }

    /// 工具栏动作分发。
    pub fn toolbar_action(&mut self, action: &str) {
        match action {
            "filter.all" => self.filter = DisplayFilter::All,
            "filter.diffs" => self.filter = DisplayFilter::Differences,
            "filter.same" => self.filter = DisplayFilter::Same,
            "session.rules" | "session.format" => self.settings_open = true,
            "session.swap" => self.swap(),
            "file.reload" => {
                self.reload_guarded(false);
            }
            "nav.next" => self.next_difference(true),
            "nav.prev" => self.next_difference(false),
            "edit.copy" => {
                if let Some(bytes) = self.copy_selection(self.active_side) {
                    let hex: String = bytes.iter().map(|b| format!("{b:02X} ")).collect();
                    self.pending_copy_text = Some(hex);
                }
            }
            _ => self.message = format!("{action}: 未实现（后续任务组）"),
        }
        self.cache = None;
    }

    /// H06 Reload Files：重读两侧；脏时由调用方先确认（6.x 接确认对话）。
    pub fn reload(&mut self) {
        let threshold = self.map.rules.lock_threshold;
        let reload_side = |c: &mut HexContent| -> Result<(), CoreError> {
            if let Some(path) = c.source().map(|p| p.to_path_buf()) {
                *c = HexContent::load(&path, threshold).map_err(CoreError::Io)?;
            }
            Ok(())
        };
        let (mut l, mut r) = (HexContent::default(), HexContent::default());
        std::mem::swap(&mut l, &mut self.map.left);
        std::mem::swap(&mut r, &mut self.map.right);
        let result = reload_side(&mut l).and_then(|()| reload_side(&mut r));
        self.map.left = l;
        self.map.right = r;
        match result {
            Ok(()) => self.recompare(),
            Err(e) => self.message = e.to_string(),
        }
    }

    /// File Info 头单行文本（时间戳与字节数）。
    pub fn file_info_label(&self, side: usize) -> String {
        let content = if side == 0 { &self.map.left } else { &self.map.right };
        let size = format!("{} bytes", content.len());
        let time = content
            .modified()
            .map(|t| {
                let secs = t
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                format!("mtime {secs}")
            })
            .unwrap_or_else(|| "no timestamp".into());
        format!("{size} · {time}")
    }

    /// 状态栏字段（结论 / Insert / Load time / 每侧光标地址）。
    pub fn status_fields(&self) -> Vec<StatusField> {
        let stats = self.stats();
        let conclusion = if self.is_ready() { stats.summary() } else { "Comparing…" };
        let headline = if self.message.is_empty() { conclusion.to_owned() } else { self.message.clone() };
        let mut fields = vec![
            StatusField::flexible(headline),
            StatusField::fixed(if self.insert_mode { "Insert" } else { "Overwrite" }, 70.0),
        ];
        if let Some(ms) = self.map.left.load_millis().or(self.map.right.load_millis()) {
            fields.push(StatusField::fixed(
                format!("Load time: {:.2} seconds", ms as f64 / 1000.0),
                170.0,
            ));
        }
        fields.push(StatusField::fixed(format!("{:08X}", self.cursor[0]), 90.0));
        fields.push(StatusField::fixed(format!("{:08X}", self.cursor[1]), 90.0));
        fields
    }

    /// 缩略图条标刻：差异行在全文件中的纵向位置（0.0–1.0）。
    pub fn thumbnail_marks(&self) -> Vec<f32> {
        let total = self.map.align.len().max(1) as f32;
        self.map
            .align
            .rows()
            .iter()
            .enumerate()
            .filter(|(_, r)| r.kind != DiffKind::Same)
            .map(|(i, _)| i as f32 / total)
            .collect()
    }

    // ---- 任务 4.3：布局对话框与显示开关 ----

    /// Layout 对话框（Bytes per group / Groups per item / Fit to window / Context count）。
    pub fn layout_dialog(&mut self, ctx: &egui::Context) {
        if !self.layout_open {
            return;
        }
        let mut open = true;
        Window::new("Layout")
            .id(egui::Id::unique("hex-layout"))
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Bytes per group:");
                    let mut v = self.map.rules.bytes_per_group;
                    if ui.add(egui::DragValue::new(&mut v).range(1..=16)).changed() {
                        self.map.rules.bytes_per_group = v;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Groups per item:");
                    ui.add(egui::DragValue::new(&mut self.groups_per_item).range(1..=16));
                });
                widgets::checkbox(ui, &mut self.fit_to_window, "Fit to window");
                ui.horizontal(|ui| {
                    ui.label("Context count:");
                    let mut v = self.map.rules.context_count as u32;
                    if ui.add(egui::DragValue::new(&mut v).range(0..=1024)).changed() {
                        self.map.rules.context_count = v as usize;
                    }
                });
            });
        self.layout_open = open;
        self.cache = None;
    }

    /// 测试口：按指定行宽取网格模型（含过滤；不经 egui 布局）。
    #[cfg(test)]
    pub fn grid_rows_for_test(&mut self, per_line: u16) -> Vec<GridRowModel> {
        self.fit_to_window = false;
        self.map.rules.bytes_per_group = 8;
        self.groups_per_item = per_line / 8;
        self.cache = None;
        self.build_grid_rows(usize::from(per_line))
    }

    /// 键盘命令（模态打开时让位）。修饰键沿用 text 视图的 `command`（macOS Cmd / 其它 Ctrl）。
    fn keyboard(&mut self, ctx: &egui::Context) {
        // 文本框（地址栏/查找栏）持焦时键盘归它：Ctrl+A/C/X/V/Delete 等不得落到字节上。
        // 例外：Open Clipboard 等待的 Paste 仍由视图接收。
        let text_focus = ctx.egui_wants_keyboard_input();
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            if text_focus && !(matches!(event, egui::Event::Paste(_)) && self.clipboard_side.is_some()) {
                continue;
            }
            if let egui::Event::Paste(text) = &event {
                // Open Clipboard：作为该侧内容载入；否则在当前光标处粘贴字节。
                if let Some(side) = self.clipboard_side.take() {
                    self.open_clipboard(side, text.clone().into_bytes());
                } else {
                    let bytes = text.clone().into_bytes();
                    let _ = self.type_bytes(self.active_side, &bytes);
                }
                continue;
            }
            let egui::Event::Key { key, pressed: true, modifiers, .. } = &event else {
                continue;
            };
            let cmd = modifiers.command;
            match (*key, cmd, modifiers.shift) {
                (egui::Key::N, true, _) => self.next_difference(true),
                (egui::Key::P, true, _) => self.next_difference(false),
                (egui::Key::G, true, _) => self.goto_open = true,
                (egui::Key::F, true, _) => self.find_open = true,
                (egui::Key::H, true, _) => {
                    self.find_open = true;
                    self.replace_open = true;
                }
                (egui::Key::R, true, false) => {
                    let _ = self.copy_to_other_side();
                }
                (egui::Key::Z, true, false) => {
                    let _ = self.undo(self.active_side);
                }
                (egui::Key::Z, true, true) | (egui::Key::Y, true, false) => {
                    let _ = self.redo(self.active_side);
                }
                (egui::Key::A, true, false) => self.select_all(self.active_side),
                (egui::Key::C, true, false) => {
                    if let Some(bytes) = self.copy_selection(self.active_side) {
                        self.pending_copy_text =
                            Some(bytes.iter().map(|b| format!("{b:02X} ")).collect());
                    }
                }
                (egui::Key::X, true, false) => {
                    if let Some(bytes) = self.cut(self.active_side) {
                        self.pending_copy_text =
                            Some(bytes.iter().map(|b| format!("{b:02X} ")).collect());
                    }
                }
                (egui::Key::V, true, false) => self.paste_pending = true,
                (egui::Key::Insert, false, _) => self.insert_mode = !self.insert_mode,
                (egui::Key::Delete, false, _) => {
                    let _ = self.delete(self.active_side);
                }
                _ => {}
            }
        }
    }

    /// 对齐行号 → 当前网格行纵向位置（滚入视野用）。
    fn grid_y_of_align_row(&self, align_row: usize, row_height: f32) -> Option<f32> {
        let rows = &self.cache.as_ref()?.rows;
        let row = self.map.align.row(align_row)?;
        let off = row.left.or(row.right)?;
        let mut target = 0usize;
        for (i, r) in rows.iter().enumerate() {
            match r.left_off.or(r.right_off) {
                Some(o) if o <= off => target = i,
                _ => break,
            }
        }
        Some(target as f32 * row_height)
    }

    /// 内联查找栏（H25）：hex/文本模式、大小写、Prev/Next；Replace 展开替换行。
    fn find_bar(&mut self, ui: &mut egui::Ui) {
        if !self.find_open {
            return;
        }
        ui.horizontal(|ui| {
            ui.label("Find:");
            ui.add(widgets::field(&mut self.find_input).desired_width(220.0));
            ui.spacing_mut().item_spacing.x = 4.0;
            if ui.add(widgets::Button::new("Hex").selected(self.find_hex)).clicked() {
                self.find_hex = true;
            }
            if ui.add(widgets::Button::new("Text").selected(!self.find_hex)).clicked() {
                self.find_hex = false;
            }
            ui.spacing_mut().item_spacing.x = 8.0;
            if !self.find_hex {
                widgets::checkbox(ui, &mut self.find_case, "Match case");
            }
            if widgets::button(ui, "Previous").clicked() {
                self.find_next(false);
            }
            if widgets::primary_button(ui, "Next").clicked() {
                self.find_next(true);
            }
            if ui
                .add(widgets::Button::new("\u{00d7}").min_width(28.0))
                .on_hover_text("Close")
                .clicked()
            {
                self.find_open = false;
                self.replace_open = false;
            }
        });
        if self.replace_open {
            ui.horizontal(|ui| {
                ui.label("Replace:");
                ui.add(widgets::field(&mut self.replace_input).desired_width(220.0));
                if widgets::button(ui, "Replace").clicked() {
                    self.replace_current();
                }
            });
        }
    }

    /// Go To 对话框（H26）。
    fn goto_dialog(&mut self, ctx: &egui::Context) {
        if !self.goto_open {
            return;
        }
        let mut open = true;
        Window::new("Go To")
            .id(egui::Id::unique("hex-goto"))
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Byte address:");
                    ui.add(widgets::field(&mut self.goto_input).desired_width(160.0));
                });
                ui.horizontal(|ui| {
                    widgets::radio_value(ui, &mut self.goto_hex, true, "Hex");
                    widgets::radio_value(ui, &mut self.goto_hex, false, "Decimal");
                    ui.add_space(12.0);
                    widgets::radio_value(ui, &mut self.goto_side, 0, "Left");
                    widgets::radio_value(ui, &mut self.goto_side, 1, "Right");
                });
                match widgets::dialog_buttons(ui, &[("Cancel", Role::Secondary), ("OK", Role::Primary)]) {
                    Some(0) => self.goto_open = false,
                    Some(1) => self.goto_apply(),
                    _ => {}
                }
            });
        if self.goto_open {
            self.goto_open = open;
        }
    }

    /// Hex Compare Info 对话框（H09）：四分类字节计数。
    fn info_dialog(&mut self, ctx: &egui::Context) {
        if !self.info_open {
            return;
        }
        let mut open = true;
        Window::new("Hex Compare Info")
            .id(egui::Id::unique("hex-info"))
            .open(&mut open)
            .show(ctx, |ui| {
                for line in self.info_lines() {
                    ui.label(line);
                }
            });
        self.info_open = open;
    }

    /// 退出/重载确认与打开/另存对话框。
    fn modal_dialogs(&mut self, ctx: &egui::Context) {
        if let Some(kind) = self.exit_confirm {
            use crate::chrome::{
                controls::Role,
                modal::{Modal, Tone},
            };
            let open = true;
            let title = match kind {
                ExitConfirm::Close => "Save byte edits before closing?",
                ExitConfirm::Exit => "Save byte edits before quitting?",
            };
            let choice = match Modal::new("hex-unsaved", Tone::Warning, title)
                .message("There are unsaved byte edits. They will be lost if you don't save them.")
                .action("Don't Save", Role::Destructive)
                .action("Cancel", Role::Secondary)
                .action("Save", Role::Primary)
                .cancel(1)
                .show(ctx)
            {
                Some(0) => Some("discard"),
                Some(1) => Some("cancel"),
                Some(2) => Some("save"),
                _ => None,
            };
            match choice {
                Some("save") => {
                    for side in 0..2 {
                        if self.dirty[side] {
                            if self.content(side).source().is_some() {
                                self.save_side(side);
                            } else {
                                // 无来源侧先另存；完成后再关。
                                self.file_dialog = Some(HexFileDialog { side, save_as: true, path: String::new() });
                            }
                        }
                    }
                    if !self.is_dirty() && self.file_dialog.is_none() {
                        self.exit_confirm = None;
                        self.action = Some(match kind {
                            ExitConfirm::Close => HexAction::Close,
                            ExitConfirm::Exit => HexAction::Exit,
                        });
                    } else {
                        self.exit_confirm = None;
                    }
                }
                Some("discard") => {
                    self.dirty = [false, false];
                    self.exit_confirm = None;
                    self.action = Some(match kind {
                        ExitConfirm::Close => HexAction::Close,
                        ExitConfirm::Exit => HexAction::Exit,
                    });
                }
                _ => {
                    if !open || choice == Some("cancel") {
                        self.exit_confirm = None;
                    }
                }
            }
        }
        if self.reload_confirm {
            use crate::chrome::{
                controls::Role,
                modal::{Modal, Tone},
            };
            let open = true;
            let choice = Modal::new("hex-reload", Tone::Warning, "Reload and discard edits?")
                .message("Reloading reads both files from disk again; your unsaved byte edits will be discarded.")
                .action("Cancel", Role::Secondary)
                .action("Reload", Role::Primary)
                .cancel(0)
                .show(ctx);
            let confirm = choice == Some(1);
            let cancel = choice == Some(0);
            self.reload_confirm = open && !cancel && !confirm;
            if confirm {
                self.reload_guarded(true);
            }
        }
        if let Some(dialog) = self.file_dialog.take() {
            use crate::chrome::native_dialog::{side_title, Kind, Request};
            let (kind, action) = if dialog.save_as {
                (Kind::SaveFile, "Save File As")
            } else {
                (Kind::OpenFile, "Open File on File System")
            };
            let current = self.paths()[dialog.side].clone();
            let initial = current.unwrap_or_else(|| PathBuf::from(dialog.path.trim()));
            if let Some(p) = Request::new(kind, side_title(action, dialog.side))
                .initial(&initial)
                .show()
            {
                if dialog.save_as {
                    self.save_side_as(dialog.side, &p);
                } else if let Err(e) = self.open_path(dialog.side, &p) {
                    self.message = e.to_string();
                }
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<HexAction> {
        self.poll();
        if self.worker.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));
        }
        // 菜单栏（BC 会话菜单结构；命令分派见 command()）。
        if let Some(id) = menubar::show(ui, &menu::menus(self)) {
            self.command(&id);
        }
        let modal = self.goto_open
            || self.info_open
            || self.layout_open
            || self.settings_open
            || self.exit_confirm.is_some()
            || self.reload_confirm
            || self.file_dialog.is_some();
        if !modal {
            self.keyboard(ui.ctx());
        }
        if self.paste_pending {
            self.paste_pending = false;
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::RequestPaste);
        }
        if let Some(text) = self.pending_copy_text.take() {
            ui.ctx().copy_text(text);
        }
        if self.show_toolbar {
            let response = toolbar::show(ui, &self.toolbar_items());
            if let Some(action) = response.clicked.or(response.dropdown) {
                self.toolbar_action(&action);
            }
        }
        // 地址栏（每侧路径；对齐 text 视图 header）。
        self.sync_path_edits();
        let history: Vec<String> = self.recent.iter().map(|p| p.display().to_string()).collect();
        // 地址栏矩形：拖放落点判定的目标之一（over-under 时两条地址栏都叠在上窗格之上）。
        let mut header_rects = [egui::Rect::NOTHING; 2];
        let mut header = |this: &mut Self, ui: &mut egui::Ui, side: usize| {
            let top = ui.cursor().min;
            // 每侧独立 id 作用域：columns 子 ui 的 id 相同，不隔开会让两条编辑框 id 冲突，
            // 命中测试失效（点击/拖选都进不了编辑框）。
            ui.push_id(("hex-path", side), |ui| this.path_header(ui, side, &history));
            header_rects[side] =
                egui::Rect::from_min_max(top, egui::pos2(ui.max_rect().right(), ui.cursor().top()));
        };
        if self.side_by_side {
            ui.columns(2, |cols| {
                header(self, &mut cols[0], 0);
                header(self, &mut cols[1], 1);
            });
        } else {
            for side in 0..2 {
                header(self, ui, side);
            }
        }
        self.find_bar(ui);
        // 布局：最左缩略图列（52 px，自地址栏下缘起，对齐参考工具）+ 每窗格右缘滚动条（16 px）。
        let thumb_w = if self.show_thumbnail { render::THUMB_COL_W } else { 0.0 };
        let region_top = ui.available_rect_before_wrap().top();
        if self.show_file_info {
            ui.horizontal(|ui| {
                ui.add_space(thumb_w);
                ui.label(self.file_info_label(0));
                ui.separator();
                ui.label(self.file_info_label(1));
            });
        }

        let dark = ui.visuals().dark_mode;
        // 手工虚拟化滚动（沿用 text 视图约定：自身持有像素偏移 + chrome::wheel）。
        let available = ui.available_rect_before_wrap();
        let body = egui::Rect::from_min_max(
            available.min,
            egui::pos2(available.right(), (available.bottom() - 26.0).max(available.top() + 60.0)),
        );
        let content = egui::Rect::from_min_max(body.min + egui::vec2(thumb_w, 0.0), body.max);
        let sb = render::SCROLLBAR_W;
        // 每窗格内容矩形与右缘滚动条矩形（side-by-side：中缝一条 + 最右一条；over-under：上下各一条）。
        let (panes, bars) = if self.side_by_side {
            let w = ((content.width() - 2.0 * sb) / 2.0).max(40.0);
            let pane0 = egui::Rect::from_min_size(content.min, egui::vec2(w, content.height()));
            let bar0 = egui::Rect::from_min_size(pane0.right_top(), egui::vec2(sb, content.height()));
            let pane1 = egui::Rect::from_min_size(bar0.right_top(), egui::vec2(w, content.height()));
            let bar1 = egui::Rect::from_min_size(pane1.right_top(), egui::vec2(sb, content.height()));
            ([pane0, pane1], [bar0, bar1])
        } else {
            let h = (content.height() / 2.0).max(30.0);
            let w = (content.width() - sb).max(40.0);
            let pane0 = egui::Rect::from_min_size(content.min, egui::vec2(w, h));
            let bar0 = egui::Rect::from_min_size(pane0.right_top(), egui::vec2(sb, h));
            let pane1 = egui::Rect::from_min_max(pane0.left_bottom(), content.max - egui::vec2(sb, 0.0));
            let bar1 = egui::Rect::from_min_max(pane1.right_top(), content.max);
            ([pane0, pane1], [bar0, bar1])
        };
        let style = GridStyle::new(ui, self.map.rules.bytes_per_group);
        let row_count = self.grid_rows(panes[0].width(), ui).len();
        // 待滚动目标换算为像素（需在取出 cache 前完成借用）。
        let pending_row = self.pending_scroll.take();
        if let Some(y) = pending_row.and_then(|row| self.grid_y_of_align_row(row, style.row_height)) {
            self.scroll = y;
        }
        // 缩略图当前位置指针：活动侧光标所在网格行的内容比例（需在取出 cache 前换算）。
        let cursor_frac = self
            .align_row_of(self.active_side, self.cursor[self.active_side])
            .and_then(|row| self.grid_y_of_align_row(row, style.row_height))
            .map(|y| y / (row_count as f32 * style.row_height).max(1.0));
        let cache = self.cache.take().unwrap();
        let rows_ref = &cache.rows;
        let pane_h = row_count as f32 * style.row_height;
        let max_scroll = (pane_h - body.height()).max(0.0);
        let delta = crate::chrome::wheel::delta(ui.ctx(), &[body]);
        self.scroll = (self.scroll - delta.y).clamp(0.0, max_scroll);
        ui.advance_cursor_after_rect(body);

        // 覆盖层 child：缩略图列与滚动条的命中与绘制（不随内容滚动）。
        let region = egui::Rect::from_min_max(egui::pos2(body.left(), region_top), body.max);
        let mut overlay = ui.new_child(egui::UiBuilder::new().max_rect(region));
        overlay.set_clip_rect(region.intersect(ui.clip_rect()));

        // 缩略图列：点击/拖动迷你图定位页面（“图形框拖动页面”）。
        let thumb_geom = if self.show_thumbnail {
            let column = egui::Rect::from_min_max(region.min, egui::pos2(region.left() + thumb_w, region.bottom()));
            let geom = render::thumbnail_geometry(column, row_count, self.scroll, pane_h, body.height(), cursor_frac);
            let r = overlay.interact(geom.hit, overlay.scope_id().with("thumbnail"), egui::Sense::click_and_drag());
            if (r.clicked() || r.dragged()) && max_scroll > 0.0 {
                if let Some(pos) = r.interact_pointer_pos() {
                    let f = ((pos.y - geom.inner.top()) / geom.inner.height()).clamp(0.0, 1.0);
                    self.scroll = render::thumbnail_scroll_at(f, pane_h, body.height());
                }
            }
            Some(geom)
        } else {
            None
        };
        // 两条竖滚动条：点击轨道定位、拖动滑块跟随（两侧共用同一滚动偏移）。
        let mut bar_resp = Vec::with_capacity(2);
        for (side, bar) in bars.iter().enumerate() {
            let bar = *bar;
            let thumb = render::scrollbar_thumb(bar, pane_h, body.height(), self.scroll);
            let r = overlay.interact(
                bar,
                overlay.scope_id().with(("scrollbar", side)),
                egui::Sense::click_and_drag(),
            );
            if (r.clicked() || r.dragged()) && max_scroll > 0.0 {
                if let Some(pos) = r.interact_pointer_pos() {
                    let thumb_h = thumb.map(|t| t.height()).unwrap_or(20.0);
                    self.scroll = render::scrollbar_scroll_at(pos.y, bar, thumb_h, pane_h, body.height());
                }
            }
            bar_resp.push(r);
        }

        // 双窗格绘制（各自裁剪到窗格，互不越界）。
        let top = content.min - egui::vec2(0.0, self.scroll);
        let origins = if self.side_by_side {
            [egui::pos2(panes[0].left(), top.y), egui::pos2(panes[1].left(), top.y)]
        } else {
            [egui::pos2(panes[0].left(), top.y), egui::pos2(panes[1].left(), top.y + pane_h / 2.0 + style.row_height)]
        };
        for side in 0..2 {
            let mut pane_ui = ui.new_child(
                egui::UiBuilder::new().max_rect(body).id_salt(("hex-pane", side)),
            );
            pane_ui.set_clip_rect(panes[side].intersect(ui.clip_rect()));
            let first = render::paint_pane(&mut pane_ui, rows_ref, side, &style, origins[side], dark, self.show_byte_addresses);
            if side == 0 {
                self.visible_first_row = first;
            }
        }
        // 缩略图列与滚动条落笔（hover/drag 时滑块加深）。
        if let Some(geom) = thumb_geom {
            render::paint_thumbnail(overlay.painter(), &geom, &cache.thumb_bins, cache.per_line, dark);
        }
        for side in 0..2 {
            let thumb = render::scrollbar_thumb(bars[side], pane_h, body.height(), self.scroll);
            let hot = bar_resp[side].hovered() || bar_resp[side].dragged();
            render::paint_scrollbar(overlay.painter(), bars[side], thumb, dark, hot);
        }
        #[cfg(test)]
        {
            self.last_layout = LastLayout { thumb: thumb_geom, bars, pane_h, view_h: body.height() };
        }
        self.cache = Some(cache);
        // 系统文件拖放：单文件按落点（窗格/滚动条/地址栏）替换对应一侧；同时拖入
        // 两个文件成对载入左右（与主页拖入两个文件一致，不看落点）；对话框打开时忽略。
        let dropped = crate::chrome::drop::dropped_paths(ui.ctx());
        if !dropped.is_empty() && !modal {
            if dropped.len() >= 2 {
                self.drop_pair(&dropped[0], &dropped[1]);
            } else {
                let targets = (0..2).flat_map(|s| [(s, panes[s]), (s, bars[s]), (s, header_rects[s])]);
                let side = crate::chrome::drop::side_at(
                    crate::chrome::drop::drop_pos(ui.ctx()),
                    targets,
                    self.active_side,
                );
                self.drop_path(side, &dropped[0]);
            }
        }

        statusbar::show(ui, &self.status_fields());
        self.layout_dialog(ui.ctx());
        self.goto_dialog(ui.ctx());
        self.info_dialog(ui.ctx());
        self.settings_dialog(ui.ctx());
        self.modal_dialogs(ui.ctx());
        self.action.take()
    }
}
