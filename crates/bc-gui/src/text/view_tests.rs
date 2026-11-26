use super::*;
use std::sync::Arc;
fn view(left: &str, right: &str) -> TextCompareView {
    let mut v = TextCompareView::from_contents(
        TextContent::from_bytes(left.as_bytes().to_vec(), None).unwrap(),
        TextContent::from_bytes(right.as_bytes().to_vec(), None).unwrap(),
        FormatRegistry::with_builtins(),
    );
    ready(&mut v);
    v
}
fn ready(v: &mut TextCompareView) {
    let start = Instant::now();
    while !v.is_ready() {
        v.poll();
        assert!(start.elapsed() < Duration::from_secs(5), "{}", v.message);
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn frame(
    ctx: &egui::Context,
    v: &mut TextCompareView,
    events: Vec<egui::Event>,
) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        v.ui(ui, &mut Prefs::default());
    });
    // Headless tests tessellate but have no GPU texture uploader.
    out.textures_delta.clear();
    out
}
fn click(ctx: &egui::Context, v: &mut TextCompareView, pos: egui::Pos2) {
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
fn text_shapes(output: &egui::FullOutput) -> Vec<(String, egui::Rect)> {
    output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some((
                t.galley.text().to_string(),
                egui::Rect::from_min_size(t.pos, t.galley.size()),
            )),
            _ => None,
        })
        .collect()
}
struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!(
            "matcha-gui-test-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn raw_input_edit_realign_undo_ime_and_tessellation() {
    let ctx = egui::Context::default();
    let mut v = view("abc\n", "ab\n");
    frame(&ctx, &mut v, vec![]);
    let pos = v.pane_rects[1].left_top() + egui::vec2(110.0, 8.0);
    click(&ctx, &mut v, pos);
    assert_eq!(v.active, 1);
    assert!(
        ctx.memory(|m| m.has_focus(editor_id(1))),
        "editor focus {:?}",
        ctx.memory(|m| m.focused())
    );
    frame(
        &ctx,
        &mut v,
        vec![key(egui::Key::End, false), egui::Event::Text("c".into())],
    );
    let start = Instant::now();
    ready(&mut v);
    eprintln!(
        "gui.realign_after_edit_ms={:.2}",
        start.elapsed().as_secs_f64() * 1000.0
    );
    assert!(start.elapsed() < Duration::from_millis(500));
    assert_eq!(v.map.right.to_bytes().unwrap(), b"abc\n");
    assert_eq!(v.counts.important, 0);
    frame(&ctx, &mut v, vec![key(egui::Key::Z, true)]);
    ready(&mut v);
    assert_eq!(v.map.right.to_bytes().unwrap(), b"ab\n");
    assert_eq!(v.counts.important, 1);
    frame(
        &ctx,
        &mut v,
        vec![egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "中文".into(),
            active_range_chars: Some(0..2),
        })],
    );
    assert_eq!(v.map.right.to_bytes().unwrap(), b"ab\n");
    let out = frame(
        &ctx,
        &mut v,
        vec![egui::Event::Ime(egui::ImeEvent::Commit("中文".into()))],
    );
    assert_eq!(v.map.right.to_bytes().unwrap(), "ab中文\n".as_bytes());
    assert!(
        out.platform_output.ime.is_some(),
        "native IME caret output must be provided"
    );
    let meshes = ctx.tessellate(out.shapes, out.pixels_per_point);
    assert!(!meshes.is_empty());
    v.map.right.read_only = true;
    frame(&ctx, &mut v, vec![egui::Event::Text("rejected".into())]);
    assert_eq!(v.map.right.to_bytes().unwrap(), "ab中文\n".as_bytes());
    assert!(v.message.contains("read-only"));
}
#[test]
fn filters_navigation_context_and_public_alignment_options() {
    let mut v = view("a\nvalue=x\na\nvalue=y\na\n", "a\nvalue=X\na\nvalue=Y\na\n");
    v.navigate(true, true);
    assert_eq!(v.current_row, 1);
    v.navigate(true, false);
    assert_eq!(v.current_row, 3);
    v.navigate(true, false);
    assert_eq!(v.current_row, 3);
    assert!(v.not_found.is_shown());
    v.filter = DisplayFilter::Differences;
    v.rebuild_projection();
    // 2 个差异行 + 3 个折叠分隔行 + 可编辑 EOF 行。
    assert_eq!(v.projection.len(), 6);
    v.filter = DisplayFilter::Same;
    v.rebuild_projection();
    assert_eq!(v.projection.len(), 6); // three Same + editable EOF + 2 fold separators
    v.filter = DisplayFilter::Context;
    v.context = 0;
    v.rebuild_projection();
    let Some(DisplayRow::Fold(range)) = v.projection.row(0) else {
        panic!("fold expected")
    };
    v.expand(range);
    assert_eq!(v.projection.row(0), Some(DisplayRow::Text(0)));
    v.map.rules.comparison.importance.case_unimportant = true;
    v.recompare();
    ready(&mut v);
    assert_eq!(v.counts.unimportant, 2);
    let bytes = v.map.left.to_bytes().unwrap();
    v.map.rules.ignore_unimportant = true;
    v.after_alignment();
    assert_eq!(v.counts.unimportant, 0);
    assert_eq!(v.map.left.to_bytes().unwrap(), bytes);
    let mut v = view("total = price * qty\n", "total = price * quantity\n");
    assert_eq!(v.map.row_count(), 1);
    // Closeness off still pairs the block by position (no similarity check).
    v.map.rules.alignment.use_closeness = false;
    v.recompare();
    ready(&mut v);
    assert_eq!(v.map.row_count(), 1);
    v.map.rules.alignment.use_closeness = true;
    v.map.rules.alignment.never_align_differences = true;
    v.recompare();
    ready(&mut v);
    assert_eq!(v.map.row_count(), 2);
    v.map.rules.alignment.never_align_differences = false;
    v.map.rules.alignment.strategy = bc_core::align::AlignStrategy::Quick;
    v.recompare();
    ready(&mut v);
    assert_eq!(v.map.row_count(), 1);
    v.map.rules.alignment.never_align_differences = true;
    v.recompare();
    ready(&mut v);
    assert_eq!(v.map.row_count(), 2);
}
#[test]
fn independent_save_backup_shared_mmap_history_and_output_target() {
    let temp = Temp::new();
    let l = temp.file("left.txt", b"left\n");
    let r = temp.file("right.txt", b"right\n");
    let mut v = TextCompareView::open(&l, &r).unwrap();
    ready(&mut v);
    v.copy_to(1).unwrap();
    v.map.left.replace_line(0, b"left edited").unwrap();
    let prefs = Prefs {
        backups: bc_session::prefs::BackupPrefs {
            enabled: true,
            extension: Some(".bak".into()),
        },
        ..Default::default()
    };
    // Right's copied line/history shares left's mmap: save must rebase both panes.
    v.save_to(0, &l, &prefs, false).unwrap();
    assert_eq!(std::fs::read(&l).unwrap(), b"left edited\n");
    assert_eq!(
        std::fs::read(l.with_file_name("left.txt.bak")).unwrap(),
        b"left\n"
    );
    assert_eq!(std::fs::read(&r).unwrap(), b"right\n");
    assert!(v.map.right.is_dirty());
    assert!(!v.map.left.is_dirty());
    v.save_to(1, &r, &prefs, false).unwrap();
    assert_eq!(std::fs::read(&r).unwrap(), b"left\n");
    v.map.right.edits.undo();
    assert_eq!(v.map.right.to_bytes().unwrap(), b"right\n");
    v.map.right.edits.redo();
    assert_eq!(v.map.right.to_bytes().unwrap(), b"left\n");
    v.map.right.replace_line(0, b"target").unwrap();
    let target = temp.0.join("output.txt");
    v.save_to(1, &target, &prefs, true).unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"target\n");
    assert_eq!(std::fs::read(&r).unwrap(), b"left\n");
    assert_eq!(v.files[1].path.as_ref(), Some(&r));
    drop(v);
}
#[test]
fn unsaved_close_cancel_reload_swap_format_and_encoding_override() {
    let ctx = egui::Context::default();
    let temp = Temp::new();
    let l = temp.file("a.txt", b"abc\n");
    let r = temp.file("b.txt", b"abc\n");
    let mut v = TextCompareView::open(&l, &r).unwrap();
    ready(&mut v);
    v.map.right.replace_line(0, b"edited").unwrap();
    v.request_close();
    assert!(v.pending.is_some());
    assert!(v.action.is_none());
    let mut out = frame(&ctx, &mut v, vec![]); // first window pass can move while sizing
    for _ in 0..2 {
        out = frame(&ctx, &mut v, vec![]);
    }
    let cancel = text_shapes(&out)
        .into_iter()
        .find(|(t, _)| t == "Cancel")
        .unwrap()
        .1
        .center();
    click(&ctx, &mut v, cancel);
    assert!(v.pending.is_none());
    assert!(v.action.is_none());
    assert!(v.map.right.is_dirty());
    v.perform(Pending::Reload(1));
    ready(&mut v);
    assert_eq!(v.map.right.to_bytes().unwrap(), b"abc\n");
    let python = v.registry.get(FormatKind::Text, "PYTHON").unwrap().clone();
    v.map.right.set_format(Some(python)).unwrap();
    v.map.right.read_only = true;
    v.swap();
    ready(&mut v);
    assert!(v.map.left.read_only);
    assert_eq!(v.map.left.format().unwrap().name, "PYTHON");
    assert_eq!(v.files[0].path.as_ref(), Some(&r));
    v.recompare();
    ready(&mut v);
    assert_eq!(v.map.left.format().unwrap().name, "PYTHON");
    v.perform(Pending::Encoding(0, Encoding::CodePage(1252)));
    ready(&mut v);
    assert_eq!(v.map.left.encoding, Encoding::CodePage(1252));
    assert_eq!(v.map.right.encoding, Encoding::Utf8);
}
#[test]
fn find_replace_difference_scope_and_selected_copy_undo() {
    let mut v = view("TODO same\nTODO old\n", "TODO same\nTODO new\n");
    v.find.options.pattern = "TODO".into();
    v.find.options.differences_only = true;
    v.find_action(FindAction::Next).unwrap();
    assert_eq!(v.find.last_hit.as_ref().unwrap().line, 1);
    v.find.replacement = "DONE".into();
    v.find_action(FindAction::All).unwrap();
    ready(&mut v);
    assert_eq!(v.map.left.to_bytes().unwrap(), b"TODO same\nDONE old\n");
    v.map.left.edits.undo();
    v.recompare();
    ready(&mut v);
    assert_eq!(v.map.left.to_bytes().unwrap(), b"TODO same\nTODO old\n");
    v.editors[0].anchor = Some(Caret { line: 1, byte: 0 });
    v.editors[0].caret = Caret { line: 1, byte: 8 };
    v.copy_to(1).unwrap();
    ready(&mut v);
    assert_eq!(v.map.right.to_bytes().unwrap(), b"TODO same\nTODO old\n");
    v.map.right.edits.undo();
    v.recompare();
    ready(&mut v);
    assert_eq!(v.map.right.to_bytes().unwrap(), b"TODO same\nTODO new\n");
    v.find.options.pattern = "(?P<t>TODO)".into();
    v.find.options.is_regexp = true;
    v.find.last_hit = None;
    v.editors[0].place(Caret::default(), false);
    v.find_action(FindAction::Next).unwrap();
    v.find.replacement = "$t!".into();
    v.find_action(FindAction::Replace).unwrap();
    ready(&mut v);
    assert_eq!(v.map.left.to_bytes().unwrap(), b"TODO same\nTODO! old\n");
}
#[test]
fn virtual_scroll_stacked_gaps_colors_and_pointer_status() {
    let left = (0..1000).map(|i| format!("line {i}\n")).collect::<String>();
    let mut v = view(&left, &left);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    v.scroll_to_row(700);
    let out = frame(&ctx, &mut v, vec![]);
    assert!(v.last_stats.rows_drawn < 120);
    assert!((v.top_row(v.row_height()) as isize - 700).abs() <= 1);
    assert!(
        text_shapes(&out)
            .iter()
            .filter(|(t, _)| t == "line 700")
            .count()
            >= 2
    );
    v.stacked = true;
    frame(&ctx, &mut v, vec![]);
    assert!(
        v.pane_rects[0].bottom() <= v.pane_rects[1].top(),
        "{:?}",
        v.pane_rects
    );
    v.editors[0].place(Caret { line: 119, byte: 7 }, false);
    assert!(status_bar::side_text(&v.map.left, &v.editors[0]).starts_with("120:8"));
    let mut v = view("a\nb\nc\n", "a\ninsert\nb\nc\n");
    let out = frame(&ctx, &mut v, vec![]);
    assert_eq!(v.map.row(1).unwrap().left, None);
    assert_eq!(v.map.row(1).unwrap().right, Some(1));
    let green = crate::chrome::theme::Theme::default().difference_fill;
    assert!(out
        .shapes
        .iter()
        .any(|s| matches!(&s.shape,egui::Shape::Rect(r) if r.fill==green)));
    v.editors[0].place(Caret { line: 0, byte: 1 }, false);
    assert!(status_bar::side_text(&v.map.left, &v.editors[0]).starts_with("1:2"));
    let mut prefs = Prefs::default();
    prefs.colors.inserted = [255, 128, 0];
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            ..Default::default()
        },
        |ui| {
            v.ui(ui, &mut prefs);
        },
    );
    out.textures_delta.clear();
    let orange = super::super::nav_bar::color(&prefs.colors, Importance::Important, true, 38);
    assert!(out
        .shapes
        .iter()
        .any(|s| matches!(&s.shape,egui::Shape::Rect(r) if r.fill==orange)));
    let meshes = ctx.tessellate(out.shapes, out.pixels_per_point);
    assert!(!meshes.is_empty());
}
#[test]
fn two_inline_spans_and_unimportant_colors_reach_the_painter() {
    let ctx = egui::Context::default();
    let mut v = view(
        "value = 1; many_unchanged_letters; flag = 2\n",
        "value = 3; many_unchanged_letters; flag = 4\n",
    );
    let out = frame(&ctx, &mut v, vec![]);
    let dark = crate::chrome::theme::Theme::default().difference_text;
    let changed = out
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text().starts_with("value = 1") => Some(t),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        changed
            .galley
            .job
            .sections
            .iter()
            .filter(|s| s.format.color == dark)
            .count(),
        2
    );
    let mut prefs = Prefs::default();
    prefs.colors.important = [255, 128, 0];
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            ..Default::default()
        },
        |ui| {
            v.ui(ui, &mut prefs);
        },
    );
    out.textures_delta.clear();
    let orange = super::super::nav_bar::color(&prefs.colors, Importance::Important, false, 38);
    assert!(out
        .shapes
        .iter()
        .any(|s| matches!(&s.shape,egui::Shape::Rect(r) if r.fill==orange)));
    let mut v = view("name = value\n", "name = VALUE\n");
    v.map.rules.comparison.importance.case_unimportant = true;
    v.recompare();
    ready(&mut v);
    let out = frame(&ctx, &mut v, vec![]);
    let blue = egui::Color32::from_rgb(238, 238, 255);
    assert!(out
        .shapes
        .iter()
        .any(|s| matches!(&s.shape,egui::Shape::Rect(r) if r.fill==blue)));
    v.map.rules.ignore_unimportant = true;
    v.after_alignment();
    let out = frame(&ctx, &mut v, vec![]);
    assert!(!out
        .shapes
        .iter()
        .any(|s| matches!(&s.shape,egui::Shape::Rect(r) if r.fill==blue)));
}
#[derive(Debug)]
struct Dropped(PathBuf);
impl egui::DroppedFile for Dropped {
    fn path(&self) -> &Path {
        &self.0
    }
    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|e| e.to_string())
    }
}
#[test]
fn raw_drop_into_right_clipboard_and_home_two_file_drop() {
    let temp = Temp::new();
    let l = temp.file("a.txt", b"a");
    let r = temp.file("b.txt", b"b");
    let ctx = egui::Context::default();
    let mut v = view("left", "right");
    frame(&ctx, &mut v, vec![]);
    let pos = v.pane_rects[1].center();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        events: vec![egui::Event::PointerMoved(pos)],
        dropped_files: vec![Arc::new(Dropped(r.clone()))],
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| {
        v.ui(ui, &mut Prefs::default());
    });
    output.textures_delta.clear();
    ready(&mut v);
    assert_eq!(v.map.left.to_bytes().unwrap(), b"left");
    assert_eq!(v.map.right.to_bytes().unwrap(), b"b");
    v.clipboard_side = Some(1);
    frame(&ctx, &mut v, vec![egui::Event::Paste("clip\n文本".into())]);
    ready(&mut v);
    assert_eq!(v.map.right.to_bytes().unwrap(), "clip\n文本".as_bytes());
    assert!(v.map.right.is_dirty());
    let mut home = crate::home::HomeView::default();
    let mut action = None;
    let mut output = ctx.run_ui(
        egui::RawInput {
            dropped_files: vec![Arc::new(Dropped(l.clone())), Arc::new(Dropped(r.clone()))],
            ..Default::default()
        },
        |ui| {
            if let Some(a) = home.ui(ui) {
                action = Some(a);
            }
        },
    );
    output.textures_delta.clear();
    assert_eq!(
        action,
        Some(crate::home::HomeAction::OpenTextCompare { left: l, right: r })
    );
}
#[test]
fn raw_drop_on_right_header_replaces_right_even_when_left_is_active() {
    let temp = Temp::new();
    let r = temp.file("b.txt", b"b");
    let ctx = egui::Context::default();
    let mut v = view("left", "right");
    frame(&ctx, &mut v, vec![]);
    v.active = 0;
    // Right path bar: above the right text body, outside both pane rects.
    let pos = v.pane_rects[1].center_top() - egui::vec2(0.0, 10.0);
    assert!(!v.pane_rects.iter().any(|p| p.contains(pos)));
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            events: vec![egui::Event::PointerMoved(pos)],
            dropped_files: vec![Arc::new(Dropped(r))],
            ..Default::default()
        },
        |ui| {
            v.ui(ui, &mut Prefs::default());
        },
    );
    output.textures_delta.clear();
    ready(&mut v);
    assert_eq!(v.map.left.to_bytes().unwrap(), b"left");
    assert_eq!(v.map.right.to_bytes().unwrap(), b"b");
}
#[test]
fn raw_clipboard_multiline_cut_paste_and_only_save_right() {
    let temp = Temp::new();
    let left = temp.file("left.txt", b"left\r\n");
    let right = temp.file("right.txt", b"one\r\ntwo\nthree");
    let mut v = TextCompareView::open(&left, &right).unwrap();
    ready(&mut v);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    let source_row = v.row_for_line(1, 0);
    let pos =
        v.pane_rects[1].left_top() + egui::vec2(95.0, 8.0 + source_row as f32 * v.row_height());
    click(&ctx, &mut v, pos);
    frame(&ctx, &mut v, vec![key(egui::Key::A, true)]);
    let output = frame(&ctx, &mut v, vec![egui::Event::Copy]);
    assert!(output
        .platform_output
        .commands
        .iter()
        .any(|c| matches!(c,egui::OutputCommand::CopyText(t) if t=="one\r\ntwo\nthree")));
    frame(&ctx, &mut v, vec![egui::Event::Cut]);
    assert!(v.map.right.to_bytes().unwrap().is_empty());
    frame(&ctx, &mut v, vec![key(egui::Key::Z, true)]);
    ready(&mut v);
    assert_eq!(v.map.right.to_bytes().unwrap(), b"one\r\ntwo\nthree");
    v.map.left.replace_line(0, b"left dirty").unwrap();
    v.map.right.replace_line(1, b"TWO").unwrap();
    v.save_to(1, &right, &Prefs::default(), false).unwrap();
    assert_eq!(std::fs::read(&right).unwrap(), b"one\r\nTWO\nthree");
    assert_eq!(std::fs::read(&left).unwrap(), b"left\r\n");
    assert!(v.map.left.is_dirty());
    assert!(!v.map.right.is_dirty());
    frame(&ctx, &mut v, vec![egui::Event::Paste("粘贴\nnext".into())]);
    assert!(String::from_utf8(v.map.right.to_bytes().unwrap())
        .unwrap()
        .contains("粘贴"));
}
#[test]
fn replace_all_unicode_clamps_stale_caret_before_painting() {
    let mut v = view("ab\n", "ab\n");
    v.editors[0].caret = Caret { line: 0, byte: 1 };
    v.find.options.pattern = "a".into();
    v.find.replacement = "中文".into();
    v.find_action(FindAction::All).unwrap();
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    ready(&mut v);
    assert_eq!(v.map.left.to_bytes().unwrap(), "中文b\n".as_bytes());
    assert_eq!(v.editors[0].caret.byte, 0);
}
#[test]
fn manual_anchors_survive_recompare_and_clear() {
    let mut v = view("a\nx\nb\n", "a\ny\nz\nb\n");
    v.map.overrides.insert(1, 2).unwrap();
    v.recompare();
    ready(&mut v);
    assert!(v
        .map
        .rows()
        .any(|r| r.left == Some(1) && r.right == Some(2)));
    v.recompare();
    ready(&mut v);
    assert!(v
        .map
        .rows()
        .any(|r| r.left == Some(1) && r.right == Some(2)));
    v.map.overrides.clear();
    v.recompare();
    ready(&mut v);
    assert!(v.map.overrides.pairs().is_empty());
}

