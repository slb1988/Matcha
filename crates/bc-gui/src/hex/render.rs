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

/// 单侧窗格的可见行绘制。只画 `ui.clip_rect()` 内的行。
#[allow(clippy::too_many_arguments)]
pub fn paint_pane(
    ui: &mut egui::Ui,
    rows: &[GridRowModel],
    side: usize, // 0 = left, 1 = right
    style: &GridStyle,
    origin: egui::Pos2,
    dark: bool,
    show_addresses: bool,
) {
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
}
