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
    assert!(v
        .pane_rects
        .iter()
        .all(|p| p.width() > 100.0 && p.height() > 100.0));
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
    assert_eq!(
        text(&v),
        joined(&edit(&b, &[(5, "R5"), (33, "L33"), (35, "R35")]))
    );
    // 下一改动（含冲突）：第 20 行 / 近距离冲突的两段 / 到头。
    frame(&ctx, &mut v, vec![key(egui::Key::N, true)]);
    assert_eq!(v.current_row, 19);
    frame(&ctx, &mut v, vec![key(egui::Key::N, true)]);
    assert_eq!(v.current_row, 32);
    frame(&ctx, &mut v, vec![key(egui::Key::N, true)]);
    assert_eq!(v.current_row, 34);
    frame(&ctx, &mut v, vec![key(egui::Key::N, true)]);
    assert_eq!(v.current_row, 34);
    assert!(v.not_found.is_shown());
    assert!(!v.message.contains("No more"));
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
    assert!(text(&v).contains(
        "L20
"
    ));
    frame(&ctx, &mut v, vec![key(egui::Key::Z, true)]);
    assert_eq!(v.unresolved(), 1);
    assert!(!text(&v).contains(
        "L20
"
    ));
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
    assert_eq!(
        v.model.map.left.to_bytes().unwrap(),
        bytes(&edit(&b, &[(3, "L3")]))
    );
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
    assert!(
        v.scroll.y > 0.0,
        "one scroll position shared by all four panes"
    );
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
    assert!(matches!(
        c.prompt,
        Some(Prompt::SaveAs {
            confirmed: false,
            ..
        })
    ));
    c.prompt = None;
    c.save_as(out.display().to_string(), None, false);
    assert!(matches!(
        c.prompt,
        Some(Prompt::Conflicts { path: Some(_), .. })
    ));
    assert!(!out.exists());
    frame(&ctx, &mut c, vec![]);
    c.prompt = None;
    c.save_as(out.display().to_string(), None, true);
    assert!(!c.prompt_open());
    assert_eq!(std::fs::read(&out).unwrap(), bytes(&b));
    assert_eq!(c.result_code(), 14);
    // 写不进去：留在对话框里。
    let nowhere = std::env::temp_dir()
        .join("matcha-no-such-folder")
        .join("out.txt");
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

fn shift_ctrl(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers {
            ctrl: true,
            command: true,
            shift: true,
            ..Default::default()
        },
    }
}

#[test]
fn line_takes_ordered_takes_and_selected_rows() {
    let b = base(40);
    let left = edit(&b, &[(5, "L5"), (6, "L6")]);
    let right = edit(&b, &[(5, "R5"), (6, "R6")]);
    let mut v = view(&left, &right, &b);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.current_row, 4);
    // Shift+Ctrl+L takes only the cursor's line; the two-row conflict stays open.
    frame(&ctx, &mut v, vec![shift_ctrl(egui::Key::L)]);
    assert!(text(&v).contains("L5\n") && !text(&v).contains("L6\n"));
    assert_eq!(v.unresolved(), 1);
    frame(&ctx, &mut v, vec![key(egui::Key::Z, true)]);

    // Take Right Then Left: the whole section, right lines first.
    v.command("Take Right Then Left");
    assert_eq!(
        text(&v),
        joined(&insert(&right, 6, &["L5", "L6"])),
        "right block followed by the left block"
    );
    assert_eq!(v.unresolved(), 0);
    frame(&ctx, &mut v, vec![key(egui::Key::Z, true)]);
    assert_eq!(v.unresolved(), 1);

    // A selection limits Take to the selected rows (ending at a line start
    // excludes that line).
    v.editor.place(Caret { line: 4, byte: 0 }, false);
    v.editor.place(Caret { line: 5, byte: 0 }, true);
    v.command("Take Left");
    assert!(text(&v).contains("L5\n") && !text(&v).contains("L6\n"));
}

#[test]
fn output_context_menu_offers_takes_and_clipboard() {
    let b = base(20);
    let mut v = view(&edit(&b, &[(5, "L5")]), &edit(&b, &[(5, "R5")]), &b);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    let enabled = |v: &MergeView, id: &str| {
        v.context_items()
            .iter()
            .find(|i| i.action_id == id)
            .unwrap_or_else(|| panic!("{id} missing"))
            .enabled
    };
    // On the conflict: every Take is available; nothing selected → no Cut/Copy.
    for id in [
        "Take Left",
        "Take Center",
        "Take Right",
        "Take Left Then Right",
        "Take Right Then Left",
        "Take Left Line",
        "Accept Current Content",
        "Paste",
        "Select All",
    ] {
        assert!(enabled(&v, id), "{id}");
    }
    assert!(!enabled(&v, "Copy") && !enabled(&v, "Cut"));
    // Off any change: section takes and Accept are disabled, line takes stay.
    v.goto_row(0);
    assert!(!enabled(&v, "Take Left") && !enabled(&v, "Accept Current Content"));
    assert!(enabled(&v, "Take Right Line"));
    // Select All through the menu path selects the output; Copy becomes available.
    v.clipboard("Select All", &ctx);
    assert!(v.editor.selection().is_some());
    assert!(enabled(&v, "Copy") && enabled(&v, "Take Left"));
}

