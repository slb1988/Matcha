use super::*;
use bc_engine_text::TextContent;
use bc_formats::registry::FormatRegistry;
use egui::{Pos2, Rect, Vec2};

thread_local! { static TICK: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) }; }
/// 参考窗口客户区 1384x961；标签条出现后 Home 内容整体下移 26 px。
const STRIP: f32 = tabstrip::HEIGHT;
const HOME_TAB: Pos2 = Pos2::new(60.0, 21.0);

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
    // 单个 Home 页：无标签条，五种会话卡片一行排开（y403，Text 居中）。
    click(&ctx, &mut app, Pos2::new(834.0, 403.0));
    assert_eq!((app.tab_count(), app.active_tab()), (2, 1));
    assert!(matches!(app.active_view(), View::TextCompare(_)));
    assert!(empty_paths(app.active_view()));
    assert_eq!(app.active_view().caption(), "Text Compare");
    // 其余类型：回到 Home 页（标签条已出现，内容下移）再点图标。
    let y = 403.0 + STRIP;
    for (x, kind) in [
        (676.0, "Folder Sync"),
        (518.0, "Folder Compare"),
        (992.0, "Table Compare"),
    ] {
        click(&ctx, &mut app, HOME_TAB);
        assert_eq!(app.active_tab(), 0);
        assert!(matches!(app.active_view(), View::Home(_)));
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
    let close = Pos2::new(6.0 + 2.0 * tabstrip_width(3) - 1.0 - 16.0, 21.0);
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
    ((1384.0 - 26.0 - 8.0) / count as f32)
        .clamp(110.0, 240.0)
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
    click(&ctx, &mut app, text_at(&out, "Don't Save"));
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
    click(&ctx, &mut app, text_at(&out, "Don't Save"));
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
        click(&ctx, &mut app, text_at(&out, "Don't Save"));
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
        // 折叠 New（首行展开钮）；Auto-saved 与 Today 默认展开，最近比较在第 4 行。
        click(&ctx, &mut app, Pos2::new(10.0, 71.0));
        let row = Pos2::new(100.0, 131.0);
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
            View::Home(home) => assert!(!home.new_open()),
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
        merge: None,
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
            merge: None,
            done: done(&outcomes),
        })
        .is_ok());
    app.poll_remote(&ctx);
    assert_eq!(app.tab_count(), 3);
    assert!(matches!(app.tabs[1].view, View::FolderCompare(_)));
    assert!(matches!(app.tabs[2].view, View::FolderCompare(_)));
    assert_eq!(app.active_tab(), 2);
    assert_eq!(*outcomes.lock().unwrap(), vec![Outcome::Closed(0)]);
    run(&ctx, &mut app, vec![]);

    // 关闭被等待的那一页才通知等待方。
    app.close_tab(&ctx, 2);
    assert_eq!(outcomes.lock().unwrap().len(), 1);
    app.close_tab(&ctx, 1);
    assert_eq!(
        *outcomes.lock().unwrap(),
        vec![Outcome::Closed(0), Outcome::Closed(0)]
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
    assert_eq!(outcomes.lock().unwrap().last(), Some(&Outcome::Closed(0)));
    assert_eq!(outcomes.lock().unwrap().len(), 4);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn fv_hex_compare_opens_hex_view() {
    let corpus = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/reference-parity/hex-compare/corpus/");
    let options = LaunchOptions {
        left: Some(PathBuf::from(format!("{corpus}l2.bin"))),
        right: Some(PathBuf::from(format!("{corpus}r2.bin"))),
        force_view: Some(bc_session::SessionKind::HexCompare),
        ..Default::default()
    };
    let view = MatchaApp::launch_view(&egui::Context::default(), &options);
    assert!(matches!(view, View::HexCompare(_)));
    // 强制 Text 覆盖格式路由（用文本文件，.bin 的 NUL 内容会被文本加载拒绝）。
    let dir = std::env::temp_dir().join(format!("matcha-fv-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (a, b) = (dir.join("a.txt"), dir.join("b.txt"));
    std::fs::write(&a, b"one\n").unwrap();
    std::fs::write(&b, b"two\n").unwrap();
    let options = LaunchOptions {
        left: Some(a.clone()),
        right: Some(b.clone()),
        force_view: Some(bc_session::SessionKind::TextCompare),
        ..Default::default()
    };
    let view = MatchaApp::launch_view(&egui::Context::default(), &options);
    assert!(matches!(view, View::TextCompare(_)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn command_line_workbooks_open_table_compare_not_hex() {
    // xlsx 是 ZIP（内容嗅探为二进制），命令行两个工作簿仍须打开表格比较。
    let corpus = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/reference-parity/table-report/corpus/key-none-rows-changed/"
    );
    let options = LaunchOptions {
        left: Some(PathBuf::from(format!("{corpus}left.xlsx"))),
        right: Some(PathBuf::from(format!("{corpus}right.xlsx"))),
        ..Default::default()
    };
    let view = MatchaApp::launch_view(&egui::Context::default(), &options);
    assert!(matches!(view, View::TableCompare(_)));
}

#[test]
fn bin_mask_routes_to_hex_compare_view() {
    let corpus = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/reference-parity/hex-compare/corpus/");
    let options = LaunchOptions {
        left: Some(PathBuf::from(format!("{corpus}l2.bin"))),
        right: Some(PathBuf::from(format!("{corpus}r2.bin"))),
        ..Default::default()
    };
    let view = MatchaApp::launch_view(&egui::Context::default(), &options);
    assert!(matches!(view, View::HexCompare(_)));
}

/// 三份合并输入（左右在同一行改得不同 → 一个冲突）与一个尚不存在的输出路径。
fn merge_files(tag: &str) -> (std::path::PathBuf, crate::merge::MergeLaunch) {
    let root = std::env::temp_dir().join(format!("matcha-app-merge-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let base = "one\ntwo\nthree\nfour\nfive\n";
    std::fs::write(root.join("base.txt"), base).unwrap();
    std::fs::write(root.join("theirs.txt"), base.replace("three", "THEIRS")).unwrap();
    std::fs::write(root.join("yours.txt"), base.replace("three", "YOURS")).unwrap();
    let launch = crate::merge::MergeLaunch::new(bc_script::cli::MergeSpec {
        left: root.join("theirs.txt"),
        right: root.join("yours.txt"),
        center: root.join("base.txt"),
        output: Some(root.join("result.txt")),
    });
    (root, launch)
}

#[test]
fn merge_launch_opens_a_merge_tab_and_reports_its_result_code() {
    let (root, launch) = merge_files("launch");
    let options = LaunchOptions {
        merge: Some(launch.clone()),
        ..Default::default()
    };
    let ctx = egui::Context::default();
    let view = MatchaApp::launch_view(&ctx, &options);
    assert!(matches!(view, View::TextMerge(_)));
    assert_eq!(view.kind(), "Text Merge");
    assert_eq!(view.caption(), "theirs.txt <--> yours.txt");
    let mut app = MatchaApp::with_view(view);
    let slot: crate::remote::ResultSlot = Default::default();
    app.watch_launch(slot.clone(), true);
    run(&ctx, &mut app, vec![]);
    run(&ctx, &mut app, vec![]);
    // 带输出目标的合并自打开起未保存：关页先问，不直接关。
    app.close_tab(&ctx, 0);
    assert_eq!(app.tab_count(), 1);
    assert!(app.active_view().prompt_open());
    assert_eq!(*slot.lock().unwrap(), None);
    // 保存后再关：结果码 14（保存时仍有未解决冲突）。
    let View::TextMerge(merge) = &mut app.tabs[0].view else {
        unreachable!()
    };
    let target = merge.model.target.clone().unwrap();
    merge.model.save(&target).unwrap();
    drop(app);
    assert_eq!(*slot.lock().unwrap(), Some(14));
    assert_eq!(
        std::fs::read_to_string(root.join("result.txt")).unwrap(),
        "one\ntwo\nthree\nfour\nfive\n"
    );

    // 启动时就打不开：回 Home 显示错误，结果码立即为 100。
    let mut missing = launch;
    missing.spec.center = root.join("missing.txt");
    let options = LaunchOptions {
        merge: Some(missing),
        ..Default::default()
    };
    let view = MatchaApp::launch_view(&ctx, &options);
    assert!(matches!(view, View::Home(_)));
    let mut app = MatchaApp::with_view(view);
    let slot: crate::remote::ResultSlot = Default::default();
    app.watch_launch(slot.clone(), true);
    assert_eq!(*slot.lock().unwrap(), Some(100));
    // 非合并启动：结果码恒为 0。
    let mut app = home_app();
    let slot: crate::remote::ResultSlot = Default::default();
    app.watch_launch(slot.clone(), false);
    drop(app);
    assert_eq!(*slot.lock().unwrap(), Some(0));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn handed_off_merges_open_tabs_and_answer_with_the_merge_result() {
    use crate::remote::{channel, RemoteOpen};
    use std::sync::{Arc, Mutex};
    let (root, launch) = merge_files("remote");
    let outcomes: Arc<Mutex<Vec<Outcome>>> = Default::default();
    let done = |outcomes: &Arc<Mutex<Vec<Outcome>>>| {
        let outcomes = outcomes.clone();
        Some(Box::new(move |o| outcomes.lock().unwrap().push(o)) as crate::remote::Completion)
    };
    let request = |launch: &crate::merge::MergeLaunch, done| RemoteOpen {
        left: None,
        right: None,
        merge: Some(Box::new(launch.clone())),
        done,
    };
    let ctx = egui::Context::default();
    let mut app = home_app();
    let (tx, inbox) = channel();
    app.attach_remote(&ctx, inbox);
    run(&ctx, &mut app, vec![]);
    assert!(tx.send(request(&launch, done(&outcomes))).is_ok());
    app.poll_remote(&ctx);
    assert_eq!(app.tab_count(), 2);
    assert!(matches!(app.active_view(), View::TextMerge(_)));
    run(&ctx, &mut app, vec![]);
    // 用户不保存就关：等待方得到 101（合并输出未写出）。
    let View::TextMerge(merge) = &mut app.tabs[1].view else {
        unreachable!()
    };
    assert!(merge.is_dirty());
    app.remove_tab(&ctx, 1);
    assert_eq!(*outcomes.lock().unwrap(), vec![Outcome::Closed(101)]);
    assert!(!root.join("result.txt").exists());

    // 解决冲突并保存后关：0。
    assert!(tx.send(request(&launch, done(&outcomes))).is_ok());
    app.poll_remote(&ctx);
    let View::TextMerge(merge) = &mut app.tabs[1].view else {
        unreachable!()
    };
    merge.command("Take Right");
    merge.command("Save Output");
    assert!(!merge.is_dirty());
    app.close_tab(&ctx, 1);
    assert_eq!(app.tab_count(), 1);
    assert_eq!(outcomes.lock().unwrap().last(), Some(&Outcome::Closed(0)));
    assert_eq!(
        std::fs::read_to_string(root.join("result.txt")).unwrap(),
        "one\ntwo\nYOURS\nfour\nfive\n"
    );

    // 打不开：回 Home 显示错误，等待方得到 Failed。
    let mut missing = launch;
    missing.spec.left = root.join("missing.txt");
    assert!(tx.send(request(&missing, done(&outcomes))).is_ok());
    app.poll_remote(&ctx);
    assert_eq!(app.tab_count(), 1);
    assert!(matches!(
        outcomes.lock().unwrap().last(),
        Some(Outcome::Failed(message)) if message.contains("missing.txt")
    ));
    let _ = std::fs::remove_dir_all(&root);
}

/// 表格合并（xlsx）：启动打开表格合并标签页，关页的结果码与文本合并同一条管道。
#[test]
fn table_merge_launch_and_hand_off_report_result_codes() {
    use crate::remote::{channel, RemoteOpen};
    use std::sync::{Arc, Mutex};
    let f = crate::table_merge::model::tests::two_sheet_books("app");
    let ctx = egui::Context::default();

    // 本实例启动：标签页类型/标题；带输出目标未保存，关页先问；带冲突保存后关为 14。
    let options = LaunchOptions {
        merge: Some(f.launch.clone()),
        ..Default::default()
    };
    let view = MatchaApp::launch_view(&ctx, &options);
    assert!(matches!(view, View::TableMerge(_)));
    assert_eq!(view.kind(), "Table Merge");
    assert_eq!(view.caption(), "yours.xlsx <--> theirs.xlsx");
    let mut app = MatchaApp::with_view(view);
    let slot: crate::remote::ResultSlot = Default::default();
    app.watch_launch(slot.clone(), true);
    run(&ctx, &mut app, vec![]);
    app.close_tab(&ctx, 0);
    assert_eq!(app.tab_count(), 1);
    assert!(app.active_view().prompt_open());
    let View::TableMerge(merge) = &mut app.tabs[0].view else {
        unreachable!()
    };
    let target = merge.model.target.clone().unwrap();
    merge.model.save(&target).unwrap();
    drop(app);
    assert_eq!(*slot.lock().unwrap(), Some(14));
    assert!(f.dir.join("result.xlsx").is_file());
    std::fs::remove_file(f.dir.join("result.xlsx")).unwrap();

    // 转交给常驻实例：不保存关 → 101 且不写输出；两个 sheet 都解决、保存后关 → 0。
    let outcomes: Arc<Mutex<Vec<Outcome>>> = Default::default();
    let request = |launch: &crate::merge::MergeLaunch| {
        let outcomes = outcomes.clone();
        RemoteOpen {
            left: None,
            right: None,
            merge: Some(Box::new(launch.clone())),
            done: Some(Box::new(move |o| outcomes.lock().unwrap().push(o))),
        }
    };
    let mut app = home_app();
    let (tx, inbox) = channel();
    app.attach_remote(&ctx, inbox);
    run(&ctx, &mut app, vec![]);
    assert!(tx.send(request(&f.launch)).is_ok());
    app.poll_remote(&ctx);
    assert_eq!(app.tab_count(), 2);
    assert!(matches!(app.active_view(), View::TableMerge(_)));
    run(&ctx, &mut app, vec![]);
    app.remove_tab(&ctx, 1);
    assert_eq!(*outcomes.lock().unwrap(), vec![Outcome::Closed(101)]);
    assert!(!f.dir.join("result.xlsx").exists());

    assert!(tx.send(request(&f.launch)).is_ok());
    app.poll_remote(&ctx);
    run(&ctx, &mut app, vec![]);
    let View::TableMerge(merge) = &mut app.tabs[1].view else {
        unreachable!()
    };
    merge.command("Take Right");
    merge.command("Next Change Section");
    merge.command("Take Right");
    assert_eq!(merge.unresolved(), 0);
    merge.command("Save Output");
    assert!(!merge.is_dirty(), "{}", merge.message);
    app.close_tab(&ctx, 1);
    assert_eq!(app.tab_count(), 1);
    assert_eq!(outcomes.lock().unwrap().last(), Some(&Outcome::Closed(0)));
    let mut book = bc_engine_table::workbook::Workbook::open(
        &f.dir.join("result.xlsx"),
        std::sync::Arc::new(bc_core::cancel::NeverCancel),
    )
    .unwrap();
    assert_eq!(book.sheet_names(), ["Stock", "Prices"]);
    let prices = book.load_sheet("Prices").unwrap();
    assert_eq!(prices.display_at(1, 1).as_deref(), Some("120"));
}

fn esc(ctx: &egui::Context, app: &mut MatchaApp) {
    run(
        ctx,
        app,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
}

#[test]
fn escape_closes_tabs_one_at_a_time_then_exits() {
    let ctx = egui::Context::default();
    let mut app = home_app();
    app.open_tab(View::TableCompare(Box::new(TableCompareView::empty())));
    app.open_tab(View::TableCompare(Box::new(TableCompareView::empty())));
    run(&ctx, &mut app, vec![]);
    assert_eq!((app.tab_count(), app.active_tab()), (3, 2));
    // 多标签：每次 Esc 只关当前页，不退出进程。
    esc(&ctx, &mut app);
    assert_eq!((app.tab_count(), app.active_tab()), (2, 1));
    assert!(!app.exiting && !app.exit_confirmed);
    esc(&ctx, &mut app);
    assert_eq!((app.tab_count(), app.active_tab()), (1, 0));
    assert!(!app.exit_confirmed);
    // 关掉最后一页即退出进程。
    esc(&ctx, &mut app);
    assert!(app.exit_confirmed);
}

#[test]
fn escape_on_a_dirty_tab_prompts_and_escape_again_cancels() {
    let ctx = egui::Context::default();
    let mut app = home_app();
    app.open_tab(dirty_text());
    run(&ctx, &mut app, vec![]);
    esc(&ctx, &mut app);
    run(&ctx, &mut app, vec![]);
    assert!(app.active_view().prompt_open());
    // 保存确认框吃掉这次 Esc（= Cancel）：标签页保留。
    esc(&ctx, &mut app);
    run(&ctx, &mut app, vec![]);
    assert!(!app.active_view().prompt_open());
    assert_eq!(app.tab_count(), 2);
    assert!(!app.exit_confirmed);
    // 再按 Esc 重新发起关闭；放弃修改后只关这一页。
    esc(&ctx, &mut app);
    run(&ctx, &mut app, vec![]);
    let out = run(&ctx, &mut app, vec![]);
    click(&ctx, &mut app, text_at(&out, "Don't Save"));
    run(&ctx, &mut app, vec![]);
    assert_eq!(app.tab_count(), 1);
    assert!(matches!(app.active_view(), View::Home(_)));
    assert!(!app.exit_confirmed);
}

#[test]
fn escape_closes_the_find_bar_instead_of_exiting() {
    let ctx = egui::Context::default();
    let content = || TextContent::from_bytes(b"same\n".to_vec(), None).unwrap();
    let view = TextCompareView::from_contents(content(), content(), FormatRegistry::with_builtins());
    let mut app = MatchaApp::with_view(View::TextCompare(Box::new(view)));
    run(&ctx, &mut app, vec![]);
    ctrl(&ctx, &mut app, egui::Key::F);
    let View::TextCompare(view) = app.active_view() else {
        unreachable!()
    };
    assert!(view.find_open());
    // Esc 关闭查找条且不触发退出；查找条已关后再按 Esc 才退出。
    esc(&ctx, &mut app);
    let View::TextCompare(view) = app.active_view() else {
        unreachable!()
    };
    assert!(!view.find_open());
    assert!(!app.exiting && !app.exit_confirmed);
    esc(&ctx, &mut app);
    assert!(app.exit_confirmed);
}
