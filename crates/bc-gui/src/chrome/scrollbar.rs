use eframe::egui;
pub fn show(ui:&mut egui::Ui,range:std::ops::RangeInclusive<usize>,value:&mut usize)->egui::Response{ui.add(egui::Slider::new(value,range).vertical().show_value(false))}
