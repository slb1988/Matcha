//! 端到端：测试进程扮演常驻实例（持有互斥体并监听管道），启动真实的 `matcha` /
//! `matcha-compc` 作为后来的实例，检查转交内容与退出码。不打开任何窗口。
#![cfg(windows)]
use bc_instance::{Reply, Request, Responder, Role};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{mpsc, Mutex};
use std::time::{Duration, Instant};

struct FakePrimary {
    name: String,
    rx: mpsc::Receiver<(Request, Responder)>,
    _guard: bc_instance::PrimaryGuard,
}

fn fake_primary(tag: &str) -> FakePrimary {
    let name = format!("Matcha-test-{tag}-{}", std::process::id());
    let Role::Primary(guard) = bc_instance::claim(&name).unwrap() else {
        panic!("{name} already claimed");
    };
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    bc_instance::serve(&name, move |request, responder| {
        let _ = tx.lock().unwrap().send((request, responder));
    })
    .unwrap();
    FakePrimary {
        name,
        rx,
        _guard: guard,
    }
}

impl FakePrimary {
    fn spawn(&self, exe: &Path, args: &[&str], cwd: &Path, wait: bool) -> Child {
        let mut command = Command::new(exe);
        command
            .args(args)
            .current_dir(cwd)
            .env("MATCHA_INSTANCE", &self.name)
            .env_remove(bc_instance::WAIT_ENV);
        if wait {
            command.env(bc_instance::WAIT_ENV, "1");
        }
        command.spawn().unwrap()
    }
    fn next(&self) -> (Request, Responder) {
        self.rx
            .recv_timeout(Duration::from_secs(20))
            .expect("no hand-off arrived")
    }
}

fn exit_code(mut child: Child) -> Option<i32> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status.code();
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("process did not exit");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn matcha() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_matcha"))
}

fn work_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("matcha-handoff-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn second_instance_forwards_cwd_and_arguments_then_exits_zero() {
    let primary = fake_primary("plain");
    let cwd = work_dir("plain");
    let child = primary.spawn(&matcha(), &["a.txt", "/ro", "b.txt"], &cwd, false);
    let (request, responder) = primary.next();
    assert_eq!(request.args, ["a.txt", "/ro", "b.txt"]);
    assert_eq!(
        request.cwd.canonicalize().unwrap(),
        cwd.canonicalize().unwrap()
    );
    assert!(!request.wait);
    responder.reply(Reply::code(0));
    assert_eq!(exit_code(child), Some(0));
}

#[test]
fn waiting_instance_blocks_until_the_tab_closes_and_returns_its_code() {
    let primary = fake_primary("wait");
    let cwd = work_dir("wait");
    let mut child = primary.spawn(&matcha(), &["a.txt", "b.txt"], &cwd, true);
    let (request, responder) = primary.next();
    assert!(request.wait);
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        child.try_wait().unwrap().is_none(),
        "returned before the tab closed"
    );
    responder.reply(Reply::code(13));
    assert_eq!(exit_code(child), Some(13));
}

#[test]
fn losing_the_running_instance_while_waiting_exits_102() {
    let primary = fake_primary("lost");
    let cwd = work_dir("lost");
    let child = primary.spawn(&matcha(), &["a.txt", "b.txt"], &cwd, true);
    drop(primary.next());
    assert_eq!(exit_code(child), Some(102));
}

#[test]
fn unsupported_invocations_fail_locally_without_a_hand_off() {
    let primary = fake_primary("unsupported");
    let cwd = work_dir("unsupported");
    let child = primary.spawn(&matcha(), &["fix.patch"], &cwd, false);
    assert_eq!(exit_code(child), Some(100));
    assert!(primary.rx.try_recv().is_err());
}

