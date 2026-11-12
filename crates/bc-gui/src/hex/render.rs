//! 网格渲染：地址列、分组 hex 格、ASCII 列、斜纹占位格、差异着色；只画可见行。
//!
//! 可测模型：`grid_rows` 把引擎的 cell packing 行（`HexRow`）展开成带字节值与着色的
//! 渲染模型，RawInput 回归直接断言它；`paint_pane` 只负责按模型落笔。

use bc_engine_bin::hex::{HexCell, HexContent, HexMap, HexRow};
use bc_core::DiffKind;
use eframe::egui;

/// 一个字节格的渲染模型。`None` 侧 = 斜纹占位格。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellModel {
    pub kind: DiffKind,
    pub left: Option<u8>,
    pub right: Option<u8>,
}

/// 一行网格的渲染模型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridRowModel {
    /// 8 位大写 hex 地址标签（整行占位侧为 None）。
    pub left_label: Option<String>,
    pub right_label: Option<String>,
    /// 行标签的原始字节偏移（导航滚动定位用）。
    pub left_off: Option<usize>,
    pub right_off: Option<usize>,
    pub cells: Vec<CellModel>,
}

/// 由对齐图 + 每行格数派生渲染模型（标签与相位规则由打包层保证）。
pub fn grid_rows(map: &HexMap, per_line: usize) -> Vec<GridRowModel> {
    let packed = bc_engine_bin::hex::pack_rows(&map.align, per_line as u16);
    models_from_packed(&map.left, &map.right, &packed)
}

/// 打包行 → 渲染模型（字节值从两侧内容取）。
pub fn models_from_packed(left: &HexContent, right: &HexContent, packed: &[HexRow]) -> Vec<GridRowModel> {
    packed
        .iter()
        .map(|row| GridRowModel {
            left_label: row.left_label.map(|v| format!("{v:08X}")),
            right_label: row.right_label.map(|v| format!("{v:08X}")),
            left_off: row.left_label,
            right_off: row.right_label,
            cells: row
                .cells
                .iter()
                .map(|cell| match *cell {
                    HexCell::Pair { left: l, right: r, kind } => CellModel {
                        kind,
                        left: left.bytes().get(l).copied(),
                        right: right.bytes().get(r).copied(),
                    },
                    HexCell::LeftOrphan(i) => CellModel {
                        kind: DiffKind::LeftOnly,
                        left: left.bytes().get(i).copied(),
                        right: None,
                    },
                    HexCell::RightOrphan(i) => CellModel {
                        kind: DiffKind::RightOnly,
                        left: None,
                        right: right.bytes().get(i).copied(),
                    },
                })
                .collect(),
        })
        .collect()
}

/// ASCII 预览字符：不可打印字节显示为 `.`。
pub fn ascii_char(byte: u8) -> char {
    if (0x20..=0x7E).contains(&byte) {
        byte as char
    } else {
        '.'
    }
}

/// 网格绘制样式（等宽字体驱动）。
#[derive(Debug, Clone, Copy)]
pub struct GridStyle {
    pub row_height: f32,
    /// 单个 hex 格（两位字符）的步进宽度。
    pub cell_pitch: f32,
    /// 组间额外间隙（bytes per group 分组显示）。
    pub group_gap: f32,
    pub address_width: f32,
    pub ascii_pitch: f32,
    pub bytes_per_group: usize,
}

impl GridStyle {
    pub fn new(ui: &egui::Ui, bytes_per_group: u16) -> Self {
        let font = egui::FontId::monospace(12.0);
        // painter 的 layout 系列只读测量（fonts() 的可变借用在 egui 0.37 不可行）。
        let digit = ui
            .painter()
            .layout_no_wrap("0".into(), font.clone(), egui::Color32::TRANSPARENT)
            .size()
            .x;
        let row_height = ui
            .painter()
            .layout_no_wrap("0".into(), font, egui::Color32::TRANSPARENT)
            .size()
            .y
            + 2.0;
        Self {
            row_height,
            cell_pitch: digit * 2.0 + 6.0,
            group_gap: 6.0,
            address_width: digit * 8.0 + 16.0,
            ascii_pitch: digit + 1.0,
            bytes_per_group: usize::from(bytes_per_group.max(1)),
        }
    }

