use super::cache::projected;
use crate::text::projection::Projection;
use bc_core::DiffKind;
use bc_engine_table::{compare::Importance, map::TableMapRow, TableMap};
use bc_session::prefs::DiffColors;
use eframe::egui;
use std::ops::Range;
pub const ROW_HEIGHT: f32 = 22.0;
pub const GUTTER: f32 = 62.0;
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cursor {
    pub row: usize,
    pub column: usize,
}
pub struct GridState {
    pub scroll: egui::Vec2,
    pub cursor: Cursor,
    pub anchor: Option<Cursor>,
    pub active: usize,
    pub stacked: bool,
    pub columns: Vec<usize>,
    pub offsets: Vec<f32>,
    pub widths: Vec<f32>,
    pub row_numbers: bool,
    pub whitespace: bool,
    pub pane_rects: [egui::Rect; 2],
    pub visible: Range<usize>,
    pub rows_drawn: usize,
    pub cells_drawn: usize,
    pub edit_target: Option<(usize, Cursor)>,
    pub edit_rect: Option<egui::Rect>,
}
impl Default for GridState {
    fn default() -> Self {
        Self {
            scroll: egui::Vec2::ZERO,
            cursor: Cursor::default(),
            anchor: None,
            active: 0,
            stacked: false,
            columns: Vec::new(),
            offsets: vec![0.0],
            widths: Vec::new(),
            row_numbers: true,
            whitespace: false,
            pane_rects: [egui::Rect::NOTHING; 2],
            visible: 0..0,
            rows_drawn: 0,
            cells_drawn: 0,
            edit_target: None,
            edit_rect: None,
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub enum GridAction {
    Select(usize, Cursor, bool),
    Edit(usize, Cursor),
}
pub fn color(
    colors: &DiffColors,
    importance: Importance,
    orphan: bool,
    alpha: u8,
) -> egui::Color32 {
    let [r, g, b] = if orphan {
        colors.inserted
    } else if importance == Importance::Unimportant {
        colors.unimportant
    } else {
        colors.important
    };
    egui::Color32::from_rgba_unmultiplied(r, g, b, alpha)
}
impl GridState {
    pub fn rebuild_columns(&mut self, count: usize, different: &[bool], hide: bool) {
        self.columns = (0..count)
            .filter(|&c| !hide || different.get(c) == Some(&true))
            .collect();
        self.offsets = vec![0.0];
        for &c in &self.columns {
            self.offsets
                .push(self.offsets.last().unwrap() + self.widths.get(c).copied().unwrap_or(130.0));
        }
    }
    pub fn selected(&self, row: usize, col: usize) -> bool {
        let a = self.anchor.unwrap_or(self.cursor);
        (a.row.min(self.cursor.row)..=a.row.max(self.cursor.row)).contains(&row)
            && (a.column.min(self.cursor.column)..=a.column.max(self.cursor.column)).contains(&col)
    }
}
pub fn mapped(map: &TableMap, row: usize) -> TableMapRow {
    if !map.is_stale() {
        if let Some(row) = map.row(row) {
            return row;
        }
    }
    TableMapRow {
        left: (row < map.left.borrow().row_count()).then_some(row),
        right: (row < map.right.borrow().row_count()).then_some(row),
        kind: DiffKind::Same,
        importance: Importance::Same,
        edited: false,
    }
}
/// Wheel/trackpad deltas over the grid panes, applied immediately. egui
/// 0.37 low-passes wheel deltas through `WheelState` (exponential bleed,
/// ~90% in 0.1s plus an asymptotic tail; Line/Page notches always take the
/// backlog path), so fast wheel scrolling keeps gliding after the input
/// stops; the grid ScrollAreas run with `mouse_wheel: false` and consume the
/// raw deltas here instead, 1:1 in the frame they arrive. Conversion and
/// modifiers use egui's native defaults (nothing in this workspace overrides
/// `InputOptions`, which has no public getter): 40pt per line, Page =
/// viewport height, Shift = horizontal, Alt = vertical, Ctrl = zoom (left to
/// egui, never scrolls).
pub fn wheel(ctx: &egui::Context, state: &mut GridState, enabled: bool) {
    if !enabled {
        return;
    }
    let Some(pointer) = ctx.input(|i| i.pointer.hover_pos()) else {
        return;
    };
    if !state.pane_rects.iter().any(|r| r.contains(pointer)) {
        return;
    }
    let (events, page) = ctx.input(|i| (i.events.clone(), i.viewport_rect().height()));
    // egui 0.37 native `InputOptions` defaults.
    const LINE_SPEED: f32 = 40.0;
    let zoom = egui::Modifiers::COMMAND;
    let horizontal = egui::Modifiers::SHIFT;
    let vertical = egui::Modifiers::ALT;
    let mut delta = egui::Vec2::ZERO;
    for event in events {
        if let egui::Event::MouseWheel {
            unit,
            delta: d,
            modifiers,
            ..
        } = event
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
    if delta == egui::Vec2::ZERO {
        return;
    }
    // Offset space is inverted from wheel deltas (same as egui's ScrollArea):
    // wheel up (positive y) decreases the scroll offset.
    state.scroll.x = (state.scroll.x - delta.x).max(0.0);
    state.scroll.y = (state.scroll.y - delta.y).max(0.0);
    // Upper-bound clamping happens in `pane` later this same frame.
}
pub fn pane(
    ui: &mut egui::Ui,
    map: &TableMap,
    projection: &Projection,
    state: &mut GridState,
    side: usize,
    height: f32,
    colors: &DiffColors,
) -> Option<GridAction> {
    let height = (height - 25.0).max(35.0);
    state.scroll.y = state
        .scroll
        .y
        .min((projection.len() as f32 * ROW_HEIGHT - height + 16.0).max(0.0));
    let width = GUTTER + state.offsets.last().copied().unwrap_or(0.0);
    state.scroll.x = state
        .scroll
        .x
        .min((width - ui.available_width() + 16.0).max(0.0));
    super::header::ui(ui, map, state, side);
    let scroll = state.scroll;
    let mut action = None;
    state.pane_rects[side] =
        egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), height));
    let focus_id = egui::Id::unique(("table_grid_focus", side));
    // Arrow keys and Tab drive the grid cursor (see `commands::keyboard`):
    // lock them onto the pane, otherwise egui's focus navigation moves focus
    // away on the first arrow press and key navigation freezes.
    ui.memory_mut(|m| {
        m.set_focus_lock_filter(
            focus_id,
            egui::EventFilter {
                tab: true,
                horizontal_arrows: true,
                vertical_arrows: true,
                escape: false,
            },
        )
    });
    let pane_response = ui.interact(
        state.pane_rects[side],
        focus_id,
        egui::Sense::click_and_drag(),
    );
    ui.spacing_mut().item_spacing.y = 0.0;
    let output = egui::ScrollArea::both()
        .id_salt(("table_grid", side))
        // Wheel deltas are consumed by `grid::wheel` (immediate, unsmoothed);
        // scrollbars and touch drag keep the framework behavior.
        .scroll_source(egui::containers::scroll_area::ScrollSource {
            scroll_bar: true,
            drag: egui::containers::scroll_area::DragScroll::OnTouch,
            mouse_wheel: false,
        })
        .auto_shrink([false, false])
        .max_height(height)
        .vertical_scroll_offset(scroll.y)
        .horizontal_scroll_offset(scroll.x)
        .show_rows(ui, ROW_HEIGHT, projection.len(), |ui, range| {
            let rules = map.rules();
            let sheet = if side == 0 {
                map.left.borrow()
            } else {
                map.right.borrow()
            };
            state.visible = projected(projection, range.start).unwrap_or(0)
                ..projected(projection, range.end.saturating_sub(1)).map_or(0, |r| r + 1);
            for display in range {
                let Some(row) = projected(projection, display) else {
                    continue;
                };
                let pair = mapped(map, row);
                let source = if side == 0 { pair.left } else { pair.right };
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(width.max(ui.available_width()), ROW_HEIGHT),
                    egui::Sense::hover(),
                );
                let clip = ui.clip_rect();
                let gutter = egui::Rect::from_min_size(
                    egui::pos2(clip.left(), rect.top()),
                    egui::vec2(GUTTER, ROW_HEIGHT),
                );
                let start = state
                    .offsets
                    .partition_point(|&x| x < scroll.x)
                    .saturating_sub(1);
                for d in start..state.columns.len() {
                    let c = state.columns[d];
                    let x = rect.left() + GUTTER + state.offsets[d];
                    if x > clip.right() {
                        break;
                    }
                    let cell_rect = egui::Rect::from_min_size(
                        egui::pos2(x, rect.top()),
                        egui::vec2(state.offsets[d + 1] - state.offsets[d], ROW_HEIGHT),
                    );
                    if cell_rect.right() < gutter.right() {
                        continue;
                    }
                    let paint_rect = cell_rect
                        .intersect(clip)
                        .intersect(egui::Rect::from_min_max(
                            egui::pos2(gutter.right(), clip.top()),
                            clip.max,
                        ));
                    if state.edit_target == Some((side, Cursor { row, column: c })) {
                        state.edit_rect = Some(paint_rect);
                    }
                    if !paint_rect.is_positive() {
                        continue;
                    }
                    let imp = if map.is_stale() {
                        Importance::Same
                    } else {
                        map.cell_importance(row, c).unwrap_or(Importance::Same)
                    };
                    if imp != Importance::Same {
                        ui.painter().rect_filled(
                            paint_rect,
                            0.0,
                            color(colors, imp, source.is_none(), 42),
                        );
                    }
                    if state.active == side && state.selected(row, c) {
                        ui.painter().rect_filled(
                            paint_rect,
                            0.0,
                            ui.visuals().selection.bg_fill.gamma_multiply(0.45),
                        );
                    }
                    let col = if side == 0 {
                        rules.columns.0.get(c).and_then(|c| c.left)
                    } else {
                        rules.columns.0.get(c).and_then(|c| c.right)
                    };
                    let start_char = ((paint_rect.left() - cell_rect.left() - 5.0).max(0.0) / 7.8)
                        .floor() as usize;
                    let mut text = source
                        .and_then(|r| col.and_then(|c| sheet.preview(r, c, start_char, 256)))
                        .unwrap_or_else(|| "·".into());
                    if state.whitespace {
                        text = text.replace(' ', "·").replace('\t', "→").replace('\n', "↵");
                    } else {
                        text = text.replace(['\n', '\r'], " ↵ ");
                    }
                    ui.painter().with_clip_rect(paint_rect).text(
                        cell_rect.left_center() + egui::vec2(5.0 + start_char as f32 * 7.8, 0.0),
                        egui::Align2::LEFT_CENTER,
                        text,
                        egui::FontId::monospace(13.0),
                        ui.visuals().text_color(),
                    );
                    ui.painter().line_segment(
                        [cell_rect.right_top(), cell_rect.right_bottom()],
                        egui::Stroke::new(0.5, ui.visuals().widgets.noninteractive.bg_stroke.color),
                    );
                    state.cells_drawn += 1;
                }
                ui.painter()
                    .rect_filled(gutter, 0.0, ui.visuals().faint_bg_color);
                if pair.importance != Importance::Same {
                    ui.painter().circle_filled(
                        gutter.left_center() + egui::vec2(6.0, 0.0),
                        3.0,
                        color(
                            colors,
                            pair.importance,
                            matches!(pair.kind, DiffKind::LeftOnly | DiffKind::RightOnly),
                            255,
                        ),
                    );
                }
                if state.row_numbers {
                    ui.painter().text(
                        gutter.right_center() - egui::vec2(5.0, 0.0),
                        egui::Align2::RIGHT_CENTER,
                        source.map_or("—".into(), |r| (r + 1).to_string()),
                        egui::FontId::monospace(11.0),
                        ui.visuals().weak_text_color(),
                    );
                }
                state.rows_drawn += 1;
            }
        });
    let offset = output.state.clamped_offset();
    if offset != scroll {
        state.scroll = offset;
    }
    if (pane_response.clicked() || pane_response.dragged()) && !state.columns.is_empty() {
        if let Some(pos) = pane_response
            .interact_pointer_pos()
            .filter(|p| state.pane_rects[side].contains(*p))
        {
            let display =
                ((pos.y - state.pane_rects[side].top() + scroll.y) / ROW_HEIGHT).max(0.0) as usize;
            if let Some(row) = projected(projection, display) {
                let x = (pos.x - state.pane_rects[side].left() + scroll.x - GUTTER).max(0.0);
                let c = state
                    .offsets
                    .partition_point(|&v| v <= x)
                    .saturating_sub(1)
                    .min(state.columns.len() - 1);
                ui.memory_mut(|m| m.request_focus(focus_id));
                let cursor = Cursor {
                    row,
                    column: state.columns[c],
                };
                action = Some(if pane_response.double_clicked() {
                    GridAction::Edit(side, cursor)
                } else {
                    GridAction::Select(
                        side,
                        cursor,
                        ui.input(|i| i.modifiers.shift)
                            || (pane_response.dragged() && !pane_response.drag_started()),
                    )
                });
            }
        }
    }
    action
}
