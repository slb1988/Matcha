//! Text Details strip interaction (openspec `table-details-interaction`):
//! own column widths, own horizontal scroll, and clicks routed like the grid.
use super::super::{details, grid::MIN_COLUMN_WIDTH};
use super::*;

const LONG: &str =
    "/Script/PLCoreGame.PhysicalMaterialWithTags'/Game/005_Material/PM_Metal_Copper'";

/// 12 columns, the second one long: the strip content is wider than its view.
fn wide_view() -> TableCompareView {
    let rows = |right: bool| {
        let mut text = String::new();
        for r in 1..=3 {
            let mut cells = vec![r.to_string()];
            cells.push(if right && r == 1 {
                format!("{LONG}_v2")
            } else {
                LONG.to_string()
            });
            for c in 2..12 {
                cells.push(if right && r == 1 && c == 5 {
                    format!("changed_{r}_{c}_padding_xxxxx")
                } else {
                    format!("value_{r}_{c}_padding_xxxxxxx")
                });
            }
            text.push_str(&cells.join(","));
            text.push('\n');
        }
        text
    };
    let mut v = TableCompareView::from_sheets(
        sheet(&rows(false)),
        sheet(&rows(true)),
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            strategy: AlignStrategy::Quick,
            ..Default::default()
        },
    );
    // Fixture starts from the origin; first-difference landing is tested separately.
    v.land_on_first_diff = false;
    ready(&mut v);
    v
}
fn warm(ctx: &egui::Context, v: &mut TableCompareView) {
    crate::chrome::theme::apply(ctx);
    frame(ctx, v, vec![]);
    frame(ctx, v, vec![]);
}
fn strip(v: &TableCompareView) -> details::Layout {
    assert!(v.grid.details_rect.is_positive(), "strip not painted");
    details::layout(v.grid.details_rect)
}
/// Strip x of display column `d`'s right boundary.
fn boundary_x(v: &TableCompareView, d: usize) -> f32 {
    strip(v).left + v.grid.details_offsets[d + 1] - v.grid.details_scroll_x
}
/// Point inside display column `d` of the strip row of `side`.
fn cell_point(v: &TableCompareView, side: usize, d: usize) -> egui::Pos2 {
    let lay = strip(v);
    egui::pos2(
        lay.left + v.grid.details_offsets[d] - v.grid.details_scroll_x + 8.0,
        lay.rows[side].center().y,
    )
}
fn button_event(
    p: egui::Pos2,
    button: egui::PointerButton,
    pressed: bool,
    shift: bool,
) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(p),
        egui::Event::PointerButton {
            pos: p,
            button,
            pressed,
            modifiers: egui::Modifiers {
                shift,
                ..Default::default()
            },
        },
    ]
}
fn press_release(
    ctx: &egui::Context,
    v: &mut TableCompareView,
    p: egui::Pos2,
    button: egui::PointerButton,
    shift: bool,
) {
    let modifiers = egui::Modifiers {
        shift,
        ..Default::default()
    };
    for pressed in [true, false] {
        // egui tracks held modifiers from `ModifiersChanged`, not the button event.
        let mut events = vec![egui::Event::ModifiersChanged(modifiers)];
        events.extend(button_event(p, button, pressed, shift));
        frame(ctx, v, events);
    }
    if shift {
        frame(
            ctx,
            v,
            vec![egui::Event::ModifiersChanged(Default::default())],
        );
    }
}
fn drag(ctx: &egui::Context, v: &mut TableCompareView, from: egui::Pos2, dx: f32) {
    frame(ctx, v, vec![egui::Event::PointerMoved(from)]);
    frame(
        ctx,
        v,
        button_event(from, egui::PointerButton::Primary, true, false),
    );
    frame(
        ctx,
        v,
        vec![egui::Event::PointerMoved(from + egui::vec2(dx / 2.0, 0.0))],
    );
    let to = from + egui::vec2(dx, 0.0);
    frame(ctx, v, vec![egui::Event::PointerMoved(to)]);
    frame(
        ctx,
        v,
        button_event(to, egui::PointerButton::Primary, false, false),
    );
    frame(ctx, v, vec![]);
}
fn width(v: &TableCompareView, d: usize) -> f32 {
    v.grid.details_offsets[d + 1] - v.grid.details_offsets[d]
}
fn has(out: &egui::FullOutput, text: &str) -> bool {
    count_text(out, text) > 0
}
fn count_text(out: &egui::FullOutput, text: &str) -> usize {
    out.shapes
        .iter()
        .filter(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == text))
        .count()
}

