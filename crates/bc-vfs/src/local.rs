//! Local file system provider – BC `TVfsLocalResource` / `TVfsLocalResourceFactory`.
//!
//! Scaffold: every method is `todo!()`. The implementation will be thin wrappers over
//! `std::fs` plus platform specifics (DOS attributes, recycle bin, junctions on Windows).

use crate::{FileAttrs, Vfs, VfsEntry, VfsPath, VfsResult, Watcher};
use std::io::Read;
use std::path::Path;
use std::time::SystemTime;

/// Local disk provider (`file` scheme).
#[derive(Debug, Default)]
pub struct LocalFs {
    /// Follow symlinks/junctions when listing (criteria `follow-symlinks`).
    pub follow_symlinks: bool,
}

impl LocalFs {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Vfs for LocalFs {
    fn scheme(&self) -> &'static str {
        "file"
    }

    fn list(&self, dir: &VfsPath) -> VfsResult<Vec<VfsEntry>> {
        todo!("bc-vfs::local: read_dir + metadata (DOS attrs, symlink handling)")
    }

    fn metadata(&self, path: &VfsPath) -> VfsResult<VfsEntry> {
        todo!("bc-vfs::local: metadata")
    }

    fn open_read(&self, path: &VfsPath) -> VfsResult<Box<dyn Read + Send>> {
        todo!("bc-vfs::local: open file")
    }

    fn get(&self, src: &VfsPath, local_dest: &Path) -> VfsResult<()> {
        todo!("bc-vfs::local: copy")
    }

    fn put(&self, local_src: &Path, dest: &VfsPath) -> VfsResult<()> {
        todo!("bc-vfs::local: copy (preserve timestamps/attrs per session options)")
    }

    fn delete(&self, target: &VfsPath, recycle_bin: bool) -> VfsResult<()> {
        todo!("bc-vfs::local: delete / recycle bin")
    }

    fn rename(&self, from: &VfsPath, to: &VfsPath) -> VfsResult<()> {
        todo!("bc-vfs::local: rename")
    }

    fn make_dir(&self, dir: &VfsPath) -> VfsResult<()> {
        todo!("bc-vfs::local: create_dir_all")
    }

    fn crc32(&self, file: &VfsPath) -> VfsResult<u32> {
        todo!("bc-vfs::local: streamed crc32 via bc-core::hash::Crc32 (chunked, early exit)")
    }

    fn set_modified(&self, path: &VfsPath, when: SystemTime) -> VfsResult<()> {
        todo!("bc-vfs::local: set mtime")
    }

    fn set_attrs(&self, path: &VfsPath, attrs: FileAttrs) -> VfsResult<()> {
        todo!("bc-vfs::local: set DOS attributes / unix mode")
    }

    fn watch(&self, dir: &VfsPath) -> VfsResult<Box<dyn Watcher>> {
        todo!("bc-vfs::local: directory change notifications")
    }
}
