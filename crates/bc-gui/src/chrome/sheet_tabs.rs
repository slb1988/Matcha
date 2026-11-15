//! Worksheet tab strip (table compare / table merge): underline tabs in the
//! design-system style — the active sheet in strong text over a 2 px accent bar,
//! others muted with a hover capsule. Each tab may carry a status dot (sheet
//! comparison state) and its own text color (e.g. unresolved merge conflicts).
//! Tabs that do not fit scroll horizontally with the wheel.
use crate::chrome::theme;
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};

/// Strip height including its top border.
pub const HEIGHT: f32 = 28.0;
const PAD: f32 = 10.0;
const DOT: f32 = 3.0;

/// What one tab shows.
#[derive(Clone, Debug, PartialEq)]
pub struct SheetTab {
    pub label: String,
    /// Status dot left of the label (`None` = no dot).
    pub dot: Option<Color32>,
    /// Text color override (`None` = normal / muted by selection).
    pub color: Option<Color32>,
    /// Struck through (e.g. a sheet left out of the merge output).
    pub strike: bool,
}

impl SheetTab {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            dot: None,
            color: None,
            strike: false,
        }
    }
}

/// Paint the strip into `rect`; returns the clicked tab.
pub fn show(ui: &mut egui::Ui, rect: Rect, tabs: &[SheetTab], active: usize) -> Option<usize> {
    let p = theme::palette(ui.ctx());
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    painter.rect_filled(rect, 0.0, theme::PANEL_BG);
    painter.hline(
        rect.x_range(),
        rect.top() + 0.5,
        Stroke::new(1.0, theme::BORDER),
    );
    let id = ui.scope_id().with("sheet-tabs");
    let mut offset = ui.data(|d| d.get_temp::<f32>(id)).unwrap_or(0.0);
    let response = ui.interact(rect, id, Sense::hover());
    if response.hovered() {
        let wheel = ui.input(|i| i.smooth_scroll_delta);
        offset -= wheel.x + wheel.y;
    }
    // Lay out first: widths decide the scroll range.
    let galleys: Vec<_> = tabs
        .iter()
        .enumerate()
        .map(|(i, tab)| {
            let on = i == active;
            let font = if on {
                theme::strong(ui.ctx(), theme::UI_SIZE - 1.0)
            } else {
                egui::FontId::proportional(theme::UI_SIZE - 1.0)
            };
            let ink = tab.color.unwrap_or(if on { p.text } else { p.text_muted });
            (painter.layout_no_wrap(tab.label.clone(), font, ink), ink)
        })
        .collect();
    let widths: Vec<f32> = galleys
        .iter()
        .zip(tabs)
        .map(|((g, _), tab)| {
            (g.size().x
                + 2.0 * PAD
                + if tab.dot.is_some() {
                    2.0 * DOT + 6.0
                } else {
                    0.0
                })
            .round()
        })
        .collect();
    let total: f32 = widths.iter().sum::<f32>() + 8.0;
    offset = offset.clamp(0.0, (total - rect.width()).max(0.0));
    ui.data_mut(|d| d.insert_temp(id, offset));
    let mut clicked = None;
    let mut x = rect.left() + 4.0 - offset;
    for (i, (((galley, ink), tab), width)) in galleys.into_iter().zip(tabs).zip(widths).enumerate()
    {
        let tab_rect = Rect::from_min_max(
            Pos2::new(x, rect.top() + 1.0),
            Pos2::new(x + width, rect.bottom()),
        );
        x += width;
        if tab_rect.right() < rect.left() || tab_rect.left() > rect.right() {
            continue;
        }
        let on = i == active;
        let hit = ui.interact(
            tab_rect.intersect(rect),
            id.with(("tab", i)),
            Sense::click(),
        );
        if !on && hit.hovered() {
            painter.rect_filled(
                tab_rect.shrink2(Vec2::new(1.0, 3.0)),
                theme::RADIUS_MD,
                p.hover,
            );
        }
        let mut text_x = tab_rect.left() + PAD;
        if let Some(dot) = tab.dot {
            painter.circle_filled(Pos2::new(text_x + DOT, tab_rect.center().y), DOT, dot);
            text_x += 2.0 * DOT + 6.0;
        }
        let pos = Pos2::new(text_x, tab_rect.center().y - galley.size().y / 2.0);
        let text_rect = Rect::from_min_size(pos, galley.size());
        painter.galley(pos, galley, ink);
        if tab.strike {
            painter.hline(
                text_rect.x_range(),
                text_rect.center().y,
                Stroke::new(1.0, ink),
            );
        }
        if on {
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(tab_rect.left() + 4.0, tab_rect.bottom() - 2.0),
                    Pos2::new(tab_rect.right() - 4.0, tab_rect.bottom()),
                ),
                1.0,
                p.accent,
            );
        }
        if hit.clicked() {
            clicked = Some(i);
        }
    }
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(ctx: &egui::Context, tabs: &[SheetTab], events: Vec<egui::Event>) -> Option<usize> {
        let mut clicked = None;
        super::super::test_frame(
            ctx,
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 100.0))),
                events,
                ..Default::default()
            },
            |ui| {
                clicked = show(
                    ui,
                    Rect::from_min_size(Pos2::new(0.0, 50.0), Vec2::new(400.0, HEIGHT)),
                    tabs,
                    0,
                );
            },
        );
        clicked
    }

    #[test]
    fn clicking_a_tab_reports_it() {
        let ctx = egui::Context::default();
        let tabs = [
            SheetTab::new("Stock"),
            SheetTab {
                dot: Some(Color32::RED),
                ..SheetTab::new("Prices")
            },
        ];
        run(&ctx, &tabs, vec![]);
        // "Stock" is ~40 px wide plus padding; the second tab starts after it.
        let pos = Pos2::new(120.0, 50.0 + HEIGHT / 2.0);
        let press = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        run(
            &ctx,
            &tabs,
            vec![egui::Event::PointerMoved(pos), press(true)],
        );
        assert_eq!(run(&ctx, &tabs, vec![press(false)]), Some(1));
    }
}