    /// Fit to window：给定窗格宽度，能容纳的每行格数（至少 1）。
    pub fn fit_per_line(&self, pane_width: f32) -> usize {
        let per_cell = self.cell_pitch + self.ascii_pitch;
        let fixed = self.address_width + 8.0;
        let mut n = ((pane_width - fixed) / per_cell).floor().max(1.0) as usize;
        // 组间隙也要计入。
        while n > 1 {
            let gaps = (n.saturating_sub(1) / self.bytes_per_group) as f32 * self.group_gap;
            if fixed + n as f32 * per_cell + gaps <= pane_width {
                break;
            }
            n -= 1;
        }
        n
    }
}

/// 差异/孤儿着色的底色与文字色（参考工具：红底红字，实测底色 RGB≈255,227,227）。
pub fn cell_colors(kind: DiffKind, dark: bool) -> Option<(egui::Color32, egui::Color32)> {
    match kind {
        DiffKind::Same => None,
        DiffKind::Changed | DiffKind::LeftOnly | DiffKind::RightOnly => {
            let (bg, ink) = if dark {
                (egui::Color32::from_rgb(110, 40, 40), egui::Color32::from_rgb(255, 150, 150))
            } else {
                (egui::Color32::from_rgb(255, 227, 227), egui::Color32::from_rgb(180, 0, 0))
            };
            Some((bg, ink))
        }
    }
}

/// 斜纹占位格。
fn paint_placeholder(painter: &egui::Painter, rect: egui::Rect) {
    let base = egui::Color32::from_rgb(235, 235, 235);
    let line = egui::Color32::from_rgb(180, 180, 180);
    painter.rect_filled(rect, 0.0, base);
    let step = 4.0;
    let mut x = rect.left() - rect.height();
    while x < rect.right() {
        painter.line_segment(
            [
                egui::pos2(x.max(rect.left()), rect.bottom()),
                egui::pos2((x + rect.height()).min(rect.right()), rect.top()),
            ],
            egui::Stroke::new(1.0, line),
        );
        x += step;
    }
}

// ---- 缩略图列与竖滚动条（几何对齐参考工具：最左 52 px 缩略图列 + 每窗格右缘 16 px 滚动条）----

/// 缩略图列宽（与表格视图的参考列宽一致）。
pub const THUMB_COL_W: f32 = 52.0;
/// 迷你图内框在列内的水平偏移与宽度（2026-11 起有意宽于参考实测 x≈16..40：更宽更易看）。
pub const THUMB_INNER_X: f32 = 10.0;
pub const THUMB_INNER_W: f32 = 34.0;
/// 视口框水平偏移与宽度（对称越过内框两侧各 5 px，对齐参考的外扩框形）。
pub const THUMB_VIEW_X: f32 = 5.0;
pub const THUMB_VIEW_W: f32 = 44.0;
/// 视口框最小高度（px）：可见占比再小也保持框形，不退化成横线。
pub const THUMB_VIEW_MIN_H: f32 = 9.0;
/// 缩略图 bins 分辨率上限（行数超出时压缩：每 bin 聚合多行）。
pub const THUMB_BIN_MAX: usize = 4096;
/// 竖滚动条宽（参考实测 16 px）。
pub const SCROLLBAR_W: f32 = 16.0;

/// 缩略图列几何：列矩形、迷你图内框、视口框与命中区。
#[derive(Debug, Clone, Copy)]
pub struct ThumbnailGeometry {
    pub column: egui::Rect,
    pub inner: egui::Rect,
    pub viewport: egui::Rect,
    /// 命中区：内框与视口框的并集（列内空白不响应，对齐参考工具）。
    pub hit: egui::Rect,
    /// 当前位置指针的纵向坐标（列坐标系；无光标内容时 None）。
    pub pointer_y: Option<f32>,
}