#[test]
fn chrome_commands_keep_legacy_actions_and_filter_state() {
    let ctx = egui::Context::default();
    let mut v = view("same\nleft\ntail\n", "same\nright\ntail\n");
    let prefs = Prefs::default();
    v.command("Show Context", &ctx, &prefs);
    v.command("Show Differences", &ctx, &prefs);
    assert_eq!(v.filter, DisplayFilter::Differences);
    assert_eq!(v.checked("Show Differences"), Some(true));
    v.command("First difference", &ctx, &prefs);
    assert_eq!(v.current_row, 1);
    v.command("Copy to Right", &ctx, &prefs);
    ready(&mut v);
    assert_eq!(
        v.map.left.to_bytes().unwrap(),
        v.map.right.to_bytes().unwrap()
    );
    v.command("Over-Under Layout", &ctx, &prefs);
    assert!(v.stacked);
    v.command("Line Numbers", &ctx, &prefs);
    assert!(!v.show_numbers);
    v.command("File properties...", &ctx, &prefs);
    assert!(v.show_file_options);
    v.update_menus();
    assert_eq!(
        v.menus.iter().map(|m| m.title.as_str()).collect::<Vec<_>>(),
        ["Session", "File", "Edit", "Search", "View", "Tools", "Help"]
    );
    let enabled = |label: &str| {
        v.menus[0]
            .items
            .iter()
            .find(|i| i.label == label)
            .unwrap()
            .enabled
    };
    // 标签页命令已接到应用的标签页容器；New Window 仍未实现。
    assert!(enabled("New Tab"));
    assert!(enabled("Close Tab"));
    assert!(!enabled("New Window"));
    v.command("New Tab", &ctx, &prefs);
    assert!(matches!(v.action.take(), Some(ViewAction::NewTab)));
    v.command("Home", &ctx, &prefs);
    assert!(matches!(v.action.take(), Some(ViewAction::Home)));
    // Copy to Right 留下了未保存编辑：Close Tab 先弹保存确认。
    assert!(v.is_dirty());
    v.command("Close Tab", &ctx, &prefs);
    assert!(v.prompt_open() && v.action.is_none());
    v.perform(Pending::Close);
    assert!(matches!(v.action.take(), Some(ViewAction::Close)));
}

#[test]
fn status_title_and_core_geometry_follow_reference_slots() {
    let ctx = egui::Context::default();
    let mut v = view("one\n", "two\n");
    v.files[0].path = Some("left.txt".into());
    v.files[1].path = Some("right.txt".into());
    assert_eq!(v.title(), "left.txt <--> right.txt - Text Compare - Matcha");
    let out = frame(&ctx, &mut v, vec![]);
    assert_eq!(v.pane_rects[0].top(), 129.0);
    assert_eq!(v.pane_rects[0].left(), 52.0);
    assert_eq!(v.pane_rects[0].bottom(), 807.0);
    assert!(text_shapes(&out).iter().any(|(t, _)| t == "Default text"));
    assert!(text_shapes(&out).iter().any(|(t, _)| t == "Insert"));
    assert!(text_shapes(&out)
        .iter()
        .any(|(t, _)| t.contains("difference section(s)")));
}

#[test]
fn toolbar_mouse_filter_and_reference_presets_use_real_projection() {
    let ctx = egui::Context::default();
    let mut v = view(
        "alpha\r\nbeta\r\ngamma\r\n",
        "alpha\r\nBETA\r\ngamma\r\ninserted\r\n",
    );
    v.snapshot_rules();
    ready(&mut v);
    v.snapshot_preset("text-diffs-filter");
    assert_eq!(v.filter, DisplayFilter::Differences);
    assert!(!v.context_enabled);
    // 折叠分隔行让隐藏行可见为「N FILTERED LINES」；行数可等于总行数。
    assert!(v.projection.len() <= v.row_count());
    assert_eq!(v.projection.row(0), Some(DisplayRow::Fold(0..1)));
    let out = frame(&ctx, &mut v, vec![]);
    let all = text_shapes(&out)
        .into_iter()
        .find(|(t, _)| t == "All")
        .unwrap()
        .1
        .center();
    click(&ctx, &mut v, all);
    assert_eq!(v.filter, DisplayFilter::All);
    v.snapshot_preset("text-cursor-on-diff");
    assert_eq!(v.editors[0].caret.line, 1);
    assert_eq!(v.editors[0].caret.byte, 4);
    assert_eq!(v.row(v.current_row).importance, Importance::Unimportant);
}

fn wheel_lines(lines: f32, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: egui::vec2(0.0, lines),
        modifiers,
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
    }
}
/// Top y of the "line N" text in the left/right pane (x split at the splitter).
fn line_tops(out: &egui::FullOutput, label: &str, split: f32) -> [Option<f32>; 2] {
    let mut tops = [None, None];
    for (t, r) in text_shapes(out) {
        if t == label {
            tops[usize::from(r.left() > split)] = Some(r.top());
        }
    }
    tops
}
/// Topmost "line N" label drawn in each pane.
fn first_rows(out: &egui::FullOutput, split: f32) -> [Option<String>; 2] {
    let mut best: [Option<(f32, String)>; 2] = [None, None];
    for (t, r) in text_shapes(out) {
        if !t.starts_with("line ") {
            continue;
        }
        let side = usize::from(r.left() > split);
        if best[side].as_ref().is_none_or(|(y, _)| r.top() < *y) {
            best[side] = Some((r.top(), t));
        }
    }
    best.map(|b| b.map(|(_, t)| t))
}
#[test]
fn wheel_scroll_tracks_input_immediately_and_keeps_panes_in_lockstep() {
    let left = (0..5000).map(|i| format!("line {i}\n")).collect::<String>();
    let mut v = view(&left, &left);
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    let pane = v.pane_rects[1];
    let pos = egui::pos2(pane.left() + 100.0, pane.top() + 100.0);
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(pos)]);
    assert_eq!(v.scroll.y, 0.0);
    // 10 wheel-down notches of 3 lines in one frame over the right pane.
    let mut events = vec![egui::Event::PointerMoved(pos)];
    events.extend((0..10).map(|_| wheel_lines(-3.0, Default::default())));
    let mut trajectory = Vec::new();
    let split = (v.pane_rects[0].right() + v.pane_rects[1].left()) / 2.0;
    let mut out = frame(&ctx, &mut v, events);
    let mut out_of_step = usize::from(first_rows(&out, split)[0] != first_rows(&out, split)[1]);
    trajectory.push(v.scroll.y);
    for _ in 0..20 {
        out = frame(&ctx, &mut v, vec![egui::Event::PointerMoved(pos)]);
        let rows = first_rows(&out, split);
        out_of_step += usize::from(rows[0] != rows[1]);
        trajectory.push(v.scroll.y);
    }
    eprintln!("frames with left/right panes out of step: {out_of_step}");
    assert_eq!(out_of_step, 0, "panes scroll in the same frame");
    let target = 10.0 * 3.0 * 40.0;
    eprintln!("text wheel trajectory: {trajectory:?}");
    assert!(
        trajectory.iter().all(|&y| y == target),
        "full delta applies in the arrival frame without glide: {trajectory:?}"
    );
    // Both panes show the same row at the same height in that frame.
    let row = (target / v.row_height()) as usize + 3;
    let tops = line_tops(&out, &format!("line {row}"), split);
    assert!(tops[0].is_some() && tops[0] == tops[1], "{tops:?}");
    // Ctrl+wheel is zoom, never scroll; outside the panes nothing scrolls.
    frame(
        &ctx,
        &mut v,
        vec![
            egui::Event::PointerMoved(pos),
            wheel_lines(-3.0, egui::Modifiers::COMMAND),
        ],
    );
    assert_eq!(v.scroll.y, target);
    let outside = egui::pos2(5.0, 5.0);
    frame(
        &ctx,
        &mut v,
        vec![
            egui::Event::PointerMoved(outside),
            wheel_lines(-3.0, Default::default()),
        ],
    );
    assert_eq!(v.scroll.y, target);
    // Wheel up past the top clamps at zero.
    frame(
        &ctx,
        &mut v,
        vec![
            egui::Event::PointerMoved(pos),
            wheel_lines(50.0, Default::default()),
        ],
    );
    assert_eq!(v.scroll.y, 0.0);
}

/// Interactive-latency probe over real files (not part of the default run):
/// `cargo test --release -p bc-gui --lib text_interaction_latency -- --ignored --nocapture`
/// with `MATCHA_PERF_LEFT`/`MATCHA_PERF_RIGHT` (default: the 110 MB audit
/// inputs). Each step is one real egui pass plus tessellation, timed
/// end-to-end on this thread the way a frame would be.
#[test]
#[ignore]
fn text_interaction_latency() {
    let path = |var: &str, default: &str| {
        std::env::var_os(var)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(default))
    };
    let left = path(
        "MATCHA_PERF_LEFT",
        "../../target/table-wave7/audit/inputs/text-left.txt",
    );
    let right = path(
        "MATCHA_PERF_RIGHT",
        "../../target/table-wave7/audit/inputs/text-right.txt",
    );
    let ctx = egui::Context::default();
    let t = Instant::now();
    let mut v = TextCompareView::open(&left, &right).unwrap();
    let open_ms = t.elapsed().as_secs_f64() * 1e3;
    let start = Instant::now();
    while !v.is_ready() {
        v.poll();
        std::thread::sleep(Duration::from_millis(1));
        assert!(start.elapsed() < Duration::from_secs(300));
    }
    let ready_ms = start.elapsed().as_secs_f64() * 1e3;
    eprintln!(
        "[latency] text open={open_ms:.1}ms align={ready_ms:.1}ms rows={}",
        v.row_count()
    );
    let prefs = Prefs::default();
    let timed = |label: &str, v: &mut TextCompareView, events: Vec<egui::Event>| {
        let t = Instant::now();
        let out = frame(&ctx, v, events);
        let ui = t.elapsed().as_secs_f64() * 1e3;
        let _ = ctx.tessellate(out.shapes, out.pixels_per_point);
        let total = t.elapsed().as_secs_f64() * 1e3;
        if !label.is_empty() {
            eprintln!("[latency] text {label:<26} ui={ui:>8.2}ms ui+tess={total:>8.2}ms");
        }
        total
    };
    timed("first frame", &mut v, vec![]);
    timed("second frame", &mut v, vec![]);
    let pane = v.pane_rects[1];
    let pos = egui::pos2(pane.left() + 100.0, pane.top() + 100.0);
    timed("pointer move", &mut v, vec![egui::Event::PointerMoved(pos)]);
    let mut wheel_ms = Vec::new();
    for _ in 0..60 {
        wheel_ms.push(timed(
            "",
            &mut v,
            vec![
                egui::Event::PointerMoved(pos),
                wheel_lines(-3.0, Default::default()),
            ],
        ));
    }
    wheel_ms.sort_by(f64::total_cmp);
    eprintln!(
        "[latency] text wheel x60 p50={:.2}ms max={:.2}ms",
        wheel_ms[30], wheel_ms[59]
    );
    for command in [
        "Show Differences",
        "Show Same",
        "Show Context",
        "Show All",
        "Next Difference",
        "Last difference",
        "First difference",
    ] {
        let t = Instant::now();
        v.command(command, &ctx, &prefs);
        let cmd = t.elapsed().as_secs_f64() * 1e3;
        let total = timed(command, &mut v, vec![egui::Event::PointerMoved(pos)]);
        eprintln!("[latency] text   command {command:<20} cmd={cmd:.2}ms (+frame {total:.2}ms)");
    }
    v.scroll_to_row(v.row_count() / 2);
    timed("jump to middle", &mut v, vec![]);
    v.scroll_to_row(v.row_count());
    timed("jump to end", &mut v, vec![]);
}