/// 三份带文件格式（按 `name` 解析，如 `x.rs`）的输入。
fn formatted_view(name: &str, left: &[&str], right: &[&str], center: &[&str]) -> MergeView {
    let formats = bc_formats::registry::FormatRegistry::with_builtins();
    let format = formats
        .resolve(
            std::path::Path::new(name),
            Some(bc_formats::FormatKind::Text),
        )
        .cloned();
    let content = |lines: &[&str]| {
        let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
        TextContent::from_bytes(text.into_bytes(), format.clone()).unwrap()
    };
    let model = MergeModel::from_contents(
        content(left),
        content(right),
        content(center),
        Default::default(),
    )
    .unwrap();
    MergeView::from_model(model, launch(None))
}
/// 窗格 `pane` 对齐行 `row` 的排版，返回每个字节位置的颜色与字体。
fn line_inks(
    v: &mut MergeView,
    ctx: &egui::Context,
    pane: usize,
    row: usize,
) -> Vec<(egui::Color32, egui::FontId)> {
    let line = match pane {
        OUT => v.model.out_line_of_row(row),
        _ => {
            let mapped = v.model.map.row(row).unwrap();
            [mapped.left, mapped.center, mapped.right][pane].unwrap()
        }
    };
    let text = String::from_utf8(
        v.content(pane)
            .lines
            .borrow()
            .line_bytes(line)
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    let ink = v.pane_ink(pane, ctx, NORMAL);
    let (job, _) = v.line_job(pane, row, line, &text, &ink);
    (0..job.text.len())
        .map(|byte| {
            let section = job
                .sections
                .iter()
                .find(|s| s.byte_range.contains(&egui::text::ByteIndex(byte)))
                .unwrap();
            (section.format.color, section.format.font_id.clone())
        })
        .collect()
}
const NORMAL: egui::Color32 = egui::Color32::from_rgb(1, 2, 3);
/// 着色（非正文色）的字节区间。
fn inked(inks: &[(egui::Color32, egui::FontId)], color: egui::Color32) -> Vec<usize> {
    (0..inks.len()).filter(|&i| inks[i].0 == color).collect()
}

#[test]
fn syntax_highlighting_colours_every_pane_and_can_be_switched_off() {
    let lines = ["int main() {", "    char *s = \"text\"; // note", "}"];
    let mut v = formatted_view("x.cpp", &lines, &lines, &lines);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    let mono = egui::FontId::monospace(v.font_size);
    let plain = |inks: &[(egui::Color32, egui::FontId)]| {
        inks.iter().all(|(c, f)| *c == NORMAL && *f == mono)
    };
    for pane in 0..4 {
        let inks = line_inks(&mut v, &ctx, pane, 1);
        assert!(!plain(&inks), "pane {pane} has syntax colours");
    }
    let checked = |v: &MergeView| {
        v.menus
            .iter()
            .flat_map(|m| m.items.iter())
            .find(|i| i.action_id == "Syntax Highlighting")
            .map(|i| i.kind.clone())
    };
    assert_eq!(checked(&v), Some(MenuItemKind::Check { checked: true }));
    v.command("Syntax Highlighting");
    frame(&ctx, &mut v, vec![]);
    assert!(!v.show_syntax);
    assert_eq!(checked(&v), Some(MenuItemKind::Check { checked: false }));
    for pane in 0..4 {
        assert!(plain(&line_inks(&mut v, &ctx, pane, 1)), "pane {pane}");
    }
}

#[test]
fn inline_differences_ink_only_the_changed_characters() {
    // Rows 1..=4 keep the left-only change at row 0 out of the conflict at row 5.
    let base = [
        "let total = price * qty;",
        "s",
        "s",
        "s",
        "s",
        "a = 1;",
        "end",
    ];
    let left = [
        "let total = cost * qty;",
        "s",
        "s",
        "s",
        "s",
        "a = 2;",
        "end",
    ];
    let right = [
        "let total = price * qty;",
        "s",
        "s",
        "s",
        "s",
        "a = 3;",
        "end",
    ];
    let mut v = formatted_view("x.txt", &left, &right, &base);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    // Left changed one word: only `cost` carries the left change ink.
    let inks = line_inks(&mut v, &ctx, 0, 0);
    assert_eq!(
        inked(&inks, theme::MERGE_LEFT),
        (12..16).collect::<Vec<_>>()
    );
    assert!(inks[..12].iter().all(|(c, _)| *c == NORMAL));
    // Right did not change row 0: no inline entry, plain text.
    assert!(inked(&line_inks(&mut v, &ctx, 2, 0), theme::MERGE_RIGHT).is_empty());
    // A conflict: both sides differ from base → the conflict ink on both.
    assert!(!v.model.state.in_conflict(0) && v.model.state.in_conflict(5));
    let at = |inks: &[(egui::Color32, egui::FontId)]| inked(inks, theme::MERGE_CONFLICT);
    assert_eq!(at(&line_inks(&mut v, &ctx, 0, 5)), vec![4]);
    assert_eq!(at(&line_inks(&mut v, &ctx, 2, 5)), vec![4]);
    // Base and output panes never get inline highlighting.
    for pane in [1, OUT] {
        v.ensure_inline(pane, 0);
        v.ensure_inline(pane, 5);
        assert!(!v.inline.keys().any(|(p, _)| *p == pane));
        let inks = line_inks(&mut v, &ctx, pane, 0);
        assert!(inks.iter().all(|(c, _)| *c == NORMAL), "pane {pane}");
    }
    // Once the conflict is resolved the left pane falls back to its own change colour.
    v.goto_row(5);
    v.command("Take Left");
    assert!(!v.model.state.in_conflict(5));
    assert_eq!(
        inked(&line_inks(&mut v, &ctx, 0, 5), theme::MERGE_LEFT),
        vec![4]
    );
}

#[test]
fn whole_line_additions_get_no_inline_diff_and_the_cache_is_bounded() {
    let b = base(6);
    let added = insert(&b, 3, &["brand new"]);
    let mut v = view(&added, &b, &b);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    let row = (0..v.model.map.row_count())
        .find(|&r| v.model.map.row(r).unwrap().center.is_none())
        .unwrap();
    v.ensure_inline(0, row);
    assert!(!v.inline.contains_key(&(0, row)));
    // Every left line changed: the cache empties itself when it reaches the limit.
    let b = base(INLINE_LIMIT + 10);
    let changes: Vec<(usize, String)> = (1..=b.len()).map(|i| (i, format!("x{i}"))).collect();
    let changes: Vec<(usize, &str)> = changes.iter().map(|(i, s)| (*i, s.as_str())).collect();
    let mut v = view(&edit(&b, &changes), &b, &b);
    for row in 0..INLINE_LIMIT {
        v.ensure_inline(0, row);
    }
    assert_eq!(v.inline.len(), INLINE_LIMIT);
    v.ensure_inline(0, INLINE_LIMIT);
    assert_eq!(v.inline.len(), 1);
}

/// 一帧，带键盘修饰状态；返回完整输出（剪贴板命令等）。
fn run(
    ctx: &egui::Context,
    v: &mut MergeView,
    events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
) -> egui::FullOutput {
    // egui tracks held modifiers from `ModifiersChanged`, not from the button event.
    let mut all = vec![egui::Event::ModifiersChanged(modifiers)];
    all.extend(events);
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        events: all,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        v.ui(ui, &mut Prefs::default());
    });
    out.textures_delta.clear();
    out
}
fn button(
    pos: egui::Pos2,
    button: egui::PointerButton,
    pressed: bool,
    modifiers: egui::Modifiers,
) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers,
    }
}
/// 原始输入拖选：按下、经过中点、到达、松开。
fn drag(ctx: &egui::Context, v: &mut MergeView, from: egui::Pos2, to: egui::Pos2) {
    let none = egui::Modifiers::default();
    let primary = egui::PointerButton::Primary;
    run(
        ctx,
        v,
        vec![
            egui::Event::PointerMoved(from),
            button(from, primary, true, none),
        ],
        none,
    );
    run(
        ctx,
        v,
        vec![egui::Event::PointerMoved(from.lerp(to, 0.5))],
        none,
    );
    run(ctx, v, vec![egui::Event::PointerMoved(to)], none);
    run(ctx, v, vec![button(to, primary, false, none)], none);
}
/// 文本区左缘（第 0 字节）处的位置。
fn line_start(v: &MergeView, pane: usize, row: usize) -> egui::Pos2 {
    egui::pos2(
        v.pane_rects[pane].left() + GUTTER + 1.0,
        row_pos(v, pane, row).y,
    )
}
fn copied(out: &egui::FullOutput) -> Option<String> {
    out.platform_output.commands.iter().find_map(|c| match c {
        egui::OutputCommand::CopyText(t) => Some(t.clone()),
        _ => None,
    })
}