/// 由列矩形、总行数与滚动状态推导缩略图几何。
/// 内框高度 = min(总行数, 列高-2)（参考：内容每行 1 px，超出列高则压缩到列高）；
/// 视口框 = 可见区间 [scroll, scroll+view_h] 在全内容高度 content_h 上的占比，
/// 高度不小于 `THUMB_VIEW_MIN_H`（占比过小时保持框形可读）；
/// `current` = 当前光标在内容中的纵向比例（0..1），映射为指针 y。
pub fn thumbnail_geometry(
    column: egui::Rect,
    total_rows: usize,
    scroll: f32,
    content_h: f32,
    view_h: f32,
    current: Option<f32>,
) -> ThumbnailGeometry {
    let lines = (total_rows as f32)
        .min((column.height() - 2.0).max(1.0))
        .floor()
        .max(1.0);
    let inner = egui::Rect::from_min_size(
        column.min + egui::vec2(THUMB_INNER_X, 1.0),
        egui::vec2(THUMB_INNER_W, lines),
    );
    let top_frac = if content_h > 0.0 { scroll / content_h } else { 0.0 };
    let view_frac = if content_h > 0.0 { (view_h / content_h).min(1.0) } else { 1.0 };
    let h = (view_frac * inner.height()).clamp(THUMB_VIEW_MIN_H.min(inner.height()), inner.height());
    let y = (inner.top() + top_frac.clamp(0.0, 1.0) * inner.height())
        .clamp(inner.top(), (inner.bottom() - h).max(inner.top()));
    let viewport = egui::Rect::from_min_size(
        egui::pos2(column.left() + THUMB_VIEW_X, y),
        egui::vec2(THUMB_VIEW_W, h),
    );
    let hit = inner.union(viewport).expand(1.0);
    let pointer_y = current.map(|f| inner.top() + f.clamp(0.0, 1.0) * inner.height());
    ThumbnailGeometry { column, inner, viewport, hit, pointer_y }
}

/// 缩略图 bins：把网格行的差异标记压缩到固定分辨率。
/// 每 bin 打包为 u32：0 = 无差异；否则 (lo+1)<<16 | (hi+1)（红色横线的水平格号区间）。
/// 水平区间规则对齐参考工具：Changed 行满宽、LeftOnly 行左半、RightOnly 行右半（混合取并集）。
pub fn thumb_bins(rows: &[GridRowModel], per_line: usize, resolution: usize) -> Vec<u32> {
    let total = rows.len();
    if total == 0 {
        return Vec::new();
    }
    let per_line = per_line.max(1) as u16;
    let n = total.min(resolution.max(1));
    let mut bins = vec![0u32; n];
    for (i, row) in rows.iter().enumerate() {
        let mut changed = false;
        let mut left = false;
        let mut right = false;
        for cell in &row.cells {
            match cell.kind {
                DiffKind::Same => {}
                DiffKind::Changed => changed = true,
                DiffKind::LeftOnly => left = true,
                DiffKind::RightOnly => right = true,
            }
        }
        let range = if changed {
            Some((0, per_line - 1))
        } else if left || right {
            let lo = if left { 0 } else { per_line / 2 };
            let hi = if right { per_line - 1 } else { per_line / 2 - 1 };
            Some((lo, hi))
        } else {
            None
        };
        if let Some((lo, hi)) = range {
            let bin = &mut bins[i * n / total];
            let (plo, phi) = match thumb_bin_range(*bin) {
                Some((l, h)) => (l.min(lo), h.max(hi)),
                None => (lo, hi),
            };
            *bin = ((plo as u32 + 1) << 16) | (phi as u32 + 1);
        }
    }
    bins
}

/// bin 解包为差异格列号区间（None = 该 bin 无差异）。
pub fn thumb_bin_range(bin: u32) -> Option<(u16, u16)> {
    if bin == 0 {
        None
    } else {
        Some((
            ((bin >> 16) as u16).saturating_sub(1),
            ((bin & 0xFFFF) as u16).saturating_sub(1),
        ))
    }
}

