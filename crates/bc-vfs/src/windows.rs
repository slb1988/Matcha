//! Audited Windows API boundary. All handles are owned by std::fs::File.
use crate::{ChangeEvent, FileAttrs, VfsError, VfsPath, VfsResult, Watcher};
use std::{
    fs::{File, OpenOptions},
    os::windows::{ffi::OsStrExt, fs::OpenOptionsExt, io::AsRawHandle},
    path::Path,
};
use windows_sys::Win32::{
    Foundation::*, Globalization::*, Storage::FileSystem::*, System::IO::*, UI::Shell::*,
};

fn wide(path: &Path) -> VfsResult<Vec<u16>> {
    let mut value: Vec<u16> = path.as_os_str().encode_wide().collect();
    if value.contains(&0) {
        return Err(VfsError::Other("NUL in path".into()));
    }
    value.push(0);
    Ok(value)
}

pub fn free_space(path: &Path) -> Option<u64> {
    let path = wide(path).ok()?;
    let mut free = 0u64;
    // SAFETY: `path` is NUL-terminated; the two unused totals may be null.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            path.as_ptr(),
            &mut free,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (ok != 0).then_some(free)
}

pub fn normalize_nfc(name: &str) -> VfsResult<String> {
    let source: Vec<u16> = name.encode_utf16().collect();
    if source.is_empty() {
        return Ok(String::new());
    }
    let len = i32::try_from(source.len()).map_err(|_| VfsError::Other("name too long".into()))?;
    // SAFETY: pointers reference counted UTF-16 buffers; first call requests required capacity.
    let size = unsafe {
        NormalizeString(
            NormalizationC,
            source.as_ptr(),
            len,
            std::ptr::null_mut(),
            0,
        )
    };
    if size <= 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let mut output = vec![0; size as usize];
    // SAFETY: destination capacity and both lengths match their allocations.
    let written = unsafe {
        NormalizeString(
            NormalizationC,
            source.as_ptr(),
            len,
            output.as_mut_ptr(),
            size,
        )
    };
    if written <= 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    String::from_utf16(&output[..written as usize]).map_err(|e| VfsError::Other(e.to_string()))
}

