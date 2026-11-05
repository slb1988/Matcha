use bc_vfs::{ChangeEvent, CopyOptions, EntryKind, FileAttrs, LocalFs, Vfs, VfsError, VfsPath};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant, UNIX_EPOCH},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "matcha-vfs-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn path(&self, name: &str) -> VfsPath {
        VfsPath::local(&self.0.join(name))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn paths_roundtrip() {
    for input in [
        r"C:\work\a.txt",
        r"\\server\share\dir",
        "./rel/a",
        "file:///C:/work/a",
        "file://server/share/a",
        "sftp://host/a",
    ] {
        let path = VfsPath::parse(input).unwrap();
        assert_eq!(path.to_string(), input);
        assert_eq!(path.join("child").parent().unwrap(), path);
        assert_eq!(path.join("child").file_name(), Some("child"));
    }
    for input in ["", "a\0b", "://missing", "1bad://x", "file://", "a\nb"] {
        assert!(VfsPath::parse(input).is_err(), "{input:?}");
    }
    assert!(VfsPath::parse(r"C:\").unwrap().parent().is_none());
    assert!(VfsPath::parse(r"\\server\share")
        .unwrap()
        .parent()
        .is_none());
    #[cfg(windows)]
    assert_eq!(
        VfsPath::parse("file:///C:/work/a")
            .unwrap()
            .to_local_path()
            .unwrap(),
        PathBuf::from("C:/work/a")
    );
}

#[test]
fn listing_copy_touch_crc_and_delete() {
    let t = Temp::new();
    let vfs = LocalFs::new();
    vfs.make_dir(&t.path("source/nested")).unwrap();
    fs::write(t.0.join("source/a.txt"), b"123456789").unwrap();
    let timestamp = UNIX_EPOCH + Duration::new(1_767_225_600, 123_456_700);
    vfs.set_modified(&t.path("source/a.txt"), timestamp)
        .unwrap();
    let attributes = FileAttrs {
        hidden: cfg!(windows),
        archive: cfg!(windows),
        ..FileAttrs::default()
    };
    vfs.set_attrs(&t.path("source/a.txt"), attributes).unwrap();
    let listing = vfs.list(&t.path("source")).unwrap();
    assert_eq!(listing.len(), 2);
    assert_eq!(listing[0].size, Some(9));
    assert_eq!(listing[0].kind, EntryKind::File);
    assert_eq!(listing[1].kind, EntryKind::Dir);
    assert_eq!(listing[0].modified, Some(timestamp));
    assert!(listing.iter().all(|e| e.error.is_none()));
    assert_eq!(vfs.crc32(&t.path("source/a.txt")).unwrap(), 0xcbf43926);
    vfs.get(&t.path("source"), &t.0.join("copy")).unwrap();
    let copied = vfs.metadata(&t.path("copy/a.txt")).unwrap();
    assert_eq!(copied.modified, Some(timestamp));
    assert_eq!(copied.attrs.hidden, attributes.hidden);
    vfs.put(&t.0.join("copy/a.txt"), &t.path("put.txt"))
        .unwrap();
    vfs.rename(&t.path("copy"), &t.path("moved")).unwrap();
    assert!(!t.0.join("copy").exists());
    vfs.delete(&t.path("moved"), false).unwrap();
    assert!(!t.0.join("moved").exists());
    assert!(vfs
        .get(&t.path("source"), &t.0.join("source/inside"))
        .is_err());
}

#[test]
fn cancel_copy_cleans_temp_and_preserves_existing_destination() {
    let t = Temp::new();
    let vfs = LocalFs::new();
    fs::File::create(t.0.join("large"))
        .unwrap()
        .set_len(16 * 1024 * 1024)
        .unwrap();
    for existing in [false, true] {
        if existing {
            fs::write(t.0.join("dest"), b"old").unwrap();
        }
        let polls = AtomicUsize::new(0);
        let cancel = || polls.fetch_add(1, Ordering::Relaxed) >= 5;
        let result = vfs.get_cancellable(
            &t.path("large"),
            &t.0.join("dest"),
            CopyOptions {
                overwrite: existing,
                ..CopyOptions::default()
            },
            &cancel,
        );
        assert!(matches!(result, Err(VfsError::Cancelled)));
        if existing {
            assert_eq!(fs::read(t.0.join("dest")).unwrap(), b"old");
        } else {
            assert!(!t.0.join("dest").exists());
        }
        assert!(!fs::read_dir(&t.0).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".matcha-copy")));
    }
    assert!(matches!(
        vfs.crc32_cancellable(&t.path("large"), &|| true),
        Err(VfsError::Cancelled)
    ));
}

