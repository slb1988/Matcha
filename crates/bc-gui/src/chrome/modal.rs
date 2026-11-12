//! Modern confirmation modal: dimmed backdrop, centered card, tone badge,
//! title + message, optional item list, and an action footer.
//!
//! Keyboard: Enter triggers the first `Primary` action, Esc the action
//! registered with [`Modal::cancel`]. Destructive actions sit on the left of
//! the footer, away from the default; the rest are right-aligned in order
//! (put the primary last so it lands bottom-right).
use super::{
    controls::{self, Role},
    theme,
};
use eframe::egui::{self, Color32, CornerRadius, FontId, Margin, Sense, Stroke, Ui, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Info,
    Question,
    Warning,
    Danger,
}

impl Tone {
    fn colors(self) -> (Color32, Color32) {
        match self {
            Tone::Info => (theme::INFO_SOFT, theme::INFO),
            Tone::Question => (theme::ACCENT_SOFT, theme::ACCENT),
            Tone::Warning => (theme::WARNING_SOFT, theme::WARNING),
            Tone::Danger => (theme::DANGER_SOFT, theme::DANGER),
        }
    }
    fn glyph(self) -> &'static str {
        match self {
            Tone::Info => "i",
            Tone::Question => "?",
            Tone::Warning | Tone::Danger => "!",
        }
    }
}

/// One row in the modal's item list (e.g. an unsaved file).
#[derive(Clone, Debug)]
pub struct Item {
    pub badge: String,
    pub title: String,
    pub detail: String,
}

#[derive(Clone, Debug)]
struct Action {
    label: String,
    role: Role,
}

pub struct Modal {
    id: egui::Id,
    tone: Tone,
    title: String,
    message: String,
    items: Vec<Item>,
    actions: Vec<Action>,
    cancel: Option<usize>,
    width: f32,
}

pub const WIDTH: f32 = 440.0;
const PAD_X: i8 = 24;
const BADGE: f32 = 40.0;

impl Modal {
    pub fn new(id: &str, tone: Tone, title: impl Into<String>) -> Self {
        Self {
            id: egui::Id::unique(("matcha-modal", id)),
            tone,
            title: title.into(),
            message: String::new(),
            items: Vec::new(),
            actions: Vec::new(),
            cancel: None,
            width: WIDTH,
        }
    }
    pub fn message(mut self, text: impl Into<String>) -> Self {
        self.message = text.into();
        self
    }
    pub fn item(
        mut self,
        badge: impl Into<String>,
        title: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        self.items.push(Item {
            badge: badge.into(),
            title: title.into(),
            detail: detail.into(),
        });
        self
    }
    /// Add an action button; its index is the value [`Modal::show`] returns.
    pub fn action(mut self, label: impl Into<String>, role: Role) -> Self {
        self.actions.push(Action {
            label: label.into(),
            role,
        });
        self
    }
    /// The action Esc triggers (usually "Cancel").
    pub fn cancel(mut self, index: usize) -> Self {
        self.cancel = Some(index);
        self
    }
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Show the modal this frame; returns the chosen action index.
    pub fn show(self, ctx: &egui::Context) -> Option<usize> {
        let frame = egui::Frame::NONE
            .fill(theme::SURFACE)
            .stroke(Stroke::new(1.0, theme::LINE))
            .corner_radius(theme::RADIUS_XL)
            .shadow(theme::shadow_dialog());
        let response = egui::Modal::new(self.id)
            .backdrop_color(theme::SCRIM)
            .frame(frame)
            .show(ctx, |ui| self.contents(ui));
        let mut chosen = response.inner;
        if chosen.is_none() && response.is_top_modal {
            let (enter, escape) = ctx.input_mut(|i| {
                (
                    i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
                    i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
                )
            });
            if enter {
                chosen = self.actions.iter().position(|a| a.role == Role::Primary);
            } else if escape {
                chosen = self.cancel;
            }
        }
        chosen
    }

