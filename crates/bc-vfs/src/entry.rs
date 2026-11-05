//! Directory entry model – BC `TVfsEntry` / `TVfsEntryList` / `TVfsEntryExList`.

use crate::VfsPath;
use std::time::SystemTime;

/// Entry type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EntryKind {
    File,
    Dir,
    /// Symbolic link or NTFS junction (criteria `follow-symlinks` decides treatment).
    Symlink,
    /// Archive that can be opened as a folder (`TVfsArchiveResource`), M3+.
    Archive,
}

/// DOS attributes + unix mode – what `attrib` / `criteria attrib:` operate on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FileAttrs {
    pub readonly: bool,
    pub hidden: bool,
    pub system: bool,
    pub archive: bool,
    /// Unix permission bits when available.
    pub unix_mode: Option<u32>,
}

impl FileAttrs {
    /// Parse a BC attribute set string: any of `a`, `s`, `h`, `r` in any order.
    pub fn parse_set(set: &str) -> Option<FileAttrs> {
        let mut attrs = Self::default();
        for c in set.chars() {
            match c.to_ascii_lowercase() {
                'a' => attrs.archive = true,
                's' => attrs.system = true,
                'h' => attrs.hidden = true,
                'r' => attrs.readonly = true,
                _ => return None,
            }
        }
        Some(attrs)
    }
}

/// One file-system entry as listed by [`crate::Vfs::list`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VfsEntry {
    pub name: String,
    pub kind: EntryKind,
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
    pub attrs: FileAttrs,
    /// PE version string when `criteria version` is enabled (`TVfsGetExeVersionJob`).
    pub version: Option<String>,
    /// Cached CRC when already computed (snapshots store it: `snapshot save-crc`).
    pub crc32: Option<u32>,
    /// Per-entry failure; siblings remain usable when a child cannot be read.
    pub error: Option<String>,
}

/// File-system change notification – `TVfsChangeNotifier` payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeEvent {
    Created(VfsPath),
    Modified(VfsPath),
    Deleted(VfsPath),
    Renamed {
        from: VfsPath,
        to: VfsPath,
    },
    /// Too many changes – caller should rescan.
    Overflow,
}
