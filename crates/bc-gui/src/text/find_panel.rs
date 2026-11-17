//! 查找状态独立于编辑焦点；搜索必须通过 TextMap 提供差异范围。
//!
//! Docked find bar under the toolbar, laid out like the reference
//! (`text-find-bar.png`): search glyph, focused search combo with an inner
//! history arrow, a history button, regex / case / whole-word / wrap toggles,
//! Prev and (default) Next buttons and, at the right edge, an options
//! dropdown. Options without a button of their own (replace row, search
//! backwards, differences only, close) live in that dropdown, drawn with the
//! shared popup menu; controls use the design-system tokens.
use crate::chrome::{
    menubar::{MenuItem, MenuItemKind},
    popup, theme,
};
use bc_engine_text::search::{FindHit, FindOptions};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

/// Bar height below the toolbar (reference y70..106; the path bars follow).
pub const BAR_HEIGHT: f32 = 37.0;
/// Extra height of the replace row when it is shown.
const REPLACE_HEIGHT: f32 = 30.0;
const HISTORY_LIMIT: usize = 12;

#[derive(Default)]
pub struct FindPanel {
    pub open: bool,
    pub options: FindOptions,
    pub replacement: String,
    pub last_hit: Option<FindHit>,
    /// Second row with the replacement field and Replace / Replace All.
    pub replace_open: bool,
    /// Recently searched patterns, newest first.
    history: Vec<String>,
    /// Open dropdown: (which, popup path).
    menu: Option<(Dropdown, Vec<usize>)>,
    /// Focus (and select) the search field on the next frame.
    focus_query: bool,
    /// The field was opened with its text selected but has not had focus
    /// yet (another widget took it): paint the selection ourselves.
    select_hint: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Dropdown {
    History,
    Options,
}
#[derive(Clone, Copy)]
pub enum FindAction {
    Next,
    Previous,
    Replace,
    All,
}
fn query_id() -> egui::Id {
    egui::Id::unique("find_query")
}
/// Find-bar push button in the shared control style: neutral outlined,
/// `on` = accent tint (toggles), `default` = filled accent (Next). Returns
/// the ink its label/glyph should use.
fn button(ui: &egui::Ui, painter: &egui::Painter, rect: Rect, default: bool, on: bool) -> Color32 {
    let p = theme::palette(ui.ctx());
    let hovered = ui.rect_contains_pointer(rect);
    let (fill, edge, ink) = if default {
        (
            if hovered { p.accent_hover } else { p.accent },
            None,
            Color32::WHITE,
        )
    } else if on {
        (p.accent_soft, Some(p.accent_soft_line), p.accent_pressed)
    } else {
        (
            if hovered { p.hover } else { p.surface },
            Some(p.line_strong),
            p.text,
        )
    };
    painter.rect_filled(rect, theme::RADIUS_MD, fill);
    if let Some(edge) = edge {
        painter.rect_stroke(
            rect,
            theme::RADIUS_MD,
            Stroke::new(1.0, edge),
            StrokeKind::Inside,
        );
    }
    ink
}
/// Stroke chevron (same as combo boxes elsewhere).
fn chevron(painter: &egui::Painter, c: Pos2, color: Color32) {
    painter.add(egui::Shape::line(
        vec![
            c + Vec2::new(-3.5, -1.5),
            c + Vec2::new(0.0, 2.0),
            c + Vec2::new(3.5, -1.5),
        ],
        Stroke::new(1.4, color),
    ));
}
impl FindPanel {
    /// Open the bar and focus its search field (Search > Find).
    pub fn show(&mut self) {
        self.open = true;
        self.focus_query = true;
        self.select_hint = true;
    }
    fn remember(&mut self) {
        let pattern = self.options.pattern.clone();
        if pattern.is_empty() {
            return;
        }
        self.history.retain(|p| *p != pattern);
        self.history.insert(0, pattern);
        self.history.truncate(HISTORY_LIMIT);
    }
    fn options_menu(&self) -> Vec<MenuItem> {
        let check = |label: &str, id: &str, checked: bool| MenuItem {
            kind: MenuItemKind::Check { checked },
            ..MenuItem::action(label, id)
        };
        vec![
            check("Replace", "replace", self.replace_open),
            check("Search Backwards", "backwards", self.options.backwards),
            check(
                "Differences Only",
                "differences",
                self.options.differences_only,
            ),
            MenuItem {
                kind: MenuItemKind::Separator,
                enabled: false,
                ..MenuItem::action("", "")
            },
            MenuItem {
                shortcut: Some("Esc".into()),
                ..MenuItem::action("Close Find Bar", "close")
            },
        ]
    }
    fn history_menu(&self) -> Vec<MenuItem> {
        if self.history.is_empty() {
            return vec![MenuItem {
                enabled: false,
                ..MenuItem::action("(no recent searches)", "")
            }];
        }
        self.history
            .iter()
            .enumerate()
            .map(|(i, p)| MenuItem::action(p.clone(), format!("history:{i}")))
            .collect()
    }
    fn menu_action(&mut self, id: &str) {
        match id {
            "replace" => self.replace_open = !self.replace_open,
            "backwards" => self.options.backwards = !self.options.backwards,
            "differences" => self.options.differences_only = !self.options.differences_only,
            "close" => self.open = false,
            _ => {
                if let Some(pattern) = id
                    .strip_prefix("history:")
                    .and_then(|i| i.parse::<usize>().ok())
                    .and_then(|i| self.history.get(i).cloned())
                {
                    self.options.pattern = pattern;
                }
            }
        }
    }
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<FindAction> {
        if !self.open {
            return None;
        }
        let before = self.options.clone();
        let mut action = None;
        let height = BAR_HEIGHT
            + if self.replace_open {
                REPLACE_HEIGHT
            } else {
                0.0
            };
        let (bar, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
        // Lay the controls out in a child so their absolute placement does not
        // move the parent's cursor (the path bars follow the full bar).
        let ui = &mut ui.new_child(egui::UiBuilder::new().max_rect(bar));
        let painter = ui.painter_at(bar);
        let at = |x: f32, y: f32| bar.min + Vec2::new(x, y);
        let span = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(at(x0, y0), at(x1, y1));
        let p = theme::palette(ui.ctx());
        painter.rect_filled(bar, 0.0, p.surface_chrome);
        painter.rect_filled(span(0.0, 0.0, bar.width(), 1.0), 0.0, p.line);
        // Search glyph: ring with a handle toward the lower right.
        let ring = at(15.5, 17.0);
        painter.circle_stroke(ring, 4.5, Stroke::new(1.6, p.text_muted));
        painter.line_segment(
            [ring + Vec2::new(3.2, 3.2), ring + Vec2::new(7.5, 7.5)],
            Stroke::new(2.0, p.text_muted),
        );
        // Search combo: rounded field, accent outline while focused.
        let combo = span(41.0, 8.0, 696.0, 33.0);
        let focused = ui.memory(|m| m.has_focus(query_id()));
        painter.rect_filled(combo, theme::RADIUS_MD, p.surface);
        painter.rect_stroke(
            combo,
            theme::RADIUS_MD,
            Stroke::new(1.0, if focused { p.accent } else { p.line_strong }),
            StrokeKind::Inside,
        );
        chevron(&painter, at(686.0, 20.5), p.text_muted);
        let field = span(49.0, 11.0, 676.0, 30.0);
        if focused {
            self.select_hint = false;
        }
        let hinted = self.select_hint && !self.options.pattern.is_empty();
        if hinted {
            let width = painter
                .layout_no_wrap(
                    self.options.pattern.clone(),
                    FontId::proportional(theme::UI_SIZE),
                    Color32::WHITE,
                )
                .size()
                .x;
            painter.rect_filled(
                Rect::from_min_size(at(50.0, 11.0), Vec2::new(width.ceil(), 17.0)),
                0.0,
                p.selection,
            );
        }
        let output = ui
            .scope_builder(egui::UiBuilder::new().max_rect(field), |ui| {
                egui::TextEdit::singleline(&mut self.options.pattern)
                    .id(query_id())
                    .text_color(p.text)
                    .frame(egui::Frame::NONE)
                    .margin(egui::Margin::symmetric(1, 1))
                    .font(FontId::proportional(theme::UI_SIZE))
                    .desired_width(field.width())
                    .show(ui)
            })
            .inner;
        if self.focus_query {
            self.focus_query = false;
            output.response.request_focus();
            let mut state = output.state.clone();
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(self.options.pattern.chars().count()),
                )));
            state.store(ui.ctx(), output.response.id);
        }
        if output.response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            action = Some(if ui.input(|i| i.modifiers.shift) {
                FindAction::Previous
            } else {
                FindAction::Next
            });
        }
        // Buttons share one row, y77..103 (27 px) in the reference.
        let row = |x0: f32, x1: f32| span(x0, 7.0, x1, 34.0);
        let id = egui::Id::unique("find_bar");
        let click = |ui: &egui::Ui, rect: Rect, key: &str| {
            ui.interact(rect, id.with(key), Sense::click()).clicked()
        };
        let history = row(702.0, 730.0);
        let ink = button(ui, &painter, history, false, false);
        chevron(&painter, history.center(), ink);
        let toggles: [(f32, &str, bool, bool); 4] = [
            (742.0, ".*", self.options.is_regexp, true),
            (772.0, "Aa", self.options.case_sensitive, true),
            (802.0, "ab", self.options.whole_words, true),
            // No wrap option in the search engine: shown, disabled.
            (832.0, "wrap", false, false),
        ];
        let mut toggled = [false; 4];
        for (i, (x, label, on, enabled)) in toggles.into_iter().enumerate() {
            let rect = row(x, x + 28.0);
            let ink = button(ui, &painter, rect, false, on && enabled);
            let ink = if enabled { ink } else { p.text_faint };
            let c = rect.center();
            match label {
                "ab" => {
                    // Whole word: "ab" between two bars.
                    painter.text(
                        c,
                        egui::Align2::CENTER_CENTER,
                        "ab",
                        FontId::proportional(11.0),
                        ink,
                    );
                    for dx in [-7.5, 7.5] {
                        painter.line_segment(
                            [c + Vec2::new(dx, -4.5), c + Vec2::new(dx, 4.5)],
                            Stroke::new(1.0, ink),
                        );
                    }
                }
                "wrap" => {
                    // Two opposed arrows.
                    for (y, dir) in [(-2.5, 1.0), (2.5, -1.0)] {
                        let a = c + Vec2::new(-5.0 * dir, y);
                        let b = c + Vec2::new(5.0 * dir, y);
                        painter.line_segment([a, b], Stroke::new(1.2, ink));
                        painter.line_segment(
                            [b, b + Vec2::new(-2.5 * dir, -2.0)],
                            Stroke::new(1.2, ink),
                        );
                    }
                }
                _ => {
                    painter.text(
                        c,
                        egui::Align2::CENTER_CENTER,
                        label,
                        FontId::proportional(theme::UI_SIZE),
                        ink,
                    );
                }
            }
            toggled[i] = enabled && click(ui, rect, label);
        }
        self.options.is_regexp ^= toggled[0];
        self.options.case_sensitive ^= toggled[1];
        self.options.whole_words ^= toggled[2];
        let prev = row(872.0, 916.0);
        let next = row(918.0, 964.0);
        for (rect, label, default) in [(prev, "Prev", false), (next, "Next", true)] {
            let ink = button(ui, &painter, rect, default, false);
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                label,
                FontId::proportional(theme::UI_SIZE),
                ink,
            );
        }
        if click(ui, prev, "prev") {
            action = Some(FindAction::Previous);
        }
        if click(ui, next, "next") {
            action = Some(FindAction::Next);
        }
        let expand = row(bar.width() - 35.0, bar.width() - 6.0);
        let ink = button(ui, &painter, expand, false, false);
        chevron(&painter, expand.center(), ink);
        // Dropdowns (history / options) in the classic popup style.
        if click(ui, history, "history") {
            self.menu = Some((Dropdown::History, vec![popup::NONE]));
        }
        if click(ui, expand, "expand") {
            self.menu = Some((Dropdown::Options, vec![popup::NONE]));
        }
        if let Some((which, mut path)) = self.menu.take() {
            let (items, anchor) = match which {
                Dropdown::History => (self.history_menu(), Pos2::new(combo.left(), combo.bottom())),
                Dropdown::Options => {
                    let items = self.options_menu();
                    (items, Pos2::new(expand.right() - 200.0, expand.bottom()))
                }
            };
            let (chosen, rects) = popup::cascade(
                ui.ctx(),
                id.with("dropdown"),
                anchor,
                &items,
                &mut path,
                false,
            );
            let away = ui.input(|i| {
                i.pointer.any_click()
                    && i.pointer.interact_pos().is_some_and(|p| {
                        !rects.iter().any(|r| r.contains(p))
                            && !history.contains(p)
                            && !expand.contains(p)
                    })
            });
            if let Some(chosen) = chosen {
                self.menu_action(&chosen);
            } else if !away
                && !ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
            {
                self.menu = Some((which, path));
            }
        } else if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.open = false;
        }
        if self.replace_open {
            let top = BAR_HEIGHT;
            painter.text(
                at(41.0, top + 13.0),
                egui::Align2::LEFT_CENTER,
                "Replace with:",
                FontId::proportional(theme::UI_SIZE),
                p.text,
            );
            let edit = span(130.0, top + 2.0, 696.0, top + 25.0);
            let editing = ui.memory(|m| m.has_focus(egui::Id::unique("find_replacement")));
            painter.rect_filled(edit, theme::RADIUS_MD, p.surface);
            painter.rect_stroke(
                edit,
                theme::RADIUS_MD,
                Stroke::new(1.0, if editing { p.accent } else { p.line_strong }),
                StrokeKind::Inside,
            );
            ui.scope_builder(
                egui::UiBuilder::new().max_rect(edit.shrink2(Vec2::new(7.0, 3.0))),
                |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.replacement)
                            .id(egui::Id::unique("find_replacement"))
                            .frame(egui::Frame::NONE)
                            .desired_width(edit.width() - 14.0),
                    );
                },
            );
            let replace = span(702.0, top + 1.0, 790.0, top + 26.0);
            let all = span(794.0, top + 1.0, 890.0, top + 26.0);
            for (rect, label) in [(replace, "Replace"), (all, "Replace All")] {
                let ink = button(ui, &painter, rect, false, false);
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(theme::UI_SIZE),
                    ink,
                );
            }
            if click(ui, replace, "replace") {
                action = Some(FindAction::Replace);
            }
            if click(ui, all, "replace-all") {
                action = Some(FindAction::All);
            }
        }
        if matches!(action, Some(FindAction::Next | FindAction::Previous)) {
            self.remember();
        }
        if before != self.options {
            self.last_hit = None;
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(
        ctx: &egui::Context,
        panel: &mut FindPanel,
        events: Vec<egui::Event>,
    ) -> Option<FindAction> {
        let mut action = None;
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1384.0, 300.0))),
                events,
                ..Default::default()
            },
            |ui| action = panel.ui(ui),
        )
        .drop_without_applying_deltas();
        action
    }
    fn click(pos: Pos2) -> Vec<egui::Event> {
        let press = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        vec![egui::Event::PointerMoved(pos), press(true), press(false)]
    }
    #[test]
    fn bar_buttons_toggles_and_dropdown_keep_find_behaviour() {
        let ctx = egui::Context::default();
        let mut panel = FindPanel::default();
        assert!(frame(&ctx, &mut panel, vec![]).is_none());
        panel.show();
        panel.options.pattern = "alpha".into();
        frame(&ctx, &mut panel, vec![]);
        assert!(matches!(
            frame(&ctx, &mut panel, click(Pos2::new(940.0, 20.0))),
            Some(FindAction::Next)
        ));
        assert!(matches!(
            frame(&ctx, &mut panel, click(Pos2::new(894.0, 20.0))),
            Some(FindAction::Previous)
        ));
        assert_eq!(panel.history, ["alpha"]);
        // Regex / case / whole word toggles; wrap is disabled.
        for x in [756.0, 786.0, 816.0, 846.0] {
            frame(&ctx, &mut panel, click(Pos2::new(x, 20.0)));
        }
        assert!(
            panel.options.is_regexp && panel.options.case_sensitive && panel.options.whole_words
        );
        // Options dropdown → Replace shows the replace row.
        frame(&ctx, &mut panel, click(Pos2::new(1363.0, 20.0)));
        frame(&ctx, &mut panel, vec![]);
        let replace_item = Pos2::new(1384.0 - 41.0 - 200.0 + 40.0, 34.0 + 2.0 + 10.0);
        frame(&ctx, &mut panel, click(replace_item));
        assert!(panel.replace_open);
        frame(&ctx, &mut panel, vec![]);
        assert!(matches!(
            frame(&ctx, &mut panel, click(Pos2::new(840.0, 50.0))),
            Some(FindAction::All)
        ));
        // Escape closes the bar.
        frame(
            &ctx,
            &mut panel,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert!(!panel.open);
    }
}
