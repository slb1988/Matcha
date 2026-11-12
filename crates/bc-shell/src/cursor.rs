//! Audited OS boundary: the desktop cursor position.
//!
//! winit discards the drop point and the window receives no mouse moves while
//! an OS file drag is in progress (Windows OLE drag; macOS dragging session),
//! so the GUI's own pointer position is stale at drop time; the OS cursor is
//! the only reliable source of the drop location.
//!
//! - Windows: `GetCursorPos` (physical pixels; the process is per-monitor DPI
//!   aware under winit, so these match the window's physical coordinates).
//! - macOS: `CGEventCreate` location (global display coordinates in points,
//!   top-left origin), scaled to physical pixels by the caller's
//!   pixels-per-point (the window's backing scale factor).

/// Cursor position in physical desktop pixels.
#[cfg(windows)]
pub fn screen_pos(_pixels_per_point: f64) -> Option<(i32, i32)> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let mut point = POINT { x: 0, y: 0 };
    // SAFETY: `point` is a valid, writable POINT for the duration of the call.
    let ok = unsafe { GetCursorPos(&mut point) };
    (ok != 0).then_some((point.x, point.y))
}

/// Cursor position in physical desktop pixels: CoreGraphics reports the cursor
/// in global display points (top-left origin), so they are converted with the
/// window's pixels-per-point to keep the physical-pixel contract.
#[cfg(target_os = "macos")]
pub fn screen_pos(pixels_per_point: f64) -> Option<(i32, i32)> {
    use core_graphics::event::CGEvent;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok()?;
    // `CGEventCreate` fills the event with the current mouse location.
    let event = CGEvent::new(source).ok()?;
    let point = event.location();
    let scale = if pixels_per_point.is_finite() && pixels_per_point > 0.0 {
        pixels_per_point
    } else {
        1.0
    };
    Some((
        (point.x * scale).round() as i32,
        (point.y * scale).round() as i32,
    ))
}
