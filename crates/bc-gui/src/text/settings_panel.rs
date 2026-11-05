//! 会话比较规则与全局颜色/备份偏好；规则验证后才交给运行中的视图。
use bc_core::align::AlignStrategy;
use bc_engine_text::{importance::CompiledComparison, rules::TextRules};
use bc_formats::{
    text_rules::{Replacement, UnimportantText},
    ElementKind,
};
use bc_session::prefs::Prefs;
use eframe::egui;

#[derive(Default)]
pub struct SettingsPanel {
    pub open: bool,
    pub draft: TextRules,
    error: Option<String>,
}
impl SettingsPanel {
    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        rules: &mut TextRules,
        context: &mut usize,
        prefs: &mut Prefs,
    ) -> bool {
        if !self.open {
            return false;
        }
        let old = rules.clone();
        egui::Window::new("Session settings")
            .open(&mut self.open)
            .default_width(560.0)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.heading("Alignment");
                ui.horizontal(|ui| {
                    for (strategy, name) in [
                        (AlignStrategy::Smart, "Standard"),
                        (AlignStrategy::Quick, "Myers O(ND)"),
                        (AlignStrategy::Patience, "Patience"),
                        (AlignStrategy::None, "Unaligned"),
                    ] {
                        ui.selectable_value(&mut self.draft.alignment.strategy, strategy, name);
                    }
                });
                ui.add_enabled_ui(
                    self.draft.alignment.strategy == AlignStrategy::Smart,
                    |ui| {
                        ui.checkbox(
                            &mut self.draft.alignment.use_closeness,
                            "Use closeness matching",
                        );
                    },
                );
                ui.checkbox(
                    &mut self.draft.alignment.never_align_differences,
                    "Never align differences",
                );
                ui.horizontal(|ui| {
                    ui.label("Skew tolerance");
                    ui.add(
                        egui::DragValue::new(&mut self.draft.alignment.skew_tolerance)
                            .range(0..=100000),
                    );
                });
                ui.separator();
                ui.heading("Importance");
                let i = &mut self.draft.comparison.importance;
                ui.checkbox(
                    &mut i.leading_whitespace_unimportant,
                    "Leading whitespace unimportant",
                );
                ui.checkbox(
                    &mut i.embedded_whitespace_unimportant,
                    "Embedded whitespace unimportant",
                );
                ui.checkbox(
                    &mut i.trailing_whitespace_unimportant,
                    "Trailing whitespace unimportant",
                );
                ui.checkbox(&mut i.case_unimportant, "Character case unimportant");
                ui.checkbox(&mut i.line_endings_unimportant, "Line endings unimportant");
                ui.label("Grammar elements (checked = important)");
                ui.horizontal_wrapped(|ui| {
                    for kind in ElementKind::ALL {
                        let mut important = !i.unimportant.contains(&kind);
                        if ui.checkbox(&mut important, kind.as_str()).changed() {
                            i.unimportant.retain(|e| *e != kind);
                            if !important {
                                i.unimportant.push(kind);
                            }
                        }
                    }
                });
                ui.checkbox(
                    &mut self.draft.ignore_unimportant,
                    "Ignore unimportant differences",
                );
                ui.horizontal(|ui| {
                    ui.label("Context lines");
                    ui.add(egui::DragValue::new(context).range(0..=1000));
                });
                ui.collapsing("Unimportant text", |ui| {
                    let mut remove = None;
                    for (n, r) in self
                        .draft
                        .comparison
                        .unimportant_text
                        .iter_mut()
                        .enumerate()
                    {
                        ui.push_id(n, |ui| {
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut r.pattern);
                                ui.checkbox(&mut r.is_regexp, "Regex");
                                ui.checkbox(&mut r.case_sensitive, "Case");
                                if ui.button("Remove").clicked() {
                                    remove = Some(n);
                                }
                            });
                            egui::ComboBox::from_id_salt("element")
                                .selected_text(r.only_in.map_or("Any element", |k| k.as_str()))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut r.only_in, None, "Any element");
                                    for kind in ElementKind::ALL {
                                        ui.selectable_value(
                                            &mut r.only_in,
                                            Some(kind),
                                            kind.as_str(),
                                        );
                                    }
                                });
                        });
                    }
                    if let Some(n) = remove {
                        self.draft.comparison.unimportant_text.remove(n);
                    }
                    if ui.button("New unimportant text").clicked() {
                        self.draft
                            .comparison
                            .unimportant_text
                            .push(UnimportantText {
                                pattern: String::new(),
                                is_regexp: false,
                                case_sensitive: true,
                                only_in: None,
                            });
                    }
                });
                ui.collapsing("Replacements", |ui| {
                    let mut remove = None;
                    for (n, r) in self.draft.comparison.replacements.iter_mut().enumerate() {
                        ui.push_id(n, |ui| {
                            ui.horizontal(|ui| {
                                ui.label("From");
                                ui.text_edit_singleline(&mut r.from);
                                ui.label("To");
                                ui.text_edit_singleline(&mut r.to);
                            });
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut r.is_regexp, "Regex");
                                ui.checkbox(&mut r.case_sensitive, "Case");
                                ui.checkbox(&mut r.whole_words, "Whole words");
                                ui.checkbox(&mut r.bidirectional, "Both directions");
                                if ui.button("Remove").clicked() {
                                    remove = Some(n);
                                }
                            });
                        });
                    }
                    if let Some(n) = remove {
                        self.draft.comparison.replacements.remove(n);
                    }
                    if ui.button("New replacement").clicked() {
                        self.draft.comparison.replacements.push(Replacement {
                            from: String::new(),
                            to: String::new(),
                            is_regexp: false,
                            case_sensitive: true,
                            whole_words: false,
                            bidirectional: false,
                        });
                    }
                });
                ui.collapsing("Preferences: colors and backups", |ui| {
                    for (name, rgb) in [
                        ("Important", &mut prefs.colors.important),
                        ("Unimportant", &mut prefs.colors.unimportant),
                        ("Inserted / deleted", &mut prefs.colors.inserted),
                    ] {
                        ui.horizontal(|ui| {
                            ui.color_edit_button_srgb(rgb);
                            ui.label(name);
                        });
                    }
                    ui.checkbox(&mut prefs.backups.enabled, "Create backup before saving");
                    let mut ext = prefs.backups.extension.clone().unwrap_or_default();
                    ui.horizontal(|ui| {
                        ui.label("Suffix (empty = ~ prefix)");
                        if ui.text_edit_singleline(&mut ext).changed() {
                            prefs.backups.extension = (!ext.is_empty()).then_some(ext);
                        }
                    });
                });
                match CompiledComparison::new(&self.draft.comparison) {
                    Ok(_) => {
                        *rules = self.draft.clone();
                        self.error = None;
                    }
                    Err(e) => self.error = Some(e.to_string()),
                }
                if let Some(error) = &self.error {
                    ui.colored_label(egui::Color32::RED, error);
                }
            });
        old != *rules
    }
}
