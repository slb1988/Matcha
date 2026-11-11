//! # bc-instance
//!
//! `matcha` 的单实例守卫与参数转交通道（README FR-G1）。
//!
//! * 首个实例创建按用户命名的互斥体 `Local\<name>`（[`claim`]），并在
//!   `\\.\pipe\<name>` 上监听（[`serve`]）。
//! * 后来的实例发现互斥体已存在，把工作目录与原始参数作为一行 JSON 发给常驻实例
//!   （[`send`]），读一行 JSON 回复后按其退出码结束。
//! * `wait = true` 的请求（`matcha-compc`）在常驻实例关闭对应标签页时才得到回复，
//!   同一条管道连接兼作完成通知——不需要另传同步句柄。
//!
//! 选命名管道而不是窗口消息/WM_COPYDATA：不依赖窗口已创建，能双向回传退出码，且
//! `PIPE_REJECT_REMOTE_CLIENTS` 加默认 DACL（其他用户只读）限制在本机本用户。
//! 非 Windows 平台没有单实例：[`claim`] 恒为首个实例，[`send`] 返回 `Unsupported`。
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::time::Duration;

#[cfg(windows)]
mod windows;

/// 一次转交：调用方工作目录、未改动的参数，以及是否等待会话结束。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub cwd: PathBuf,
    pub args: Vec<String>,
    pub wait: bool,
}

/// 常驻实例的回复：调用方进程用它作为退出码，`message` 打印到 stderr。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reply {
    pub exit_code: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl Reply {
    pub fn code(exit_code: i32) -> Self {
        Reply {
            exit_code,
            message: None,
        }
    }
    pub fn error(exit_code: i32, message: impl Into<String>) -> Self {
        Reply {
            exit_code,
            message: Some(message.into()),
        }
    }
}

/// 控制台启动器（`matcha-compc`，发行名 `matcha.com`）设置此环境变量启动 `matcha`，
/// 要求等待会话结束（相当于参考实现的隐藏完成句柄；命令行参数本身原样转发）。`matcha` 读取后立即删除，不让子进程继承。
pub const WAIT_ENV: &str = "MATCHA_LAUNCHER_WAIT";

/// 请求一行的上限：防止本机其他进程塞入无界数据。
const MAX_LINE: u64 = 1 << 20;

/// 把回复写回请求方；可跨线程移动（等待型请求随标签页保存，关页时回复）。
/// 未回复就丢弃时连接直接关闭，请求方读到 EOF。
pub struct Responder {
    stream: Box<dyn Write + Send>,
}

impl Responder {
    pub fn new(stream: impl Write + Send + 'static) -> Self {
        Responder {
            stream: Box::new(stream),
        }
    }
    /// 写一行 JSON；请求方已离开（如 Ctrl+C）时忽略错误。
    pub fn reply(mut self, reply: Reply) {
        let _ = write_line(&mut self.stream, &reply).and_then(|_| self.stream.flush());
    }
}

fn write_line<T: Serialize>(out: &mut dyn Write, value: &T) -> io::Result<()> {
    let mut line = serde_json::to_vec(value).map_err(io::Error::other)?;
    line.push(b'\n');
    out.write_all(&line)
}

fn read_line<T: for<'de> Deserialize<'de>>(input: impl Read) -> io::Result<T> {
    let mut line = String::new();
    BufReader::new(input.take(MAX_LINE)).read_line(&mut line)?;
    if line.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "connection closed without a message",
        ));
    }
    serde_json::from_str(&line).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// 服务端处理一条连接：读请求交给 `handler`；请求无效时直接回 100。
pub fn handle_connection<S>(stream: S, handler: &(dyn Fn(Request, Responder) + Send + Sync))
where
    S: Write + Send + 'static,
    for<'a> &'a S: Read,
{
    match read_line::<Request>(&stream) {
        Ok(request) => handler(request, Responder::new(stream)),
        Err(e) => Responder::new(stream).reply(Reply::error(100, format!("bad request: {e}"))),
    }
}

/// 客户端一次往返：写请求，阻塞读回复。
pub fn exchange<S>(mut stream: S, request: &Request) -> io::Result<Reply>
where
    S: Write,
    for<'a> &'a S: Read,
{
    write_line(&mut stream, request)?;
    stream.flush()?;
    read_line(&stream)
}

/// 按用户区分的实例名：`MATCHA_INSTANCE` 覆盖（测试用，避免撞上正在运行的 matcha），
/// 否则 `Matcha-<用户名>`。命名对象名里不能有反斜杠。
pub fn instance_name() -> String {
    if let Some(name) = std::env::var_os("MATCHA_INSTANCE").filter(|n| !n.is_empty()) {
        return name.to_string_lossy().replace('\\', "_");
    }
    let user = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_default();
    format!("Matcha-{}", user.replace('\\', "_"))
}

/// [`claim`] 的结果。
pub enum Role {
    /// 本进程是首个实例；守卫存活期间互斥体保持存在。
    Primary(PrimaryGuard),
    /// 已有实例在运行。
    Secondary,
}

/// 持有互斥体句柄；进程退出时由系统释放。
pub struct PrimaryGuard {
    #[cfg(windows)]
    _mutex: windows::Mutex,
}

/// 抢占单实例互斥体。
pub fn claim(name: &str) -> io::Result<Role> {
    #[cfg(windows)]
    {
        Ok(match windows::Mutex::create(name)? {
            Some(mutex) => Role::Primary(PrimaryGuard { _mutex: mutex }),
            None => Role::Secondary,
        })
    }
    #[cfg(not(windows))]
    {
        let _ = name;
        Ok(Role::Primary(PrimaryGuard {}))
    }
}

