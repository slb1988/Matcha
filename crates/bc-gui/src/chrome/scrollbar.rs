//! 经典竖向滚动条近似：上下按钮、比例滑块、轨道定位与拖动。
//! egui 不提供 Win32 的原生自动重复时序；长按箭头重复是已知限制。
use eframe::egui::{self, Color32, Rect, Sense, Stroke};
/// `range` 是可滚动首行范围。返回合并的响应，数值变化时标记 changed。
pub fn show(
    ui: &mut egui::Ui,
    range: std::ops::RangeInclusive<usize>,
    value: &mut usize,
) -> egui::Response {
    let min = *range.start();
    let max = (*range.end()).max(min);
    let before = *value;
    *value = (*value).clamp(min, max);
    let (rect, mut response) = ui.allocate_exact_size(
        egui::vec2(16.0, ui.available_height().max(64.0)),
        Sense::hover(),
    );
    let top = Rect::from_min_size(rect.min, egui::vec2(16.0, 16.0));
    let bottom = Rect::from_min_size(
        rect.left_bottom() - egui::vec2(0.0, 16.0),
        egui::vec2(16.0, 16.0),
    );
    let track = Rect::from_min_max(top.left_bottom(), bottom.right_top());
    ui.painter().rect_filled(rect, 0.0, super::theme::PANEL_BG);
    for (button, up) in [(top, true), (bottom, false)] {
        let r = ui.interact(button, response.id.with(up), Sense::click());
        if r.clicked() {
            *value = if up {
                value.saturating_sub(1).max(min)
            } else {
                value.saturating_add(1).min(max)
            };
        }
        if r.hovered() {
            ui.painter().rect_filled(
                button.shrink(1.0),
                super::theme::RADIUS_SM,
                super::theme::HOVER,
            );
        }
        let c = button.center();
        let y = if up { -1.0 } else { 1.0 };
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                c + egui::vec2(0.0, -3.0 * y),
                c + egui::vec2(3.0, 2.0 * y),
                c + egui::vec2(-3.0, 2.0 * y),
            ],
            super::theme::TEXT_FAINT,
            Stroke::NONE,
        ));
        response = response.union(r);
    }
    let thumb_height =
        (track.height() / ((max - min + 1) as f32).sqrt()).clamp(16.0, track.height());
    let travel = track.height() - thumb_height;
    let y = track.top() + travel * ((*value - min) as f32 / (max - min).max(1) as f32);
    let thumb = Rect::from_min_size(egui::pos2(track.left(), y), egui::vec2(16.0, thumb_height));
    let r = ui.interact(track, response.id.with("track"), Sense::click_and_drag());
    if (r.clicked() || r.dragged()) && travel > 0.0 {
        if let Some(pos) = r.interact_pointer_pos() {
            let ratio = ((pos.y - track.top() - thumb_height / 2.0) / travel).clamp(0.0, 1.0);
            *value = min + (ratio * (max - min) as f32).round() as usize;
        }
    }
    // Rounded pill thumb inset from the track.
    ui.painter().rect_filled(
        thumb.shrink2(egui::vec2(4.0, 1.0)),
        4.0,
        if r.hovered() || r.dragged() {
            Color32::from_rgb(0xA9, 0xB0, 0xB8)
        } else {
            Color32::from_rgb(0xC6, 0xCB, 0xD1)
        },
    );
    response = response.union(r);
    if *value != before {
        response.mark_changed();
    }
    response
}
/// 经典横向滚动条近似：左右按钮、比例滑块、轨道定位与拖动（`show` 的横向版）。
/// `range` 是可滚动首列范围。返回合并的响应，数值变化时标记 changed。
pub fn show_horizontal(
    ui: &mut egui::Ui,
    range: std::ops::RangeInclusive<usize>,
    value: &mut usize,
) -> egui::Response {
    let min = *range.start();
    let max = (*range.end()).max(min);
    let before = *value;
    *value = (*value).clamp(min, max);
    let (rect, mut response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(64.0), 16.0),
        Sense::hover(),
    );
    let left = Rect::from_min_size(rect.min, egui::vec2(16.0, 16.0));
    let right = Rect::from_min_size(
        rect.right_top() - egui::vec2(16.0, 0.0),
        egui::vec2(16.0, 16.0),
    );
    let track = Rect::from_min_max(left.right_top(), right.left_bottom());
    ui.painter().rect_filled(rect, 0.0, super::theme::PANEL_BG);
    for (button, is_left) in [(left, true), (right, false)] {
        let r = ui.interact(button, response.id.with(is_left), Sense::click());
        if r.clicked() {
            *value = if is_left {
                value.saturating_sub(1).max(min)
            } else {
                value.saturating_add(1).min(max)
            };
        }
        if r.hovered() {
            ui.painter().rect_filled(
                button.shrink(1.0),
                super::theme::RADIUS_SM,
                super::theme::HOVER,
            );
        }
        let c = button.center();
        let x = if is_left { -1.0 } else { 1.0 };
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                c + egui::vec2(-3.0 * x, 0.0),
                c + egui::vec2(2.0 * x, -3.0),
                c + egui::vec2(2.0 * x, 3.0),
            ],
            super::theme::TEXT_FAINT,
            Stroke::NONE,
        ));
        response = response.union(r);
    }
    let thumb_width =
        (track.width() / ((max - min + 1) as f32).sqrt()).clamp(16.0, track.width());
    let travel = track.width() - thumb_width;
    let x = track.left() + travel * ((*value - min) as f32 / (max - min).max(1) as f32);
    let thumb = Rect::from_min_size(egui::pos2(x, track.top()), egui::vec2(thumb_width, 16.0));
    let r = ui.interact(track, response.id.with("track"), Sense::click_and_drag());
    if (r.clicked() || r.dragged()) && travel > 0.0 {
        if let Some(pos) = r.interact_pointer_pos() {
            let ratio = ((pos.x - track.left() - thumb_width / 2.0) / travel).clamp(0.0, 1.0);
            *value = min + (ratio * (max - min) as f32).round() as usize;
        }
    }
    ui.painter().rect_filled(
        thumb.shrink2(egui::vec2(1.0, 4.0)),
        4.0,
        if r.hovered() || r.dragged() {
            Color32::from_rgb(0xA9, 0xB0, 0xB8)
        } else {
            Color32::from_rgb(0xC6, 0xCB, 0xD1)
        },
    );
    response = response.union(r);
    if *value != before {
        response.mark_changed();
    }
    response
}
#[cfg(test)]
mod tests {
    #[test]
    fn empty_and_out_of_range_scroll_positions_are_clamped() {
        let ctx = eframe::egui::Context::default();
        let mut value = 999;
        let _ = crate::chrome::test_frame(&ctx, Default::default(), |ui| {
            super::show(ui, 0..=0, &mut value);
        });
        assert_eq!(value, 0);
    }
}
