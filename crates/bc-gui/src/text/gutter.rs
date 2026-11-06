//! 行号和空隙不依赖显示过滤，始终使用来源行号。
use eframe::egui;
pub const WIDTH: f32 = 72.0;
pub fn paint(ui: &egui::Ui, rect: egui::Rect, line: Option<usize>, current: bool) {
    let gutter = egui::Rect::from_min_size(rect.min, egui::vec2(WIDTH, rect.height()));
    ui.painter()
        .rect_filled(gutter, 0.0, ui.visuals().faint_bg_color);
    if let Some(line) = line {
        ui.painter().text(
            gutter.right_center() - egui::vec2(8.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            (line + 1).to_string(),
            egui::FontId::monospace(12.0),
            if current {
                ui.visuals().text_color()
            } else {
                ui.visuals().weak_text_color()
            },
        );
    } else {
        ui.painter().text(
            gutter.center(),
            egui::Align2::CENTER_CENTER,
            "·",
            egui::FontId::monospace(12.0),
            ui.visuals().weak_text_color(),
        );
    }
}
