//! Dependency-free folder picker with explicit path entry and recent locations.
use crate::egui;
#[derive(Default)]
pub struct PathsDialog {
    pub open: bool,
    pub left: String,
    pub right: String,
    pub recent: Vec<String>,
}
impl PathsDialog {
    pub fn show(&mut self, ctx: &egui::Context) -> Option<[String; 2]> {
        let mut result = None;
        let mut open = self.open;
        egui::Window::new("Base folders")
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label("Left folder");
                ui.text_edit_singleline(&mut self.left);
                ui.label("Right folder");
                ui.text_edit_singleline(&mut self.right);
                for side in 0..2 {
                    egui::ComboBox::from_id_salt(("recent_folder", side))
                        .selected_text(if side == 0 {
                            "Recent left"
                        } else {
                            "Recent right"
                        })
                        .show_ui(ui, |ui| {
                            for path in &self.recent {
                                if ui.button(path).clicked() {
                                    if side == 0 {
                                        self.left = path.clone();
                                    } else {
                                        self.right = path.clone();
                                    }
                                    ui.close();
                                }
                            }
                        });
                }
                if ui.button("Compare folders").clicked()
                    && !self.left.trim().is_empty()
                    && !self.right.trim().is_empty()
                {
                    result = Some([self.left.clone(), self.right.clone()]);
                }
                ui.small("Native file dialogs are deferred; the worker validates entered paths.");
            });
        self.open = open && result.is_none();
        result
    }
    pub fn remember(&mut self, paths: &[String; 2]) {
        for path in paths {
            if !self.recent.contains(path) {
                self.recent.insert(0, path.clone());
            }
        }
        self.recent.truncate(12);
    }
}
