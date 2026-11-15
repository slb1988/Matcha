//! Flow-layout controls in the Matcha design system — the drop-in
//! replacements for egui's stock widgets in panels, floating windows and
//! toolbars. (`chrome::dialog` keeps the fixed-position variants used by the
//! pixel-laid-out Session Settings / Options dialogs; both paint the same.)
//!
//! | egui stock | Matcha |
//! |---|---|
//! | `ui.button(t)` / `egui::Button::new(t)` | [`button`] / [`Button`] (`.primary()`, `.danger()`) |
//! | `ui.checkbox(&mut b, t)` | [`checkbox`] / [`Checkbox`] |
//! | `ui.radio(sel, t)` / `ui.radio_value(..)` | [`radio`] / [`radio_value`] |
//! | `ui.selectable_label` / `selectable_value` (lists) | [`list_row`] / [`list_value`] |
//! | `egui::ComboBox` | [`Combo`] (list = shared popup menu) |
//! | `ui.text_edit_singleline` | [`text_field`] / [`text_field_width`] |
//! | `ui.menu_button` | [`menu_button`] (items = [`MenuItem`]s) |
//! | OK / Cancel rows | [`dialog_buttons`] |
//!
//! All controls honour `ui.is_enabled()` (so `ui.add_enabled(..)` /
//! `add_enabled_ui` work) and read colors from [`theme::palette`], never from
//! constants, so a future skin swap restyles them.
use super::{
    controls::{self, Role},
    menubar::{ActionId, MenuItem},
    popup, theme,
};
use eframe::egui::{
    self, Color32, Galley, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, Widget, WidgetText,
};
use std::sync::Arc;

/// Height of compact controls (buttons, combos, fields) in panels.
pub const CONTROL_HEIGHT: f32 = 28.0;
const BUTTON_MIN_WIDTH: f32 = 64.0;
const BUTTON_PAD_X: f32 = 12.0;
const BOX: f32 = 14.0;
const BOX_GAP: f32 = 7.0;
const ROW_HEIGHT: f32 = 24.0;

fn layout(ui: &Ui, text: WidgetText) -> Arc<Galley> {
    text.into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    )
}

fn sense(enabled: bool) -> Sense {
    if enabled {
        Sense::click()
    } else {
        Sense::hover()
    }
}

// ------------------------------------------------------------- buttons --

/// Push button (28 px compact, or 32 px [`Button::large`] for dialog
/// footers). Secondary by default.
#[must_use = "add it with `ui.add(..)`"]
pub struct Button {
    text: WidgetText,
    role: Role,
    min_width: f32,
    height: f32,
    dropdown: bool,
    selected: bool,
}

impl Button {
    pub fn new(text: impl Into<WidgetText>) -> Self {
        Self {
            text: text.into(),
            role: Role::Secondary,
            min_width: BUTTON_MIN_WIDTH,
            height: CONTROL_HEIGHT,
            dropdown: false,
            selected: false,
        }
    }
    /// Filled accent (the default action).
    pub fn primary(mut self) -> Self {
        self.role = Role::Primary;
        self
    }
    /// Red ghost (delete / discard).
    pub fn danger(mut self) -> Self {
        self.role = Role::Destructive;
        self
    }
    pub fn role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }
    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }
    /// Dialog-footer size (32 px, same as `chrome::modal`).
    pub fn large(mut self) -> Self {
        self.height = controls::BUTTON_HEIGHT;
        self.min_width = self.min_width.max(controls::BUTTON_MIN_WIDTH);
        self
    }
    /// Trailing drop-down triangle (menu buttons).
    pub fn dropdown(mut self, dropdown: bool) -> Self {
        self.dropdown = dropdown;
        self
    }
    /// Toggled-on look (accent tint), for toggle buttons.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }
}

