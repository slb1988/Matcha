use eframe::egui;
#[derive(Clone,Debug,PartialEq,Eq)]pub enum ButtonKind{Normal,Toggle,Split,Dropdown,Disabled}
#[derive(Clone,Debug)]pub struct ToolItem{pub label:String,pub action_id:String,pub kind:ButtonKind,pub selected:bool}
pub fn show(ui:&mut egui::Ui,items:&[ToolItem])->Option<String>{let mut hit=None;ui.horizontal(|ui|{for i in items{let mut text=format!("◈\n{}",i.label);if i.kind==ButtonKind::Split{text.push_str(" ▾");}let r=ui.add_enabled(i.kind!=ButtonKind::Disabled,egui::Button::new(text).min_size(egui::vec2(56.0,38.0)));if r.clicked(){hit=Some(i.action_id.clone());}}});hit}