#[test]
fn input_panes_select_by_dragging_shift_clicking_and_select_all() {
    let b = base(12);
    let mut v = view(&edit(&b, &[(10, "L10")]), &b, &b);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.scroll.y, 0.0);
    // Drag across rows 2..=4 of the base pane.
    let (from, to) = (row_pos(&v, 1, 2), row_pos(&v, 1, 4));
    drag(&ctx, &mut v, from, to);
    assert_eq!(v.active, 1);
    let (a, b2) = v.input_editors[1].selection().expect("a selection");
    assert_eq!((a.line, b2.line), (2, 4));
    assert!(a.byte > 0 && a.byte == b2.byte);
    assert!(
        v.editor.selection().is_none(),
        "the output keeps its own caret"
    );
    // Shift+click extends the selection; a plain click collapses it.
    let shift = egui::Modifiers {
        shift: true,
        ..Default::default()
    };
    let at = line_start(&v, 1, 6);
    let primary = egui::PointerButton::Primary;
    run(
        &ctx,
        &mut v,
        vec![
            egui::Event::PointerMoved(at),
            button(at, primary, true, shift),
        ],
        shift,
    );
    run(&ctx, &mut v, vec![button(at, primary, false, shift)], shift);
    let (a, b2) = v.input_editors[1].selection().unwrap();
    assert_eq!((a.line, b2), (2, Caret { line: 6, byte: 0 }));
    assert_eq!(v.current_row, 6);
    let at = line_start(&v, 1, 3);
    run(&ctx, &mut v, vec![], egui::Modifiers::default());
    click(&ctx, &mut v, at);
    assert!(v.input_editors[1].selection().is_none());
    assert_eq!(v.input_editors[1].caret, Caret { line: 3, byte: 0 });
    // Ctrl+A selects the whole base pane.
    frame(&ctx, &mut v, vec![key(egui::Key::A, true)]);
    let (a, b2) = v.input_editors[1].selection().unwrap();
    assert_eq!((a, b2.line), (Caret::default(), 12));
}

