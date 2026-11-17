//! 行号20px，箭头20px；数字保持来源行号。箭头可点击：区块箭头整块复制，选区箭头复制选中行。
//! 与参考一致：每个区块（或选区）只在首行画一个黄色块箭头，其余行画一条灰色竖括号，末行带一个向右的脚。
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
/// 本行在区块中的位置：首行画箭头（可点击），其余行画括号线。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GutterMark {
    Arrow,
    /// 括号竖线；`last` 为区块末行（画向右的脚）。
    Bracket {
        last: bool,
    },
}
impl GutterMark {
    /// `row` 在首行为 `first`、末行为 `last`（含）的区块中的标记。
    pub fn at(row: usize, first: usize, last: usize) -> Self {
        if row == first {
            Self::Arrow
        } else {
            Self::Bracket { last: row >= last }
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GutterArrow {
    pub kind: ArrowKind,
    pub mark: GutterMark,
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
            // 参考的行号为中灰（比界面的淡灰深，便于辨认）。
            egui::Color32::from_gray(128),
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

/// 箭头左缘距行号栏左缘（像素中心）。
const ARROW_LEFT: f32 = 3.5;
/// 括号竖线距箭头左缘。
const BRACKET_INSET: f32 = 3.0;
pub(crate) const BRACKET: egui::Color32 = egui::Color32::from_rgb(105, 105, 105);
/// 区块箭头描边（常态）。
pub(crate) const SECTION_OUTLINE: egui::Color32 = egui::Color32::from_rgb(88, 78, 50);
/// 选区箭头描边（常态）。
pub(crate) const SELECTION_OUTLINE: egui::Color32 = egui::Color32::from_rgb(30, 64, 130);

fn paint_arrow(ui: &egui::Ui, gutter: egui::Rect, arrow: &GutterArrow, hovered: bool, side: usize) {
    let painter = ui.painter();
    // 像素对齐：1 px 描边落在整像素上，保持锐利。
    let left = gutter.left().round() + ARROW_LEFT;
    let cy = gutter.center().y.floor() + 0.5;
    if let GutterMark::Bracket { last } = arrow.mark {
        let x = left + BRACKET_INSET;
        let stroke = egui::Stroke::new(1.0, BRACKET);
        let bottom = if last {
            gutter.bottom().round() - 3.5
        } else {
            gutter.bottom()
        };
        painter.line_segment([egui::pos2(x, gutter.top()), egui::pos2(x, bottom)], stroke);
        if last {
            painter.line_segment(
                [egui::pos2(x - 0.5, bottom), egui::pos2(x + 4.0, bottom)],
                stroke,
            );
        }
        return;
    }
    let hot = arrow.disabled.is_none() && hovered;
    if hot {
        painter.rect_filled(
            arrow_rect(gutter).shrink2(egui::vec2(1.0, 0.5)),
            2.0,
            egui::Color32::from_rgba_unmultiplied(0, 0, 0, 24),
        );
    }
    // 参考：只读目标侧的箭头同样是黄色（点击时在状态栏说明原因）。
    let (top, bottom, outline) = match (arrow.kind, hot) {
        (ArrowKind::Section, false) => (
            egui::Color32::from_rgb(255, 236, 140),
            egui::Color32::from_rgb(238, 186, 40),
            SECTION_OUTLINE,
        ),
        (ArrowKind::Section, true) => (
            egui::Color32::from_rgb(255, 220, 100),
            egui::Color32::from_rgb(225, 160, 10),
            egui::Color32::from_rgb(50, 42, 25),
        ),
        (ArrowKind::Selection, false) => (
            egui::Color32::from_rgb(190, 220, 255),
            egui::Color32::from_rgb(80, 140, 225),
            SELECTION_OUTLINE,
        ),
        (ArrowKind::Selection, true) => (
            egui::Color32::from_rgb(150, 195, 250),
            egui::Color32::from_rgb(40, 105, 210),
            egui::Color32::from_rgb(15, 45, 110),
        ),
    };
    // 13×9 px 块箭头：杆高 5 px，头高 9 px、长 5 px；左窗格指右，右窗格指左。
    let flip = |x: f32| if side == 0 { x } else { 12.0 - x };
    let at = |x: f32, y: f32| egui::pos2(left + flip(x), cy + y);
    let outline_points: Vec<egui::Pos2> = [
        (0.0, -2.0),
        (7.0, -2.0),
        (7.0, -4.0),
        (12.0, 0.0),
        (7.0, 4.0),
        (7.0, 2.0),
        (0.0, 2.0),
    ]
    .into_iter()
    .map(|(x, y)| at(x, y))
    .collect();
    // 竖向渐变填充：杆两个三角形 + 头一个三角形（多边形非凸，不用 convex_polygon）。
    let shade = |y: f32| {
        let t = ((y + 4.0) / 8.0).clamp(0.0, 1.0);
        top.lerp_to_gamma(bottom, t)
    };
    let mut mesh = egui::Mesh::default();
    for (x, y) in [
        (0.0, -2.0),
        (7.0, -2.0),
        (7.0, 2.0),
        (0.0, 2.0),
        (7.0, -4.0),
        (12.0, 0.0),
        (7.0, 4.0),
    ] {
        mesh.colored_vertex(at(x, y), shade(y));
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    mesh.add_triangle(4, 5, 6);
    painter.add(egui::Shape::mesh(mesh));
    painter.add(egui::Shape::closed_line(
        outline_points,
        egui::Stroke::new(1.0, outline),
    ));
}
