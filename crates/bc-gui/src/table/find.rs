use bc_core::{pattern::Pattern, CoreError};
use eframe::egui;
#[derive(Default)]
pub struct FindPanel {
    pub open: bool,
    pub query: String,
    pub replacement: String,
    pub regex: bool,
    pub case: bool,
    pub whole: bool,
    pub side: usize,
    pub last: Option<(usize, usize, std::ops::Range<usize>)>,
    /// Side that `last` was computed on; a hit position is side-local.
    pub last_side: usize,
}
#[derive(Clone, Copy)]
pub enum Action {
    Next,
    Previous,
    Replace,
    All,
}
impl FindPanel {
    pub fn pattern(&self) -> Result<Pattern, CoreError> {
        if self.regex {
            Pattern::new(&self.query, self.case)
        } else {
            Pattern::literal(&self.query, self.case)
        }
    }
    pub fn accepts(&self, text: &str, span: std::ops::Range<usize>) -> bool {
        let word = |c: char| c.is_alphanumeric() || c == '_';
        !self.whole
            || (!text[..span.start].chars().next_back().is_some_and(word)
                && !text[span.end..].chars().next().is_some_and(word))
    }
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        if !self.open {
            return None;
        }
        let mut action = None;
        egui::Panel::bottom("table_find").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Find");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut self.query)
                            .id_salt("table_find_query")
                            .desired_width(180.0),
                    )
                    .changed()
                {
                    self.last = None;
                }
                for (name, a) in [("Next", Action::Next), ("Previous", Action::Previous)] {
                    if ui.button(name).clicked() {
                        action = Some(a);
                    }
                }
                ui.checkbox(&mut self.regex, "Regex");
                ui.checkbox(&mut self.case, "Case");
                ui.checkbox(&mut self.whole, "Whole words");
                if ui.selectable_value(&mut self.side, 0, "Left").changed() {
                    self.last = None;
                }
                if ui.selectable_value(&mut self.side, 1, "Right").changed() {
                    self.last = None;
                }
                if ui.button("×").clicked() {
                    self.open = false;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Replace");
                ui.text_edit_singleline(&mut self.replacement);
                if ui.button("Replace").clicked() {
                    action = Some(Action::Replace);
                }
                if ui.button("Replace all").clicked() {
                    action = Some(Action::All);
                }
            });
        });
        action
    }
}
