//! Per-operation scope review happens before a plan is built, not after I/O starts.
use crate::chrome::{controls::Role, widgets, window::Window};
use crate::egui;
use bc_core::Side;
use bc_engine_dir::ops::DirOp;
pub struct ScopeDialog {
    pub op: DirOp,
    pub include_hidden: bool,
    pub choose_side: bool,
    pub destination: usize,
}
/// Dialog title and one-line description of the operation being reviewed.
pub fn describe(op: &DirOp) -> (&'static str, &'static str) {
    match op {
        DirOp::CopyOther => (
            "Copy to Other Side",
            "Copy the selected items to the other side.",
        ),
        DirOp::MoveOther => (
            "Move to Other Side",
            "Move the selected items to the other side.",
        ),
        DirOp::Exchange => ("Exchange", "Swap the selected items between the two sides."),
        DirOp::Exclude => ("Exclude", "Hide the selected items from this comparison."),
        DirOp::Ignored(true) => ("Ignored", "Mark the selected items as ignored."),
        DirOp::Ignored(false) => ("Ignored", "Clear the ignored mark on the selected items."),
        _ => (
            "Review Operation",
            "Run the operation on the selected items.",
        ),
    }
}
impl ScopeDialog {
    pub fn new(op: DirOp, include_hidden: bool, choose_side: bool) -> Self {
        Self {
            op,
            include_hidden,
            choose_side,
            destination: 1,
        }
    }
    pub fn show(&mut self, ctx: &egui::Context) -> Option<Result<(DirOp, bool), ()>> {
        let mut result = None;
        let mut open = true;
        let (title, text) = describe(&self.op);
        Window::new(title).open(&mut open).show(ctx, |ui| {
            ui.label(text);
            widgets::checkbox(
                ui,
                &mut self.include_hidden,
                "Include hidden / filtered contents",
            );
            if self.choose_side {
                ui.horizontal(|ui| {
                    ui.label("Destination");
                    widgets::radio_value(ui, &mut self.destination, 0, "Left");
                    widgets::radio_value(ui, &mut self.destination, 1, "Right");
                });
            }
            widgets::hint(
                ui,
                "Overwrite, newer-target, read-only and deletion confirmations follow separately.",
            );
            match widgets::dialog_buttons(
                ui,
                &[("Cancel", Role::Secondary), ("Continue", Role::Primary)],
            ) {
                Some(1) => {
                    let (from, to) = if self.destination == 0 {
                        (Side::Right, Side::Left)
                    } else {
                        (Side::Left, Side::Right)
                    };
                    let op = if self.choose_side {
                        match self.op {
                            DirOp::CopyOther => DirOp::Copy { from, to },
                            DirOp::MoveOther => DirOp::Move { from, to },
                            _ => self.op.clone(),
                        }
                    } else {
                        self.op.clone()
                    };
                    result = Some(Ok((op, self.include_hidden)));
                }
                Some(0) => result = Some(Err(())),
                _ => {}
            }
        });
        if !open {
            result = Some(Err(()));
        }
        result
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::operation::{path_mode_label, target_label, Kind};
    use bc_engine_dir::ops::{PathMode, Target};
    #[test]
    fn operation_dialogs_show_words_not_enum_names() {
        assert_eq!(describe(&DirOp::CopyOther).0, "Copy to Other Side");
        assert_eq!(
            describe(&DirOp::Exclude).1,
            "Hide the selected items from this comparison."
        );
        assert_eq!(Kind::NewFolder.title(), "New Folder");
        assert_eq!(Kind::CopyTo.title(), "Copy to Folder");
        assert_eq!(target_label(Target::All), "Both sides");
        assert_eq!(
            path_mode_label(PathMode::Relative),
            "Keep relative folder structure"
        );
    }
}
