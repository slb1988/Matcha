//! 4.4 Search 子项：Find/Replace 的真实输入回归。只钉用户可见行为；
//! 当前实现未满足的项以失败形式复现，修复后转绿。注册由主 worker 在 tests.rs 添加。
use super::*;

fn search_view() -> TableCompareView {
    let mut v = TableCompareView::from_sheets(
        sheet("id,name,price\n7,apple,1.0\n8,pear,2.0\n9,PEAR,3.0\n10,applepie,4.0\n"),
        sheet("id,name,price\n7,apple,1.2\n8,pear,2.1\n9,pear,3.1\n10,applepie,4.1\n"),
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            ..Default::default()
        },
    );
    ready(&mut v);
    v
}

fn cell(v: &TableCompareView, side: usize, row: usize, column: usize) -> String {
    let p = v.pair.as_ref().unwrap();
    let s = if side == 0 {
        p.map.left.borrow()
    } else {
        p.map.right.borrow()
    };
    s.display_at(row, column).unwrap()
}

fn dirty(v: &TableCompareView, side: usize) -> bool {
    v.pair.as_ref().unwrap().history[side].is_dirty()
}

fn menu(ctx: &egui::Context, v: &mut TableCompareView, label: &str) {
    frame(ctx, v, vec![]);
    let out = frame(ctx, v, vec![]);
    click(ctx, v, button(&out, label));
}

