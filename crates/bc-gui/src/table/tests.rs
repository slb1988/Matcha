use super::{
    cache::Filter,
    find::Action,
    grid::{Cursor, GridState},
    view::*,
};
use bc_core::{align::AlignStrategy, cancel::NeverCancel};
use bc_engine_table::{
    rules::{ColumnHandling, KeySelection, Layers},
    source::{read_text, ReadOptions},
    Cell, Sheet, TableRules,
};
use bc_session::prefs::Prefs;
use eframe::egui;
use std::{
    io::Cursor as IoCursor,
    path::Path,
    time::{Duration, Instant},
};
#[path = "command_tests.rs"]
mod command_matrix;
#[path = "copy_tests.rs"]
mod copying;
#[path = "details_tests.rs"]
mod details_strip;
#[path = "edit_tests.rs"]
mod editing;
#[path = "eval_tests.rs"]
mod eval;
#[path = "lazy_tests.rs"]
mod lazy;
#[path = "navigation_tests.rs"]
mod navigation;
#[path = "projection_tests.rs"]
mod projections;
#[path = "review_tests.rs"]
mod review;
#[path = "search_tests.rs"]
mod search;
#[path = "session_tests.rs"]
mod session;
#[path = "sheet_tests.rs"]
mod sheets;
#[path = "sort_tests.rs"]
mod sorting;
#[path = "view_tests.rs"]
mod view_features;

