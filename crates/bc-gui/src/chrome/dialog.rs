//! Fixed-layout dialog chrome shared by Session Settings and Options.
//!
//! Controls are painted at fixed positions with the Matcha design tokens:
//! rounded push buttons (default = filled accent), accent checkboxes and
//! radios, rounded text fields and combo boxes whose list uses the shared
//! popup. Everything is painted without animation.
use super::{
    menubar::{ActionId, MenuItem},
    popup, theme,
};
use eframe::egui::{
    self, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2,
};

pub const FACE: Color32 = theme::SURFACE_CHROME;
pub const PAGE: Color32 = theme::SURFACE;
const BUTTON_FILL: Color32 = theme::SURFACE;
const DISABLED_EDGE: Color32 = theme::LINE;
const DISABLED_FILL: Color32 = theme::SURFACE_SUBTLE;
pub const DISABLED_TEXT: Color32 = theme::TEXT_FAINT;
const CHECK_EDGE: Color32 = theme::TEXT_FAINT;
const ACCENT: Color32 = theme::ACCENT;
const FIELD_EDGE: Color32 = theme::LINE_STRONG;
const COMBO_EDGE: Color32 = theme::LINE_STRONG;
const HEADING: Color32 = theme::TEXT;
pub const RULE: Color32 = theme::LINE;
/// Classic push-button size (OK / Cancel / Apply).
pub const BUTTON: Vec2 = Vec2::new(94.0, 29.0);
/// Header height of framed dialogs (same as `chrome::window`).
pub const TITLE_HEIGHT: f32 = 44.0;

pub fn font() -> FontId {
    FontId::proportional(theme::UI_SIZE)
}
fn text_color(enabled: bool) -> Color32 {
    if enabled {
        theme::TEXT
    } else {
        DISABLED_TEXT
    }
}

/// Where a dialog's client area goes: a framed window centered on the
/// screen, or (deterministic snapshots of the dialog alone) the whole
/// viewport without frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    Window,
    ClientOnly,
}

/// Show a dialog with the given client size. Returns `(close_clicked,
/// inner)`; the title bar's × reports a close request.
pub fn window<R>(
    ctx: &egui::Context,
    id: egui::Id,
    title: &str,
    client: Vec2,
    placement: Placement,
    add: impl FnOnce(&mut Ui, Rect) -> R,
) -> (bool, R) {
    let screen = ctx.content_rect();
    let (outer, origin) = match placement {
        Placement::ClientOnly => (client, screen.min),
        Placement::Window => {
            let outer = client + Vec2::new(2.0, TITLE_HEIGHT + 2.0);
            let origin = (screen.center() - outer / 2.0).max(screen.min);
            (outer, Pos2::new(origin.x.round(), origin.y.round()))
        }
    };
    let mut closed = false;
    let inner = egui::Area::new(id)
        .order(egui::Order::Foreground)
        .fixed_pos(origin)
        .movable(false)
        .fade_in(false)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(outer, Sense::hover());
            let painter = ui.painter().clone();
            let client_rect = match placement {
                Placement::ClientOnly => {
                    painter.rect_filled(rect, 0.0, FACE);
                    rect
                }
                Placement::Window => {
                    // Same card as `chrome::window::Window` / `chrome::modal`.
                    let p = theme::palette(ui.ctx());
                    let radius = theme::RADIUS_XL;
                    painter.add(theme::shadow_dialog().as_shape(rect, radius));
                    painter.rect_filled(rect, radius, p.surface);
                    painter.text(
                        rect.min + Vec2::new(16.0, TITLE_HEIGHT / 2.0),
                        egui::Align2::LEFT_CENTER,
                        title,
                        theme::strong(ui.ctx(), theme::UI_SIZE + 1.0),
                        p.text,
                    );
                    let close = Rect::from_center_size(
                        Pos2::new(rect.right() - 24.0, rect.top() + TITLE_HEIGHT / 2.0),
                        Vec2::splat(28.0),
                    );
                    let response = ui.interact(close, id.with("close"), Sense::click());
                    if response.hovered() {
                        painter.rect_filled(close, theme::RADIUS_MD, p.hover);
                    }
                    let ink = if response.hovered() {
                        p.text
                    } else {
                        p.text_muted
                    };
                    let c = close.center();
                    for d in [Vec2::new(4.5, 4.5), Vec2::new(4.5, -4.5)] {
                        painter.line_segment([c - d, c + d], Stroke::new(1.4, ink));
                    }
                    closed = response.clicked();
                    let client_rect =
                        Rect::from_min_size(rect.min + Vec2::new(1.0, TITLE_HEIGHT + 1.0), client);
                    painter.hline(
                        rect.x_range(),
                        rect.top() + TITLE_HEIGHT,
                        Stroke::new(1.0, p.line),
                    );
                    let bottom = radius - 1;
                    painter.rect_filled(
                        client_rect,
                        egui::CornerRadius {
                            nw: 0,
                            ne: 0,
                            sw: bottom,
                            se: bottom,
                        },
                        FACE,
                    );
                    painter.rect_stroke(rect, radius, Stroke::new(1.0, p.line), StrokeKind::Inside);
                    client_rect
                }
            };
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(client_rect));
            add(&mut child, client_rect)
        })
        .inner;
    (closed, inner)
}

