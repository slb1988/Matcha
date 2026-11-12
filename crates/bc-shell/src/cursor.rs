//! Audited Win32 boundary: the desktop cursor position (`GetCursorPos`).
//!
//! winit's OLE drop target discards the drop point and the window receives no
//! `WM_MOUSEMOVE` while an Explorer drag is in progress, so the GUI's own pointer
//! position is stale at drop time; the OS cursor is the only reliable source.
use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

/// Cursor position in physical desktop pixels (the process is per-monitor DPI
/// aware under winit, so these match the window's physical coordinates).
pub fn screen_pos() -> Option<(i32, i32)> {
    let mut point = POINT { x: 0, y: 0 };
    // SAFETY: `point` is a valid, writable POINT for the duration of the call.
    let ok = unsafe { GetCursorPos(&mut point) };
    (ok != 0).then_some((point.x, point.y))
}
