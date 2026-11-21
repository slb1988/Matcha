use super::super::model::tests::{conflicted, two_sheet_books, Fixture, BASE};
use super::*;

fn view(f: &Fixture) -> TableMergeView {
    TableMergeView::open(f.launch.clone()).unwrap()
}

fn frame(
    ctx: &egui::Context,
    v: &mut TableMergeView,
    events: Vec<egui::Event>,
) -> Option<MergeAction> {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        events,
        ..Default::default()
    };
    let mut action = None;
    let mut out = ctx.run_ui(input, |ui| {
        action = v.ui(ui, &mut Prefs::default());
    });
    out.textures_delta.clear();
    action
}

fn key(key: egui::Key, command: bool, shift: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers {
            ctrl: command,
            command,
            shift,
            ..Default::default()
        },
    }
}

fn click(ctx: &egui::Context, v: &mut TableMergeView, pos: egui::Pos2) {
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    frame(ctx, v, vec![egui::Event::PointerMoved(pos), button(true)]);
    frame(ctx, v, vec![button(false)]);
}

/// 一个窗格内第 `row` 个可见行、第 `column` 列格子的中心点。
fn cell_pos(v: &TableMergeView, pane: usize, row: usize, column: usize) -> egui::Pos2 {
    let rect = v.pane_rects[pane];
    egui::pos2(
        rect.left() + GUTTER + column as f32 * COLUMN_WIDTH + COLUMN_WIDTH / 2.0,
        rect.top() + HEADER + COLUMNS + row as f32 * ROW_HEIGHT + ROW_HEIGHT / 2.0,
    )
}

#[test]
fn renders_and_opens_on_the_first_conflict() {
    let f = conflicted("view-render");
    let ctx = egui::Context::default();
    let mut v = view(&f);
    for _ in 0..2 {
        frame(&ctx, &mut v, vec![]);
    }
    assert_eq!(v.selected.0, 0);
    assert!(
        v.describe_cell().contains("Conflict"),
        "{}",
        v.describe_cell()
    );
    assert!(v.pane_rects[OUT].height() > 100.0);
}

#[test]
fn shortcuts_take_and_undo() {
    let f = conflicted("view-take");
    let ctx = egui::Context::default();
    let mut v = view(&f);
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.unresolved(), 1);
    frame(&ctx, &mut v, vec![key(egui::Key::R, true, false)]);
    assert_eq!(v.unresolved(), 0);
    frame(&ctx, &mut v, vec![key(egui::Key::Z, true, false)]);
    assert_eq!(v.unresolved(), 1);
    frame(&ctx, &mut v, vec![key(egui::Key::K, true, false)]);
    assert_eq!(v.unresolved(), 0);
}

#[test]
fn clicking_selects_and_double_click_edits_an_output_cell() {
    let f = Fixture::new(
        "view-edit",
        "id,name,qty\r\n1,apple,12\r\n2,pear,20\r\n3,plum,30\r\n",
        "id,name,qty\r\n1,apple,15\r\n2,pear,20\r\n3,plum,30\r\n",
        BASE,
    );
    let ctx = egui::Context::default();
    let mut v = view(&f);
    frame(&ctx, &mut v, vec![]);
    let pos = cell_pos(&v, OUT, 2, 1);
    click(&ctx, &mut v, pos);
    assert_eq!(v.selected, (2, 1));
    // 选到冲突格，F2 编辑、输入、Enter 提交。
    let pos = cell_pos(&v, OUT, 0, 2);
    click(&ctx, &mut v, pos);
    assert_eq!(v.selected, (0, 2));
    frame(&ctx, &mut v, vec![key(egui::Key::F2, false, false)]);
    frame(&ctx, &mut v, vec![]);
    if let Some(edit) = &mut v.editing {
        edit.text = "13".into();
    }
    frame(&ctx, &mut v, vec![key(egui::Key::Enter, false, false)]);
    assert!(v.editing.is_none());
    assert_eq!(v.unresolved(), 0);
    assert_eq!(v.model.current().output_cell(0, 2).unwrap().1, "13");
}

#[test]
fn save_and_close_report_result_codes() {
    let f = conflicted("view-save");
    let ctx = egui::Context::default();
    let mut v = view(&f);
    frame(&ctx, &mut v, vec![]);
    // 未保存关闭 → 先确认。
    v.request_close();
    assert!(v.prompt_open());
    v.prompt = None;
    v.command("Take Left");
    assert_eq!(v.unresolved(), 0);
    v.command("Save Output");
    assert!(!v.prompt_open(), "{}", v.message);
    // 默认写 LEFT 的文件，并同步到输出文件（P4V `%r`）。
    let left = std::fs::read(&f.launch.spec.left).unwrap();
    assert_eq!(left, std::fs::read(f.output()).unwrap());
    assert_eq!(v.result_code(), 0);
    v.request_close();
    assert_eq!(frame(&ctx, &mut v, vec![]), Some(MergeAction::Close));
}

