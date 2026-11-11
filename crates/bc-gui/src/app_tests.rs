use super::*;
use bc_engine_text::TextContent;
use bc_formats::registry::FormatRegistry;
use egui::{Pos2, Rect, Vec2};

thread_local! { static TICK: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) }; }
/// 参考窗口客户区 1384x961；标签条出现后 Home 内容整体下移 26 px。
const STRIP: f32 = tabstrip::HEIGHT;
const HOME_TAB: Pos2 = Pos2::new(60.0, 14.0);

fn run(ctx: &egui::Context, app: &mut MatchaApp, events: Vec<egui::Event>) -> egui::FullOutput {
    run_input(
        ctx,
        app,
        egui::RawInput {
            events,
            ..Default::default()
        },
    )
}
fn run_input(
    ctx: &egui::Context,
    app: &mut MatchaApp,
    mut input: egui::RawInput,
) -> egui::FullOutput {
    input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1384.0, 961.0)));
    // Advancing time lets egui pair presses into double clicks.
    input.time = Some(TICK.with(|t| {
        t.set(t.get() + 0.05);
        t.get()
    }));
    let mut out = ctx.run_ui(input, |ui| app.frame(ui, None));
    out.textures_delta.clear();
    out
}
fn press(pos: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}
fn click(ctx: &egui::Context, app: &mut MatchaApp, pos: Pos2) {
    run(ctx, app, vec![egui::Event::PointerMoved(pos)]);
    run(ctx, app, vec![press(pos, true)]);
    run(ctx, app, vec![press(pos, false)]);
    // Let the click age out so the next one is never paired into a double click.
    for _ in 0..8 {
        run(ctx, app, vec![]);
    }
}
fn ctrl(ctx: &egui::Context, app: &mut MatchaApp, key: egui::Key) {
    run(
        ctx,
        app,
        vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }],
    );
}
fn text_at(output: &egui::FullOutput, text: &str) -> Pos2 {
    output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::Shape::Text(t) if t.galley.text() == text => {
                Some(Rect::from_min_size(t.pos, t.galley.size()).center())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("no {text:?} drawn"))
}
fn home_app() -> MatchaApp {
    MatchaApp::with_view(View::Home(HomeView::default()))
}
fn dirty_text() -> View {
    let content = || TextContent::from_bytes(b"same\n".to_vec(), None).unwrap();
    let mut v =
        TextCompareView::from_contents(content(), content(), FormatRegistry::with_builtins());
    v.map.right.replace_line(0, b"edited").unwrap();
    assert!(v.is_dirty());
    View::TextCompare(Box::new(v))
}
fn empty_paths(view: &View) -> bool {
    view.side_names() == [None, None]
}

#[test]
fn home_icons_open_empty_session_tabs_without_a_dialog() {
    let ctx = egui::Context::default();
    let mut app = home_app();
    run(&ctx, &mut app, vec![]);
    // 单个 Home 页：无标签条，图标网格在参考坐标（y403）。
    click(&ctx, &mut app, Pos2::new(992.0, 403.0));
    assert_eq!((app.tab_count(), app.active_tab()), (2, 1));
    assert!(matches!(app.active_view(), View::TextCompare(_)));
    assert!(empty_paths(app.active_view()));
    assert_eq!(app.active_view().caption(), "Text Compare");
    // 其余类型：回到 Home 页（标签条已出现，内容下移）再点图标。
    let y = 403.0 + STRIP;
    for (x, page, kind) in [
        (834.0, false, "Folder Sync"),
        (676.0, false, "Folder Compare"),
        (676.0, true, "Table Compare"),
    ] {
        click(&ctx, &mut app, HOME_TAB);
        assert_eq!(app.active_tab(), 0);
        assert!(matches!(app.active_view(), View::Home(_)));
        if page {
            click(&ctx, &mut app, Pos2::new(1295.0, y));
        }
        let before = app.tab_count();
        click(&ctx, &mut app, Pos2::new(x, y));
        assert_eq!(app.tab_count(), before + 1);
        assert_eq!(app.active_tab(), before);
        assert_eq!(app.active_view().kind(), kind);
        assert!(empty_paths(app.active_view()), "{kind} opened with paths");
    }
    // 空会话的窗口标题只有类型名。
    assert_eq!(app.active_view().window_title(), "Table Compare - Matcha");
}

