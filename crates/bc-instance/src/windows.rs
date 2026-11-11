//! 经审计的 Windows API 边界：命名互斥体与命名管道。连接建立后句柄交给
//! `std::fs::File` 拥有，读写与关闭都走 std。
use crate::{Request, Responder};
use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::sync::Arc;
use std::time::{Duration, Instant};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY,
        ERROR_PIPE_CONNECTED, HANDLE, INVALID_HANDLE_VALUE,
    },
    Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX},
    System::{
        Pipes::{
            ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeServerProcessId, PIPE_READMODE_BYTE,
            PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
        },
        Threading::CreateMutexW,
    },
    UI::WindowsAndMessaging::AllowSetForegroundWindow,
};

type Handler = Arc<dyn Fn(Request, Responder) + Send + Sync>;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn pipe_path(name: &str) -> String {
    format!(r"\\.\pipe\{name}")
}

/// 命名互斥体句柄（`Local\` 命名空间：按登录会话隔离）。
pub struct Mutex(HANDLE);

// SAFETY: 互斥体句柄只用于存续期与关闭，可在线程间移动。
unsafe impl Send for Mutex {}
unsafe impl Sync for Mutex {}

impl Mutex {
    /// `Some` = 新建（首个实例）；`None` = 已存在（另一实例持有）。
    pub fn create(name: &str) -> io::Result<Option<Mutex>> {
        let name = wide(&format!(r"Local\{name}"));
        // SAFETY: name 是以 0 结尾的 UTF-16；空安全属性 = 默认 DACL。
        let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
        // 必须紧接调用读取，期间不能有其他 API 调用改写线程错误码。
        let error = unsafe { GetLastError() };
        if handle.is_null() {
            return Err(io::Error::from_raw_os_error(error as i32));
        }
        if error == ERROR_ALREADY_EXISTS {
            // SAFETY: handle 有效且仅此处关闭。
            unsafe { CloseHandle(handle) };
            return Ok(None);
        }
        Ok(Some(Mutex(handle)))
    }
}

impl Drop for Mutex {
    fn drop(&mut self) {
        // SAFETY: 句柄由 create 获得，只关闭一次。
        unsafe { CloseHandle(self.0) };
    }
}

fn create_instance(path: &[u16], first: bool) -> io::Result<File> {
    let open_mode = PIPE_ACCESS_DUPLEX
        | if first {
            FILE_FLAG_FIRST_PIPE_INSTANCE
        } else {
            0
        };
    // SAFETY: path 以 0 结尾；同步（非重叠）句柄；默认安全描述符。
    let handle = unsafe {
        CreateNamedPipeW(
            path.as_ptr(),
            open_mode,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            PIPE_UNLIMITED_INSTANCES,
            4096,
            4096,
            0,
            std::ptr::null(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: 新建且独占的句柄，交给 File 负责关闭。
    Ok(unsafe { File::from_raw_handle(handle as _) })
}

/// 阻塞直到有客户端连上该管道实例。
fn accept(pipe: &File) -> io::Result<()> {
    // SAFETY: 句柄由 pipe 拥有且同步打开，lpOverlapped 为空。
    let ok = unsafe { ConnectNamedPipe(pipe.as_raw_handle() as HANDLE, std::ptr::null_mut()) };
    // 客户端在 CreateNamedPipe 与 ConnectNamedPipe 之间连入时返回 ERROR_PIPE_CONNECTED。
    if ok != 0 || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn serve(name: &str, handler: Handler) -> io::Result<()> {
    let path = wide(&pipe_path(name));
    // 首个实例同步创建：FILE_FLAG_FIRST_PIPE_INSTANCE 保证管道名不被其他进程抢注。
    let mut pending = Some(create_instance(&path, true)?);
    std::thread::Builder::new()
        .name("matcha-instance-listener".into())
        .spawn(move || loop {
            let pipe = match pending.take() {
                Some(pipe) => pipe,
                None => match create_instance(&path, false) {
                    Ok(pipe) => pipe,
                    Err(_) => {
                        std::thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                },
            };
            if accept(&pipe).is_err() {
                continue;
            }
            // 先补建下一个实例再处理本连接，缩短客户端看到“无管道”的空窗。
            pending = create_instance(&path, false).ok();
            let handler = handler.clone();
            let _ = std::thread::Builder::new()
                .name("matcha-instance-connection".into())
                .spawn(move || crate::handle_connection(pipe, &*handler));
        })?;
    Ok(())
}

pub fn connect(name: &str, timeout: Duration) -> io::Result<File> {
    let path = pipe_path(name);
    let deadline = Instant::now() + timeout;
    loop {
        match OpenOptions::new().read(true).write(true).open(&path) {
            Ok(pipe) => {
                allow_server_foreground(&pipe);
                return Ok(pipe);
            }
            Err(e)
                if matches!(
                    e.raw_os_error().map(|c| c as u32),
                    Some(ERROR_FILE_NOT_FOUND | ERROR_PIPE_BUSY)
                ) && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => return Err(e),
        }
    }
}

/// 前台进程（刚被用户启动的本进程）把前台权交给常驻实例，使其能把窗口提到最前；
/// 否则 SetForegroundWindow 只会让任务栏按钮闪烁。
fn allow_server_foreground(pipe: &File) {
    let mut pid = 0u32;
    // SAFETY: 句柄由 pipe 拥有；pid 为有效输出指针。
    unsafe {
        if GetNamedPipeServerProcessId(pipe.as_raw_handle() as HANDLE, &mut pid) != 0 {
            AllowSetForegroundWindow(pid);
        }
    }
}
