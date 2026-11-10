//! 可编辑路径组合框。调用者用 `push_id` 区分左右路径，负责历史持久化及文件操作。
use crate::chrome::icons::{self, Icon};
use eframe::egui::{self, Stroke, StrokeKind};
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PathbarResponse {
    pub action: Option<String>,
    pub submitted: bool,
}
/// 无历史的简便入口；`show_with_history` 返回 Enter 提交和历史选择。
pub fn show(ui: &mut egui::Ui, path: &mut String) -> Option<String> {
    show_with_history(ui, path, &[]).action
}
pub fn show_with_history(
    ui: &mut egui::Ui,
    path: &mut String,
    history: &[String],
) -> PathbarResponse {
    let mut result = PathbarResponse::default();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.visuals_mut().selection.bg_fill = egui::Color32::from_rgb(194, 220, 245);
        let edit_width = (ui.available_width() - 100.0).max(40.0);
        let edit = ui.add_sized([edit_width, 22.0], egui::TextEdit::singleline(path));
        if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            result.submitted = true;
            result.action = Some("path-submit".into());
        }
        egui::ComboBox::from_id_salt("path-history")
            .width(20.0)
            .selected_text("")
            .show_ui(ui, |ui| {
                if history.is_empty() {
                    ui.add_enabled(false, egui::Label::new("No recent paths"));
                }
                for entry in history {
                    if ui.selectable_label(*path == *entry, entry).clicked() {
                        *path = entry.clone();
                        result.submitted = true;
                        result.action = Some("history-select".into());
                    }
                }
            });
        for (icon, id, tip) in [
            (Icon::History, "history", "Path history"),
            (Icon::Browse, "browse", "Browse folder"),
            (Icon::Save, "save", "Save path"),
        ] {
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(18.0, 22.0), egui::Sense::click());
            if response.clicked() {
                result.action = Some(id.into());
            }
            if response.hovered() {
                ui.painter()
                    .rect_filled(rect, 0.0, super::theme::HOVER_BLUE);
            }
            ui.painter().rect_stroke(
                rect,
                0.0,
                Stroke::new(1.0, super::theme::BORDER),
                StrokeKind::Inside,
            );
            icons::paint(ui.painter(), rect.shrink(3.0), icon);
            response.on_hover_text(tip);
        }
    });
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn path_and_history_render_without_overflow_or_unrequested_changes() {
        let ctx = egui::Context::default();
        let mut path = "left.txt".to_string();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(500.0, 100.0),
            )),
            ..Default::default()
        };
        let _ = crate::chrome::test_frame(&ctx, input, |ui| {
            let out = show_with_history(ui, &mut path, &["old.txt".into()]);
            assert_eq!(out, PathbarResponse::default());
            assert!(ui.min_rect().right() <= 501.0);
        });
        assert_eq!(path, "left.txt");
    }
}