#[test]
fn overwrite_is_explicit_and_readonly_metadata_is_preserved() {
    let t = Temp::new();
    let vfs = LocalFs::new();
    fs::write(t.0.join("src"), b"new").unwrap();
    fs::write(t.0.join("dest"), b"old").unwrap();
    assert!(vfs.get(&t.path("src"), &t.0.join("dest")).is_err());
    vfs.get_cancellable(
        &t.path("src"),
        &t.0.join("dest"),
        CopyOptions {
            overwrite: true,
            ..CopyOptions::default()
        },
        &|| false,
    )
    .unwrap();
    assert_eq!(fs::read(t.0.join("dest")).unwrap(), b"new");
    vfs.set_attrs(
        &t.path("src"),
        FileAttrs {
            readonly: true,
            ..FileAttrs::default()
        },
    )
    .unwrap();
    vfs.get(&t.path("src"), &t.0.join("readonly")).unwrap();
    assert!(vfs.metadata(&t.path("readonly")).unwrap().attrs.readonly);
    for name in ["src", "readonly"] {
        vfs.set_attrs(&t.path(name), FileAttrs::default()).unwrap();
    }
}

#[test]
fn watch_create_modify_rename_delete_and_drop() {
    let t = Temp::new();
    let vfs = LocalFs::new();
    let mut watcher = vfs.watch(&t.path("")).unwrap();
    assert!(watcher.poll().is_empty());
    fs::write(t.0.join("created"), b"hello").unwrap();
    let start = Instant::now();
    loop {
        if watcher.poll().iter().any(
            |event| matches!(event, ChangeEvent::Created(p) if p.file_name() == Some("created")),
        ) {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(3));
        std::thread::sleep(Duration::from_millis(5));
    }
    fs::rename(t.0.join("created"), t.0.join("renamed")).unwrap();
    let start = Instant::now();
    loop {
        if watcher.poll().iter().any(|event| matches!(event, ChangeEvent::Renamed { to, .. } | ChangeEvent::Created(to) if to.file_name() == Some("renamed"))) { break; }
        assert!(start.elapsed() < Duration::from_secs(3));
        std::thread::sleep(Duration::from_millis(5));
    }
    fs::remove_file(t.0.join("renamed")).unwrap();
    let start = Instant::now();
    loop {
        if watcher.poll().iter().any(
            |event| matches!(event, ChangeEvent::Deleted(p) if p.file_name() == Some("renamed")),
        ) {
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(3));
        std::thread::sleep(Duration::from_millis(5));
    }
    let start = Instant::now();
    drop(watcher);
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[cfg(windows)]
#[test]
fn junction_is_not_traversed_unless_requested() {
    let t = Temp::new();
    fs::create_dir(t.0.join("target")).unwrap();
    fs::write(t.0.join("target/keep"), b"keep").unwrap();
    let status = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(t.0.join("link"))
        .arg(t.0.join("target"))
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(
        LocalFs::new().metadata(&t.path("link")).unwrap().kind,
        EntryKind::Symlink
    );
    assert_eq!(
        LocalFs {
            follow_symlinks: true
        }
        .metadata(&t.path("link"))
        .unwrap()
        .kind,
        EntryKind::Dir
    );
    LocalFs::new().delete(&t.path("link"), false).unwrap();
    assert!(t.0.join("target/keep").exists());
}

#[cfg(windows)]
#[test]
fn denied_child_is_error_not_lost_siblings() {
    let t = Temp::new();
    fs::create_dir(t.0.join("denied")).unwrap();
    fs::write(t.0.join("visible"), b"ok").unwrap();
    let identity = std::process::Command::new("whoami").output().unwrap();
    let identity = String::from_utf8(identity.stdout).unwrap();
    let identity = identity.trim();
    let status = std::process::Command::new("icacls")
        .arg(t.0.join("denied"))
        .arg("/deny")
        .arg(format!("{identity}:(RD)"))
        .output()
        .unwrap();
    assert!(status.status.success());
    let listing = LocalFs::new().list(&t.path("")).unwrap();
    let reset = std::process::Command::new("icacls")
        .arg(t.0.join("denied"))
        .arg("/remove:d")
        .arg(identity)
        .output()
        .unwrap();
    assert!(reset.status.success());
    assert!(listing
        .iter()
        .find(|e| e.name == "denied")
        .unwrap()
        .error
        .is_some());
    assert!(listing
        .iter()
        .find(|e| e.name == "visible")
        .unwrap()
        .error
        .is_none());
}

#[cfg(windows)]
#[test]
#[ignore = "manual: creates a disposable file in the Windows recycle bin"]
fn recycle_bin_manual() {
    let t = Temp::new();
    let name = format!("matcha-recycle-proof-{}.txt", std::process::id());
    fs::write(t.0.join(&name), b"disposable recycle verification").unwrap();
    LocalFs::new().delete(&t.path(&name), true).unwrap();
    assert!(!t.0.join(&name).exists());
    let script = format!("$s=New-Object -ComObject Shell.Application; $items=@($s.Namespace(10).Items() | Where-Object {{$_.Name -eq '{name}'}}); if ($items.Count -eq 0) {{exit 1}}; $items | ForEach-Object {{$_.Path}}");
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .output()
        .unwrap();
    println!("{}", String::from_utf8_lossy(&output.stdout));
    assert!(
        output.status.success(),
        "file must be discoverable in recycle bin"
    );
}
