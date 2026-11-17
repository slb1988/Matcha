use crate::chrome::{controls::Role, theme, widgets, window::Window};
use crate::egui;
use bc_core::pattern::Pattern;
#[derive(Default)]
pub struct FindDialog {
    pub open: bool,
    pub text: String,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
    pub wrap: bool,
    pub backwards: bool,
    pub side: usize,
    pub error: Option<String>,
}
impl FindDialog {
    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut search = false;
        Window::new("Find filename")
            .open(&mut self.open)
            .show(ctx, |ui| {
                widgets::text_field(ui, &mut self.text);
                ui.horizontal(|ui| {
                    widgets::checkbox(ui, &mut self.case_sensitive, "Case");
                    widgets::checkbox(ui, &mut self.whole_word, "Whole word");
                    widgets::checkbox(ui, &mut self.regex, "Regex");
                });
                ui.horizontal(|ui| {
                    widgets::checkbox(ui, &mut self.wrap, "Wrap");
                    widgets::checkbox(ui, &mut self.backwards, "Up");
                });
                ui.horizontal(|ui| {
                    ui.label("Side");
                    widgets::Combo::new("find_side", ["Left", "Right", "Either"][self.side]).show(
                        ui,
                        |list| {
                            for (s, label) in ["Left", "Right", "Either"].iter().enumerate() {
                                list.selectable_value(&mut self.side, s, *label);
                            }
                        },
                    );
                });
                if let Some(error) = &self.error {
                    ui.colored_label(theme::palette(ui.ctx()).danger, error);
                }
                search = widgets::dialog_buttons(ui, &[("Find", Role::Primary)]) == Some(0);
            });
        search
    }
    pub fn pattern(&self) -> Result<Pattern, String> {
        if self.regex {
            Pattern::new(&self.text, self.case_sensitive)
        } else {
            Pattern::literal(&self.text, self.case_sensitive)
        }
        .map_err(|e| e.to_string())
    }
    pub fn matches(&self, pattern: &Pattern, name: &str) -> bool {
        let mut at = 0;
        while let Some(found) = pattern.find_at(name, at) {
            if !self.whole_word {
                return true;
            }
            let word = |c: char| c.is_alphanumeric() || c == '_';
            let before = name[..found.span.start]
                .chars()
                .next_back()
                .is_some_and(word);
            let after = name[found.span.end..].chars().next().is_some_and(word);
            if !before && !after {
                return true;
            }
            if found.span.end == name.len() {
                break;
            }
            at = found.span.start
                + name[found.span.start..]
                    .chars()
                    .next()
                    .map_or(1, char::len_utf8);
        }
        false
    }
}
