//! Floating tool window in the Matcha design system — the replacement for
//! egui's stock `egui::Window` (gray title bar, centered title, collapse
//! arrow). Same card as `chrome::modal`: white surface, hairline border,
//! 14 px radius and the dialog shadow; a 44 px header with a strong
//! left-aligned title and a ghost × button, a hairline under it, and a padded
//! body. Non-blocking (no scrim): for confirmations use `chrome::modal`.
//!
//! ```ignore
//! Window::new("Go To").open(&mut self.goto_open).show(ctx, |ui| { … });
//! ```
//!
//! The window is movable by dragging its header (or any empty area), opens
//! centered on the viewport, and reads colors from [`theme::palette`].
use super::theme;
use eframe::egui::{self, Rect, Sense, Stroke, Ui, Vec2};

const HEADER_HEIGHT: f32 = 44.0;
const PAD_X: f32 = 16.0;
const CLOSE: f32 = 28.0;
const DEFAULT_MIN_WIDTH: f32 = 280.0;

#[must_use = "call `.show(ctx, ..)`"]
pub struct Window<'open> {
    title: String,
    id: egui::Id,
    open: Option<&'open mut bool>,
    min_width: f32,
    default_width: Option<f32>,
    default_size: Option<Vec2>,
    resizable: bool,
    scroll: bool,
    close_on_escape: bool,
}

