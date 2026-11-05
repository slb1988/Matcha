//! 内置只读、复制为用户格式、XML 编辑/导入/导出；由 registry 事务验证并持久化。
use bc_formats::{
    bcff::{parse_bcfileformats, write_bcfileformats},
    registry::{user_formats_dir, FormatRegistry},
};
use eframe::egui;
#[derive(Default)]
pub struct FormatPanel {
    pub open: bool,
    xml: String,
    path: String,
    copy_name: String,
    builtin: bool,
    message: String,
}
impl FormatPanel {
    pub fn ui(&mut self, ctx: &egui::Context, registry: &mut FormatRegistry) {
        if !self.open {
            return;
        }
        egui::Window::new("File formats").open(&mut self.open).default_size([850.0,650.0]).show(ctx,|ui|{
            ui.horizontal(|ui|{
                egui::ScrollArea::vertical().id_salt("formats_list").max_height(220.0).show(ui,|ui|{
                    for f in registry.formats(){
                        let builtin=registry.is_builtin(f.kind,&f.name);
                        if ui.selectable_label(false,format!("{} · {:?}{}",f.name,f.kind,if builtin{" · built-in (read-only)"}else{""})).clicked(){
                            self.xml=write_bcfileformats(std::slice::from_ref(f)).unwrap_or_default();self.builtin=builtin;self.copy_name=format!("{} copy",f.name);
                        }
                    }
                });
            });
            ui.horizontal(|ui|{
                ui.label("Copy name");ui.text_edit_singleline(&mut self.copy_name);
                if ui.button("Copy to user format").clicked(){
                    match parse_bcfileformats(&self.xml){
                        Ok(mut formats) if !formats.is_empty() && !self.copy_name.trim().is_empty()=>{
                            formats[0].name=self.copy_name.trim().into();
                            match write_bcfileformats(&formats[..1]){Ok(xml)=>{self.xml=xml;self.builtin=false;},Err(e)=>self.message=e.to_string()}
                        }
                        _=>self.message="Select a format and enter a name".into(),
                    }
                }
                if ui.button("New text format").clicked(){self.xml="<BCFileFormats>\n<TTextFormat Value=\"Custom\"><Mask Value=\"*.custom\"/></TTextFormat>\n</BCFileFormats>".into();self.builtin=false;}
            });
            egui::ScrollArea::vertical().id_salt("format_xml").max_height(280.0).show(ui,|ui|{
                ui.add_enabled(!self.builtin,egui::TextEdit::multiline(&mut self.xml).code_editor().desired_width(f32::INFINITY).desired_rows(12));
            });
            if ui.add_enabled(!self.builtin,egui::Button::new("Validate and save user formats")).clicked(){
                let mut staging=registry.clone();
                let result=staging.import_xml(&self.xml).and_then(|n|{staging.save_user_dir(&user_formats_dir()?)?;Ok(n)});
                match result{Ok(n)=>{*registry=staging;self.message=format!("Saved {n} user format(s); select them in the pane format menu");},Err(e)=>self.message=e.to_string()}
            }
            ui.horizontal(|ui|{
                ui.label("XML path");ui.text_edit_singleline(&mut self.path);
                if ui.button("Import").clicked(){
                    match std::fs::read_to_string(&self.path){Ok(xml)=>{self.xml=xml;self.builtin=false;self.message="Review, then validate and save".into();},Err(e)=>self.message=e.to_string()}
                }
                if ui.button("Export users").clicked(){
                    let result=registry.export_xml().and_then(|xml|std::fs::write(&self.path,xml).map_err(Into::into));
                    self.message=match result{Ok(())=>"Exported".into(),Err(e)=>e.to_string()};
                }
            });
            ui.label(&self.message);
        });
    }
}