#[test]
fn an_empty_table_tab_shows_blank_panes_not_a_load_failure() {
    let ctx = egui::Context::default();
    let mut app = MatchaApp::with_view(View::TableCompare(Box::new(TableCompareView::empty())));
    let out = run(&ctx, &mut app, vec![]);
    let texts: Vec<String> = out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect();
    assert!(
        !texts.iter().any(|t| t.starts_with("Load failed")),
        "{texts:?}"
    );
    assert!(!texts.iter().any(|t| t == "Retry load"));
}
#[test]
fn tabs_activate_close_and_follow_ctrl_t_and_ctrl_w() {
    let ctx = egui::Context::default();
    let mut app = home_app();
    app.open_tab(View::TableCompare(Box::new(TableCompareView::empty())));
    app.open_tab(View::TableCompare(Box::new(TableCompareView::empty())));
    run(&ctx, &mut app, vec![]);
    assert_eq!((app.tab_count(), app.active_tab()), (3, 2));
    // Ctrl+W 关闭活动页（无未保存编辑，直接关闭），激活左邻。
    ctrl(&ctx, &mut app, egui::Key::W);
    assert_eq!((app.tab_count(), app.active_tab()), (2, 1));
    // Ctrl+T 新开 Home 页并激活。
    ctrl(&ctx, &mut app, egui::Key::T);
    assert_eq!((app.tab_count(), app.active_tab()), (3, 2));
    assert!(matches!(app.active_view(), View::Home(_)));
    // 标签条：点击激活第一页，"x" 关闭第二页。
    run(&ctx, &mut app, vec![]);
    click(&ctx, &mut app, HOME_TAB);
    assert_eq!(app.active_tab(), 0);
    let close = Pos2::new(2.0 + 2.0 * tabstrip_width(3) - 1.0 - 11.0, 14.5);
    click(&ctx, &mut app, close);
    assert_eq!((app.tab_count(), app.active_tab()), (2, 0));
    assert!(matches!(app.tabs[1].view, View::Home(_)));
    // "+" 新开 Home 页。
    let plus = Pos2::new(2.0 + 2.0 * tabstrip_width(2) + 4.0 + 11.0, 14.0);
    click(&ctx, &mut app, plus);
    assert_eq!((app.tab_count(), app.active_tab()), (3, 2));
    // 关闭最后一页即退出。
    let mut single = home_app();
    ctrl(&ctx, &mut single, egui::Key::W);
    assert!(single.exit_confirmed);
}
/// 1384 宽窗口下每个标签页的宽度（与 tabstrip::layout 相同的取整与夹取）。
fn tabstrip_width(count: usize) -> f32 {
    ((1384.0 - 22.0 - 8.0) / count as f32)
        .clamp(90.0, 220.0)
        .floor()
}

#[test]
fn closing_a_dirty_text_tab_prompts_to_save_first() {
    let ctx = egui::Context::default();
    let mut app = home_app();
    app.open_tab(dirty_text());
    run(&ctx, &mut app, vec![]);
    ctrl(&ctx, &mut app, egui::Key::W);
    // 未保存编辑：标签页保留并弹出保存确认。
    assert_eq!(app.tab_count(), 2);
    assert!(app.active_view().prompt_open());
    run(&ctx, &mut app, vec![]);
    let out = run(&ctx, &mut app, vec![]);
    click(&ctx, &mut app, text_at(&out, "Cancel"));
    assert_eq!(app.tab_count(), 2);
    assert!(!app.active_view().prompt_open());
    // 再次关闭并放弃修改：标签页移除，回到 Home。
    ctrl(&ctx, &mut app, egui::Key::W);
    run(&ctx, &mut app, vec![]);
    let out = run(&ctx, &mut app, vec![]);
    click(&ctx, &mut app, text_at(&out, "Discard"));
    assert_eq!(app.tab_count(), 1);
    assert!(matches!(app.active_view(), View::Home(_)));
    assert!(!app.exit_confirmed);
}