#[test]
fn filtered_lines_separators_are_thin_rows_and_jumps_follow_them() {
    let ctx = egui::Context::default();
    let left = (0..400).map(|i| format!("line {i}\n")).collect::<String>();
    let right = left
        .replace("line 100\n", "LINE 100\n")
        .replace("line 300\n", "LINE 300\n");
    let mut v = view(&left, &right);
    // Differences without context lines (the reference filter scene).
    v.context_enabled = false;
    v.filter = DisplayFilter::Differences;
    v.rebuild_projection();
    let out = frame(&ctx, &mut v, vec![]);
    let folds: Vec<f32> = text_shapes(&out)
        .iter()
        .filter(|(t, _)| t.ends_with("FILTERED LINES"))
        .map(|(_, r)| r.center().y)
        .collect();
    // Fold, difference rows, fold: separators are 9 px + whole 15 px rows
    // apart (uniform 15 px rows would give a multiple of 15).
    assert!(folds.len() >= 2, "{folds:?}");
    let gap = folds[1] - folds[0];
    assert!(gap > 9.0 && ((gap - 9.0) % 15.0).abs() < 0.01, "{folds:?}");
    // Jumping to a display row lands its top at row offsets that account for
    // the thin separators, and top_row maps back.
    let g = super::super::projection::RowGeometry::for_row_height(v.row_height());
    let row = v.projection.len() - 2;
    v.scroll_to_row(row);
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.scroll.y.min(v.projection.y_of(row, g)), v.scroll.y);
    assert!(v.top_row(v.row_height()) <= row);
}

#[test]
fn details_take_the_current_rows_difference_colors_and_caret_bar() {
    let ctx = egui::Context::default();
    let mut v = view("same\nleft\n", "same\nright\n");
    // Opening lands on the first difference; start from the equal row.
    v.goto_row(0);
    let details_rects = |out: &egui::FullOutput, below: f32| -> Vec<(egui::Rect, egui::Color32)> {
        out.shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Rect(r) if r.rect.top() >= below => Some((r.rect, r.fill)),
                _ => None,
            })
            .collect()
    };
    let out = frame(&ctx, &mut v, vec![]);
    let below = v.pane_rects[0].bottom() + 19.0;
    // Equal row: white box, no difference fill.
    let fill = crate::chrome::theme::Theme::default().difference_fill;
    assert!(!details_rects(&out, below).iter().any(|(_, f)| *f == fill));
    v.goto_row(1);
    v.editors[v.active].caret.byte = 2;
    let out = frame(&ctx, &mut v, vec![]);
    let rects = details_rects(&out, below);
    assert!(
        rects.iter().any(|(r, f)| *f == fill && r.height() == 15.0),
        "{rects:?}"
    );
    // 2 px caret bar on the active side.
    let caret = egui::Color32::from_gray(0xC0);
    assert!(
        rects
            .iter()
            .any(|(r, f)| *f == caret && r.width() == 2.0 && r.height() == 15.0),
        "{rects:?}"
    );
}

#[test]
fn drag_selection_past_pane_edges_scrolls_horizontally_and_extends() {
    let ctx = egui::Context::default();
    let long = "abcdefghij".repeat(60);
    let text = (0..20)
        .map(|n| format!("row {n}: {long}\n"))
        .collect::<String>();
    let mut v = view(&text, &text);
    for _ in 0..3 {
        frame(&ctx, &mut v, vec![]);
    }
    v.scroll.x = 2000.0;
    frame(&ctx, &mut v, vec![]);
    let pane = v.pane_rects[1];
    let p = egui::pos2(pane.center().x, pane.top() + 68.0);
    frame(
        &ctx,
        &mut v,
        vec![egui::Event::PointerMoved(p), pointer(p, true)],
    );
    let start = v.editors[1].caret;
    // Leftwards past the line-number gutter: the view follows step by step.
    let left = egui::pos2(pane.left() + 10.0, p.y);
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(left)]);
    let first = v.scroll.x;
    assert!(first < 2000.0, "{first}");
    for _ in 0..5 {
        frame(&ctx, &mut v, vec![]);
    }
    assert!(v.scroll.x < first, "{} vs {first}", v.scroll.x);
    let caret = v.editors[1].caret;
    assert_eq!(caret.line, start.line);
    assert!(caret.byte < start.byte, "{caret:?} vs {start:?}");
    assert_eq!(v.editors[1].selection().map(|(_, b)| b), Some(start));
    // Back over the text: scrolling stops.
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(p)]);
    let still = v.scroll.x;
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.scroll.x, still);
    // Rightwards past the vertical scrollbar: the view follows the other way.
    let right = egui::pos2(pane.right() + 40.0, p.y);
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(right)]);
    for _ in 0..5 {
        frame(&ctx, &mut v, vec![]);
    }
    assert!(v.scroll.x > still, "{} vs {still}", v.scroll.x);
    let caret = v.editors[1].caret;
    assert!(caret.byte > start.byte, "{caret:?} vs {start:?}");
    frame(&ctx, &mut v, vec![pointer(right, false)]);
    let done = v.scroll.x;
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(right)]);
    assert_eq!(v.scroll.x, done);
}

#[test]
fn drag_selection_past_pane_top_and_bottom_scrolls_vertically_and_extends() {
    let ctx = egui::Context::default();
    let text = (0..400).map(|n| format!("row {n}\n")).collect::<String>();
    let mut v = view(&text, &text);
    for _ in 0..3 {
        frame(&ctx, &mut v, vec![]);
    }
    v.scroll.y = 3000.0;
    frame(&ctx, &mut v, vec![]);
    let pane = v.pane_rects[1];
    let p = egui::pos2(pane.left() + 120.0, pane.center().y);
    frame(
        &ctx,
        &mut v,
        vec![egui::Event::PointerMoved(p), pointer(p, true)],
    );
    let start = v.editors[1].caret;
    // Above the pane: the view follows upwards and the selection grows to the top row.
    let above = egui::pos2(p.x, pane.top() - 30.0);
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(above)]);
    let first = v.scroll.y;
    assert!(first < 3000.0, "{first}");
    for _ in 0..5 {
        frame(&ctx, &mut v, vec![]);
    }
    assert!(v.scroll.y < first, "{} vs {first}", v.scroll.y);
    let caret = v.editors[1].caret;
    assert!(caret.line < start.line, "{caret:?} vs {start:?}");
    assert_eq!(v.editors[1].selection().map(|(_, b)| b), Some(start));
    // Both panes move together.
    assert!(v.visible.start <= caret.line + 1, "{:?} {caret:?}", v.visible);
    // Back inside: stops.
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(p)]);
    let still = v.scroll.y;
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.scroll.y, still);
    // Below the pane: follows downwards.
    let below = egui::pos2(p.x, pane.bottom() + 30.0);
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(below)]);
    for _ in 0..5 {
        frame(&ctx, &mut v, vec![]);
    }
    assert!(v.scroll.y > still, "{} vs {still}", v.scroll.y);
    let caret = v.editors[1].caret;
    assert!(caret.line > start.line, "{caret:?} vs {start:?}");
    frame(&ctx, &mut v, vec![pointer(below, false)]);
    let done = v.scroll.y;
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(below)]);
    assert_eq!(v.scroll.y, done);
}

#[test]
fn drag_autoscroll_speed_grows_with_distance() {
    use super::super::render::drag_autoscroll;
    assert_eq!(drag_autoscroll(50.0, 10.0, 100.0, 0.1), 0.0);
    let near = drag_autoscroll(5.0, 10.0, 100.0, 0.1);
    let far = drag_autoscroll(-100.0, 10.0, 100.0, 0.1);
    assert!(near < 0.0 && far < near);
    assert!(drag_autoscroll(150.0, 10.0, 100.0, 0.1) > 0.0);
    assert_eq!(drag_autoscroll(1e6, 10.0, 100.0, 0.1), 600.0);
}

#[test]
fn pane_scrollbars_pan_without_changing_the_selected_row_or_keyboard_focus() {
    let ctx = egui::Context::default();
    let long = "1234567890".repeat(28);
    let text = (0..120)
        .map(|n| format!("row {n}: {long}\n"))
        .collect::<String>();
    let mut v = view(&text, &text);
    for _ in 0..3 {
        frame(&ctx, &mut v, vec![]);
    }
    let panes = v.pane_rects;
    // A real drag selection in the right editor.
    let p = panes[1].min + egui::vec2(100.0, 68.0);
    let q = p + egui::vec2(35.0, 0.0);
    frame(
        &ctx,
        &mut v,
        vec![egui::Event::PointerMoved(p), pointer(p, true)],
    );
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(q)]);
    frame(&ctx, &mut v, vec![pointer(q, false)]);
    let selection = v.editors[1].selection();
    assert!(selection.is_some());
    let row = v.current_row;
    let carets = v.editors.each_ref().map(|e| e.caret);
    let assert_owner = |v: &TextCompareView| {
        assert_eq!(v.active, 1);
        assert_eq!(v.current_row, row);
        assert_eq!(v.editors.each_ref().map(|e| e.caret), carets);
        assert_eq!(v.editors[1].selection(), selection);
        assert!(ctx.memory(|m| m.has_focus(editor_id(1))));
    };
    // Opposite horizontal track overlaps an overscan row; the vertical
    // track of the active pane can pan the selection entirely off screen.
    click(
        &ctx,
        &mut v,
        panes[0].right_bottom() - egui::vec2(60.0, 9.0),
    );
    assert!(v.scroll.x > 0.0);
    assert_owner(&v);
    click(
        &ctx,
        &mut v,
        egui::pos2(panes[1].right() - 9.0, panes[1].center().y),
    );
    assert!(v.scroll.y > 0.0);
    assert_owner(&v);
    let out = frame(&ctx, &mut v, vec![]);
    let detail_line = format!("row {row}: {long}");
    assert_eq!(
        text_shapes(&out)
            .into_iter()
            .filter(|(t, r)| t == &detail_line && r.top() > panes[1].bottom())
            .count(),
        2
    );
    // Details have their own scroll offset, but shouldn't steal editor focus either.
    click(
        &ctx,
        &mut v,
        panes[1].right_bottom() + egui::vec2(-8.0, 59.0),
    );
    assert!(v.details_scroll_x > 0.0);
    assert_owner(&v);
}

#[test]
fn details_scrollbar_moves_both_rows_without_scrolling_either_editor() {
    let ctx = egui::Context::default();
    // Only the inactive side overflows. Include tab and multibyte text in the width.
    let long = format!("{}\t行尾", "0123456789".repeat(45));
    let mut v = view("left\nshort\n", &format!("{long}\nshort\n"));
    v.goto_row(0);
    let out = frame(&ctx, &mut v, vec![]);
    let top = v.pane_rects[1].bottom() + 19.0;
    let detail_text = |out: &egui::FullOutput, label: &str| {
        out.shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t)
                    if (top..top + 32.0).contains(&t.pos.y) && t.galley.text() == label =>
                {
                    Some((
                        egui::Rect::from_min_size(t.pos, t.galley.size()),
                        s.clip_rect,
                    ))
                }
                _ => None,
            })
            .unwrap()
    };
    let origins = ["left", &long].map(|label| detail_text(&out, label).0.left());
    let clip = detail_text(&out, &long).1;
    let bar_y = top + 40.0;
    let panes_before = v.scroll;

    // Right arrow, then a wheel notch over the text itself.
    click(&ctx, &mut v, egui::pos2(clip.right() - 8.0, bar_y));
    let out = frame(
        &ctx,
        &mut v,
        vec![
            egui::Event::PointerMoved(egui::pos2(clip.center().x, top + 8.0)),
            wheel_lines(-1.0, Default::default()),
        ],
    );
    for (label, x) in ["left", &long].into_iter().zip(origins) {
        let (text, clipped) = detail_text(&out, label);
        assert_eq!(x - text.left(), 80.0, "{label}");
        assert_eq!(clipped.left(), clip.left(), "fixed arrow gutter");
    }
    // Left edge of the track -> start. Drag the thumb to the end and let go.
    let start = egui::pos2(v.pane_rects[0].left() + 17.0, bar_y);
    click(&ctx, &mut v, start);
    assert_eq!(v.details_scroll_x, 0.0);
    let end = egui::pos2(clip.right() - 17.0, bar_y);
    frame(
        &ctx,
        &mut v,
        vec![egui::Event::PointerMoved(start), pointer(start, true)],
    );
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(end)]);
    frame(&ctx, &mut v, vec![pointer(end, false)]);
    let out = frame(&ctx, &mut v, vec![]);
    let marker = out
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t)
                if (top + 16.0..top + 32.0).contains(&t.pos.y) && t.galley.text() == "↵" =>
            {
                Some(egui::Rect::from_min_size(t.pos, t.galley.size()))
            }
            _ => None,
        })
        .unwrap();
    assert!(
        (marker.right() - clip.right()).abs() < 1.0,
        "line ending reachable: {marker:?} {clip:?}"
    );
    assert_eq!(v.scroll, panes_before);
    assert_eq!(v.current_row, 0);
    assert_eq!(v.active, 0);

    // Changing to a fitting row drops the bar and its stale offset.
    v.goto_row(1);
    let out = frame(&ctx, &mut v, vec![]);
    assert_eq!(detail_text(&out, "short").0.left(), origins[0]);
    assert!(out
        .shapes
        .iter()
        .any(|s| matches!(&s.shape, egui::Shape::Rect(r)
        if r.rect.size() == egui::vec2(17.0, 2.0) && (r.rect.center().y - bar_y).abs() < 0.5)));
}

#[test]
fn resize_reflows_panes_and_recalculates_the_detail_scroll_range() {
    let ctx = egui::Context::default();
    // A line that fits a wide details strip but not a narrow one.
    let line = "0123456789".repeat(15);
    let text = format!("{line}\n{}", "short\n".repeat(150));
    let mut v = view(&text, &text);
    let run = |v: &mut TextCompareView, w, h| {
        let size = egui::vec2(w, h);
        let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(bounds),
                ..Default::default()
            },
            |ui| {
                v.ui(ui, &mut Prefs::default());
                assert!(
                    ui.min_rect().right() <= bounds.right() + 1.0,
                    "mustn't grow the host"
                );
            },
        );
        out.textures_delta.clear();
        assert!(
            (v.pane_rects[1].right() - w).abs() < 1.0,
            "{:?}",
            v.pane_rects
        );
        assert!((v.pane_rects[1].bottom() + 19.0 + 49.0 + 25.0 - h).abs() < 1.0);
        out
    };
    for stacked in [false, true] {
        v.stacked = stacked;
        run(&mut v, 1450.0, 950.0);
        let wide = v.pane_rects;
        let rows = v.visible.len();
        v.details_scroll_x = f32::MAX;
        let out = run(&mut v, 740.0, 540.0);
        assert!(v.pane_rects[0].width() < wide[0].width());
        assert!(v.visible.len() < rows);
        assert!(v.details_scroll_x > 0.0);
        // At the clamped end the last marker sits at the NEW right edge, not hidden outside it.
        assert!(text_shapes(&out).into_iter().any(|(t, r)| t == "↵"
            && r.top() > v.pane_rects[1].bottom()
            && (r.right() - 740.0).abs() < 1.0));
        run(&mut v, 1450.0, 950.0);
        assert_eq!(v.details_scroll_x, 0.0);
        assert_eq!(v.pane_rects, wide);
        assert_eq!(v.current_row, 0);
    }
}

#[test]
fn details_long_preview_ends_at_a_utf8_boundary_and_marks_truncation() {
    let ctx = egui::Context::default();
    let prefix = "a".repeat(4095);
    let line = format!("{prefix}界\r\n");
    let mut v = view(&line, &line);
    let out = frame(&ctx, &mut v, vec![]);
    let top = v.pane_rects[1].bottom() + 19.0;
    let rows: Vec<_> = text_shapes(&out)
        .into_iter()
        .filter(|(_, r)| (top..top + 32.0).contains(&r.top()))
        .map(|(t, _)| t)
        .collect();
    assert_eq!(rows, [prefix.as_str(), "…", prefix.as_str(), "…"]);
}

#[test]
fn session_settings_dialog_applies_on_ok_only() {
    let ctx = egui::Context::default();
    let prefs = Prefs::default();
    let mut v = view("same\nLeft\n", "same\nleft\n");
    // The menu command opens the dialog with the live state as its draft.
    v.command("Session Settings...", &ctx, &prefs);
    assert!(v.session.open);
    assert_eq!(v.session.draft().unwrap().rules, v.map.rules);
    // Cancel path: closing without OK leaves the view untouched.
    let before = v.map.rules.clone();
    v.session
        .draft_mut()
        .unwrap()
        .rules
        .comparison
        .importance
        .case_unimportant = true;
    v.session = Default::default();
    assert_eq!(v.map.rules, before);
    // OK path: the draft goes through apply_session.
    let mut draft = v.session_draft();
    draft.rules.comparison.importance.case_unimportant = true;
    draft.read_only = [true, false];
    draft.context = 5;
    v.apply_session(draft);
    ready(&mut v);
    assert!(v.map.rules.comparison.importance.case_unimportant);
    assert!(v.content(0).read_only && !v.content(1).read_only);
    assert_eq!(v.context, 5);
    // Case is now unimportant: the only difference is unimportant.
    assert_eq!(v.counts.important, 0);
    // Tools > Options opens the global options dialog.
    v.command("Options...", &ctx, &prefs);
    assert!(v.options.open);
}

