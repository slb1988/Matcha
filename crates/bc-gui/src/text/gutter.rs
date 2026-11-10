//! 行号20px，箭头20px；数字保持来源行号。
use eframe::egui;
pub const WIDTH: f32 = 40.0;
pub fn paint(
    ui: &egui::Ui,
    rect: egui::Rect,
    line: Option<usize>,
    current: bool,
    difference: bool,
    side: usize,
) {
    let gutter = egui::Rect::from_min_size(rect.min, egui::vec2(WIDTH, rect.height()));
    ui.painter()
        .rect_filled(gutter, 0.0, crate::chrome::theme::PANEL_BG);
    if let Some(line) = line {
        ui.painter().text(
            gutter.right_center() - egui::vec2(8.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            (line + 1).to_string(),
            egui::FontId::proportional(8.666667),
            egui::Color32::from_rgb(105, 105, 105),
        );
        if difference {
            let c = gutter.left_center() + egui::vec2(13.0, 0.0);
            let dir = if side == 0 { 1.0 } else { -1.0 };
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
            .map(|(x, y)| c + egui::vec2(x * dir, y))
            .collect();
            ui.painter().add(egui::Shape::convex_polygon(
                points,
                egui::Color32::from_rgb(255, 220, 110),
                egui::Stroke::new(0.6, egui::Color32::from_gray(110)),
            ));
        }
    }
    if current {
        ui.painter().line_segment(
            [gutter.right_top(), gutter.right_bottom()],
            egui::Stroke::new(1.0, egui::Color32::BLACK),
        );
    }
}
