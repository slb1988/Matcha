//! 内置只读、复制为用户格式、XML 编辑/导入/导出；由 registry 事务验证并持久化。
use crate::chrome::{
    native_dialog::{Kind, Request},
    widgets,
    window::Window,
};
use bc_formats::{
    bcff::{parse_bcfileformats, write_bcfileformats},
    registry::{user_formats_dir, FormatRegistry},
};
use eframe::egui;
#[derive(Default)]
pub struct FormatPanel {
    pub open: bool,
    xml: String,
    /// List entry whose XML is in the editor (`name · kind`).
    selected: Option<String>,
    copy_name: String,
    builtin: bool,
    message: String,
}
impl FormatPanel {
    pub fn ui(&mut self, ctx: &egui::Context, registry: &mut FormatRegistry) {
        let mut open = self.open;
        Window::new("File formats")
            .open(&mut open)
            .default_size([850.0, 650.0])
            .show(ctx, |ui| self.contents(ui, registry));
        self.open &= open;
    }
    fn contents(&mut self, ui: &mut egui::Ui, registry: &mut FormatRegistry) {
        widgets::section(ui, "Formats");
        egui::ScrollArea::vertical()
            .id_salt("formats_list")
            .max_height(220.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for f in registry.formats() {
                    let builtin = registry.is_builtin(f.kind, &f.name);
                    let key = format!("{} · {:?}", f.name, f.kind);
                    let label = if builtin {
                        format!("{key} · built-in (read-only)")
                    } else {
                        key.clone()
                    };
                    let selected = self.selected.as_ref() == Some(&key);
                    if widgets::list_row(ui, selected, label).clicked() {
                        self.xml = write_bcfileformats(std::slice::from_ref(f)).unwrap_or_default();
                        self.builtin = builtin;
                        self.copy_name = format!("{} copy", f.name);
                        self.selected = Some(key);
                    }
                }
            });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label("Copy name");
            widgets::text_field_width(ui, &mut self.copy_name, 220.0);
            if widgets::button(ui, "Copy to user format").clicked() {
                match parse_bcfileformats(&self.xml) {
                    Ok(mut formats) if !formats.is_empty() && !self.copy_name.trim().is_empty() => {
                        formats[0].name = self.copy_name.trim().into();
                        match write_bcfileformats(&formats[..1]) {
                            Ok(xml) => {
                                self.xml = xml;
                                self.builtin = false;
                                self.selected = None;
                            }
                            Err(e) => self.message = e.to_string(),
                        }
                    }
                    _ => self.message = "Select a format and enter a name".into(),
                }
            }
            if widgets::button(ui, "New text format").clicked() {
                self.xml = "<BCFileFormats>\n<TTextFormat Value=\"Custom\"><Mask Value=\"*.custom\"/></TTextFormat>\n</BCFileFormats>".into();
                self.builtin = false;
                self.selected = None;
            }
        });
        ui.add_space(6.0);
        widgets::section(ui, "Definition (XML)");
        egui::ScrollArea::vertical()
            .id_salt("format_xml")
            .max_height(280.0)
            .show(ui, |ui| {
                ui.add_enabled(
                    !self.builtin,
                    egui::TextEdit::multiline(&mut self.xml)
                        .code_editor()
                        .desired_width(f32::INFINITY)
                        .desired_rows(12),
                );
            });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.builtin,
                    widgets::Button::new("Validate and save user formats").primary(),
                )
                .clicked()
            {
                let mut staging = registry.clone();
                let result = staging.import_xml(&self.xml).and_then(|n| {
                    staging.save_user_dir(&user_formats_dir()?)?;
                    Ok(n)
                });
                match result {
                    Ok(n) => {
                        *registry = staging;
                        self.message =
                            format!("Saved {n} user format(s); select them in the pane format menu");
                    }
                    Err(e) => self.message = e.to_string(),
                }
            }
            if widgets::button(ui, "Import…").clicked() {
                let picked = Request::new(Kind::OpenFile, "Import File Formats")
                    .filter("File formats (*.xml)", &["xml"])
                    .show();
                if let Some(path) = picked {
                    match std::fs::read_to_string(&path) {
                        Ok(xml) => {
                            self.xml = xml;
                            self.builtin = false;
                            self.selected = None;
                            self.message = "Review, then validate and save".into();
                        }
                        Err(e) => self.message = e.to_string(),
                    }
                }
            }
            if widgets::button(ui, "Export user formats…").clicked() {
                let picked = Request::new(Kind::SaveFile, "Export User File Formats")
                    .initial("BCFileFormats.xml")
                    .filter("File formats (*.xml)", &["xml"])
                    .show();
                if let Some(path) = picked {
                    let result = registry
                        .export_xml()
                        .and_then(|xml| std::fs::write(&path, xml).map_err(Into::into));
                    self.message = match result {
                        Ok(()) => format!("Exported to {}", path.display()),
                        Err(e) => e.to_string(),
                    };
                }
            }
        });
        if !self.message.is_empty() {
            widgets::hint(ui, &self.message);
        }
    }
}