#[test]
fn input_panes_copy_and_move_but_never_change() {
    let b = base(12);
    let mut v = view(&edit(&b, &[(10, "L10")]), &b, &b);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    frame(&ctx, &mut v, vec![]);
    let (from, to) = (line_start(&v, 2, 2), line_start(&v, 2, 4));
    drag(&ctx, &mut v, from, to);
    assert_eq!(v.active, 2);
    let selection = v.input_editors[2].selection();
    assert_eq!(
        selection,
        Some((Caret { line: 2, byte: 0 }, Caret { line: 4, byte: 0 }))
    );
    let content = |v: &MergeView| v.content(2).lines.borrow().to_bytes();
    let (before, revision, output) = (content(&v), v.content(2).revision(), text(&v));
    let none = egui::Modifiers::default();
    run(
        &ctx,
        &mut v,
        vec![
            egui::Event::Text("x".into()),
            key(egui::Key::Delete, false),
            key(egui::Key::Backspace, false),
            key(egui::Key::Enter, false),
            key(egui::Key::Tab, false),
            egui::Event::Paste("pasted".into()),
            egui::Event::Cut,
            egui::Event::Ime(egui::ImeEvent::Commit("ime".into())),
        ],
        none,
    );
    assert_eq!(content(&v), before);
    assert_eq!(v.content(2).revision(), revision);
    assert_eq!(text(&v), output);
    assert_eq!(v.input_editors[2].selection(), selection);
    // Ctrl+C copies the two selected lines with their line endings.
    let out = run(&ctx, &mut v, vec![egui::Event::Copy], none);
    assert_eq!(copied(&out).as_deref(), Some("line03\nline04\n"));
    // Arrow keys move the caret and the current row follows.
    run(&ctx, &mut v, vec![key(egui::Key::ArrowDown, false)], none);
    assert!(v.input_editors[2].selection().is_none());
    assert_eq!(v.input_editors[2].caret.line, 5);
    assert_eq!(v.current_row, 5);
}

#[test]
fn input_context_menu_takes_the_clicked_section_and_copies() {
    let b = base(20);
    let left = edit(&b, &[(5, "L5"), (15, "L15")]);
    let right = edit(&b, &[(5, "R5"), (15, "R15")]);
    let mut v = view(&left, &right, &b);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    frame(&ctx, &mut v, vec![]);
    assert_eq!((v.unresolved(), v.current_row), (2, 4));
    // Right-click the second conflict in the left pane: it becomes the current row.
    let at = row_pos(&v, 0, 14);
    let none = egui::Modifiers::default();
    let secondary = egui::PointerButton::Secondary;
    run(
        &ctx,
        &mut v,
        vec![
            egui::Event::PointerMoved(at),
            button(at, secondary, true, none),
        ],
        none,
    );
    run(&ctx, &mut v, vec![button(at, secondary, false, none)], none);
    assert_eq!((v.active, v.current_row), (0, 14));
    let enabled = |v: &MergeView, id: &str| {
        v.input_context_items(0)
            .iter()
            .find(|i| i.action_id == id)
            .unwrap_or_else(|| panic!("{id} missing"))
            .enabled
    };
    assert!(enabled(&v, "Take Left") && enabled(&v, "Take Right") && enabled(&v, "Select All"));
    assert!(!enabled(&v, "Copy"));
    v.input_command(0, "Take Left", &ctx);
    assert_eq!(v.unresolved(), 1);
    assert!(text(&v).contains("L15\n") && v.model.state.in_conflict(4));
    // Select All selects the whole left input and enables Copy.
    v.input_command(0, "Select All", &ctx);
    assert!(enabled(&v, "Copy"));
    assert_eq!(
        v.input_editors[0].selected_text(v.content(0)),
        joined(&left)
    );
}

