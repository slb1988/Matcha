//! VFS path – scheme + provider-specific path string.
//!
//! BC stores remote locations as `vfs://` style URLs in session specs and plain paths for
//! local files. `VfsPath` keeps both representations behind one type.

use std::path::{Path, PathBuf};

/// A location inside some file-system provider.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VfsPath {
    scheme: String,
    /// Scheme-specific remainder (local: an OS path; ftp: `user@host:port/dir`).
    path: String,
}

impl VfsPath {
    /// Local file-system path (`file` scheme).
    pub fn local(path: &Path) -> Self {
        Self { scheme: "file".into(), path: path.to_string_lossy().into_owned() }
    }

    /// Parse either a bare local path or a `scheme://…` URL.
    pub fn parse(spec: &str) -> Self {
        todo!("bc-vfs::path: parse spec (local path vs scheme URL, UNC, drive letters)")
    }

    pub fn scheme(&self) -> &str {
        &self.scheme
    }

    /// Raw scheme-specific path.
    pub fn as_str(&self) -> &str {
        &self.path
    }

    pub fn is_local(&self) -> bool {
        self.scheme == "file"
    }

    /// Local `PathBuf` when this is a `file` path.
    pub fn to_local_path(&self) -> Option<PathBuf> {
        self.is_local().then(|| PathBuf::from(&self.path))
    }

    /// Append a child name.
    pub fn join(&self, name: &str) -> VfsPath {
        todo!("bc-vfs::path: join with provider separator rules")
    }

    /// Parent location, if any.
    pub fn parent(&self) -> Option<VfsPath> {
        todo!("bc-vfs::path: parent")
    }

    /// Final path component.
    pub fn file_name(&self) -> Option<&str> {
        todo!("bc-vfs::path: file name")
    }

    /// Display form used in session specs (`C:\dir` or `ftp://host/dir`).
    pub fn to_spec_string(&self) -> String {
        if self.is_local() {
            self.path.clone()
        } else {
            format!("{}://{}", self.scheme, self.path)
        }
    }
}

impl std::fmt::Display for VfsPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_spec_string())
    }
}