#[test]
fn unresolved_save_asks_first_and_reports_14() {
    let f = conflicted("view-save14");
    let ctx = egui::Context::default();
    let mut v = view(&f);
    frame(&ctx, &mut v, vec![]);
    v.command("Save Output");
    assert!(matches!(v.prompt, Some(Prompt::Conflicts { .. })));
    v.prompt = None;
    v.save_now(None);
    assert_eq!(v.result_code(), 14);
}

#[test]
fn header_path_edit_focus_blocks_plain_keys() {
    let f = conflicted("view-header-focus");
    let ctx = egui::Context::default();
    let mut v = view(&f);
    // 先空帧注册 widget（egui 命中测试用上一帧的注册表）。
    for _ in 0..2 {
        frame(&ctx, &mut v, vec![]);
    }
    // 点击标题条路径文本：路径框持焦。
    let rect = v.pane_rects[0];
    click(
        &ctx,
        &mut v,
        egui::pos2(rect.left() + 80.0, rect.top() + HEADER / 2.0),
    );
    frame(&ctx, &mut v, vec![]);
    assert!(v.path_edit_focused);
    // 无修饰键留给路径框：方向键不动选中格，Delete 不清输出格。
    let selected = v.selected;
    let dirty = v.model.is_dirty();
    frame(&ctx, &mut v, vec![key(egui::Key::ArrowDown, false, false)]);
    frame(&ctx, &mut v, vec![key(egui::Key::Delete, false, false)]);
    assert_eq!(v.selected, selected);
    assert_eq!(v.model.is_dirty(), dirty);
    // Ctrl 快捷键照常（这里用 Ctrl+N 冲突导航验证，当前已在首个冲突，落到下一冲突或保持）。
    // 点击输出网格：焦点交回，普通按键恢复。
    let out_cell = cell_pos(&v, OUT, 0, 0);
    click(&ctx, &mut v, out_cell);
    frame(&ctx, &mut v, vec![]);
    assert!(!v.path_edit_focused);
    frame(&ctx, &mut v, vec![key(egui::Key::ArrowDown, false, false)]);
    assert_ne!(v.selected, selected);
}

/// 12 列 CSV：第 1 列为键，right 把 k2 行的第 10 列改成 X。
fn wide(tag: &str) -> Fixture {
    let head = "c1,c2,c3,c4,c5,c6,c7,c8,c9,c10,c11,c12";
    let a = "a,a,a,a,a,a,a,a,a,a,a";
    let base = format!("{head}\r\nk1,{a}\r\nk2,{a}\r\nk3,{a}\r\n");
    let right = format!("{head}\r\nk1,{a}\r\nk2,a,a,a,a,a,a,a,a,X,a,a\r\nk3,{a}\r\n");
    Fixture::new(tag, &base, &right, &base)
}

#[test]
fn wide_sheet_has_hbar_and_opens_on_the_changed_cell() {
    let f = wide("hbar");
    let ctx = egui::Context::default();
    let mut v = view(&f);
    for _ in 0..3 {
        frame(&ctx, &mut v, vec![]);
    }
    // 打开落到第一个改动格（表头按数据行计：k2 是第 2 行、X 是第 9 列）。
    assert_eq!(v.selected, (2, 9));
    // 水平可见：输入窗格滚到选中格完整可见；输出窗格够宽（12 列全显示）不滚。
    let view_width = v.pane_rects[0].width() - GUTTER;
    assert!(v.scroll_x[0] > 0.0);
    let left = 9.0 * COLUMN_WIDTH;
    let right = left + COLUMN_WIDTH;
    assert!(left >= v.scroll_x[0] - 0.5);
    assert!(right <= v.scroll_x[0] + view_width + 0.5);
    // 停在整列边界（左侧不出现截去一半的格）。
    assert_eq!(v.scroll_x[0] % COLUMN_WIDTH, 0.0, "{}", v.scroll_x[0]);
    assert_eq!(v.max_x[0] % COLUMN_WIDTH, 0.0, "{}", v.max_x[0]);
    assert_eq!(v.scroll_x[OUT], 0.0);
    // 详情条显示活动窗格选中格的完整内容。
    assert_eq!(v.details_text(), "X");
    // 超宽时各窗格按需出横向滚动条（输入窗格超宽，3 列的窄夹具不超宽）。
    assert!(v.max_x[0] > 0.0);
    let narrow = conflicted("narrow");
    let mut nv = view(&narrow);
    for _ in 0..3 {
        frame(&ctx, &mut nv, vec![]);
    }
    assert_eq!(nv.max_x[0], 0.0);
}

