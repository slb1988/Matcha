//! 28 px column header: ▼ on the cursor column, difference marker, and the
//! Excel-style column letter (A, B, C…; the grid keeps every source row as
//! data, so letters are the stable column identity).
use super::grid::{self, GridState, GUTTER};
use bc_engine_table::TableMap;
use bc_session::prefs::DiffColors;
use eframe::egui;
pub fn paint(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    map: &TableMap,
    state: &GridState,
    side: usize,
    different: &[bool],
    colors: &DiffColors,
) {
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let line = if state.active == side {
        grid::GRID_LINE
    } else {
        grid::GRID_LINE_INACTIVE
    };
    let content_right = (rect.left() + GUTTER + state.offsets.last().copied().unwrap_or(0.0)
        - state.scroll.x)
        .max(rect.left() + GUTTER)
        .min(rect.right());
    let filled = egui::Rect::from_min_max(rect.min, egui::pos2(content_right, rect.bottom()));
    painter.rect_filled(filled, 0.0, crate::chrome::theme::HEADER_FILL);
    // Top line spans the filled header only (canvas beyond it).
    painter.rect_filled(
        egui::Rect::from_min_max(rect.min, egui::pos2(filled.right(), rect.top() + 1.0)),
        0.0,
        grid::GRID_LINE,
    );
    // Right edge of the header row.
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(filled.right() - 1.0, rect.top()),
            filled.right_bottom(),
        ),
        0.0,
        grid::GRID_LINE,
    );
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.bottom() - 1.0),
            filled.right_bottom(),
        ),
        0.0,
        grid::GRID_LINE,
    );
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(rect.left() + GUTTER - 1.0, rect.top()),
            egui::pos2(rect.left() + GUTTER, rect.bottom()),
        ),
        0.0,
        grid::GRID_LINE,
    );
    let rules = map.rules();
    let hidden = rules.columns.0.len().saturating_sub(state.columns.len());
    if hidden > 0 {
        painter.text(
            rect.left_center() + egui::vec2(3.0, 0.0),
            egui::Align2::LEFT_CENTER,
            format!("{hidden} hidden"),
            egui::FontId::proportional(9.0),
            egui::Color32::from_gray(0x6D),
        );
    }
    // The key marker only carries information when the keys are a proper
    // subset of the compared columns.
    let keys = rules.key_columns();
    let keyed = |c: usize| {
        rules.columns.0[c].handling.is_key
            || (keys.contains(&c) && keys.len() < rules.columns.0.len())
    };
    let columns = painter.with_clip_rect(
        egui::Rect::from_min_max(egui::pos2(rect.left() + GUTTER, rect.top()), rect.max)
            .intersect(ui.clip_rect()),
    );
    for (display, &c) in state.columns.iter().enumerate() {
        let x = rect.left() + GUTTER + state.offsets[display] - state.scroll.x;
        let width = state.offsets[display + 1] - state.offsets[display];
        if x + width < rect.left() + GUTTER || x > rect.right() {
            continue;
        }
        columns.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(x + width - 1.0, rect.top()),
                egui::pos2(x + width, rect.bottom()),
            ),
            0.0,
            line,
        );
        if c == state.cursor.column {
            grid::column_indicator(&columns, x + width / 2.0, rect.top() + 2.0);
        }
        let marked = different.get(c) == Some(&true);
        if marked {
            grid::marker(
                &columns,
                egui::pos2(x + 4.0, rect.top() + 13.0),
                grid::marker_ink(colors, bc_engine_table::compare::Importance::Important),
            );
        }
        let col = &rules.columns.0[c];
        // Excel-style letter regardless of a promoted header row; "—" marks a
        // comparison column missing on this side.
        let mapped = col
            .index(if side == 0 {
                bc_engine_table::columns::Side::Left
            } else {
                bc_engine_table::columns::Side::Right
            })
            .is_some();
        let name = if mapped {
            grid::column_letter(c)
        } else {
            "—".to_string()
        };
        let label = format!(
            "{}{}{}",
            if keyed(c) { "◆ " } else { "" },
            name,
            if col.changes.is_empty() { "" } else { " *" }
        );
        columns.text(
            egui::pos2(x + if marked { 15.0 } else { 5.0 }, rect.top() + 17.0),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(crate::chrome::theme::UI_SIZE),
            egui::Color32::from_gray(0x6D),
        );
    }
}
