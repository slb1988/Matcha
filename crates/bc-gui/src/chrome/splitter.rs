//! Draggable splitters between panes, persisted through [`crate::layout`].
//!
//! Views compute pane rectangles from [`Split::first`] at the start of the frame, draw the
//! panes, then call [`handle`] last so the narrow hit strip sits above the pane widgets.
//! A drag updates the stored value (applied next frame), releasing writes the layout file,
//! double-clicking restores the default. See `docs/features/resizable-splitters.md`.

use eframe::egui::{self, CursorIcon, Pos2, Rangef, Rect, Sense};

/// Half width of the hit strip around a splitter line.
pub const GRAB: f32 = 3.0;

/// How the stored value maps to a position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Unit {
    /// Fraction (0–1) of the span measured from its start.
    Ratio,
    /// Pixels from the start of the span (e.g. a sidebar width).
    Pixels,
    /// Pixels from the end of the span (e.g. a bottom panel height).
    PixelsFromEnd,
}

/// One persisted split: layout key, default value and minimum side sizes.
#[derive(Clone, Copy, Debug)]
pub struct Split {
    pub key: &'static str,
    pub default: f32,
    pub unit: Unit,
    /// Minimum size of the start / end side in pixels.
    pub min: [f32; 2],
}

impl Split {
    pub const fn ratio(key: &'static str, default: f32) -> Self {
        Split {
            key,
            default,
            unit: Unit::Ratio,
            min: [80.0, 80.0],
        }
    }
    pub const fn pixels(key: &'static str, default: f32) -> Self {
        Split {
            key,
            default,
            unit: Unit::Pixels,
            min: [80.0, 80.0],
        }
    }
    pub const fn pixels_from_end(key: &'static str, default: f32) -> Self {
        Split {
            key,
            default,
            unit: Unit::PixelsFromEnd,
            min: [80.0, 80.0],
        }
    }
    /// Minimum sizes of the start and end sides.
    pub const fn min(mut self, start: f32, end: f32) -> Self {
        self.min = [start, end];
        self
    }
    fn value(&self) -> f32 {
        crate::layout::get(self.key).unwrap_or(self.default)
    }
    fn clamp(&self, first: f32, len: f32) -> f32 {
        let [lo, hi] = self.min;
        if len <= lo + hi {
            // Too small for both minimums: share proportionally.
            return (len * lo / (lo + hi).max(1.0)).max(0.0);
        }
        first.clamp(lo, len - hi)
    }
    fn unclamped_first(&self, value: f32, len: f32) -> f32 {
        match self.unit {
            Unit::Ratio => value * len,
            Unit::Pixels => value,
            Unit::PixelsFromEnd => len - value,
        }
    }
    /// Size of the start side for a span of `len` pixels (stored value, clamped).
    pub fn first(&self, len: f32) -> f32 {
        self.clamp(self.unclamped_first(self.value(), len), len)
    }
    /// Size of the end side for a span of `len` pixels.
    pub fn last(&self, len: f32) -> f32 {
        len - self.first(len)
    }
    /// Default start-side size, ignoring the stored value.
    pub fn default_first(&self, len: f32) -> f32 {
        self.clamp(self.unclamped_first(self.default, len), len)
    }
    /// Store the split so that the start side is `first` pixels of `len`.
    pub fn store(&self, first: f32, len: f32) {
        let first = self.clamp(first, len);
        let value = match self.unit {
            Unit::Ratio => first / len.max(1.0),
            Unit::Pixels => first,
            Unit::PixelsFromEnd => len - first,
        };
        crate::layout::set(self.key, value);
    }
    /// Forget the stored value.
    pub fn reset(&self) {
        crate::layout::remove(self.key);
    }
}

/// Hit strip for a vertical line at `x` (panes left / right; drag moves along x).
pub fn hit_x(x: f32, y: Rangef) -> Rect {
    Rect::from_x_y_ranges(Rangef::new(x - GRAB, x + GRAB), y)
}

/// Hit strip for a horizontal line at `y` (panes above / below; drag moves along y).
pub fn hit_y(y: f32, x: Rangef) -> Rect {
    Rect::from_x_y_ranges(x, Rangef::new(y - GRAB, y + GRAB))
}