fn sheet(text: &str) -> Sheet {
    read_text(IoCursor::new(text), &ReadOptions::default(), &NeverCancel)
        .unwrap()
        .sheet
}
fn view() -> TableCompareView {
    let mut v = TableCompareView::from_sheets(
        sheet("id,name,price\n7,apple,1.0\n8,pear,2.0\n"),
        sheet("id,name,price\n7,apple,1.2\n8,PEAR,2.1\n"),
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
fn ready(v: &mut TableCompareView) {
    let start = Instant::now();
    while !v.is_ready() {
        v.poll();
        assert!(start.elapsed() < Duration::from_secs(15), "{}", v.message);
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn frame(
    ctx: &egui::Context,
    v: &mut TableCompareView,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            events,
            ..Default::default()
        },
        |ui| {
            v.ui(ui, &mut Prefs::default());
        },
    );
    output.textures_delta.clear();
    output
}
fn key(key: egui::Key, command: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers {
            ctrl: command,
            command,
            ..Default::default()
        },
    }
}
fn click(ctx: &egui::Context, v: &mut TableCompareView, p: egui::Pos2) {
    for pressed in [true, false] {
        frame(
            ctx,
            v,
            vec![
                egui::Event::PointerMoved(p),
                egui::Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
}
fn button(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
    output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == label => {
                Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center())
            }
            _ => None,
        })
        // Topmost (modal) layer paints last.
        .last()
        .unwrap_or_else(|| panic!("missing button {label}"))
}
fn no_backup_prefs() -> Prefs {
    Prefs {
        backups: bc_session::prefs::BackupPrefs {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn temp_file(name: &str, content: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("matcha-wave7-{}-{name}", std::process::id()));
    std::fs::write(&path, content).unwrap();
    path
}
fn open_files(left: &Path, right: &Path) -> TableCompareView {
    let mut v = TableCompareView::open_with_registry(
        left,
        right,
        &bc_formats::registry::FormatRegistry::with_builtins(),
    )
    .unwrap();
    ready(&mut v);
    v
}
#[test]
fn grid_tessellation_cell_colours_two_dimensional_virtualization_and_stacked_sync() {
    let mut l = Sheet::default();
    for r in 0..1000 {
        l.push_row((0..100).map(|c| Cell::text(format!("R{r}C{c}"))).collect());
    }
    let mut r = l.clone();
    r.set_cell(700, 70, Cell::text("changed")).unwrap();
    let mut v = TableCompareView::from_sheets(
        l,
        r,
        TableRules {
            strategy: AlignStrategy::None,
            ..Default::default()
        },
    );
    ready(&mut v);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    v.scroll_to_row(700);
    v.scroll_to_column(70);
    let out = frame(&ctx, &mut v, vec![]);
    // 2 panes × ~35 visible rows × ~11 visible 61 px columns of 100 000 cells.
    assert!(v.grid.cells_drawn < 1200);
    assert!(v.grid.rows_drawn < 100);
    assert!(!ctx.tessellate(out.shapes, out.pixels_per_point).is_empty());
    assert_eq!(v.grid.visible.start, 700);
    v.grid.stacked = true;
    frame(&ctx, &mut v, vec![]);
    assert!(v.grid.pane_rects[0].bottom() <= v.grid.pane_rects[1].top());
    assert_eq!(
        v.pair.as_ref().unwrap().map.row(700).unwrap().left,
        Some(700)
    );
}
#[test]
fn navigation_filter_ignore_and_cell_copy_are_background_then_undoable() {
    let mut v = view();
    v.navigate(true, 0);
    assert_eq!(v.grid.cursor, Cursor { row: 0, column: 2 });
    v.copy(false);
    assert!(v.pair.as_ref().unwrap().map.is_stale());
    let start = Instant::now();
    ready(&mut v);
    assert!(start.elapsed() < Duration::from_millis(700));
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.same, 1);
    v.undo(false);
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.important, 2);
    {
        let p = v.pair.as_ref().unwrap();
        let mut rules = p.map.rules_mut();
        for c in &mut rules.columns.0 {
            c.handling = ColumnHandling {
                unimportant: true,
                ..Default::default()
            };
        }
    }
    v.recompare();
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.unimportant, 2);
    v.pair.as_ref().unwrap().map.rules_mut().ignore_unimportant = true;
    v.project();
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.same, 2);
    v.filter = Filter::Differences;
    v.pair.as_mut().unwrap().projection = v.pair.as_ref().unwrap().cache.project(v.filter);
    assert_eq!(v.row_count(), 0);
}
#[test]
fn raw_pointer_modify_commit_undo_and_find_replace() {
    let ctx = egui::Context::default();
    let mut v = view();
    frame(&ctx, &mut v, vec![]);
    let pos = v.grid.pane_rects[1].left_top() + egui::vec2(80.0, 10.0);
    click(&ctx, &mut v, pos);
    assert_eq!(v.grid.active, 1);
    frame(&ctx, &mut v, vec![key(egui::Key::F2, false)]);
    assert!(v.editor.is_some());
    frame(
        &ctx,
        &mut v,
        vec![key(egui::Key::A, true), egui::Event::Text("7".into())],
    );
    let out = frame(&ctx, &mut v, vec![]);
    let commit = button(&out, "Commit (Ctrl+Enter)");
    click(&ctx, &mut v, commit);
    ready(&mut v);
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .right
            .borrow()
            .display_at(0, 0)
            .unwrap(),
        "7"
    );
    v.find.query = "pear".into();
    v.find.side = 1;
    v.find.case = false;
    v.find_action(Action::Next).unwrap();
    assert_eq!(v.grid.cursor, Cursor { row: 1, column: 1 });
    v.find.replacement = "梨".into();
    v.find_action(Action::All).unwrap();
    ready(&mut v);
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .right
            .borrow()
            .display_at(1, 1)
            .unwrap(),
        "梨"
    );
    v.undo(false);
    ready(&mut v);
    assert_eq!(
        v.pair
            .as_ref()
            .unwrap()
            .map
            .right
            .borrow()
            .display_at(1, 1)
            .unwrap(),
        "PEAR"
    );
}
#[test]
fn structures_saved_fields_and_independent_history() {
    let mut v = view();
    v.grid.active = 1;
    v.grid.cursor = Cursor { row: 1, column: 1 };
    v.structure(0).unwrap();
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().map.right.borrow().row_count(), 3);
    v.undo(false);
    ready(&mut v);
    v.structure(3).unwrap();
    ready(&mut v);
    assert_eq!(
        v.pair.as_ref().unwrap().map.right.borrow().column_count(),
        4
    );
    v.undo(false);
    ready(&mut v);
    assert_eq!(
        v.pair.as_ref().unwrap().map.right.borrow().column_count(),
        3
    );
    let path =
        std::env::temp_dir().join(format!("matcha-table-gui-save-{}.csv", std::process::id()));
    v.begin_edit(1, Cursor { row: 0, column: 2 }, Some("4.00".into()));
    v.commit_edit().unwrap();
    v.save_to(
        1,
        &path,
        &Prefs {
            backups: bc_session::prefs::BackupPrefs {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        },
        false,
    )
    .unwrap();
    ready(&mut v);
    assert!(std::fs::read_to_string(&path)
        .unwrap()
        .contains("7,apple,4.00"));
    assert!(!v.pair.as_ref().unwrap().history[1].is_dirty());
    std::fs::remove_file(path).unwrap();
}
#[test]
fn layer_controls_preserve_raw_states_details_and_readonly_commands() {
    let mut l = sheet("id,value\n1,6\n");
    let mut r = l.clone();
    let mut cell = Cell::number(6.0);
    cell.formula = Some("=1+5".into());
    l.set_cell(0, 1, cell.clone()).unwrap();
    cell.formula = Some("=SUM(1,5)".into());
    r.set_cell(0, 1, cell).unwrap();
    let mut v = TableCompareView::from_sheets(
        l,
        r,
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            ..Default::default()
        },
    );
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.same, 1);
    v.pair.as_ref().unwrap().map.rules_mut().layers = Layers::ValuesAndFormulas;
    v.project();
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().cache.statistics.important, 1);
    v.grid.cursor = Cursor { row: 0, column: 1 };
    v.show_details = true;
    let ctx = egui::Context::default();
    let out = frame(&ctx, &mut v, vec![]);
    assert!(out
        .shapes
        .iter()
        .any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.text().contains("=SUM(1,5)"))));
    v.files[1].readonly = true;
    v.begin_edit(1, v.grid.cursor, None);
    assert!(v.editor.is_none());
    v.grid.active = 0;
    v.copy(false);
    assert!(v.message.contains("disabled"));
}
#[test]
fn home_masks_and_unsaved_cancel_do_not_discard() {
    for ext in ["csv", "TSV", "xlsx", "XLSM"] {
        assert!(crate::home::table_paths(
            Path::new(&format!("a.{ext}")),
            Path::new("b.txt")
        ));
    }
    assert!(!crate::home::table_paths(
        Path::new("a.txt"),
        Path::new("b.cpp")
    ));
    let mut v = view();
    v.begin_edit(1, Cursor { row: 0, column: 1 }, Some("dirty".into()));
    v.commit_edit().unwrap();
    v.request_close();
    assert!(v.pending.is_some());
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    let out = frame(&ctx, &mut v, vec![]);
    let cancel = button(&out, "Cancel");
    click(&ctx, &mut v, cancel);
    assert!(v.pending.is_none());
    assert!(v.is_dirty());
    assert!(v.action.is_none());
    let _ = GridState::default();
}