fn shift_key(key: egui::Key) -> egui::Event {
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
fn right_click(ctx: &egui::Context, v: &mut TextCompareView, pos: egui::Pos2) {
    frame(ctx, v, vec![egui::Event::PointerMoved(pos)]);
    for pressed in [true, false] {
        frame(
            ctx,
            v,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Secondary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }
}
fn copied(output: &egui::FullOutput) -> Option<String> {
    output
        .platform_output
        .commands
        .iter()
        .find_map(|c| match c {
            egui::OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        })
}
/// Center of context-menu item `index` (`separators` separators precede it).
fn menu_item(origin: egui::Pos2, index: usize, separators: usize) -> egui::Pos2 {
    let skipped = index - separators;
    origin
        + egui::vec2(
            60.0,
            5.0 + skipped as f32 * crate::chrome::popup::ITEM_HEIGHT
                + separators as f32 * crate::chrome::popup::SEPARATOR_HEIGHT
                + crate::chrome::popup::ITEM_HEIGHT / 2.0,
        )
}

#[test]
fn context_menu_copies_selection_and_keeps_ctrl_c_working() {
    let mut v = view("a\nkeep\nz\n", "a\nheasdl\nz\n");
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    frame(&ctx, &mut v, vec![]);
    let row = v.row_for_line(1, 1);
    let pos = v.pane_rects[1].left_top() + egui::vec2(95.0, 8.0 + row as f32 * v.row_height());
    click(&ctx, &mut v, pos);
    // Whole line selected: a right-click inside the selection keeps it.
    v.editors[1].anchor = Some(Caret { line: 1, byte: 0 });
    v.editors[1].caret = Caret { line: 2, byte: 0 };
    right_click(&ctx, &mut v, pos);
    assert_eq!(v.active, 1);
    assert!(v.editors[1].selection().is_some());
    let items = v.context_items(1);
    let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(
        &labels[..6],
        [
            "Align With...",
            "Isolate",
            "Copy to Left",
            "Copy Line to Left",
            "Increase Indent",
            "Decrease Indent"
        ]
    );
    let enabled = |items: &[crate::chrome::menubar::MenuItem], id: &str| {
        items.iter().find(|i| i.action_id == id).unwrap().enabled
    };
    assert!(enabled(&items, "Copy") && enabled(&items, "Cut"));
    assert!(!enabled(&items, "Save File"));
    // Left pane: copy targets the right side; nothing selected disables Copy.
    let left = v.context_items(0);
    assert!(left.iter().any(|i| i.action_id == "Copy Line to Right"));
    assert!(!enabled(&left, "Copy") && enabled(&left, "Paste"));
    // The classic popup body opens at the right-click point.
    let out = frame(&ctx, &mut v, vec![]);
    assert!(out.shapes.iter().any(|c| matches!(
        &c.shape,
        egui::Shape::Rect(r) if r.fill == crate::chrome::popup::BACKGROUND && r.rect.min == pos
    )));
    // Item 8 = Copy (after six commands, a separator and Cut).
    let target = menu_item(pos, 8, 1);
    frame(
        &ctx,
        &mut v,
        vec![
            egui::Event::PointerMoved(target),
            egui::Event::PointerButton {
                pos: target,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    // The press over the menu must not move the caret in the pane below.
    assert!(v.editors[1].selection().is_some());
    let output = frame(
        &ctx,
        &mut v,
        vec![egui::Event::PointerButton {
            pos: target,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    );
    assert_eq!(copied(&output).as_deref(), Some("heasdl\n"));
    // Focus returns to the editor, so Ctrl+C keeps copying.
    frame(&ctx, &mut v, vec![]);
    let output = frame(&ctx, &mut v, vec![egui::Event::Copy]);
    assert_eq!(copied(&output).as_deref(), Some("heasdl\n"));
    // Nothing focused at all (e.g. focus surrendered): Copy still uses the active pane.
    ctx.memory_mut(|m| m.surrender_focus(editor_id(1)));
    let output = frame(&ctx, &mut v, vec![egui::Event::Copy]);
    assert_eq!(copied(&output).as_deref(), Some("heasdl\n"));
}

#[test]
fn context_commands_copy_line_isolate_indent_and_select_section() {
    let ctx = egui::Context::default();
    let prefs = Prefs::default();
    let mut v = view("a\nx\nb\n", "a\ny\nb\n");
    frame(&ctx, &mut v, vec![]);
    let row = v.row_for_line(1, 1);
    let pos = v.pane_rects[1].left_top() + egui::vec2(95.0, 8.0 + row as f32 * v.row_height());
    click(&ctx, &mut v, pos);
    assert_eq!(v.editors[1].caret.line, 1);
    // Shift+Ctrl+L: only the caret line goes left, replacing its aligned line.
    frame(&ctx, &mut v, vec![shift_key(egui::Key::L)]);
    ready(&mut v);
    assert_eq!(v.map.left.to_bytes().unwrap(), b"a\ny\nb\n");
    v.map.left.edits.undo();
    v.recompare();
    ready(&mut v);
    // Shift+Ctrl+I / Shift+Ctrl+U indent the right caret line.
    frame(&ctx, &mut v, vec![shift_key(egui::Key::I)]);
    ready(&mut v);
    assert_eq!(v.map.right.to_bytes().unwrap(), b"a\n\ty\nb\n");
    frame(&ctx, &mut v, vec![shift_key(egui::Key::U)]);
    ready(&mut v);
    assert_eq!(v.map.right.to_bytes().unwrap(), b"a\ny\nb\n");
    // Ctrl+D selects the current difference section on the active side.
    v.goto_row(row);
    frame(&ctx, &mut v, vec![key(egui::Key::D, true)]);
    assert_eq!(v.editors[1].selected_text(v.content(1)), "y\n");
    // Ctrl+L with a selection copies the selected lines left.
    frame(&ctx, &mut v, vec![key(egui::Key::L, true)]);
    ready(&mut v);
    assert_eq!(v.map.left.to_bytes().unwrap(), b"a\ny\nb\n");
    // Isolate splits a same row into two orphans until alignment is cleared.
    let mut v = view("a\nb\nc\n", "a\nb\nc\n");
    v.active = 0;
    v.editors[0].place(Caret { line: 1, byte: 0 }, false);
    v.context_command(0, "Isolate", &ctx, &prefs);
    ready(&mut v);
    assert!(v.map.rows().any(|r| r.left == Some(1) && r.right.is_none()));
    assert!(v.map.rows().any(|r| r.right == Some(1) && r.left.is_none()));
    v.swap();
    ready(&mut v);
    assert!(v.map.rows().any(|r| r.right == Some(1) && r.left.is_none()));
    v.command("Clear manual alignment", &ctx, &prefs);
    ready(&mut v);
    assert!(v
        .map
        .rows()
        .any(|r| r.left == Some(1) && r.right == Some(1)));
}

#[test]
fn compare_selection_to_clipboard_opens_a_text_session() {
    let ctx = egui::Context::default();
    let prefs = Prefs::default();
    let mut v = view("one\ntwo\n", "x\n");
    frame(&ctx, &mut v, vec![]);
    v.command("Compare Selection to Clipboard", &ctx, &prefs);
    assert!(v.compare_selection.is_none(), "{}", v.message);
    v.editors[0].anchor = Some(Caret { line: 0, byte: 0 });
    v.editors[0].caret = Caret { line: 1, byte: 0 };
    v.context_command(0, "Compare Selection to Clipboard", &ctx, &prefs);
    let mut action = None;
    ctx.run_ui(
        egui::RawInput {
            events: vec![egui::Event::Paste("clip\n".into())],
            ..Default::default()
        },
        |ui| action = v.ui(ui, &mut Prefs::default()),
    )
    .textures_delta
    .clear();
    match action {
        Some(ViewAction::CompareTexts { left, right }) => {
            assert_eq!(left, "one\n");
            assert_eq!(right, "clip\n");
            let mut session = TextCompareView::from_texts(&left, &right).unwrap();
            ready(&mut session);
            assert_eq!(session.map.right.to_bytes().unwrap(), b"clip\n");
        }
        other => panic!("{other:?}"),
    }
    // A stale request (an empty clipboard never answers) cannot hijack a later paste.
    v.context_command(0, "Compare Selection to Clipboard", &ctx, &prefs);
    for _ in 0..5 {
        frame(&ctx, &mut v, vec![]);
    }
    assert!(v.compare_selection.is_none());
}
#[test]
fn clicking_a_gap_row_moves_focus_so_paste_reaches_that_pane() {
    let mut v = view("", "");
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    let left = v.pane_rects[0].center();
    click(&ctx, &mut v, left);
    let long: String = (0..400).map(|i| format!("left {i}\n")).collect();
    frame(&ctx, &mut v, vec![egui::Event::Paste(long)]);
    frame(&ctx, &mut v, vec![]);
    // 左侧 400 行把右窗格全部变成空隙行：点在哪里都必须能把焦点切到右侧。
    let right = v.pane_rects[1].center();
    click(&ctx, &mut v, right);
    assert_eq!(v.active, 1);
    assert_eq!(ctx.memory(|m| m.focused()), Some(editor_id(1)));
    frame(
        &ctx,
        &mut v,
        vec![egui::Event::Paste("right a\nright b\n".into())],
    );
    // 空文件按平台默认行尾写入，只比较内容。
    let right_text = String::from_utf8(v.map.right.to_bytes().unwrap()).unwrap();
    assert_eq!(right_text.replace("\r\n", "\n"), "right a\nright b\n");
    assert_eq!(v.map.left.lines.borrow().len(), 400);
    // 点回左侧空隙之外的文字行仍正常切换。
    click(&ctx, &mut v, left);
    assert_eq!(v.active, 0);
    assert_eq!(ctx.memory(|m| m.focused()), Some(editor_id(0)));
}
#[test]
fn open_file_uses_native_dialog_seeded_with_current_path() {
    use crate::chrome::native_dialog::{self, Kind};
    let dir = std::env::temp_dir().join(format!("matcha-native-open-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let current = dir.join("current.txt");
    let picked = dir.join("picked.txt");
    std::fs::write(&current, "a\n").unwrap();
    std::fs::write(&picked, "picked\n").unwrap();
    let mut v = view("a\n", "b\n");
    v.files[0].path = Some(current.clone());
    let ctx = egui::Context::default();
    frame(&ctx, &mut v, vec![]);
    native_dialog::mock::respond(Some(picked.clone()));
    frame(&ctx, &mut v, vec![key(egui::Key::O, true)]);
    let request = native_dialog::mock::last().expect("native open dialog shown");
    assert_eq!(request.kind, Kind::OpenFile);
    assert_eq!(request.title, "Open File on File System - Left Side");
    assert_eq!(request.initial.as_deref(), Some(current.as_path()));
    assert!(v.file_dialog.is_none(), "no in-app path window left behind");
    ready(&mut v);
    assert_eq!(v.files[0].path.as_deref(), Some(picked.as_path()));
    // 取消：不改路径。
    frame(&ctx, &mut v, vec![key(egui::Key::O, true)]);
    assert!(native_dialog::mock::last().is_some());
    ready(&mut v);
    assert_eq!(v.files[0].path.as_deref(), Some(picked.as_path()));
    let _ = std::fs::remove_dir_all(dir);
}

/// Dragging the gap between the panes resizes them; the split is remembered and a
/// double-click restores the even split (docs/features/resizable-splitters.md).
#[test]
fn pane_divider_drag_resizes_and_double_click_restores() {
    TEXT_SPLIT_X.reset();
    let ctx = egui::Context::default();
    let mut v = view("a\nb\n", "a\nc\n");
    frame(&ctx, &mut v, vec![]);
    let before = v.pane_rects;
    let line = egui::pos2(before[0].right() + 2.0, before[0].center().y);
    let to = line + egui::vec2(-200.0, 0.0);
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(line)]);
    frame(&ctx, &mut v, vec![pointer(line, true)]);
    frame(
        &ctx,
        &mut v,
        vec![egui::Event::PointerMoved(line + egui::vec2(-100.0, 0.0))],
    );
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(to)]);
    frame(&ctx, &mut v, vec![pointer(to, false)]);
    frame(&ctx, &mut v, vec![]);
    let after = v.pane_rects;
    assert!(
        (after[0].width() - (before[0].width() - 200.0)).abs() < 1.0,
        "{before:?} -> {after:?}"
    );
    assert!((after[1].left() - (before[1].left() - 200.0)).abs() < 1.0);
    assert!(crate::layout::get("text.split.x").is_some());
    // A new view of the same kind starts from the remembered split.
    let mut other = view("x\n", "y\n");
    frame(&ctx, &mut other, vec![]);
    assert!((other.pane_rects[0].width() - after[0].width()).abs() < 1.0);
    // Double-click the divider: back to the even split.
    let line = egui::pos2(after[0].right() + 2.0, after[0].center().y);
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(line)]);
    for _ in 0..2 {
        frame(&ctx, &mut v, vec![pointer(line, true)]);
        frame(&ctx, &mut v, vec![pointer(line, false)]);
    }
    frame(&ctx, &mut v, vec![]);
    assert!((v.pane_rects[0].width() - before[0].width()).abs() < 1.0);
    assert_eq!(crate::layout::get("text.split.x"), None);
}
fn pointer(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}

// ---- 行号栏复制箭头 ----

/// `s` 侧第 `row` 行箭头中心（屏幕坐标）。
fn arrow_pos(v: &TextCompareView, s: usize, row: usize) -> egui::Pos2 {
    let g = super::super::projection::RowGeometry::for_row_height(v.row_height());
    let y = v.projection.y_of(v.projection.display_for(row), g);
    v.pane_rects[s].left_top() + egui::vec2(13.0, 1.0 + y + v.row_height() / 2.0)
}
fn select_lines(v: &mut TextCompareView, s: usize, lines: Range<usize>) {
    v.editors[s].anchor = Some(Caret {
        line: lines.start,
        byte: 0,
    });
    v.editors[s].caret = Caret {
        line: lines.end,
        byte: 0,
    };
}

#[test]
fn gutter_arrow_kinds_and_disabled_states() {
    let mut v = view("a\nb1\nb2\nb3\nc\n", "a\nB1\nB2\nB3\nc\n");
    assert_eq!(v.gutter_arrow(0, 0, None), None, "same row has no arrow");
    let section = v.gutter_arrow(0, 2, None).unwrap();
    assert_eq!(section.kind, ArrowKind::Section);
    assert_eq!(section.disabled, None);
    // 区块第 1–3 行：首行箭头，其余为括号线，末行带脚。
    assert_eq!(v.gutter_arrow(0, 1, None).unwrap().mark, GutterMark::Arrow);
    assert_eq!(section.mark, GutterMark::Bracket { last: false });
    assert_eq!(
        v.gutter_arrow(1, 3, None).unwrap().mark,
        GutterMark::Bracket { last: true }
    );
    assert_eq!(section.tooltip(0), "Copy section to Right");
    assert_eq!(
        v.gutter_arrow(1, 2, None).unwrap().tooltip(1),
        "Copy section to Left"
    );
    // 选区覆盖第 0–1 行：相同行 0 与差异行 1 都是选区箭头，选区外的差异行仍是区块箭头。
    select_lines(&mut v, 0, 0..2);
    let selected = v.selected_line_range(0);
    assert_eq!(selected, Some(0..2));
    let kind = |v: &TextCompareView, row| v.gutter_arrow(0, row, selected.as_ref()).map(|a| a.kind);
    assert_eq!(kind(&v, 0), Some(ArrowKind::Selection));
    assert_eq!(kind(&v, 1), Some(ArrowKind::Selection));
    assert_eq!(kind(&v, 2), Some(ArrowKind::Section));
    let mark = |v: &TextCompareView, row| v.gutter_arrow(0, row, selected.as_ref()).unwrap().mark;
    assert_eq!(mark(&v, 0), GutterMark::Arrow);
    assert_eq!(mark(&v, 1), GutterMark::Bracket { last: true });
    assert_eq!(kind(&v, 4), None);
    assert_eq!(
        v.gutter_arrow(0, 0, selected.as_ref()).unwrap().tooltip(0),
        "Copy selected lines to Right"
    );
    // 光标无选区时不算选区。
    v.editors[0].anchor = None;
    assert_eq!(v.selected_line_range(0), None);
    // 目标侧只读。
    v.map.right.read_only = true;
    let disabled = v.gutter_arrow(0, 2, None).unwrap();
    assert_eq!(
        disabled.disabled.as_deref(),
        Some("Right side is read-only")
    );
    assert_eq!(v.gutter_arrow(1, 2, None).unwrap().disabled, None);
    v.map.right.read_only = false;
    // 编辑后对齐过期：差异信息不可用，只剩禁用的选区箭头。
    v.map.left.replace_line(4, b"c edited").unwrap();
    assert!(v.map.is_stale());
    assert_eq!(v.gutter_arrow(0, 2, None), None);
    select_lines(&mut v, 0, 0..2);
    let stale = v
        .gutter_arrow(0, 0, v.selected_line_range(0).as_ref())
        .unwrap();
    assert_eq!(stale.kind, ArrowKind::Selection);
    assert_eq!(stale.disabled.as_deref(), Some("Comparison is updating"));
}

#[test]
fn gutter_arrow_gap_side_also_has_section_arrow() {
    // 与参考一致：本侧是空隙的区块也画箭头（点击 = 用空白覆盖对侧，即删除对侧这些行）。
    let v = view("a\nc\n", "a\nx\nc\n");
    let row = (0..v.map.row_count())
        .find(|r| v.row(*r).left.is_none())
        .expect("gap row");
    for s in 0..2 {
        let arrow = v.gutter_arrow(s, row, None).unwrap();
        assert_eq!(arrow.kind, ArrowKind::Section);
        assert_eq!(arrow.mark, GutterMark::Arrow);
    }
}

#[test]
fn clicking_gap_side_arrow_deletes_other_side_lines() {
    let ctx = egui::Context::default();
    let mut v = view("a\nc\n", "a\nx\ny\nc\n");
    frame(&ctx, &mut v, vec![]);
    let row = (0..v.map.row_count())
        .find(|r| v.row(*r).left.is_none())
        .unwrap();
    let pos = arrow_pos(&v, 0, row);
    click(&ctx, &mut v, pos);
    assert_eq!(v.map.right.to_bytes().unwrap(), b"a\nc\n");
    assert_eq!(v.counts.important, 0);
}

#[test]
fn apply_gutter_copy_section_and_selection() {
    let mut v = view("a\nb1\nb2\nb3\nc\n", "a\nB1\nB2\nB3\nc\n");
    v.apply_gutter_copy(0, 2, ArrowKind::Section);
    assert_eq!(v.map.right.to_bytes().unwrap(), b"a\nb1\nb2\nb3\nc\n");
    assert_eq!(v.counts.important, 0);
    assert_eq!(v.message, "Copied section to Right");
    v.map.right.edits.undo();
    v.recompare();
    ready(&mut v);
    assert_eq!(v.map.right.to_bytes().unwrap(), b"a\nB1\nB2\nB3\nc\n");
    // 只把选中的第 2–3 行（b1、b2）覆盖到右侧，区块其余行不变，选区被清除。
    select_lines(&mut v, 0, 1..3);
    v.apply_gutter_copy(0, 1, ArrowKind::Selection);
    ready(&mut v);
    assert_eq!(v.map.right.to_bytes().unwrap(), b"a\nb1\nb2\nB3\nc\n");
    assert_eq!(v.editors[0].selection(), None);
    // 目标只读：不改内容，消息说明原因。
    v.map.left.read_only = true;
    v.apply_gutter_copy(1, 3, ArrowKind::Section);
    assert_eq!(v.map.left.to_bytes().unwrap(), b"a\nb1\nb2\nb3\nc\n");
    assert_eq!(v.message, "Left side is read-only");
}

#[test]
fn clicking_gutter_arrows_copies_without_moving_caret() {
    let ctx = egui::Context::default();
    let mut v = view("a\nb1\nb2\nb3\nc\n", "a\nB1\nB2\nB3\nc\n");
    frame(&ctx, &mut v, vec![]);
    v.editors[0].place(Caret { line: 4, byte: 1 }, false);
    let caret = v.editors[0].caret;
    // 区块首行（第 1 行）是箭头；其余行是括号线。
    let pos = arrow_pos(&v, 0, 1);
    let out = frame(&ctx, &mut v, vec![egui::Event::PointerMoved(pos)]);
    assert_eq!(
        out.platform_output.cursor_icon,
        egui::CursorIcon::PointingHand
    );
    click(&ctx, &mut v, pos);
    assert_eq!(v.map.right.to_bytes().unwrap(), b"a\nb1\nb2\nb3\nc\n");
    assert_eq!(
        v.editors[0].caret, caret,
        "arrow click must not move the caret"
    );
    assert_eq!(v.editors[0].anchor, None);
    assert_eq!(v.dragging, None);
}

#[test]
fn hovering_text_area_shows_ibeam_but_gutter_does_not() {
    let ctx = egui::Context::default();
    let mut v = view(
        "abc
", "abd
",
    );
    frame(&ctx, &mut v, vec![]);
    let arrow = arrow_pos(&v, 0, 0);
    let text = arrow + egui::vec2(super::super::gutter::WIDTH + 20.0, 0.0);
    let out = frame(&ctx, &mut v, vec![egui::Event::PointerMoved(text)]);
    assert_eq!(out.platform_output.cursor_icon, egui::CursorIcon::Text);
    // 行号栏右半（无箭头处）保持默认箭头光标。
    let numbers = arrow + egui::vec2(20.0, 0.0);
    let out = frame(&ctx, &mut v, vec![egui::Event::PointerMoved(numbers)]);
    assert_eq!(out.platform_output.cursor_icon, egui::CursorIcon::Default);
}

#[test]
fn clicking_right_only_section_arrow_inserts_on_left() {
    let ctx = egui::Context::default();
    let mut v = view("a\nc\n", "a\nx\ny\nc\n");
    frame(&ctx, &mut v, vec![]);
    let row = (0..v.map.row_count())
        .find(|r| v.row(*r).left.is_none())
        .unwrap();
    let pos = arrow_pos(&v, 1, row);
    click(&ctx, &mut v, pos);
    assert_eq!(v.map.left.to_bytes().unwrap(), b"a\nx\ny\nc\n");
    assert_eq!(v.counts.important, 0);
}

#[test]
fn clicking_selection_arrow_copies_only_selected_lines() {
    let ctx = egui::Context::default();
    let mut v = view("a\nb1\nb2\nb3\nc\n", "a\nB1\nB2\nB3\nc\n");
    frame(&ctx, &mut v, vec![]);
    select_lines(&mut v, 0, 2..4);
    frame(&ctx, &mut v, vec![]);
    let pos = arrow_pos(&v, 0, 2);
    click(&ctx, &mut v, pos);
    ready(&mut v);
    assert_eq!(v.map.right.to_bytes().unwrap(), b"a\nB1\nb2\nb3\nc\n");
    assert_eq!(v.editors[0].selection(), None);
}

#[test]
fn clicking_disabled_arrows_changes_nothing() {
    let ctx = egui::Context::default();
    let mut v = view("a\nb1\nb2\nb3\nc\n", "a\nB1\nB2\nB3\nc\n");
    v.map.right.read_only = true;
    frame(&ctx, &mut v, vec![]);
    let pos = arrow_pos(&v, 0, 1);
    click(&ctx, &mut v, pos);
    assert_eq!(v.map.right.to_bytes().unwrap(), b"a\nB1\nB2\nB3\nc\n");
    assert_eq!(v.message, "Right side is read-only");
    // 括号线行不可点：点击落到文本，不复制。
    let pos = arrow_pos(&v, 1, 2);
    click(&ctx, &mut v, pos);
    assert_eq!(v.map.left.to_bytes().unwrap(), b"a\nb1\nb2\nb3\nc\n");
    v.map.right.read_only = false;
    // 编辑后对齐尚未完成：选区箭头禁用，点击不改任何一侧。
    v.map.left.replace_line(4, b"c edited").unwrap();
    select_lines(&mut v, 0, 1..3);
    frame(&ctx, &mut v, vec![]);
    assert!(v.map.is_stale());
    let pos = arrow_pos(&v, 0, 1);
    click(&ctx, &mut v, pos);
    assert_eq!(v.map.left.to_bytes().unwrap(), b"a\nb1\nb2\nb3\nc edited\n");
    assert_eq!(v.map.right.to_bytes().unwrap(), b"a\nB1\nB2\nB3\nc\n");
}

#[test]
fn selection_arrows_paint_distinct_from_section_arrows() {
    use super::super::gutter::{BRACKET, DISABLED_OUTLINE, SECTION_OUTLINE, SELECTION_OUTLINE};
    let ctx = egui::Context::default();
    let mut v = view("a\nb1\nb2\nb3\nc\n", "a\nB1\nB2\nB3\nc\n");
    // 箭头 = 7 点闭合描边（填充是网格渐变）；括号 = BRACKET 色线段。
    let outlines = |out: &egui::FullOutput, color: egui::Color32| {
        out.shapes
            .iter()
            .filter(|s| match &s.shape {
                egui::Shape::Path(p) => {
                    p.closed
                        && p.points.len() == 7
                        && p.stroke.color == egui::epaint::ColorMode::Solid(color)
                }
                _ => false,
            })
            .count()
    };
    let brackets = |out: &egui::FullOutput| {
        out.shapes
            .iter()
            .filter(|s| {
                matches!(&s.shape, egui::Shape::LineSegment { stroke, .. } if stroke.color == BRACKET)
            })
            .count()
    };
    frame(&ctx, &mut v, vec![]);
    let out = frame(&ctx, &mut v, vec![]);
    // 3 行区块 × 两侧：每侧首行一个箭头，其余 2 行括号（末行另有一段脚）。
    assert_eq!(outlines(&out, SECTION_OUTLINE), 2);
    assert_eq!(outlines(&out, SELECTION_OUTLINE), 0);
    assert_eq!(brackets(&out), 2 * (2 + 1));
    select_lines(&mut v, 0, 0..2);
    let out = frame(&ctx, &mut v, vec![]);
    // 左侧选中第 0–1 行：首行蓝色箭头；左侧区块首行被选区覆盖，只剩右侧的黄色箭头。
    assert_eq!(outlines(&out, SELECTION_OUTLINE), 1);
    assert_eq!(outlines(&out, SECTION_OUTLINE), 1);
    // 右侧只读：左侧（覆盖到右侧的）选区箭头变灰，右侧箭头不受影响。
    v.map.right.read_only = true;
    let out = frame(&ctx, &mut v, vec![]);
    assert_eq!(outlines(&out, DISABLED_OUTLINE), 1);
    assert_eq!(outlines(&out, SELECTION_OUTLINE), 0);
    assert_eq!(outlines(&out, SECTION_OUTLINE), 1);
}

#[test]
fn diff_rows_keep_comment_style_but_ink_code() {
    use super::super::render::{layout, LineInk, EDITOR_TEXT};
    use super::super::syntax_style::SyntaxStyle;
    use bc_formats::{grammar::Token, ElementKind};
    let text = "int x; // note";
    let tokens = [
        Token {
            span: 0..3,
            element: ElementKind::Keyword,
            grammar_index: 0,
        },
        Token {
            span: 7..14,
            element: ElementKind::Comment,
            grammar_index: 0,
        },
    ];
    let syntax = SyntaxStyle::base();
    let bold = egui::FontId::monospace(13.0);
    let red = egui::Color32::RED;
    let color_at_with = |diff: Option<egui::Color32>, byte: usize, unimportant: &[ElementKind]| {
        let ink = LineInk {
            normal: EDITOR_TEXT,
            diff,
            syntax: &syntax,
            bold: &bold,
            unimportant,
            important: None,
        };
        let (job, _) = layout(
            text,
            0,
            0,
            &tokens,
            &[],
            &Default::default(),
            13.0,
            false,
            false,
            4,
            &ink,
        );
        job.sections
            .iter()
            .find(|s| s.byte_range.contains(&egui::text::ByteIndex(byte)))
            .unwrap()
            .format
            .color
    };
    let grey = egui::Color32::from_rgb(128, 128, 128);
    let color_at = |diff, byte| color_at_with(diff, byte, &[ElementKind::Comment]);
    // 普通行：关键字正文色，注释灰。
    assert_eq!(color_at(None, 0), EDITOR_TEXT);
    assert_eq!(color_at(None, 9), grey);
    // 单侧差异行（默认注释不重要）：代码用差异色，注释用不重要差异色（参考的蓝字）。
    assert_eq!(color_at(Some(red), 0), red);
    assert_eq!(color_at(Some(red), 4), red);
    assert_eq!(color_at(Some(red), 9), egui::Color32::BLUE);
    // 注释被设为重要时：注释保留注释风格，仍可辨认。
    assert_eq!(color_at_with(Some(red), 9, &[]), grey);
}

#[test]
fn comments_are_unimportant_by_default() {
    let mut v = view("a\nint x;\nc\n", "a\nint x;\n// added note\nc\n");
    let cpp = v.registry.get(FormatKind::Text, "CPP").unwrap().clone();
    v.map.left.set_format(Some(cpp.clone())).unwrap();
    v.map.right.set_format(Some(cpp)).unwrap();
    v.recompare();
    ready(&mut v);
    assert_eq!(v.counts.important, 0);
    assert_eq!(v.counts.unimportant, 1);
}
#[test]
fn first_comparison_lands_on_first_difference() {
    let v = view("a\nb\nc\nd\n", "a\nb\nX\nd\n");
    assert_eq!(v.current_row, 2);
    assert_eq!(v.editors[0].caret.line, 2);
    assert_eq!(v.editors[1].caret.line, 2);
}
#[test]
fn identical_files_stay_on_first_row() {
    let v = view("a\nb\n", "a\nb\n");
    assert_eq!(v.current_row, 0);
    assert_eq!(v.editors[0].caret, Caret::default());
}
#[test]
fn realign_after_edit_does_not_jump_to_first_difference() {
    let mut v = view("a\nb\nc\n", "a\nb\nX\n");
    v.goto_row(0);
    v.active = 0;
    v.editors[0].place(Caret { line: 0, byte: 0 }, false);
    let lines = v.content(0).lines.borrow().len();
    v.edited(lines);
    ready(&mut v);
    assert_eq!(v.current_row, 0);
}
#[test]
fn first_difference_landing_keeps_context_rows_above() {
    let ctx = egui::Context::default();
    let left: String = (0..300).map(|i| format!("line {i}\n")).collect();
    let right = left.replace("line 40\n", "changed\n");
    let mut v = view(&left, &right);
    assert_eq!(v.current_row, 40);
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.top_row(v.row_height()), 40 - LANDING_CONTEXT_ROWS);
    // Plain jumps still put the target row at the top.
    v.scroll_to_row(60);
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.top_row(v.row_height()), 60);
}

/// Hosted in a resizable floating window (folder view's "Text Compare —
/// selected files"), the view must not report more content than the window
/// gives it: egui grows the window to the content size every frame.
#[test]
fn floating_window_height_is_stable() {
    let ctx = egui::Context::default();
    let text: String = (0..35).map(|i| format!("line {i}\n")).collect();
    let mut v = view(&text, &text);
    let id = egui::Id::new("probe-floating-text");
    let mut heights = Vec::new();
    for _ in 0..8 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 1000.0),
            )),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            crate::chrome::window::Window::new("probe")
                .id(id)
                .default_size([1150.0, 650.0])
                .show(ui.ctx(), |ui| {
                    v.ui(ui, &mut Prefs::default());
                });
        });
        out.textures_delta.clear();
        heights.push(ctx.memory(|m| m.area_rect(id)).unwrap().height());
    }
    assert!(
        heights.windows(2).skip(2).all(|w| (w[1] - w[0]).abs() < 0.5),
        "window keeps growing: {heights:?}"
    );
}
fn numbered(n: usize, tag: &str) -> Vec<u8> {
    (1..=n)
        .map(|i| format!("{tag} {i}\n"))
        .collect::<String>()
        .into_bytes()
}
const ASK: bc_session::prefs::DiskChangePrefs = bc_session::prefs::DiskChangePrefs {
    check: true,
    auto_reload: false,
};
#[test]
fn own_save_is_not_an_outside_change_but_an_outside_write_is() {
    use crate::disk_watch::{plan, stamp};
    let temp = Temp::new();
    let l = temp.file("a.txt", b"abc\n");
    let r = temp.file("b.txt", b"abc\n");
    let mut v = TextCompareView::open(&l, &r).unwrap();
    ready(&mut v);
    assert!(plan(&mut v, &ASK, &stamp).is_empty());
    v.map.right.replace_line(0, b"edited by matcha").unwrap();
    assert!(v.save(1, &Prefs::default()).unwrap());
    ready(&mut v);
    assert!(plan(&mut v, &ASK, &stamp).is_empty(), "own save");
    // The view keeps no handle on the file: another program can rewrite it.
    std::fs::write(&r, b"written outside, longer\n").unwrap();
    let p = plan(&mut v, &ASK, &stamp);
    assert_eq!(p.ask.len(), 1);
    assert_eq!(p.ask[0].side, 1);
    assert!(!p.ask[0].dirty);
}
#[test]
fn disk_reload_keeps_the_caret_line_and_clamps_when_the_file_shrinks() {
    use crate::disk_watch::DiskTracked;
    let temp = Temp::new();
    let l = temp.file("a.txt", &numbered(40, "same"));
    let r = temp.file("b.txt", &numbered(40, "same"));
    let mut v = TextCompareView::open(&l, &r).unwrap();
    ready(&mut v);
    v.active = 1;
    v.editors[1].place(Caret { line: 20, byte: 0 }, false);
    // Appended lines: the caret stays on line 20 instead of jumping to the first difference.
    let mut grown = numbered(40, "same");
    grown.extend_from_slice(b"new tail\n");
    std::fs::write(&r, &grown).unwrap();
    v.disk_reload(1).unwrap();
    ready(&mut v);
    assert_eq!(v.map.right.lines.borrow().len(), 41);
    assert_eq!(v.editors[1].caret.line, 20);
    assert_eq!(v.current_row, v.row_for_line(1, 20));
    // Truncated below the caret: clamp to the last line.
    std::fs::write(&r, numbered(5, "same")).unwrap();
    v.disk_reload(1).unwrap();
    ready(&mut v);
    assert_eq!(v.editors[1].caret.line, 4);
}
#[test]
fn auto_reload_message_survives_the_recompare() {
    let temp = Temp::new();
    let l = temp.file("a.txt", b"abc\n");
    let r = temp.file("b.txt", b"abc\n");
    let mut v = TextCompareView::open(&l, &r).unwrap();
    ready(&mut v);
    std::fs::write(&r, b"changed outside\n").unwrap();
    crate::disk_watch::check(
        &mut v,
        &bc_session::prefs::DiskChangePrefs {
            check: true,
            auto_reload: true,
        },
    );
    assert!(v.disk_prompt.is_none());
    ready(&mut v);
    assert_eq!(v.map.right.to_bytes().unwrap(), b"changed outside\n");
    assert!(v.message.contains("Reloaded b.txt"), "{}", v.message);
}
#[test]
fn disk_prompt_reload_and_keep_current_buttons() {
    let ctx = egui::Context::default();
    let temp = Temp::new();
    let l = temp.file("a.txt", b"abc\n");
    let r = temp.file("b.txt", b"abc\n");
    let mut v = TextCompareView::open(&l, &r).unwrap();
    ready(&mut v);
    let press = |ctx: &egui::Context, v: &mut TextCompareView, label: &str| {
        let mut out = frame(ctx, v, vec![]);
        for _ in 0..2 {
            out = frame(ctx, v, vec![]);
        }
        // The modal paints last; toolbar and pane headers also have a "Reload".
        let at = text_shapes(&out)
            .into_iter()
            .rev()
            .find(|(t, _)| t == label)
            .unwrap_or_else(|| panic!("no {label} button"))
            .1
            .center();
        click(ctx, v, at);
    };
    // Keep Current: content unchanged, and the same disk state is not asked again.
    std::fs::write(&r, b"outside one\n").unwrap();
    crate::disk_watch::check(&mut v, &ASK);
    assert_eq!(v.disk_prompt.as_ref().unwrap().changed.len(), 1);
    press(&ctx, &mut v, "Keep Current");
    assert!(v.disk_prompt.is_none());
    assert_eq!(v.map.right.to_bytes().unwrap(), b"abc\n");
    crate::disk_watch::check(&mut v, &ASK);
    assert!(v.disk_prompt.is_none(), "declined change asked again");
    // Changed again: asked again; Reload loads the new content.
    std::fs::write(&r, b"outside two, longer\n").unwrap();
    crate::disk_watch::check(&mut v, &ASK);
    assert!(v.disk_prompt.is_some());
    press(&ctx, &mut v, "Reload");
    ready(&mut v);
    assert!(v.disk_prompt.is_none());
    assert_eq!(v.map.right.to_bytes().unwrap(), b"outside two, longer\n");
}
#[test]
fn dirty_side_prompt_says_edits_are_lost_and_reload_discards_them() {
    let temp = Temp::new();
    let l = temp.file("a.txt", b"abc\n");
    let r = temp.file("b.txt", b"abc\n");
    let mut v = TextCompareView::open(&l, &r).unwrap();
    ready(&mut v);
    v.map.left.replace_line(0, b"unsaved").unwrap();
    std::fs::write(&l, b"outside edit\n").unwrap();
    crate::disk_watch::check(
        &mut v,
        &bc_session::prefs::DiskChangePrefs {
            check: true,
            auto_reload: true,
        },
    );
    let prompt = v
        .disk_prompt
        .clone()
        .expect("dirty side is asked, not auto-reloaded");
    assert!(prompt.changed[0].dirty);
    assert_eq!(v.map.left.to_bytes().unwrap(), b"unsaved\n");
    crate::disk_watch::answer(&mut v, true);
    ready(&mut v);
    assert!(v.pending.is_none(), "no second save prompt");
    assert_eq!(v.map.left.to_bytes().unwrap(), b"outside edit\n");
    assert!(!v.is_dirty());
}

