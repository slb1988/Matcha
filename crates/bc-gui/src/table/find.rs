use bc_core::{pattern::Pattern, CoreError};
use crate::chrome::widgets;
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
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Find");
                if ui
                    .add(
                        widgets::field(&mut self.query)
                            .id_salt("table_find_query")
                            .desired_width(180.0),
                    )
                    .changed()
                {
                    self.last = None;
                }
                for (name, a) in [("Next", Action::Next), ("Previous", Action::Previous)] {
                    if widgets::button(ui, name).clicked() {
                        action = Some(a);
                    }
                }
                widgets::checkbox(ui, &mut self.regex, "Regex");
                widgets::checkbox(ui, &mut self.case, "Case");
                widgets::checkbox(ui, &mut self.whole, "Whole words");
                for (side, name) in [(0, "Left"), (1, "Right")] {
                    if ui
                        .add(widgets::Button::new(name).selected(self.side == side))
                        .clicked()
                        && self.side != side
                    {
                        self.side = side;
                        self.last = None;
                    }
                }
                if ui
                    .add(widgets::Button::new("×").min_width(widgets::CONTROL_HEIGHT))
                    .on_hover_text("Close")
                    .clicked()
                {
                    self.open = false;
                }
            });
            ui.horizontal(|ui| {
                ui.label("Replace");
                ui.add(widgets::field(&mut self.replacement).desired_width(180.0));
                if widgets::button(ui, "Replace").clicked() {
                    action = Some(Action::Replace);
                }
                if widgets::button(ui, "Replace all").clicked() {
                    action = Some(Action::All);
                }
            });
            ui.add_space(4.0);
        });
        action
    }
}