/// Tab strip over a tab page (`page` is the page's outer rect; tabs sit
/// above it, the first starting at its left edge). Returns the clicked tab.
/// Modern underline tabs: the page is a rounded card, the selected tab's
/// label is strong with an accent underline on the page edge, the others are
/// muted with a hover pill. (Tab widths keep the classic label + 18 / + 10 px
/// so hit positions are stable.)
pub fn tabs(ui: &mut Ui, page: Rect, labels: &[&str], selected: usize) -> Option<usize> {
    let p = theme::palette(ui.ctx());
    let painter = ui.painter().clone();
    painter.rect_filled(page, theme::RADIUS_LG, p.surface);
    painter.rect_stroke(
        page,
        theme::RADIUS_LG,
        Stroke::new(1.0, p.line),
        StrokeKind::Inside,
    );
    let mut clicked = None;
    let mut x = page.left();
    for (i, label) in labels.iter().enumerate() {
        let on = i == selected;
        let font = if on {
            theme::strong(ui.ctx(), theme::UI_SIZE)
        } else {
            font()
        };
        let ink = if on { p.text } else { p.text_muted };
        let galley = painter.layout_no_wrap(label.to_string(), font, ink);
        let width = (galley.size().x + if on { 18.0 } else { 10.0 }).round();
        let rect = Rect::from_min_max(
            Pos2::new(x, page.top() - 24.0),
            Pos2::new(x + width + 1.0, page.top()),
        );
        let response = ui.interact(rect, ui.scope_id().with(("dialog-tab", i)), Sense::click());
        if !on && response.hovered() {
            painter.rect_filled(rect.shrink2(Vec2::new(1.0, 2.0)), theme::RADIUS_MD, p.hover);
        }
        painter.galley(rect.center() - galley.size() / 2.0, galley, ink);
        if on {
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(rect.left() + 4.0, page.top() - 2.0),
                    Pos2::new(rect.right() - 4.0, page.top()),
                ),
                1.0,
                p.accent,
            );
        }
        if response.clicked() {
            clicked = Some(i);
        }
        x += width;
    }
    clicked
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Normal,
    Default,
}
/// Classic push button with a centered label; true when clicked (enabled).
pub fn button(ui: &mut Ui, rect: Rect, label: &str, kind: ButtonKind, enabled: bool) -> bool {
    button_face(ui, rect, label, kind, enabled, true)
}
fn button_face(
    ui: &mut Ui,
    rect: Rect,
    label: &str,
    kind: ButtonKind,
    enabled: bool,
    interactive: bool,
) -> bool {
    let sense = if interactive {
        Sense::click()
    } else {
        Sense::hover()
    };
    let response = ui.interact(
        rect,
        ui.scope_id()
            .with(("dialog-button", label, rect.min.x as i32, rect.min.y as i32)),
        sense,
    );
    let role = match kind {
        ButtonKind::Default => super::controls::Role::Primary,
        ButtonKind::Normal => super::controls::Role::Secondary,
    };
    super::controls::paint_button(ui, rect, &response, label, role, enabled);
    enabled && response.clicked()
}