#[test]
fn ctrl_f_and_search_menu_open_panel_literal_next_locates_cell() {
    let ctx = egui::Context::default();
    let mut v = search_view();
    frame(&ctx, &mut v, vec![key(egui::Key::F, true)]);
    assert!(v.find.open, "Ctrl+F opens the find panel");
    let out = frame(&ctx, &mut v, vec![]);
    assert!(out
        .shapes
        .iter()
        .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Whole words")));
    v.find.open = false;
    menu(&ctx, &mut v, "Search");
    menu(&ctx, &mut v, "Find...");
    assert!(v.find.open, "Search menu reopens the find panel");

    v.find.query = "pear".into();
    v.find.side = 0;
    v.find_action(Action::Next).unwrap();
    assert_eq!(v.grid.cursor, Cursor { row: 1, column: 1 });
    assert_eq!(v.message, "Found at row 2, column 2");
    assert!(v.find.last.is_some());
    v.find_action(Action::Next).unwrap();
    assert_eq!(v.grid.cursor, Cursor { row: 2, column: 1 });
}

#[test]
fn regex_case_and_whole_word_matching() {
    let mut v = search_view();
    v.find.query = "pear".into();
    v.find.side = 0;
    // 大小写不敏感：pear 与 PEAR 都命中。
    v.find_action(Action::Next).unwrap();
    assert_eq!(v.grid.cursor.row, 1);
    v.find_action(Action::Next).unwrap();
    assert_eq!(v.grid.cursor.row, 2);
    v.find_action(Action::Next).unwrap();
    assert_eq!(v.message, "No more matches");
    // 大小写敏感后只剩小写。
    v.find.last = None;
    v.find.case = true;
    v.find_action(Action::Next).unwrap();
    assert_eq!(v.grid.cursor.row, 1);
    v.find_action(Action::Next).unwrap();
    assert_eq!(v.message, "No more matches");
    // 全词：applepie 不含独立词 apple（"apple pie" 中的 apple 反而是合法全词命中）。
    v.find = Default::default();
    v.find.query = "apple".into();
    v.find.whole = true;
    v.find_action(Action::Next).unwrap();
    assert_eq!(v.grid.cursor, Cursor { row: 0, column: 1 });
    v.find_action(Action::Next).unwrap();
    assert_eq!(v.message, "No more matches");
    // 正则：跨单元格内容模式。
    v.find = Default::default();
    v.find.query = "p.*r".into();
    v.find.regex = true;
    v.find_action(Action::Next).unwrap();
    assert_eq!(v.grid.cursor, Cursor { row: 1, column: 1 });
}

#[test]
fn next_previous_direction_sequence_and_boundaries() {
    let mut v = search_view();
    v.find.query = "apple".into();
    v.find.side = 0;
    v.find_action(Action::Next).unwrap();
    assert_eq!(v.grid.cursor.row, 0);
    v.find_action(Action::Next).unwrap();
    assert_eq!(v.grid.cursor.row, 3);
    v.find_action(Action::Previous).unwrap();
    assert_eq!(v.grid.cursor.row, 0);
    v.find_action(Action::Next).unwrap();
    assert_eq!(v.grid.cursor.row, 3);
    v.find_action(Action::Next).unwrap();
    assert_eq!(v.message, "No more matches");
    // Previous 回到上一个真实命中而不是起点之外。
    v.find_action(Action::Previous).unwrap();
    assert_eq!(v.grid.cursor.row, 0);
}

#[test]
fn side_selection_limits_hits_and_switching_side_resets_position() {
    let mut v = search_view();
    v.find.query = "PEAR".into();
    v.find.case = true;
    v.find.side = 0;
    v.find_action(Action::Next).unwrap();
    assert_eq!(v.grid.cursor, Cursor { row: 2, column: 1 });
    assert_eq!(v.grid.active, 0);
    // 右侧没有大写 PEAR。
    v.find.side = 1;
    v.find.last = None;
    v.find_action(Action::Next).unwrap();
    assert_eq!(v.message, "No more matches");
    // 换侧后旧的命中坐标不得沿用：先在左侧定位到行 3，再切到右侧 Next，
    // 若 last 未重置会从行 3 之后继续而漏掉右侧行 0 的命中。
    let mut v = search_view();
    v.find.query = "apple".into();
    v.find.side = 0;
    v.find_action(Action::Next).unwrap();
    v.find_action(Action::Next).unwrap();
    assert_eq!(v.grid.cursor.row, 3);
    v.find.side = 1;
    v.find_action(Action::Next).unwrap();
    assert_eq!(
        v.grid.cursor,
        Cursor { row: 0, column: 1 },
        "switching sides must restart from the first hit of the new side"
    );
    // 面板单选切换侧同样重置位置。
    let ctx = egui::Context::default();
    let mut v = search_view();
    frame(&ctx, &mut v, vec![key(egui::Key::F, true)]);
    v.find.query = "apple".into();
    v.find_action(Action::Next).unwrap();
    assert!(v.find.last.is_some());
    let out = frame(&ctx, &mut v, vec![]);
    click(&ctx, &mut v, button(&out, "Right"));
    assert!(v.find.open && v.find.side == 1, "panel radio switches side");
    assert!(v.find.last.is_none(), "side switch resets the position");
}

#[test]
fn typing_new_query_through_ui_resets_position() {
    let ctx = egui::Context::default();
    let mut v = search_view();
    frame(&ctx, &mut v, vec![key(egui::Key::F, true)]);
    v.find.query = "apple".into();
    v.find_action(Action::Next).unwrap();
    v.find_action(Action::Next).unwrap();
    assert_eq!(v.grid.cursor.row, 3);
    assert!(v.find.last.is_some());
    // 真实输入：聚焦查询框并键入一个字符。
    let out = frame(&ctx, &mut v, vec![]);
    let label = button(&out, "Find");
    click(&ctx, &mut v, label + egui::vec2(90.0, 0.0));
    frame(&ctx, &mut v, vec![egui::Event::Text("x".into())]);
    assert_eq!(v.find.query, "applex", "typed text reaches the query box");
    assert!(
        v.find.last.is_none(),
        "editing the query resets the position"
    );
}

#[test]
fn zero_width_and_unicode_patterns_do_not_hang() {
    let mut v = search_view();
    // 零宽正则：每个单元格都可能产生空匹配，必须推进不死循环。
    v.find.query = "x*".into();
    v.find.regex = true;
    v.find.side = 0;
    v.find_action(Action::Next).unwrap();
    assert!(v.message.starts_with("Found at row 1, column 1"));
    v.find_action(Action::All).unwrap();
    ready(&mut v);
    // Unicode 查询与全词边界。
    v.find = Default::default();
    v.find.query = "梨".into();
    v.find_action(Action::Next).unwrap();
    assert_eq!(v.message, "No more matches");
    v.find.query = "π.*".into();
    v.find.regex = true;
    v.find_action(Action::Next).unwrap();
    assert_eq!(v.message, "No more matches");
}

#[test]
fn find_reveals_hidden_columns_and_filtered_rows() {
    let mut v = search_view();
    // 制造全部单元格 Unimportant 并在投影中忽略：所有列隐藏、Differences 过滤为空。
    // 派生列默认 use_default=true，必须同时关掉才能让逐列 unimportant 生效。
    {
        let p = v.pair.as_ref().unwrap();
        let mut rules = p.map.rules_mut();
        for c in &mut rules.columns.0 {
            c.handling.use_default = false;
            c.handling.unimportant = true;
        }
        rules.ignore_unimportant = true;
    }
    v.project();
    ready(&mut v);
    v.filter = Filter::Differences;
    v.pair.as_mut().unwrap().projection = v.pair.as_ref().unwrap().cache.project(v.filter);
    v.hide_same = true;
    v.refresh_grid();
    assert!(v.grid.columns.is_empty(), "all columns hidden while same");
    assert_eq!(v.row_count(), 0, "all rows filtered out");
    v.find.query = "pear".into();
    v.find.side = 1;
    v.find_action(Action::Next).unwrap();
    assert!(!v.hide_same, "hit in a hidden column reveals columns");
    assert_eq!(v.filter, Filter::All, "hit on a filtered row reveals rows");
    assert_eq!(v.grid.cursor, Cursor { row: 1, column: 1 });
}

#[test]
fn replace_once_then_undo_redo() {
    let mut v = search_view();
    v.find.query = "pear".into();
    v.find.side = 1;
    v.find.replacement = "梨".into();
    v.find_action(Action::Next).unwrap();
    assert_eq!(v.grid.cursor, Cursor { row: 1, column: 1 });
    v.find_action(Action::Replace).unwrap();
    ready(&mut v);
    assert_eq!(cell(&v, 1, 1, 1), "梨");
    assert_eq!(cell(&v, 1, 2, 1), "pear", "single Replace touches one cell");
    v.undo(false);
    ready(&mut v);
    assert_eq!(cell(&v, 1, 1, 1), "pear");
    v.undo(true);
    ready(&mut v);
    assert_eq!(cell(&v, 1, 1, 1), "梨");
}

#[test]
fn replace_all_is_atomic_and_undo_restores_everything() {
    let mut v = search_view();
    v.find.query = "apple".into();
    v.find.side = 1;
    v.find.replacement = "杏".into();
    v.find_action(Action::All).unwrap();
    ready(&mut v);
    assert_eq!(cell(&v, 1, 0, 1), "杏");
    assert_eq!(cell(&v, 1, 3, 1), "杏pie");
    // 撤销路由：Replace All 必须把活动侧/焦点切到被替换侧，Undo 才能到达编辑。
    assert_eq!(v.grid.active, 1);
    v.undo(false);
    ready(&mut v);
    assert_eq!(cell(&v, 1, 0, 1), "apple");
    assert_eq!(cell(&v, 1, 3, 1), "applepie");
}

#[test]
fn readonly_side_rejects_replace_and_all() {
    let mut v = search_view();
    v.files[1].readonly = true;
    v.find.query = "pear".into();
    v.find.side = 1;
    v.find.replacement = "梨".into();
    let err = v.find_action(Action::All).unwrap_err();
    assert!(err.to_string().contains("disabled"));
    assert!(!dirty(&v, 1), "rejected replace must not create history");
    v.find_action(Action::Next).unwrap();
    let err = v.find_action(Action::Replace).unwrap_err();
    assert!(err.to_string().contains("disabled"));
    assert!(!dirty(&v, 1));
    assert_eq!(cell(&v, 1, 1, 1), "pear");
}

#[test]
fn replace_without_hit_or_same_value_creates_no_history() {
    let mut v = search_view();
    v.find.query = "pear".into();
    v.find.side = 1;
    v.find.replacement = "梨".into();
    // 未先定位就 Replace：没有当前命中，不得产生任何编辑历史。
    v.find_action(Action::Replace).unwrap();
    assert!(
        !dirty(&v, 1),
        "Replace with no current hit must not create an empty history entry"
    );
    // 把值替换成它自己：没有任何变化，不得建历史。
    v.find.replacement = "pear".into();
    v.find_action(Action::All).unwrap();
    assert_eq!(v.message, "Replaced 0 cell(s)");
    assert!(
        !dirty(&v, 1),
        "Replace All with identical values must not create history"
    );
    ready(&mut v);
    assert_eq!(cell(&v, 1, 1, 1), "pear");
}

#[test]
fn find_requires_ready_comparison() {
    let mut v = TableCompareView::from_sheets(
        sheet("id,name\n1,a\n"),
        sheet("id,name\n1,b\n"),
        TableRules::default(),
    );
    v.find.query = "a".into();
    let err = v.find_action(Action::Next).unwrap_err();
    assert!(err.to_string().contains("Wait for comparison"));
}
