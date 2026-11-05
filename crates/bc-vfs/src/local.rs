//! Streaming local I/O with atomic file publication and cooperative cancellation.
use crate::{EntryKind, FileAttrs, Vfs, VfsEntry, VfsError, VfsPath, VfsResult, Watcher};
use bc_core::{
    cancel::{CancelCheck, NeverCancel},
    hash::Crc32,
};
use std::{
    fs::{self, File, FileTimes, Metadata, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::SystemTime,
};

pub const CHUNK_SIZE: usize = 64 * 1024;

#[derive(Debug, Clone, Copy)]
pub struct CopyOptions {
    pub timestamps: bool,
    pub attributes: bool,
    pub created: bool,
    pub overwrite: bool,
}
impl Default for CopyOptions {
    fn default() -> Self {
        Self {
            timestamps: true,
            attributes: true,
            created: false,
            overwrite: false,
        }
    }
}

#[derive(Debug, Default)]
pub struct LocalFs {
    pub follow_symlinks: bool,
}

pub(crate) fn check(cancel: &dyn CancelCheck) -> VfsResult<()> {
    if cancel.is_cancelled() {
        Err(VfsError::Cancelled)
    } else {
        Ok(())
    }
}
pub(crate) fn local(path: &VfsPath) -> VfsResult<PathBuf> {
    path.to_local_path()
        .ok_or(VfsError::Unsupported("non-local path"))
}

fn open_metadata(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.access_mode(0x100).custom_flags(0x02000000); // FILE_WRITE_ATTRIBUTES, BACKUP_SEMANTICS
    }
    options.open(path)
}

fn attrs(meta: &Metadata) -> FileAttrs {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::*;
        let bits = meta.file_attributes();
        FileAttrs {
            readonly: bits & FILE_ATTRIBUTE_READONLY != 0,
            hidden: bits & FILE_ATTRIBUTE_HIDDEN != 0,
            system: bits & FILE_ATTRIBUTE_SYSTEM != 0,
            archive: bits & FILE_ATTRIBUTE_ARCHIVE != 0,
            unix_mode: None,
        }
    }
    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        FileAttrs {
            readonly: meta.permissions().readonly(),
            unix_mode: Some(meta.permissions().mode()),
            ..FileAttrs::default()
        }
    }
}

fn is_link(meta: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes()
            & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
            != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}

impl LocalFs {
    pub fn new() -> Self {
        Self::default()
    }

    /// Normalize a name using the operating system's canonical Unicode composition.
    /// Non-Windows callers must not silently claim normalization support.
    pub fn normalize_nfc(name: &str) -> VfsResult<String> {
        #[cfg(windows)]
        {
            crate::windows::normalize_nfc(name)
        }
        #[cfg(not(windows))]
        {
            let _ = name;
            Err(VfsError::Unsupported("NFC normalization on this platform"))
        }
    }

    fn entry(&self, path: &Path) -> VfsResult<VfsEntry> {
        let raw = fs::symlink_metadata(path)?;
        let link = is_link(&raw);
        let meta = if link && self.follow_symlinks {
            fs::metadata(path)?
        } else {
            raw
        };
        let kind = if link && !self.follow_symlinks {
            EntryKind::Symlink
        } else if meta.is_dir() {
            EntryKind::Dir
        } else {
            EntryKind::File
        };
        let error = if kind == EntryKind::Dir {
            fs::read_dir(path).err().map(|e| e.to_string())
        } else {
            None
        };
        Ok(VfsEntry {
            name: path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .into_owned(),
            kind,
            size: (kind == EntryKind::File).then_some(meta.len()),
            modified: meta.modified().ok(),
            attrs: attrs(&meta),
            version: None,
            crc32: None,
            error,
        })
    }

    pub fn get_cancellable(
        &self,
        src: &VfsPath,
        dest: &Path,
        options: CopyOptions,
        cancel: &dyn CancelCheck,
    ) -> VfsResult<()> {
        let src = local(src)?;
        // Reject copying a tree into itself, including aliases through existing symlink ancestors.
        let source = fs::canonicalize(&src)?;
        let destination = absolute_destination(dest)?;
        if source == destination || (source.is_dir() && destination.starts_with(&source)) {
            return Err(VfsError::Other("copy destination is inside source".into()));
        }
        self.copy_tree(&src, dest, options, cancel, &mut Vec::new())
    }

    pub fn put_cancellable(
        &self,
        src: &Path,
        dest: &VfsPath,
        options: CopyOptions,
        cancel: &dyn CancelCheck,
    ) -> VfsResult<()> {
        self.get_cancellable(&VfsPath::local(src), &local(dest)?, options, cancel)
    }