fn tall_view() -> TableCompareView {
    let mut left = String::from(
        "id,name,code,note,tag
",
    );
    for i in 0..5000 {
        left.push_str(&format!(
            "{i},item-{i}-padding-abcdefghijklmnopqrstuvwxyz-0123456789-abcdefghijklmnopqrstuvwxyz,code-{i}-padding-abcdefghijklmnopqrstuvwxyz-0123456789-abcdefghijklmnopqrstuvwxyz,note-{i}-padding-abcdefghijklmnopqrstuvwxyz-0123456789-abcdefghijklmnopqrstuvwxyz,tag-{i}-padding-abcdefghijklmnopqrstuvwxyz-0123456789-abcdefghijklmnopqrstuvwxyz
"
        ));
    }
    let right = left.clone();
    let mut v = TableCompareView::from_sheets(
        sheet(&left),
        sheet(&right),
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            strategy: AlignStrategy::Quick,
            ..Default::default()
        },
    );
    ready(&mut v);
    v
}
fn wheel(lines: Vec2, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: lines,
        modifiers,
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
    }
}
use egui::Vec2;

/// The grid consumes raw Line/Page wheel deltas 1:1 in the frame they
/// arrive (egui's exponential wheel smoothing is bypassed on the grid).
#[test]
fn wheel_scroll_tracks_input_immediately() {
    let mut v = tall_view();
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    let pane0 = v.grid.pane_rects[0];
    let pos = egui::pos2(pane0.left() + 100.0, pane0.top() + 100.0);
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(pos)]);
    assert_eq!(v.grid.scroll.y, 0.0);
    // 10 wheel-down notches of 3 lines in one frame: fully applied at once.
    let mut events = vec![egui::Event::PointerMoved(pos)];
    for _ in 0..10 {
        events.push(wheel(egui::vec2(0.0, -3.0), egui::Modifiers::default()));
    }
    frame(&ctx, &mut v, events);
    let target = 10.0 * 3.0 * 40.0;
    assert_eq!(
        v.grid.scroll.y, target,
        "full delta applies in the same frame"
    );
    // No residual motion afterwards (no smoothing backlog).
    for _ in 0..10 {
        frame(&ctx, &mut v, vec![egui::Event::PointerMoved(pos)]);
        assert_eq!(v.grid.scroll.y, target);
    }
    // Wheel up past the top clamps at zero.
    frame(
        &ctx,
        &mut v,
        vec![
            egui::Event::PointerMoved(pos),
            wheel(egui::vec2(0.0, 500.0), egui::Modifiers::default()),
        ],
    );
    assert_eq!(v.grid.scroll.y, 0.0);
}

