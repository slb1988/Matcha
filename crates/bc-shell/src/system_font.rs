//! Audited Win32 boundary: the system message font and the file it lives in.
//!
//! - `SystemParametersInfoForDpi(SPI_GETNONCLIENTMETRICS, …, 96)` gives the
//!   message font's family and height at 96 DPI (the GUI scales by its own
//!   pixels-per-point, so the size must not include the system DPI).
//! - The family's file comes from the installed-fonts registry
//!   (`HKLM`, then the per-user `HKCU` key): the first regular entry (name
//!   without `Bold`/`Italic`) whose `" (TrueType)"`-stripped, `&`-separated
//!   names contain the family, compared case-insensitively. Relative values
//!   are under `%WINDIR%\Fonts`.
//!
//! Both are plain in-process calls (microseconds); they replace a hidden
//! PowerShell helper that compiled C# at start-up (~0.4 s warm, seconds cold).
use std::path::PathBuf;
use windows_sys::Win32::Foundation::{ERROR_MORE_DATA, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegEnumValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE,
    KEY_READ, REG_EXPAND_SZ, REG_SZ,
};
use windows_sys::Win32::UI::HiDpi::SystemParametersInfoForDpi;
use windows_sys::Win32::UI::WindowsAndMessaging::{NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS};

const FONTS_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts";

/// The message font: family, height in pixels at 96 DPI, and its font file.
#[derive(Clone, Debug, PartialEq)]
pub struct MessageFont {
    pub family: String,
    pub height_px: f32,
    pub path: PathBuf,
}

/// `None` when the metrics query fails or the family has no registered file.
pub fn message_font() -> Option<MessageFont> {
    let (family, height_px) = message_metrics()?;
    let path = font_file(&family)?;
    Some(MessageFont {
        family,
        height_px,
        path,
    })
}

fn message_metrics() -> Option<(String, f32)> {
    let mut metrics = NONCLIENTMETRICSW {
        cbSize: std::mem::size_of::<NONCLIENTMETRICSW>() as u32,
        ..Default::default()
    };
    // SAFETY: `metrics` is a writable NONCLIENTMETRICSW whose cbSize is its own
    // size, as SPI_GETNONCLIENTMETRICS requires; no profile update flags.
    let ok = unsafe {
        SystemParametersInfoForDpi(
            SPI_GETNONCLIENTMETRICS,
            metrics.cbSize,
            (&mut metrics as *mut NONCLIENTMETRICSW).cast(),
            0,
            96,
        )
    };
    if ok == 0 {
        return None;
    }
    let face = &metrics.lfMessageFont.lfFaceName;
    let len = face.iter().position(|&c| c == 0).unwrap_or(face.len());
    let family = String::from_utf16_lossy(&face[..len]);
    (!family.is_empty()).then(|| (family, metrics.lfMessageFont.lfHeight.unsigned_abs() as f32))
}

fn font_file(family: &str) -> Option<PathBuf> {
    let entry = [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER]
        .into_iter()
        .find_map(|root| {
            string_values(root, FONTS_KEY)
                .into_iter()
                .find(|(name, _)| entry_matches(name, family))
        })?;
    let path = PathBuf::from(entry.1);
    Some(if path.is_absolute() {
        path
    } else {
        windows_dir().join("Fonts").join(path)
    })
}

fn windows_dir() -> PathBuf {
    std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
}

/// Registry font entry names look like `Segoe UI (TrueType)` or
/// `Microsoft YaHei & Microsoft YaHei UI (TrueType)`.
fn entry_matches(name: &str, family: &str) -> bool {
    let lower = name.to_lowercase();
    if lower.contains("bold") || lower.contains("italic") {
        return false;
    }
    let names = name.strip_suffix(" (TrueType)").unwrap_or(name);
    names
        .split('&')
        .any(|part| part.trim().eq_ignore_ascii_case(family))
}

/// All string values of `root\path` (empty when the key is missing).
fn string_values(root: HKEY, path: &str) -> Vec<(String, String)> {
    struct Key(HKEY);
    impl Drop for Key {
        fn drop(&mut self) {
            // SAFETY: the handle came from a successful RegOpenKeyExW and is closed once.
            unsafe { RegCloseKey(self.0) };
        }
    }
    let wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    let mut raw: HKEY = std::ptr::null_mut();
    // SAFETY: `wide` is NUL-terminated; `raw` receives a handle only on success.
    if unsafe { RegOpenKeyExW(root, wide.as_ptr(), 0, KEY_READ, &mut raw) } != ERROR_SUCCESS {
        return Vec::new();
    }
    let key = Key(raw);
    let mut values = Vec::new();
    // Value names are at most 16383 characters; font file values are short paths.
    let mut name = vec![0u16; 16_384];
    let mut data = vec![0u16; 1024];
    let mut index = 0;
    loop {
        let mut name_len = name.len() as u32;
        let mut data_bytes = (data.len() * 2) as u32;
        let mut kind = 0u32;
        // SAFETY: the lengths describe the two buffers (characters / bytes).
        let status = unsafe {
            RegEnumValueW(
                key.0,
                index,
                name.as_mut_ptr(),
                &mut name_len,
                std::ptr::null(),
                &mut kind,
                data.as_mut_ptr().cast(),
                &mut data_bytes,
            )
        };
        if status == ERROR_MORE_DATA {
            // Data larger than the buffer: grow it and retry the same index.
            data = vec![0u16; (data_bytes as usize).div_ceil(2) + 1];
            continue;
        }
        if status != ERROR_SUCCESS {
            break;
        }
        index += 1;
        if kind != REG_SZ && kind != REG_EXPAND_SZ {
            continue;
        }
        let chars = &data[..(data_bytes as usize / 2).min(data.len())];
        let end = chars.iter().position(|&c| c == 0).unwrap_or(chars.len());
        values.push((
            String::from_utf16_lossy(&name[..name_len as usize]),
            String::from_utf16_lossy(&chars[..end]),
        ));
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_names_match_any_ampersand_member_but_not_styles() {
        let yahei = "Microsoft YaHei & Microsoft YaHei UI (TrueType)";
        assert!(entry_matches(yahei, "Microsoft YaHei UI"));
        assert!(entry_matches(yahei, "microsoft yahei"));
        assert!(entry_matches("Segoe UI (TrueType)", "Segoe UI"));
        assert!(!entry_matches("Segoe UI Bold (TrueType)", "Segoe UI"));
        assert!(!entry_matches("Segoe UI Italic (TrueType)", "Segoe UI"));
        assert!(!entry_matches("Segoe UI Semibold (TrueType)", "Segoe UI"));
    }

    #[test]
    fn the_running_system_has_a_registered_message_font() {
        let font = message_font().expect("message font");
        assert!(!font.family.is_empty());
        assert!((6.0..=96.0).contains(&font.height_px), "{font:?}");
        assert!(font.path.is_file(), "{font:?}");
    }
}
