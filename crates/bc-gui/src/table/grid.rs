use super::cache::projected;
use crate::text::projection::Projection;
use bc_core::DiffKind;
use bc_engine_table::{compare::Importance, map::TableMapRow, TableData, TableMap};
use bc_session::prefs::DiffColors;
use eframe::egui;
use std::ops::Range;
/// Reference grid metrics (tokens `tableRowHeight` / `tableNumberGutterWidth` /
/// `tableHeaderHeight`): 18 px rows, 34 px row-number gutter, 28 px header.
pub const ROW_HEIGHT: f32 = 18.0;
pub const GUTTER: f32 = 34.0;
pub const HEADER_HEIGHT: f32 = 28.0;
/// Consolas 10 pt advance at 96 dpi (horizontal preview offset only).
const CHAR_WIDTH: f32 = 7.33;
pub const GRID_LINE: egui::Color32 = egui::Color32::from_rgb(0xDA, 0xDE, 0xE3);
/// Drag-resize floor for a column (header marker + letter stay visible).
pub const MIN_COLUMN_WIDTH: f32 = 20.0;
/// Header boundary grab half-width (px) for column resize.
const RESIZE_GRAB: f32 = 3.0;
pub const GRID_LINE_INACTIVE: egui::Color32 = egui::Color32::from_rgb(0xE3, 0xE6, 0xEA);
pub const PANE_INACTIVE: egui::Color32 = egui::Color32::from_rgb(0xFC, 0xFC, 0xFD);
pub const SELECTION: egui::Color32 = crate::chrome::theme::SELECTION;
pub const INDICATOR: egui::Color32 = crate::chrome::theme::ACCENT;
const DIFF_FILL: egui::Color32 = egui::Color32::from_rgb(0xFF, 0xE3, 0xE3);
const DIFF_FILL_INACTIVE: egui::Color32 = egui::Color32::from_rgb(0xFC, 0xE6, 0xE6);
const MINOR_FILL: egui::Color32 = egui::Color32::from_rgb(0xEE, 0xEE, 0xFF);
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
    /// Display column whose right header boundary is being drag-resized.
    pub resizing: Option<usize>,
    /// The cursor cell is painted as a selection (blue) only after the user
    /// placed it (click/keyboard/select-all); a freshly loaded comparison shows
    /// just the focus frame, like the reference.
    pub selection_visible: bool,
    /// Text Details strip: per comparison column width overrides (columns
    /// without one follow `widths`), its own prefix offsets in display
    /// order, horizontal offset, the cursor column it last scrolled to, and
    /// the display column whose boundary is being dragged.
    pub details_widths: std::collections::HashMap<usize, f32>,
    pub details_offsets: Vec<f32>,
    pub details_scroll_x: f32,
    pub details_follow: Option<usize>,
    pub details_resizing: Option<usize>,
    /// Last painted Text Details strip rect (NOTHING while hidden).
    pub details_rect: egui::Rect,
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
            resizing: None,
            selection_visible: false,
            details_widths: std::collections::HashMap::new(),
            details_offsets: vec![0.0],
            details_scroll_x: 0.0,
            details_follow: None,
            details_resizing: None,
            details_rect: egui::Rect::NOTHING,
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub enum GridAction {
    Select(usize, Cursor, bool),
    Edit(usize, Cursor),
    /// Right click on a cell: select it (unless it is inside the selection)
    /// before the context menu opens.
    Context(usize, Cursor),
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
/// Cell background of a differing cell in the active / inactive pane; user
/// colors from Options are blended like the text view does.
pub fn cell_fill(
    colors: &DiffColors,
    importance: Importance,
    orphan: bool,
    active: bool,
) -> egui::Color32 {
    let defaults = DiffColors::default();
    let custom = if orphan {
        colors.inserted != defaults.inserted
    } else if importance == Importance::Unimportant {
        colors.unimportant != defaults.unimportant
    } else {
        colors.important != defaults.important
    };
    if custom {
        color(colors, importance, orphan, 38)
    } else if importance == Importance::Unimportant {
        MINOR_FILL
    } else if active {
        DIFF_FILL
    } else {
        DIFF_FILL_INACTIVE
    }
}
/// Text color of a differing cell (red / blue by importance).
pub fn cell_ink(colors: &DiffColors, importance: Importance) -> egui::Color32 {
    let defaults = DiffColors::default();
    if importance == Importance::Unimportant {
        if colors.unimportant == defaults.unimportant {
            egui::Color32::BLUE
        } else {
            color(colors, importance, false, 255)
        }
    } else if colors.important == defaults.important {
        egui::Color32::RED
    } else {
        color(colors, importance, false, 255)
    }
}
/// Small difference marker: 9 px square, white rim, shaded 7 px colored core.
pub fn marker(painter: &egui::Painter, min: egui::Pos2, ink: egui::Color32) {
    let rect = egui::Rect::from_min_size(min, egui::vec2(9.0, 9.0));
    painter.rect_filled(rect, 0.0, egui::Color32::WHITE);
    let core = rect.shrink(1.0);
    // Lighten each channel toward white by `num/den`.
    let tint = |num: u16, den: u16| {
        let lift = |c: u8| (c as u16 + (255 - c as u16) * num / den) as u8;
        egui::Color32::from_rgb(lift(ink.r()), lift(ink.g()), lift(ink.b()))
    };
    painter.rect_filled(core, 0.0, tint(1, 3));
    painter.rect_filled(
        egui::Rect::from_min_max(core.min, egui::pos2(core.right(), core.center().y)),
        0.0,
        tint(2, 7),
    );
}
/// Marker ink of a row/column with differences.
pub fn marker_ink(colors: &DiffColors, importance: Importance) -> egui::Color32 {
    if importance == Importance::Unimportant {
        cell_ink(colors, importance)
    } else {
        egui::Color32::from_rgb(0xE0, 0x10, 0x10)
    }
}
/// Blue current-row (▶) indicator: 6 px wide, 11 px tall, pixel-built and
/// centered on the pixel row `center_y`.
pub fn row_indicator(painter: &egui::Painter, left: f32, center_y: f32) {
    let mut mesh = egui::Mesh::default();
    for i in 0..6 {
        let half = 5.0 - i as f32;
        mesh.add_colored_rect(
            egui::Rect::from_min_max(
                egui::pos2(left + i as f32, center_y - half),
                egui::pos2(left + i as f32 + 1.0, center_y + half + 1.0),
            ),
            INDICATOR,
        );
    }
    painter.add(egui::Shape::mesh(mesh));
}
/// Blue current-column (▼) indicator: 11 px wide, 6 px tall, centered on the
/// pixel column `center_x`.
pub fn column_indicator(painter: &egui::Painter, center_x: f32, top: f32) {
    let center_x = center_x.floor();
    let mut mesh = egui::Mesh::default();
    for i in 0..6 {
        let half = 5.0 - i as f32;
        mesh.add_colored_rect(
            egui::Rect::from_min_max(
                egui::pos2(center_x - half, top + i as f32),
                egui::pos2(center_x + half + 1.0, top + i as f32 + 1.0),
            ),
            INDICATOR,
        );
    }
    painter.add(egui::Shape::mesh(mesh));
}
/// 1 px focus frame: dotted black for the active pane, solid dark otherwise.
pub fn focus_frame(painter: &egui::Painter, rect: egui::Rect, dotted: bool) {
    if !dotted {
        painter.rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.0, egui::Color32::from_gray(0x30)),
            egui::StrokeKind::Inside,
        );
        return;
    }
    // One unfeathered mesh: alternating 1 px dots along the four edges.
    let ink = egui::Color32::BLACK;
    let (l, t, r, b) = (
        rect.left(),
        rect.top(),
        rect.right() - 1.0,
        rect.bottom() - 1.0,
    );
    let mut mesh = egui::Mesh::default();
    let mut dot = |x: f32, y: f32| {
        mesh.add_colored_rect(
            egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(1.0, 1.0)),
            ink,
        )
    };
    let mut x = l;
    while x <= r {
        dot(x, t);
        dot(x, b);
        x += 2.0;
    }
    let mut y = t + 2.0;
    while y < b {
        dot(l, y);
        dot(r, y);
        y += 2.0;
    }
    painter.add(egui::Shape::mesh(mesh));
}
/// Spreadsheet column letters: 0 → A, 25 → Z, 26 → AA.
pub fn column_letter(mut column: usize) -> String {
    let mut out = Vec::new();
    loop {
        out.push(b'A' + (column % 26) as u8);
        if column < 26 {
            break;
        }
        column = column / 26 - 1;
    }
    out.reverse();
    String::from_utf8(out).unwrap()
}
impl GridState {
    pub fn rebuild_columns(&mut self, count: usize, different: &[bool], hide: bool) {
        self.columns = (0..count)
            .filter(|&c| !hide || different.get(c) == Some(&true))
            .collect();
        self.rebuild_offsets();
    }
    /// Prefix offsets from `widths` in display order (shared by both panes).
    pub fn rebuild_offsets(&mut self) {
        self.offsets = vec![0.0];
        for &c in &self.columns {
            self.offsets
                .push(self.offsets.last().unwrap() + self.widths.get(c).copied().unwrap_or(68.0));
        }
    }
    /// Header boundary hit test: the display column whose right edge sits
    /// within `RESIZE_GRAB` px of pane-space `x` (boundaries scrolled under
    /// the row-number gutter are not grabbable).
    pub fn resize_target(&self, pane_left: f32, x: f32) -> Option<usize> {
        let origin = pane_left + GUTTER - self.scroll.x;
        (0..self.columns.len()).find(|&d| {
            let boundary = origin + self.offsets[d + 1];
            (boundary - x).abs() <= RESIZE_GRAB && boundary > pane_left + GUTTER + RESIZE_GRAB
        })
    }
    /// Drag-resize display column `d` so its right edge sits at content-space
    /// `right`; later columns shift by the same delta.
    pub fn resize_column(&mut self, d: usize, right: f32) {
        let Some(&c) = self.columns.get(d) else {
            return;
        };
        if c >= self.widths.len() {
            self.widths.resize(c + 1, 68.0);
        }
        let width = (right - self.offsets[d]).clamp(MIN_COLUMN_WIDTH, 1000.0);
        let delta = width - (self.offsets[d + 1] - self.offsets[d]);
        if delta == 0.0 {
            return;
        }
        self.widths[c] = width;
        for o in &mut self.offsets[d + 1..] {
            *o += delta;
        }
    }
    /// Text Details offsets: override width when present, else the grid width.
    pub fn rebuild_details_offsets(&mut self) {
        self.details_offsets.clear();
        self.details_offsets.push(0.0);
        for &c in &self.columns {
            let w = self
                .details_widths
                .get(&c)
                .or_else(|| self.widths.get(c))
                .copied()
                .unwrap_or(68.0);
            self.details_offsets
                .push(self.details_offsets.last().unwrap() + w);
        }
    }
    /// Text Details header boundary hit test (`left` is the strip's content
    /// origin before scrolling); same ±`RESIZE_GRAB` rule as the grid.
    pub fn details_resize_target(&self, left: f32, x: f32) -> Option<usize> {
        let origin = left - self.details_scroll_x;
        (0..self.details_offsets.len().saturating_sub(1)).find(|&d| {
            let boundary = origin + self.details_offsets[d + 1];
            (boundary - x).abs() <= RESIZE_GRAB && boundary > left + RESIZE_GRAB
        })
    }
    /// Drag-resize Text Details display column `d` so its right edge sits at
    /// strip content-space `right` (override only; the grid keeps its width).
    pub fn details_resize_column(&mut self, d: usize, right: f32) {
        if d + 1 >= self.details_offsets.len() {
            return;
        }
        let width = right - self.details_offsets[d];
        self.details_set_width(d, width);
    }
    /// Set the Text Details width of display column `d` (clamped 20–1000 px)
    /// and shift the later offsets.
    pub fn details_set_width(&mut self, d: usize, width: f32) {
        let Some(&c) = self.columns.get(d) else {
            return;
        };
        if d + 1 >= self.details_offsets.len() {
            return;
        }
        let width = width.clamp(MIN_COLUMN_WIDTH, 1000.0);
        let delta = width - (self.details_offsets[d + 1] - self.details_offsets[d]);
        self.details_widths.insert(c, width);
        if delta != 0.0 {
            for o in &mut self.details_offsets[d + 1..] {
                *o += delta;
            }
        }
    }
    /// Drop every Text Details width override (Resize Columns to Fit,
    /// reload, column count change); the strip follows the grid again.
    pub fn clear_details_widths(&mut self) {
        self.details_widths.clear();
        self.details_resizing = None;
        self.rebuild_details_offsets();
    }
    /// Scroll the Text Details strip once per cursor column change so the
    /// cursor column is visible (minimal scroll; a column wider than the view
    /// aligns its left edge), then clamp to the content.
    pub fn details_follow_cursor(&mut self, view: f32) {
        let column = self.cursor.column;
        if self.details_follow != Some(column) {
            self.details_follow = Some(column);
            if let Some(d) = self.columns.iter().position(|&c| c == column) {
                let left = self.details_offsets[d];
                let right = self.details_offsets[d + 1];
                if left < self.details_scroll_x || right - left > view {
                    self.details_scroll_x = left;
                } else if right > self.details_scroll_x + view {
                    self.details_scroll_x = right - view;
                }
            }
        }
        let max = (self.details_offsets.last().copied().unwrap_or(0.0) - view).max(0.0);
        self.details_scroll_x = self.details_scroll_x.clamp(0.0, max);
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
/// Wheel/trackpad deltas over the grid panes, applied immediately (see
/// [`crate::chrome::wheel`]); the grid ScrollAreas run with
/// `mouse_wheel: false`.
pub fn wheel(ctx: &egui::Context, state: &mut GridState, enabled: bool) {
    if !enabled {
        return;
    }
    let delta = crate::chrome::wheel::delta(ctx, &state.pane_rects);
    if delta == egui::Vec2::ZERO {
        return;
    }
    // Offset space is inverted from wheel deltas (same as egui's ScrollArea):
    // wheel up (positive y) decreases the scroll offset.
    state.scroll.x = (state.scroll.x - delta.x).max(0.0);
    state.scroll.y = (state.scroll.y - delta.y).max(0.0);
    // Upper-bound clamping happens in `pane` later this same frame.
}
/// One comparison pane: 28 px column header plus the virtualized rows.
/// `rect` starts at the pane's left border and the header's top line.
/// Also returns the rows area response (the owner of the context menu).
#[allow(clippy::too_many_arguments)]
pub fn pane(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    map: &TableMap,
    projection: &Projection,
    state: &mut GridState,
    side: usize,
    different: &[bool],
    colors: &DiffColors,
) -> (Option<GridAction>, egui::Response) {
    let active = state.active == side;
    let line = if active {
        GRID_LINE
    } else {
        GRID_LINE_INACTIVE
    };
    // Empty area is canvas; each drawn row lays a white sheet over it.
    crate::chrome::theme::paint_canvas(&ui.painter_at(rect), rect);
    let rows_rect = egui::Rect::from_min_max(rect.min + egui::vec2(0.0, HEADER_HEIGHT), rect.max);
    let view_height = rows_rect.height().max(ROW_HEIGHT);
    let width = GUTTER + state.offsets.last().copied().unwrap_or(0.0);
    // The vertical scrollbar column is 18 px (16 px bar plus margins).
    let need_hscroll = width > rows_rect.width() - 18.0;
    let visible_height = view_height - if need_hscroll { 18.0 } else { 0.0 };
    state.scroll.y = state
        .scroll
        .y
        .min((projection.len() as f32 * ROW_HEIGHT - visible_height).max(0.0));
    state.scroll.x = state
        .scroll
        .x
        .min((width - (rows_rect.width() - 18.0)).max(0.0));
    super::header::paint(
        ui,
        egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rows_rect.top())),
        map,
        state,
        side,
        different,
        colors,
    );
    // Column resize: drag a lettered header boundary (Excel style). Widths
    // are shared by both panes; the other pane picks up the new offsets when
    // it paints later this same frame.
    let header_rect = egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), rows_rect.top()));
    let header_response = ui.interact(
        header_rect,
        egui::Id::unique(("table_header_resize", side)),
        egui::Sense::click_and_drag(),
    );
    if header_response.drag_started() {
        // Hit-test the press origin (interact_pointer_pos is already past it).
        state.resizing = ui
            .ctx()
            .input(|i| i.pointer.press_origin())
            .and_then(|p| state.resize_target(header_rect.left(), p.x));
    }
    if header_response.dragged() {
        if let Some(d) = state.resizing {
            if let Some(pos) = header_response.interact_pointer_pos() {
                let right = pos.x - (header_rect.left() + GUTTER) + state.scroll.x;
                state.resize_column(d, right);
            }
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
    } else if header_response.drag_stopped() {
        state.resizing = None;
    } else if header_response.hovered() {
        if let Some(pos) = ui.ctx().pointer_hover_pos() {
            if state.resize_target(header_rect.left(), pos.x).is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
        }
    }
    let scroll = state.scroll;
    let mut action = None;
    state.pane_rects[side] = rows_rect;
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
    let pane_response = ui.interact(rows_rect, focus_id, egui::Sense::click_and_drag());
    let font = egui::FontId::monospace(crate::chrome::theme::EDITOR_SIZE);
    let cursor = state.cursor;
    let output = ui
        .scope_builder(
            egui::UiBuilder::new()
                .id_salt(("table_grid_area", side))
                .max_rect(rows_rect),
            |ui| {
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                ui.visuals_mut().extreme_bg_color = crate::chrome::theme::CANVAS;
                ui.spacing_mut().scroll.bar_width = 16.0;
                ui.spacing_mut().scroll.bar_inner_margin = 1.0;
                ui.spacing_mut().scroll.bar_outer_margin = 1.0;
                egui::ScrollArea::new([need_hscroll, true])
                    .id_salt(("table_grid", side))
                    // Wheel deltas are consumed by `grid::wheel` (immediate,
                    // unsmoothed); scrollbars and touch drag keep the framework
                    // behavior.
                    .scroll_source(crate::chrome::wheel::scroll_source())
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .max_height(view_height)
                    .vertical_scroll_offset(scroll.y)
                    .horizontal_scroll_offset(scroll.x)
                    .show_rows(ui, ROW_HEIGHT, projection.len(), |ui, range| {
                        rows(
                            ui,
                            range,
                            map,
                            projection,
                            state,
                            RowStyle {
                                side,
                                active,
                                line,
                                width,
                                scroll,
                                cursor,
                                font: &font,
                                colors,
                            },
                        );
                    })
            },
        )
        .inner;
    // Pane border: left edge (the header draws the top line).
    ui.painter().rect_filled(
        egui::Rect::from_min_max(rect.min, egui::pos2(rect.left() + 1.0, rect.bottom())),
        0.0,
        GRID_LINE,
    );
    let offset = output.state.clamped_offset();
    if offset != scroll {
        state.scroll = offset;
    }
    let secondary = pane_response.secondary_clicked();
    if (pane_response.clicked() || pane_response.dragged() || secondary)
        && !state.columns.is_empty()
    {
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
                action = Some(if secondary {
                    GridAction::Context(side, cursor)
                } else if pane_response.double_clicked() {
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
    (action, pane_response)
}
struct RowStyle<'a> {
    side: usize,
    active: bool,
    line: egui::Color32,
    width: f32,
    scroll: egui::Vec2,
    cursor: Cursor,
    font: &'a egui::FontId,
    colors: &'a DiffColors,
}
fn rows(
    ui: &mut egui::Ui,
    range: Range<usize>,
    map: &TableMap,
    projection: &Projection,
    state: &mut GridState,
    style: RowStyle,
) {
    let RowStyle {
        side,
        active,
        line,
        width,
        scroll,
        cursor,
        font,
        colors,
    } = style;
    let rules = map.rules();
    let sheet = if side == 0 {
        map.left.borrow()
    } else {
        map.right.borrow()
    };
    let painter = ui.painter().clone();
    let clip = ui.clip_rect();
    state.visible = projected(projection, range.start).unwrap_or(0)
        ..projected(projection, range.end.saturating_sub(1)).map_or(0, |r| r + 1);
    // Grid lines are batched: one horizontal line per row, one vertical line
    // per visible column spanning all drawn rows.
    let mut span: Option<(f32, f32, f32)> = None; // (origin x, first top, last bottom)
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
            // Sheet under the cell (canvas shows outside the data).
            let sheet_rect = cell_rect
                .intersect(clip)
                .intersect(egui::Rect::from_min_max(
                    egui::pos2(gutter.right(), clip.top()),
                    clip.max,
                ));
            if sheet_rect.is_positive() {
                painter.rect_filled(
                    sheet_rect,
                    0.0,
                    if active {
                        egui::Color32::WHITE
                    } else {
                        PANE_INACTIVE
                    },
                );
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
            let selected = active && state.selection_visible && state.selected(row, c);
            let fill = if selected {
                Some(SELECTION)
            } else if imp != Importance::Same {
                Some(cell_fill(colors, imp, source.is_none(), active))
            } else {
                None
            };
            if let Some(fill) = fill {
                painter.rect_filled(paint_rect, 0.0, fill);
            }
            let col = if side == 0 {
                rules.columns.0.get(c).and_then(|c| c.left)
            } else {
                rules.columns.0.get(c).and_then(|c| c.right)
            };
            let start_char = ((paint_rect.left() - cell_rect.left() - 6.0).max(0.0) / CHAR_WIDTH)
                .floor() as usize;
            let numeric = source.zip(col).is_some_and(|(r, c)| {
                matches!(
                    TableData::cell_info(&*sheet, r, c).map(|i| i.value),
                    Some(bc_engine_table::CellData::Number(_))
                )
            });
            let mut text = source
                .and_then(|r| col.and_then(|c| sheet.preview(r, c, start_char, 256)))
                .unwrap_or_default();
            if state.whitespace {
                text = text.replace(' ', "·").replace('\t', "→").replace('\n', "↵");
            } else if text.contains(['\n', '\r']) {
                text = text.replace(['\n', '\r'], " ↵ ");
            }
            let ink = if imp != Importance::Same {
                cell_ink(colors, imp)
            } else {
                egui::Color32::BLACK
            };
            let cell_painter = painter.with_clip_rect(paint_rect);
            if numeric && start_char == 0 {
                cell_painter.text(
                    egui::pos2(cell_rect.right() - 6.0, cell_rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    text,
                    font.clone(),
                    ink,
                );
            } else {
                cell_painter.text(
                    egui::pos2(
                        cell_rect.left() + 6.0 + start_char as f32 * CHAR_WIDTH,
                        cell_rect.center().y,
                    ),
                    egui::Align2::LEFT_CENTER,
                    text,
                    font.clone(),
                    ink,
                );
            }
            if row == cursor.row && c == cursor.column {
                focus_frame(
                    &cell_painter,
                    egui::Rect::from_min_max(cell_rect.min, cell_rect.max - egui::vec2(1.0, 1.0)),
                    active,
                );
            }
            state.cells_drawn += 1;
        }
        let content_right = rect.left() + GUTTER + state.offsets.last().copied().unwrap_or(0.0);
        if content_right > gutter.right() {
            painter.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(gutter.right(), rect.bottom() - 1.0),
                    egui::pos2(content_right.min(clip.right()), rect.bottom()),
                ),
                0.0,
                line,
            );
        }
        span = Some(match span {
            None => (rect.left(), rect.top(), rect.bottom()),
            Some((x, top, _)) => (x, top, rect.bottom()),
        });
        painter.rect_filled(gutter, 0.0, crate::chrome::theme::HEADER_FILL);
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(gutter.right() - 1.0, gutter.top()), gutter.max),
            0.0,
            line,
        );
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(gutter.left(), gutter.bottom() - 1.0), gutter.max),
            0.0,
            line,
        );
        let center = gutter.center().y;
        if row == cursor.row {
            row_indicator(&painter, gutter.left() + 2.0, gutter.top() + 8.0);
        }
        if pair.importance != Importance::Same {
            marker(
                &painter,
                egui::pos2(gutter.left() + 11.0, gutter.top() + 3.0),
                marker_ink(colors, pair.importance),
            );
        }
        if state.row_numbers {
            painter.text(
                egui::pos2(gutter.right() - 4.0, center),
                egui::Align2::RIGHT_CENTER,
                source.map_or("—".into(), |r| (r + 1).to_string()),
                egui::FontId::proportional(crate::chrome::theme::UI_SIZE),
                egui::Color32::from_gray(0x6D),
            );
        }
        state.rows_drawn += 1;
    }
    if let Some((origin, top, bottom)) = span {
        let gutter_right = clip.left() + GUTTER;
        let start = state
            .offsets
            .partition_point(|&x| x < scroll.x)
            .saturating_sub(1);
        for d in start..state.columns.len() {
            let x = origin + GUTTER + state.offsets[d + 1] - 1.0;
            if x > clip.right() {
                break;
            }
            if x < gutter_right {
                continue;
            }
            painter.rect_filled(
                egui::Rect::from_min_max(egui::pos2(x, top), egui::pos2(x + 1.0, bottom)),
                0.0,
                line,
            );
        }
    }
}
