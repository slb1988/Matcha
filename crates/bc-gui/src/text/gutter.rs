//! 行号20px，箭头20px；数字保持来源行号。箭头可点击：区块箭头整块复制，选区箭头复制选中行。
use eframe::egui;
pub const WIDTH: f32 = 40.0;
/// 箭头命中区宽度（行号栏左半边，不侵入文本区）。
const ARROW_WIDTH: f32 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowKind {
    /// 差异区块箭头：整块覆盖到另一侧。
    Section,
    /// 选区箭头：选中的行覆盖到另一侧。
    Selection,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GutterArrow {
    pub kind: ArrowKind,
    /// 不可用原因（目标侧只读、对齐过期）；`None` 为可点击。
    pub disabled: Option<String>,
}
impl GutterArrow {
    /// 悬停提示：可用时说明动作，禁用时说明原因。
    pub fn tooltip(&self, side: usize) -> String {
        if let Some(reason) = &self.disabled {
            return reason.clone();
        }
        let to = if side == 0 { "Right" } else { "Left" };
        match self.kind {
            ArrowKind::Section => format!("Copy section to {to}"),
            ArrowKind::Selection => format!("Copy selected lines to {to}"),
        }
    }
}

/// 一行的箭头命中矩形（`row` 为该行在视口中的矩形，行号栏贴左）。
pub fn arrow_rect(row: egui::Rect) -> egui::Rect {
    egui::Rect::from_min_size(row.min, egui::vec2(ARROW_WIDTH, row.height()))
}

pub fn paint(
    ui: &egui::Ui,
    rect: egui::Rect,
    line: Option<usize>,
    current: bool,
    arrow: Option<&GutterArrow>,
    hovered: bool,
    side: usize,
) {
    let gutter = egui::Rect::from_min_size(rect.min, egui::vec2(WIDTH, rect.height()));
    ui.painter()
        .rect_filled(gutter, 0.0, crate::chrome::theme::SURFACE);
    if let Some(line) = line {
        ui.painter().text(
            gutter.right_center() - egui::vec2(9.0, -1.0),
            egui::Align2::RIGHT_CENTER,
            (line + 1).to_string(),
            egui::FontId::proportional(9.5),
            crate::chrome::theme::TEXT_FAINT,
        );
    }
    if let Some(arrow) = arrow {
        paint_arrow(ui, gutter, arrow, hovered, side);
    }
    if current {
        // 参考的当前行标记为 2 px 宽黑线。
        for dx in [0.0, 1.0] {
            ui.painter().line_segment(
                [
                    gutter.right_top() + egui::vec2(dx, 0.0),
                    gutter.right_bottom() + egui::vec2(dx, 0.0),
                ],
                egui::Stroke::new(1.0, egui::Color32::BLACK),
            );
        }
    }
}

fn paint_arrow(ui: &egui::Ui, gutter: egui::Rect, arrow: &GutterArrow, hovered: bool, side: usize) {
    let enabled = arrow.disabled.is_none();
    let hot = enabled && hovered;
    if hot {
        ui.painter().rect_filled(
            arrow_rect(gutter).shrink2(egui::vec2(1.0, 0.5)),
            2.0,
            egui::Color32::from_rgba_unmultiplied(0, 0, 0, 24),
        );
    }
    let c = gutter.left_center() + egui::vec2(13.0, 0.0);
    let dir = if side == 0 { 1.0 } else { -1.0 };
    let (fill, outline) = match (arrow.kind, enabled, hot) {
        (_, false, _) => (egui::Color32::from_gray(200), egui::Color32::from_gray(150)),
        (ArrowKind::Section, true, false) => (
            egui::Color32::from_rgb(255, 220, 110),
            egui::Color32::from_gray(110),
        ),
        (ArrowKind::Section, true, true) => (
            egui::Color32::from_rgb(250, 190, 40),
            egui::Color32::from_gray(60),
        ),
        (ArrowKind::Selection, true, false) => (
            egui::Color32::from_rgb(90, 150, 230),
            egui::Color32::from_rgb(30, 70, 140),
        ),
        (ArrowKind::Selection, true, true) => (
            egui::Color32::from_rgb(40, 110, 215),
            egui::Color32::from_rgb(15, 45, 110),
        ),
    };
    // 选区箭头稍往回收，给尖端外的竖线（"→|"：覆盖到这一侧）留位置。
    let shift = if arrow.kind == ArrowKind::Selection {
        -1.5
    } else {
        0.0
    };
    let points = [
        (-5.0, -2.0),
        (1.0, -2.0),
        (1.0, -4.0),
        (6.0, 0.0),
        (1.0, 4.0),
        (1.0, 2.0),
        (-5.0, 2.0),
    ]
    .into_iter()
    .map(|(x, y)| c + egui::vec2((x + shift) * dir, y))
    .collect();
    ui.painter().add(egui::Shape::convex_polygon(
        points,
        fill,
        egui::Stroke::new(if hot { 1.0 } else { 0.6 }, outline),
    ));
    if arrow.kind == ArrowKind::Selection {
        let x = c.x + (6.0 + shift + 2.0) * dir;
        ui.painter().line_segment(
            [egui::pos2(x, c.y - 4.5), egui::pos2(x, c.y + 4.5)],
            egui::Stroke::new(1.5, outline),
        );
    }
}
