//! Audited Win32 boundary: `HKEY_CURRENT_USER` string values. Every handle is
//! owned by [`Key`] and closed on drop; strings cross as NUL-terminated UTF-16.
use crate::registry::Registry;
use std::io;
use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteKeyValueW, RegDeleteKeyW, RegDeleteTreeW, RegGetValueW,
    RegOpenKeyExW, RegQueryInfoKeyW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE,
    KEY_READ, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RRF_RT_REG_SZ,
};

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn check(status: WIN32_ERROR) -> io::Result<()> {
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(status as i32))
    }
}

/// `Ok(false)` for "not found", so removals of missing items succeed.
fn found(status: WIN32_ERROR) -> io::Result<bool> {
    if status == ERROR_FILE_NOT_FOUND {
        Ok(false)
    } else {
        check(status).map(|()| true)
    }
}

/// An open key handle.
struct Key(HKEY);

impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: the handle came from RegOpenKeyExW/RegCreateKeyExW and is closed once.
        unsafe { RegCloseKey(self.0) };
    }
}

impl Key {
    fn open(path: &str, access: u32) -> io::Result<Option<Key>> {
        let path = wide(path);
        let mut key: HKEY = std::ptr::null_mut();
        // SAFETY: path is NUL-terminated; `key` receives a handle only on success.
        let status =
            unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, access, &mut key) };
        Ok(found(status)?.then_some(Key(key)))
    }

    fn create(path: &str) -> io::Result<Key> {
        let path = wide(path);
        let mut key: HKEY = std::ptr::null_mut();
        // SAFETY: path is NUL-terminated; no class, default security, no disposition.
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                path.as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE | KEY_QUERY_VALUE,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            )
        };
        check(status)?;
        Ok(Key(key))
    }
}

/// The current user's hive.
#[derive(Debug, Default, Clone, Copy)]
pub struct Hkcu;

impl Registry for Hkcu {
    fn get(&self, key: &str, name: &str) -> io::Result<Option<String>> {
        let (path, name) = (wide(key), wide(name));
        let mut bytes: u32 = 0;
        // SAFETY: size query with a null buffer; strings are NUL-terminated.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                path.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut bytes,
            )
        };
        if !found(status)? {
            return Ok(None);
        }
        // The value may grow between the two calls; retry a few times.
        for _ in 0..4 {
            let mut buffer = vec![0u16; (bytes as usize).div_ceil(2) + 1];
            let mut size = (buffer.len() * 2) as u32;
            // SAFETY: `size` is the buffer's length in bytes.
            let status = unsafe {
                RegGetValueW(
                    HKEY_CURRENT_USER,
                    path.as_ptr(),
                    name.as_ptr(),
                    RRF_RT_REG_SZ,
                    std::ptr::null_mut(),
                    buffer.as_mut_ptr().cast(),
                    &mut size,
                )
            };
            if status == windows_sys::Win32::Foundation::ERROR_MORE_DATA {
                bytes = size;
                continue;
            }
            if !found(status)? {
                return Ok(None);
            }
            let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
            return Ok(Some(String::from_utf16_lossy(&buffer[..len])));
        }
        Err(io::Error::other("registry value kept changing size"))
    }

    fn exists(&self, key: &str) -> io::Result<bool> {
        Ok(Key::open(key, KEY_READ)?.is_some())
    }

    fn set(&mut self, key: &str, name: &str, value: &str) -> io::Result<()> {
        let handle = Key::create(key)?;
        let (name, data) = (wide(name), wide(value));
        // SAFETY: REG_SZ data is the NUL-terminated UTF-16 string, size in bytes.
        let status = unsafe {
            RegSetValueExW(
                handle.0,
                name.as_ptr(),
                0,
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * 2) as u32,
            )
        };
        check(status)
    }

    fn remove_value(&mut self, key: &str, name: &str) -> io::Result<()> {
        let (path, name) = (wide(key), wide(name));
        // SAFETY: both strings are NUL-terminated.
        let status = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, path.as_ptr(), name.as_ptr()) };
        found(status).map(drop)
    }

    fn remove_tree(&mut self, key: &str) -> io::Result<()> {
        let path = wide(key);
        // SAFETY: path is NUL-terminated. RegDeleteTreeW removes the subkeys and
        // values; RegDeleteKeyW then removes the (now empty) key itself.
        let status = unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, path.as_ptr()) };
        if !found(status)? {
            return Ok(());
        }
        let status = unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, path.as_ptr()) };
        found(status).map(drop)
    }

    fn remove_if_empty(&mut self, key: &str) -> io::Result<()> {
        let Some(handle) = Key::open(key, KEY_READ)? else {
            return Ok(());
        };
        let (mut subkeys, mut values) = (0u32, 0u32);
        // SAFETY: only the two counts are requested; every other out-pointer is null.
        let status = unsafe {
            RegQueryInfoKeyW(
                handle.0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
                &mut subkeys,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut values,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        check(status)?;
        drop(handle);
        if subkeys == 0 && values == 0 {
            let path = wide(key);
            // SAFETY: path is NUL-terminated; fails harmlessly if the key gained children.
            let status = unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, path.as_ptr()) };
            found(status)?;
        }
        Ok(())
    }
}
