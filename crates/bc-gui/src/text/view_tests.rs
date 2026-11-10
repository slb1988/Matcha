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
    assert_eq!(v.projection.len(), 2);
    v.filter = DisplayFilter::Same;
    v.rebuild_projection();
    assert_eq!(v.projection.len(), 4); // three Same + editable EOF
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
    v.map.rules.alignment.use_closeness = false;
    v.recompare();
    ready(&mut v);
    assert_eq!(v.map.row_count(), 2);
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
    v.request_home();
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
    assert!(
        !v.menus[0]
            .items
            .iter()
            .find(|i| i.label == "New Tab")
            .unwrap()
            .enabled
    );
}
