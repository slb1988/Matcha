use eframe::egui;
#[derive(Clone,Debug)]pub struct StatusField{pub text:String,pub width:Option<f32>}
pub fn show(ui:&mut egui::Ui,fields:&[StatusField]){ui.horizontal(|ui|{for f in fields{if let Some(w)=f.width{ui.add_sized([w,22.0],egui::Label::new(&f.text));}else{ui.label(&f.text);}ui.separator();}});}
