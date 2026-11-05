//! Dialog fields compile to the same DirOp models used by scripts.
use crate::egui;
use bc_core::Side;
use bc_engine_dir::{
    filter::FilterClock,
    filter_script::parse_date,
    ops::{DirOp, PathMode, RenameSpec, Target, TouchSpec},
};
use bc_vfs::FileAttrs;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    CopyTo,
    MoveTo,
    Rename,
    Attributes,
    Touch,
    NewFolder,
    Delete,
}
pub struct OperationDialog {
    pub kind: Kind,
    pub text: String,
    pub old: String,
    pub regex: bool,
    pub target: Target,
    pub path_mode: PathMode,
    pub include_hidden: bool,
    pub recycle: bool,
    pub touch_mode: usize,
    pub error: Option<String>,
    pub recent: Vec<String>,
}
impl OperationDialog {
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            text: String::new(),
            old: String::new(),
            regex: false,
            target: Target::All,
            path_mode: PathMode::None,
            include_hidden: false,
            recycle: true,
            touch_mode: 0,
            error: None,
            recent: Vec::new(),
        }
    }
    pub fn operation(&self, clock: FilterClock) -> Result<DirOp, String> {
        if matches!(self.kind, Kind::CopyTo | Kind::MoveTo) {
            bc_vfs::VfsPath::parse(self.text.trim()).map_err(|e| e.to_string())?;
        }
        if self.kind == Kind::NewFolder {
            bc_engine_dir::ops::valid_name(&self.text).map_err(|e| e.to_string())?;
        }
        Ok(match self.kind {
            Kind::CopyTo => DirOp::CopyTo {
                target: self.target,
                path_mode: self.path_mode,
                dest: self.text.clone(),
            },
            Kind::MoveTo => DirOp::MoveTo {
                target: self.target,
                path_mode: self.path_mode,
                dest: self.text.clone(),
            },
            Kind::Rename => DirOp::Rename(if self.regex {
                RenameSpec::Regex {
                    old_mask: self.old.clone(),
                    new_mask: self.text.clone(),
                }
            } else {
                RenameSpec::Dos {
                    new_mask: self.text.clone(),
                }
            }),
            Kind::Attributes => DirOp::Attrib {
                set: FileAttrs::parse_set(&self.text).ok_or("invalid set attributes")?,
                clear: FileAttrs::parse_set(&self.old).ok_or("invalid clear attributes")?,
            },
            Kind::Touch => DirOp::Touch(match self.touch_mode {
                1 => TouchSpec::Copy {
                    from: Side::Left,
                    to: Side::Right,
                },
                2 => TouchSpec::Copy {
                    from: Side::Right,
                    to: Side::Left,
                },
                _ => TouchSpec::Set {
                    target: self.target,
                    when: if self.text.trim().is_empty() || self.text == "now" {
                        None
                    } else {
                        Some(parse_date(&self.text, clock.utc_offset_seconds)?)
                    },
                },
            }),
            Kind::NewFolder => DirOp::NewFolder {
                target: self.target,
                name: self.text.clone(),
            },
            Kind::Delete => DirOp::Delete {
                target: self.target,
                recycle_bin: self.recycle,
            },
        })
    }
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        clock: FilterClock,
    ) -> Option<Result<(DirOp, bool), ()>> {
        let mut result = None;
        let mut open = true;
        egui::Window::new(format!("{:?}", self.kind))
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                if self.kind != Kind::Rename {
                    egui::ComboBox::from_id_salt("operation_target")
                        .selected_text(format!("{:?}", self.target))
                        .show_ui(ui, |ui| {
                            for value in [Target::Left, Target::Right, Target::All] {
                                ui.selectable_value(&mut self.target, value, format!("{value:?}"));
                            }
                        });
                }
                match self.kind {
                    Kind::CopyTo | Kind::MoveTo => {
                        ui.label("Destination folder");
                        ui.text_edit_singleline(&mut self.text);
                        egui::ComboBox::from_id_salt("operation_recent")
                            .selected_text("Recent folders")
                            .show_ui(ui, |ui| {
                                for path in &self.recent {
                                    if ui.button(path).clicked() {
                                        self.text = path.clone();
                                        ui.close();
                                    }
                                }
                            });
                        egui::ComboBox::from_id_salt("operation_path_mode")
                            .selected_text(format!("{:?}", self.path_mode))
                            .show_ui(ui, |ui| {
                                for mode in [PathMode::None, PathMode::Relative, PathMode::Base] {
                                    ui.selectable_value(
                                        &mut self.path_mode,
                                        mode,
                                        format!("{mode:?}"),
                                    );
                                }
                            });
                    }
                    Kind::Rename => {
                        ui.checkbox(&mut self.regex, "Regular expression");
                        if self.regex {
                            ui.label("Find pattern");
                            ui.text_edit_singleline(&mut self.old);
                        }
                        ui.label("New mask / replacement");
                        ui.text_edit_singleline(&mut self.text);
                    }
                    Kind::Attributes => {
                        ui.label("Set attributes (rhas)");
                        ui.text_edit_singleline(&mut self.text);
                        ui.label("Clear attributes (rhas)");
                        ui.text_edit_singleline(&mut self.old);
                    }
                    Kind::Touch => {
                        egui::ComboBox::from_id_salt("touch_kind")
                            .selected_text(
                                ["Set now / timestamp", "Left -> right", "Right -> left"]
                                    [self.touch_mode],
                            )
                            .show_ui(ui, |ui| {
                                for (i, label) in
                                    ["Set now / timestamp", "Left -> right", "Right -> left"]
                                        .iter()
                                        .enumerate()
                                {
                                    ui.selectable_value(&mut self.touch_mode, i, *label);
                                }
                            });
                        if self.touch_mode == 0 {
                            ui.text_edit_singleline(&mut self.text);
                            ui.label("Empty = now; explicit dates use the local UTC offset.");
                        }
                    }
                    Kind::NewFolder => {
                        ui.label("Folder name (selected folder, or bases if none selected)");
                        ui.text_edit_singleline(&mut self.text);
                    }
                    Kind::Delete => {
                        ui.checkbox(&mut self.recycle, "Use recycle bin");
                    }
                }
                ui.checkbox(
                    &mut self.include_hidden,
                    "Include filtered / hidden items for this operation",
                );
                if let Some(error) = &self.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
                ui.horizontal(|ui| {
                    if ui.button("Plan and run").clicked() {
                        match self.operation(clock) {
                            Ok(op) => result = Some(Ok((op, self.include_hidden))),
                            Err(e) => self.error = Some(e),
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        result = Some(Err(()));
                    }
                });
            });
        if !open {
            result = Some(Err(()));
        }
        result
    }
}