#[test]
fn details_widths_override_follow_and_clamp() {
    let mut s = GridState {
        widths: vec![33.0, 47.0, 33.0],
        ..Default::default()
    };
    s.rebuild_columns(3, &[], false);
    s.rebuild_details_offsets();
    assert_eq!(s.details_offsets, vec![0.0, 33.0, 80.0, 113.0]);
    // ±3 px grab around a boundary; mid-column and the strip edge are not.
    let left = 100.0;
    assert_eq!(s.details_resize_target(left, left + 33.0 + 2.0), Some(0));
    assert_eq!(s.details_resize_target(left, left + 33.0 + 6.0), None);
    assert_eq!(s.details_resize_target(left, left), None);
    // Resize only touches the strip: later strip offsets shift, the grid keeps its widths.
    s.details_resize_column(0, 63.0);
    assert_eq!(s.details_offsets, vec![0.0, 63.0, 110.0, 143.0]);
    assert_eq!(s.widths, vec![33.0, 47.0, 33.0]);
    assert_eq!(s.offsets, vec![0.0, 33.0, 80.0, 113.0]);
    // Floor and ceiling.
    s.details_resize_column(0, 1.0);
    assert_eq!(s.details_widths[&0], MIN_COLUMN_WIDTH);
    s.details_set_width(0, 5000.0);
    assert_eq!(s.details_widths[&0], 1000.0);
    // A column without an override follows the grid width.
    s.widths[1] = 60.0;
    s.rebuild_offsets();
    s.rebuild_details_offsets();
    assert_eq!(s.details_offsets, vec![0.0, 1000.0, 1060.0, 1093.0]);
    // Clearing returns every column to the grid width.
    s.clear_details_widths();
    assert_eq!(s.details_offsets, s.offsets);
}

#[test]
fn details_follow_cursor_scrolls_minimally_once_per_column_change() {
    let mut s = GridState {
        widths: vec![100.0; 10],
        ..Default::default()
    };
    s.rebuild_columns(10, &[], false);
    s.rebuild_details_offsets();
    s.details_follow_cursor(250.0);
    assert_eq!(s.details_scroll_x, 0.0, "column A already visible");
    // Column F (500..600): minimal scroll puts its right edge at the view edge.
    s.cursor.column = 5;
    s.details_follow_cursor(250.0);
    assert_eq!(s.details_scroll_x, 350.0);
    // Manual scroll away is not pulled back while the cursor column is unchanged.
    s.details_scroll_x = 0.0;
    s.details_follow_cursor(250.0);
    assert_eq!(s.details_scroll_x, 0.0);
    // Moving left of the view aligns the column's left edge.
    s.details_scroll_x = 700.0;
    s.cursor.column = 2;
    s.details_follow_cursor(250.0);
    assert_eq!(s.details_scroll_x, 200.0);
    // A column wider than the view aligns its left edge.
    s.details_widths.insert(7, 400.0);
    s.rebuild_details_offsets();
    s.cursor.column = 7;
    s.details_follow_cursor(250.0);
    assert_eq!(s.details_scroll_x, 700.0);
    // Scroll is clamped to the content.
    s.details_scroll_x = 1e6;
    s.details_follow_cursor(250.0);
    assert_eq!(
        s.details_scroll_x,
        s.details_offsets.last().unwrap() - 250.0
    );
}

