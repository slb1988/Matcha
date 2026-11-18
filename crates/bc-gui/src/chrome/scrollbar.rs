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
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width().max(64.0), 16.0),
        Sense::hover(),
    );
    let thumb = |track: f32| (track / ((max - min + 1) as f32).sqrt()).clamp(16.0, track);
    let position = (*value - min) as f32 / (max - min).max(1) as f32;
    let (step, ratio, mut response) =
        horizontal_frame(ui, rect, response.id, response, thumb, position);
    if step < 0 {
        *value = value.saturating_sub(1).max(min);
    } else if step > 0 {
        *value = value.saturating_add(1).min(max);
    }
    if let Some(ratio) = ratio {
        *value = min + (ratio * (max - min) as f32).round() as usize;
    }
    if *value != before {
        response.mark_changed();
    }
    response
}
/// 像素版横向滚动条：`content` 为内容总宽、`view` 为可见宽，`offset` 为像素偏移
/// （夹在 `0..=content-view`）。绘制在给定 16 px 高的 `rect` 中；按钮步进 40 px；
/// 内容不超宽时滑块填满轨道。返回合并的响应，偏移变化时标记 changed。
pub fn show_horizontal_px(
    ui: &mut egui::Ui,
    rect: Rect,
    id: egui::Id,
    content: f32,
    view: f32,
    offset: &mut f32,
) -> egui::Response {
    const STEP: f32 = 40.0;
    let max = (content - view).max(0.0);
    let before = *offset;
    *offset = offset.clamp(0.0, max);
    let response = ui.interact(rect, id, Sense::hover());
    let thumb = |track: f32| {
        if content <= view || content <= 0.0 {
            track
        } else {
            (track * view / content).clamp(16.0_f32.min(track), track)
        }
    };
    let position = if max > 0.0 { *offset / max } else { 0.0 };
    let (step, ratio, mut response) = horizontal_frame(ui, rect, id, response, thumb, position);
    if step != 0 {
        *offset = (*offset + step as f32 * STEP).clamp(0.0, max);
    }
    if let Some(ratio) = ratio {
        *offset = ratio * max;
    }
    if *offset != before {
        response.mark_changed();
    }
    response
}
/// 横向滚动条共享绘制与命中：左右按钮（返回 -1/+1 步进）、轨道点击/拖动
/// （返回滑块中心对应的 0..=1 比例）。`thumb` 由轨道宽算滑块宽，`position`
/// 为当前 0..=1 位置。
fn horizontal_frame(
    ui: &mut egui::Ui,
    rect: Rect,
    id: egui::Id,
    mut response: egui::Response,
    thumb: impl Fn(f32) -> f32,
    position: f32,
) -> (i8, Option<f32>, egui::Response) {
    let left = Rect::from_min_size(rect.min, egui::vec2(16.0, 16.0));
    let right = Rect::from_min_size(
        rect.right_top() - egui::vec2(16.0, 0.0),
        egui::vec2(16.0, 16.0),
    );
    let track = Rect::from_min_max(left.right_top(), right.left_bottom());
    ui.painter().rect_filled(rect, 0.0, super::theme::PANEL_BG);
    let mut step = 0;
    for (button, is_left) in [(left, true), (right, false)] {
        let r = ui.interact(button, id.with(is_left), Sense::click());
        if r.clicked() {
            step = if is_left { -1 } else { 1 };
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
                c + egui::vec2(3.0 * x, 0.0),
                c + egui::vec2(-2.0 * x, -3.0),
                c + egui::vec2(-2.0 * x, 3.0),
            ],
            super::theme::TEXT_FAINT,
            Stroke::NONE,
        ));
        response = response.union(r);
    }
    let thumb_width = thumb(track.width().max(0.0));
    let travel = track.width() - thumb_width;
    let x = track.left() + travel * position.clamp(0.0, 1.0);
    let thumb = Rect::from_min_size(egui::pos2(x, track.top()), egui::vec2(thumb_width, 16.0));
    let r = ui.interact(track, id.with("track"), Sense::click_and_drag());
    let mut ratio = None;
    if (r.clicked() || r.dragged()) && travel > 0.0 {
        if let Some(pos) = r.interact_pointer_pos() {
            ratio = Some(((pos.x - track.left() - thumb_width / 2.0) / travel).clamp(0.0, 1.0));
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
    (step, ratio, response.union(r))
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
    #[test]
    fn pixel_scrollbar_clamps_steps_and_tracks_to_the_end() {
        use eframe::egui;
        let ctx = egui::Context::default();
        let rect = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(216.0, 16.0));
        let run = |offset: &mut f32, events: Vec<egui::Event>| {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400.0, 100.0),
                )),
                events,
                ..Default::default()
            };
            let _ = crate::chrome::test_frame(&ctx, input, |ui| {
                super::show_horizontal_px(ui, rect, egui::Id::unique("px"), 1000.0, 200.0, offset);
            });
        };
        let press = |p: egui::Pos2, pressed: bool| {
            vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ]
        };
        // Out-of-range offsets clamp to content - view.
        let mut offset = 5000.0;
        run(&mut offset, vec![]);
        assert_eq!(offset, 800.0);
        // Left button steps 40 px.
        let left = rect.min + egui::vec2(8.0, 8.0);
        run(&mut offset, press(left, true));
        run(&mut offset, press(left, false));
        assert_eq!(offset, 760.0);
        // Track click at the far right end jumps to the end.
        offset = 0.0;
        let end = egui::pos2(rect.right() - 17.0, rect.center().y);
        run(&mut offset, vec![]);
        run(&mut offset, press(end, true));
        run(&mut offset, press(end, false));
        assert_eq!(offset, 800.0);
        // Content that fits: offset stays 0.
        let mut fits = 30.0;
        let _ = crate::chrome::test_frame(&ctx, Default::default(), |ui| {
            super::show_horizontal_px(ui, rect, egui::Id::unique("fits"), 100.0, 200.0, &mut fits);
        });
        assert_eq!(fits, 0.0);
    }
}