/// 把活动窗格设为 `pane`（像用户点过它一样拿到焦点）。
fn activate(ctx: &egui::Context, v: &mut MergeView, pane: usize, row: usize) {
    let at = line_start(v, pane, row);
    click(ctx, v, at);
    assert_eq!(v.active, pane);
}
fn selected(v: &MergeView, pane: usize) -> Option<(Caret, Caret)> {
    v.editor_of(pane).selection()
}

#[test]
fn find_searches_the_active_pane_without_wrapping() {
    let b = base(60);
    let right = edit(&b, &[(3, "a timeout"), (50, "timeout again")]);
    let mut v = view(&b, &right, &b);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    frame(&ctx, &mut v, vec![]);
    v.goto_row(0);
    frame(&ctx, &mut v, vec![]);
    activate(&ctx, &mut v, 2, 0);
    // Ctrl+F opens the bar; Enter in the field finds the first match after the caret.
    frame(&ctx, &mut v, vec![key(egui::Key::F, true)]);
    assert!(v.find.open);
    v.find.options.pattern = "timeout".into();
    v.find_action(FindAction::Next);
    let hit = |line: usize, start: usize| {
        Some((
            Caret { line, byte: start },
            Caret {
                line,
                byte: start + 7,
            },
        ))
    };
    assert_eq!(selected(&v, 2), hit(2, 2));
    assert_eq!(v.current_row, 2);
    // F3: the second match, scrolled into view in every pane.
    frame(&ctx, &mut v, vec![key(egui::Key::F3, false)]);
    frame(&ctx, &mut v, vec![]);
    assert_eq!(selected(&v, 2), hit(49, 0));
    assert_eq!(v.current_row, 49);
    let top = (v.scroll.y / v.row_height()) as usize;
    let shown = ((v.pane_rects[2].height() - HEADER) / v.row_height()) as usize;
    assert!(
        (top..top + shown).contains(&49),
        "row 49 visible from {top}"
    );
    // At the last match F3 reports it and keeps the selection (no wrap).
    frame(&ctx, &mut v, vec![key(egui::Key::F3, false)]);
    assert_eq!(selected(&v, 2), hit(49, 0));
    assert!(v.message.contains("No more matches"), "{}", v.message);
    // Shift+F3 goes back.
    let shift_f3 = egui::Event::Key {
        key: egui::Key::F3,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers {
            shift: true,
            ..Default::default()
        },
    };
    frame(&ctx, &mut v, vec![shift_f3]);
    assert_eq!(selected(&v, 2), hit(2, 2));
    // The base pane has no match: the output's and right's selections stay put.
    assert!(selected(&v, 1).is_none());
}

#[test]
fn find_in_changes_only_skips_unchanged_rows() {
    // `timeout` on an unchanged row (10) and on a right change (40).
    let b = edit(&base(50), &[(10, "timeout same")]);
    let right = edit(&b, &[(40, "timeout changed")]);
    let mut v = view(&b, &right, &b);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    v.goto_row(0);
    frame(&ctx, &mut v, vec![]);
    activate(&ctx, &mut v, 2, 0);
    v.find.options.pattern = "timeout".into();
    v.find.options.differences_only = true;
    v.find_action(FindAction::Next);
    assert_eq!(selected(&v, 2).map(|(a, _)| a.line), Some(39));
    // Without the option the unchanged row is found first; the output pane searches too.
    v.find.options.differences_only = false;
    v.input_editors[2] = Editor::default();
    v.find.last_hit = None;
    v.find_action(FindAction::Next);
    assert_eq!(selected(&v, 2).map(|(a, _)| a.line), Some(9));
    activate(&ctx, &mut v, OUT, 0);
    v.find.options.differences_only = true;
    v.find_action(FindAction::Next);
    assert_eq!(selected(&v, OUT).map(|(a, _)| a.line), Some(39));
}

