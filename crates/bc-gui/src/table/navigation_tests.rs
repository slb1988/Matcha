use super::super::cache::projected;
use super::*;
use bc_engine_table::{columns::ColumnMapping, compare::Importance};

fn nav_view() -> TableCompareView {
    let mut rules = TableRules {
        columns: ColumnMapping::by_number(3, 3),
        keys: KeySelection::Manual(vec![0]),
        strategy: AlignStrategy::Quick,
        ..Default::default()
    };
    rules.columns.0[1].handling = ColumnHandling {
        use_default: false,
        ignore_case: true,
        ..Default::default()
    };
    let mut v = TableCompareView::from_sheets(
        sheet("id,text,n\n0,base,0\n1,Apple,1\n2,old,2\n3,same,3\n4,left,4\n6,end,6\n"),
        sheet("id,text,n\n0,base,0\n1,apple,1\n2,new,3\n3,same,3\n5,right,5\n6,end,6\n"),
        rules,
    );
    // Fixture starts from the origin; first-difference landing is tested separately.
    v.land_on_first_diff = false;
    ready(&mut v);
    v
}
/// Wave 8 moved the old toolbar rows into the classic menus/toolbar; the
/// legacy labels used by these tests resolve to their new entry paths.
fn legacy_path(label: &str) -> Vec<&str> {
    match label {
        "Differences" => vec!["Diffs"],
        "None" => vec!["View", "Show None"],
        "Ignore Unimportant" => vec!["Minor"],
        "Go" => vec!["Search", "Go To...", "Go"],
        "Hide Same Columns" => vec!["View", "Hide Same Columns"],
        "Over-Under" => vec!["View", "Over-Under Layout"],
        "+ Formulas" => vec!["View", "Compare Values + Formulas"],
        "+ Number formats" => vec!["View", "Compare Values + Formulas + Number Formats"],
        "Values" => vec!["View", "Compare Values"],
        other => vec![other],
    }
}
fn choose(ctx: &egui::Context, v: &mut TableCompareView, label: &str) {
    for label in legacy_path(label) {
        frame(ctx, v, vec![]);
        let out = frame(ctx, v, vec![]);
        click(ctx, v, button(&out, label));
    }
}
fn search(ctx: &egui::Context, v: &mut TableCompareView, label: &str) {
    choose(ctx, v, "Search");
    choose(ctx, v, label);
}
fn rows(v: &TableCompareView) -> Vec<usize> {
    let p = v.pair.as_ref().unwrap();
    (0..p.projection.len())
        .map(|i| projected(&p.projection, i).unwrap())
        .collect()
}

#[test]
fn difference_cell_and_row_commands_visit_row_major_targets_and_stop_at_both_ends() {
    let mut v = nav_view();
    let ctx = egui::Context::default();
    let cells = [
        (1, 1),
        (2, 1),
        (2, 2),
        (4, 0),
        (4, 1),
        (4, 2),
        (5, 0),
        (5, 1),
        (5, 2),
    ];
    for &(row, column) in &cells {
        search(&ctx, &mut v, "Next Difference Cell");
        assert_eq!(v.grid.cursor, Cursor { row, column });
    }
    let last = v.grid.cursor;
    search(&ctx, &mut v, "Next Difference Cell");
    assert_eq!(v.grid.cursor, last);
    assert!(v.not_found.is_shown());
    v.grid.cursor = Cursor { row: 6, column: 2 };
    for &(row, column) in cells.iter().rev() {
        search(&ctx, &mut v, "Previous Difference Cell");
        assert_eq!(v.grid.cursor, Cursor { row, column });
    }
    search(&ctx, &mut v, "Previous Difference Cell");
    assert_eq!(v.grid.cursor, Cursor { row: 1, column: 1 });
    assert!(v.not_found.is_shown());
    v.grid.cursor = Cursor::default();
    for row in [1, 2, 4, 5] {
        search(&ctx, &mut v, "Next Difference Row");
        assert_eq!(v.grid.cursor.row, row);
    }
    search(&ctx, &mut v, "Next Difference Row");
    assert!(v.not_found.is_shown());
    v.grid.cursor.row = 6;
    for row in [5, 4, 2, 1] {
        search(&ctx, &mut v, "Previous Difference Row");
        assert_eq!(v.grid.cursor.row, row);
    }
    search(&ctx, &mut v, "Previous Difference Row");
    assert!(v.not_found.is_shown());
}

