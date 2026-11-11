//! Text Details strip (current row of both sides as a two-row mini grid) and
//! the 52 px thumbnail column.
use super::grid::{self, Cursor, GridState};
use bc_engine_table::{compare::Importance, TableMap};
use bc_session::prefs::DiffColors;
use eframe::egui;
/// Reference details area: 27 px header, one 18 px row per side, a 16 px
/// horizontal-scroll band (81 px total with the edge lines).
pub const HEIGHT: f32 = 81.0;
const HEADER: f32 = 27.0;
pub fn paint(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    map: &TableMap,
    state: &GridState,
    different: &[bool],
    colors: &DiffColors,
) {
    let cursor: Cursor = state.cursor;
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    painter.rect_filled(rect, 0.0, egui::Color32::WHITE);
    // Bottom band: white edge, 16 px panel band (scroll placeholder), white edge.
    let band = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.bottom() - 17.0),
        egui::pos2(rect.right(), rect.bottom() - 1.0),
    );
    painter.rect_filled(band, 0.0, crate::chrome::theme::PANEL_BG);
    painter.text(
        egui::pos2(band.left() + 18.0, band.center().y),
        egui::Align2::LEFT_CENTER,
        "\u{2014}",
        egui::FontId::proportional(crate::chrome::theme::UI_SIZE),
        egui::Color32::from_gray(0x85),
    );
    let left = rect.left() + 1.0;
    let width = state.offsets.last().copied().unwrap_or(0.0);
    let right_edge = (left + width - state.scroll.x).clamp(left, rect.right());
    let header = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.top()),
        egui::pos2(right_edge, rect.top() + HEADER),
    );
    painter.rect_filled(header, 0.0, crate::chrome::theme::PANEL_BG);
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
        let x = left + state.offsets[display] - state.scroll.x;
        let w = state.offsets[display + 1] - state.offsets[display];
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
            let fill = if imp != Importance::Same {
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
