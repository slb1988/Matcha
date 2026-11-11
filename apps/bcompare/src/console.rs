//! 输出通道。
//!
//! Windows 上 `matcha.exe` 是 GUI 子系统程序：双击、Explorer 菜单、VCS 启动都不弹控制台。
//! 代价是进程启动时没有控制台——从真实控制台启动时 std 句柄为 NULL（控制台句柄不会
//! 交给未附着的进程），只有被重定向到文件/管道的句柄才是有效的。因此：
//!
//! * [`attach`]：附着到父进程的控制台（若有）。`AttachConsole` 只填补 NULL 的 std 句柄，
//!   已重定向的保持不变，所以 `matcha.com … > out.txt` 仍写进文件。由控制台启动器
//!   `matcha.com` 启动（等待模式）时在入口立即附着：启动器在等，输出与退出码同步可见。
//! * [`tell`]：面向用户的结果（用法、参数错误、不支持的调用、致命错误）。先附着，
//!   仍无输出通道（Explorer/快捷方式启动）时改用消息框，不让错误静默丢失。
//!
//! 其余诊断（`eprintln!`）不主动附着：直接在交互式 cmd 里运行 `matcha.exe` 时，shell 不等
//! GUI 进程，后台日志不应在提示符之后冒出来。
//!
//! 其它平台（macOS/Linux）可执行文件没有子系统之分：终端启动即有 stdout/stderr，
//! Finder/Dock 启动时它们指向 `/dev/null` 或系统日志，两个函数都退化为直接打印。

/// 消息种类：决定输出流（stdout/stderr）与消息框图标。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Info,
    Error,
}

/// 附着到父进程的控制台（若有）；幂等，失败无副作用。
pub fn attach() {
    imp::attach();
}

/// 面向用户的消息：有控制台/重定向则打印，否则弹消息框（仅 Windows）。
pub fn tell(kind: Kind, text: &str) {
    attach();
    if imp::has_stream(kind) {
        match kind {
            Kind::Info => print!("{}", with_newline(text)),
            Kind::Error => eprint!("{}", with_newline(text)),
        }
    } else {
        imp::message_box(kind, text.trim_end());
    }
}

fn with_newline(text: &str) -> std::borrow::Cow<'_, str> {
    if text.ends_with('\n') {
        text.into()
    } else {
        format!("{text}\n").into()
    }
}

#[cfg(windows)]
mod imp {
    use super::Kind;
    use std::sync::Once;
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::System::Console::{
        AttachConsole, GetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE, STD_HANDLE,
        STD_OUTPUT_HANDLE,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONERROR, MB_ICONINFORMATION, MB_OK,
    };

    fn usable(which: STD_HANDLE) -> bool {
        // SAFETY: GetStdHandle 无指针参数，只读取进程参数块。
        let handle = unsafe { GetStdHandle(which) };
        !handle.is_null() && handle != INVALID_HANDLE_VALUE
    }

    pub fn attach() {
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            if usable(STD_OUTPUT_HANDLE) && usable(STD_ERROR_HANDLE) {
                return;
            }
            // SAFETY: 无指针参数；父进程没有控制台（或已附着）时返回 FALSE，不改变任何状态。
            unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
        });
    }

    pub fn has_stream(kind: Kind) -> bool {
        usable(match kind {
            Kind::Info => STD_OUTPUT_HANDLE,
            Kind::Error => STD_ERROR_HANDLE,
        })
    }

    pub fn message_box(kind: Kind, text: &str) {
        let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
        let (text, caption) = (wide(text), wide("Matcha"));
        let icon = match kind {
            Kind::Info => MB_ICONINFORMATION,
            Kind::Error => MB_ICONERROR,
        };
        // SAFETY: 两个缓冲区以 NUL 结尾且在调用期间存活；无属主窗口。
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                caption.as_ptr(),
                MB_OK | icon,
            )
        };
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Kind;

    pub fn attach() {}

    pub fn has_stream(_: Kind) -> bool {
        true
    }

    pub fn message_box(_: Kind, _: &str) {}
}