#[test]
fn all_four_filters_and_ignore_project_counts_without_changing_raw_states() {
    let mut v = nav_view();
    let ctx = egui::Context::default();
    for (label, expected) in [
        ("Differences", vec![1, 2, 4, 5]),
        ("Same", vec![0, 3, 6]),
        ("None", vec![]),
        ("All", (0..7).collect()),
    ] {
        choose(&ctx, &mut v, label);
        assert_eq!(rows(&v), expected);
    }
    choose(&ctx, &mut v, "Ignore Unimportant");
    ready(&mut v);
    for (label, expected) in [
        ("Differences", vec![2, 4, 5]),
        ("Same", vec![0, 1, 3, 6]),
        ("None", vec![]),
        ("All", (0..7).collect()),
    ] {
        choose(&ctx, &mut v, label);
        assert_eq!(rows(&v), expected);
    }
    let p = v.pair.as_ref().unwrap();
    assert_eq!(p.cache.statistics.same, 4);
    assert_eq!(p.cache.statistics.unimportant, 0);
    assert_eq!(
        p.map.cell_diff(1, 1).unwrap().value,
        Importance::Unimportant
    );
    v.grid.cursor = Cursor::default();
    search(&ctx, &mut v, "Next Difference Cell");
    assert_eq!(v.grid.cursor, Cursor { row: 2, column: 1 });
    search(&ctx, &mut v, "Previous Difference Cell");
    assert!(v.not_found.is_shown());
    choose(&ctx, &mut v, "Ignore Unimportant");
    ready(&mut v);
    search(&ctx, &mut v, "Previous Difference Cell");
    assert_eq!(v.grid.cursor, Cursor { row: 1, column: 1 });
}

#[test]
fn go_to_reveals_filtered_rows_and_hidden_columns_and_clamps_bounds() {
    let mut v = nav_view();
    let ctx = egui::Context::default();
    choose(&ctx, &mut v, "Differences");
    v.goto = [4, 2]; // same comparison row hidden by Differences
    choose(&ctx, &mut v, "Go");
    assert_eq!(v.grid.cursor, Cursor { row: 3, column: 1 });
    assert_eq!(v.filter, Filter::All);
    assert!(rows(&v).contains(&3));
    choose(&ctx, &mut v, "Same");
    v.goto = [7, 3];
    choose(&ctx, &mut v, "Go");
    assert_eq!(v.filter, Filter::Same); // no need to disturb a compatible filter
    assert_eq!(v.grid.cursor, Cursor { row: 6, column: 2 });
    choose(&ctx, &mut v, "None");
    v.goto = [usize::MAX, usize::MAX];
    choose(&ctx, &mut v, "Go");
    assert_eq!(v.grid.cursor, Cursor { row: 6, column: 2 });
    assert_eq!(v.goto, [7, 3]);
    assert_eq!(v.filter, Filter::All);
    assert!(!v.message.contains("No more"));

    let mut v = view();
    let ctx = egui::Context::default();
    choose(&ctx, &mut v, "Hide Same Columns");
    assert!(!v.grid.columns.contains(&0));
    v.goto = [1, 1];
    choose(&ctx, &mut v, "Go");
    assert_eq!(v.grid.cursor, Cursor::default());
    assert!(v.grid.columns.contains(&0));
    assert!(!v.hide_same);
}

#[test]
fn edit_navigation_reveals_same_rows_and_follows_undo_redo_marks() {
    let mut v = nav_view();
    v.begin_edit(1, Cursor { row: 3, column: 1 }, Some("changed".into()));
    v.commit_edit().unwrap();
    ready(&mut v);
    v.grid.active = 0;
    v.grid.cursor = Cursor { row: 1, column: 1 };
    v.copy(false); // makes row 1 Same but still an edited row
    ready(&mut v);
    let ctx = egui::Context::default();
    choose(&ctx, &mut v, "Differences");
    assert!(!rows(&v).contains(&1));
    v.grid.cursor = Cursor::default();
    search(&ctx, &mut v, "Next Edit");
    assert_eq!(v.grid.cursor.row, 1);
    assert!(rows(&v).contains(&1));
    assert_eq!(v.filter, Filter::All);
    search(&ctx, &mut v, "Next Edit");
    assert_eq!(v.grid.cursor.row, 3);
    search(&ctx, &mut v, "Next Edit");
    assert!(v.not_found.is_shown());
    search(&ctx, &mut v, "Previous Edit");
    assert_eq!(v.grid.cursor.row, 1);
    search(&ctx, &mut v, "Previous Edit");
    assert!(v.not_found.is_shown());
    v.undo(false);
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().cache.edited_rows, [3]);
    v.undo(true);
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().cache.edited_rows, [1, 3]);
}