#[test]
fn wheel_scroll_modifiers_and_hover_gating() {
    let mut v = tall_view();
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    let pane0 = v.grid.pane_rects[0];
    let pos = egui::pos2(pane0.left() + 100.0, pane0.top() + 100.0);
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(pos)]);
    // Ctrl+wheel is zoom in egui semantics: the grid does not scroll.
    frame(
        &ctx,
        &mut v,
        vec![
            egui::Event::PointerMoved(pos),
            wheel(egui::vec2(0.0, -3.0), egui::Modifiers::COMMAND),
        ],
    );
    assert_eq!(v.grid.scroll.y, 0.0);
    // Shift+wheel scrolls horizontally.
    frame(
        &ctx,
        &mut v,
        vec![
            egui::Event::PointerMoved(pos),
            wheel(egui::vec2(0.0, -3.0), egui::Modifiers::SHIFT),
        ],
    );
    assert_eq!(v.grid.scroll.y, 0.0);
    assert_eq!(v.grid.scroll.x, 120.0);
    // Pointer outside the grid panes: no grid scrolling.
    let outside = egui::pos2(5.0, 5.0);
    frame(
        &ctx,
        &mut v,
        vec![
            egui::Event::PointerMoved(outside),
            wheel(egui::vec2(0.0, -3.0), egui::Modifiers::default()),
        ],
    );
    assert_eq!(v.grid.scroll.x, 120.0);
    assert_eq!(v.grid.scroll.y, 0.0);
    // Page units convert through the viewport height.
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(pos)]);
    frame(
        &ctx,
        &mut v,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Page,
                delta: egui::vec2(0.0, -1.0),
                modifiers: egui::Modifiers::default(),
                phase: egui::TouchPhase::Move,
                source: egui::MouseWheelSource::Wheel,
            },
        ],
    );
    assert_eq!(v.grid.scroll.y, 900.0);
}

