//! Hex Compare 视图控制器（`THexCompareView` 对应物）。

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;

use std::path::Path;

use bc_core::{CoreError, DiffKind};
use bc_engine_bin::hex::{DisplayFilter, HexAlignJob, HexContent, HexMap, HexStats};
use eframe::egui;

use crate::chrome::icons::Icon;
use crate::chrome::statusbar::{self, StatusField};
use crate::chrome::toolbar::{self, ToolItem, ToolItemKind};

use super::render::{self, GridRowModel, GridStyle};
use super::worker::Worker;

/// 网格行模型缓存：revision + 每行格数变化时重算。
#[derive(Debug)]
struct GridCache {
    revisions: (u64, u64),
    per_line: usize,
    rows: Vec<GridRowModel>,
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
    /// Groups per item（Layout 对话框；非 Fit 时每行格数 = bytes_per_group × groups_per_item）。
    pub groups_per_item: u16,
    /// Context count 的视图侧副本（与 rules.context_count 同步，Layout 对话框编辑）。
    cache: Option<GridCache>,
    scroll: f32,
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
        Self {
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
            groups_per_item: 2,
            cache: None,
            scroll: 0.0,
        }
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
        self.recompare();
        Ok(())
    }

    /// 统计（四分类）。
    pub fn stats(&self) -> HexStats {
        self.map.stats()
    }

    pub fn is_ready(&self) -> bool {
        self.worker.is_none()
    }

    pub fn is_dirty(&self) -> bool {
        false // 编辑落地后（任务 5.3）改为按两侧脏标记
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

    /// 当前网格行模型（Fit to window 时按窗格宽度重算每行格数）。
    pub fn grid_rows(&mut self, pane_width: f32, ui: &egui::Ui) -> &[GridRowModel] {
        let style = GridStyle::new(ui, self.map.rules.bytes_per_group);
        let per_line = self.per_line(&style, pane_width);
        let revisions = (self.map.left.revision(), self.map.right.revision());
        let stale = self
            .cache
            .as_ref()
            .is_none_or(|c| c.revisions != revisions || c.per_line != per_line);
        if stale {
            let rows = render::grid_rows(&self.map, per_line);
            self.cache = Some(GridCache { revisions, per_line, rows });
        }
        &self.cache.as_ref().unwrap().rows
    }

    /// 每行格数：Fit 开启时按宽度算；否则 bytes_per_group × groups_per_item。
    fn per_line(&self, style: &GridStyle, pane_width: f32) -> usize {
        if self.fit_to_window {
            style.fit_per_line(pane_width)
        } else {
            usize::from(self.map.rules.bytes_per_group.max(1)) * usize::from(self.groups_per_item.max(1))
        }
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
            "file.reload" => self.reload(),
            // nav/edit 在任务组 5 接线。
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
            fields.push(StatusField::fixed(format!("Load time: {ms} ms"), 120.0));
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
        egui::Window::new("Layout")
            .open(&mut open)
            .resizable(false)
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
                ui.checkbox(&mut self.fit_to_window, "Fit to window");
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

    /// 测试口：按指定行宽取网格模型（不经 egui 布局）。
    #[cfg(test)]
    pub fn grid_rows_for_test(&mut self, per_line: u16) -> Vec<GridRowModel> {
        self.fit_to_window = false;
        self.map.rules.bytes_per_group = 8;
        self.groups_per_item = per_line / 8;
        self.cache = None;
        render::grid_rows(&self.map, usize::from(per_line))
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.poll();
        if self.worker.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));
        }
        if self.show_toolbar {
            let response = toolbar::show(ui, &self.toolbar_items());
            if let Some(action) = response.clicked.or(response.dropdown) {
                self.toolbar_action(&action);
            }
        }
        if self.show_file_info {
            ui.horizontal(|ui| {
                ui.label(self.file_info_label(0));
                ui.separator();
                ui.label(self.file_info_label(1));
            });
        }

        let width = ui.available_width() / if self.side_by_side { 2.0 } else { 1.0 };
        let dark = ui.visuals().dark_mode;
        let style = GridStyle::new(ui, self.map.rules.bytes_per_group);
        let row_count = self.grid_rows(width, ui).len();
        let cache = self.cache.take().unwrap();
        let rows_ref = &cache.rows;
        let pane_h = row_count as f32 * style.row_height;

        egui::ScrollArea::vertical().show(ui, |ui| {
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), pane_h.max(ui.available_height())),
                egui::Sense::hover(),
            );
            let origins = if self.side_by_side {
                [rect.min, egui::pos2(rect.min.x + rect.width() / 2.0, rect.min.y)]
            } else {
                [rect.min, egui::pos2(rect.min.x, rect.min.y + pane_h / 2.0 + style.row_height)]
            };
            render::paint_pane(ui, rows_ref, 0, &style, origins[0], dark, self.show_byte_addresses);
            render::paint_pane(ui, rows_ref, 1, &style, origins[1], dark, self.show_byte_addresses);
            // 缩略图条（右侧窄条，差异标刻）。
            if self.show_thumbnail {
                let strip = egui::Rect::from_min_size(
                    egui::pos2(rect.right() - 10.0, rect.top()),
                    egui::vec2(10.0, rect.height().min(ui.clip_rect().height())),
                );
                ui.painter().rect_filled(strip, 0.0, egui::Color32::from_rgb(245, 245, 245));
                for mark in self.thumbnail_marks() {
                    let y = strip.top() + mark * strip.height();
                    ui.painter().line_segment(
                        [egui::pos2(strip.left(), y), egui::pos2(strip.right(), y)],
                        egui::Stroke::new(2.0, egui::Color32::from_rgb(220, 50, 50)),
                    );
                }
            }
        });
        self.cache = Some(cache);

        statusbar::show(ui, &self.status_fields());
        self.layout_dialog(ui.ctx());
        let _ = self.scroll;
    }
}