#[test]
fn replace_is_output_only_and_replaces_then_finds_next() {
    let b = edit(&base(20), &[(2, "foo a"), (6, "foo b"), (9, "foo c")]);
    let mut v = view(&b, &b, &b);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    v.goto_row(0);
    frame(&ctx, &mut v, vec![]);
    let replace_enabled = |v: &MergeView| {
        v.menus
            .iter()
            .flat_map(|m| m.items.iter())
            .find(|i| i.action_id == "Replace...")
            .unwrap()
            .enabled
    };
    // An input pane: Replace is disabled and the bar shows no replace row.
    activate(&ctx, &mut v, 0, 0);
    frame(&ctx, &mut v, vec![key(egui::Key::H, true)]);
    frame(&ctx, &mut v, vec![]);
    assert!(!replace_enabled(&v) && !v.find.replace_open);
    v.find.show();
    v.find.replace_open = true;
    frame(&ctx, &mut v, vec![]);
    assert!(!v.find.replace_open);
    v.find.options.pattern = "foo".into();
    v.find.replacement = "bar".into();
    v.find_action(FindAction::All);
    assert!(text(&v).contains("foo a"), "inputs never replace");
    // The output pane: Ctrl+H opens the replace row.
    activate(&ctx, &mut v, OUT, 0);
    frame(&ctx, &mut v, vec![key(egui::Key::H, true)]);
    frame(&ctx, &mut v, vec![]);
    assert!(replace_enabled(&v) && v.find.replace_open);
    // Replace with no current match finds one first, then replaces and moves on.
    v.find_action(FindAction::Replace);
    assert_eq!(selected(&v, OUT).map(|(a, _)| a.line), Some(1));
    v.find_action(FindAction::Replace);
    assert!(text(&v).contains("bar a") && text(&v).contains("foo b"));
    assert_eq!(selected(&v, OUT).map(|(a, _)| a.line), Some(5));
    assert_eq!(v.model.out_line(1).unwrap().origin, OutOrigin::Manual);
    // Replace All takes the rest; one undo puts them back.
    v.find_action(FindAction::All);
    assert!(v.message.contains("Replaced 2"), "{}", v.message);
    assert!(!text(&v).contains("foo"));
    v.command("Undo");
    assert!(text(&v).contains("foo b") && text(&v).contains("foo c"));
    assert!(text(&v).contains("bar a"));
}

/// 指定窗口宽度跑一帧。
fn frame_wide(ctx: &egui::Context, v: &mut MergeView, width: f32, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(width, 900.0),
        )),
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        v.ui(ui, &mut Prefs::default());
    });
    out.textures_delta.clear();
}

#[test]
fn a_shared_horizontal_scrollbar_appears_only_for_wide_content() {
    let b = base(10);
    let long = format!("long {}", "x".repeat(150));
    let mut v = view(&edit(&b, &[(3, &long)]), &b, &b);
    let ctx = egui::Context::default();
    for _ in 0..3 {
        frame(&ctx, &mut v, vec![]);
    }
    let bar = v
        .hbar
        .expect("the long left line needs a horizontal scrollbar");
    assert!(v.max_x > 0.0 && v.scroll.x == 0.0);
    // Across the bottom of the pane area, below the output pane.
    assert!(bar.top() >= v.pane_rects[OUT].bottom() - 0.5);
    assert!(bar.left() <= v.pane_rects[1].left() && bar.right() >= v.pane_rects[2].right() - 0.5);
    // Drag the thumb past the right end: every pane scrolls to the limit.
    let thumb = egui::pos2(bar.left() + 16.0 + 4.0, bar.center().y);
    drag(
        &ctx,
        &mut v,
        thumb,
        egui::pos2(bar.right() + 300.0, bar.center().y),
    );
    frame(&ctx, &mut v, vec![]);
    assert!(
        (v.scroll.x - v.max_x).abs() < 0.5,
        "{} vs {}",
        v.scroll.x,
        v.max_x
    );
    // The wheel over the bar scrolls horizontally.
    let wheel = |dy: f32| egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, dy),
        phase: egui::TouchPhase::Move,
        modifiers: Default::default(),
        source: egui::MouseWheelSource::Wheel,
    };
    let before = v.scroll.x;
    frame(
        &ctx,
        &mut v,
        vec![egui::Event::PointerMoved(bar.center()), wheel(30.0)],
    );
    assert!(v.scroll.x < before);
    // A window wide enough for every line: the bar goes away and the offset resets.
    for _ in 0..3 {
        frame_wide(&ctx, &mut v, 6000.0, vec![]);
    }
    assert!(v.hbar.is_none());
    assert_eq!((v.max_x, v.scroll.x), (0.0, 0.0));
}

// ---- 编号书签 ----