// ---- 编号书签 ----

/// `Ctrl+数字`（`shift` 时 `Shift+Ctrl+数字`）；Shift 时逻辑键给成符号，验证按物理键识别。
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
        key: if shift {
            egui::Key::Exclamationmark
        } else {
            DIGITS[n]
        },
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
fn lines_of(n: usize) -> String {
    (0..n).map(|i| format!("line {i}\n")).collect()
}
fn menu_entry(
    items: &[crate::chrome::menubar::MenuItem],
    id: &str,
) -> Option<crate::chrome::menubar::MenuItem> {
    use crate::chrome::menubar::MenuItemKind;
    items.iter().find_map(|item| {
        if item.action_id == id {
            return Some(item.clone());
        }
        match &item.kind {
            MenuItemKind::Submenu(children) => menu_entry(children, id),
            _ => None,
        }
    })
}
fn menu_item_by_id(v: &TextCompareView, id: &str) -> crate::chrome::menubar::MenuItem {
    v.menus
        .iter()
        .find_map(|m| menu_entry(&m.items, id))
        .unwrap_or_else(|| panic!("no menu item {id}"))
}
fn menu_enabled(v: &TextCompareView, id: &str) -> bool {
    menu_item_by_id(v, id).enabled
}
fn line_text(v: &TextCompareView, s: usize, line: usize) -> String {
    String::from_utf8(
        v.content(s)
            .lines
            .borrow()
            .line_bytes(line)
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

#[test]
fn bookmark_menu_items_follow_the_set_bookmarks() {
    let ctx = egui::Context::default();
    let mut v = view("a\nb\nc\n", "a\nb\nc\n");
    v.update_menus();
    let toggle = menu_item_by_id(&v, "Toggle Bookmark 4");
    let go = menu_item_by_id(&v, "Go to Bookmark 4");
    assert_eq!(
        (toggle.label.as_str(), toggle.shortcut.as_deref()),
        ("Bookmark 4", Some("Shift+Ctrl+4"))
    );
    assert_eq!(
        (go.label.as_str(), go.shortcut.as_deref()),
        ("Bookmark 4", Some("Ctrl+4"))
    );
    for n in 0..10 {
        assert!(menu_enabled(&v, &format!("Toggle Bookmark {n}")));
        assert!(!menu_enabled(&v, &format!("Go to Bookmark {n}")));
    }
    assert!(!menu_enabled(&v, "Clear Bookmarks"));

    // 只设置书签 4：Go to 子菜单只有 Bookmark 4 可用。
    v.editors[0].place(Caret { line: 1, byte: 0 }, false);
    v.command("Toggle Bookmark 4", &ctx, &Prefs::default());
    assert_eq!(v.bookmarks.get(4), Some((0, 1)));
    v.update_menus();
    for n in 0..10 {
        assert_eq!(
            menu_enabled(&v, &format!("Go to Bookmark {n}")),
            n == 4,
            "{n}"
        );
    }
    assert!(menu_enabled(&v, "Clear Bookmarks"));

    // Clear Bookmarks：全部移除，之后 Go To 提示未设置，Clear 再次禁用。
    v.command("Toggle Bookmark 1", &ctx, &Prefs::default());
    v.editors[1].place(Caret { line: 2, byte: 0 }, false);
    v.active = 1;
    v.command("Toggle Bookmark 3", &ctx, &Prefs::default());
    assert_eq!(
        v.bookmarks.iter().count(),
        2,
        "4 was replaced by 1 on the same line"
    );
    v.command("Clear Bookmarks", &ctx, &Prefs::default());
    assert!(v.bookmarks.is_empty());
    v.command("Go to Bookmark 1", &ctx, &Prefs::default());
    assert_eq!(v.not_found.text(), Some("Bookmark 1 is not set."));
    v.update_menus();
    assert!(!menu_enabled(&v, "Clear Bookmarks"));
}

#[test]
fn shift_ctrl_digit_toggles_moves_and_replaces_bookmarks() {
    let ctx = egui::Context::default();
    let mut v = view(&lines_of(500), &lines_of(500));
    frame(&ctx, &mut v, vec![]);
    v.editors[0].place(Caret { line: 119, byte: 3 }, false);
    frame(&ctx, &mut v, vec![bookmark_key(3, true)]);
    assert_eq!(v.bookmarks.get(3), Some((0, 119)), "{}", v.message);
    // 再按一次即移除。
    frame(&ctx, &mut v, vec![bookmark_key(3, true)]);
    assert_eq!(v.bookmarks.get(3), None);
    // 放回去后在第 400 行再按：移过去。
    frame(&ctx, &mut v, vec![bookmark_key(3, true)]);
    v.editors[0].place(Caret { line: 399, byte: 0 }, false);
    frame(&ctx, &mut v, vec![bookmark_key(3, true)]);
    assert_eq!(v.bookmarks.get(3), Some((0, 399)));
    assert_eq!(v.bookmarks.at((0, 119)), None);
    // 同一行按 5：5 取代 3。
    frame(&ctx, &mut v, vec![bookmark_key(5, true)]);
    assert_eq!(v.bookmarks.get(5), Some((0, 399)));
    assert_eq!(v.bookmarks.get(3), None);
}

#[test]
fn ctrl_digit_jumps_to_the_bookmark_side_and_line() {
    let ctx = egui::Context::default();
    let left = lines_of(500);
    let right = left.replace("line 200\n", "changed 200\n");
    let mut v = view(&left, &right);
    frame(&ctx, &mut v, vec![]);
    v.active = 1;
    v.editors[1].place(Caret { line: 399, byte: 0 }, false);
    frame(&ctx, &mut v, vec![bookmark_key(3, true)]);
    assert_eq!(v.bookmarks.get(3), Some((1, 399)));

    v.active = 0;
    v.editors[0].place(Caret { line: 9, byte: 2 }, false);
    v.goto_row(9);
    frame(&ctx, &mut v, vec![]);
    frame(&ctx, &mut v, vec![]);
    assert!(!v.visible.contains(&399), "{:?}", v.visible);
    frame(&ctx, &mut v, vec![bookmark_key(3, false)]);
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.active, 1);
    assert_eq!(v.editors[1].caret, Caret { line: 399, byte: 0 });
    assert!(v.visible.contains(&399), "{:?}", v.visible);
    assert!(ctx.memory(|m| m.has_focus(editor_id(1))));

    // 书签行被 Show Differences 隐藏时先显示全部。
    v.command("Show Differences", &ctx, &Prefs::default());
    v.command("Show Context", &ctx, &Prefs::default());
    assert_ne!(v.filter, DisplayFilter::All);
    frame(&ctx, &mut v, vec![bookmark_key(3, false)]);
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.filter, DisplayFilter::All);
    assert!(v.visible.contains(&399), "{:?}", v.visible);

    // 未设置的书签：光标不动，状态栏上方弹出消息面板（即使关闭了 Next Difference 的面板开关），下一次按键关闭。
    let mut quiet = Prefs::default();
    quiet.next_diff.message_panel = false;
    let caret = v.editors[1].caret;
    let row = v.current_row;
    frame_with(&ctx, &mut v, &mut quiet, vec![bookmark_key(7, false)]);
    assert_eq!(v.editors[1].caret, caret);
    assert_eq!((v.active, v.current_row), (1, row));
    assert_eq!(v.not_found.text(), Some("Bookmark 7 is not set."));
    let out = frame(&ctx, &mut v, vec![]);
    let shown = text_shapes(&out)
        .into_iter()
        .find(|(text, _)| text == "Bookmark 7 is not set.")
        .expect("bookmark panel is painted");
    assert!(shown.1.bottom() <= 900.0 - crate::chrome::theme::STATUS_HEIGHT + 1.0);
    frame(&ctx, &mut v, vec![key(egui::Key::ArrowDown, false)]);
    assert!(!v.not_found.is_shown());
}

