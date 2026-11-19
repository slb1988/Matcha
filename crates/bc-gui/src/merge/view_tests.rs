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
