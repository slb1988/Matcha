use eframe::egui;
pub fn show(ui:&mut egui::Ui,path:&mut String)->Option<String>{let mut hit=None;ui.horizontal(|ui|{ui.label("Path:");ui.add(egui::TextEdit::singleline(path).desired_width(360.0));for (label,id) in [("↶","history-back"),("▾","browse"),("▣","save")]{if ui.small_button(label).clicked(){hit=Some(id.into());}}});hit}
