use bc_engine_table::{compare::Importance, sheets::SheetPairs};
use bc_session::prefs::DiffColors;
use eframe::egui;
pub fn ui(
    ui: &mut egui::Ui,
    names: &[Vec<String>; 2],
    pairs: &SheetPairs,
    states: &[Option<Importance>],
    active: usize,
    colors: &DiffColors,
) -> Option<usize> {
    let mut target = None;
    egui::Panel::bottom("table_sheet_tabs").show(ui, |ui| {
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                for (i, pair) in pairs.0.iter().enumerate() {
                    let left = pair
                        .left
                        .and_then(|n| names[0].get(n))
                        .map_or("—", String::as_str);
                    let right = pair
                        .right
                        .and_then(|n| names[1].get(n))
                        .map_or("—", String::as_str);
                    let name = if left == right {
                        left.into()
                    } else {
                        format!("{left} ↔ {right}")
                    };
                    let color = states
                        .get(i)
                        .copied()
                        .flatten()
                        .map_or(egui::Color32::GRAY, |s| {
                            if s == Importance::Same {
                                ui.visuals().text_color()
                            } else {
                                super::grid::color(colors, s, false, 255)
                            }
                        });
                    if ui
                        .selectable_label(
                            i == active,
                            egui::RichText::new(format!("● {name}")).color(color),
                        )
                        .clicked()
                    {
                        target = Some(i);
                    }
                }
            });
        });
    });
    target
}
