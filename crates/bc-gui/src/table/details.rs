//! Text Details strip (current row of both sides as a two-row mini grid) and
//! the 52 px thumbnail column.
//!
//! The strip keeps its own column width overrides and horizontal offset
//! (`GridState::details_*`) but routes every click through the grid's
//! [`GridAction`] channel, so selection, Modify, Align With and the context
//! menu behave exactly as in the panes above.
use super::grid::{self, Cursor, GridAction, GridState};
use bc_engine_table::{compare::Importance, TableMap};
use bc_session::prefs::DiffColors;
use eframe::egui;
/// Reference details area: 27 px header, one 18 px row per side, a 16 px
/// horizontal-scroll band (81 px total with the edge lines).
pub const HEIGHT: f32 = 81.0;
const HEADER: f32 = 27.0;
/// Cell text inset on each side (left/right aligned text).
const PAD: f32 = 6.0;
/// Hover tooltip text cap (characters).
const TOOLTIP_CHARS: usize = 4000;
/// Hit regions of the strip for a given rect (shared by painting, input and
/// tests): header, the two side rows, the scroll band, and the content left.
pub struct Layout {
    pub header: egui::Rect,
    pub rows: [egui::Rect; 2],
    pub band: egui::Rect,
    pub left: f32,
}
pub fn layout(rect: egui::Rect) -> Layout {
    let header = egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.top() + HEADER));
    let row = |side: usize| {
        egui::Rect::from_min_size(
            egui::pos2(
                rect.left(),
                header.bottom() + side as f32 * grid::ROW_HEIGHT,
            ),
            egui::vec2(rect.width(), grid::ROW_HEIGHT),
        )
    };
    Layout {
        header,
        rows: [row(0), row(1)],
        band: egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.bottom() - 17.0),
            egui::pos2(rect.right(), rect.bottom() - 1.0),
        ),
        left: rect.left() + 1.0,
    }
}
/// Display column under strip x (None past the last column).
fn column_at(state: &GridState, left: f32, x: f32) -> Option<usize> {
    let content = x - left + state.details_scroll_x;
    if content < 0.0 || content >= state.details_offsets.last().copied().unwrap_or(0.0) {
        return None;
    }
    Some(
        state
            .details_offsets
            .partition_point(|&v| v <= content)
            .saturating_sub(1)
            .min(state.columns.len().saturating_sub(1)),
    )
}
/// Raw text of one side's cell of comparison column `c` in comparison row
/// `row` (empty for a gap).
fn cell_text(map: &TableMap, row: usize, side: usize, c: usize) -> String {
    let pair = grid::mapped(map, row);
    let source = if side == 0 { pair.left } else { pair.right };
    let col = map
        .rules()
        .columns
        .0
        .get(c)
        .and_then(|c| if side == 0 { c.left } else { c.right });
    let sheet = if side == 0 {
        map.left.borrow()
    } else {
        map.right.borrow()
    };
    source
        .zip(col)
        .and_then(|(r, c)| sheet.display_at(r, c))
        .unwrap_or_default()
}
/// Line breaks folded for the single-line strip.
fn folded(text: &str) -> String {
    text.replace(['\n', '\r'], " ↵ ")
}
fn text_width(ui: &egui::Ui, text: String, font: &egui::FontId) -> f32 {
    ui.painter()
        .layout_no_wrap(text, font.clone(), egui::Color32::BLACK)
        .size()
        .x
}
/// Double-click autofit: widest of both sides' current-row text and the
/// column letter, plus padding (clamped by `details_set_width`).
fn fit_width(ui: &egui::Ui, map: &TableMap, row: usize, c: usize) -> f32 {
    let mono = egui::FontId::monospace(crate::chrome::theme::EDITOR_SIZE);
    let label = egui::FontId::proportional(crate::chrome::theme::UI_SIZE);
    let cells = (0..2)
        .map(|side| text_width(ui, folded(&cell_text(map, row, side, c)), &mono))
        .fold(0.0, f32::max);
    // The header letter sits after the 15 px change marker.
    let header = text_width(ui, grid::column_letter(c), &label) + 15.0;
    cells.max(header) + 2.0 * PAD
}
/// Truncated (text wider than the cell minus padding) or multi-line text.
fn needs_tooltip(ui: &egui::Ui, text: &str, width: f32, font: &egui::FontId) -> bool {
    !text.is_empty()
        && (text.contains(['\n', '\r']) || text_width(ui, folded(text), font) > width - 2.0 * PAD)
}
/// Full text for the hover tooltip, capped at `TOOLTIP_CHARS` with "…".
pub fn tooltip_text(text: &str) -> String {
    match text.char_indices().nth(TOOLTIP_CHARS) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text.to_owned(),
    }
}
/// The Text Details strip: paints the cursor row of both sides and handles
/// its mouse input. Returns the grid action to route (select / edit /
/// context) and the two side-row responses (context-menu owners).
pub fn ui(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    map: &TableMap,
    state: &mut GridState,
    different: &[bool],
    colors: &DiffColors,
) -> (Option<GridAction>, [egui::Response; 2]) {
    let lay = layout(rect);
    state.details_rect = rect;
    let view = (rect.right() - lay.left).max(0.0);
    state.rebuild_details_offsets();
    // Wheel over the strip scrolls it horizontally (either axis); the grid
    // wheel only hits the pane rects, so the panes never move.
    let delta = crate::chrome::wheel::delta(ui.ctx(), &[rect]);
    state.details_scroll_x -= delta.x + delta.y;
    state.details_follow_cursor(view);
    let mut action = None;
    let cursor = state.cursor;
    // Header: boundary drag/double-click resizes; elsewhere a click moves the
    // cursor to that column on the active side.
    let header = ui.interact(
        lay.header,
        egui::Id::unique("table_details_header"),
        egui::Sense::click_and_drag(),
    );
    if header.drag_started() {
        // Hit-test the press origin (interact_pointer_pos is already past it).
        state.details_resizing = ui
            .ctx()
            .input(|i| i.pointer.press_origin())
            .and_then(|p| state.details_resize_target(lay.left, p.x));
    }
    let boundary = ui
        .ctx()
        .pointer_hover_pos()
        .or_else(|| header.interact_pointer_pos())
        .filter(|p| lay.header.contains(*p))
        .and_then(|p| state.details_resize_target(lay.left, p.x));
    if header.dragged() {
        if let Some(d) = state.details_resizing {
            if let Some(pos) = header.interact_pointer_pos() {
                state.details_resize_column(d, pos.x - lay.left + state.details_scroll_x);
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
    } else if header.drag_stopped() {
        state.details_resizing = None;
    } else if header.hovered() && boundary.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    if header.double_clicked() {
        if let Some(d) = boundary {
            let width = fit_width(ui, map, cursor.row, state.columns[d]);
            state.details_set_width(d, width);
        }
    } else if header.clicked() && boundary.is_none() {
        if let Some(d) = header
            .interact_pointer_pos()
            .and_then(|p| column_at(state, lay.left, p.x))
        {
            action = Some(GridAction::Select(
                state.active,
                Cursor {
                    row: cursor.row,
                    column: state.columns[d],
                },
                false,
            ));
        }
    }
    // Side rows: click / Shift+click / double-click / right click, as in the
    // panes; hover shows the full text of a truncated or multi-line cell.
    let font = egui::FontId::monospace(crate::chrome::theme::EDITOR_SIZE);
    let rows = [0, 1].map(|side| {
        let response = ui.interact(
            lay.rows[side],
            egui::Id::unique(("table_details_row", side)),
            egui::Sense::click(),
        );
        let hit = |p: Option<egui::Pos2>| {
            p.and_then(|p| column_at(state, lay.left, p.x))
                .map(|d| Cursor {
                    row: cursor.row,
                    column: state.columns[d],
                })
        };
        if response.secondary_clicked() {
            if let Some(c) = hit(response.interact_pointer_pos()) {
                action = Some(GridAction::Context(side, c));
            }
        } else if response.double_clicked() {
            if let Some(c) = hit(response.interact_pointer_pos()) {
                action = Some(GridAction::Edit(side, c));
            }
        } else if response.clicked() {
            if let Some(c) = hit(response.interact_pointer_pos()) {
                action = Some(GridAction::Select(side, c, ui.input(|i| i.modifiers.shift)));
            }
        }
        if response.hovered() {
            if let Some(d) = ui
                .ctx()
                .pointer_hover_pos()
                .and_then(|p| column_at(state, lay.left, p.x))
            {
                let text = cell_text(map, cursor.row, side, state.columns[d]);
                let width = state.details_offsets[d + 1] - state.details_offsets[d];
                if needs_tooltip(ui, &text, width, &font) {
                    response.clone().on_hover_ui_at_pointer(|ui| {
                        ui.set_max_width(600.0);
                        ui.label(egui::RichText::new(tooltip_text(&text)).monospace());
                    });
                }
            }
        }
        response
    });
    paint(ui, rect, &lay, map, state, different, colors);
    // Scroll band: a real (pixel) scrollbar while the content overflows,
    // otherwise the reference's plain band with its dash.
    let content = state.details_offsets.last().copied().unwrap_or(0.0);
    if content > view {
        crate::chrome::scrollbar::show_horizontal_px(
            ui,
            lay.band,
            egui::Id::unique("table_details_hscroll"),
            content,
            view,
            &mut state.details_scroll_x,
        );
    } else {
        let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        painter.rect_filled(lay.band, 0.0, crate::chrome::theme::PANEL_BG);
        painter.text(
            egui::pos2(lay.band.left() + 18.0, lay.band.center().y),
            egui::Align2::LEFT_CENTER,
            "\u{2014}",
            egui::FontId::proportional(crate::chrome::theme::UI_SIZE),
            egui::Color32::from_gray(0x85),
        );
    }
    (action, rows)
}
fn paint(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    lay: &Layout,
    map: &TableMap,
    state: &GridState,
    different: &[bool],
    colors: &DiffColors,
) {
    let cursor: Cursor = state.cursor;
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    crate::chrome::theme::paint_canvas(&painter, rect);
    let band = lay.band;
    let left = lay.left;
    let width = state.details_offsets.last().copied().unwrap_or(0.0);
    let right_edge = (left + width - state.details_scroll_x).clamp(left, rect.right());
    let header = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.top()),
        egui::pos2(right_edge, rect.top() + HEADER),
    );
    painter.rect_filled(header, 0.0, crate::chrome::theme::HEADER_FILL);
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(rect.left(), header.bottom() - 1.0),
            header.right_bottom(),
        ),
        0.0,
        grid::GRID_LINE,
    );
    // Left border of the details grid.
    painter.rect_filled(
        egui::Rect::from_min_max(
            rect.left_top(),
            egui::pos2(rect.left() + 1.0, band.top() - 1.0),
        ),
        0.0,
        grid::GRID_LINE,
    );
    let row = grid::mapped(map, cursor.row);
    let rules = map.rules();
    let font = egui::FontId::monospace(crate::chrome::theme::EDITOR_SIZE);
    let mut notes = Vec::new();
    for (display, &c) in state.columns.iter().enumerate() {
        let x = left + state.details_offsets[display] - state.details_scroll_x;
        let w = state.details_offsets[display + 1] - state.details_offsets[display];
        if x + w < left || x > rect.right() {
            continue;
        }
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(x + w - 1.0, header.top()),
                egui::pos2(x + w, header.bottom()),
            ),
            0.0,
            grid::GRID_LINE,
        );
        if c == cursor.column {
            grid::column_indicator(&painter, x + w / 2.0, header.top() + 2.0);
        }
        let marked = different.get(c) == Some(&true);
        if marked {
            grid::marker(
                &painter,
                egui::pos2(x + 4.0, header.top() + 12.0),
                grid::marker_ink(colors, Importance::Important),
            );
        }
        painter.text(
            egui::pos2(x + if marked { 15.0 } else { 5.0 }, header.top() + 17.0),
            egui::Align2::LEFT_CENTER,
            grid::column_letter(c),
            egui::FontId::proportional(crate::chrome::theme::UI_SIZE),
            egui::Color32::from_gray(0x6D),
        );
        let imp = if map.is_stale() {
            Importance::Same
        } else {
            map.cell_importance(cursor.row, c)
                .unwrap_or(Importance::Same)
        };
        for side in 0..2 {
            let active = state.active == side;
            let top = header.bottom() + side as f32 * grid::ROW_HEIGHT;
            let cell =
                egui::Rect::from_min_size(egui::pos2(x, top), egui::vec2(w, grid::ROW_HEIGHT));
            let sheet = if side == 0 {
                map.left.borrow()
            } else {
                map.right.borrow()
            };
            let source = if side == 0 { row.left } else { row.right };
            let col = rules
                .columns
                .0
                .get(c)
                .and_then(|c| if side == 0 { c.left } else { c.right });
            let selected = active && state.selection_visible && state.selected(cursor.row, c);
            let fill = if selected {
                grid::SELECTION
            } else if imp != Importance::Same {
                grid::cell_fill(colors, imp, source.is_none(), active)
            } else if active {
                egui::Color32::WHITE
            } else {
                grid::PANE_INACTIVE
            };
            painter.rect_filled(cell, 0.0, fill);
            let text = source
                .zip(col)
                .and_then(|(r, c)| sheet.display_at(r, c))
                .unwrap_or_default();
            let numeric = source.zip(col).is_some_and(|(r, c)| {
                matches!(
                    bc_engine_table::TableData::cell_info(&*sheet, r, c).map(|i| i.value),
                    Some(bc_engine_table::CellData::Number(_))
                )
            });
            let ink = if imp != Importance::Same {
                grid::cell_ink(colors, imp)
            } else {
                egui::Color32::BLACK
            };
            let clipped = painter.with_clip_rect(cell.intersect(painter.clip_rect()));
            let shown = text.replace(['\n', '\r'], " ↵ ");
            if numeric {
                clipped.text(
                    egui::pos2(cell.right() - 6.0, cell.center().y),
                    egui::Align2::RIGHT_CENTER,
                    shown,
                    font.clone(),
                    ink,
                );
            } else {
                clipped.text(
                    egui::pos2(cell.left() + 6.0, cell.center().y),
                    egui::Align2::LEFT_CENTER,
                    shown,
                    font.clone(),
                    ink,
                );
            }
            let line = if active {
                grid::GRID_LINE
            } else {
                grid::GRID_LINE_INACTIVE
            };
            clipped.rect_filled(
                egui::Rect::from_min_max(egui::pos2(cell.right() - 1.0, cell.top()), cell.max),
                0.0,
                line,
            );
            clipped.rect_filled(
                egui::Rect::from_min_max(egui::pos2(cell.left(), cell.bottom() - 1.0), cell.max),
                0.0,
                line,
            );
            if c == cursor.column {
                if active {
                    grid::focus_frame(
                        &clipped,
                        egui::Rect::from_min_max(cell.min, cell.max - egui::vec2(1.0, 1.0)),
                        false,
                    );
                }
                let snapshot = source.zip(col).and_then(|(r, c)| sheet.cell_snapshot(r, c));
                let formula = snapshot.as_ref().and_then(|s| s.formula.clone());
                let format = sheet
                    .style(snapshot.as_ref().and_then(|s| s.style))
                    .and_then(|s| s.number_format.clone());
                notes.extend(formula.into_iter().chain(format).map(|n| (top, n)));
            }
        }
    }
    // Cached formula / number format of the current cell (workbooks only).
    let mut x = right_edge + 12.0;
    let mut row = None;
    for (top, note) in notes {
        if row != Some(top) {
            row = Some(top);
            x = right_edge + 12.0;
        }
        let rect = painter.text(
            egui::pos2(x, top + grid::ROW_HEIGHT / 2.0),
            egui::Align2::LEFT_CENTER,
            note,
            egui::FontId::monospace(crate::chrome::theme::EDITOR_SIZE),
            egui::Color32::from_gray(0x40),
        );
        x = rect.right() + 16.0;
    }
}
/// Reference thumbnail: a small framed miniature of the comparison (one
/// line per row, up to the column height) at the top of the 52 px column.
/// Returns the clicked comparison row.
pub fn thumbnail(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    bins: &[Importance],
    total: usize,
    visible: std::ops::Range<usize>,
    colors: &DiffColors,
) -> Option<usize> {
    let response = ui.interact(
        rect,
        ui.scope_id().with("table_thumbnail"),
        egui::Sense::click_and_drag(),
    );
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let lines = (total as f32)
        .min((rect.height() - 26.0).max(1.0))
        .max(1.0)
        .round();
    let frame = egui::Rect::from_min_size(
        rect.min + egui::vec2(7.0, 13.0),
        egui::vec2(38.0, lines + 2.0),
    );
    let inner = frame.shrink(1.0);
    painter.rect_filled(frame, 0.0, egui::Color32::from_gray(0x80));
    painter.rect_filled(inner, 0.0, egui::Color32::WHITE);
    let y = |r: usize| inner.top() + inner.height() * r.min(total) as f32 / total.max(1) as f32;
    for line in 0..lines as usize {
        let first = line * total / lines as usize;
        let bin = first * bins.len() / total.max(1);
        let state = bins.get(bin).copied().unwrap_or(Importance::Same);
        if state != Importance::Same {
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(inner.left(), inner.top() + line as f32),
                    egui::vec2(inner.width(), 1.0),
                ),
                0.0,
                if state == Importance::Unimportant {
                    grid::cell_ink(colors, state)
                } else {
                    egui::Color32::from_rgb(0xFF, 0x41, 0x41)
                },
            );
        }
    }
    if total > lines as usize {
        // Viewport frame only when the miniature is compressed.
        painter.rect_stroke(
            egui::Rect::from_min_max(
                egui::pos2(inner.left(), y(visible.start)),
                egui::pos2(inner.right(), y(visible.end).max(y(visible.start) + 3.0)),
            ),
            0.0,
            egui::Stroke::new(1.0, egui::Color32::from_gray(0x40)),
            egui::StrokeKind::Inside,
        );
    }
    if response.clicked() || response.dragged() {
        response
            .interact_pointer_pos()
            .map(|p| {
                (((p.y - inner.top()) / inner.height()).clamp(0.0, 1.0) * total as f32) as usize
            })
            .map(|r| r.min(total.saturating_sub(1)))
    } else {
        None
    }
}