#[test]
fn window_close_walks_every_dirty_tab_through_its_save_prompt() {
    let ctx = egui::Context::default();
    let mut app = MatchaApp::with_view(dirty_text());
    app.open_tab(View::Home(HomeView::default()));
    app.open_tab(dirty_text());
    run(&ctx, &mut app, vec![]);
    let close = || {
        let mut input = egui::RawInput::default();
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .events
            .push(egui::ViewportEvent::Close);
        input
    };
    run_input(&ctx, &mut app, close());
    // 第一个脏页被激活并弹出确认。
    assert!(!app.exit_confirmed);
    assert_eq!(app.active_tab(), 0);
    assert!(app.active_view().prompt_open());
    run(&ctx, &mut app, vec![]);
    let out = run(&ctx, &mut app, vec![]);
    click(&ctx, &mut app, text_at(&out, "Discard"));
    // 第二个脏页接着确认；取消即中止退出。
    assert_eq!(app.active_tab(), 2);
    assert!(app.active_view().prompt_open());
    let out = run(&ctx, &mut app, vec![]);
    click(&ctx, &mut app, text_at(&out, "Cancel"));
    assert!(!app.exiting && !app.exit_confirmed);
    assert_eq!(app.tab_count(), 3);
    // 再次关窗：两页都重新确认，全部放弃后退出。
    run_input(&ctx, &mut app, close());
    for expected in [0, 2] {
        assert_eq!(app.active_tab(), expected);
        run(&ctx, &mut app, vec![]);
        let out = run(&ctx, &mut app, vec![]);
        click(&ctx, &mut app, text_at(&out, "Discard"));
    }
    assert!(app.exit_confirmed);
}