/// 13 px checkbox at `pos` (box top-left) with its label 17 px to the
/// right; toggles `value` when clicked (and enabled).
pub fn checkbox(ui: &mut Ui, pos: Pos2, label: &str, value: &mut bool, enabled: bool) -> bool {
    let painter = ui.painter().clone();
    let galley = painter.layout_no_wrap(label.to_string(), font(), text_color(enabled));
    let boxed = Rect::from_min_size(pos, Vec2::splat(13.0));
    let hit = Rect::from_min_max(
        pos - Vec2::new(1.0, 2.0),
        pos + Vec2::new(17.0 + galley.size().x + 2.0, 15.0),
    );
    let response = ui.interact(
        hit,
        ui.scope_id().with(("dialog-check", label, pos.y as i32)),
        Sense::click(),
    );
    let changed = enabled && response.clicked();
    if changed {
        *value = !*value;
    }
    if *value {
        let fill = if enabled { ACCENT } else { DISABLED_TEXT };
        painter.rect_filled(boxed, 3.0, fill);
        painter.add(egui::Shape::line(
            vec![
                pos + Vec2::new(3.0, 6.5),
                pos + Vec2::new(5.5, 9.0),
                pos + Vec2::new(10.0, 4.0),
            ],
            Stroke::new(1.5, Color32::WHITE),
        ));
    } else {
        painter.rect_filled(boxed, 3.0, theme::SURFACE);
        painter.rect_stroke(
            boxed,
            3.0,
            Stroke::new(1.0, if enabled { CHECK_EDGE } else { DISABLED_TEXT }),
            StrokeKind::Inside,
        );
    }
    painter.galley(
        Pos2::new(pos.x + 17.0, boxed.center().y - galley.size().y / 2.0),
        galley,
        text_color(enabled),
    );
    changed
}

/// 13 px radio button; returns true when clicked (and enabled).
pub fn radio(ui: &mut Ui, pos: Pos2, label: &str, selected: bool, enabled: bool) -> bool {
    let painter = ui.painter().clone();
    let galley = painter.layout_no_wrap(label.to_string(), font(), text_color(enabled));
    let text_width = galley.size().x;
    let c = pos + Vec2::splat(6.5);
    if selected {
        painter.circle_filled(c, 6.5, if enabled { ACCENT } else { DISABLED_TEXT });
        painter.circle_filled(c, 3.0, Color32::WHITE);
    } else {
        painter.circle_filled(c, 6.5, theme::SURFACE);
        painter.circle_stroke(
            c,
            6.0,
            Stroke::new(1.0, if enabled { CHECK_EDGE } else { DISABLED_TEXT }),
        );
    }
    painter.galley(
        Pos2::new(pos.x + 17.0, c.y - galley.size().y / 2.0),
        galley,
        text_color(enabled),
    );
    let hit = Rect::from_min_max(
        pos - Vec2::new(1.0, 2.0),
        pos + Vec2::new(17.0 + text_width + 2.0, 15.0),
    );
    enabled
        && ui
            .interact(
                hit,
                ui.scope_id().with(("dialog-radio", label, pos.y as i32)),
                Sense::click(),
            )
            .clicked()
}

