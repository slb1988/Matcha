use eframe::egui;
pub fn vertical(ui:&mut egui::Ui,_id_source:impl std::hash::Hash)->egui::Response{ui.add(egui::Separator::default().vertical().spacing(6.0))}
