//! Log rendering is virtualized; complete records remain in the engine's shared log.
use crate::egui;
use bc_engine_dir::op_exec::OpLog;
use std::time::{Duration, Instant};
pub struct LogPanel {
    pub open: bool,
    lines: Vec<String>,
    last: Instant,
    pub save_path: String,
}
impl Default for LogPanel {
    fn default() -> Self {
        Self {
            open: false,
            lines: Vec::new(),
            last: Instant::now() - Duration::from_secs(1),
            save_path: String::new(),
        }
    }
}
impl LogPanel {
    pub fn ui(&mut self, ui: &mut egui::Ui, log: &OpLog) -> Option<String> {
        if !self.open {
            return None;
        }
        if self.last.elapsed() > Duration::from_millis(200) {
            self.lines = log
                .snapshot()
                .into_iter()
                .map(|e| {
                    format!(
                        "{:?} {:?}: {} -> {} {}",
                        e.state,
                        e.action,
                        e.source.as_ref().map_or(String::new(), ToString::to_string),
                        e.destination
                            .as_ref()
                            .map_or(String::new(), ToString::to_string),
                        e.message
                    )
                })
                .collect();
            self.last = Instant::now();
        }
        let mut save = None;
        egui::Panel::bottom("folder_log")
            .resizable(true)
            .default_size(130.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("Operation log ({} events)", self.lines.len()));
                    crate::chrome::widgets::text_field_width(ui, &mut self.save_path, 260.0);
                    if crate::chrome::widgets::button(ui, "Save log").clicked()
                        && !self.save_path.is_empty()
                    {
                        save = Some(self.save_path.clone());
                    }
                });
                egui::ScrollArea::vertical()
                    .id_salt("operation_log_rows")
                    .stick_to_bottom(true)
                    .show_rows(ui, 16.0, self.lines.len(), |ui, range| {
                        for index in range {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&self.lines[index]).monospace(),
                                )
                                .truncate(),
                            );
                        }
                    });
            });
        save
    }
}
