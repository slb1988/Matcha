use bc_engine_dir::{
    content::compare_readers, jobs::ContentCompareJob, CompareStatus, ContentCompare, Criteria,
};
use bc_jobs::CancelToken;
use bc_vfs::{VfsError, VfsManager, VfsPath};
use std::{
    fs::{self, File},
    io::{Cursor, Write},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
fn job(left: PathBuf, right: PathBuf, mode: ContentCompare) -> ContentCompareJob {
    ContentCompareJob {
        vfs: Arc::new(VfsManager::local_only()),
        left: VfsPath::local(&left),
        right: VfsPath::local(&right),
        criteria: Criteria {
            content: mode,
            ..Default::default()
        },
        quick_status: CompareStatus::Same,
        node: 0,
        revision: 1,
    }
}
#[test]
fn binary_crc_and_short_readers() {
    struct Short(Cursor<Vec<u8>>);
    impl std::io::Read for Short {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let len = buf.len().min(3);
            self.0.read(&mut buf[..len])
        }
    }
    for mode in [ContentCompare::Binary, ContentCompare::Crc] {
        let bytes = vec![42; 150_000];
        let mut a = Short(Cursor::new(bytes.clone()));
        let mut b = Cursor::new(bytes);
        assert_eq!(
            compare_readers(&mut a, &mut b, mode, &|| false).unwrap(),
            (CompareStatus::Same, [150_000, 150_000])
        );
        assert_eq!(
            compare_readers(
                &mut Cursor::new(b"abc"),
                &mut Cursor::new(b"ab"),
                mode,
                &|| false
            )
            .unwrap()
            .0,
            CompareStatus::Different
        );
    }
}
#[test]
fn rules_are_explicitly_unsupported_and_skip_reads_nothing() {
    let mut task = job(
        "nonexistent-left".into(),
        "nonexistent-right".into(),
        ContentCompare::Binary,
    );
    task.criteria.skip_content_if_quick_same = true;
    let result = task.compare(&|| false).unwrap();
    assert!(result.skipped);
    assert_eq!(result.bytes_read, [0, 0]);
    task.criteria.content = ContentCompare::RulesBased;
    assert!(matches!(
        task.compare(&|| false),
        Err(VfsError::Unsupported(_))
    ));
}
#[test]
fn sparse_gigabyte_early_exit_and_real_thread_cancel() {
    let dir = std::env::temp_dir().join(format!("matcha-content-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(dir.clone());
    for (name, byte) in [("left", 1), ("right", 2)] {
        let path = dir.join(name);
        let mut file = File::create(&path).unwrap();
        #[cfg(windows)]
        {
            let output = std::process::Command::new("fsutil")
                .args(["sparse", "setflag"])
                .arg(&path)
                .output()
                .unwrap();
            assert!(output.status.success(), "must create sparse fixtures");
        }
        file.set_len(1024 * 1024 * 1024).unwrap();
        file.write_all(&[byte]).unwrap();
    }
    let task = job(dir.join("left"), dir.join("right"), ContentCompare::Binary);
    let result = task.compare(&|| false).unwrap();
    assert_eq!(result.status, CompareStatus::Different);
    assert!(result.bytes_read.iter().sum::<u64>() < 1024 * 1024);
    println!("binary.1GiB.bytes_read={:?}", result.bytes_read);

    let task = job(dir.join("left"), dir.join("left"), ContentCompare::Crc);
    let token = CancelToken::new();
    let worker_token = token.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let signaled = std::sync::atomic::AtomicBool::new(false);
        task.compare(&|| {
            if !signaled.swap(true, std::sync::atomic::Ordering::Relaxed) {
                tx.send(()).unwrap();
            }
            worker_token.is_cancelled()
        })
    });
    rx.recv().unwrap();
    std::thread::sleep(Duration::from_millis(10));
    let start = Instant::now();
    token.cancel();
    assert!(matches!(worker.join().unwrap(), Err(VfsError::Cancelled)));
    let elapsed = start.elapsed();
    println!("crc.cancel_ms={:.3}", elapsed.as_secs_f64() * 1000.0);
    assert!(elapsed < Duration::from_secs(1));
}