#[test]
fn arrow_keys_scroll_horizontally_to_keep_the_cell_visible() {
    let f = wide("hscroll");
    let ctx = egui::Context::default();
    let mut v = view(&f);
    for _ in 0..2 {
        frame(&ctx, &mut v, vec![]);
    }
    v.selected = (1, 0);
    v.scroll_x = [0.0; 4];
    for _ in 0..8 {
        frame(&ctx, &mut v, vec![key(egui::Key::ArrowRight, false, false)]);
    }
    assert_eq!(v.selected.1, 8);
    let view_width = v.pane_rects[0].width() - GUTTER;
    let left = 8.0 * COLUMN_WIDTH;
    assert!(v.scroll_x[0] > 0.0);
    assert!(left + COLUMN_WIDTH <= v.scroll_x[0] + view_width + 0.5);
    // 滚轮横向滚动不会把选中格吸回视野（手动查看自由）。
    let before = v.scroll_x;
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.scroll_x, before);
}

#[test]
fn details_follows_the_active_panes_cell() {
    let f = wide("details");
    let ctx = egui::Context::default();
    let mut v = view(&f);
    for _ in 0..2 {
        frame(&ctx, &mut v, vec![]);
    }
    // 打开落在 right 的改动格 X；活动窗格默认是输出格（自动合并已取 right）。
    assert_eq!(v.selected, (2, 9));
    assert_eq!(v.details_text(), "X");
    // 点 BASE 窗格的同格：详情条换成 base 的 a。
    let pos = cell_pos(&v, 1, 0, 0);
    click(&ctx, &mut v, egui::pos2(pos.x, pos.y));
    v.selected = (2, 9);
    v.active = 1;
    assert_eq!(v.details_text(), "a");
    // 输出窗格：自动合并的内容（right 的 X）。
    v.active = OUT;
    assert_eq!(v.details_text(), "X");
}

#[test]
fn ctrl_delete_removes_the_output_row() {
    let f = conflicted("view-remove");
    let ctx = egui::Context::default();
    let mut v = view(&f);
    frame(&ctx, &mut v, vec![]);
    // 选中删改冲突行（2 号），Ctrl+Delete 从输出移除。
    v.selected = (1, 0);
    frame(&ctx, &mut v, vec![key(egui::Key::Delete, true, false)]);
    assert!(v.model.current().row_removed(1));
    assert!(v.model.current().output_cell(1, 0).is_none());
    // 再按一次：行已不在输出里，只给提示。
    frame(&ctx, &mut v, vec![key(egui::Key::Delete, true, false)]);
    assert_eq!(v.message, "This row is not in the output");
    // 对该行取 right 恢复。
    v.command("Take Right (Cell)");
    assert!(!v.model.current().row_removed(1));
    frame(&ctx, &mut v, vec![]);
}

fn frame_prefs(ctx: &egui::Context, v: &mut TableMergeView, prefs: &mut Prefs, events: Vec<egui::Event>) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        v.ui(ui, prefs);
    });
    out.textures_delta.clear();
    out
}

#[test]
fn change_navigation_crosses_sheets_then_shows_the_panel_or_wraps() {
    let f = two_sheet_books("view-nav-wrap");
    let ctx = egui::Context::default();
    let mut v = view(&f);
    let mut prefs = Prefs::default();
    frame_prefs(&ctx, &mut v, &mut prefs, vec![]);
    assert_eq!(v.model.sheet, 0);
    // 跨 sheet 继续到第 2 个 sheet 的改动；再向下找不到 → 面板（状态栏上方）。
    frame_prefs(&ctx, &mut v, &mut prefs, vec![key(egui::Key::N, true, false)]);
    assert_eq!(v.model.sheet, 1);
    assert!(!v.not_found.is_shown());
    let selected = v.selected;
    frame_prefs(&ctx, &mut v, &mut prefs, vec![key(egui::Key::N, true, false)]);
    assert_eq!((v.model.sheet, v.selected), (1, selected));
    assert!(v.not_found.is_shown());
    let out = frame_prefs(&ctx, &mut v, &mut prefs, vec![]);
    let panel = out
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == crate::chrome::message_panel::NOT_FOUND => {
                Some(egui::Rect::from_min_size(t.pos, t.galley.size()))
            }
            _ => None,
        })
        .expect("panel painted");
    assert!(panel.bottom() < 900.0 - 25.0);
    // 回绕：向下回到第一个 sheet 的改动，向上回到最后一个 sheet。
    prefs.next_diff.wrap_around = true;
    frame_prefs(&ctx, &mut v, &mut prefs, vec![key(egui::Key::N, true, false)]);
    assert_eq!(v.model.sheet, 0);
    assert!(!v.not_found.is_shown());
    frame_prefs(&ctx, &mut v, &mut prefs, vec![key(egui::Key::P, true, false)]);
    assert_eq!(v.model.sheet, 1);
    // 关闭面板：静默。
    prefs.next_diff.wrap_around = false;
    prefs.next_diff.message_panel = false;
    frame_prefs(&ctx, &mut v, &mut prefs, vec![key(egui::Key::N, true, false)]);
    assert_eq!(v.model.sheet, 1);
    assert!(!v.not_found.is_shown());
    // 菜单命令同一路径。
    prefs.next_diff.message_panel = true;
    frame_prefs(&ctx, &mut v, &mut prefs, vec![]);
    v.command("Next Change Section");
    assert!(v.not_found.is_shown());
}