#[test]
fn bookmarks_follow_edits_on_their_own_side_only() {
    let ctx = egui::Context::default();
    let prefs = Prefs::default();
    let mut v = view(&lines_of(100), &lines_of(100));
    frame(&ctx, &mut v, vec![]);
    v.editors[0].place(Caret { line: 49, byte: 0 }, false);
    v.command("Toggle Bookmark 1", &ctx, &prefs);
    assert_eq!(v.bookmarks.get(1), Some((0, 49)));

    // 左侧第 10 行处插入 3 行：书签到第 53 行（下标 52），仍是原来那一行文字。
    v.editors[0].place(Caret { line: 9, byte: 0 }, false);
    let old = v.map.left.lines.borrow().len();
    v.editors[0].insert(&mut v.map.left, "x\ny\nz\n").unwrap();
    v.edited(old);
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.bookmarks.get(1), Some((0, 52)));
    assert_eq!(line_text(&v, 0, 52), "line 49");

    // 撤销：回到原行。
    v.active = 0;
    v.command("Undo", &ctx, &prefs);
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.bookmarks.get(1), Some((0, 49)));
    assert_eq!(line_text(&v, 0, 49), "line 49");

    // 右侧编辑不影响左侧书签。
    v.editors[1].place(Caret { line: 9, byte: 0 }, false);
    let old = v.map.right.lines.borrow().len();
    v.editors[1].insert(&mut v.map.right, "x\ny\nz\n").unwrap();
    v.active = 1;
    v.edited(old);
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.bookmarks.get(1), Some((0, 49)));

    // 删除第 48–52 行（下标 47..52）：书签落到第 48 行（下标 47）。
    v.active = 0;
    select_lines(&mut v, 0, 47..52);
    let old = v.map.left.lines.borrow().len();
    v.editors[0].insert(&mut v.map.left, "").unwrap();
    v.edited(old);
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.bookmarks.get(1), Some((0, 47)));
    assert_eq!(line_text(&v, 0, 47), "line 52");
}

#[test]
fn reload_open_clipboard_and_swap_keep_or_clear_bookmarks() {
    let ctx = egui::Context::default();
    let prefs = Prefs::default();
    let dir = Temp::new();
    let a = dir.file("a.txt", lines_of(100).as_bytes());
    let b = dir.file("b.txt", lines_of(100).as_bytes());
    let c = dir.file("c.txt", lines_of(100).as_bytes());
    let mut v = TextCompareView::open(&a, &b).unwrap();
    ready(&mut v);
    v.editors[0].place(Caret { line: 50, byte: 0 }, false);
    v.command("Toggle Bookmark 1", &ctx, &prefs);
    v.active = 1;
    v.editors[1].place(Caret { line: 60, byte: 0 }, false);
    v.command("Toggle Bookmark 2", &ctx, &prefs);

    // 右侧打开另一个文件：右侧书签清除，左侧保留。
    v.perform(Pending::Open(1, c.clone()));
    ready(&mut v);
    assert_eq!(v.bookmarks.get(2), None);
    assert_eq!(v.bookmarks.get(1), Some((0, 50)));

    // 重载：行号保留，超出新行数的收敛到末行。
    v.editors[1].place(Caret { line: 90, byte: 0 }, false);
    v.command("Toggle Bookmark 2", &ctx, &prefs);
    std::fs::write(&c, lines_of(20)).unwrap();
    v.perform(Pending::Reload(1));
    ready(&mut v);
    assert_eq!(v.bookmarks.get(2), Some((1, 19)));
    assert_eq!(v.bookmarks.get(1), Some((0, 50)));
    v.reload_keep_position(0).unwrap();
    ready(&mut v);
    assert_eq!(
        v.bookmarks.get(1),
        Some((0, 50)),
        "same-file reload keeps it"
    );

    // Swap Sides：书签随内容换侧，行号不变。
    v.command("Swap Sides", &ctx, &prefs);
    ready(&mut v);
    assert_eq!(v.bookmarks.get(1), Some((1, 50)));
    assert_eq!(v.bookmarks.get(2), Some((0, 19)));
    // 交换后的编辑仍按新侧跟随。
    v.editors[1].place(Caret { line: 0, byte: 0 }, false);
    let old = v.map.right.lines.borrow().len();
    v.editors[1].insert(&mut v.map.right, "new\n").unwrap();
    v.active = 1;
    v.edited(old);
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.bookmarks.get(1), Some((1, 51)));
    assert_eq!(v.bookmarks.get(2), Some((0, 19)));

    // Open Clipboard 换掉左侧内容：左侧书签清除。
    v.perform(Pending::Clipboard(0, "clip\n".into()));
    ready(&mut v);
    assert_eq!(v.bookmarks.get(2), None);
    assert_eq!(v.bookmarks.get(1), Some((1, 51)));
}

