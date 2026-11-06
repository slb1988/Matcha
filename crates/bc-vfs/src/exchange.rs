//! Two-file exchange stages both copies before publication. Recovery never discards backups.
use crate::{CopyOptions, LocalFs, Vfs, VfsError, VfsPath, VfsResult};
use bc_core::cancel::CancelCheck;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn reserve(path: &Path) -> VfsResult<PathBuf> {
    for _ in 0..100 {
        let temp = path.with_file_name(format!(
            ".matcha-exchange-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::create_dir(&temp) {
            Ok(()) => return Ok(temp),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Err(VfsError::Other("cannot reserve exchange directory".into()))
}
fn cleanup(path: &Path) {
    for name in ["staged", "backup"] {
        let file = path.join(name);
        if file.exists() {
            let _ = LocalFs::new().set_attrs(&VfsPath::local(&file), Default::default());
            let _ = fs::remove_file(file);
        }
    }
    let _ = fs::remove_dir(path);
}
impl LocalFs {
    pub(crate) fn exchange_impl(
        &self,
        left: &VfsPath,
        right: &VfsPath,
        cancel: &dyn CancelCheck,
        progress: &mut dyn FnMut(u64),
    ) -> VfsResult<()> {
        crate::local::check(cancel)?;
        let left_path = crate::local::local(left)?;
        let right_path = crate::local::local(right)?;
        if fs::canonicalize(&left_path)? == fs::canonicalize(&right_path)? {
            return Err(VfsError::Other("cannot exchange a file with itself".into()));
        }
        let a = reserve(&left_path)?;
        let b = match reserve(&right_path) {
            Ok(path) => path,
            Err(e) => {
                cleanup(&a);
                return Err(e);
            }
        };
        let staged = (|| {
            self.copy_file(
                right,
                &VfsPath::local(&a.join("staged")),
                CopyOptions::default(),
                cancel,
                progress,
            )?;
            self.copy_file(
                left,
                &VfsPath::local(&b.join("staged")),
                CopyOptions::default(),
                cancel,
                progress,
            )?;
            crate::local::check(cancel)
        })();
        if let Err(e) = staged {
            cleanup(&a);
            cleanup(&b);
            return Err(e);
        }
        // Publication is a short non-cancellable transaction; cancellation was checked
        // after both streaming copies. A later cancellation cannot interrupt recovery.
        let result = publish(&left_path, &right_path, &a, &b, &mut |from, to| {
            fs::rename(from, to)
        });
        if result
            .as_ref()
            .err()
            .is_some_and(|e| e.to_string().contains("RECOVERY REQUIRED"))
        {
            return result;
        }
        cleanup(&a);
        cleanup(&b);
        result
    }
}
fn publish(
    left: &Path,
    right: &Path,
    a: &Path,
    b: &Path,
    rename: &mut dyn FnMut(&Path, &Path) -> std::io::Result<()>,
) -> VfsResult<()> {
    let mut backed = [false; 2];
    let result: VfsResult<()> = (|| {
        rename(left, &a.join("backup"))?;
        backed[0] = true;
        rename(right, &b.join("backup"))?;
        backed[1] = true;
        rename(&a.join("staged"), left)?;
        rename(&b.join("staged"), right)?;
        Ok(())
    })();
    if let Err(error) = result {
        let mut recovery = Vec::new();
        for (index, path, temp) in [(0, left, a), (1, right, b)] {
            if !backed[index] {
                continue;
            }
            if path.exists() {
                let _ = LocalFs::new().set_attrs(&VfsPath::local(path), Default::default());
            }
            if let Err(e) = fs::rename(temp.join("backup"), path) {
                recovery.push(format!("{}: {e}", temp.display()));
            }
        }
        if !recovery.is_empty() {
            return Err(VfsError::Other(format!(
                "RECOVERY REQUIRED: {}; original failure: {error}",
                recovery.join("; ")
            )));
        }
        return Err(error);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test]
    fn failed_second_publication_restores_both_files() {
        let root =
            std::env::temp_dir().join(format!("matcha-exchange-rollback-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let left = root.join("left");
        let right = root.join("right");
        std::fs::write(&left, b"left").unwrap();
        std::fs::write(&right, b"right").unwrap();
        let a = super::reserve(&left).unwrap();
        let b = super::reserve(&right).unwrap();
        std::fs::write(a.join("staged"), b"right").unwrap();
        std::fs::write(b.join("staged"), b"left").unwrap();
        let mut count = 0;
        assert!(super::publish(&left, &right, &a, &b, &mut |from, to| {
            count += 1;
            if count == 4 {
                return Err(std::io::Error::other("injected publication failure"));
            }
            std::fs::rename(from, to)
        })
        .is_err());
        assert_eq!(std::fs::read(&left).unwrap(), b"left");
        assert_eq!(std::fs::read(&right).unwrap(), b"right");
        std::fs::remove_dir_all(root).unwrap();
    }
}