#[test]
fn navigation_scrolls_to_a_distant_cell_in_both_dimensions() {
    let mut left = Sheet::default();
    for row in 0..1000 {
        left.push_row((0..40).map(|c| Cell::text(format!("R{row}C{c}"))).collect());
    }
    let mut right = left.clone();
    right.set_cell(900, 35, Cell::text("changed")).unwrap();
    let mut v = TableCompareView::from_sheets(
        left,
        right,
        TableRules {
            columns: ColumnMapping::by_number(40, 40),
            keys: KeySelection::Manual(vec![0]),
            strategy: AlignStrategy::Quick,
            ..Default::default()
        },
    );
    v.land_on_first_diff = false;
    ready(&mut v);
    let ctx = egui::Context::default();
    choose(&ctx, &mut v, "None");
    search(&ctx, &mut v, "Next Difference Cell");
    assert_eq!(
        v.grid.cursor,
        Cursor {
            row: 900,
            column: 35
        }
    );
    let out = frame(&ctx, &mut v, vec![]);
    assert!(v.grid.visible.contains(&900));
    assert!(v.grid.scroll.x > 0.0);
    assert!(out
        .shapes
        .iter()
        .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "R900C35")));
    assert!(!ctx.tessellate(out.shapes, out.pixels_per_point).is_empty());
}

#[test]
fn empty_same_and_stale_tables_never_navigate_to_phantom_cells() {
    let empty = read_text(
        IoCursor::new("id,n\n"),
        &ReadOptions {
            header: Some(bc_formats::model::HeaderMode::Names),
            ..Default::default()
        },
        &NeverCancel,
    )
    .unwrap()
    .sheet;
    for source in [empty, sheet("id,n\n1,same\n2,same\n")] {
        let mut v = TableCompareView::from_sheets(
            source.clone(),
            source,
            TableRules {
                keys: KeySelection::Manual(vec![0]),
                ..Default::default()
            },
        );
        ready(&mut v);
        for forward in [false, true] {
            for kind in 0..3 {
                let old = v.grid.cursor;
                v.navigate(forward, kind);
                assert_eq!(v.grid.cursor, old);
                assert!(v.not_found.is_shown());
                assert!(!v.message.contains("No more"));
            }
        }
    }
    let mut v = nav_view();
    v.pair
        .as_ref()
        .unwrap()
        .map
        .rules_mut()
        .default_handling
        .ignore_whitespace = true;
    let ctx = egui::Context::default();
    v.goto = [7, 3];
    choose(&ctx, &mut v, "Go");
    assert_eq!(v.grid.cursor, Cursor::default());
    assert!(v.message.contains("Wait"));
    v.navigate(true, 0);
    assert_eq!(v.grid.cursor, Cursor::default());
}

