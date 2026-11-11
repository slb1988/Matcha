//! `matcha` – main GUI executable (reference: the single-instance main program).
//!
//! 命令行先剥离开发用 `--` 开关（[`routing::split_dev_flags`]），其余交给
//! `bc_script::cli::CliArgs::parse`（参考开关集 + 位置参数推断）。
//!
//! 单实例（默认）：首个实例持有按用户命名的互斥体并在命名管道上监听；后来的实例把
//! 工作目录与原始参数转交过去后退出，常驻实例把比较开成新标签页（无路径时只把窗口
//! 提到前台）。`/solo`、开发开关与 `MATCHA_*PERF*` 计时运行不参与单实例，总是自己开窗口。
//!
//! 等待：环境变量 `MATCHA_LAUNCHER_WAIT`（`matcha-compc` 设置，参数本身原样转发）使转交
//! 方阻塞到常驻实例关闭该标签页，并以回复的退出码结束。本进程自己成为首个实例时，
//! 等待即等本进程退出（关闭唯一的标签页会关闭窗口）。
//!
//! ```text
//! matcha [<switches>] [<left> <right>] [--bench[=<frames>]] [--snapshot=<scene>] ...
//! ```
mod routing;

use bc_gui::remote::{Outcome, RemoteOpen, RemoteSender};
use bc_instance::{Reply, Request, Responder, Role, WAIT_ENV};
use bc_script::cli::{self, CliArgs};
use bc_script::ExitCode as Code;
use routing::Route;
use std::process::ExitCode;
use std::time::Duration;

/// 互斥体已存在而管道迟迟连不上（常驻实例卡住）时，放弃转交改为自己开窗口。
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

fn exit(code: Code) -> ExitCode {
    code.into()
}

fn exit_raw(code: i32) -> ExitCode {
    ExitCode::from(u8::try_from(code).unwrap_or(Code::UnknownError.code() as u8))
}

fn main() -> ExitCode {
    bc_gui::perf::t0();
    bc_gui::perf::mark("main:enter");
    // 读后即删：GUI 再启动的外部程序不应继承。
    let wait = std::env::var_os(WAIT_ENV).is_some();
    std::env::remove_var(WAIT_ENV);

    let (dev, args) = match routing::split_dev_flags(std::env::args().skip(1)) {
        Ok(split) => split,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };
    let cli = match CliArgs::parse(&args) {
        Ok(cli) => cli,
        Err(e) => {
            eprintln!("{e}\n\n{}", cli::usage());
            return exit(Code::UnknownError);
        }
    };
    let perf_env = bc_gui::perf::enabled() || bc_gui::perf::exit_after_ready();
    let route = routing::route(&dev, &cli, perf_env);
    let pair = routing::comparison_pair(&cli.invocation);
    match route {
        Route::Help => {
            print!("{}\n{}", cli::usage(), routing::DEV_USAGE);
            println!("scenes: {}", bc_gui::snapshot::SCENES.join(", "));
            ExitCode::SUCCESS
        }
        Route::Unsupported(message) => {
            eprintln!("{message}");
            exit(Code::UnknownError)
        }
        Route::Standalone => {
            if dev.bench_dir && !matches!(&pair, Some((l, r)) if l.is_dir() && r.is_dir()) {
                eprintln!("--bench-dir requires two readable folders");
                return ExitCode::FAILURE;
            }
            run_gui(dev.into_launch(pair), None)
        }
        Route::SingleInstance => single_instance(dev.into_launch(pair), args, wait),
    }
}

fn run_gui(
    options: bc_gui::LaunchOptions,
    remote: Option<bc_gui::remote::RemoteInbox>,
) -> ExitCode {
    match bc_gui::run_with_remote(options, remote) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("gui error: {e}");
            exit(Code::UnknownError)
        }
    }
}

/// 已有实例则转交（必要时等待），否则成为常驻实例。任何单实例环节失败都退回
/// “自己开窗口”，不让用户的比较丢失。
fn single_instance(options: bc_gui::LaunchOptions, args: Vec<String>, wait: bool) -> ExitCode {
    let name = bc_instance::instance_name();
    match bc_instance::claim(&name) {
        Ok(Role::Secondary) => {
            if let Some(code) = hand_off(&name, args, wait) {
                return code;
            }
            run_gui(options, None)
        }
        Ok(Role::Primary(_guard)) => {
            let (sender, inbox) = bc_gui::remote::channel();
            match bc_instance::serve(&name, move |request, responder| {
                accept(&sender, request, responder)
            }) {
                Ok(()) => run_gui(options, Some(inbox)),
                Err(e) => {
                    eprintln!("single-instance listener unavailable ({e}); running alone");
                    run_gui(options, None)
                }
            }
        }
        Err(e) => {
            eprintln!("single-instance check failed ({e}); running alone");
            run_gui(options, None)
        }
    }
}

/// 转交方：`None` = 连不上常驻实例（调用方改为自己开窗口）。
fn hand_off(name: &str, args: Vec<String>, wait: bool) -> Option<ExitCode> {
    let client = match bc_instance::connect(name, CONNECT_TIMEOUT) {
        Ok(client) => client,
        Err(e) => {
            eprintln!("running instance did not answer ({e}); opening a separate window");
            return None;
        }
    };
    let request = Request {
        cwd: std::env::current_dir().unwrap_or_default(),
        args,
        wait,
    };
    Some(match client.send(&request) {
        Ok(reply) => {
            if let Some(message) = reply.message {
                eprintln!("{message}");
            }
            exit_raw(reply.exit_code)
        }
        // 常驻实例在回复前消失：无从得知会话结果。
        Err(e) => {
            eprintln!("lost the running instance while waiting ({e})");
            exit(Code::LauncherCannotWait)
        }
    })
}

/// 常驻实例侧（监听线程）：按调用方工作目录解析参数，投递给 UI 线程。不等待的请求
/// 投递成功即回 0；等待的请求在标签页关闭时回复。
fn accept(sender: &RemoteSender, request: Request, responder: Responder) {
    let cli = match CliArgs::parse_in(&request.args, &request.cwd) {
        Ok(cli) => cli,
        Err(e) => {
            return responder.reply(Reply::error(
                Code::UnknownError.code(),
                format!("{e}\n\n{}", cli::usage()),
            ))
        }
    };
    if let Some(message) = routing::unsupported(&cli) {
        return responder.reply(Reply::error(Code::UnknownError.code(), message));
    }
    let (left, right) = routing::comparison_pair(&cli.invocation).unzip();
    let (now, done) = if request.wait {
        let done: bc_gui::remote::Completion = Box::new(move |outcome| {
            responder.reply(match outcome {
                Outcome::Closed => Reply::code(Code::Success.code()),
                Outcome::Failed(message) => Reply::error(Code::UnknownError.code(), message),
            })
        });
        (None, Some(done))
    } else {
        (Some(responder), None)
    };
    match sender.send(RemoteOpen { left, right, done }) {
        Ok(()) => {
            if let Some(responder) = now {
                responder.reply(Reply::code(Code::Success.code()));
            }
        }
        Err(open) => {
            // 窗口已关闭、进程正在退出。
            let message = "the running instance is shutting down";
            match (open.done, now) {
                (Some(done), _) => done(Outcome::Failed(message.into())),
                (None, Some(responder)) => {
                    responder.reply(Reply::error(Code::UnknownError.code(), message))
                }
                (None, None) => {}
            }
        }
    }
}