    fn copy_tree(
        &self,
        src: &Path,
        dest: &Path,
        options: CopyOptions,
        cancel: &dyn CancelCheck,
        ancestors: &mut Vec<PathBuf>,
    ) -> VfsResult<()> {
        check(cancel)?;
        let raw = fs::symlink_metadata(src)?;
        if is_link(&raw) && !self.follow_symlinks {
            return Err(VfsError::Unsupported(
                "copying a link without follow-symlinks",
            ));
        }
        let meta = fs::metadata(src)?;
        if meta.is_dir() {
            let canonical = fs::canonicalize(src)?;
            if ancestors.contains(&canonical) {
                return Err(VfsError::Other("symlink cycle".into()));
            }
            ancestors.push(canonical);
            fs::create_dir_all(dest)?;
            let result = (|| {
                for child in fs::read_dir(src)? {
                    let child = child?;
                    self.copy_tree(
                        &child.path(),
                        &dest.join(child.file_name()),
                        options,
                        cancel,
                        ancestors,
                    )?;
                }
                self.preserve(dest, &meta, options)
            })();
            ancestors.pop();
            return result;
        }
        if !options.overwrite && dest.try_exists()? {
            return Err(VfsError::AlreadyExists(VfsPath::local(dest)));
        }
        if let Some(parent) = dest.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        let (tmp, mut output) = temporary_file(dest)?;
        let result = (|| {
            let mut input = File::open(src)?;
            let mut buf = vec![0; CHUNK_SIZE];
            loop {
                check(cancel)?;
                let n = input.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                output.write_all(&buf[..n])?;
            }
            output.sync_all()?;
            drop(output);
            self.preserve(&tmp, &meta, options)?;
            check(cancel)?;
            if options.overwrite {
                fs::rename(&tmp, dest)?;
            } else {
                // Atomic no-clobber publication; a racing destination cannot be overwritten.
                #[cfg(windows)]
                crate::windows::publish_no_replace(&tmp, dest)?;
                #[cfg(not(windows))]
                {
                    fs::hard_link(&tmp, dest)?;
                    fs::remove_file(&tmp)?;
                }
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = clear_readonly(&tmp);
            let _ = fs::remove_file(&tmp);
        }
        result
    }

    fn preserve(&self, dest: &Path, meta: &Metadata, options: CopyOptions) -> VfsResult<()> {
        if options.timestamps || options.created {
            let mut times = FileTimes::new();
            if options.timestamps {
                times = times.set_modified(meta.modified()?);
            }
            #[cfg(windows)]
            if options.created {
                use std::os::windows::fs::FileTimesExt;
                times = times.set_created(meta.created()?);
            }
            #[cfg(not(windows))]
            if options.created {
                return Err(VfsError::Unsupported("creation time"));
            }
            open_metadata(dest)?.set_times(times)?;
        }
        if options.attributes {
            self.set_attrs(&VfsPath::local(dest), attrs(meta))?;
        }
        Ok(())
    }

    pub fn rename_cancellable(
        &self,
        from: &VfsPath,
        to: &VfsPath,
        cancel: &dyn CancelCheck,
    ) -> VfsResult<()> {
        check(cancel)?;
        let src = local(from)?;
        let dest = local(to)?;
        match fs::rename(&src, &dest) {
            Ok(()) => Ok(()),
            Err(e) if cross_device(&e) => {
                self.get_cancellable(from, &dest, CopyOptions::default(), cancel)?;
                check(cancel)?;
                self.delete(from, false)
            }
            Err(e) => Err(e.into()),
        }
    }

    pub fn crc32_cancellable(&self, path: &VfsPath, cancel: &dyn CancelCheck) -> VfsResult<u32> {
        let mut input = self.open_read(path)?;
        let mut crc = Crc32::new();
        let mut buf = vec![0; CHUNK_SIZE];
        loop {
            check(cancel)?;
            let n = input.read(&mut buf)?;
            if n == 0 {
                break;
            }
            crc.update(&buf[..n]);
        }
        check(cancel)?;
        Ok(crc.finish())
    }
}

fn absolute_destination(path: &Path) -> std::io::Result<PathBuf> {
    if path.exists() {
        return fs::canonicalize(path);
    }
    let absolute = std::path::absolute(path)?;
    let mut ancestor = absolute.as_path();
    let mut tail = Vec::new();
    while !ancestor.exists() {
        tail.push(
            ancestor
                .file_name()
                .ok_or_else(|| std::io::Error::other("invalid destination"))?,
        );
        ancestor = ancestor
            .parent()
            .ok_or_else(|| std::io::Error::other("invalid destination"))?;
    }
    let mut result = fs::canonicalize(ancestor)?;
    for component in tail.into_iter().rev() {
        result.push(component);
    }
    Ok(result)
}

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);
fn temporary_file(dest: &Path) -> VfsResult<(PathBuf, File)> {
    for _ in 0..100 {
        let name = format!(
            ".matcha-copy-{}-{}.tmp",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        );
        let path = dest.with_file_name(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(VfsError::Other("cannot reserve copy temporary file".into()))
}

fn cross_device(error: &std::io::Error) -> bool {
    #[cfg(windows)]
    {
        error.raw_os_error() == Some(17)
    }
    #[cfg(not(windows))]
    {
        error.raw_os_error() == Some(18)
    }
}

fn clear_readonly(path: &Path) -> std::io::Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    #[cfg(windows)]
    #[allow(clippy::permissions_set_readonly_false)]
    // Windows only; Unix uses owner-write below.
    permissions.set_readonly(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(permissions.mode() | 0o200);
    }
    fs::set_permissions(path, permissions)
}

impl Vfs for LocalFs {
    fn scheme(&self) -> &'static str {
        "file"
    }
    fn list(&self, dir: &VfsPath) -> VfsResult<Vec<VfsEntry>> {
        let mut entries = Vec::new();
        for child in fs::read_dir(local(dir)?)? {
            let child = child?;
            match self.entry(&child.path()) {
                Ok(entry) => entries.push(entry),
                Err(error) => entries.push(VfsEntry {
                    name: child.file_name().to_string_lossy().into_owned(),
                    kind: if child.file_type().is_ok_and(|t| t.is_dir()) {
                        EntryKind::Dir
                    } else {
                        EntryKind::File
                    },
                    size: None,
                    modified: None,
                    attrs: FileAttrs::default(),
                    version: None,
                    crc32: None,
                    error: Some(error.to_string()),
                }),
            }
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }
    fn metadata(&self, path: &VfsPath) -> VfsResult<VfsEntry> {
        self.entry(&local(path)?)
    }
    fn open_read(&self, path: &VfsPath) -> VfsResult<Box<dyn Read + Send>> {
        Ok(Box::new(File::open(local(path)?)?))
    }
    fn get(&self, src: &VfsPath, dest: &Path) -> VfsResult<()> {
        self.get_cancellable(src, dest, CopyOptions::default(), &NeverCancel)
    }
    fn put(&self, src: &Path, dest: &VfsPath) -> VfsResult<()> {
        self.put_cancellable(src, dest, CopyOptions::default(), &NeverCancel)
    }
    fn delete(&self, target: &VfsPath, recycle_bin: bool) -> VfsResult<()> {
        let path = local(target)?;
        if recycle_bin {
            #[cfg(windows)]
            {
                return crate::windows::recycle(&path);
            }
            #[cfg(not(windows))]
            {
                return Err(VfsError::Unsupported("recycle bin"));
            }
        }
        let meta = fs::symlink_metadata(&path)?;
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if is_link(&meta)
                && meta.file_attributes()
                    & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_DIRECTORY
                    != 0
            {
                fs::remove_dir(&path)?;
                return Ok(());
            }
        }
        if meta.is_dir() {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
        Ok(())
    }
    fn rename(&self, from: &VfsPath, to: &VfsPath) -> VfsResult<()> {
        self.rename_cancellable(from, to, &NeverCancel)
    }
    fn make_dir(&self, dir: &VfsPath) -> VfsResult<()> {
        Ok(fs::create_dir_all(local(dir)?)?)
    }
    fn crc32(&self, file: &VfsPath) -> VfsResult<u32> {
        self.crc32_cancellable(file, &NeverCancel)
    }
    fn set_modified(&self, path: &VfsPath, when: SystemTime) -> VfsResult<()> {
        Ok(open_metadata(&local(path)?)?.set_times(FileTimes::new().set_modified(when))?)
    }
    fn set_attrs(&self, path: &VfsPath, attrs: FileAttrs) -> VfsResult<()> {
        let path = local(path)?;
        #[cfg(windows)]
        {
            crate::windows::set_attrs(&path, attrs)
        }
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;
            if attrs.hidden || attrs.system || attrs.archive {
                return Err(VfsError::Unsupported("DOS attributes"));
            }
            if let Some(mode) = attrs.unix_mode {
                fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
            } else if attrs.readonly {
                let mut p = fs::metadata(&path)?.permissions();
                p.set_mode(p.mode() & !0o222);
                fs::set_permissions(path, p)?;
            } else {
                clear_readonly(&path)?;
            }
            Ok(())
        }
    }
    fn watch(&self, dir: &VfsPath) -> VfsResult<Box<dyn Watcher>> {
        #[cfg(windows)]
        {
            Ok(Box::new(crate::windows::LocalWatcher::new(dir)?))
        }
        #[cfg(not(windows))]
        {
            Ok(Box::new(crate::poll_watch::PollWatcher::new(dir)?))
        }
    }
}