/// Interact with one splitter strip. `hit` straddles the line; `axis` is the direction the
/// line moves (`Horizontal` = vertical line between left / right panes); `span` is the
/// pixel range the split divides along that axis. Returns the drag response.
pub fn handle(
    ui: &mut egui::Ui,
    split: &Split,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    hit: Rect,
    axis: egui::Direction,
    span: Rangef,
) -> egui::Response {
    let along_x = matches!(
        axis,
        egui::Direction::LeftToRight | egui::Direction::RightToLeft
    );
    let along = |p: Pos2| if along_x { p.x } else { p.y };
    let id = egui::Id::unique(("splitter", split.key)).with(id_salt);
    let response = ui.interact(hit, id, Sense::click_and_drag());
    let line = if along_x {
        hit.center().x
    } else {
        hit.center().y
    };
    if response.hovered() || response.dragged() {
        ui.ctx().set_cursor_icon(if along_x {
            CursorIcon::ResizeHorizontal
        } else {
            CursorIcon::ResizeVertical
        });
        let mark = if along_x {
            Rect::from_x_y_ranges(Rangef::new(line - 1.0, line + 1.0), hit.y_range())
        } else {
            Rect::from_x_y_ranges(hit.x_range(), Rangef::new(line - 1.0, line + 1.0))
        };
        ui.painter()
            .rect_filled(mark, 0.0, crate::chrome::theme::ACCENT_SOFT_LINE);
    }
    // Drag is relative to the split at grab time, so a line drawn inside a gap (offset from
    // the stored position) does not jump when grabbed.
    if response.drag_started() {
        // egui reports the start after the drag threshold; measure from the press point.
        let origin = ui.input(|i| i.pointer.press_origin());
        if let Some(p) = origin.or(response.interact_pointer_pos()) {
            let start = (split.first(span.span()), along(p));
            ui.data_mut(|d| d.insert_temp(id, start));
        }
    }
    if response.dragged() {
        if let (Some(p), Some((first, origin))) = (
            response.interact_pointer_pos(),
            ui.data(|d| d.get_temp::<(f32, f32)>(id)),
        ) {
            split.store(first + along(p) - origin, span.span());
            ui.ctx().request_repaint();
        }
    }
    if response.drag_stopped() {
        crate::layout::save();
    }
    if response.double_clicked() {
        split.reset();
        crate::layout::save();
        ui.ctx().request_repaint();
    }
    response
}

/// Vertical splitter at `x` spanning `y` that divides `span` (left / right panes).
pub fn vertical_line(
    ui: &mut egui::Ui,
    split: &Split,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    x: f32,
    y: Rangef,
    span: Rangef,
) -> egui::Response {
    handle(
        ui,
        split,
        id_salt,
        hit_x(x, y),
        egui::Direction::LeftToRight,
        span,
    )
}

/// Horizontal splitter at `y` spanning `x` that divides `span` (top / bottom panes).
pub fn horizontal_line(
    ui: &mut egui::Ui,
    split: &Split,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    y: f32,
    x: Rangef,
    span: Rangef,
) -> egui::Response {
    handle(
        ui,
        split,
        id_salt,
        hit_y(y, x),
        egui::Direction::TopDown,
        span,
    )
}

/// Three columns separated by 1 px lines (merge input panes). `splits` place the two
/// boundaries as fractions of the width; each is clamped against the other so columns keep
/// their minimum width and the dividers never cross.
#[derive(Clone, Copy, Debug)]
pub struct Columns3 {
    pub area: Rect,
    /// Column rectangles left to right.
    pub rects: [Rect; 3],
    /// The two splits with their relational minimums applied (use for [`Columns3::handles`]).
    pub splits: [Split; 2],
}

