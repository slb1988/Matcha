//! Provider paths retain their original display spelling; local I/O uses std::path.
use crate::{VfsError, VfsResult};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VfsPath {
    scheme: String,
    path: String,
    explicit_scheme: bool,
}

impl VfsPath {
    pub fn local(path: &Path) -> Self {
        Self {
            scheme: "file".into(),
            path: path.to_string_lossy().into_owned(),
            explicit_scheme: false,
        }
    }

    pub fn parse(spec: &str) -> VfsResult<Self> {
        if spec.is_empty() || spec.contains('\0') || spec.contains(['\r', '\n']) {
            return Err(VfsError::Other(
                "empty path or control character in path".into(),
            ));
        }
        if let Some((scheme, path)) = spec.split_once("://") {
            if scheme.is_empty()
                || !scheme.starts_with(|c: char| c.is_ascii_alphabetic())
                || !scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
                || path.is_empty()
            {
                return Err(VfsError::Other("invalid provider URL".into()));
            }
            Ok(Self {
                scheme: scheme.into(),
                path: path.into(),
                explicit_scheme: true,
            })
        } else {
            Ok(Self::local(Path::new(spec)))
        }
    }

    pub fn scheme(&self) -> &str {
        &self.scheme
    }
    pub fn as_str(&self) -> &str {
        &self.path
    }
    pub fn is_local(&self) -> bool {
        self.scheme.eq_ignore_ascii_case("file")
    }

    pub fn to_local_path(&self) -> Option<PathBuf> {
        if !self.is_local() {
            return None;
        }
        let p = &self.path;
        if !self.explicit_scheme {
            return Some(PathBuf::from(p));
        }
        #[cfg(windows)]
        {
            if p.starts_with('/') && p.as_bytes().get(2) == Some(&b':') {
                return Some(PathBuf::from(&p[1..]));
            }
            if !p.starts_with('/') && p.as_bytes().get(1) != Some(&b':') && !p.starts_with("\\\\") {
                return Some(PathBuf::from(format!("\\\\{}", p.replace('/', "\\"))));
            }
        }
        Some(PathBuf::from(p))
    }

    pub fn join(&self, name: &str) -> Self {
        let separator = if self.path.contains('\\') { '\\' } else { '/' };
        let path = if self.path.ends_with(['/', '\\']) {
            format!("{}{name}", self.path)
        } else {
            format!("{}{separator}{name}", self.path)
        };
        Self {
            path,
            ..self.clone()
        }
    }

    pub fn parent(&self) -> Option<Self> {
        let p = self.path.trim_end_matches(['/', '\\']);
        let i = p.rfind(['/', '\\'])?;
        let end = if i == 0 || (i == 2 && p.as_bytes().get(1) == Some(&b':')) {
            i + 1
        } else {
            i
        };
        // Do not ascend above an UNC share or a URL authority.
        if p.starts_with("\\\\") && p[2..].split('\\').count() <= 2 {
            return None;
        }
        if end == p.len() {
            return None;
        }
        Some(Self {
            path: p[..end].into(),
            ..self.clone()
        })
    }

    pub fn file_name(&self) -> Option<&str> {
        let p = self.path.trim_end_matches(['/', '\\']);
        if p.is_empty() || (p.len() == 2 && p.ends_with(':')) {
            return None;
        }
        p.rsplit(['/', '\\']).next()
    }

    pub fn to_spec_string(&self) -> String {
        if self.explicit_scheme {
            format!("{}://{}", self.scheme, self.path)
        } else {
            self.path.clone()
        }
    }
}

impl std::fmt::Display for VfsPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_spec_string())
    }
}
