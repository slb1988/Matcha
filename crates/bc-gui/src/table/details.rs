use super::grid::Cursor;
use bc_engine_table::{compare::Importance, TableMap};
use bc_session::prefs::DiffColors;
use eframe::egui;
pub fn ui(ui: &mut egui::Ui, map: &TableMap, cursor: Cursor) {
    egui::Panel::bottom("table_details")
        .resizable(true)
        .default_size(140.0)
        .show(ui, |ui| {
            ui.label("Text Details — full cell text / cached formula / number format");
            let row = super::grid::mapped(map, cursor.row);
            let rules = map.rules();
            ui.columns(2, |cols| {
                for (side, ui) in cols.iter_mut().enumerate() {
                    let sheet = if side == 0 {
                        map.left.borrow()
                    } else {
                        map.right.borrow()
                    };
                    let r = if side == 0 { row.left } else { row.right };
                    let c = rules.columns.0.get(cursor.column).and_then(|c| {
                        if side == 0 {
                            c.left
                        } else {
                            c.right
                        }
                    });
                    let cell = r.and_then(|r| c.and_then(|c| sheet.cell(r, c)));
                    let text = r
                        .and_then(|r| c.and_then(|c| sheet.display_at(r, c)))
                        .unwrap_or_default();
                    egui::ScrollArea::both()
                        .id_salt(("table_details_scroll", side))
                        .max_height(90.0)
                        .show(ui, |ui| {
                            ui.label(text);
                            ui.monospace(
                                cell.and_then(|c| c.formula.as_deref())
                                    .unwrap_or("(no formula)"),
                            );
                            ui.label(
                                sheet
                                    .style(cell.and_then(|c| c.style))
                                    .and_then(|s| s.number_format.as_deref())
                                    .unwrap_or("(no number format)"),
                            );
                        });
                }
            });
        });
}
pub fn thumbnail(
    ui: &mut egui::Ui,
    bins: &[Importance],
    total: usize,
    visible: std::ops::Range<usize>,
    colors: &DiffColors,
) -> Option<usize> {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(20.0, ui.available_height()),
        egui::Sense::click_and_drag(),
    );
    ui.painter()
        .rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
    for (i, &state) in bins.iter().enumerate() {
        if state != Importance::Same {
            ui.painter().rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(rect.left(), rect.top() + rect.height() * i as f32 / 512.0),
                    egui::vec2(rect.width(), (rect.height() / 512.0).max(1.0)),
                ),
                0.0,
                super::grid::color(colors, state, false, 220),
            );
        }
    }
    let y = |r: usize| rect.top() + rect.height() * r.min(total) as f32 / total.max(1) as f32;
    ui.painter().rect_stroke(
        egui::Rect::from_min_max(
            egui::pos2(rect.left(), y(visible.start)),
            egui::pos2(rect.right(), y(visible.end).max(y(visible.start) + 3.0)),
        ),
        0.0,
        egui::Stroke::new(1.0, egui::Color32::WHITE),
        egui::StrokeKind::Inside,
    );
    if response.clicked() || response.dragged() {
        response
            .interact_pointer_pos()
            .map(|p| (((p.y - rect.top()) / rect.height()).clamp(0.0, 1.0) * total as f32) as usize)
            .map(|r| r.min(total.saturating_sub(1)))
    } else {
        None
    }
}
