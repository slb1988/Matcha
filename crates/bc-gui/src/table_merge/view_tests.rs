use super::super::model::tests::{conflicted, Fixture, BASE};
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
    assert!(f.output().is_file());
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