#[test]
fn navigation_past_the_ends_shows_the_panel_wraps_or_stays_silent() {
    use bc_session::prefs::NextDiffPrefs;
    let ctx = egui::Context::default();
    let mut v = nav_view();
    frame(&ctx, &mut v, vec![]);
    let diff_rows = v.pair.as_ref().unwrap().cache.diff_rows.clone();
    let (first, last) = (diff_rows[0], *diff_rows.last().unwrap());
    // 快捷键 Ctrl+N（Next Difference Row）走到最后一个差异行，再按一次弹面板、光标不动。
    for _ in 0..diff_rows.len() {
        frame(&ctx, &mut v, vec![key(egui::Key::N, true)]);
    }
    assert_eq!(v.grid.cursor.row, last);
    assert!(!v.not_found.is_shown());
    let out = frame(&ctx, &mut v, vec![key(egui::Key::N, true)]);
    assert_eq!(v.grid.cursor.row, last);
    assert!(v.not_found.is_shown());
    // 面板画在状态栏上方，状态栏不再出现旧提示。
    let panel = button(&out, crate::chrome::message_panel::NOT_FOUND);
    assert!(panel.y < 900.0 - 25.0);
    assert!(!v.message.contains("No more"));
    // 下一次按键关闭面板。
    frame(&ctx, &mut v, vec![key(egui::Key::ArrowLeft, false)]);
    assert!(!v.not_found.is_shown());
    // 菜单命令 id 同一路径：Difference Cell 一直向后直到越过末端。
    for _ in 0..64 {
        let before = v.grid.cursor;
        v.command("Next Difference Cell", &ctx, &Prefs::default());
        if v.not_found.is_shown() {
            assert_eq!(v.grid.cursor, before);
            break;
        }
    }
    assert!(v.not_found.is_shown());
    // 回绕：向下回到第一个差异行，向上回到最后一个；不弹面板。
    let mut wrap = Prefs::default();
    wrap.next_diff.wrap_around = true;
    frame(&ctx, &mut v, vec![key(egui::Key::ArrowLeft, false)]);
    v.next_diff = wrap.next_diff;
    v.navigate(true, 1);
    assert_eq!(v.grid.cursor.row, first);
    assert!(!v.not_found.is_shown());
    v.navigate(false, 1);
    assert_eq!(v.grid.cursor.row, last);
    // Difference Cell：先走到最后一个差异格，再回绕到第一个差异行的第一个差异格。
    v.next_diff = NextDiffPrefs::default();
    while !v.not_found.is_shown() {
        v.navigate(true, 0);
    }
    v.not_found = Default::default();
    v.next_diff = wrap.next_diff;
    v.navigate(true, 0);
    let wrapped = v.grid.cursor;
    let p = v.pair.as_ref().unwrap();
    assert_eq!(wrapped.row, first);
    assert!(p
        .map
        .cell_importance(wrapped.row, wrapped.column)
        .is_some_and(|i| i != Importance::Same));
    assert!((0..wrapped.column).all(|c| p
        .map
        .cell_importance(first, c)
        .is_none_or(|i| i == Importance::Same)));
    // 没有编辑：开启回绕也找不到，仍弹面板。
    v.navigate(true, 2);
    assert!(v.not_found.is_shown());
    // 关闭面板：静默，光标不动。
    frame(&ctx, &mut v, vec![key(egui::Key::ArrowLeft, false)]);
    v.next_diff = NextDiffPrefs {
        message_panel: false,
        wrap_around: false,
    };
    let old = v.grid.cursor;
    v.navigate(true, 2);
    assert_eq!(v.grid.cursor, old);
    assert!(!v.not_found.is_shown());
}

#[test]
fn opening_lands_on_the_first_difference_cell_and_scrolls_to_it() {
    let mut left = Sheet::default();
    for row in 0..1000 {
        left.push_row((0..40).map(|c| Cell::text(format!("R{row}C{c}"))).collect());
    }
    let mut right = left.clone();
    right.set_cell(900, 35, Cell::text("changed")).unwrap();
    right.set_cell(950, 2, Cell::text("later")).unwrap();
    let mut v = TableCompareView::from_sheets(
        left,
        right,
        TableRules {
            columns: ColumnMapping::by_number(40, 40),
            keys: KeySelection::Manual(vec![0]),
            strategy: AlignStrategy::Quick,
            ..Default::default()
        },
    );
    ready(&mut v);
    assert_eq!(
        v.grid.cursor,
        Cursor {
            row: 900,
            column: 35
        }
    );
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    assert!(v.grid.visible.contains(&900));
    assert!(v.grid.scroll.x > 0.0);
    // One-shot: a later recompare keeps the user's position.
    v.goto_cell(Cursor::default());
    v.recompare();
    ready(&mut v);
    assert_eq!(v.grid.cursor, Cursor::default());
}

#[test]
fn opening_identical_tables_stays_on_the_origin_and_a_moved_cursor_is_kept() {
    let same = sheet("id,n\n1,a\n2,b\n");
    let mut v = TableCompareView::from_sheets(
        same.clone(),
        same,
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            ..Default::default()
        },
    );
    ready(&mut v);
    assert_eq!(v.grid.cursor, Cursor::default());
    let mut v = TableCompareView::from_sheets(
        sheet("id,n\n1,a\n2,b\n"),
        sheet("id,n\n1,a\n2,c\n"),
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            ..Default::default()
        },
    );
    // The user moved before the comparison finished: never yank the cursor.
    v.grid.cursor = Cursor { row: 1, column: 0 };
    ready(&mut v);
    assert_eq!(v.grid.cursor, Cursor { row: 1, column: 0 });
}
