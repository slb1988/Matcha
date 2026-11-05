use bc_core::line::{LineList, StorageBackend};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Temp(PathBuf);
impl Temp {
    fn new(bytes: &[u8]) -> Self {
        static ID: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "matcha-mmap-{}-{}.txt",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, bytes).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
fn mapped_edits_keep_unchanged_bytes_and_snapshot_alive() {
    let file = Temp::new(b"a\r\nb\nc");
    let mut lines = LineList::open_mapped(&file.0).unwrap();
    assert_eq!(
        lines.backend(),
        if cfg!(windows) {
            StorageBackend::Mapped
        } else {
            StorageBackend::Owned
        }
    );
    assert_eq!(lines.to_bytes(), b"a\r\nb\nc");
    let snapshot = lines.clone();
    let pointer = lines.line_bytes(2).unwrap().as_ptr();
    lines.replace(1, b"changed");
    assert_eq!(lines.backend(), StorageBackend::Segmented);
    assert_eq!(lines.line_bytes(2).unwrap().as_ptr(), pointer);
    assert_eq!(snapshot.to_bytes(), b"a\r\nb\nc");
    assert_eq!(lines.to_bytes(), b"a\r\nchanged\nc");
    #[cfg(windows)]
    {
        assert!(std::fs::OpenOptions::new()
            .write(true)
            .open(&file.0)
            .is_err());
        assert!(std::fs::remove_file(&file.0).is_err());
        drop(lines);
        assert!(std::fs::OpenOptions::new()
            .write(true)
            .open(&file.0)
            .is_err());
        drop(snapshot);
        assert!(std::fs::OpenOptions::new()
            .write(true)
            .open(&file.0)
            .is_ok());
    }
}

#[test]
fn empty_and_missing_files() {
    let file = Temp::new(b"");
    let lines = LineList::open_mapped(&file.0).unwrap();
    assert!(lines.is_empty());
    assert_eq!(lines.backend(), StorageBackend::Owned);
    drop(lines);
    std::fs::remove_file(&file.0).unwrap();
    assert!(LineList::open_mapped(&file.0).is_err());
}

#[cfg(windows)]
#[test]
fn existing_writer_falls_back_to_owned_bytes() {
    let file = Temp::new(b"existing writer\n");
    let _writer = std::fs::OpenOptions::new()
        .write(true)
        .open(&file.0)
        .unwrap();
    let lines = LineList::open_mapped(&file.0).unwrap();
    assert_eq!(lines.backend(), StorageBackend::Owned);
    assert_eq!(lines.to_bytes(), b"existing writer\n");
}