pub fn set_attrs(path: &Path, attrs: FileAttrs) -> VfsResult<()> {
    let path = wide(path)?;
    // SAFETY: nul-terminated path lives through both synchronous calls.
    let current = unsafe { GetFileAttributesW(path.as_ptr()) };
    if current == INVALID_FILE_ATTRIBUTES {
        return Err(std::io::Error::last_os_error().into());
    }
    let mut bits = current
        & !(FILE_ATTRIBUTE_READONLY
            | FILE_ATTRIBUTE_HIDDEN
            | FILE_ATTRIBUTE_SYSTEM
            | FILE_ATTRIBUTE_ARCHIVE
            | FILE_ATTRIBUTE_NORMAL);
    for (enabled, bit) in [
        (attrs.readonly, FILE_ATTRIBUTE_READONLY),
        (attrs.hidden, FILE_ATTRIBUTE_HIDDEN),
        (attrs.system, FILE_ATTRIBUTE_SYSTEM),
        (attrs.archive, FILE_ATTRIBUTE_ARCHIVE),
    ] {
        if enabled {
            bits |= bit;
        }
    }
    if bits == 0 {
        bits = FILE_ATTRIBUTE_NORMAL;
    }
    // SAFETY: same valid path; bits preserve non-DOS metadata flags.
    if unsafe { SetFileAttributesW(path.as_ptr(), bits) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

pub fn publish_no_replace(from: &Path, to: &Path) -> VfsResult<()> {
    let from = wide(from)?;
    let to = wide(to)?;
    // SAFETY: two owned nul-terminated paths, no retained pointers; flags=0 forbids replacement.
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

pub fn recycle(path: &Path) -> VfsResult<()> {
    let full = std::path::absolute(path)?;
    let display = full.to_string_lossy();
    if display.starts_with("\\\\") {
        return Err(VfsError::Unsupported("recycle bin on UNC paths"));
    }
    let root = full
        .ancestors()
        .last()
        .ok_or(VfsError::Unsupported("recycle volume"))?;
    let root = wide(root)?;
    // SAFETY: root is nul-terminated; call does not retain it.
    let drive = unsafe { GetDriveTypeW(root.as_ptr()) };
    const DRIVE_FIXED: u32 = 3; // winbase.h, not exposed under the selected feature set
    if drive != DRIVE_FIXED {
        return Err(VfsError::Unsupported("recycle bin on non-fixed drives"));
    }
    let mut from = wide(&full)?;
    from.push(0); // SHFileOperation takes a double-NUL list.
    let mut op = SHFILEOPSTRUCTW {
        wFunc: FO_DELETE,
        pFrom: from.as_ptr(),
        fFlags: (FOF_ALLOWUNDO
            | FOF_NOCONFIRMATION
            | FOF_NOERRORUI
            | FOF_SILENT
            | FOF_WANTNUKEWARNING) as u16,
        ..Default::default()
    };
    // SAFETY: the owned path list and operation structure outlive the synchronous shell call.
    let code = unsafe { SHFileOperationW(&mut op) };
    if op.fAnyOperationsAborted != 0 {
        return Err(VfsError::Unsupported(
            "recycle declined or unavailable; permanent deletion requires separate confirmation",
        ));
    }
    if code != 0 {
        return Err(VfsError::Other(format!(
            "recycle failed (shell code {code}): {}",
            std::io::Error::from_raw_os_error(code)
        )));
    }
    Ok(())
}

pub struct LocalWatcher {
    dir: VfsPath,
    file: File,
    // Heap allocations cannot move while an overlapped request owns their addresses.
    overlapped: Box<OVERLAPPED>,
    buffer: Vec<u32>,
    pending: bool,
    old_name: Option<VfsPath>,
}
// SAFETY: File is Send; the API references only stable heap allocations, never self.
// No field is shared concurrently and poll/drop require exclusive ownership.
unsafe impl Send for LocalWatcher {}

impl LocalWatcher {
    pub fn new(dir: &VfsPath) -> VfsResult<Self> {
        let file = OpenOptions::new()
            .read(true)
            .access_mode(FILE_LIST_DIRECTORY)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED)
            .open(crate::local::local(dir)?)?;
        let mut watcher = Self {
            dir: dir.clone(),
            file,
            overlapped: Box::default(),
            buffer: vec![0; 16384],
            pending: false,
            old_name: None,
        };
        watcher.arm()?;
        Ok(watcher)
    }
    fn arm(&mut self) -> VfsResult<()> {
        *self.overlapped = OVERLAPPED::default();
        // SAFETY: live directory handle, stable aligned buffer and OVERLAPPED are retained
        // until completion (including in Drop); no completion routine is installed.
        let ok = unsafe {
            ReadDirectoryChangesW(
                self.file.as_raw_handle(),
                self.buffer.as_mut_ptr().cast(),
                (self.buffer.len() * 4) as u32,
                1,
                FILE_NOTIFY_CHANGE_FILE_NAME
                    | FILE_NOTIFY_CHANGE_DIR_NAME
                    | FILE_NOTIFY_CHANGE_SIZE
                    | FILE_NOTIFY_CHANGE_LAST_WRITE
                    | FILE_NOTIFY_CHANGE_ATTRIBUTES,
                std::ptr::null_mut(),
                &mut *self.overlapped,
                None,
            )
        };
        if ok == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        self.pending = true;
        Ok(())
    }
}

impl Watcher for LocalWatcher {
    fn poll(&mut self) -> Vec<ChangeEvent> {
        if !self.pending {
            return vec![ChangeEvent::Overflow];
        }
        let mut size = 0;
        // SAFETY: querying the exact live request with nonblocking wait.
        if unsafe {
            GetOverlappedResult(self.file.as_raw_handle(), &*self.overlapped, &mut size, 0)
        } == 0
        {
            if std::io::Error::last_os_error().raw_os_error() == Some(ERROR_IO_INCOMPLETE as i32) {
                return vec![];
            }
            self.pending = false;
            let _ = self.arm();
            return vec![ChangeEvent::Overflow];
        }
        self.pending = false;
        let mut events = Vec::new();
        if size == 0 {
            events.push(ChangeEvent::Overflow);
        }
        // SAFETY: completed request wrote at most the supplied buffer size.
        let bytes = unsafe {
            std::slice::from_raw_parts(
                self.buffer.as_ptr().cast::<u8>(),
                (size as usize).min(self.buffer.len() * 4),
            )
        };
        let mut offset = 0;
        while offset + 12 <= bytes.len() {
            let number =
                |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
            let next = number(offset);
            let action = number(offset + 4) as u32;
            let len = number(offset + 8);
            if len % 2 != 0 || offset + 12 + len > bytes.len() {
                events.push(ChangeEvent::Overflow);
                break;
            }
            let name: Vec<u16> = bytes[offset + 12..offset + 12 + len]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|v| u16::from_le_bytes(*v))
                .collect();
            let path = self.dir.join(&String::from_utf16_lossy(&name));
            match action {
                FILE_ACTION_ADDED => events.push(ChangeEvent::Created(path)),
                FILE_ACTION_REMOVED => events.push(ChangeEvent::Deleted(path)),
                FILE_ACTION_MODIFIED => events.push(ChangeEvent::Modified(path)),
                FILE_ACTION_RENAMED_OLD_NAME => self.old_name = Some(path),
                FILE_ACTION_RENAMED_NEW_NAME => {
                    if let Some(from) = self.old_name.take() {
                        events.push(ChangeEvent::Renamed { from, to: path });
                    } else {
                        events.push(ChangeEvent::Created(path));
                    }
                }
                _ => events.push(ChangeEvent::Overflow),
            }
            if next == 0 {
                break;
            }
            if next < 12 || next > bytes.len() - offset {
                events.push(ChangeEvent::Overflow);
                break;
            }
            offset += next;
        }
        if self.arm().is_err() {
            events.push(ChangeEvent::Overflow);
        }
        events
    }
}

impl Drop for LocalWatcher {
    fn drop(&mut self) {
        if self.pending {
            let mut bytes = 0;
            // SAFETY: cancel only our request, then wait for completion before releasing buffers.
            unsafe {
                CancelIoEx(self.file.as_raw_handle(), &*self.overlapped);
                GetOverlappedResult(self.file.as_raw_handle(), &*self.overlapped, &mut bytes, 1);
            }
        }
    }
}
