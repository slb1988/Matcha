pub mod find;
pub mod operation;
pub mod options;
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
    use crate::chrome::{
        controls::Role,
        modal::{Modal, Tone},
    };
    let answers = [
        Confirmation::NoToAll,
        Confirmation::YesToAll,
        Confirmation::No,
        Confirmation::Yes,
    ];
    let answer = Modal::new("file-op-confirm", Tone::Question, "Confirm file operation")
        .message(format!(
            "{}

Permanent deletes always ask for their own confirmation.",
            question.text
        ))
        .action("No to All", Role::Secondary)
        .action("Yes to All", Role::Secondary)
        .action("No", Role::Secondary)
        .action("Yes", Role::Primary)
        .cancel(2)
        .width(520.0)
        .show(ctx)
        .map(|i| answers[i]);
    answer
}