/// 缩略图列绘制：列底、带边框迷你图（差异行 = 红色像素横线，水平区间对齐参考工具：
/// Changed 满宽 / LeftOnly 左半 / RightOnly 右半）、灰色视口框、蓝色当前位置指针。
pub fn paint_thumbnail(
    painter: &egui::Painter,
    geom: &ThumbnailGeometry,
    bins: &[u32],
    per_line: usize,
    dark: bool,
) {
    let (column_bg, inner_bg, border, red, view_border) = if dark {
        (
            egui::Color32::from_rgb(51, 49, 48),
            egui::Color32::from_rgb(38, 38, 38),
            egui::Color32::from_rgb(90, 90, 90),
            egui::Color32::from_rgb(255, 65, 65),
            egui::Color32::from_rgb(128, 128, 128),
        )
    } else {
        (
            egui::Color32::from_rgb(240, 240, 240),
            egui::Color32::WHITE,
            egui::Color32::from_rgb(160, 160, 160),
            egui::Color32::from_rgb(255, 65, 65),
            egui::Color32::from_rgb(128, 128, 128),
        )
    };
    painter.rect_filled(geom.column, 0.0, column_bg);
    painter.rect_filled(geom.inner.expand(1.0), 0.0, border);
    painter.rect_filled(geom.inner, 0.0, inner_bg);
    if !bins.is_empty() {
        let lines = geom.inner.height() as usize;
        let per_line = per_line.max(1) as f32;
        for line in 0..lines {
            let b0 = line * bins.len() / lines;
            let b1 = (((line + 1) * bins.len() / lines).max(b0 + 1)).min(bins.len());
            let (mut lo, mut hi) = (u16::MAX, 0u16);
            for &bin in &bins[b0..b1] {
                if let Some((l, h)) = thumb_bin_range(bin) {
                    lo = lo.min(l);
                    hi = hi.max(h);
                }
            }
            if lo != u16::MAX {
                let y = geom.inner.top() + line as f32;
                let x0 = geom.inner.left() + lo as f32 / per_line * geom.inner.width();
                let x1 = geom.inner.left() + (hi as f32 + 1.0) / per_line * geom.inner.width();
                painter.rect_filled(
                    egui::Rect::from_min_max(egui::pos2(x0, y), egui::pos2(x1.max(x0 + 1.0), y + 1.0)),
                    0.0,
                    red,
                );
            }
        }
    }
    // 视口框：2 px 灰色描边（颜色对齐参考实测 (128,128,128)，比参考略粗保证可读）。
    painter.rect_stroke(
        geom.viewport,
        0.0,
        egui::Stroke::new(2.0, view_border),
        egui::StrokeKind::Inside,
    );
    // 当前位置指针：列左缘右指三角（对齐参考；亮色 #0078D7 / 暗色 #3199FF）。
    // 用无羽化 mesh：egui 会把 1x1 rect 的尖端羽化掉。
    if let Some(py) = geom.pointer_y {
        let pointer = if dark {
            egui::Color32::from_rgb(49, 153, 255)
        } else {
            egui::Color32::from_rgb(0, 120, 215)
        };
        let x0 = geom.column.left() + 1.0;
        let mut mesh = egui::Mesh::default();
        for d in -6i32..=6 {
            let w = 8 - d.abs();
            mesh.add_colored_rect(
                egui::Rect::from_min_max(
                    egui::pos2(x0, py + d as f32),
                    egui::pos2(x0 + w as f32, py + d as f32 + 1.0),
                ),
                pointer,
            );
        }
        painter.add(egui::Shape::mesh(mesh));
    }
}

/// 缩略图点击/拖动 → 目标滚动偏移（视口中心对齐指针所指的内容比例）。
pub fn thumbnail_scroll_at(fraction: f32, content_h: f32, view_h: f32) -> f32 {
    (fraction * content_h - view_h / 2.0).clamp(0.0, (content_h - view_h).max(0.0))
}

/// 滚动条滑块矩形（内容放得下时 None，参考不画滑块）。
pub fn scrollbar_thumb(
    track: egui::Rect,
    content_h: f32,
    view_h: f32,
    scroll: f32,
) -> Option<egui::Rect> {
    if content_h <= view_h || track.height() <= 4.0 {
        return None;
    }
    let thumb_h = (view_h / content_h * track.height()).clamp(20.0, track.height());
    let travel = (track.height() - thumb_h).max(1.0);
    let max_scroll = content_h - view_h;
    let frac = if max_scroll > 0.0 { (scroll / max_scroll).clamp(0.0, 1.0) } else { 0.0 };
    Some(egui::Rect::from_min_size(
        egui::pos2(track.left() + 2.0, track.top() + travel * frac),
        egui::vec2((track.width() - 4.0).max(2.0), thumb_h),
    ))
}

/// 滚动条点击/拖动 → 目标滚动偏移（滑块中心对齐指针；沿用文件夹视图的轨道定位约定）。
pub fn scrollbar_scroll_at(pointer_y: f32, track: egui::Rect, thumb_h: f32, content_h: f32, view_h: f32) -> f32 {
    let max_scroll = (content_h - view_h).max(0.0);
    let travel = (track.height() - thumb_h).max(1.0);
    let frac = ((pointer_y - track.top() - thumb_h / 2.0) / travel).clamp(0.0, 1.0);
    frac * max_scroll
}