impl Widget for Button {
    fn ui(self, ui: &mut Ui) -> Response {
        let enabled = ui.is_enabled();
        let galley = layout(ui, self.text);
        let arrow = if self.dropdown { 14.0 } else { 0.0 };
        let width = (galley.size().x + 2.0 * BUTTON_PAD_X + arrow)
            .max(self.min_width)
            .round();
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(width, self.height), sense(enabled));
        let ink = if self.selected && enabled && self.role == Role::Secondary {
            paint_selected_face(ui, rect, &response)
        } else {
            controls::paint_face(ui, rect, &response, self.role, enabled)
        };
        let text_center = rect.center() - Vec2::new(arrow / 2.0, 0.0);
        ui.painter()
            .galley(text_center - galley.size() / 2.0, galley, ink);
        if self.dropdown {
            let c = Rect::from_min_max(
                egui::pos2(rect.right() - BUTTON_PAD_X - arrow + 4.0, rect.top()),
                egui::pos2(rect.right() - BUTTON_PAD_X + 4.0, rect.bottom()),
            )
            .center();
            dropdown_triangle(ui.painter(), c, ink);
        }
        response
    }
}

fn paint_selected_face(ui: &Ui, rect: Rect, response: &Response) -> Color32 {
    let p = theme::palette(ui.ctx());
    let fill = if response.is_pointer_button_down_on() {
        p.pressed
    } else {
        p.accent_soft
    };
    ui.painter()
        .rect_filled(rect, controls::BUTTON_RADIUS, fill);
    ui.painter().rect_stroke(
        rect,
        controls::BUTTON_RADIUS,
        Stroke::new(1.0, p.accent_soft_line),
        StrokeKind::Inside,
    );
    p.accent_pressed
}

fn dropdown_triangle(painter: &egui::Painter, c: egui::Pos2, color: Color32) {
    painter.add(egui::Shape::convex_polygon(
        vec![
            c + Vec2::new(-3.5, -1.5),
            c + Vec2::new(3.5, -1.5),
            c + Vec2::new(0.0, 2.5),
        ],
        color,
        Stroke::NONE,
    ));
}

/// Secondary push button.
pub fn button(ui: &mut Ui, text: impl Into<WidgetText>) -> Response {
    ui.add(Button::new(text))
}
/// Primary (filled accent) push button.
pub fn primary_button(ui: &mut Ui, text: impl Into<WidgetText>) -> Response {
    ui.add(Button::new(text).primary())
}

/// Right-aligned dialog footer: a hairline, then the actions in order (put
/// the primary last so it lands bottom-right; destructive ones go left).
/// Returns the clicked index. (Enter/Esc stay with the caller: a window may
/// hold multi-line fields where Enter is text.)
pub fn dialog_buttons(ui: &mut Ui, actions: &[(&str, Role)]) -> Option<usize> {
    let p = theme::palette(ui.ctx());
    ui.add_space(8.0);
    let y = ui.cursor().top();
    ui.painter()
        .hline(ui.max_rect().x_range(), y, Stroke::new(1.0, p.line));
    ui.add_space(12.0);
    let mut chosen = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        for (i, (label, role)) in actions.iter().enumerate() {
            if *role == Role::Destructive && ui.add(Button::new(*label).danger().large()).clicked()
            {
                chosen = Some(i);
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for (i, (label, role)) in actions.iter().enumerate().rev() {
                if *role != Role::Destructive
                    && ui.add(Button::new(*label).role(*role).large()).clicked()
                {
                    chosen = Some(i);
                }
            }
        });
    });
    chosen
}

// ------------------------------------------------- checkbox and radio --

/// Accent checkbox; the response is `changed()` when toggled.
#[must_use = "add it with `ui.add(..)`"]
pub struct Checkbox<'a> {
    value: &'a mut bool,
    text: WidgetText,
}

impl<'a> Checkbox<'a> {
    pub fn new(value: &'a mut bool, text: impl Into<WidgetText>) -> Self {
        Self {
            value,
            text: text.into(),
        }
    }
}

impl Widget for Checkbox<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let enabled = ui.is_enabled();
        let (rect, mut response, galley) = mark_layout(ui, self.text, enabled);
        if enabled && response.clicked() {
            *self.value = !*self.value;
            response.mark_changed();
        }
        let p = theme::palette(ui.ctx());
        let mark = Rect::from_min_size(
            egui::pos2(rect.left(), rect.center().y - BOX / 2.0),
            Vec2::splat(BOX),
        );
        let painter = ui.painter();
        if *self.value {
            let fill = if !enabled {
                p.text_faint
            } else if response.hovered() {
                p.accent_hover
            } else {
                p.accent
            };
            painter.rect_filled(mark, theme::RADIUS_SM, fill);
            let o = mark.min;
            painter.add(egui::Shape::line(
                vec![
                    o + Vec2::new(3.5, 7.0),
                    o + Vec2::new(6.0, 9.5),
                    o + Vec2::new(10.5, 4.5),
                ],
                Stroke::new(1.6, Color32::WHITE),
            ));
        } else {
            let edge = if !enabled {
                p.line
            } else if response.hovered() {
                p.accent
            } else {
                p.text_faint
            };
            painter.rect_filled(mark, theme::RADIUS_SM, p.surface);
            painter.rect_stroke(
                mark,
                theme::RADIUS_SM,
                Stroke::new(1.0, edge),
                StrokeKind::Inside,
            );
        }
        paint_mark_label(ui, rect, galley, enabled, &response);
        response
    }
}