/// Single-line text field: white with a #8D8D8D border, text inset 3 px.
pub fn text_field(
    ui: &mut Ui,
    rect: Rect,
    id: egui::Id,
    text: &mut String,
    enabled: bool,
) -> Response {
    let painter = ui.painter().clone();
    let radius = theme::RADIUS_MD;
    painter.rect_filled(
        rect,
        radius,
        if enabled {
            theme::SURFACE
        } else {
            DISABLED_FILL
        },
    );
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, if enabled { FIELD_EDGE } else { DISABLED_EDGE }),
        StrokeKind::Inside,
    );
    let inner = Rect::from_min_max(
        rect.min + Vec2::new(7.0, 1.0),
        rect.max - Vec2::new(7.0, 1.0),
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
        ui.add_enabled(
            enabled,
            egui::TextEdit::singleline(text)
                .id(id)
                .frame(egui::Frame::NONE)
                .font(font())
                .align(egui::Align2::LEFT_CENTER)
                .min_size(inner.size())
                .desired_width(inner.width()),
        )
    })
    .inner
}

/// Original vector glyphs drawn on small push buttons.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    /// Down arrow (copy spec downwards).
    Down,
    /// Two opposed horizontal arrows (swap sides).
    Swap,
    /// Solid down triangle (drop-down list).
    DropDown,
}
pub fn paint_glyph(painter: &egui::Painter, center: Pos2, glyph: Glyph, color: Color32) {
    let stroke = Stroke::new(1.5, color);
    match glyph {
        Glyph::Down => {
            painter.line_segment(
                [center + Vec2::new(0.0, -6.0), center + Vec2::new(0.0, 5.0)],
                stroke,
            );
            painter.add(egui::Shape::convex_polygon(
                vec![
                    center + Vec2::new(-4.0, 1.5),
                    center + Vec2::new(4.0, 1.5),
                    center + Vec2::new(0.0, 6.5),
                ],
                color,
                Stroke::NONE,
            ));
        }
        Glyph::Swap => {
            for (dy, dir) in [(-3.0, 1.0), (3.0, -1.0)] {
                let a = center + Vec2::new(-5.5 * dir, dy);
                let b = center + Vec2::new(5.5 * dir, dy);
                painter.line_segment([a, b], stroke);
                painter.line_segment([b, b + Vec2::new(-3.0 * dir, -2.5)], stroke);
                painter.line_segment([b, b + Vec2::new(-3.0 * dir, 2.5)], stroke);
            }
        }
        Glyph::DropDown => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    center + Vec2::new(-4.0, -2.0),
                    center + Vec2::new(4.0, -2.0),
                    center + Vec2::new(0.0, 2.5),
                ],
                color,
                Stroke::NONE,
            ));
        }
    }
}
/// Small push button showing a glyph; true when clicked (enabled).
pub fn glyph_button(ui: &mut Ui, rect: Rect, glyph: Glyph, enabled: bool) -> bool {
    let clicked = button(ui, rect, "", ButtonKind::Normal, enabled);
    let p = theme::palette(ui.ctx());
    paint_glyph(
        ui.painter(),
        rect.center(),
        glyph,
        if enabled { p.text } else { p.text_faint },
    );
    clicked
}

/// Down chevron used inside combo boxes (same stroke chevron as
/// `widgets::Combo`); `left_top` is the top-left of its 6 x 3 px box.
pub fn combo_chevron(painter: &egui::Painter, left_top: Pos2, color: Color32) {
    let c = left_top + Vec2::new(3.0, 1.5);
    painter.add(egui::Shape::line(
        vec![
            c + Vec2::new(-3.5, -1.5),
            c + Vec2::new(0.0, 2.0),
            c + Vec2::new(3.5, -1.5),
        ],
        Stroke::new(1.4, color),
    ));
}