/// 平面竖滚动条绘制（参考实测：浅灰轨道 + 左缘分隔线 + 深灰比例滑块）。
pub fn paint_scrollbar(
    painter: &egui::Painter,
    track: egui::Rect,
    thumb: Option<egui::Rect>,
    dark: bool,
    hot: bool,
) {
    let (track_bg, edge, thumb_fg, thumb_hot) = if dark {
        (
            egui::Color32::from_rgb(45, 45, 45),
            egui::Color32::from_rgb(70, 70, 70),
            egui::Color32::from_rgb(130, 130, 130),
            egui::Color32::from_rgb(160, 160, 160),
        )
    } else {
        (
            egui::Color32::from_rgb(240, 240, 240),
            egui::Color32::from_rgb(160, 160, 160),
            egui::Color32::from_rgb(133, 133, 133),
            egui::Color32::from_rgb(105, 105, 105),
        )
    };
    painter.rect_filled(track, 0.0, track_bg);
    painter.line_segment(
        [track.left_top(), track.left_bottom()],
        egui::Stroke::new(1.0, edge),
    );
    if let Some(thumb) = thumb {
        painter.rect_filled(thumb, 2.0, if hot { thumb_hot } else { thumb_fg });
    }
}

/// 单侧窗格的可见行绘制。只画 `ui.clip_rect()` 内的行。
/// 返回首个可见行索引（RawInput 回归断言滚动用）。
#[allow(clippy::too_many_arguments)]
pub fn paint_pane(
    ui: &mut egui::Ui,
    rows: &[GridRowModel],
    side: usize, // 0 = left, 1 = right
    style: &GridStyle,
    origin: egui::Pos2,
    dark: bool,
    show_addresses: bool,
) -> usize {
    let painter = ui.painter();
    let font = egui::FontId::monospace(12.0);
    let label_color = egui::Color32::from_rgb(105, 105, 105);
    let ink = if dark {
        egui::Color32::from_rgb(230, 230, 230)
    } else {
        egui::Color32::from_rgb(30, 30, 30)
    };
    let clip = ui.clip_rect();
    let first = ((clip.top() - origin.y) / style.row_height).floor().max(0.0) as usize;
    let last = (((clip.bottom() - origin.y) / style.row_height).ceil() as usize).min(rows.len());
    let _ = last;
    for (row_ix, row) in rows.iter().enumerate().take(last).skip(first) {
        let y = origin.y + row_ix as f32 * style.row_height;
        let mut x = origin.x;
        // 地址列
        if show_addresses {
            let label = if side == 0 { &row.left_label } else { &row.right_label };
            if let Some(label) = label {
                painter.text(
                    egui::pos2(x, y),
                    egui::Align2::LEFT_TOP,
                    label,
                    font.clone(),
                    label_color,
                );
            }
            x += style.address_width;
        }
        // hex 格 + ASCII 列
        let ascii_x = x
            + row.cells.len() as f32 * style.cell_pitch
            + (row.cells.len().saturating_sub(1) / style.bytes_per_group) as f32 * style.group_gap
            + 12.0;
        for (ci, cell) in row.cells.iter().enumerate() {
            let cx = x + ci as f32 * style.cell_pitch + (ci / style.bytes_per_group) as f32 * style.group_gap;
            let cell_rect = egui::Rect::from_min_size(
                egui::pos2(cx, y),
                egui::vec2(style.cell_pitch - 4.0, style.row_height - 1.0),
            );
            let byte = if side == 0 { cell.left } else { cell.right };
            match byte {
                Some(b) => {
                    if let Some((bg, fg)) = cell_colors(cell.kind, dark) {
                        painter.rect_filled(cell_rect, 0.0, bg);
                        painter.text(
                            cell_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("{b:02X}"),
                            font.clone(),
                            fg,
                        );
                    } else {
                        painter.text(
                            cell_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            format!("{b:02X}"),
                            font.clone(),
                            ink,
                        );
                    }
                }
                None => paint_placeholder(painter, cell_rect),
            }
            // ASCII 列（同步着色）
            if let Some(b) = byte {
                let pos = egui::pos2(ascii_x + ci as f32 * style.ascii_pitch, y);
                let color = cell_colors(cell.kind, dark).map(|(_, fg)| fg).unwrap_or(ink);
                painter.text(pos, egui::Align2::LEFT_TOP, ascii_char(b).to_string(), font.clone(), color);
            }
        }
    }
    first
}
