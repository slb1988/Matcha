//! 查找状态独立于编辑焦点；搜索必须通过 TextMap 提供差异范围。
use bc_engine_text::search::{FindHit, FindOptions};
use eframe::egui;
#[derive(Default)]
pub struct FindPanel {
    pub open: bool,
    pub options: FindOptions,
    pub replacement: String,
    pub last_hit: Option<FindHit>,
}
#[derive(Clone, Copy)]
pub enum FindAction {
    Next,
    Previous,
    Replace,
    All,
}
impl FindPanel {
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<FindAction> {
        if !self.open {
            return None;
        }
        let before = self.options.clone();
        let mut action = None;
        egui::Panel::bottom("text_find").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Find");
                ui.add(
                    egui::TextEdit::singleline(&mut self.options.pattern)
                        .id_salt("find_query")
                        .desired_width(200.0),
                );
                if ui.button("Next").clicked() {
                    action = Some(FindAction::Next);
                }
                if ui.button("Previous").clicked() {
                    action = Some(FindAction::Previous);
                }
                ui.checkbox(&mut self.options.backwards, "Backwards");
                if ui.button("×").clicked() {
                    self.open = false;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Replace");
                ui.add(egui::TextEdit::singleline(&mut self.replacement).desired_width(200.0));
                if ui.button("Replace").clicked() {
                    action = Some(FindAction::Replace);
                }
                if ui.button("Replace all").clicked() {
                    action = Some(FindAction::All);
                }
                ui.checkbox(&mut self.options.is_regexp, "Regex");
                ui.checkbox(&mut self.options.case_sensitive, "Case");
                ui.checkbox(&mut self.options.whole_words, "Whole words");
                ui.checkbox(&mut self.options.differences_only, "Differences only");
            });
        });
        if before != self.options {
            self.last_hit = None;
        }
        action
    }
}
