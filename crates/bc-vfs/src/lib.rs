//! # bc-vfs
//!
//! Unified file-system abstraction – BC `Vfs*` units (`TVfsManager`, `TVfsResource`,
//! `TVfsLocalResource`, `TVfsEntry`, `TVfs*Job`, `TVfsChangeNotifier`).
//!
//! Milestone 1 ships only [`local::LocalFs`]. FTP/SFTP/WebDAV/S3/archives/snapshots
//! (`TVfsFtp*`, `TVfsArchiveResource`, `TVfsRarResource`, …) are out of scope: they will
//! be additional [`Vfs`] implementations registered with [`VfsManager`].
//!
//! Design notes
//! * Synchronous API; concurrency and cancellation are the job framework's business
//!   (bc-jobs wraps each call in a `TVfs*Job` equivalent). This keeps bc-vfs free of
//!   any threading dependency.
//! * Paths are [`VfsPath`]s: a scheme plus a scheme-specific string, mirroring BC's
//!   `vfs://` URLs in `BCSessions.xml` while keeping local paths as `std::path`.
#![deny(unsafe_code)]

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;

pub mod entry;
mod exchange;
pub mod local;
pub mod manager;
pub mod path;
#[cfg(not(windows))]
mod poll_watch;

pub use entry::{ChangeEvent, EntryKind, FileAttrs, VfsEntry};
pub use local::{CopyOptions, LocalFs};
pub use manager::VfsManager;
pub use path::VfsPath;

use std::io::Read;
use std::path::Path;
use std::time::SystemTime;

/// VFS error – maps onto the `TOp*` error dialogs on the UI side.
#[derive(Debug)]
pub enum VfsError {
    Cancelled,
    NotFound(VfsPath),
    AlreadyExists(VfsPath),
    PermissionDenied(VfsPath),
    /// Operation not available on this provider (e.g. recycle bin on FTP).
    Unsupported(&'static str),
    /// Scheme has no registered provider.
    NoProvider(String),
    Io(std::io::Error),
    Other(String),
}

impl std::fmt::Display for VfsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VfsError::Cancelled => f.write_str("cancelled"),
            VfsError::NotFound(p) => write!(f, "not found: {p}"),
            VfsError::AlreadyExists(p) => write!(f, "already exists: {p}"),
            VfsError::PermissionDenied(p) => write!(f, "permission denied: {p}"),
            VfsError::Unsupported(what) => write!(f, "unsupported: {what}"),
            VfsError::NoProvider(s) => write!(f, "no provider for scheme {s:?}"),
            VfsError::Io(e) => write!(f, "io: {e}"),
            VfsError::Other(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for VfsError {}

impl From<std::io::Error> for VfsError {
    fn from(e: std::io::Error) -> Self {
        VfsError::Io(e)
    }
}

pub type VfsResult<T> = Result<T, VfsError>;

/// Change notification source – BC `TVfsChangeNotifier` (local: `ReadDirectoryChangesW`).
pub trait Watcher: Send {
    /// Drain pending change events (non-blocking).
    fn poll(&mut self) -> Vec<ChangeEvent>;
}

/// A file-system provider – BC `TVfsResource` (one per scheme/connection).
///
/// Every method is a direct counterpart of a `TVfs*Job` class, noted per method.
pub trait Vfs: Send + Sync {
    /// URL scheme handled (`file`, `ftp`, `sftp`, `zip`, `bcss`, …).
    fn scheme(&self) -> &'static str;

    /// Directory listing (non-recursive) – `TVfsLoadDirJob` / `TVfsEnumChildrenJob`.
    fn list(&self, dir: &VfsPath) -> VfsResult<Vec<VfsEntry>>;

    /// Metadata of one entry – `TVfsGetEntryJob`.
    fn metadata(&self, path: &VfsPath) -> VfsResult<VfsEntry>;

    /// Open for sequential reading – `TVfsGetReadFileStreamJob`.
    fn open_read(&self, path: &VfsPath) -> VfsResult<Box<dyn Read + Send>>;

    /// Download `src` to a local file – `TVfsGetFileJob` (local→local = copy).
    fn get(&self, src: &VfsPath, local_dest: &Path) -> VfsResult<()>;

    /// Upload a local file to `dest` – `TVfsPutFileJob` / `TVfsPutLocalFileJob`.
    fn put(&self, local_src: &Path, dest: &VfsPath) -> VfsResult<()>;

    /// Delete file or directory tree – `TVfsDeleteFileJob` / `TVfsRemoveDirJob`.
    /// `recycle_bin` honours the script `delete recyclebin=yes` option where supported.
    fn delete(&self, target: &VfsPath, recycle_bin: bool) -> VfsResult<()>;

    /// Rename/move within the same provider – `TVfsRenameJob` / `TVfsMoveToJob`.
    fn rename(&self, from: &VfsPath, to: &VfsPath) -> VfsResult<()>;

    /// Create a directory (and parents) – `TVfsMakeDirJob` / `TVfsForceDirsJob`.
    fn make_dir(&self, dir: &VfsPath) -> VfsResult<()>;

    /// CRC32 of a file's contents – `TVfsGetCRC32Job` (folder compare `criteria CRC`).
    fn crc32(&self, file: &VfsPath) -> VfsResult<u32>;

    /// Set last-modified time – `TVfsSetLastModifiedJob` (script `touch`).
    fn set_modified(&self, path: &VfsPath, when: SystemTime) -> VfsResult<()>;

    /// Set DOS attributes / unix mode – `TVfsSetDOSAttrJob` / `TVfsSetUnixOwnerAndModeJob`
    /// (script `attrib`).
    fn set_attrs(&self, path: &VfsPath, attrs: FileAttrs) -> VfsResult<()>;

    /// Atomic, cancellable single-file transfer. Progress reports byte deltas.
    /// Providers must not recurse here: operation planning owns filtered traversal.
    fn copy_file(
        &self,
        _from: &VfsPath,
        _to: &VfsPath,
        _options: CopyOptions,
        _cancel: &dyn bc_core::cancel::CancelCheck,
        _progress: &mut dyn FnMut(u64),
    ) -> VfsResult<()> {
        Err(VfsError::Unsupported("cancellable file copy"))
    }
    fn move_file(
        &self,
        _from: &VfsPath,
        _to: &VfsPath,
        _options: CopyOptions,
        _cancel: &dyn bc_core::cancel::CancelCheck,
        _progress: &mut dyn FnMut(u64),
    ) -> VfsResult<()> {
        Err(VfsError::Unsupported("cancellable file move"))
    }
    fn exchange_files(
        &self,
        _left: &VfsPath,
        _right: &VfsPath,
        _cancel: &dyn bc_core::cancel::CancelCheck,
        _progress: &mut dyn FnMut(u64),
    ) -> VfsResult<()> {
        Err(VfsError::Unsupported("file exchange"))
    }
    /// Copy timestamps/attributes/optional creation time without copying data.
    fn copy_metadata(
        &self,
        _from: &VfsPath,
        _to: &VfsPath,
        _options: CopyOptions,
    ) -> VfsResult<()> {
        Err(VfsError::Unsupported("metadata preservation"))
    }
    /// Never recursively delete newly appeared or filtered children.
    fn remove_empty_dir(&self, _path: &VfsPath) -> VfsResult<()> {
        Err(VfsError::Unsupported("remove empty directory"))
    }

    /// Watch a directory for changes – `TVfsChangeNotifier`.
    fn watch(&self, dir: &VfsPath) -> VfsResult<Box<dyn Watcher>>;
}