/// Accent radio button; the response is `clicked()` when chosen.
#[must_use = "add it with `ui.add(..)`"]
pub struct RadioButton {
    selected: bool,
    text: WidgetText,
}

impl RadioButton {
    pub fn new(selected: bool, text: impl Into<WidgetText>) -> Self {
        Self {
            selected,
            text: text.into(),
        }
    }
}

impl Widget for RadioButton {
    fn ui(self, ui: &mut Ui) -> Response {
        let enabled = ui.is_enabled();
        let (rect, response, galley) = mark_layout(ui, self.text, enabled);
        let p = theme::palette(ui.ctx());
        let c = egui::pos2(rect.left() + BOX / 2.0, rect.center().y);
        let painter = ui.painter();
        if self.selected {
            let fill = if enabled { p.accent } else { p.text_faint };
            painter.circle_filled(c, BOX / 2.0, fill);
            painter.circle_filled(c, 3.0, Color32::WHITE);
        } else {
            let edge = if !enabled {
                p.line
            } else if response.hovered() {
                p.accent
            } else {
                p.text_faint
            };
            painter.circle_filled(c, BOX / 2.0, p.surface);
            painter.circle_stroke(c, BOX / 2.0 - 0.5, Stroke::new(1.0, edge));
        }
        paint_mark_label(ui, rect, galley, enabled, &response);
        response
    }
}

fn mark_layout(ui: &mut Ui, text: WidgetText, enabled: bool) -> (Rect, Response, Arc<Galley>) {
    let galley = layout(ui, text);
    let label_w = if galley.size().x > 0.0 {
        BOX_GAP + galley.size().x
    } else {
        0.0
    };
    let size = Vec2::new(
        BOX + label_w,
        galley.size().y.max(ROW_HEIGHT - 4.0),
    );
    let (rect, response) = ui.allocate_exact_size(size, sense(enabled));
    (rect, response, galley)
}

fn paint_mark_label(ui: &Ui, rect: Rect, galley: Arc<Galley>, enabled: bool, response: &Response) {
    let p = theme::palette(ui.ctx());
    let pos = egui::pos2(
        rect.left() + BOX + BOX_GAP,
        rect.center().y - galley.size().y / 2.0,
    );
    let ink = if enabled { p.text } else { p.text_faint };
    ui.painter().galley(pos, galley, ink);
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(2.0),
            theme::RADIUS_SM,
            Stroke::new(1.5, p.accent_soft_line),
            StrokeKind::Outside,
        );
    }
}

pub fn checkbox(ui: &mut Ui, value: &mut bool, text: impl Into<WidgetText>) -> Response {
    ui.add(Checkbox::new(value, text))
}
pub fn radio(ui: &mut Ui, selected: bool, text: impl Into<WidgetText>) -> Response {
    ui.add(RadioButton::new(selected, text))
}
/// Radio that sets `*current = value` when clicked (`changed()` then).
pub fn radio_value<T: PartialEq>(
    ui: &mut Ui,
    current: &mut T,
    value: T,
    text: impl Into<WidgetText>,
) -> Response {
    let mut response = radio(ui, *current == value, text);
    if response.clicked() && *current != value {
        *current = value;
        response.mark_changed();
    }
    response
}

// ----------------------------------------------------------- list rows --

/// Full-width selectable list row (recent paths, format lists, presets):
/// hover pill, accent tint when selected.
#[must_use = "add it with `ui.add(..)`"]
pub struct ListRow {
    selected: bool,
    text: WidgetText,
}

