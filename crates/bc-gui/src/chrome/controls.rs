//! Modern push buttons and small decorations shared by dialogs and modals.
//!
//! Buttons are 32 px tall with an 8 px radius. Roles map to intent:
//! `Primary` (filled accent, the Enter default), `Secondary` (outlined
//! neutral) and `Destructive` (red ghost — for "Don't Save" / "Delete").
use super::theme;
use eframe::egui::{self, Color32, FontId, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

pub const BUTTON_HEIGHT: f32 = 32.0;
pub const BUTTON_MIN_WIDTH: f32 = 84.0;
pub const BUTTON_RADIUS: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Primary,
    Secondary,
    Destructive,
}

/// Width a button needs for `label`.
pub fn button_width(ui: &Ui, label: &str) -> f32 {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), font(), Color32::BLACK);
    (galley.size().x + 32.0).max(BUTTON_MIN_WIDTH).round()
}

fn font() -> FontId {
    FontId::proportional(theme::UI_SIZE)
}

/// Lay out and paint a button in the current layout.
pub fn button(ui: &mut Ui, label: &str, role: Role, enabled: bool) -> Response {
    let size = Vec2::new(button_width(ui, label), BUTTON_HEIGHT);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    paint_button(ui, rect, &response, label, role, enabled);
    response
}

/// Paint a button face into `rect` for an existing `response`.
pub fn paint_button(
    ui: &Ui,
    rect: Rect,
    response: &Response,
    label: &str,
    role: Role,
    enabled: bool,
) {
    let ink = paint_face(ui, rect, response, role, enabled);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        font(),
        ink,
    );
}

/// Paint only the button face (fill, outline, focus ring) and return the ink
/// color its label should use.
pub fn paint_face(ui: &Ui, rect: Rect, response: &Response, role: Role, enabled: bool) -> Color32 {
    let hovered = enabled && response.hovered();
    let pressed = enabled && response.is_pointer_button_down_on();
    let p = theme::palette(ui.ctx());
    let (fill, line, ink) = match (enabled, role) {
        (false, _) => (p.surface_subtle, p.line, p.text_faint),
        (true, Role::Primary) => (
            if pressed {
                p.accent_pressed
            } else if hovered {
                p.accent_hover
            } else {
                p.accent
            },
            Color32::TRANSPARENT,
            Color32::WHITE,
        ),
        (true, Role::Secondary) => (
            if pressed {
                p.pressed
            } else if hovered {
                p.hover
            } else {
                p.surface
            },
            p.line_strong,
            p.text,
        ),
        (true, Role::Destructive) => (
            if pressed {
                p.danger_pressed
            } else if hovered {
                p.danger_soft
            } else {
                Color32::TRANSPARENT
            },
            Color32::TRANSPARENT,
            p.danger,
        ),
    };
    let painter = ui.painter();
    painter.rect_filled(rect, BUTTON_RADIUS, fill);
    if line != Color32::TRANSPARENT {
        painter.rect_stroke(
            rect,
            BUTTON_RADIUS,
            Stroke::new(1.0, line),
            StrokeKind::Inside,
        );
    }
    if response.has_focus() {
        painter.rect_stroke(
            rect.expand(2.0),
            BUTTON_RADIUS + 2,
            Stroke::new(2.0, p.accent_soft_line),
            StrokeKind::Inside,
        );
    }
    ink
}

/// Small rounded label chip (e.g. "LEFT" / "RIGHT").
pub fn chip(ui: &mut Ui, text: &str, fill: Color32, ink: Color32) -> Response {
    let font = FontId::proportional(10.5);
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, ink);
    let size = Vec2::new((galley.size().x + 12.0).max(44.0), 18.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, 9.0, fill);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, ink);
    response
}
