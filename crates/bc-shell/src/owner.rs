//! Audited OS boundary: the owner window for native file dialogs.
//!
//! The GUI shows the system file/folder pickers (`IFileOpenDialog` via `rfd`)
//! synchronously from inside a frame. Without an owner the dialog is modeless:
//! it can fall behind the main window, which keeps taking clicks while the UI
//! thread is blocked. Owning it by this process's foreground window (the one
//! the user just clicked Browse in) makes it modal like any Explorer dialog.
use raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawWindowHandle,
    Win32WindowHandle, WindowHandle,
};
use std::num::NonZeroIsize;
use windows_sys::Win32::System::Threading::GetCurrentProcessId;
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

/// A top-level window of this process, usable as a dialog parent.
pub struct DialogOwner(NonZeroIsize);

/// This process's foreground window, if the foreground belongs to it.
pub fn foreground() -> Option<DialogOwner> {
    // SAFETY: plain queries; `pid` is a valid, writable u32 for the call.
    let (hwnd, pid) = unsafe {
        let hwnd = GetForegroundWindow();
        let mut pid = 0u32;
        if !hwnd.is_null() {
            GetWindowThreadProcessId(hwnd, &mut pid);
        }
        (hwnd, pid)
    };
    // SAFETY: no preconditions.
    let own = pid != 0 && pid == unsafe { GetCurrentProcessId() };
    own.then(|| NonZeroIsize::new(hwnd as isize))
        .flatten()
        .map(DialogOwner)
}

impl HasWindowHandle for DialogOwner {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let raw = RawWindowHandle::Win32(Win32WindowHandle::new(self.0));
        // SAFETY: the handle is a top-level window of this process, and the
        // caller blocks in the modal dialog on the UI thread that owns it, so
        // the window cannot be destroyed while the borrow is alive.
        Ok(unsafe { WindowHandle::borrow_raw(raw) })
    }
}

impl HasDisplayHandle for DialogOwner {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::windows())
    }
}