impl ListRow {
    pub fn new(selected: bool, text: impl Into<WidgetText>) -> Self {
        Self {
            selected,
            text: text.into(),
        }
    }
}

impl Widget for ListRow {
    fn ui(self, ui: &mut Ui) -> Response {
        let enabled = ui.is_enabled();
        let galley = layout(ui, self.text);
        let width = ui
            .available_width()
            .max(galley.size().x + 16.0)
            .min(ui.max_rect().width().max(galley.size().x + 16.0));
        let size = Vec2::new(width, galley.size().y.max(ROW_HEIGHT - 6.0) + 6.0);
        let (rect, response) = ui.allocate_exact_size(size, sense(enabled));
        let p = theme::palette(ui.ctx());
        let fill = if self.selected {
            Some(p.accent_soft)
        } else if enabled && response.hovered() {
            Some(p.hover)
        } else {
            None
        };
        if let Some(fill) = fill {
            ui.painter().rect_filled(rect, theme::RADIUS_MD, fill);
        }
        let ink = if !enabled {
            p.text_faint
        } else if self.selected {
            p.accent_pressed
        } else {
            p.text
        };
        ui.painter().galley(
            egui::pos2(rect.left() + 8.0, rect.center().y - galley.size().y / 2.0),
            galley,
            ink,
        );
        response
    }
}

pub fn list_row(ui: &mut Ui, selected: bool, text: impl Into<WidgetText>) -> Response {
    ui.add(ListRow::new(selected, text))
}
/// List row that sets `*current = value` when clicked.
pub fn list_value<T: PartialEq>(
    ui: &mut Ui,
    current: &mut T,
    value: T,
    text: impl Into<WidgetText>,
) -> Response {
    let mut response = list_row(ui, *current == value, text);
    if response.clicked() && *current != value {
        *current = value;
        response.mark_changed();
    }
    response
}

// ---------------------------------------------------------- text field --

/// Single-line text field, 28 px tall (global visuals give it the token
/// fill, outline and accent focus ring).
pub fn text_field(ui: &mut Ui, text: &mut String) -> Response {
    let width = ui.available_width().min(ui.spacing().text_edit_width.max(200.0));
    text_field_width(ui, text, width)
}
/// [`text_field`] with an explicit width.
pub fn text_field_width(ui: &mut Ui, text: &mut String, width: f32) -> Response {
    ui.add(field(text).desired_width(width))
}
/// The configured `TextEdit` behind [`text_field`] (chain `.hint_text`,
/// `.id`, `.password` … then `ui.add`).
pub fn field(text: &mut String) -> egui::TextEdit<'_> {
    egui::TextEdit::singleline(text)
        .margin(Vec2::new(8.0, 4.0))
        .min_size(Vec2::new(0.0, CONTROL_HEIGHT))
        .align(egui::Align2::LEFT_CENTER)
}

// ---------------------------------------------------------------- combo --

/// Drop-down list. The list is the shared popup menu; options are declared
/// in the `show` closure like egui's `ComboBox::show_ui`:
///
/// ```ignore
/// Combo::new("encoding", current.name()).show(ui, |list| {
///     list.selectable_value(&mut current, Enc::Utf8, "UTF-8");
///     list.separator();
///     list.selectable_value(&mut current, Enc::Ansi, "ANSI");
/// });
/// ```
pub struct Combo {
    id_salt: egui::IdSalt,
    text: String,
    width: Option<f32>,
    flat: bool,
}