fn bookmark_key(n: usize, shift: bool) -> egui::Event {
    const DIGITS: [egui::Key; 10] = [
        egui::Key::Num0,
        egui::Key::Num1,
        egui::Key::Num2,
        egui::Key::Num3,
        egui::Key::Num4,
        egui::Key::Num5,
        egui::Key::Num6,
        egui::Key::Num7,
        egui::Key::Num8,
        egui::Key::Num9,
    ];
    egui::Event::Key {
        key: DIGITS[n],
        physical_key: Some(DIGITS[n]),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers {
            ctrl: true,
            command: true,
            shift,
            ..Default::default()
        },
    }
}
fn search_item(v: &MergeView, id: &str) -> MenuItem {
    fn find(items: &[MenuItem], id: &str) -> Option<MenuItem> {
        items.iter().find_map(|item| {
            if item.action_id == id {
                return Some(item.clone());
            }
            match &item.kind {
                MenuItemKind::Submenu(children) => find(children, id),
                _ => None,
            }
        })
    }
    v.menus
        .iter()
        .find_map(|m| find(&m.items, id))
        .unwrap_or_else(|| panic!("no menu item {id}"))
}
/// 窗格 `pane` 文本区右缘画出的书签编号。
fn badge_drawn(out: &egui::FullOutput, v: &MergeView, pane: usize, n: usize) -> bool {
    let rect = v.pane_rects[pane];
    out.shapes.iter().any(|s| match &s.shape {
        egui::Shape::Text(t) => {
            let at = egui::Rect::from_min_size(t.pos, t.galley.size()).center();
            t.galley.text() == n.to_string() && rect.contains(at) && at.x > rect.right() - 24.0
        }
        _ => false,
    })
}
fn frame_out(ctx: &egui::Context, v: &mut MergeView) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        v.ui(ui, &mut Prefs::default());
    });
    out.textures_delta.clear();
    out
}

#[test]
fn bookmarks_jump_to_their_pane_and_row_and_survive_takes() {
    let b = base(200);
    // 左侧在冲突处多出 2 行：初始输出含这 2 行，Take Right 后输出少 2 行。
    let left = insert(&edit(&b, &[(20, "L")]), 20, &["L2", "L3"]);
    let mut v = view(&left, &edit(&b, &[(20, "R")]), &b);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    frame(&ctx, &mut v, vec![]);
    let conflict = v.current_row;
    assert_eq!(v.unresolved(), 1);
    assert!(!search_item(&v, "Go to Bookmark 1").enabled);
    assert!(!search_item(&v, "Clear Bookmarks").enabled);
    let toggle = search_item(&v, "Toggle Bookmark 1");
    assert_eq!(
        (toggle.label.as_str(), toggle.shortcut.as_deref()),
        ("Bookmark 1", Some("Shift+Ctrl+1"))
    );

    // base 窗格的冲突行（区段里第一个有 base 行的对齐行）放书签 1。
    let (marked, line) = (conflict..)
        .find_map(|r| Some((r, v.model.map.row(r)?.center?)))
        .unwrap();
    assert!(v.model.state.in_conflict(marked));
    v.active = 1;
    v.input_editors[1].place(Caret { line, byte: 0 }, false);
    frame(&ctx, &mut v, vec![bookmark_key(1, true)]);
    assert_eq!(v.bookmarks.get(1), Some((1, marked)), "{}", v.message);
    let out = frame_out(&ctx, &mut v);
    assert!(badge_drawn(&out, &v, 1, 1), "badge in the base pane");
    assert!(!badge_drawn(&out, &v, OUT, 1), "only in the anchor pane");
    assert!(search_item(&v, "Go to Bookmark 1").enabled);
    assert!(!search_item(&v, "Go to Bookmark 2").enabled);
    assert!(search_item(&v, "Clear Bookmarks").enabled);

    // 输出窗格靠后的一行放书签 2。
    v.active = OUT;
    v.editor.place(Caret { line: 150, byte: 0 }, false);
    let late = v.model.out_line(150).unwrap().row;
    frame(&ctx, &mut v, vec![bookmark_key(2, true)]);
    assert_eq!(v.bookmarks.get(2), Some((OUT, late)));

    // 滚到末尾，从输出窗格 Ctrl+1：base 成为活动窗格，滚到冲突行。
    v.scroll.y = 1.0e6;
    frame(&ctx, &mut v, vec![]);
    frame(&ctx, &mut v, vec![bookmark_key(1, false)]);
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.active, 1);
    assert_eq!(v.current_row, marked);
    assert_eq!(v.input_editors[1].caret, Caret { line, byte: 0 });
    let top = (v.scroll.y / v.row_height()) as usize;
    let grid = v.model.grid_row_of(marked);
    assert!(top <= grid && grid < top + 20, "top {top}, grid {grid}");
    assert!(ctx.memory(|m| m.has_focus(pane_id(1))));

    // 对该区段 Take Right（输出少 2 行）：书签仍在同一对齐行，跳转定位到该区段。
    let before = v.model.output.lines.borrow().len();
    v.active = OUT;
    v.goto_row(conflict);
    v.command("Take Right");
    assert_eq!(v.model.output.lines.borrow().len(), before - 2);
    assert_eq!(v.bookmarks.get(1), Some((1, marked)));
    assert_eq!(v.bookmarks.get(2), Some((OUT, late)));
    v.scroll.y = 0.0;
    frame(&ctx, &mut v, vec![bookmark_key(2, false)]);
    assert_eq!(v.active, OUT);
    assert_eq!(v.current_row, late);
    assert_eq!(
        v.editor.caret.line, 148,
        "same row, output shifted by the take"
    );
    frame(&ctx, &mut v, vec![bookmark_key(1, false)]);
    assert_eq!((v.active, v.current_row), (1, marked));

    // 未设置与清除。
    frame(&ctx, &mut v, vec![bookmark_key(7, false)]);
    assert_eq!(v.not_found.text(), Some("Bookmark 7 is not set."));
    assert!(!v.message.contains("Bookmark 7"), "{}", v.message);
    assert_eq!(v.current_row, marked);
    v.command("Clear Bookmarks");
    assert!(v.bookmarks.is_empty());
    frame(&ctx, &mut v, vec![]);
    assert!(!search_item(&v, "Clear Bookmarks").enabled);
}

