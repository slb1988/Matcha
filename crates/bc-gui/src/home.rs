//! Home view placeholder – BC `THomeView` (session list / new-session buttons).

use eframe::egui;
use std::path::PathBuf;

/// Requests the Home page can emit to the application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomeAction {
    OpenTextCompare { left: PathBuf, right: PathBuf },
}

/// Home page state.
#[derive(Debug, Default)]
pub struct HomeView {
    left: String,
    right: String,
    error: Option<String>,
}

impl HomeView {
    pub fn new(left: Option<&PathBuf>, right: Option<&PathBuf>) -> Self {
        Self {
            left: left.map(|p| p.display().to_string()).unwrap_or_default(),
            right: right.map(|p| p.display().to_string()).unwrap_or_default(),
            error: None,
        }
    }

    pub fn set_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
    }

    /// Draw; returns an action when the user asked for something.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<HomeAction> {
        let mut action = None;
        ui.heading("Home");
        ui.label("Scaffold placeholder. Session types (Text / Table / Hex / Picture / Folder …) will be listed here.");
        ui.separator();

        ui.group(|ui| {
            ui.label("Text Compare prototype");
            egui::Grid::new("home_paths").num_columns(2).show(ui, |ui| {
                ui.label("Left file");
                ui.add(egui::TextEdit::singleline(&mut self.left).desired_width(600.0));
                ui.end_row();
                ui.label("Right file");
                ui.add(egui::TextEdit::singleline(&mut self.right).desired_width(600.0));
                ui.end_row();
            });
            if ui.button("Open").clicked() {
                if self.left.trim().is_empty() || self.right.trim().is_empty() {
                    self.error = Some("both paths are required".into());
                } else {
                    action = Some(HomeAction::OpenTextCompare {
                        left: PathBuf::from(self.left.trim()),
                        right: PathBuf::from(self.right.trim()),
                    });
                }
            }
            if let Some(err) = &self.error {
                ui.colored_label(egui::Color32::RED, err);
            }
        });

        ui.separator();
        ui.add_enabled(false, egui::Button::new("Folder Compare (not in scaffold)"));
        ui.add_enabled(false, egui::Button::new("Table Compare (not in scaffold)"));
        action
    }
}
