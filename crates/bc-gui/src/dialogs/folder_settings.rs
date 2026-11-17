//! Session Settings dialog for folder compare / sync (Session > Session Settings).
//!
//! Same frame as the text and table Session Settings: underline tabs over a page
//! card and an OK / Cancel / Apply row. The tabs follow the reference help
//! (Specs, Comparison, Handling, Name Filters, Other Filters, Misc). It edits a
//! draft of the view's settings; OK / Apply hand the validated draft to the view,
//! Cancel / × / Esc discard it. Saving a named filter preset and adding an
//! alignment override are actions in their own right and run immediately.
use crate::chrome::{
    dialog::{self, ButtonKind, Placement},
    widgets,
};
use crate::dir_view::ScanOptions;
use bc_engine_dir::{rules::HandlingOptions, tree::PairingOptions, Criteria};
use eframe::egui;

pub const CLIENT: egui::Vec2 = egui::vec2(583.0, 563.0);
pub const TABS: [&str; 6] = [
    "Specs",
    "Comparison",
    "Handling",
    "Name Filters",
    "Other Filters",
    "Misc",
];

/// Everything the dialog edits; produced from and applied to the view.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub paths: [String; 2],
    /// Name filter mask (also edited in the toolbar filter box).
    pub names: String,
    pub other: String,
    pub content: String,
    pub exclude_containing: bool,
    /// Comparison criteria script arguments.
    pub criteria: String,
    pub pairing: PairingOptions,
    pub scan: ScanOptions,
    pub handling: HandlingOptions,
}

/// What the view should do after a frame of the dialog.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// OK / Apply: apply the (validated) draft.
    Apply(Draft),
    /// Save the draft's name mask as a named preset.
    SavePreset { name: String, names: String },
    /// Add an alignment override now.
    AddAlignment {
        left: String,
        right: String,
        folder: Option<String>,
        regex: bool,
    },
}

#[derive(Default)]
pub struct FolderSettings {
    pub open: bool,
    pub draft: Option<Draft>,
    pub tab: usize,
    /// `None` = framed window; snapshots of the dialog alone use `ClientOnly`.
    pub placement: Option<Placement>,
    pub error: Option<String>,
    pub preset_name: String,
    pub align_left: String,
    pub align_right: String,
    pub align_folder: String,
    pub align_regex: bool,
}