impl Combo {
    pub fn new(id_salt: impl egui::AsIdSalt, selected_text: impl Into<WidgetText>) -> Self {
        Self {
            id_salt: egui::IdSalt::new(id_salt),
            text: selected_text.into().text().to_owned(),
            width: None,
            flat: false,
        }
    }
    /// Borderless box that only shows its outline on hover / while open
    /// (inline in info rows and status bars).
    pub fn flat(mut self) -> Self {
        self.flat = true;
        self
    }
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }
    /// Paint the box and, while open, its list. The response is `changed()`
    /// when an option was chosen this frame.
    pub fn show(self, ui: &mut Ui, mut add: impl FnMut(&mut ComboList)) -> Response {
        let enabled = ui.is_enabled();
        let id = ui.make_persistent_id(self.id_salt);
        let p = theme::palette(ui.ctx());
        let galley = layout(ui, WidgetText::from(self.text.as_str()));
        // Text + 8 px inset + 24 px chevron slot; flat (inline) combos never
        // truncate their value, boxed ones honour the requested width.
        let fit = (galley.size().x + 36.0).ceil();
        let width = match self.width {
            Some(w) if self.flat => w.max(fit),
            Some(w) => w,
            None => fit.max(120.0),
        };
        let (rect, mut response) =
            ui.allocate_exact_size(Vec2::new(width, CONTROL_HEIGHT), sense(enabled));
        let menu_id = id.with("combo-menu");
        let open = popup::is_menu_open(ui.ctx(), menu_id);
        let edge = if !enabled {
            p.line
        } else if open || response.has_focus() {
            p.accent
        } else if response.hovered() {
            p.text_faint
        } else {
            p.line_strong
        };
        let painter = ui.painter();
        if !self.flat {
            painter.rect_filled(
                rect,
                theme::RADIUS_MD,
                if enabled { p.surface } else { p.surface_subtle },
            );
            painter.rect_stroke(
                rect,
                theme::RADIUS_MD,
                Stroke::new(1.0, edge),
                StrokeKind::Inside,
            );
        } else if enabled && (open || response.hovered()) {
            painter.rect_filled(rect, theme::RADIUS_MD, p.hover);
        }
        let text_rect = Rect::from_min_max(
            rect.min + Vec2::new(8.0, 0.0),
            rect.max - Vec2::new(24.0, 0.0),
        );
        let ink = if !enabled {
            p.text_faint
        } else if self.flat {
            p.text_muted
        } else {
            p.text
        };
        painter.with_clip_rect(text_rect).galley(
            egui::pos2(text_rect.left(), rect.center().y - galley.size().y / 2.0),
            galley,
            ink,
        );
        let c = egui::pos2(rect.right() - 13.0, rect.center().y);
        painter.add(egui::Shape::line(
            vec![
                c + Vec2::new(-3.5, -1.5),
                c + Vec2::new(0.0, 2.0),
                c + Vec2::new(3.5, -1.5),
            ],
            Stroke::new(1.4, if enabled { p.text_muted } else { p.text_faint }),
        ));
        if enabled && response.clicked() {
            popup::toggle_menu_below(ui.ctx(), menu_id, rect);
        }
        let chosen = popup::menu(ui.ctx(), menu_id, rect.width(), || {
            let mut list = ComboList::new(None);
            add(&mut list);
            list.items
        });
        if let Some(index) = chosen.and_then(|a| a.parse::<usize>().ok()) {
            let mut list = ComboList::new(Some(index));
            add(&mut list);
            if list.changed {
                response.mark_changed();
            }
        }
        response
    }
}

/// Options of an open [`Combo`]; mirrors egui's `ComboBox::show_ui` calls.
pub struct ComboList {
    items: Vec<MenuItem>,
    chosen: Option<usize>,
    next: usize,
    changed: bool,
}

impl ComboList {
    fn new(chosen: Option<usize>) -> Self {
        Self {
            items: Vec::new(),
            chosen,
            next: 0,
            changed: false,
        }
    }
    fn push(&mut self, label: WidgetText, selected: bool, enabled: bool) -> bool {
        let index = self.next;
        self.next += 1;
        self.items.push(MenuItem {
            enabled,
            ..popup::item::radio(label.text(), index.to_string(), selected)
        });
        self.chosen == Some(index)
    }
    /// Option that sets `*current = value` when chosen; true if it was.
    pub fn selectable_value<T: PartialEq>(
        &mut self,
        current: &mut T,
        value: T,
        label: impl Into<WidgetText>,
    ) -> bool {
        let hit = self.push(label.into(), *current == value, true);
        if hit && *current != value {
            *current = value;
            self.changed = true;
        }
        hit
    }
    /// Option shown checked when `selected`; true when chosen this frame.
    pub fn selectable_label(&mut self, selected: bool, label: impl Into<WidgetText>) -> bool {
        let hit = self.push(label.into(), selected, true);
        self.changed |= hit;
        hit
    }
    /// Greyed-out option.
    pub fn disabled(&mut self, label: impl Into<WidgetText>) {
        self.push(label.into(), false, false);
    }
    pub fn separator(&mut self) {
        self.items.push(popup::item::separator());
    }
}