#[test]
fn home_tree_expansion_survives_a_tab_round_trip() {
    let root = std::env::temp_dir().join(format!("matcha-app-tabs-{}", std::process::id()));
    let (left, right) = (root.join("left"), root.join("right"));
    std::fs::create_dir_all(&left).unwrap();
    std::fs::create_dir_all(&right).unwrap();
    {
        let ctx = egui::Context::default();
        let mut app = MatchaApp::with_view(View::Home(HomeView::with_recent(vec![(
            left.clone(),
            right.clone(),
        )])));
        run(&ctx, &mut app, vec![]);
        // 展开 Auto-saved（第二行展开钮），再双击其下的最近比较。
        click(&ctx, &mut app, Pos2::new(10.0, 90.0));
        let row = Pos2::new(100.0, 110.0);
        run(&ctx, &mut app, vec![egui::Event::PointerMoved(row)]);
        for pressed in [true, false, true, false] {
            run(&ctx, &mut app, vec![press(row, pressed)]);
        }
        assert_eq!((app.tab_count(), app.active_tab()), (2, 1));
        assert!(matches!(app.active_view(), View::FolderCompare(_)));
        assert_eq!(app.active_view().caption(), "left <--> right");
        // 切回 Home 页：同一个 HomeView，展开状态仍在。
        click(&ctx, &mut app, HOME_TAB);
        match app.active_view() {
            View::Home(home) => assert!(home.auto_open()),
            _ => panic!("Home tab not active"),
        }
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn empty_sessions_load_once_paths_are_entered() {
    let root = std::env::temp_dir().join(format!("matcha-app-empty-{}", std::process::id()));
    let (left, right) = (root.join("l"), root.join("r"));
    std::fs::create_dir_all(&left).unwrap();
    std::fs::create_dir_all(&right).unwrap();
    std::fs::write(left.join("a.txt"), "one\n").unwrap();
    std::fs::write(right.join("a.txt"), "two\n").unwrap();
    {
        // 文本：逐侧加载，两侧都到位后比较出差异。
        let mut text = TextCompareView::empty().unwrap();
        text.open_path(0, &left.join("a.txt")).unwrap();
        text.open_path(1, &right.join("a.txt")).unwrap();
        let start = Instant::now();
        while !text.is_ready() {
            text.poll();
            assert!(start.elapsed().as_secs() < 5);
        }
        assert_eq!(
            text.paths(),
            [Some(left.join("a.txt")), Some(right.join("a.txt"))]
        );
        assert!(text.row_count() > 1);
        // 文件夹：只填一侧时不加载，两侧都填上才扫描。
        let ctx = egui::Context::default();
        let mut folder = DirCompareView::empty(&ctx, false, false);
        folder.open_paths([left.display().to_string(), String::new()]);
        assert!(!folder.busy());
        assert_eq!(folder.path_texts()[0], left.display().to_string());
        folder.open_paths([left.display().to_string(), right.display().to_string()]);
        let start = Instant::now();
        loop {
            folder.poll(&ctx);
            if !folder.busy() && folder.snapshot.complete {
                break;
            }
            assert!(start.elapsed().as_secs() < 10, "{}", folder.row_count());
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(folder.row_count() > 0);
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn handed_off_comparisons_open_new_tabs_and_report_when_closed() {
    use crate::remote::{channel, Outcome, RemoteOpen};
    use std::sync::{Arc, Mutex};
    let root = std::env::temp_dir().join(format!("matcha-app-remote-{}", std::process::id()));
    // 只用文件夹对：成功打开的文本比较会写入用户真实的最近列表。
    let (dirs, swapped) = (
        [root.join("l"), root.join("r")],
        [root.join("r"), root.join("l")],
    );
    for d in &dirs {
        std::fs::create_dir_all(d).unwrap();
    }
    let outcomes: Arc<Mutex<Vec<Outcome>>> = Default::default();
    let done = |outcomes: &Arc<Mutex<Vec<Outcome>>>| {
        let outcomes = outcomes.clone();
        Some(Box::new(move |o| outcomes.lock().unwrap().push(o)) as crate::remote::Completion)
    };
    let open = |pair: &[std::path::PathBuf; 2], done| RemoteOpen {
        left: Some(pair[0].clone()),
        right: Some(pair[1].clone()),
        done,
    };

    let ctx = egui::Context::default();
    let mut app = home_app();
    let (tx, inbox) = channel();
    app.attach_remote(&ctx, inbox);
    run(&ctx, &mut app, vec![]);
    // 监听线程投递：每个请求开一个新标签页（与 Home 打开比较同一路由）。
    assert!(tx.send(open(&dirs, done(&outcomes))).is_ok());
    assert!(tx.send(open(&swapped, None)).is_ok());
    // 无路径的转交只提到前台，不开页；等待方立即得到 Closed。
    assert!(tx
        .send(RemoteOpen {
            left: None,
            right: None,
            done: done(&outcomes),
        })
        .is_ok());
    app.poll_remote(&ctx);
    assert_eq!(app.tab_count(), 3);
    assert!(matches!(app.tabs[1].view, View::FolderCompare(_)));
    assert!(matches!(app.tabs[2].view, View::FolderCompare(_)));
    assert_eq!(app.active_tab(), 2);
    assert_eq!(*outcomes.lock().unwrap(), vec![Outcome::Closed]);
    run(&ctx, &mut app, vec![]);

    // 关闭被等待的那一页才通知等待方。
    app.close_tab(&ctx, 2);
    assert_eq!(outcomes.lock().unwrap().len(), 1);
    app.close_tab(&ctx, 1);
    assert_eq!(
        *outcomes.lock().unwrap(),
        vec![Outcome::Closed, Outcome::Closed]
    );

    // 打不开的比较：回到 Home 显示错误，等待方得到 Failed。
    let missing = [root.join("missing-a.txt"), root.join("missing-b.txt")];
    assert!(tx.send(open(&missing, done(&outcomes))).is_ok());
    app.poll_remote(&ctx);
    assert_eq!(app.tab_count(), 1);
    assert!(matches!(app.active_view(), View::Home(_)));
    assert!(matches!(
        outcomes.lock().unwrap().last(),
        Some(Outcome::Failed(_))
    ));

    // 应用退出（标签页随应用一起丢弃）同样算会话结束。
    assert!(tx.send(open(&dirs, done(&outcomes))).is_ok());
    app.poll_remote(&ctx);
    assert_eq!(outcomes.lock().unwrap().len(), 3);
    drop(app);
    assert_eq!(outcomes.lock().unwrap().last(), Some(&Outcome::Closed));
    assert_eq!(outcomes.lock().unwrap().len(), 4);
    let _ = std::fs::remove_dir_all(&root);
}
