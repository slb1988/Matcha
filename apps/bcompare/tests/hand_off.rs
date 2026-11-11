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
    let child = primary.spawn(&matcha(), &["a", "b", "c"], &cwd, false);
    assert_eq!(exit_code(child), Some(100));
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