// ---------------------------------------------------------- menu button --

/// Button with a drop-down triangle that opens `items` (shared popup menu)
/// under it. Returns the chosen action.
pub fn menu_button(
    ui: &mut Ui,
    text: impl Into<WidgetText>,
    items: impl FnOnce() -> Vec<MenuItem>,
) -> Option<ActionId> {
    let response = ui.add(Button::new(text).dropdown(true));
    popup::menu_below(&response, items)
}

// ------------------------------------------------------------ headings --

/// Section heading inside panels and windows (strong, 13 px).
pub fn section(ui: &mut Ui, text: impl Into<String>) -> Response {
    let p = theme::palette(ui.ctx());
    ui.add_space(4.0);
    let response = ui.label(
        egui::RichText::new(text.into())
            .font(theme::strong(ui.ctx(), theme::UI_SIZE))
            .color(p.text),
    );
    ui.add_space(2.0);
    response
}
/// Secondary explanatory text (muted, wraps).
pub fn hint(ui: &mut Ui, text: impl Into<String>) -> Response {
    let p = theme::palette(ui.ctx());
    ui.add(egui::Label::new(egui::RichText::new(text.into()).color(p.text_muted)).wrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        ctx: &egui::Context,
        events: Vec<egui::Event>,
        run: impl FnMut(&mut Ui),
    ) -> egui::FullOutput {
        crate::chrome::test_frame(
            ctx,
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, Vec2::splat(400.0))),
                events,
                ..Default::default()
            },
            run,
        )
    }
    fn click(pos: egui::Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ]
    }

    #[test]
    fn checkbox_toggles_and_reports_change_but_not_when_disabled() {
        let ctx = egui::Context::default();
        let mut on = false;
        let mut changed = false;
        let mut rect = Rect::NOTHING;
        frame(&ctx, vec![], |ui| rect = checkbox(ui, &mut on, "Enabled").rect);
        frame(&ctx, click(rect.center()), |ui| {
            changed = checkbox(ui, &mut on, "Enabled").changed();
        });
        assert!(on && changed);
        frame(&ctx, click(rect.center()), |ui| {
            ui.add_enabled(false, Checkbox::new(&mut on, "Enabled"));
        });
        assert!(on, "disabled checkbox ignores clicks");
    }

    #[test]
    fn combo_opens_list_and_applies_choice() {
        let ctx = egui::Context::default();
        let mut value = 1;
        let mut changed = false;
        let mut rect = Rect::NOTHING;
        let show = |ui: &mut Ui, changed: &mut bool, value: &mut i32| {
            let r = Combo::new("c", format!("v{value}"))
                .width(150.0)
                .show(ui, |list| {
                    list.selectable_value(value, 1, "one");
                    list.separator();
                    list.selectable_value(value, 2, "two");
                });
            *changed |= r.changed();
            r.rect
        };
        frame(&ctx, vec![], |ui| rect = show(ui, &mut changed, &mut value));
        frame(&ctx, click(rect.center()), |ui| {
            show(ui, &mut changed, &mut value);
        });
        // Popup sizing pass, then click the second option (row 1 + separator).
        frame(&ctx, vec![], |ui| {
            show(ui, &mut changed, &mut value);
        });
        let two = egui::pos2(
            rect.left() + 40.0,
            rect.bottom() + 2.0 + 1.0 + 4.0 + popup::ITEM_HEIGHT + popup::SEPARATOR_HEIGHT + 13.0,
        );
        frame(&ctx, click(two), |ui| {
            show(ui, &mut changed, &mut value);
        });
        assert_eq!(value, 2);
        assert!(changed);
    }

    #[test]
    fn dialog_buttons_report_clicked_index() {
        let ctx = egui::Context::default();
        frame(&ctx, vec![], |ui| {
            dialog_buttons(ui, &[("Cancel", Role::Secondary), ("OK", Role::Primary)]);
        });
        // Primary is right-most: the last 84+ px of the row.
        let mut chosen = None;
        frame(&ctx, click(egui::pos2(395.0 - 40.0, 8.0 + 12.0 + 16.0)), |ui| {
            chosen = dialog_buttons(ui, &[("Cancel", Role::Secondary), ("OK", Role::Primary)]);
        });
        assert_eq!(chosen, Some(1));
    }
}
