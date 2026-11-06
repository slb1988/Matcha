use super::grid::{GridState, GUTTER};
use bc_engine_table::{columns::Side, TableMap};
use eframe::egui;
pub fn ui(ui: &mut egui::Ui, map: &TableMap, state: &GridState, side: usize) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 25.0), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, 0.0, ui.visuals().faint_bg_color);
    let rules = map.rules();
    let hidden = rules.columns.0.len().saturating_sub(state.columns.len());
    if hidden > 0 {
        ui.painter().text(
            rect.left_center() + egui::vec2(3.0, 0.0),
            egui::Align2::LEFT_CENTER,
            format!("{hidden} hidden"),
            egui::FontId::proportional(11.0),
            ui.visuals().weak_text_color(),
        );
    }
    let sheet = if side == 0 {
        map.left.borrow()
    } else {
        map.right.borrow()
    };
    for (display, &c) in state.columns.iter().enumerate() {
        let x = rect.left() + GUTTER + state.offsets[display] - state.scroll.x;
        let width = state.offsets[display + 1] - state.offsets[display];
        if x + width < rect.left() + GUTTER || x > rect.right() {
            continue;
        }
        let col = &rules.columns.0[c];
        let source = col.index(if side == 0 { Side::Left } else { Side::Right });
        let label = source.map_or_else(|| "—".into(), |c| sheet.column_name(c));
        let label = format!(
            "{}{}{}",
            if col.handling.is_key || rules.key_columns().contains(&c) {
                "◆ "
            } else {
                ""
            },
            label,
            if col.changes.is_empty() { "" } else { " *" }
        );
        let cell =
            egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(width, rect.height()));
        ui.painter()
            .with_clip_rect(cell.intersect(rect).intersect(egui::Rect::from_min_max(
                egui::pos2(rect.left() + GUTTER, rect.top()),
                rect.max,
            )))
            .text(
                cell.left_center() + egui::vec2(6.0, 0.0),
                egui::Align2::LEFT_CENTER,
                label,
                egui::FontId::proportional(13.0),
                ui.visuals().strong_text_color(),
            );
    }
}