/// Keyboard navigation: arrows/PageUp/PageDown keep acting on the grid
/// (focus stays locked on the pane) and scroll the view so the cursor row
/// stays visible.
#[test]
fn arrow_keys_keep_focus_and_track_cursor_into_view() {
    let mut v = tall_view();
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    let pane0 = v.grid.pane_rects[0];
    let pos = egui::pos2(pane0.left() + 100.0, pane0.top() + 100.0);
    frame(
        &ctx,
        &mut v,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::default(),
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::default(),
            },
        ],
    );
    let start = v.grid.cursor.row;
    let mut events = vec![];
    for _ in 0..60 {
        events.push(egui::Event::Key {
            key: egui::Key::ArrowDown,
            physical_key: None,
            pressed: true,
            modifiers: egui::Modifiers::default(),
            repeat: false,
        });
    }
    frame(&ctx, &mut v, events);
    assert_eq!(
        v.grid.cursor.row,
        start + 60,
        "every arrow press moves the cursor"
    );
    assert!(
        v.key_focus,
        "grid keeps the keyboard across arrow keys (egui focus may still roam)"
    );
    assert!(
        v.grid.visible.contains(&v.grid.cursor.row),
        "cursor row {:?} visible in {:?}",
        v.grid.cursor,
        v.grid.visible
    );
    assert!(v.grid.scroll.y > 0.0, "view scrolled to follow the cursor");
    // ArrowUp back above the top scrolls up.
    let mut events = vec![];
    for _ in 0..100 {
        events.push(egui::Event::Key {
            key: egui::Key::ArrowUp,
            physical_key: None,
            pressed: true,
            modifiers: egui::Modifiers::default(),
            repeat: false,
        });
    }
    frame(&ctx, &mut v, events);
    assert_eq!(v.grid.cursor.row, 0);
    assert_eq!(v.grid.scroll.y, 0.0);
    // PageDown jumps by a page and lands in view.
    frame(
        &ctx,
        &mut v,
        vec![egui::Event::Key {
            key: egui::Key::PageDown,
            physical_key: None,
            pressed: true,
            modifiers: egui::Modifiers::default(),
            repeat: false,
        }],
    );
    let row = v.grid.cursor.row;
    assert!(row >= 10, "PageDown moved a page, got row {row}");
    assert!(v.grid.visible.contains(&row));
}

#[derive(Debug)]
struct DropPath(std::path::PathBuf);
impl egui::DroppedFile for DropPath {
    fn path(&self) -> &Path {
        &self.0
    }
    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|e| e.to_string())
    }
}
fn drop_frame(ctx: &egui::Context, v: &mut TableCompareView, pos: egui::Pos2, path: &Path) {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            events: vec![egui::Event::PointerMoved(pos)],
            dropped_files: vec![std::sync::Arc::new(DropPath(path.to_path_buf()))],
            ..Default::default()
        },
        |ui| {
            v.ui(ui, &mut Prefs::default());
        },
    );
    output.textures_delta.clear();
}
#[test]
fn os_drop_replaces_the_side_under_the_pointer_not_the_active_one() {
    let dir = std::env::temp_dir().join(format!("matcha-table-drop-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("dropped.csv");
    std::fs::write(&file, "id,name,price\n7,kiwi,9.0\n").unwrap();
    let ctx = egui::Context::default();
    let mut v = view();
    frame(&ctx, &mut v, vec![]);
    v.grid.active = 0;
    // Right path bar / column header: above the right grid body, outside
    // every pane rect (used to fall back to the active, left side).
    let pos = v.grid.pane_rects[1].center_top() - egui::vec2(0.0, 12.0);
    assert!(!v.grid.pane_rects.iter().any(|r| r.contains(pos)));
    // The settings draft open: the drop is ignored instead of racing it.
    v.settings.open = true;
    drop_frame(&ctx, &mut v, pos, &file);
    ready(&mut v);
    assert_eq!(v.paths()[1], None);
    v.settings.open = false;
    drop_frame(&ctx, &mut v, pos, &file);
    ready(&mut v);
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.paths()[1].as_deref(), Some(file.as_path()));
    assert_eq!(v.paths()[0], None);
    let _ = std::fs::remove_dir_all(&dir);
}