/// 开始监听转交请求：首个管道实例同步创建（失败即返回错误，例如名字被他人占用），
/// 之后由后台线程循环接受连接，每条连接一个线程调用 `handler`。
pub fn serve<F>(name: &str, handler: F) -> io::Result<()>
where
    F: Fn(Request, Responder) + Send + Sync + 'static,
{
    #[cfg(windows)]
    {
        windows::serve(name, std::sync::Arc::new(handler))
    }
    #[cfg(not(windows))]
    {
        let _ = (name, handler);
        Ok(())
    }
}

/// 到常驻实例的一条连接。
pub struct Client {
    #[cfg(windows)]
    pipe: std::fs::File,
}

/// 连接常驻实例。`timeout` 只限制建立连接：互斥体已建而管道尚未就绪、或服务端正在
/// 补建下一个管道实例时重试。
pub fn connect(name: &str, timeout: Duration) -> io::Result<Client> {
    #[cfg(windows)]
    {
        Ok(Client {
            pipe: windows::connect(name, timeout)?,
        })
    }
    #[cfg(not(windows))]
    {
        let _ = (name, timeout);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "single-instance hand-off is only implemented on Windows",
        ))
    }
}

impl Client {
    /// 发请求并阻塞读回复（等待型请求的回复不限时）。连接中断返回错误。
    pub fn send(self, request: &Request) -> io::Result<Reply> {
        #[cfg(windows)]
        {
            exchange(self.pipe, request)
        }
        #[cfg(not(windows))]
        {
            let _ = request;
            unreachable!("Client is never constructed off Windows")
        }
    }
}

/// [`connect`] + [`Client::send`]。
pub fn send(name: &str, request: &Request, connect_timeout: Duration) -> io::Result<Reply> {
    connect(name, connect_timeout)?.send(request)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reply_round_trips_without_message_field() {
        let mut buf = Vec::new();
        write_line(&mut buf, &Reply::code(0)).unwrap();
        assert_eq!(buf, b"{\"exit_code\":0}\n");
        let back: Reply = read_line(&buf[..]).unwrap();
        assert_eq!(back, Reply::code(0));
    }

    #[test]
    fn request_round_trips_and_eof_is_an_error() {
        let request = Request {
            cwd: PathBuf::from("C:/work"),
            args: vec!["a b.txt".into(), "/solo".into()],
            wait: true,
        };
        let mut buf = Vec::new();
        write_line(&mut buf, &request).unwrap();
        assert_eq!(read_line::<Request>(&buf[..]).unwrap(), request);
        let eof = read_line::<Reply>(&b""[..]).unwrap_err();
        assert_eq!(eof.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn instance_name_is_a_valid_object_name() {
        // 只读检查：不同测试并行时不修改环境变量。
        let name = instance_name();
        assert!(!name.contains('\\'));
        assert!(!name.is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn second_claim_in_the_same_session_is_secondary() {
        let name = format!("Matcha-test-claim-{}", std::process::id());
        let first = claim(&name).unwrap();
        assert!(matches!(first, Role::Primary(_)));
        assert!(matches!(claim(&name).unwrap(), Role::Secondary));
        drop(first);
        // 句柄全部关闭后对象消失，可以重新成为首个实例。
        assert!(matches!(claim(&name).unwrap(), Role::Primary(_)));
    }

    #[cfg(windows)]
    #[test]
    fn pipe_hand_off_replies_immediately_or_after_completion() {
        use std::sync::mpsc;
        let name = format!("Matcha-test-pipe-{}", std::process::id());
        let (tx, rx) = mpsc::channel::<(Request, Responder)>();
        let tx = std::sync::Mutex::new(tx);
        serve(&name, move |request, responder| {
            if request.wait {
                // 等待型：交给“UI 线程”，稍后关页时回复。
                tx.lock().unwrap().send((request, responder)).unwrap();
            } else {
                responder.reply(Reply::code(0));
            }
        })
        .unwrap();
        // 同名第二个服务端被 FILE_FLAG_FIRST_PIPE_INSTANCE 拒绝（防止抢注）。
        assert!(serve(&name, |_, r| r.reply(Reply::code(0))).is_err());

        let quick = Request {
            cwd: PathBuf::from("."),
            args: vec!["a".into(), "b".into()],
            wait: false,
        };
        let timeout = Duration::from_secs(5);
        assert_eq!(send(&name, &quick, timeout).unwrap(), Reply::code(0));

        let waiting = std::thread::spawn({
            let name = name.clone();
            move || {
                let request = Request {
                    wait: true,
                    ..quick
                };
                send(&name, &request, timeout)
            }
        });
        let (request, responder) = rx.recv_timeout(timeout).unwrap();
        assert!(request.wait);
        assert!(!waiting.is_finished());
        responder.reply(Reply::code(13));
        assert_eq!(waiting.join().unwrap().unwrap(), Reply::code(13));

        // 服务端不回复就丢弃 → 请求方读到 EOF。
        let dropped = std::thread::spawn({
            let name = name.clone();
            move || {
                let request = Request {
                    cwd: PathBuf::from("."),
                    args: vec![],
                    wait: true,
                };
                send(&name, &request, timeout)
            }
        });
        drop(rx.recv_timeout(timeout).unwrap());
        assert!(dropped.join().unwrap().is_err());
    }

    #[cfg(windows)]
    #[test]
    fn connecting_without_a_server_times_out() {
        let name = format!("Matcha-test-absent-{}", std::process::id());
        let request = Request {
            cwd: PathBuf::from("."),
            args: vec![],
            wait: false,
        };
        let started = std::time::Instant::now();
        assert!(send(&name, &request, Duration::from_millis(200)).is_err());
        assert!(started.elapsed() < Duration::from_secs(3));
    }
}
