//! 可编辑路径组合框。调用者用 `push_id` 区分左右路径，负责历史持久化及文件操作。
//! 外观对齐经典浅色参考：25 px 白底编辑框（#8D8D8D 边）、右端内嵌历史下拉小箭头、
//! 右侧三个无边框 16 px 图标按钮（历史/浏览/保存），浏览按钮带静态 ▼ 字形。
use crate::chrome::icons::{self, Icon};
use eframe::egui::{self, Stroke};
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PathbarResponse {
    pub action: Option<String>,
    pub submitted: bool,
}
/// 无历史的简便入口；`show_with_history` 返回 Enter 提交和历史选择。
pub fn show(ui: &mut egui::Ui, path: &mut String) -> Option<String> {
    show_with_history(ui, path, &[], false).action
}
pub fn show_with_history(
    ui: &mut egui::Ui,
    path: &mut String,
    history: &[String],
    select_all: bool,
) -> PathbarResponse {
    let mut result = PathbarResponse::default();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.visuals_mut().selection.bg_fill = egui::Color32::from_rgb(194, 220, 245);
        ui.visuals_mut().widgets.inactive.bg_stroke =
            Stroke::new(1.0, egui::Color32::from_rgb(141, 141, 141));
        let edit_width = (ui.available_width() - 82.0).max(40.0);
        let edit = ui.add_sized(
            [edit_width, 25.0],
            egui::TextEdit::singleline(path).margin(egui::vec2(2.0, 1.0)),
        );
        if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            result.submitted = true;
            result.action = Some("path-submit".into());
        }
        // 快照场景复刻参考「打开后全选路径」状态。
        if select_all {
            let mut state = egui::widgets::text_edit::TextEditState::load(ui.ctx(), edit.id)
                .unwrap_or_default();
            let len = path.chars().count();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(len),
                )));
            state.store(ui.ctx(), edit.id);
            ui.memory_mut(|m| m.request_focus(edit.id));
        }
        // 编辑框右端内嵌历史下拉箭头。
        let chevron = egui::Rect::from_min_size(
            egui::pos2(edit.rect.right() - 16.0, edit.rect.top()),
            egui::vec2(15.0, edit.rect.height()),
        );
        let chev = ui.interact(
            chevron,
            ui.scope_id().with("path-history-chevron"),
            egui::Sense::click(),
        );
        let c = chevron.center();
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                c + egui::vec2(-4.0, -1.5),
                c + egui::vec2(4.0, -1.5),
                c + egui::vec2(0.0, 2.5),
            ],
            egui::Color32::from_rgb(110, 110, 110),
            Stroke::NONE,
        ));
        egui::Popup::from_toggle_button_response(&chev)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                if history.is_empty() {
                    ui.add_enabled(false, egui::Label::new("No recent paths"));
                }
                for entry in history {
                    if ui.selectable_label(*path == *entry, entry).clicked() {
                        *path = entry.clone();
                        result.submitted = true;
                        result.action = Some("history-select".into());
                        ui.close();
                    }
                }
            });
        chev.on_hover_text("Path history");
        // 右侧无边框图标按钮：历史 / 浏览(带 ▼) / 保存。
        ui.add_space(9.0);
        for (i, (icon, id, tip, extra)) in [
            (Icon::History, "history", "Path history", 17.0),
            (Icon::Browse, "browse", "Browse folder", 28.0),
            (Icon::Save, "save", "Save path", 17.0),
        ]
        .into_iter()
        .enumerate()
        {
            if i == 2 {
                ui.add_space(8.0);
            }
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(extra, 25.0), egui::Sense::click());
            if response.clicked() {
                result.action = Some(id.into());
            }
            if response.hovered() {
                ui.painter()
                    .rect_filled(rect, 0.0, super::theme::HOVER_BLUE);
                ui.painter().rect_stroke(
                    rect,
                    0.0,
                    Stroke::new(1.0, super::theme::CLASSIC_BLUE),
                    egui::StrokeKind::Inside,
                );
            }
            icons::paint(
                ui.painter(),
                egui::Rect::from_center_size(
                    rect.left_center() + egui::vec2(8.0, 0.0),
                    egui::vec2(16.0, 16.0),
                ),
                icon,
            );
            if id == "browse" {
                let c = rect.left_center() + egui::vec2(23.0, 0.0);
                ui.painter().add(egui::Shape::convex_polygon(
                    vec![
                        c + egui::vec2(-2.5, -1.0),
                        c + egui::vec2(2.5, -1.0),
                        c + egui::vec2(0.0, 2.0),
                    ],
                    egui::Color32::BLACK,
                    Stroke::NONE,
                ));
            }
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
            let out = show_with_history(ui, &mut path, &["old.txt".into()], false);
            assert_eq!(out, PathbarResponse::default());
            assert!(ui.min_rect().right() <= 501.0);
        });
        assert_eq!(path, "left.txt");
    }
}