/// 合并调用总是等待：没有启动器的等待标记，转交方也阻塞到标签页关闭，并以其结果码结束。
#[test]
fn merge_hand_off_waits_without_the_launcher_and_returns_the_tab_code() {
    let primary = fake_primary("merge");
    let cwd = work_dir("merge");
    let args = ["base.txt", "theirs.txt", "yours.txt", "result.txt"];
    let mut child = primary.spawn(&matcha(), &args, &cwd, false);
    let (request, responder) = primary.next();
    assert_eq!(request.args, args);
    assert!(request.wait, "P4V starts matcha.exe directly and must still wait");
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        child.try_wait().unwrap().is_none(),
        "returned before the merge tab closed"
    );
    // 用户没保存就关了标签页。
    responder.reply(Reply::code(101));
    assert_eq!(exit_code(child), Some(101));
    // 三个路径、`/center=` 的写法同样等待。
    for args in [
        &["base.txt", "theirs.txt", "yours.txt"][..],
        &["/center=base.txt", "/mergeoutput=result.txt", "theirs.txt", "yours.txt"],
    ] {
        let child = primary.spawn(&matcha(), args, &cwd, false);
        let (request, responder) = primary.next();
        assert!(request.wait, "{args:?}");
        responder.reply(Reply::code(0));
        assert_eq!(exit_code(child), Some(0));
    }
}

const MERGE_BASE: &str = "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\n";

/// `/automerge` 在发起进程内完成：已有常驻实例时也不转交、不开窗口；退出码 0 / 14 / 101 / 100。
#[test]
fn automerge_never_hands_off() {
    let primary = fake_primary("automerge");
    let cwd = work_dir("automerge");
    std::fs::write(cwd.join("base.txt"), MERGE_BASE).unwrap();
    std::fs::write(cwd.join("a.txt"), MERGE_BASE.replace("two", "TWO")).unwrap();
    std::fs::write(cwd.join("b.txt"), MERGE_BASE.replace("seven", "SEVEN")).unwrap();
    std::fs::write(cwd.join("c.txt"), MERGE_BASE.replace("two", "2")).unwrap();
    let run = |args: &[&str]| exit_code(primary.spawn(&matcha(), args, &cwd, false));
    // 无冲突：写出，0。
    assert_eq!(
        run(&["/silent", "/automerge", "base.txt", "b.txt", "a.txt", "out.txt"]),
        Some(0)
    );
    assert_eq!(
        std::fs::read_to_string(cwd.join("out.txt")).unwrap(),
        MERGE_BASE.replace("two", "TWO").replace("seven", "SEVEN")
    );
    // 有冲突：不写，101；`/force`：带标记写出，14。
    assert_eq!(
        run(&["/silent", "/automerge", "base.txt", "c.txt", "a.txt", "conflict.txt"]),
        Some(101)
    );
    assert!(!cwd.join("conflict.txt").exists());
    assert_eq!(
        run(&[
            "/silent",
            "/automerge",
            "/force",
            "/title1=Theirs",
            "/mergeoutput=conflict.txt",
            "base.txt",
            "c.txt",
            "a.txt"
        ]),
        Some(14)
    );
    assert_eq!(
        std::fs::read_to_string(cwd.join("conflict.txt")).unwrap(),
        MERGE_BASE.replace("two\n", "<<<<<<< Theirs\nTWO\n=======\n2\n>>>>>>> c.txt\n")
    );
    // 缺输入、缺输出目标：100。
    assert_eq!(
        run(&["/silent", "/automerge", "base.txt", "missing.txt", "a.txt", "x.txt"]),
        Some(100)
    );
    assert_eq!(
        run(&["/silent", "/automerge", "base.txt", "b.txt", "a.txt"]),
        Some(100)
    );
    assert!(!cwd.join("x.txt").exists());
    assert!(primary.rx.try_recv().is_err());
}