/// 文本区右缘的书签徽标：编号文字落在窗格右侧（纵向滚动条左边）。
fn badge(out: &egui::FullOutput, v: &TextCompareView, s: usize, n: usize) -> Option<egui::Rect> {
    let pane = v.pane_rects[s];
    let edge = pane.right() - 16.0 - 2.0 - super::super::render::BOOKMARK_BADGE_WIDTH - 4.0;
    text_shapes(out)
        .into_iter()
        .find(|(t, r)| *t == n.to_string() && pane.contains(r.center()) && r.center().x > edge)
        .map(|(_, r)| r)
}

#[test]
fn badges_sit_at_the_text_edge_and_leave_the_copy_arrow_working() {
    let ctx = egui::Context::default();
    let mut v = view("a\nb1\nb2\nb3\nc\n", "a\nB1\nB2\nB3\nc\n");
    frame(&ctx, &mut v, vec![]);
    // 差异区块首行（第 1 行）放书签 2，光标留在别处。
    v.editors[0].place(Caret { line: 1, byte: 0 }, false);
    v.command("Toggle Bookmark 2", &ctx, &Prefs::default());
    v.editors[0].place(Caret { line: 4, byte: 1 }, false);
    let out = frame(&ctx, &mut v, vec![]);
    let mark = badge(&out, &v, 0, 2).expect("badge drawn in the left pane");
    let arrow = arrow_pos(&v, 0, 1);
    assert!(
        (mark.center().y - arrow.y).abs() < v.row_height() / 2.0,
        "same row"
    );
    assert!(
        mark.left() > arrow.x + 40.0,
        "badge stays clear of the gutter"
    );
    // 箭头照常复制。
    click(&ctx, &mut v, arrow);
    assert_eq!(v.map.right.to_bytes().unwrap(), b"a\nb1\nb2\nb3\nc\n");
    assert_eq!(v.bookmarks.get(2), Some((0, 1)));
}

#[test]
fn badges_stay_inside_the_pane_when_scrolled_horizontally() {
    let ctx = egui::Context::default();
    let long = format!("{}\nshort\n", "x".repeat(3000));
    let mut v = view(&long, &long);
    frame(&ctx, &mut v, vec![]);
    v.editors[0].place(Caret { line: 1, byte: 0 }, false);
    v.command("Toggle Bookmark 6", &ctx, &Prefs::default());
    let before = frame(&ctx, &mut v, vec![]);
    let at_rest = badge(&before, &v, 0, 6).expect("badge drawn");
    v.scroll.x = 2000.0;
    frame(&ctx, &mut v, vec![]);
    let out = frame(&ctx, &mut v, vec![]);
    assert!(v.scroll.x > 0.0);
    let scrolled = badge(&out, &v, 0, 6).expect("badge still drawn after scrolling");
    assert_eq!(
        scrolled.center().x,
        at_rest.center().x,
        "fixed horizontally"
    );
    assert!(v.pane_rects[0].contains(scrolled.center()));
}

#[test]
fn nav_bar_draws_a_tick_for_each_bookmark() {
    let ctx = egui::Context::default();
    let mut v = view(&lines_of(300), &lines_of(300));
    frame(&ctx, &mut v, vec![]);
    let ticks = |out: &egui::FullOutput| {
        out.shapes
            .iter()
            .filter(|s| match &s.shape {
                egui::Shape::Rect(r) => {
                    r.fill == crate::chrome::theme::ACCENT
                        && r.rect.right() <= 52.0
                        && r.rect.height() <= 2.5
                }
                _ => false,
            })
            .count()
    };
    let none = ticks(&frame(&ctx, &mut v, vec![]));
    v.editors[0].place(Caret { line: 250, byte: 0 }, false);
    v.command("Toggle Bookmark 0", &ctx, &Prefs::default());
    assert_eq!(ticks(&frame(&ctx, &mut v, vec![])), none + 1);
    assert_eq!(v.bookmark_rows(), vec![(0, 250)]);
}

#[test]
fn altgr_digits_are_left_for_typing() {
    let ctx = egui::Context::default();
    let mut v = view(&lines_of(10), &lines_of(10));
    frame(&ctx, &mut v, vec![]);
    v.editors[0].place(Caret { line: 4, byte: 0 }, false);
    // AltGr+0 在德语等布局上是 `}`，egui 报告为 Ctrl+Alt+0。
    let altgr = egui::Event::Key {
        key: egui::Key::Num0,
        physical_key: Some(egui::Key::Num0),
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers {
            ctrl: true,
            command: true,
            alt: true,
            ..Default::default()
        },
    };
    let mut shifted = altgr.clone();
    if let egui::Event::Key { modifiers, .. } = &mut shifted {
        modifiers.shift = true;
    }
    frame(&ctx, &mut v, vec![shifted, altgr]);
    assert!(v.bookmarks.is_empty());
    assert!(
        !v.not_found.is_shown(),
        "AltGr must not run Go To Bookmark 0"
    );
}
#[test]
fn navigating_past_the_last_difference_shows_and_dismisses_the_message_panel() {
    let ctx = egui::Context::default();
    let mut v = view("a\nb\nc\n", "a\nB\nc\n");
    frame(&ctx, &mut v, vec![]);
    v.goto_row(0);
    frame(&ctx, &mut v, vec![key(egui::Key::N, true)]);
    assert_eq!(v.current_row, 1);
    assert!(!v.not_found.is_shown());
    let out = frame(&ctx, &mut v, vec![key(egui::Key::N, true)]);
    assert_eq!(v.current_row, 1);
    assert!(v.not_found.is_shown());
    let shown = text_shapes(&out)
        .into_iter()
        .find(|(text, _)| text == crate::chrome::message_panel::NOT_FOUND)
        .expect("message panel is painted");
    assert!(shown.1.bottom() <= 900.0 - crate::chrome::theme::STATUS_HEIGHT + 1.0);
    // 再按一次仍找不到：面板保持（重新弹出）。
    frame(&ctx, &mut v, vec![key(egui::Key::N, true)]);
    assert!(v.not_found.is_shown());
    // 任何其他按键都会关闭面板。
    frame(&ctx, &mut v, vec![key(egui::Key::ArrowDown, false)]);
    assert!(!v.not_found.is_shown());
    // 向上同理：第一个差异之前没有差异；菜单命令与快捷键走同一路径。
    v.goto_row(1);
    frame(&ctx, &mut v, vec![key(egui::Key::P, true)]);
    assert!(v.not_found.is_shown());
    frame(&ctx, &mut v, vec![key(egui::Key::ArrowDown, false)]);
    v.goto_row(1);
    v.command("Previous Difference", &ctx, &Prefs::default());
    assert!(v.not_found.is_shown());
}
fn frame_with(
    ctx: &egui::Context,
    v: &mut TextCompareView,
    prefs: &mut Prefs,
    events: Vec<egui::Event>,
) {
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
}
#[test]
fn next_difference_prefs_wrap_around_and_silence_the_panel() {
    let ctx = egui::Context::default();
    let mut v = view("a\nb\nc\nd\ne\n", "a\nB\nc\nD\ne\n");
    let mut prefs = Prefs::default();
    prefs.next_diff.wrap_around = true;
    frame_with(&ctx, &mut v, &mut prefs, vec![]);
    // 向下回绕到第一个差异，不弹面板。
    v.goto_row(3);
    frame_with(&ctx, &mut v, &mut prefs, vec![key(egui::Key::N, true)]);
    assert_eq!(v.current_row, 1);
    assert!(!v.not_found.is_shown());
    // 向上回绕到最后一个差异。
    frame_with(&ctx, &mut v, &mut prefs, vec![key(egui::Key::P, true)]);
    assert_eq!(v.current_row, 3);
    // First / Last Difference 不受回绕影响。
    v.command("Last difference", &ctx, &prefs);
    assert_eq!(v.current_row, 3);
    v.command("First difference", &ctx, &prefs);
    assert_eq!(v.current_row, 1);
    // 关闭面板：找不到时光标不动、没有面板。
    prefs.next_diff = bc_session::prefs::NextDiffPrefs {
        message_panel: false,
        wrap_around: false,
    };
    frame_with(&ctx, &mut v, &mut prefs, vec![key(egui::Key::P, true)]);
    assert_eq!(v.current_row, 1);
    assert!(!v.not_found.is_shown());
    // 两侧相同：开启回绕也没有目标，仍弹面板。
    let mut same = view("a\nb\n", "a\nb\n");
    let mut prefs = Prefs::default();
    prefs.next_diff.wrap_around = true;
    frame_with(&ctx, &mut same, &mut prefs, vec![]);
    frame_with(&ctx, &mut same, &mut prefs, vec![key(egui::Key::N, true)]);
    assert!(same.not_found.is_shown());
}

// ---------------------------------------------------------------------------
// Word Wrap (docs/features/text-word-wrap.md, openspec change text-word-wrap)
// ---------------------------------------------------------------------------

/// `n` four-char words: "w00 w01 …" (unique, so a wrapped segment locates itself).
fn words(n: usize) -> String {
    (0..n).map(|i| format!("w{i:02} ")).collect::<String>().trim_end().to_string()
}
/// Monospace advance of the editor font (after the first frame loaded the fonts).
fn char_width(ctx: &egui::Context, v: &TextCompareView) -> f32 {
    ctx.fonts_mut(|f| f.glyph_width(&egui::FontId::monospace(v.font_size), 'w'))
}
/// Left pane sized for `left` chars of text; the right pane takes the rest.
fn split_for_chars(ctx: &egui::Context, v: &mut TextCompareView, left: f32) {
    let total = v.pane_rects[1].right() - v.pane_rects[0].left();
    let cw = char_width(ctx, v);
    let pane = left * cw + 18.0 + crate::text::gutter::WIDTH + 1.0 + 4.0 + 0.3 * cw;
    TEXT_SPLIT_X.store(pane, total);
    frame(ctx, v, vec![]);
    frame(ctx, v, vec![]);
}
/// Text shapes of one pane containing `needle`, top to bottom.
fn pane_texts(
    out: &egui::FullOutput,
    v: &TextCompareView,
    s: usize,
    needle: &str,
) -> Vec<(String, egui::Rect)> {
    let pane = v.pane_rects[s];
    let mut found: Vec<_> = text_shapes(out)
        .into_iter()
        .filter(|(t, r)| t.contains(needle) && pane.contains(r.left_top()))
        .collect();
    found.sort_by(|a, b| a.1.top().total_cmp(&b.1.top()));
    found
}
fn wrapped_view(left: &str, right: &str) -> (egui::Context, TextCompareView) {
    TEXT_SPLIT_X.reset();
    let ctx = egui::Context::default();
    let mut v = view(left, right);
    frame(&ctx, &mut v, vec![]);
    v.command("Word Wrap", &ctx, &Prefs::default());
    frame(&ctx, &mut v, vec![]);
    (ctx, v)
}

#[test]
fn word_wrap_menu_toggles_per_tab_and_defaults_off() {
    let ctx = egui::Context::default();
    let mut a = view("a\n", "b\n");
    let b = view("a\n", "b\n");
    assert_eq!(a.checked("Word Wrap"), Some(false));
    a.command("Word Wrap", &ctx, &Prefs::default());
    assert_eq!(a.checked("Word Wrap"), Some(true));
    // The other tab keeps its own (default) state.
    assert_eq!(b.checked("Word Wrap"), Some(false));
    a.update_menus();
    let item = a
        .menus
        .iter()
        .flat_map(|m| m.items.iter())
        .find(|i| i.action_id == "Word Wrap")
        .expect("View › Word Wrap");
    assert!(item.enabled);
    assert!(matches!(
        item.kind,
        crate::chrome::menubar::MenuItemKind::Check { checked: true }
    ));
    a.command("Word Wrap", &ctx, &Prefs::default());
    assert_eq!(a.checked("Word Wrap"), Some(false));
}

#[test]
fn word_wrap_hides_horizontal_scrolling_and_keeps_text_inside_panes() {
    TEXT_SPLIT_X.reset();
    let long = format!("{}\nshort\n", words(200));
    let ctx = egui::Context::default();
    let mut v = view(&long, &long);
    frame(&ctx, &mut v, vec![]);
    // Without wrap the long line scrolls horizontally.
    let shift = egui::Modifiers {
        shift: true,
        ..Default::default()
    };
    let over = v.pane_rects[0].center();
    frame(
        &ctx,
        &mut v,
        vec![egui::Event::PointerMoved(over), wheel_lines(-5.0, shift)],
    );
    frame(&ctx, &mut v, vec![]);
    assert!(v.scroll.x > 0.0, "{:?}", v.scroll);
    v.command("Word Wrap", &ctx, &Prefs::default());
    let out = frame(&ctx, &mut v, vec![]);
    assert_eq!(v.scroll.x, 0.0);
    // Horizontal wheel no longer pans.
    frame(
        &ctx,
        &mut v,
        vec![egui::Event::PointerMoved(over), wheel_lines(-5.0, shift)],
    );
    let out2 = frame(&ctx, &mut v, vec![]);
    assert_eq!(v.scroll.x, 0.0);
    for out in [&out, &out2] {
        for s in 0..2 {
            let right = v.pane_rects[s].right() - 18.0;
            let pieces = pane_texts(out, &v, s, "w");
            assert!(pieces.len() > 1, "{pieces:?}");
            for (t, r) in pieces {
                assert!(r.right() <= right + 0.5, "{t:?} {r:?} > {right}");
            }
        }
    }
    // Turning it off restores the single long line.
    v.command("Word Wrap", &ctx, &Prefs::default());
    let out = frame(&ctx, &mut v, vec![]);
    assert_eq!(pane_texts(&out, &v, 0, "w00").len(), 1);
    assert!(v.projection.heights().is_none());
}

#[test]
fn word_wrap_folds_each_pane_at_its_own_width_and_rows_take_the_taller_side() {
    let line = words(38);
    let text = format!("{line}\nnext\n");
    let (ctx, mut v) = wrapped_view(&text, &text);
    split_for_chars(&ctx, &mut v, 45.0);
    let out = frame(&ctx, &mut v, vec![]);
    let left = pane_texts(&out, &v, 0, "w");
    let right = pane_texts(&out, &v, 1, "w");
    assert!(left.len() > right.len() && right.len() >= 2, "{left:?} {right:?}");
    // Every visual line is one row below the previous; text is not lost.
    let rh = v.row_height();
    for side in [&left, &right] {
        for pair in side.windows(2) {
            assert!((pair[1].1.top() - pair[0].1.top() - rh).abs() < 0.5);
        }
        let joined: String = side.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(joined, line);
    }
    // The next aligned row starts at the same height in both panes, below the taller side.
    let next = [
        pane_texts(&out, &v, 0, "next")[0].1.top(),
        pane_texts(&out, &v, 1, "next")[0].1.top(),
    ];
    assert!((next[0] - next[1]).abs() < 0.5, "{next:?}");
    assert!((next[0] - left[0].1.top() - left.len() as f32 * rh).abs() < 0.5);
    let g = crate::text::projection::RowGeometry::for_row_height(rh);
    assert_eq!(v.projection.row_height(0, g), left.len() as f32 * rh);
    // Line number 1 is drawn once per pane (first visual line only).
    for s in 0..2 {
        let ones: Vec<_> = text_shapes(&out)
            .into_iter()
            .filter(|(t, r)| t == "1" && v.pane_rects[s].x_range().contains(r.left()))
            .collect();
        assert_eq!(ones.len(), 1, "{ones:?}");
        assert!(ones[0].1.center().y < left[0].1.top() + rh);
    }
    TEXT_SPLIT_X.reset();
}

#[test]
fn word_wrap_left_only_rows_fill_the_missing_side_and_endings_follow_the_last_line() {
    let line = words(38);
    let left = format!("same\r\n{line}\r\nend\r\n");
    let right = "same\r\nend\r\n";
    let (ctx, mut v) = wrapped_view(&left, right);
    v.command("Visible Whitespace", &ctx, &Prefs::default());
    split_for_chars(&ctx, &mut v, 45.0);
    let out = frame(&ctx, &mut v, vec![]);
    let rh = v.row_height();
    let pieces = pane_texts(&out, &v, 0, "w");
    assert!(pieces.len() >= 3, "{pieces:?}");
    // The left-only row is as tall as its left side; "end" lines up in both panes.
    let g = crate::text::projection::RowGeometry::for_row_height(rh);
    assert_eq!(v.projection.row_height(1, g), pieces.len() as f32 * rh);
    let ends = [
        pane_texts(&out, &v, 0, "end")[0].1.top(),
        pane_texts(&out, &v, 1, "end")[0].1.top(),
    ];
    assert!((ends[0] - ends[1]).abs() < 0.5, "{ends:?}");
    // CRLF marker of the wrapped row sits on its last visual line.
    let last = pieces.last().unwrap().1;
    let crlf: Vec<_> = pane_texts(&out, &v, 0, "CRLF")
        .into_iter()
        .filter(|(_, r)| (r.top() - last.top()).abs() < 0.5)
        .collect();
    assert_eq!(crlf.len(), 1, "{crlf:?} last={last:?}");
    assert!(crlf[0].1.left() >= last.right() - 0.5);
    TEXT_SPLIT_X.reset();
}