impl FolderSettings {
    pub fn show(&mut self, draft: Draft, placement: Placement) {
        self.draft = Some(draft);
        self.placement = Some(placement);
        self.error = None;
        self.open = true;
    }
    pub fn close(&mut self) {
        self.open = false;
        self.draft = None;
        self.error = None;
    }
    fn validated(&self) -> Result<Draft, String> {
        let draft = self.draft.clone().ok_or("no settings")?;
        Criteria::parse_script(&draft.criteria).map_err(|e| format!("Comparison: {e}"))?;
        Ok(draft)
    }
    /// Draw the dialog. `title` names the session type ("Folder Compare" / "Folder Sync").
    pub fn ui(&mut self, ctx: &egui::Context, title: &str) -> Vec<Outcome> {
        let mut out = Vec::new();
        if !self.open || self.draft.is_none() {
            return out;
        }
        let placement = self.placement.unwrap_or(Placement::Window);
        let (mut ok, mut cancel, mut apply) = (false, false, false);
        let (closed, ()) = dialog::window(
            ctx,
            egui::Id::unique("folder-session-settings"),
            &format!("Session Settings - {title}"),
            CLIENT,
            placement,
            |ui, client| {
                let o = client.min;
                let at = |x0: f32, y0: f32, x1: f32, y1: f32| {
                    egui::Rect::from_min_max(o + egui::vec2(x0, y0), o + egui::vec2(x1, y1))
                };
                let page = at(8.0, 32.0, 575.0, 511.0);
                if let Some(tab) = dialog::tabs(ui, page, &TABS, self.tab) {
                    self.tab = tab;
                }
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(page.shrink2(egui::vec2(14.0, 12.0))),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("folder_settings_page")
                            .auto_shrink([false, false])
                            .show(ui, |ui| self.page(ui, &mut out));
                    },
                );
                if let Some(error) = &self.error {
                    ui.painter().text(
                        o + egui::vec2(12.0, 537.0),
                        egui::Align2::LEFT_CENTER,
                        error,
                        dialog::font(),
                        crate::chrome::theme::palette(ui.ctx()).danger,
                    );
                }
                ok = dialog::button(
                    ui,
                    at(268.0, 523.0, 362.0, 552.0),
                    "OK",
                    ButtonKind::Default,
                    true,
                );
                cancel = dialog::button(
                    ui,
                    at(373.0, 523.0, 467.0, 552.0),
                    "Cancel",
                    ButtonKind::Normal,
                    true,
                );
                apply = dialog::button(
                    ui,
                    at(478.0, 523.0, 572.0, 552.0),
                    "Apply",
                    ButtonKind::Normal,
                    true,
                );
            },
        );
        if closed
            || cancel
            || ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.close();
            return out;
        }
        if ok || apply {
            match self.validated() {
                Ok(draft) => {
                    out.push(Outcome::Apply(draft));
                    if ok {
                        self.close();
                    } else {
                        self.error = None;
                    }
                }
                Err(e) => self.error = Some(e),
            }
        }
        out
    }
    fn page(&mut self, ui: &mut egui::Ui, out: &mut Vec<Outcome>) {
        let Some(draft) = self.draft.as_mut() else {
            return;
        };
        let width = ui.available_width();
        match self.tab {
            0 => {
                for (s, label) in ["Left folder", "Right folder"].iter().enumerate() {
                    ui.label(*label);
                    widgets::text_field_width(ui, &mut draft.paths[s], width);
                    ui.add_space(6.0);
                }
                widgets::hint(ui, "Changing a folder reloads the comparison.");
            }
            1 => {
                widgets::section(ui, "Comparison criteria");
                widgets::text_field_width(ui, &mut draft.criteria, width);
                widgets::hint(
                    ui,
                    "Examples: timestamp:2sec;IgnoreDST size / CRC / binary / rules-based / \
                     skip-quick-same. Rules-based uses the default text rules (comment \
                     edits are unimportant); add comments-important to count them.",
                );
                ui.add_space(8.0);
                widgets::section(ui, "Alignment");
                widgets::checkbox(
                    ui,
                    &mut draft.pairing.case_sensitive,
                    "Case-sensitive pairing",
                );
                widgets::checkbox(
                    ui,
                    &mut draft.pairing.ignore_extension,
                    "Align different extensions",
                );
                widgets::checkbox(
                    ui,
                    &mut draft.pairing.normalize_nfc,
                    "Align Unicode normalization forms",
                );
            }
            2 => {
                widgets::section(ui, "Copy and move");
                let h = &mut draft.handling;
                widgets::checkbox(ui, &mut h.copy_timestamps, "Preserve timestamps");
                widgets::checkbox(ui, &mut h.copy_attributes, "Preserve attributes");
                widgets::checkbox(ui, &mut h.copy_creation_time, "Preserve creation time");
                widgets::checkbox(ui, &mut h.include_hidden, "Include hidden items by default");
                ui.add_enabled(
                    false,
                    widgets::Checkbox::new(&mut h.copy_ntfs_permissions, "NTFS permissions"),
                );
            }
            3 => {
                widgets::section(ui, "Name filters");
                widgets::text_field_width(ui, &mut draft.names, width);
                widgets::hint(
                    ui,
                    "Same mask as the toolbar filter box; *.* shows everything.",
                );
                ui.add_space(8.0);
                widgets::section(ui, "Named preset");
                ui.horizontal(|ui| {
                    widgets::text_field_width(ui, &mut self.preset_name, 260.0);
                    if widgets::button(ui, "Save named preset").clicked() {
                        out.push(Outcome::SavePreset {
                            name: self.preset_name.clone(),
                            names: draft.names.clone(),
                        });
                    }
                });
            }
            4 => {
                widgets::section(ui, "Script filters");
                widgets::text_field_width(ui, &mut draft.other, width);
                ui.add_space(8.0);
                widgets::section(ui, "Content");
                ui.label("UTF-8 content text (empty disables)");
                widgets::text_field_width(ui, &mut draft.content, width);
                widgets::checkbox(
                    ui,
                    &mut draft.exclude_containing,
                    "Exclude containing (otherwise exclude not containing)",
                );
            }
            _ => {
                widgets::section(ui, "Scanning");
                let scan = &mut draft.scan;
                widgets::checkbox(ui, &mut scan.automatic, "Automatic recursive scan");
                widgets::checkbox(ui, &mut scan.scan_orphans, "Scan top-level orphan folders");
                widgets::checkbox(ui, &mut scan.expand_on_load, "Expand folders on load");
                widgets::checkbox(ui, &mut scan.differences_only, "Expand differences only");
                widgets::checkbox(ui, &mut scan.auto_refresh, "Automatic filesystem refresh");
                ui.add_space(8.0);
                widgets::section(ui, "Alignment override");
                widgets::hint(
                    ui,
                    "Left pattern / right pattern / optional relative folder",
                );
                widgets::text_field_width(ui, &mut self.align_left, width);
                widgets::text_field_width(ui, &mut self.align_right, width);
                widgets::text_field_width(ui, &mut self.align_folder, width);
                widgets::checkbox(ui, &mut self.align_regex, "Regex captures");
                if widgets::button(ui, "Add alignment override").clicked() {
                    out.push(Outcome::AddAlignment {
                        left: self.align_left.clone(),
                        right: self.align_right.clone(),
                        folder: (!self.align_folder.is_empty()).then(|| self.align_folder.clone()),
                        regex: self.align_regex,
                    });
                }
            }
        }
    }
}