    fn contents(&self, ui: &mut Ui) -> Option<usize> {
        ui.set_width(self.width);
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
        egui::Frame::NONE
            .inner_margin(Margin {
                left: PAD_X,
                right: PAD_X,
                top: 24,
                bottom: 20,
            })
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(BADGE), Sense::hover());
                    let (soft, strong) = self.tone.colors();
                    ui.painter().circle_filled(rect.center(), BADGE / 2.0, soft);
                    ui.painter().text(
                        rect.center() + Vec2::new(0.0, -0.5),
                        egui::Align2::CENTER_CENTER,
                        self.tone.glyph(),
                        theme::strong(ui.ctx(), 19.0),
                        strong,
                    );
                    ui.add_space(8.0);
                    ui.vertical(|ui| {
                        ui.set_width(ui.available_width());
                        ui.add_space(1.0);
                        ui.label(
                            egui::RichText::new(&self.title)
                                .font(theme::strong(ui.ctx(), 16.0))
                                .color(theme::TEXT),
                        );
                        if !self.message.is_empty() {
                            ui.add_space(6.0);
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&self.message)
                                        .font(FontId::proportional(theme::UI_SIZE))
                                        .color(theme::TEXT_MUTED),
                                )
                                .wrap(),
                            );
                        }
                        if !self.items.is_empty() {
                            ui.add_space(14.0);
                            self.item_list(ui);
                        }
                    });
                });
            });
        self.footer(ui)
    }

    fn item_list(&self, ui: &mut Ui) {
        egui::Frame::NONE
            .fill(theme::SURFACE_SUBTLE)
            .stroke(Stroke::new(1.0, theme::LINE))
            .corner_radius(theme::RADIUS_LG)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                for (i, item) in self.items.iter().enumerate() {
                    if i > 0 {
                        let y = ui.cursor().top();
                        let r = ui.max_rect();
                        ui.painter().hline(
                            r.left() + 12.0..=r.right() - 12.0,
                            y,
                            Stroke::new(1.0, theme::LINE),
                        );
                    }
                    egui::Frame::NONE
                        .inner_margin(Margin::symmetric(12, 9))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                controls::chip(ui, &item.badge, theme::ACCENT_SOFT, theme::ACCENT);
                                ui.add_space(2.0);
                                ui.vertical(|ui| {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&item.title)
                                                .color(theme::TEXT)
                                                .font(theme::strong(ui.ctx(), theme::UI_SIZE)),
                                        )
                                        .truncate(),
                                    );
                                    if !item.detail.is_empty() {
                                        ui.add_space(2.0);
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(&item.detail)
                                                    .size(11.5)
                                                    .color(theme::TEXT_FAINT),
                                            )
                                            .truncate(),
                                        )
                                        .on_hover_text(&item.detail);
                                    }
                                });
                            });
                        });
                }
            });
    }

    fn footer(&self, ui: &mut Ui) -> Option<usize> {
        let mut chosen = None;
        let radius = CornerRadius {
            nw: 0,
            ne: 0,
            sw: theme::RADIUS_XL,
            se: theme::RADIUS_XL,
        };
        let top = ui.cursor().top();
        let r = ui.max_rect();
        ui.painter()
            .hline(r.x_range(), top, Stroke::new(1.0, theme::LINE));
        egui::Frame::NONE
            .fill(theme::SURFACE_SUBTLE)
            .corner_radius(radius)
            .inner_margin(Margin::symmetric(PAD_X - 4, 14))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    for (i, a) in self.actions.iter().enumerate() {
                        if a.role == Role::Destructive
                            && controls::button(ui, &a.label, a.role, true).clicked()
                        {
                            chosen = Some(i);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        for (i, a) in self.actions.iter().enumerate().rev() {
                            if a.role != Role::Destructive
                                && controls::button(ui, &a.label, a.role, true).clicked()
                            {
                                chosen = Some(i);
                            }
                        }
                    });
                });
            });
        chosen
    }
}

/// File name (last component) for display, with the full path as detail.
pub fn file_item(badge: &str, path: &std::path::Path) -> Item {
    Item {
        badge: badge.into(),
        title: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string()),
        detail: path.display().to_string(),
    }
}

impl Modal {
    pub fn items(mut self, items: impl IntoIterator<Item = Item>) -> Self {
        self.items.extend(items);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &egui::Context, events: Vec<egui::Event>) -> Option<usize> {
        let mut out = None;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(900.0, 700.0),
            )),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            out = Modal::new("t", Tone::Warning, "Save changes?")
                .message("Edits will be lost.")
                .item("LEFT", "a.txt", "C:/a.txt")
                .action("Don't Save", Role::Destructive)
                .action("Cancel", Role::Secondary)
                .action("Save", Role::Primary)
                .cancel(1)
                .show(ui.ctx());
        });
        output.textures_delta.clear();
        out
    }
    fn key(k: egui::Key) -> egui::Event {
        egui::Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }
    }

    fn click(at: egui::Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(at),
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ]
    }

    #[test]
    fn footer_buttons_respond_to_pointer_clicks() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        frame(&ctx, vec![]);
        frame(&ctx, vec![]);
        let layer = egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::unique(("matcha-modal", "t")),
        );
        let rect = ctx.memory(|m| m.area_rect(layer.id)).expect("modal area");
        let y = rect.bottom() - 14.0 - controls::BUTTON_HEIGHT / 2.0;
        // Primary sits bottom-right; the destructive action bottom-left.
        let save = egui::pos2(rect.right() - 20.0 - 20.0, y);
        assert_eq!(frame(&ctx, click(save)), Some(2));
        let dont = egui::pos2(rect.left() + 20.0 + 20.0, y);
        assert_eq!(frame(&ctx, click(dont)), Some(0));
    }

    #[test]
    fn enter_picks_primary_and_escape_picks_cancel() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        assert_eq!(frame(&ctx, vec![]), None);
        assert_eq!(frame(&ctx, vec![]), None);
        assert_eq!(frame(&ctx, vec![key(egui::Key::Enter)]), Some(2));
        assert_eq!(frame(&ctx, vec![key(egui::Key::Escape)]), Some(1));
    }
}