#[test]
fn resize_columns_to_fit_and_reload_clear_details_overrides() {
    let ctx = egui::Context::default();
    let mut v = wide_view();
    warm(&ctx, &mut v);
    let grid_width = width(&v, 1);
    v.grid.details_widths.insert(1, grid_width + 200.0);
    frame(&ctx, &mut v, vec![]);
    assert_eq!(width(&v, 1), grid_width + 200.0);
    v.command("Resize Columns to Fit", &ctx, &Prefs::default());
    ready(&mut v);
    frame(&ctx, &mut v, vec![]);
    assert!(
        v.grid.details_widths.is_empty(),
        "Resize Columns to Fit clears overrides"
    );
    assert_eq!(v.grid.details_offsets, v.grid.offsets);
    v.grid.details_widths.insert(1, 300.0);
    v.reload();
    ready(&mut v);
    frame(&ctx, &mut v, vec![]);
    assert!(v.grid.details_widths.is_empty(), "reload clears overrides");
}

#[test]
fn wheel_over_the_strip_scrolls_only_the_strip() {
    let ctx = egui::Context::default();
    let mut v = wide_view();
    warm(&ctx, &mut v);
    let lay = strip(&v);
    assert!(
        *v.grid.details_offsets.last().unwrap() > v.grid.details_rect.width(),
        "fixture must overflow the strip"
    );
    let grid_scroll = v.grid.scroll;
    let over = lay.rows[0].center();
    frame(
        &ctx,
        &mut v,
        vec![
            egui::Event::PointerMoved(over),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0.0, -3.0),
                phase: egui::TouchPhase::Move,
                modifiers: Default::default(),
                source: egui::MouseWheelSource::Wheel,
            },
        ],
    );
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.grid.details_scroll_x, 120.0, "3 lines × 40 pt");
    assert_eq!(v.grid.scroll, grid_scroll, "panes do not move");
}

#[test]
fn dragging_a_strip_boundary_resizes_only_the_strip() {
    let ctx = egui::Context::default();
    let mut v = wide_view();
    warm(&ctx, &mut v);
    let before = width(&v, 1);
    let grid_offsets = v.grid.offsets.clone();
    let y = strip(&v).header.center().y;
    let from = egui::pos2(boundary_x(&v, 1), y);
    drag(&ctx, &mut v, from, 40.0);
    assert_eq!(width(&v, 1), before + 40.0, "strip column B widened");
    assert_eq!(v.grid.offsets, grid_offsets, "grid widths unchanged");
    assert!(v.grid.details_resizing.is_none());
    // A later grid resize of another column still flows into the strip.
    let c_before = width(&v, 2);
    v.grid.resize_column(2, v.grid.offsets[2] + c_before + 25.0);
    frame(&ctx, &mut v, vec![]);
    assert_eq!(width(&v, 2), c_before + 25.0);
    assert_eq!(width(&v, 1), before + 40.0);
}

#[test]
fn double_clicking_a_strip_boundary_fits_the_current_row() {
    let ctx = egui::Context::default();
    let mut v = wide_view();
    warm(&ctx, &mut v);
    v.grid.details_widths.insert(1, 60.0);
    frame(&ctx, &mut v, vec![]);
    let p = egui::pos2(boundary_x(&v, 1), strip(&v).header.center().y);
    press_release(&ctx, &mut v, p, egui::PointerButton::Primary, false);
    press_release(&ctx, &mut v, p, egui::PointerButton::Primary, false);
    frame(&ctx, &mut v, vec![]);
    let fitted = width(&v, 1);
    // Row 1 right side is the longest text: LONG + "_v2".
    let chars = LONG.chars().count() + 3;
    assert!(fitted > chars as f32 * 6.0, "fitted {fitted}");
    assert!(fitted <= 1000.0);
    assert_eq!(
        v.grid.cursor.column, 0,
        "boundary double-click does not select"
    );
}

