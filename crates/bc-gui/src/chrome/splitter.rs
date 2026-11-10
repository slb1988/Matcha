//! Narrow draggable splitters used between comparison panes.

use eframe::egui::{self, CursorIcon, Sense, Vec2};

/// A vertical splitter; callers use the returned drag delta to resize panes.
pub fn vertical(
    ui: &mut egui::Ui,
    id_source: impl std::hash::Hash + std::fmt::Debug,
) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(5.0, ui.available_height()), Sense::hover());
    let response = ui.interact(rect, ui.scope_id().with(id_source), Sense::drag());
    if response.hovered() || response.dragged() {
        ui.output_mut(|output| output.cursor_icon = CursorIcon::ResizeHorizontal);
    }
    ui.painter().line_segment(
        [rect.center_top(), rect.center_bottom()],
        egui::Stroke::new(1.0, crate::chrome::theme::BORDER),
    );
    response
}

#[cfg(test)]
mod tests {
    #[test]
    fn injected_drag_reports_resize_delta() {
        use eframe::egui;
        let ctx = egui::Context::default();
        let _ = crate::chrome::test_frame(&ctx, Default::default(), |ui| {
            super::vertical(ui, "split");
        });
        let p = egui::pos2(2.0, 30.0);
        let input = egui::RawInput {
            events: vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ],
            ..Default::default()
        };
        let _ = crate::chrome::test_frame(&ctx, input, |ui| {
            super::vertical(ui, "split");
        });
        let input = egui::RawInput {
            events: vec![egui::Event::PointerMoved(p + egui::vec2(20.0, 0.0))],
            ..Default::default()
        };
        let _ = crate::chrome::test_frame(&ctx, input, |ui| {
            let r = super::vertical(ui, "split");
            assert!(r.dragged());
            assert_eq!(r.drag_delta().x, 20.0);
        });
    }
}
