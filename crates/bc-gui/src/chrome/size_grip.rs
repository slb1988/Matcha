//! Window size grip in the bottom-right corner.
//!
//! The native resize border is only a few pixels wide, so the status bar's
//! corner carries a larger, clearly textured grip: hovering shows the diagonal
//! resize cursor and highlights it, pressing hands the drag to the OS
//! (`ViewportCommand::BeginResize(SouthEast)`), which then sizes the window
//! exactly like grabbing the frame corner.

use eframe::egui::{self, Color32, CursorIcon, Pos2, Rect, Sense, Vec2};

/// Side of the square hit area. The status bar is 25-26 px tall, so the grip
/// stays inside it.
pub const HIT: f32 = 22.0;
/// Rows/columns of the dot triangle (4 → 10 dots).
const ROWS: usize = 4;
/// Dot pitch and size.
const STEP: f32 = 4.0;
const DOT: f32 = 2.0;
/// Inset of the bottom-right dot from the window corner.
const INSET: f32 = 3.0;

/// Grip rectangle for a window content rect.
pub fn grip_rect(content: Rect) -> Rect {
    Rect::from_min_max(content.max - Vec2::splat(HIT), content.max)
}

/// Top-left corners of the dots: a right triangle filling the bottom-right
/// corner, the same shape as the classic Windows size grip but one row larger.
pub fn dot_positions(grip: Rect) -> Vec<Pos2> {
    let origin = grip.max - Vec2::splat(INSET + DOT);
    let mut dots = Vec::new();
    for row in 0..ROWS {
        for col in 0..ROWS - row {
            dots.push(origin - egui::vec2(col as f32 * STEP, row as f32 * STEP));
        }
    }
    dots
}

/// Draw the grip over everything else and start an OS resize on press.
/// Hidden while the window is maximized or fullscreen (nothing to resize).
pub fn show(ctx: &egui::Context) {
    let fixed = ctx.input(|i| {
        let v = i.viewport();
        v.maximized == Some(true) || v.fullscreen == Some(true)
    });
    if fixed {
        return;
    }
    let grip = grip_rect(ctx.content_rect());
    egui::Area::new(egui::Id::unique("window_size_grip"))
        .order(egui::Order::Foreground)
        .fixed_pos(grip.min)
        .interactable(true)
        .show(ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(grip.size(), Sense::drag());
            let pressed = response.is_pointer_button_down_on();
            if response.hovered() && ui.input(|i| i.pointer.primary_pressed()) {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::BeginResize(
                        egui::ResizeDirection::SouthEast,
                    ));
            }
            let active = response.hovered() || pressed;
            if active {
                ui.ctx().set_cursor_icon(CursorIcon::ResizeNwSe);
            }
            paint(ui.painter(), rect, active, pressed);
        });
}

fn paint(painter: &egui::Painter, rect: Rect, hovered: bool, pressed: bool) {
    use super::theme;
    if hovered {
        let fill = if pressed {
            theme::PRESSED
        } else {
            theme::HOVER
        };
        painter.rect_filled(rect.shrink(1.0), theme::RADIUS_SM, fill);
    }
    let dot = if pressed {
        theme::ACCENT
    } else if hovered {
        theme::TEXT_MUTED
    } else {
        theme::TEXT_FAINT
    };
    for pos in dot_positions(rect) {
        // Embossed texture: a white highlight one pixel down-right of each dot.
        painter.rect_filled(
            Rect::from_min_size(pos + Vec2::splat(1.0), Vec2::splat(DOT)),
            0.0,
            Color32::WHITE,
        );
        painter.rect_filled(Rect::from_min_size(pos, Vec2::splat(DOT)), 0.0, dot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect {
        min: Pos2::ZERO,
        max: Pos2::new(800.0, 600.0),
    };

    fn frame(ctx: &egui::Context, events: Vec<egui::Event>, maximized: bool) -> egui::FullOutput {
        let mut input = egui::RawInput {
            screen_rect: Some(SCREEN),
            events,
            ..Default::default()
        };
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .maximized = Some(maximized);
        crate::chrome::test_frame(ctx, input, |ui| {
            show(ui.ctx());
        })
    }

    fn press(pos: Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ]
    }

    fn began_resize(output: &egui::FullOutput) -> bool {
        output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .iter()
            .any(|c| {
                matches!(
                    c,
                    egui::ViewportCommand::BeginResize(egui::ResizeDirection::SouthEast)
                )
            })
    }

    #[test]
    fn grip_is_a_corner_square_with_a_ten_dot_triangle() {
        let grip = grip_rect(SCREEN);
        assert_eq!(grip.max, SCREEN.max);
        assert_eq!(grip.size(), Vec2::splat(HIT));
        let dots = dot_positions(grip);
        assert_eq!(dots.len(), 10);
        assert!(dots
            .iter()
            .all(|d| grip.contains(*d + Vec2::splat(DOT + 1.0))));
    }

    #[test]
    fn hover_shows_resize_cursor_and_press_starts_os_resize() {
        let ctx = egui::Context::default();
        frame(&ctx, vec![], false);
        frame(&ctx, vec![], false);
        let corner = Pos2::new(790.0, 590.0);
        let hover = frame(&ctx, vec![egui::Event::PointerMoved(corner)], false);
        assert_eq!(hover.platform_output.cursor_icon, CursorIcon::ResizeNwSe);
        assert!(began_resize(&frame(&ctx, press(corner), false)));
    }

    #[test]
    fn press_outside_the_grip_does_not_resize() {
        let ctx = egui::Context::default();
        frame(&ctx, vec![], false);
        frame(&ctx, vec![], false);
        assert!(!began_resize(&frame(
            &ctx,
            press(Pos2::new(760.0, 590.0)),
            false
        )));
    }

    #[test]
    fn maximized_window_has_no_grip() {
        let ctx = egui::Context::default();
        frame(&ctx, vec![], true);
        frame(&ctx, vec![], true);
        let out = frame(&ctx, press(Pos2::new(790.0, 590.0)), true);
        assert!(!began_resize(&out));
        assert_eq!(out.platform_output.cursor_icon, CursorIcon::Default);
    }
}