/// `/automerge /reviewconflicts`：没有冲突就不开窗口；有冲突时不写输出，改为转交合并调用
/// （去掉 `/automerge`，并等待）。`/silent` 使 `/reviewconflicts` 失效。
#[test]
fn review_conflicts_opens_the_merge_only_when_conflicts_remain() {
    let primary = fake_primary("review");
    let cwd = work_dir("review");
    std::fs::write(cwd.join("base.txt"), MERGE_BASE).unwrap();
    std::fs::write(cwd.join("a.txt"), MERGE_BASE.replace("two", "TWO")).unwrap();
    std::fs::write(cwd.join("b.txt"), MERGE_BASE.replace("seven", "SEVEN")).unwrap();
    std::fs::write(cwd.join("c.txt"), MERGE_BASE.replace("two", "2")).unwrap();
    let clean = primary.spawn(
        &matcha(),
        &["/automerge", "/reviewconflicts", "/force", "base.txt", "b.txt", "a.txt", "out.txt"],
        &cwd,
        false,
    );
    assert_eq!(exit_code(clean), Some(0));
    assert!(cwd.join("out.txt").is_file());
    assert!(primary.rx.try_recv().is_err());

    let mut child = primary.spawn(
        &matcha(),
        &["/automerge", "/reviewconflicts", "/force", "/iu", "base.txt", "c.txt", "a.txt", "out2.txt"],
        &cwd,
        false,
    );
    let (request, responder) = primary.next();
    assert_eq!(
        request.args,
        ["/reviewconflicts", "/force", "/iu", "base.txt", "c.txt", "a.txt", "out2.txt"]
    );
    assert!(request.wait);
    assert!(!cwd.join("out2.txt").exists(), "/reviewconflicts wins over /force");
    assert!(child.try_wait().unwrap().is_none());
    responder.reply(Reply::code(0));
    assert_eq!(exit_code(child), Some(0));

    let silent = primary.spawn(
        &matcha(),
        &["/silent", "/automerge", "/reviewconflicts", "base.txt", "c.txt", "a.txt", "out3.txt"],
        &cwd,
        false,
    );
    assert_eq!(exit_code(silent), Some(101));
    assert!(primary.rx.try_recv().is_err());
}

/// 单 sheet 工作簿：表头 + `(id, qty)` 行。
fn stock_book(path: &Path, rows: &[(f64, f64)]) {
    use bc_engine_table::{xlsx_write, Cell, Sheet};
    let mut sheet = Sheet::new("Stock");
    sheet.push_row(vec![Cell::text("id"), Cell::text("qty")]);
    for (id, qty) in rows {
        sheet.push_row(vec![Cell::number(*id), Cell::number(*qty)]);
    }
    std::fs::write(path, xlsx_write::workbook_bytes(&[sheet], None).unwrap()).unwrap();
}

/// 表格合并（xlsx）与文本合并走同一条转交 / 等待管道：普通合并调用转交后等待并以标签页结果码结束；
/// `/automerge /reviewconflicts` 无冲突不开窗口，有冲突时不写输出、去掉 `/automerge` 转交
/// （`/keys=`、favor 随参数带给界面）。
#[test]
fn table_merges_hand_off_and_wait_like_text_merges() {
    let primary = fake_primary("table");
    let cwd = work_dir("table");
    stock_book(&cwd.join("base.xlsx"), &[(1.0, 10.0), (2.0, 20.0)]);
    stock_book(&cwd.join("theirs.xlsx"), &[(1.0, 15.0), (2.0, 20.0)]);
    stock_book(&cwd.join("yours.xlsx"), &[(1.0, 12.0), (2.0, 20.0)]);
    stock_book(&cwd.join("clean.xlsx"), &[(1.0, 10.0), (2.0, 22.0)]);

    // P4V 形式 `%b %1 %2 %r`：转交并等待，标签页结果码透传（未保存关闭 101、保存 0、带冲突保存 14）。
    let args = ["base.xlsx", "theirs.xlsx", "yours.xlsx", "result.xlsx"];
    for code in [101, 0, 14] {
        let mut child = primary.spawn(&matcha(), &args, &cwd, false);
        let (request, responder) = primary.next();
        assert_eq!(request.args, args);
        assert!(request.wait);
        std::thread::sleep(Duration::from_millis(200));
        assert!(
            child.try_wait().unwrap().is_none(),
            "returned before the tab closed"
        );
        responder.reply(Reply::code(code));
        assert_eq!(exit_code(child), Some(code));
    }
    assert!(!cwd.join("result.xlsx").exists());

    // 无冲突：发起进程内写出，0，不转交。
    let clean = primary.spawn(
        &matcha(),
        &[
            "/automerge",
            "/reviewconflicts",
            "base.xlsx",
            "clean.xlsx",
            "yours.xlsx",
            "out.xlsx",
        ],
        &cwd,
        false,
    );
    assert_eq!(exit_code(clean), Some(0));
    assert!(cwd.join("out.xlsx").is_file());
    assert!(primary.rx.try_recv().is_err());

    // 有冲突：不写输出，转交（带 `/keys=1`、`/favorleft`），等待标签页结果码。
    let mut child = primary.spawn(
        &matcha(),
        &[
            "/automerge",
            "/reviewconflicts",
            "/keys=1",
            "/favorleft",
            "base.xlsx",
            "theirs.xlsx",
            "yours.xlsx",
            "out2.xlsx",
        ],
        &cwd,
        false,
    );
    let (request, responder) = primary.next();
    assert_eq!(
        request.args,
        [
            "/reviewconflicts",
            "/keys=1",
            "/favorleft",
            "base.xlsx",
            "theirs.xlsx",
            "yours.xlsx",
            "out2.xlsx"
        ]
    );
    assert!(request.wait);
    assert!(!cwd.join("out2.xlsx").exists());
    assert!(child.try_wait().unwrap().is_none());
    responder.reply(Reply::code(14));
    assert_eq!(exit_code(child), Some(14));

    // `/silent` 使 `/reviewconflicts` 失效：101，不转交、不写。
    let silent = primary.spawn(
        &matcha(),
        &[
            "/silent",
            "/automerge",
            "/reviewconflicts",
            "base.xlsx",
            "theirs.xlsx",
            "yours.xlsx",
            "out3.xlsx",
        ],
        &cwd,
        false,
    );
    assert_eq!(exit_code(silent), Some(101));
    assert!(!cwd.join("out3.xlsx").exists());
    assert!(primary.rx.try_recv().is_err());
}

