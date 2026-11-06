//! Sync preview reuses the folder worker, dialogs, progress and immutable row renderer.
use crate::{
    dir_view::{choice_label, Command, DirCompareView},
    egui,
};
use bc_engine_dir::sync::{CustomSyncRules, SyncChoice, SyncDirection, SyncMode};
use std::path::Path;
pub struct SyncView {
    pub folder: DirCompareView,
    method: usize,
    create_empty: bool,
    visible: bool,
    custom: [SyncChoice; 5],
}
impl SyncView {
    pub fn open(ctx: &egui::Context, left: &Path, right: &Path) -> Self {
        Self {
            folder: DirCompareView::open(ctx, left, right, true, false),
            method: 1,
            create_empty: false,
            visible: false,
            custom: [SyncChoice::LeaveAlone; 5],
        }
    }
    fn mode(&self) -> SyncMode {
        match self.method {
            0 => SyncMode::Update(SyncDirection::RightToLeft),
            1 => SyncMode::Update(SyncDirection::LeftToRight),
            2 => SyncMode::Update(SyncDirection::Both),
            3 => SyncMode::Mirror(SyncDirection::RightToLeft),
            4 => SyncMode::Mirror(SyncDirection::LeftToRight),
            _ => SyncMode::Custom(CustomSyncRules {
                left_newer: self.custom[0],
                right_newer: self.custom[1],
                left_orphan: self.custom[2],
                right_orphan: self.custom[3],
                different: self.custom[4],
            }),
        }
    }
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<crate::dir_view::FolderNavigation> {
        self.folder.poll(ui.ctx());
        let before = (self.method, self.create_empty, self.visible, self.custom);
        ui.add_enabled_ui(!self.folder.busy(), |ui| {
            ui.horizontal(|ui| {
                ui.heading("Folder Sync");
                egui::ComboBox::from_id_salt("sync_method")
                    .selected_text(
                        [
                            "Update left",
                            "Update right",
                            "Update both",
                            "Mirror to left",
                            "Mirror to right",
                            "Custom",
                        ][self.method],
                    )
                    .show_ui(ui, |ui| {
                        for (index, label) in [
                            "Update left",
                            "Update right",
                            "Update both",
                            "Mirror to left",
                            "Mirror to right",
                            "Custom",
                        ]
                        .iter()
                        .enumerate()
                        {
                            ui.selectable_value(&mut self.method, index, *label);
                        }
                    });
                ui.checkbox(&mut self.create_empty, "Create empty folders");
                ui.checkbox(&mut self.visible, "Visible only");
            });
            if self.method == 5 {
                ui.horizontal_wrapped(|ui| {
                    for (index, label) in [
                        "Left newer",
                        "Right newer",
                        "Left orphan",
                        "Right orphan",
                        "Different",
                    ]
                    .iter()
                    .enumerate()
                    {
                        egui::ComboBox::from_id_salt(("sync_custom", index))
                            .selected_text(format!("{label}: {}", choice_label(self.custom[index])))
                            .show_ui(ui, |ui| {
                                for choice in choices() {
                                    ui.selectable_value(
                                        &mut self.custom[index],
                                        choice,
                                        choice_label(choice),
                                    );
                                }
                            });
                    }
                });
            }
        });
        if before != (self.method, self.create_empty, self.visible, self.custom)
            && !self.folder.busy()
        {
            self.folder.send(Command::SyncMode(
                self.mode(),
                self.create_empty,
                self.visible,
            ));
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.folder.busy()
                        && self.folder.snapshot.complete
                        && self.folder.snapshot.sync_stats.is_some(),
                    egui::Button::new("Sync Now"),
                )
                .clicked()
            {
                self.folder.send(Command::SyncRun);
            }
            if ui.button("Previous conflict").clicked() {
                self.folder.navigate(true, true);
            }
            if ui.button("Next conflict").clicked() {
                self.folder.navigate(false, true);
            }
            ui.add_enabled_ui(!self.folder.busy(), |ui| {
                ui.menu_button("Override selected item", |ui| {
                    for choice in choices() {
                        if ui.button(choice_label(choice)).clicked() {
                            if let Some(&(node, _)) = self.folder.selection.items.iter().next() {
                                self.folder.send(Command::SyncOverride(node, choice));
                            }
                            ui.close();
                        }
                    }
                });
            });
            if let Some(stats) = self.folder.snapshot.sync_stats {
                ui.label(format!(
                    "Preview: {} copy, {} delete, {} create, {} leave, {} conflicts",
                    stats.copies, stats.deletes, stats.creates, stats.skipped, stats.conflicts
                ));
            }
        });
        self.folder.ui(ui)
    }
}
fn choices() -> [SyncChoice; 6] {
    [
        SyncChoice::LeaveAlone,
        SyncChoice::CopyRightToLeft,
        SyncChoice::CopyLeftToRight,
        SyncChoice::DeleteLeft,
        SyncChoice::DeleteRight,
        SyncChoice::Exclude,
    ]
}