impl Columns3 {
    pub fn new(area: Rect, splits: [Split; 2]) -> Self {
        let w = (area.width() - 2.0).max(0.0);
        let [a, b] = splits;
        let min = a.min[0].max(1.0);
        // b first leaves room for two columns before it, then a against b, then b against a.
        let b_raw = b.min(2.0 * min, min).first(w);
        let a = a.min(min, w - b_raw + min);
        let c1 = a.first(w);
        let b = b.min(c1 + min, min);
        let c2 = b.first(w);
        let x = area.left();
        let col = |l: f32, r: f32| Rect::from_x_y_ranges(Rangef::new(x + l, x + r), area.y_range());
        Columns3 {
            area,
            rects: [
                col(0.0, c1),
                col(c1 + 1.0, c2 + 1.0),
                col(c2 + 2.0, area.width()),
            ],
            splits: [a, b],
        }
    }
    /// The two column dividers (call after drawing the panes).
    pub fn handles(
        &self,
        ui: &mut egui::Ui,
        id_salt: impl std::hash::Hash + std::fmt::Debug + Copy,
    ) {
        let span = Rangef::new(self.area.left(), self.area.right() - 2.0);
        for (i, split) in self.splits.iter().enumerate() {
            let x = self.rects[i].right() + 0.5;
            vertical_line(ui, split, (id_salt, i), x, self.area.y_range(), span);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(p: Pos2) -> egui::RawInput {
        egui::RawInput {
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
        }
    }
    fn moved(p: Pos2) -> egui::RawInput {
        egui::RawInput {
            events: vec![egui::Event::PointerMoved(p)],
            ..Default::default()
        }
    }
    fn release(p: Pos2) -> egui::RawInput {
        egui::RawInput {
            events: vec![egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
            ..Default::default()
        }
    }

    const SPLIT: Split = Split::ratio("splitter.test", 0.5);

    /// Draw a 400 px wide span starting at x=0 with the split line at its stored position.
    fn frame(ctx: &egui::Context, input: egui::RawInput) -> egui::Response {
        let mut out = None;
        let _ = crate::chrome::test_frame(ctx, input, |ui| {
            let span = Rangef::new(0.0, 400.0);
            let x = span.min + SPLIT.first(span.span());
            out = Some(vertical_line(
                ui,
                &SPLIT,
                0,
                x,
                Rangef::new(0.0, 100.0),
                span,
            ));
        });
        out.unwrap()
    }

    #[test]
    fn drag_moves_split_and_double_click_resets() {
        SPLIT.reset();
        let ctx = egui::Context::default();
        assert_eq!(SPLIT.first(400.0), 200.0);
        let _ = frame(&ctx, Default::default());
        // Grab 2 px right of the line: the split moves by the pointer delta, no jump.
        let _ = frame(&ctx, press(egui::pos2(202.0, 50.0)));
        let _ = frame(&ctx, moved(egui::pos2(232.0, 50.0)));
        let r = frame(&ctx, moved(egui::pos2(262.0, 50.0)));
        assert!(r.dragged());
        assert!(
            (SPLIT.first(400.0) - 260.0).abs() < 0.01,
            "{}",
            SPLIT.first(400.0)
        );
        let _ = frame(&ctx, release(egui::pos2(262.0, 50.0)));
        // Ratio scales with the span.
        assert!((SPLIT.first(800.0) - 520.0).abs() < 0.01);
        // Far past the end: clamped to the minimum of the other side.
        let _ = frame(&ctx, press(egui::pos2(260.0, 50.0)));
        let _ = frame(&ctx, moved(egui::pos2(300.0, 50.0)));
        let _ = frame(&ctx, moved(egui::pos2(900.0, 50.0)));
        assert_eq!(SPLIT.first(400.0), 320.0);
        let _ = frame(&ctx, release(egui::pos2(900.0, 50.0)));
        // Double-click on the line restores the default.
        let line = egui::pos2(320.0, 50.0);
        for _ in 0..2 {
            let _ = frame(&ctx, press(line));
            let _ = frame(&ctx, release(line));
        }
        assert_eq!(SPLIT.first(400.0), 200.0);
    }

    #[test]
    fn three_columns_default_to_thirds_and_never_cross() {
        let a = Split::ratio("splitter.test.col.a", 1.0 / 3.0);
        let b = Split::ratio("splitter.test.col.b", 2.0 / 3.0);
        a.reset();
        b.reset();
        let area = Rect::from_min_size(egui::pos2(10.0, 0.0), egui::vec2(902.0, 100.0));
        let c = Columns3::new(area, [a, b]);
        for r in c.rects {
            assert!((r.width() - 300.0).abs() < 0.01, "{r:?}");
        }
        assert_eq!(c.rects[2].right(), area.right());
        // Push a past b: it stops one minimum column short of b.
        a.store(880.0, 900.0);
        let c = Columns3::new(area, [a, b]);
        assert!((c.rects[1].width() - 80.0).abs() < 0.01, "{:?}", c.rects);
        assert!(c.rects[0].width() > 80.0 && c.rects[2].width() >= 80.0);
    }

    #[test]
    fn pixel_splits_clamp_to_minimums() {
        let panel = Split::pixels_from_end("splitter.test.panel", 106.0).min(80.0, 40.0);
        panel.reset();
        assert_eq!(panel.last(600.0), 106.0);
        panel.store(590.0, 600.0);
        assert_eq!(panel.last(600.0), 40.0);
        panel.store(10.0, 600.0);
        assert_eq!(panel.first(600.0), 80.0);
        // Window shrinks below both minimums: shared proportionally, never negative.
        assert!((panel.first(60.0) - 40.0).abs() < 0.01);
        let tree = Split::pixels("splitter.test.tree", 275.0);
        tree.reset();
        assert_eq!(tree.first(1000.0), 275.0);
        assert_eq!(tree.default_first(1000.0), 275.0);
    }
}