#[test]
fn clicking_the_strip_header_moves_the_cursor_on_the_active_side() {
    let ctx = egui::Context::default();
    let mut v = wide_view();
    warm(&ctx, &mut v);
    let p = egui::pos2(cell_point(&v, 0, 2).x, strip(&v).header.center().y);
    press_release(&ctx, &mut v, p, egui::PointerButton::Primary, false);
    assert_eq!(v.grid.cursor, Cursor { row: 0, column: 2 });
    assert_eq!(v.grid.active, 0);
}

#[test]
fn clicking_a_strip_cell_switches_side_focus_and_keys_continue() {
    let ctx = egui::Context::default();
    let mut v = wide_view();
    warm(&ctx, &mut v);
    let p = cell_point(&v, 1, 3);
    press_release(&ctx, &mut v, p, egui::PointerButton::Primary, false);
    assert_eq!(v.grid.active, 1);
    assert_eq!(v.grid.cursor, Cursor { row: 0, column: 3 });
    assert!(v.grid.selection_visible);
    frame(&ctx, &mut v, vec![]);
    frame(&ctx, &mut v, vec![key(egui::Key::ArrowRight, false)]);
    assert_eq!(v.grid.cursor.column, 4, "keyboard goes to the right grid");
    assert_eq!(v.grid.active, 1);
    // The right grid scrolled so the cursor cell is visible.
    let d = v.grid.columns.iter().position(|&c| c == 4).unwrap();
    let view = v.grid.pane_rects[1].width() - super::super::grid::GUTTER;
    assert!(v.grid.offsets[d] >= v.grid.scroll.x);
    assert!(v.grid.offsets[d + 1] <= v.grid.scroll.x + view + 1.0);
}

#[test]
fn shift_click_extends_the_selection_within_the_row() {
    let ctx = egui::Context::default();
    let mut v = wide_view();
    warm(&ctx, &mut v);
    let p = cell_point(&v, 0, 1);
    press_release(&ctx, &mut v, p, egui::PointerButton::Primary, false);
    let p = cell_point(&v, 0, 3);
    press_release(&ctx, &mut v, p, egui::PointerButton::Primary, true);
    assert_eq!(v.grid.anchor, Some(Cursor { row: 0, column: 1 }));
    assert_eq!(v.grid.cursor, Cursor { row: 0, column: 3 });
    for c in 1..=3 {
        assert!(v.grid.selected(0, c));
    }
    assert!(!v.grid.selected(0, 4));
    // The strip paints the selected cells of the active side in the selection color.
    let out = frame(&ctx, &mut v, vec![]);
    let lay = strip(&v);
    let selected = out
        .shapes
        .iter()
        .filter(|s| match &s.shape {
            egui::Shape::Rect(r) => {
                r.fill == super::super::grid::SELECTION && lay.rows[0].contains(r.rect.center())
            }
            _ => false,
        })
        .count();
    assert_eq!(selected, 3);
}

#[test]
fn double_click_edits_and_right_click_opens_the_side_menu() {
    let ctx = egui::Context::default();
    let mut v = view();
    warm(&ctx, &mut v);
    // Double-click left strip cell (column B): Modify in the left grid.
    let p = cell_point(&v, 0, 1);
    press_release(&ctx, &mut v, p, egui::PointerButton::Primary, false);
    press_release(&ctx, &mut v, p, egui::PointerButton::Primary, false);
    let editor = v
        .editor
        .as_ref()
        .expect("double-click opens the cell editor");
    assert_eq!((editor.side, editor.display.column), (0, 1));
    v.editor = None;
    frame(&ctx, &mut v, vec![]);
    // Right click on the right strip row, column C.
    let p = cell_point(&v, 1, 2);
    press_release(&ctx, &mut v, p, egui::PointerButton::Secondary, false);
    assert_eq!(v.grid.active, 1);
    assert_eq!(v.grid.cursor.column, 2);
    frame(&ctx, &mut v, vec![]);
    let out = frame(&ctx, &mut v, vec![]);
    assert!(
        has(&out, "Copy to Left"),
        "table context menu for the right side"
    );
}