/// `/qc` 在发起进程内完成：即使已有常驻实例也不转交、不开窗口。
#[test]
fn quick_compare_never_hands_off() {
    let primary = fake_primary("qc");
    let cwd = work_dir("qc");
    std::fs::write(cwd.join("a.txt"), "same\n").unwrap();
    std::fs::write(cwd.join("b.txt"), "same\n").unwrap();
    // `/silent`：不写结果行（无控制台的 CI 里也不会退化为消息框）。
    let child = primary.spawn(
        &matcha(),
        &["/silent", "/qc", "a.txt", "b.txt"],
        &cwd,
        false,
    );
    assert_eq!(exit_code(child), Some(2));
    assert!(primary.rx.try_recv().is_err());
}

/// 脚本与 `/report` 同样在发起进程内完成：已有常驻实例时也不转交、不开窗口。
#[test]
fn scripts_and_reports_never_hand_off() {
    let primary = fake_primary("scripts");
    let cwd = work_dir("scripts");
    std::fs::write(
        cwd.join("a.txt"),
        "x
",
    )
    .unwrap();
    std::fs::write(
        cwd.join("b.txt"),
        "y
",
    )
    .unwrap();
    std::fs::write(
        cwd.join("s.txt"),
        "text-report layout:patch output-to:out.patch a.txt b.txt
",
    )
    .unwrap();
    let child = primary.spawn(&matcha(), &["@s.txt"], &cwd, false);
    assert_eq!(exit_code(child), Some(0));
    assert!(cwd.join("out.patch").is_file());
    let child = primary.spawn(
        &matcha(),
        &["/report=patch", "/reportfile=r.patch", "a.txt", "b.txt"],
        &cwd,
        false,
    );
    assert_eq!(exit_code(child), Some(0));
    assert!(cwd.join("r.patch").is_file());
    assert!(primary.rx.try_recv().is_err());
}

/// `matcha-compc` → `matcha` → 常驻实例的完整链路。启动器属于另一个包，只有
/// `cargo test --workspace`（或先构建过 bcomp）时才在同一目录里。
#[test]
fn console_launcher_propagates_the_handed_off_exit_code() {
    let launcher = matcha().with_file_name(format!("matcha-compc{}", std::env::consts::EXE_SUFFIX));
    if !launcher.is_file() {
        eprintln!("skipped: {} not built", launcher.display());
        return;
    }
    let primary = fake_primary("compc");
    let cwd = work_dir("compc");
    // 启动器自己设置等待标记，这里不设。
    let mut child = primary.spawn(&launcher, &["a.txt", "b.txt"], &cwd, false);
    let (request, responder) = primary.next();
    assert!(request.wait, "matcha-compc must request waiting");
    assert_eq!(request.args, ["a.txt", "b.txt"]);
    std::thread::sleep(Duration::from_millis(300));
    assert!(child.try_wait().unwrap().is_none());
    responder.reply(Reply::code(1));
    assert_eq!(exit_code(child), Some(1));
}