#[test]
fn word_wrap_clicks_and_drags_land_on_wrapped_visual_lines() {
    let line = words(38);
    let text = format!("{line}\nnext\n");
    let (ctx, mut v) = wrapped_view(&text, &text);
    split_for_chars(&ctx, &mut v, 45.0);
    let out = frame(&ctx, &mut v, vec![]);
    let rh = v.row_height();
    let left = pane_texts(&out, &v, 0, "w");
    let right = pane_texts(&out, &v, 1, "w");
    assert!(left.len() >= 3 && right.len() < left.len());
    // Click the first char of the second visual line.
    let (second, r) = &left[1];
    click(&ctx, &mut v, r.left_top() + egui::vec2(1.0, rh / 2.0));
    assert_eq!(v.active, 0);
    assert_eq!(v.editors[0].caret.line, 0);
    assert_eq!(v.editors[0].caret.byte, line.find(second.as_str()).unwrap());
    // Below the right side's visual lines (still inside the tall row): end of line.
    let blank = egui::pos2(
        right[0].1.left() + 20.0,
        left[0].1.top() + (left.len() as f32 - 0.5) * rh,
    );
    click(&ctx, &mut v, blank);
    assert_eq!(v.active, 1);
    assert_eq!(v.editors[1].caret.line, 0);
    assert_eq!(v.editors[1].caret.byte, line.len());
    // Drag from the middle of the first to the middle of the third visual line.
    let from = left[0].1.center();
    let to = left[2].1.center();
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(from)]);
    frame(&ctx, &mut v, vec![pointer(from, true)]);
    frame(&ctx, &mut v, vec![egui::Event::PointerMoved(to)]);
    frame(&ctx, &mut v, vec![pointer(to, false)]);
    let selected = v.editors[0].selected_text(&v.map.left);
    let (a, b) = v.editors[0].selection().unwrap();
    assert_eq!(selected, &line[a.byte..b.byte]);
    assert!(a.byte < line.find(left[1].0.as_str()).unwrap());
    assert!(b.byte > line.find(left[2].0.as_str()).unwrap());
    // The selection is painted on all three visual lines.
    let out = frame(&ctx, &mut v, vec![]);
    let selection = crate::chrome::theme::palette(&ctx).text_selection;
    let painted = out
        .shapes
        .iter()
        .filter(|s| match &s.shape {
            egui::Shape::Rect(r) => {
                r.fill == selection && v.pane_rects[0].x_range().contains(r.rect.left())
            }
            _ => false,
        })
        .count();
    assert_eq!(painted, 3);
    TEXT_SPLIT_X.reset();
}

/// The pane text shape containing `needle` lies fully inside the pane's text viewport.
fn shown_in_pane(out: &egui::FullOutput, v: &TextCompareView, s: usize, needle: &str) -> bool {
    // `pane_rects`: the editor's scroll area (below the header, above the status bar).
    let viewport = v.pane_rects[s];
    text_shapes(out)
        .into_iter()
        .any(|(t, r)| t.contains(needle) && viewport.contains(r.center()))
}

#[test]
fn word_wrap_keeps_the_top_row_when_widths_change_or_wrap_toggles() {
    let text = (0..1000)
        .map(|i| {
            if i % 10 == 0 {
                format!("long {i} {}\n", words(60))
            } else {
                format!("line {i}\n")
            }
        })
        .collect::<String>();
    let (ctx, mut v) = wrapped_view(&text, &text);
    let rh = v.row_height();
    v.scroll_to_row(500);
    frame(&ctx, &mut v, vec![]);
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.top_row(rh), 500);
    let g = crate::text::projection::RowGeometry::for_row_height(rh);
    let before = v.projection.y_of(500, g);
    split_for_chars(&ctx, &mut v, 45.0);
    // Narrower left pane: the long rows above got taller, the top row stays.
    assert!(v.projection.y_of(500, g) > before);
    assert_eq!(v.top_row(rh), 500);
    TEXT_SPLIT_X.reset();
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.top_row(rh), 500);
    v.command("Over-Under Layout", &ctx, &Prefs::default());
    frame(&ctx, &mut v, vec![]);
    assert_eq!(v.top_row(rh), 500);
    v.command("Word Wrap", &ctx, &Prefs::default());
    frame(&ctx, &mut v, vec![]);
    assert!(v.projection.heights().is_none());
    assert_eq!(v.top_row(rh), 500);
    v.command("Word Wrap", &ctx, &Prefs::default());
    frame(&ctx, &mut v, vec![]);
    assert!(v.projection.heights().is_some());
    assert_eq!(v.top_row(rh), 500);
}

#[test]
fn word_wrap_arrows_move_by_logical_line_and_the_caret_stays_visible() {
    // ~2000 chars: far more visual lines than fit in the pane.
    let line = format!("{} zzz", words(1500));
    let text = format!("{line}\nnext\n");
    let (ctx, mut v) = wrapped_view(&text, &text);
    let out = frame(&ctx, &mut v, vec![]);
    let first = pane_texts(&out, &v, 0, "w00")[0].1;
    click(&ctx, &mut v, first.left_top() + egui::vec2(1.0, 2.0));
    // The editor keeps its arrow keys after the click (focus lock survives the refocus).
    assert_eq!(ctx.memory(|m| m.focused()), Some(editor_id(0)));
    assert_eq!(v.editors[0].caret, Caret { line: 0, byte: 0 });
    // ↓ goes to the next logical line, not the next visual line.
    frame(&ctx, &mut v, vec![key(egui::Key::ArrowDown, false)]);
    assert_eq!(v.editors[0].caret.line, 1);
    assert_eq!(ctx.memory(|m| m.focused()), Some(editor_id(0)));
    frame(&ctx, &mut v, vec![key(egui::Key::ArrowUp, false)]);
    assert_eq!(v.editors[0].caret.line, 0);
    // Caret deep inside the long line (below the viewport); → reveals its visual line.
    let target = line.find("w1400").unwrap();
    v.editors[0].place(Caret { line: 0, byte: target }, false);
    let out = frame(&ctx, &mut v, vec![]);
    assert!(!shown_in_pane(&out, &v, 0, "w1401"));
    frame(&ctx, &mut v, vec![key(egui::Key::ArrowRight, false)]);
    let out = frame(&ctx, &mut v, vec![]);
    assert_eq!(v.editors[0].caret.byte, target + 1);
    assert!(v.scroll.y > 0.0);
    assert!(shown_in_pane(&out, &v, 0, "w1400"), "scroll {:?}", v.scroll);
    // Find Next on a hit at the end of the long line shows that visual line.
    v.scroll.y = 0.0;
    frame(&ctx, &mut v, vec![]);
    v.find.options.pattern = "zzz".into();
    v.find.last_hit = None;
    v.editors[0].place(Caret { line: 0, byte: 0 }, false);
    v.find_action(FindAction::Next).unwrap();
    frame(&ctx, &mut v, vec![]);
    let out = frame(&ctx, &mut v, vec![]);
    let hit = v.find.last_hit.as_ref().unwrap();
    assert_eq!((hit.line, hit.range.start), (0, line.find("zzz").unwrap()));
    assert!(shown_in_pane(&out, &v, 0, "zzz"), "scroll {:?}", v.scroll);
}

#[test]
fn word_wrap_typing_rewraps_the_row_in_the_same_frame() {
    let text = "ab\nnext\n";
    let (ctx, mut v) = wrapped_view(text, text);
    split_for_chars(&ctx, &mut v, 20.0);
    let rh = v.row_height();
    let g = crate::text::projection::RowGeometry::for_row_height(rh);
    let out = frame(&ctx, &mut v, vec![]);
    let ab = pane_texts(&out, &v, 0, "ab")[0].1;
    click(&ctx, &mut v, ab.right_center() + egui::vec2(2.0, 0.0));
    assert_eq!(v.editors[0].caret, Caret { line: 0, byte: 2 });
    assert_eq!(v.projection.row_height(0, g), rh);
    let typed = " ".to_string() + &words(8);
    let out = frame(&ctx, &mut v, vec![egui::Event::Text(typed)]);
    assert_eq!(v.projection.row_height(0, g), 2.0 * rh);
    // Same frame: "next" already moved down one visual line in both panes.
    let next = [
        pane_texts(&out, &v, 0, "next")[0].1.top(),
        pane_texts(&out, &v, 1, "next")[0].1.top(),
    ];
    assert!((next[0] - next[1]).abs() < 0.5, "{next:?}");
    assert!((next[0] - ab.top() - 2.0 * rh).abs() < 0.5, "{next:?} {ab:?}");
    TEXT_SPLIT_X.reset();
}

#[test]
#[ignore]
/// Word Wrap on ~100 MB / 1M lines per side (every 20th line ~600 chars, some CJK lines).
/// `cargo test --release -p bc-gui --lib word_wrap_large_file_latency -- --ignored --nocapture`
fn word_wrap_large_file_latency() {
    let dir = Temp::new();
    let mut left = String::with_capacity(110 << 20);
    for i in 0..1_000_000usize {
        if i % 20 == 0 {
            left.push_str(&format!("long {i} {}
", words(150)));
        } else if i % 50 == 7 {
            left.push_str(&format!(
                "中文行 {i} 这是一段用于测试自动折行的中文文本，包含全角标点。重复：这是一段用于测试自动折行的中文文本。
"
            ));
        } else {
            left.push_str(&format!(
                "    let value_{i} = compute(alpha, beta, gamma); // ordinary code line
"
            ));
        }
    }
    let mut right = left.clone();
    right.replace_range(1000..1010, "XXXXXXXXXX");
    let l = dir.file("l.txt", left.as_bytes());
    let r = dir.file("r.txt", right.as_bytes());
    let ctx = egui::Context::default();
    let mut v = TextCompareView::open(&l, &r).unwrap();
    while !v.is_ready() {
        v.poll();
        std::thread::sleep(Duration::from_millis(1));
    }
    eprintln!("[wrap] {} MB per side, {} rows", left.len() >> 20, v.row_count());
    TEXT_SPLIT_X.reset();
    frame(&ctx, &mut v, vec![]);
    frame(&ctx, &mut v, vec![]);
    let timed = |label: &str, v: &mut TextCompareView, events: Vec<egui::Event>| {
        let t = Instant::now();
        frame(&ctx, v, events);
        let ms = t.elapsed().as_secs_f64() * 1e3;
        eprintln!("[wrap] {label:<26} {ms:>8.1} ms");
        ms
    };
    v.editors[0].place(Caret { line: 5, byte: 0 }, false);
    ctx.memory_mut(|m| m.request_focus(editor_id(0)));
    frame(&ctx, &mut v, vec![]);
    timed("type a char (wrap off)", &mut v, vec![egui::Event::Text("x".into())]);
    v.command("Word Wrap", &ctx, &Prefs::default());
    let mut worst: f64 = timed("wrap on", &mut v, vec![]);
    let total = v.pane_rects[1].right() - v.pane_rects[0].left();
    TEXT_SPLIT_X.store(total * 0.3, total);
    worst = worst.max(timed("split change", &mut v, vec![]));
    v.command("Over-Under Layout", &ctx, &Prefs::default());
    worst = worst.max(timed("stacked layout", &mut v, vec![]));
    v.command("Side-by-Side Layout", &ctx, &Prefs::default());
    worst = worst.max(timed("side-by-side layout", &mut v, vec![]));
    let last = v.projection.len() - 1;
    v.scroll_to_row(last);
    worst = worst.max(timed("jump to end", &mut v, vec![]));
    frame(&ctx, &mut v, vec![]);
    assert!(v.top_row(v.row_height()) > last - 60);
    v.editors[0].place(Caret { line: 5, byte: 0 }, false);
    v.scroll_to_row(0);
    frame(&ctx, &mut v, vec![]);
    timed("type a char (wrap on)", &mut v, vec![egui::Event::Text("x".into())]);
    let mut idle = 0;
    while v.projection.heights().is_some_and(|h| !h.is_complete()) {
        frame(&ctx, &mut v, vec![]);
        idle += 1;
        assert!(idle < 1000);
    }
    eprintln!("[wrap] idle frames until exact: {idle}");
    assert!(worst < 1000.0, "{worst} ms");
}

#[test]
#[ignore]
/// ~50 MB single-line JSON: capped at 10000 visual lines with the hint at the end.
/// `MATCHA_WRAP_OFF=1` keeps wrap off (for the peak-memory comparison).
fn word_wrap_single_line_json() {
    let dir = Temp::new();
    let json = format!(
        "{{\"data\":[{}]}}",
        (0..5_000_000).map(|i| format!("{i:>9},")).collect::<String>()
    );
    let j = dir.file("j.json", json.as_bytes());
    let ctx = egui::Context::default();
    let mut v = TextCompareView::open(&j, &j).unwrap();
    while !v.is_ready() {
        v.poll();
        std::thread::sleep(Duration::from_millis(1));
    }
    frame(&ctx, &mut v, vec![]);
    if std::env::var_os("MATCHA_WRAP_OFF").is_some() {
        for _ in 0..20 {
            frame(&ctx, &mut v, vec![]);
        }
        return;
    }
    v.command("Word Wrap", &ctx, &Prefs::default());
    let t = Instant::now();
    frame(&ctx, &mut v, vec![]);
    eprintln!(
        "[wrap] json {} MB: wrap on {:.1} ms, {} visual lines",
        json.len() >> 20,
        t.elapsed().as_secs_f64() * 1e3,
        v.projection.units(0)
    );
    assert_eq!(v.projection.units(0), crate::wrap::WRAP_CAP);
    v.scroll.y = 1e9;
    let t = Instant::now();
    let out = frame(&ctx, &mut v, vec![]);
    eprintln!("[wrap] json scroll to bottom {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    assert!(v.scroll.y > 100_000.0);
    assert!(text_shapes(&out)
        .iter()
        .any(|(t, _)| t.contains("more characters not shown")));
    for _ in 0..18 {
        frame(&ctx, &mut v, vec![]);
    }
}

/// Real Windows fonts (editor family + CJK fallback, as the app installs them): wrapped
/// mixed Chinese / ASCII text stays inside each pane at several widths.
#[test]
fn word_wrap_with_real_fonts_keeps_cjk_inside_the_panes() {
    let ctx = egui::Context::default();
    crate::chrome::fonts::install(&ctx, &Prefs::default().editor_font.family);
    let line = "中文行：这是一段用于测试自动折行的中文文本，包含全角标点（括号）与 ASCII words mixed in, 以及更多的汉字内容。"
        .repeat(4);
    let text = format!("{line}\r\nnext\r\n");
    TEXT_SPLIT_X.reset();
    let mut v = view(&text, &text);
    frame(&ctx, &mut v, vec![]);
    v.command("Word Wrap", &ctx, &Prefs::default());
    v.command("Visible Whitespace", &ctx, &Prefs::default());
    frame(&ctx, &mut v, vec![]);
    for chars in [30.0, 45.0, 70.0, 110.0] {
        split_for_chars(&ctx, &mut v, chars);
        let out = frame(&ctx, &mut v, vec![]);
        for s in 0..2 {
            let pane = v.pane_rects[s];
            let right = pane.right() - 18.0;
            let pieces: Vec<_> = text_shapes(&out)
                .into_iter()
                .filter(|(t, r)| {
                    pane.contains(r.left_top())
                        && r.left() >= pane.left() + crate::text::gutter::WIDTH
                        && !t.contains("CRLF")
                        && !t.starts_with("next")
                })
                .collect();
            assert!(pieces.len() > 1, "{chars}: {pieces:?}");
            for (t, r) in &pieces {
                assert!(r.right() <= right + 0.5, "{chars} side {s}: {t:?} {r:?} > {right}");
            }
            let joined: String = {
                let mut p = pieces.clone();
                p.sort_by(|a, b| a.1.top().total_cmp(&b.1.top()));
                p.into_iter().map(|(t, _)| t).collect()
            };
            // Visible Whitespace draws spaces as middle dots.
            assert_eq!(joined, line.replace(' ', "·"), "{chars} side {s}");
        }
    }
    TEXT_SPLIT_X.reset();
}