impl<'open> Window<'open> {
    /// `title` also seeds the window id (use [`Window::id`] when two windows
    /// share a title).
    pub fn new(title: impl Into<String>) -> Self {
        let title = title.into();
        Self {
            id: egui::Id::unique(("matcha-window", title.clone())),
            title,
            open: None,
            min_width: DEFAULT_MIN_WIDTH,
            default_width: None,
            default_size: None,
            resizable: false,
            scroll: false,
            close_on_escape: true,
        }
    }
    pub fn id(mut self, id: egui::Id) -> Self {
        self.id = id;
        self
    }
    /// Show the × button; it (or Esc) sets `*open = false`. Nothing is shown
    /// while `*open` is false.
    pub fn open(mut self, open: &'open mut bool) -> Self {
        self.open = Some(open);
        self
    }
    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }
    pub fn default_width(mut self, width: f32) -> Self {
        self.default_width = Some(width);
        self
    }
    /// Initial size of a resizable window (implies [`Window::resizable`]).
    pub fn default_size(mut self, size: impl Into<Vec2>) -> Self {
        self.default_size = Some(size.into());
        self.resizable = true;
        self
    }
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }
    /// Scroll the body vertically when it is taller than the window.
    pub fn scroll(mut self, scroll: bool) -> Self {
        self.scroll = scroll;
        self
    }
    /// Esc closes the window (default on; only applies with [`Window::open`]).
    pub fn close_on_escape(mut self, close: bool) -> Self {
        self.close_on_escape = close;
        self
    }

    /// Show the window; `None` when it is closed.
    pub fn show<R>(self, ctx: &egui::Context, add: impl FnOnce(&mut Ui) -> R) -> Option<R> {
        if self.open.as_deref() == Some(&false) {
            return None;
        }
        let p = theme::palette(ctx);
        let frame = egui::Frame::NONE
            .fill(p.surface)
            .stroke(Stroke::new(1.0, p.line))
            .corner_radius(theme::RADIUS_XL)
            .shadow(theme::shadow_dialog());
        let mut window = egui::Window::new(self.title.clone())
            .id(self.id)
            .title_bar(false)
            .collapsible(false)
            .resizable(self.resizable)
            .frame(frame)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .constrain(true);
        if let Some(width) = self.default_width {
            window = window.default_width(width);
        }
        if let Some(size) = self.default_size {
            window = window.default_size(size);
        }
        let closable = self.open.is_some();
        let title = self.title;
        let min_width = self.min_width;
        let scroll = self.scroll;
        let mut close = false;
        let shown = window.show(ctx, |ui| {
            ui.set_min_width(min_width);
            let top = ui.cursor().top();
            ui.add_space(HEADER_HEIGHT);
            let body = egui::Frame::NONE
                .inner_margin(egui::Margin {
                    left: PAD_X as i8,
                    right: PAD_X as i8,
                    top: 12,
                    bottom: 16,
                })
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 6.0);
                    if scroll {
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, true])
                            .show(ui, add)
                            .inner
                    } else {
                        add(ui)
                    }
                })
                .inner;
            // Header painted after the body so it spans the final width
            // (allocating the full available width up front would stretch
            // auto-sized windows to the screen).
            let full = ui.min_rect();
            let header = Rect::from_min_max(
                egui::pos2(full.left(), top),
                egui::pos2(full.right(), top + HEADER_HEIGHT),
            );
            let painter = ui.painter();
            painter.text(
                egui::pos2(header.left() + PAD_X, header.center().y),
                egui::Align2::LEFT_CENTER,
                &title,
                theme::strong(ui.ctx(), theme::UI_SIZE + 1.0),
                p.text,
            );
            painter.hline(full.x_range(), header.bottom(), Stroke::new(1.0, p.line));
            if closable {
                let rect = Rect::from_center_size(
                    egui::pos2(header.right() - 10.0 - CLOSE / 2.0, header.center().y),
                    Vec2::splat(CLOSE),
                );
                let response = ui
                    .interact(
                        rect,
                        ui.scope_id().with("matcha-window-close"),
                        Sense::click(),
                    )
                    .on_hover_text("Close");
                if response.hovered() {
                    ui.painter().rect_filled(rect, theme::RADIUS_MD, p.hover);
                }
                let c = rect.center();
                let ink = if response.hovered() {
                    p.text
                } else {
                    p.text_muted
                };
                for d in [Vec2::new(4.5, 4.5), Vec2::new(4.5, -4.5)] {
                    ui.painter()
                        .line_segment([c - d, c + d], Stroke::new(1.4, ink));
                }
                close |= response.clicked();
            }
            body
        });
        let shown = shown?;
        if closable && self.close_on_escape {
            let focused_here =
                ctx.memory(|m| m.focused().is_none()) || shown.response.contains_pointer();
            if focused_here
                && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
            {
                close = true;
            }
        }
        if close {
            if let Some(open) = self.open {
                *open = false;
            }
        }
        shown.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &egui::Context, events: Vec<egui::Event>, open: &mut bool) -> Option<Rect> {
        let mut rect = None;
        crate::chrome::test_frame(
            ctx,
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(800.0, 600.0),
                )),
                events,
                ..Default::default()
            },
            |ui| {
                rect = Window::new("Go To").open(open).show(ui.ctx(), |ui| {
                    ui.label("Line number");
                    ui.min_rect()
                });
            },
        );
        rect
    }

    #[test]
    fn centered_card_with_close_button() {
        let ctx = egui::Context::default();
        let mut open = true;
        frame(&ctx, vec![], &mut open);
        let body = frame(&ctx, vec![], &mut open).expect("shown");
        // Centered horizontally on the 800 px viewport.
        assert!((body.center().x - 400.0).abs() < 160.0, "{body:?}");
        let area =
            ctx.memory(|m| m.area_rect(egui::Id::unique(("matcha-window", "Go To".to_owned()))));
        let area = area.expect("window area");
        assert!(area.width() >= DEFAULT_MIN_WIDTH);
        // Click × (10 px from the right edge, header center).
        let x = egui::pos2(
            area.right() - 10.0 - CLOSE / 2.0,
            area.top() + HEADER_HEIGHT / 2.0,
        );
        frame(
            &ctx,
            vec![
                egui::Event::PointerMoved(x),
                egui::Event::PointerButton {
                    pos: x,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
                egui::Event::PointerButton {
                    pos: x,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                },
            ],
            &mut open,
        );
        assert!(!open);
        assert!(frame(&ctx, vec![], &mut open).is_none());
    }

    #[test]
    fn escape_closes() {
        let ctx = egui::Context::default();
        let mut open = true;
        frame(&ctx, vec![], &mut open);
        frame(
            &ctx,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
            &mut open,
        );
        assert!(!open);
    }
}
