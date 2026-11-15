//! Dependency-free folder picker with explicit path entry and recent locations.
use crate::chrome::{controls::Role, popup, widgets, window::Window};
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
        Window::new("Base folders")
            .open(&mut open)
            .min_width(420.0)
            .show(ctx, |ui| {
                for side in 0..2 {
                    ui.label(if side == 0 { "Left folder" } else { "Right folder" });
                    ui.horizontal(|ui| {
                        ui.push_id(("folder-path", side), |ui| {
                            let text = if side == 0 {
                                &mut self.left
                            } else {
                                &mut self.right
                            };
                            widgets::text_field_width(ui, text, 300.0);
                            let recent = &self.recent;
                            let picked = widgets::menu_button(ui, "Recent", || {
                                recent
                                    .iter()
                                    .enumerate()
                                    .map(|(i, path)| {
                                        popup::item::action(path.clone(), i.to_string(), true)
                                    })
                                    .collect()
                            });
                            if let Some(path) = picked
                                .and_then(|i| i.parse::<usize>().ok())
                                .and_then(|i| self.recent.get(i).cloned())
                            {
                                if side == 0 {
                                    self.left = path;
                                } else {
                                    self.right = path;
                                }
                            }
                        });
                    });
                }
                widgets::hint(ui, "The worker validates entered paths.");
                if widgets::dialog_buttons(ui, &[("Compare folders", Role::Primary)]) == Some(0)
                    && !self.left.trim().is_empty()
                    && !self.right.trim().is_empty()
                {
                    result = Some([self.left.clone(), self.right.clone()]);
                }
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
