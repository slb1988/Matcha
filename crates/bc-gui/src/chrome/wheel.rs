//! Raw mouse-wheel deltas, applied 1:1 in the frame they arrive.
//!
//! egui 0.37 low-passes wheel deltas through `WheelState` (exponential bleed,
//! ~90% in 0.1s plus an asymptotic tail; Line/Page notches always take the
//! backlog path), so fast wheel scrolling trails the input by ~2/3 of the
//! travel and keeps gliding ~0.3s after it stops. Comparison panes run their
//! ScrollAreas with `mouse_wheel: false` and read the raw deltas here instead.
//! Conversion and modifiers use egui's native defaults (nothing in this
//! workspace overrides `InputOptions`, which has no public getter): 40pt per
//! line, Page = viewport height, Shift = horizontal, Alt = vertical,
//! Ctrl = zoom (left to egui, never scrolls).
use eframe::egui;

/// Scroll source for comparison panes: scrollbars and touch drag keep the
/// framework behavior; wheel input comes from [`delta`].
pub fn scroll_source() -> egui::containers::scroll_area::ScrollSource {
    egui::containers::scroll_area::ScrollSource {
        scroll_bar: true,
        drag: egui::containers::scroll_area::DragScroll::OnTouch,
        mouse_wheel: false,
    }
}

/// This frame's wheel delta (points, egui sign: wheel down is negative y) if
/// the pointer hovers any of `areas`; zero otherwise.
pub fn delta(ctx: &egui::Context, areas: &[egui::Rect]) -> egui::Vec2 {
    let Some(pointer) = ctx.input(|i| i.pointer.hover_pos()) else {
        return egui::Vec2::ZERO;
    };
    if !areas.iter().any(|r| r.contains(pointer)) {
        return egui::Vec2::ZERO;
    }
    ctx.input(|i| {
        let page = i.viewport_rect().height();
        // egui 0.37 native `InputOptions` defaults.
        const LINE_SPEED: f32 = 40.0;
        let zoom = egui::Modifiers::COMMAND;
        let horizontal = egui::Modifiers::SHIFT;
        let vertical = egui::Modifiers::ALT;
        let mut delta = egui::Vec2::ZERO;
        for event in &i.events {
            if let egui::Event::MouseWheel {
                unit,
                delta: d,
                modifiers,
                ..
            } = *event
            {
                if modifiers.matches_any(zoom) {
                    continue; // egui treats Ctrl+wheel as zoom, never scrolling
                }
                let mut d = match unit {
                    egui::MouseWheelUnit::Point => d,
                    egui::MouseWheelUnit::Line => LINE_SPEED * d,
                    egui::MouseWheelUnit::Page => page * d,
                };
                let is_horizontal = modifiers.matches_any(horizontal);
                let is_vertical = modifiers.matches_any(vertical);
                if is_horizontal && !is_vertical {
                    d = egui::vec2(d.x + d.y, 0.0);
                }
                if !is_horizontal && is_vertical {
                    d = egui::vec2(0.0, d.x + d.y);
                }
                delta += d;
            }
        }
        delta
    })
}