#[test]
fn align_with_pending_accepts_a_click_on_the_strip() {
    let mut v = TableCompareView::from_sheets(
        sheet("id,n\n1,a\n2,b\n"),
        sheet("id,n\n3,c\n4,d\n"),
        TableRules {
            columns: bc_engine_table::columns::ColumnMapping::by_number(2, 2),
            keys: KeySelection::Manual(vec![0]),
            ..Default::default()
        },
    );
    ready(&mut v);
    let ctx = egui::Context::default();
    warm(&ctx, &mut v);
    // Left source row 0 waits for a partner; display row 1 is right "3".
    v.align_with = Some((0, 0));
    v.grid.cursor = Cursor { row: 1, column: 0 };
    frame(&ctx, &mut v, vec![]);
    let p = cell_point(&v, 1, 0);
    press_release(&ctx, &mut v, p, egui::PointerButton::Primary, false);
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().map.overrides().pairs(), &[(0, 0)]);
    assert_eq!(v.grid.active, 1);
}

#[test]
fn cursor_column_change_scrolls_the_strip_and_manual_scroll_stays() {
    let ctx = egui::Context::default();
    let mut v = wide_view();
    warm(&ctx, &mut v);
    assert_eq!(v.grid.details_scroll_x, 0.0);
    // Keyboard in the grid moves the cursor to the last column.
    let p = cell_point(&v, 0, 0);
    press_release(&ctx, &mut v, p, egui::PointerButton::Primary, false);
    frame(&ctx, &mut v, vec![]);
    frame(&ctx, &mut v, vec![key(egui::Key::End, false)]);
    for _ in 0..12 {
        frame(&ctx, &mut v, vec![key(egui::Key::ArrowRight, false)]);
    }
    frame(&ctx, &mut v, vec![]);
    let last = v.grid.columns.len() - 1;
    assert_eq!(v.grid.cursor.column, v.grid.columns[last]);
    let view = v.grid.details_rect.right() - strip(&v).left;
    let right = v.grid.details_offsets[last + 1];
    assert!(
        (v.grid.details_scroll_x - (right - view)).abs() < 0.5,
        "minimal scroll shows the last column"
    );
    // Dragging the scrollbar back to the start is not pulled back.
    v.grid.details_scroll_x = 0.0;
    frame(&ctx, &mut v, vec![]);
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.grid.details_scroll_x, 0.0);
}

#[test]
fn hover_shows_full_text_only_for_truncated_cells() {
    let ctx = egui::Context::default();
    ctx.global_style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    let mut v = wide_view();
    warm(&ctx, &mut v);
    ctx.global_style_mut(|s| {
        s.interaction.tooltip_delay = 0.0;
        s.interaction.show_tooltips_only_when_still = false;
    });
    v.grid.details_widths.insert(1, 150.0);
    frame(&ctx, &mut v, vec![]);
    let idle = frame(&ctx, &mut v, vec![]);
    let baseline = count_text(&idle, LONG);
    let p = cell_point(&v, 0, 1);
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(p)]);
    let out = frame(&ctx, &mut v, vec![egui::Event::PointerMoved(p)]);
    assert_eq!(
        count_text(&out, LONG),
        baseline + 1,
        "tooltip with the full text"
    );
    // A fully visible single-line cell (column A, "1") shows no tooltip.
    let short = cell_point(&v, 0, 0);
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(short)]);
    let out = frame(&ctx, &mut v, vec![egui::Event::PointerMoved(short)]);
    assert_eq!(count_text(&out, "1"), count_text(&idle, "1"));
    // Tooltip text cap.
    let huge = "x".repeat(5000);
    let capped = details::tooltip_text(&huge);
    assert_eq!(capped.chars().count(), 4001);
    assert!(capped.ends_with('…'));
    assert_eq!(details::tooltip_text("a\nb"), "a\nb");
}
