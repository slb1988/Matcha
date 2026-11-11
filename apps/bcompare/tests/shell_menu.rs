//! 端到端：真实的 `matcha` 处理 Explorer 菜单命令。注册表根与状态目录都改到测试专用
//! 位置（`MATCHA_SHELL_ROOT` = HKCU 下的私有键，绝不碰 `Software\Classes`），常驻实例由
//! 测试进程扮演，检查转交给它的参数。
#![cfg(windows)]
use bc_instance::{Reply, Request, Responder, Role};
use bc_shell::layout::{verb_key, Class, Verb};
use bc_shell::Registry;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{mpsc, Mutex};
use std::time::{Duration, Instant};

struct Sandbox {
    root: String,
    state: PathBuf,
    work: PathBuf,
    instance: String,
    rx: mpsc::Receiver<(Request, Responder)>,
    _guard: bc_instance::PrimaryGuard,
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let mut reg = bc_shell::registry::Hkcu;
        let _ = bc_shell::unregister(&mut reg, &self.root);
        // root = `Software\Matcha-test-shell-*\Classes`: drop the whole test key.
        if let Some((base, _)) = self.root.rsplit_once('\\') {
            let _ = reg.remove_tree(base);
        }
        let _ = std::fs::remove_dir_all(&self.work);
    }
}

fn sandbox(tag: &str) -> Sandbox {
    let id = format!("{tag}-{}", std::process::id());
    let work = std::env::temp_dir().join(format!("matcha-shell-e2e-{id}"));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    for name in ["a.txt", "b.txt", "c.txt"] {
        std::fs::write(work.join(name), name).unwrap();
    }
    let instance = format!("Matcha-test-shell-{id}");
    let Role::Primary(guard) = bc_instance::claim(&instance).unwrap() else {
        panic!("{instance} already claimed");
    };
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    bc_instance::serve(&instance, move |request, responder| {
        let _ = tx.lock().unwrap().send((request, responder));
    })
    .unwrap();
    Sandbox {
        root: format!(r"Software\Matcha-test-shell-{id}\Classes"),
        state: work.join("state"),
        work,
        instance,
        rx,
        _guard: guard,
    }
}

impl Sandbox {
    fn spawn(&self, args: &[&str]) -> Child {
        Command::new(env!("CARGO_BIN_EXE_matcha"))
            .args(args)
            .current_dir(&self.work)
            .env("MATCHA_INSTANCE", &self.instance)
            .env(bc_shell::ROOT_ENV, &self.root)
            .env(bc_shell::STATE_ENV, &self.state)
            .env_remove(bc_instance::WAIT_ENV)
            .spawn()
            .unwrap()
    }
    fn run(&self, args: &[&str]) -> Option<i32> {
        exit_code(self.spawn(args))
    }
    fn path(&self, name: &str) -> String {
        self.work.join(name).display().to_string()
    }
    fn label(&self) -> (Option<String>, bool) {
        let reg = bc_shell::registry::Hkcu;
        let key = verb_key(&self.root, Class::File, Verb::CompareTo);
        (
            reg.get(&key, "MUIVerb").unwrap(),
            reg.get(&key, "ProgrammaticAccessOnly").unwrap().is_none(),
        )
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

#[test]
fn select_left_then_compare_to_hands_off_the_pair_and_resets_the_label() {
    let sb = sandbox("left");
    assert_eq!(sb.run(&["/shellmenu=on"]), Some(0));
    let reg = bc_shell::registry::Hkcu;
    let command = reg
        .get(
            &bc_shell::layout::command_key(&sb.root, Class::File, Verb::SelectLeft),
            "",
        )
        .unwrap()
        .unwrap();
    let exe_dir = Path::new(env!("CARGO_BIN_EXE_matcha")).parent().unwrap();
    assert!(
        command.starts_with(&format!("\"{}\\", exe_dir.display())),
        "{command}"
    );
    assert!(command.ends_with(r#"" /selectleft "%1""#), "{command}");
    assert!(!sb.label().1, "Compare to hidden until a Left is chosen");

    assert_eq!(sb.run(&["/selectleft", &sb.path("a.txt")]), Some(0));
    assert!(sb.rx.try_recv().is_err(), "Select Left opens nothing");
    assert_eq!(
        sb.label(),
        (Some("Matcha: Compare to \"a.txt\"".into()), true)
    );

    let child = sb.spawn(&["/compareto", "b.txt"]);
    let (request, responder) = sb.next();
    assert_eq!(request.args, [sb.path("a.txt"), sb.path("b.txt")]);
    responder.reply(Reply::code(0));
    assert_eq!(exit_code(child), Some(0));
    assert_eq!(sb.label(), (Some("Matcha: Compare to".into()), false));

    assert_eq!(sb.run(&["/shellmenu=off"]), Some(0));
    assert!(!bc_shell::is_registered(&reg, &sb.root).unwrap());
}

#[test]
fn two_compare_invocations_pair_up_like_an_explorer_multi_selection() {
    let sb = sandbox("pair");
    // A pending Left must not hijack the multi-selection.
    assert_eq!(sb.run(&["/selectleft", &sb.path("c.txt")]), Some(0));
    let first = sb.spawn(&["/comparepair", &sb.path("a.txt")]);
    std::thread::sleep(Duration::from_millis(400));
    let second = sb.spawn(&["/comparepair", &sb.path("b.txt")]);
    let (request, responder) = sb.next();
    assert_eq!(request.args, [sb.path("a.txt"), sb.path("b.txt")]);
    responder.reply(Reply::code(0));
    assert_eq!(exit_code(second), Some(0));
    assert_eq!(exit_code(first), Some(0));
    assert!(sb.rx.try_recv().is_err(), "exactly one comparison");
    assert!(sb.state.join("shell-left.txt").is_file(), "Left untouched");
}
