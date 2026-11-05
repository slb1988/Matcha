pub mod find;
pub mod operation;
pub mod paths;
pub mod scope;
use crate::egui;
use bc_jobs::Confirmation;
use std::sync::mpsc::SyncSender;
pub struct Question {
    pub text: String,
    pub cancel: bc_jobs::CancelToken,
    pub reply: SyncSender<Confirmation>,
}
pub fn confirmation(ctx: &egui::Context, question: &Question) -> Option<Confirmation> {
    if question.cancel.is_cancelled() {
        return Some(Confirmation::No);
    }
    let mut answer = None;
    egui::Window::new("Confirm file operation")
        .collapsible(false)
        .resizable(true)
        .show(ctx, |ui| {
            ui.label(&question.text);
            ui.separator();
            ui.horizontal(|ui| {
                for (label, value) in [
                    ("Yes", Confirmation::Yes),
                    ("No", Confirmation::No),
                    ("Yes to all", Confirmation::YesToAll),
                    ("No to all", Confirmation::NoToAll),
                ] {
                    if ui.button(label).clicked() {
                        answer = Some(value);
                    }
                }
            });
            ui.small("Permanent-delete fallback always requires its own confirmation.");
        });
    answer
}
