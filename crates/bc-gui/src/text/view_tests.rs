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
    assert!(v.message.contains("No more"));
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
    assert_eq!(v.pane_rects[0].top(), 123.0);
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
            2.0 + skipped as f32 * 21.0 + separators as f32 * 10.0 + 10.5,
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
