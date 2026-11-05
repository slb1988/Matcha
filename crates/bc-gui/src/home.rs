//! Home：路径框、最近文件对，以及拖放两文件直接打开。
use eframe::egui;
use std::path::PathBuf;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomeAction {
    OpenTextCompare { left: PathBuf, right: PathBuf },
}
#[derive(Debug, Default)]
pub struct HomeView {
    left: String,
    right: String,
    error: Option<String>,
    recent: Vec<(PathBuf, PathBuf)>,
}
impl HomeView {
    pub fn new(left: Option<&PathBuf>, right: Option<&PathBuf>) -> Self {
        Self {
            left: left.map(|p| p.display().to_string()).unwrap_or_default(),
            right: right.map(|p| p.display().to_string()).unwrap_or_default(),
            recent: load_recent(),
            ..Default::default()
        }
    }
    pub fn set_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
    }
    pub fn record(left: &std::path::Path, right: &std::path::Path) {
        let left = std::fs::canonicalize(left).unwrap_or_else(|_| left.to_path_buf());
        let right = std::fs::canonicalize(right).unwrap_or_else(|_| right.to_path_buf());
        let mut recent = load_recent();
        recent.retain(|p| p.0 != left || p.1 != right);
        recent.insert(0, (left, right));
        recent.truncate(12);
        if let Some(file) = recent_path() {
            if let Some(parent) = file.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let text = recent
                .iter()
                .map(|(l, r)| format!("{}\n{}\n", l.display(), r.display()))
                .collect::<String>();
            let _ = std::fs::write(file, text);
        }
    }
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<HomeAction> {
        let mut action = None;
        ui.heading("Matcha");
        ui.label("Text comparison — drop two files here or enter their paths.");
        let dropped = ui.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect::<Vec<_>>()
        });
        if dropped.len() >= 2 {
            action = Some(HomeAction::OpenTextCompare {
                left: dropped[0].clone(),
                right: dropped[1].clone(),
            });
        } else if let Some(path) = dropped.first() {
            if self.left.is_empty() {
                self.left = path.display().to_string();
            } else {
                self.right = path.display().to_string();
            }
        }
        ui.group(|ui| {
            egui::Grid::new("home_paths").num_columns(2).show(ui, |ui| {
                ui.label("Left file");
                ui.add(egui::TextEdit::singleline(&mut self.left).desired_width(650.0));
                ui.end_row();
                ui.label("Right file");
                ui.add(egui::TextEdit::singleline(&mut self.right).desired_width(650.0));
                ui.end_row();
            });
            if ui.button("Compare").clicked() {
                if self.left.trim().is_empty() || self.right.trim().is_empty() {
                    self.error = Some("Both paths are required".into());
                } else {
                    action = Some(HomeAction::OpenTextCompare {
                        left: PathBuf::from(self.left.trim()),
                        right: PathBuf::from(self.right.trim()),
                    });
                }
            }
        });
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::RED, error);
        }
        ui.separator();
        ui.heading("Recent comparisons");
        for (left, right) in &self.recent {
            if ui
                .button(format!("{}  ↔  {}", left.display(), right.display()))
                .clicked()
            {
                action = Some(HomeAction::OpenTextCompare {
                    left: left.clone(),
                    right: right.clone(),
                });
            }
        }
        action
    }
}
fn recent_path() -> Option<PathBuf> {
    bc_formats::registry::user_formats_dir()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("recent-text.txt")))
}
fn load_recent() -> Vec<(PathBuf, PathBuf)> {
    let Some(text) = recent_path().and_then(|p| std::fs::read_to_string(p).ok()) else {
        return Vec::new();
    };
    let lines: Vec<_> = text.lines().collect();
    lines
        .as_chunks::<2>()
        .0
        .iter()
        .take(12)
        .map(|p| (PathBuf::from(p[0]), PathBuf::from(p[1])))
        .collect()
}