#[derive(Clone, Debug, Default)]
struct ComboState {
    path: Vec<usize>,
}
/// Drop-down list combo box (#D2 border, #FD face, #BC bottom line); the
/// list is a classic popup under the box. Returns the chosen index.
pub fn combo(
    ui: &mut Ui,
    rect: Rect,
    id: egui::Id,
    selected: &str,
    items: &[String],
    enabled: bool,
) -> Option<usize> {
    list(ui, rect, id, selected, items, enabled, false)
}
/// Push button with a solid down triangle that opens the same list.
pub fn dropdown_button(
    ui: &mut Ui,
    rect: Rect,
    id: egui::Id,
    items: &[String],
    enabled: bool,
) -> Option<usize> {
    list(ui, rect, id, "", items, enabled, true)
}
fn list(
    ui: &mut Ui,
    rect: Rect,
    id: egui::Id,
    selected: &str,
    items: &[String],
    enabled: bool,
    as_button: bool,
) -> Option<usize> {
    let painter = ui.painter().clone();
    let response = ui.interact(rect, id, Sense::click());
    if as_button {
        button_face(ui, rect, "", ButtonKind::Normal, enabled, false);
        let p = theme::palette(ui.ctx());
        paint_glyph(
            &painter,
            rect.center() + Vec2::new(0.0, 0.5),
            Glyph::DropDown,
            if enabled { p.text_muted } else { p.text_faint },
        );
    } else {
        let radius = theme::RADIUS_MD;
        painter.rect_filled(
            rect,
            radius,
            if enabled { BUTTON_FILL } else { DISABLED_FILL },
        );
        painter.rect_stroke(
            rect,
            radius,
            Stroke::new(1.0, if enabled { COMBO_EDGE } else { DISABLED_EDGE }),
            StrokeKind::Inside,
        );
        painter.text(
            Pos2::new(rect.left() + 8.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            selected,
            font(),
            text_color(enabled),
        );
        combo_chevron(
            &painter,
            Pos2::new(rect.right() - 16.0, rect.center().y - 1.5),
            theme::palette(ui.ctx()).text_muted,
        );
    }
    let ctx = ui.ctx().clone();
    let key = id.with("combo-state");
    let mut state = ctx.data(|d| d.get_temp::<ComboState>(key));
    if enabled && response.clicked() {
        state = if state.is_some() {
            None
        } else {
            Some(ComboState {
                path: vec![popup::NONE],
            })
        };
    }
    let mut chosen = None;
    if let Some(mut open) = state.take() {
        let menu: Vec<MenuItem> = items
            .iter()
            .enumerate()
            .map(|(i, label)| MenuItem::action(label.clone(), i.to_string()))
            .collect();
        let (action, rects): (Option<ActionId>, Vec<Rect>) =
            popup::cascade(&ctx, key, rect.left_bottom(), &menu, &mut open.path, false);
        chosen = action.and_then(|a| a.parse().ok());
        let away = ctx.input(|i| {
            i.pointer.any_click()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| !rect.contains(p) && !rects.iter().any(|r| r.contains(p)))
        });
        let escape = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if chosen.is_none() && !away && !escape {
            state = Some(open);
        }
    }
    match state {
        Some(open) => {
            ctx.data_mut(|d| d.insert_temp(key, open));
        }
        None => ctx.data_mut(|d| d.remove::<ComboState>(key)),
    }
    chosen
}

/// Options-style section heading (gray, 16 px).
pub fn heading(painter: &egui::Painter, pos: Pos2, text: &str) {
    painter.text(
        pos,
        egui::Align2::LEFT_TOP,
        text,
        FontId::proportional(16.0),
        HEADING,
    );
}
/// 1 px horizontal rule.
pub fn rule(painter: &egui::Painter, x0: f32, x1: f32, y: f32, color: Color32) {
    painter.rect_filled(
        Rect::from_min_max(Pos2::new(x0, y), Pos2::new(x1, y + 1.0)),
        0.0,
        color,
    );
}
/// Plain label at `pos` (left-top of the text box).
pub fn label(painter: &egui::Painter, pos: Pos2, text: &str, enabled: bool) {
    painter.text(
        pos,
        egui::Align2::LEFT_TOP,
        text,
        font(),
        text_color(enabled),
    );
}
