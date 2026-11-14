use super::super::model::tests::{base, bytes, edit, insert, model};
use super::*;
use bc_script::cli::MergeSpec;

fn launch(output: Option<PathBuf>) -> MergeLaunch {
    MergeLaunch::new(MergeSpec {
        left: "theirs.txt".into(),
        right: "yours.txt".into(),
        center: "base.txt".into(),
        output,
    })
}
fn view(left: &[String], right: &[String], center: &[String]) -> MergeView {
    MergeView::from_model(model(left, right, center), launch(None))
}
fn frame(ctx: &egui::Context, v: &mut MergeView, events: Vec<egui::Event>) -> Option<MergeAction> {
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
fn click(ctx: &egui::Context, v: &mut MergeView, pos: egui::Pos2) {
    frame(
        ctx,
        v,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    frame(
        ctx,
        v,
        vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
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
fn text(v: &MergeView) -> String {
    String::from_utf8(v.model.output.lines.borrow().to_bytes()).unwrap()
}
fn joined(lines: &[String]) -> String {
    String::from_utf8(bytes(lines)).unwrap()
}
/// 一个窗格内第 `row` 个可见行的中心点。
fn row_pos(v: &MergeView, pane: usize, row: usize) -> egui::Pos2 {
    let rect = v.pane_rects[pane];
    egui::pos2(
        rect.left() + GUTTER + 30.0,
        rect.top() + HEADER + (row as f32 + 0.5) * v.row_height(),
    )
}

#[test]
fn opens_on_the_first_conflict_with_four_panes() {
    let b = base(40);
    let mut v = view(&edit(&b, &[(20, "L")]), &edit(&b, &[(20, "R")]), &b);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.unresolved(), 1);
    assert_eq!(v.current_row, 19);
    // 上排三个输入并排（BASE | LEFT | RIGHT），输出在下方，四者都有面积。
    let [l, c, r, o] = v.pane_rects;
    assert!(c.right() <= l.left() && l.right() <= r.left());
    assert!(o.top() >= l.bottom() - 0.5);
    assert!(v.pane_rects.iter().all(|p| p.width() > 100.0 && p.height() > 100.0));
    // 定位：冲突行滚到视野上方附近。
    assert!(v.scroll.y > 0.0);
    assert_eq!((v.scroll.y / v.row_height()).round() as usize, 16);
}

#[test]
fn commands_resolve_navigate_and_undo() {
    let b = base(40);
    let left = edit(&b, &[(5, "L5"), (20, "L20"), (33, "L33")]);
    let right = edit(&b, &[(5, "R5"), (20, "R20"), (35, "R35")]);
    let mut v = view(&left, &right, &b);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.unresolved(), 3);
    assert_eq!(v.current_row, 4);
    // Take Right 只动当前区段。
    frame(&ctx, &mut v, vec![key(egui::Key::R, true)]);
    assert_eq!(v.unresolved(), 2);
    assert_eq!(text(&v), joined(&edit(&b, &[(5, "R5"), (33, "L33"), (35, "R35")])));
    // 下一改动（含冲突）：第 20 行 / 近距离冲突的两段 / 到头。
    frame(&ctx, &mut v, vec![key(egui::Key::N, true)]);
    assert_eq!(v.current_row, 19);
    frame(&ctx, &mut v, vec![key(egui::Key::N, true)]);
    assert_eq!(v.current_row, 32);
    frame(&ctx, &mut v, vec![key(egui::Key::N, true)]);
    assert_eq!(v.current_row, 34);
    frame(&ctx, &mut v, vec![key(egui::Key::N, true)]);
    assert_eq!(v.current_row, 34);
    assert!(v.message.contains("No more"));
    // 近距离冲突：接受当前内容，输出不变。
    let before = text(&v);
    frame(&ctx, &mut v, vec![key(egui::Key::K, true)]);
    assert_eq!(v.unresolved(), 1);
    assert_eq!(text(&v), before);
    // 上一改动两次回到第 20 行（途径近距离冲突的 32 段），Take Left，再撤销。
    frame(&ctx, &mut v, vec![key(egui::Key::P, true)]);
    assert_eq!(v.current_row, 32);
    frame(&ctx, &mut v, vec![key(egui::Key::P, true)]);
    assert_eq!(v.current_row, 19);
    frame(&ctx, &mut v, vec![key(egui::Key::L, true)]);
    assert_eq!(v.unresolved(), 0);
    assert!(text(&v).contains("L20
"));
    frame(&ctx, &mut v, vec![key(egui::Key::Z, true)]);
    assert_eq!(v.unresolved(), 1);
    assert!(!text(&v).contains("L20
"));
    frame(&ctx, &mut v, vec![key(egui::Key::Y, true)]);
    assert_eq!(v.unresolved(), 0);
    // 光标不在改动上时 Take 不做事。
    v.goto_row(0);
    frame(&ctx, &mut v, vec![key(egui::Key::B, true)]);
    assert!(v.message.contains("not on a change"));
}

#[test]
fn typing_edits_the_output_only() {
    let b = base(12);
    let mut v = view(&edit(&b, &[(3, "L3")]), &edit(&b, &[(3, "R3")]), &b);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    v.goto_row(0);
    frame(&ctx, &mut v, vec![]);
    frame(&ctx, &mut v, vec![]);
    // 点输出窗格第 3 个可见行（冲突行）并键入。
    let pos = row_pos(&v, OUT, 2);
    click(&ctx, &mut v, pos);
    assert_eq!(v.active, OUT);
    assert_eq!(v.editor.caret.line, 2);
    frame(&ctx, &mut v, vec![egui::Event::Text("x".into())]);
    assert!(text(&v).contains("x"), "{}", text(&v));
    assert_eq!(v.model.out_line(2).unwrap().origin, OutOrigin::Manual);
    assert_eq!(v.unresolved(), 0);
    let edited = text(&v);
    // 点左侧输入窗格再键入：输入恒只读，输出也不变。
    let pos = row_pos(&v, 0, 4);
    click(&ctx, &mut v, pos);
    assert_eq!(v.active, 0);
    assert_eq!(v.current_row, 4);
    frame(&ctx, &mut v, vec![egui::Event::Text("y".into())]);
    assert_eq!(text(&v), edited);
    assert_eq!(v.model.map.left.to_bytes().unwrap(), bytes(&edit(&b, &[(3, "L3")])));
}

#[test]
fn the_wheel_scrolls_every_pane_together() {
    let b = base(200);
    let mut v = view(&b, &insert(&b, 100, &["new"]), &b);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    let over = row_pos(&v, 1, 3);
    frame(
        &ctx,
        &mut v,
        vec![
            egui::Event::PointerMoved(over),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0.0, -5.0),
                phase: egui::TouchPhase::Move,
                modifiers: Default::default(),
                source: egui::MouseWheelSource::Wheel,
            },
        ],
    );
    assert!(v.scroll.y > 0.0, "one scroll position shared by all four panes");
    // 滚不过末尾。
    v.pending_scroll = Some(100_000);
    frame(&ctx, &mut v, vec![]);
    assert!(v.scroll.y <= 201.0 * v.row_height());
}

#[test]
fn closing_asks_about_an_unsaved_output_and_reports_the_result() {
    let dir = std::env::temp_dir().join(format!("matcha-merge-view-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("out.txt");
    let b = base(12);
    let mut v = MergeView::from_model(
        model(&edit(&b, &[(5, "L5")]), &edit(&b, &[(5, "R5")]), &b),
        launch(Some(out.clone())),
    );
    v.model.target = Some(out.clone());
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    // 带输出目标：打开即未保存，关闭要先问。
    assert!(v.is_dirty());
    assert_eq!(v.result_code(), 101);
    v.request_close();
    assert!(v.prompt_open());
    assert_eq!(frame(&ctx, &mut v, vec![]), None);
    assert!(v.prompt_open(), "the prompt stays until answered");
    // 选"保存"：仍有冲突 → 第二个确认；确认后写出并关闭，结果码 14。
    v.prompt = None;
    v.save(Some(Pending::Close));
    assert!(matches!(
        v.prompt,
        Some(Prompt::Conflicts {
            then: Some(Pending::Close),
            path: None
        })
    ));
    assert_eq!(frame(&ctx, &mut v, vec![]), None);
    assert!(!out.exists());
    v.prompt = None;
    v.save_now(Some(Pending::Close));
    assert_eq!(frame(&ctx, &mut v, vec![]), Some(MergeAction::Close));
    assert_eq!(std::fs::read(&out).unwrap(), bytes(&b));
    assert_eq!(v.result_code(), 14);
    assert!(!v.is_dirty());
    // 解决后保存：不再提示，结果码 0；此后关闭不再问。
    v.command("Take Left");
    assert!(v.is_dirty());
    v.command("Save Output");
    assert!(!v.prompt_open());
    assert_eq!(std::fs::read(&out).unwrap(), bytes(&edit(&b, &[(5, "L5")])));
    assert_eq!(v.result_code(), 0);
    v.request_close();
    assert!(!v.prompt_open());
    assert_eq!(frame(&ctx, &mut v, vec![]), Some(MergeAction::Close));
    // 选"不保存"：直接关闭，文件不动。
    v.command("Take Right");
    v.request_exit();
    assert!(v.prompt_open());
    v.prompt = None;
    v.finish(Pending::Exit);
    assert_eq!(frame(&ctx, &mut v, vec![]), Some(MergeAction::Exit));
    assert_eq!(std::fs::read(&out).unwrap(), bytes(&edit(&b, &[(5, "L5")])));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn saving_without_a_target_asks_for_a_path() {
    let b = base(6);
    let mut v = view(&edit(&b, &[(2, "L2")]), &b, &b);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    assert!(!v.is_dirty());
    v.command("Save Output");
    assert!(matches!(v.prompt, Some(Prompt::SaveAs { .. })));
    frame(&ctx, &mut v, vec![]);
    assert!(v.prompt_open());
    // 另存为同样要先确认未解决的冲突，确认后才写到选好的路径。
    let b = base(6);
    let mut c = view(&edit(&b, &[(2, "L2")]), &edit(&b, &[(2, "R2")]), &b);
    frame(&ctx, &mut c, vec![]);
    let out = std::env::temp_dir().join(format!("matcha-merge-saveas-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&out);
    c.command("Save Output As...");
    assert!(matches!(c.prompt, Some(Prompt::SaveAs { confirmed: false, .. })));
    c.prompt = None;
    c.save_as(out.display().to_string(), None, false);
    assert!(matches!(c.prompt, Some(Prompt::Conflicts { path: Some(_), .. })));
    assert!(!out.exists());
    frame(&ctx, &mut c, vec![]);
    c.prompt = None;
    c.save_as(out.display().to_string(), None, true);
    assert!(!c.prompt_open());
    assert_eq!(std::fs::read(&out).unwrap(), bytes(&b));
    assert_eq!(c.result_code(), 14);
    // 写不进去：留在对话框里。
    let nowhere = std::env::temp_dir().join("matcha-no-such-folder").join("out.txt");
    c.save_as(nowhere.display().to_string(), None, true);
    assert!(matches!(c.prompt, Some(Prompt::SaveAs { .. })));
    assert!(c.message.contains("cannot write"));
    let _ = std::fs::remove_file(&out);
    // 没改动、没目标：关闭不问。
    v.prompt = None;
    v.request_close();
    assert_eq!(frame(&ctx, &mut v, vec![]), Some(MergeAction::Close));
    assert_eq!(v.result_code(), 101);
}
