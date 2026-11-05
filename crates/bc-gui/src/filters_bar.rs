//! Filter controls and presets use the script-facing engine's parser.
use crate::egui;
use bc_engine_dir::{
    filter::{DisplayFilter, FolderDisplay},
    filter_script::FilterPresets,
    view::ViewOptions,
};
#[derive(Debug, Default)]
pub struct FiltersBar {
    pub names: String,
    pub other: String,
    pub options: ViewOptions,
    pub presets: FilterPresets,
    pub preset_name: String,
    pub error: Option<String>,
    pub show_none_option: bool,
    pub content: String,
    pub exclude_containing: bool,
}
#[derive(Debug, Default)]
pub struct FilterChange {
    pub apply: bool,
    pub save_presets: bool,
}
impl FiltersBar {
    pub fn ui(&mut self, ui: &mut egui::Ui, allow_flatten: bool) -> FilterChange {
        let mut change = FilterChange::default();
        ui.horizontal(|ui| {
            ui.label("Filters");
            let edited = ui.add(
                egui::TextEdit::singleline(&mut self.names)
                    .desired_width(280.0)
                    .hint_text("*.rs;-*.bak;-Backup\\"),
            );
            if edited.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                change.apply = true;
            }
            if ui.button("Apply filters").clicked() {
                change.apply = true;
            }
            egui::ComboBox::from_id_salt("name_presets")
                .selected_text("Presets")
                .show_ui(ui, |ui| {
                    for preset in &self.presets.items {
                        if ui.button(&preset.name).clicked() {
                            self.names = preset.masks.clone();
                            change.apply = true;
                            ui.close();
                        }
                    }
                });
            ui.menu_button("Add preset", |ui| {
                ui.text_edit_singleline(&mut self.preset_name);
                if ui.button("Save named preset").clicked() {
                    match self.presets.upsert(&self.preset_name, &self.names) {
                        Ok(()) => {
                            self.error = None;
                            change.save_presets = true;
                            ui.close();
                        }
                        Err(e) => self.error = Some(e),
                    }
                }
            });
        });
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("display_filter")
                .selected_text(display_label(self.options.display))
                .show_ui(ui, |ui| {
                    for value in DisplayFilter::ALL {
                        if value == DisplayFilter::None && !self.show_none_option {
                            continue;
                        }
                        change.apply |= ui
                            .selectable_value(
                                &mut self.options.display,
                                value,
                                display_label(value),
                            )
                            .changed();
                    }
                });
            egui::ComboBox::from_id_salt("folder_filter")
                .selected_text(folder_label(self.options.folders))
                .show_ui(ui, |ui| {
                    for value in [
                        FolderDisplay::AlwaysShow,
                        FolderDisplay::FilesAndStructure,
                        FolderDisplay::FilesOnly,
                        FolderDisplay::Flatten,
                    ] {
                        if value == FolderDisplay::Flatten && !allow_flatten {
                            continue;
                        }
                        change.apply |= ui
                            .selectable_value(&mut self.options.folders, value, folder_label(value))
                            .changed();
                    }
                });
            change.apply |= ui
                .checkbox(&mut self.options.suppress_filters, "Suppress filters")
                .changed();
            change.apply |= ui
                .checkbox(&mut self.options.ignore_unimportant, "Ignore unimportant")
                .changed();
            ui.menu_button("Other filters", |ui| {
                ui.label("Script fields: cutoff:<7days size:>1MB attrib:-sh");
                ui.add(egui::TextEdit::singleline(&mut self.other).desired_width(400.0));
                ui.label("UTF-8 content text (empty disables)");
                ui.text_edit_singleline(&mut self.content);
                ui.checkbox(
                    &mut self.exclude_containing,
                    "Exclude containing (otherwise exclude not containing)",
                );
                if ui.button("Apply other filters").clicked() {
                    change.apply = true;
                    ui.close();
                }
            });
        });
        if !self.other.trim().is_empty() || !self.content.is_empty() {
            ui.colored_label(
                egui::Color32::LIGHT_BLUE,
                "Other filters active; filtered partners remain visible in teal.",
            );
        }
        if let Some(e) = &self.error {
            ui.colored_label(egui::Color32::LIGHT_RED, e);
        }
        change
    }
}
pub fn display_label(value: DisplayFilter) -> &'static str {
    use DisplayFilter::*;
    match value {
        All => "Show all",
        Mismatches => "Show differences",
        Matches => "Show same",
        NoOrphans => "No orphans",
        MismatchesNoOrphans => "Differences, no orphans",
        Orphans => "Orphans",
        LeftNewer => "Left newer",
        RightNewer => "Right newer",
        LeftNewerOrphans => "Left newer + orphans",
        RightNewerOrphans => "Right newer + orphans",
        LeftOrphans => "Left orphans",
        RightOrphans => "Right orphans",
        None => "Show none",
    }
}
pub fn folder_label(value: FolderDisplay) -> &'static str {
    match value {
        FolderDisplay::AlwaysShow => "Always show folders",
        FolderDisplay::FilesAndStructure => "Files and folder structure",
        FolderDisplay::FilesOnly => "Files only",
        FolderDisplay::Flatten => "Ignore folder structure",
    }
}