fn frame_prefs(ctx: &egui::Context, v: &mut MergeView, prefs: &mut Prefs, events: Vec<egui::Event>) -> egui::FullOutput {
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
fn change_navigation_past_the_ends_shows_the_panel_wraps_or_stays_silent() {
    let b = base(40);
    let left = edit(&b, &[(5, "L5"), (20, "L20")]);
    let right = edit(&b, &[(5, "R5"), (30, "R30")]);
    let mut v = view(&left, &right, &b);
    let ctx = egui::Context::default();
    let mut prefs = Prefs::default();
    frame_prefs(&ctx, &mut v, &mut prefs, vec![]);
    let first = v.current_row;
    // 向上越过第一个改动：面板画在状态栏上方，光标不动。
    let out = frame_prefs(&ctx, &mut v, &mut prefs, vec![key(egui::Key::P, true)]);
    assert_eq!(v.current_row, first);
    assert!(v.not_found.is_shown());
    let out = if button_rect(&out, crate::chrome::message_panel::NOT_FOUND).is_some() {
        out
    } else {
        frame_prefs(&ctx, &mut v, &mut prefs, vec![])
    };
    let panel = button_rect(&out, crate::chrome::message_panel::NOT_FOUND).expect("panel painted");
    assert!(panel.bottom() < 900.0 - 25.0);
    // 菜单命令同一路径；按键关闭面板。
    frame_prefs(&ctx, &mut v, &mut prefs, vec![key(egui::Key::ArrowDown, false)]);
    assert!(!v.not_found.is_shown());
    v.goto_row(first);
    v.command("Previous Change Section");
    assert!(v.not_found.is_shown());
    // 回绕：向上回到最后一个改动，向下越过末端回到第一个。
    prefs.next_diff.wrap_around = true;
    frame_prefs(&ctx, &mut v, &mut prefs, vec![key(egui::Key::ArrowDown, false)]);
    v.goto_row(first);
    frame_prefs(&ctx, &mut v, &mut prefs, vec![key(egui::Key::P, true)]);
    let last = v.current_row;
    assert!(last > first);
    assert!(!v.not_found.is_shown());
    frame_prefs(&ctx, &mut v, &mut prefs, vec![key(egui::Key::N, true)]);
    assert_eq!(v.current_row, first);
    // 关闭面板：静默。
    prefs.next_diff.wrap_around = false;
    prefs.next_diff.message_panel = false;
    frame_prefs(&ctx, &mut v, &mut prefs, vec![key(egui::Key::P, true)]);
    assert_eq!(v.current_row, first);
    assert!(!v.not_found.is_shown());
    // 没有改动：回绕也找不到，仍弹面板。
    let mut same = view(&b, &b, &b);
    let mut prefs = Prefs::default();
    prefs.next_diff.wrap_around = true;
    frame_prefs(&ctx, &mut same, &mut prefs, vec![]);
    frame_prefs(&ctx, &mut same, &mut prefs, vec![key(egui::Key::N, true)]);
    assert!(same.not_found.is_shown());
}

fn button_rect(out: &egui::FullOutput, label: &str) -> Option<egui::Rect> {
    out.shapes.iter().find_map(|s| match &s.shape {
        egui::Shape::Text(t) if t.galley.text() == label => {
            Some(egui::Rect::from_min_size(t.pos, t.galley.size()))
        }
        _ => None,
    })
}
