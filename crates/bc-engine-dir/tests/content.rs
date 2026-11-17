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
fn skip_quick_same_reads_nothing() {
    let mut task = job(
        "nonexistent-left".into(),
        "nonexistent-right".into(),
        ContentCompare::Binary,
    );
    task.criteria.skip_content_if_quick_same = true;
    let result = task.compare(&|| false).unwrap();
    assert!(result.skipped);
    assert_eq!(result.bytes_read, [0, 0]);
}
#[test]
fn rules_based_uses_the_text_engine_default_rules() {
    let dir = std::env::temp_dir().join(format!("matcha-rules-{}", std::process::id()));
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(dir.clone());
    let pair = |name: &str, left: &[u8], right: &[u8]| {
        let [l, r] = ["left", "right"].map(|side| dir.join(side).join(name));
        for (path, bytes) in [(&l, left), (&r, right)] {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        job(l, r, ContentCompare::RulesBased)
    };
    let rules = |task: ContentCompareJob| task.compare(&|| false).unwrap();
    // Default text rules make comments unimportant (reference default): a comment-only
    // edit is "similar" (the native XML reports both as "diff").
    let comment = pair(
        "example.cpp",
        b"int value = 1; // left comment\n",
        b"int value = 1; // right comment\n",
    );
    let result = rules(comment);
    assert_eq!(result.status, CompareStatus::SimilarUnimportant);
    assert_eq!(result.bytes_read, [31, 32]);
    // `comments-important` restores comments as important differences.
    let mut comment = pair(
        "example.cpp",
        b"int value = 1; // left comment\n",
        b"int value = 1; // right comment\n",
    );
    comment.criteria = Criteria::parse_script("rules-based comments-important").unwrap();
    assert_eq!(rules(comment).status, CompareStatus::Different);
    let script = Criteria::parse_script("rules-based comments-important")
        .unwrap()
        .to_script();
    assert!(script.contains("comments-important"), "{script}");
    assert_eq!(
        Criteria::parse_script(&script).unwrap().rules,
        Criteria::parse_script("rules-based comments-important").unwrap().rules
    );
    assert!(!Criteria::parse_script("rules-based")
        .unwrap()
        .to_script()
        .contains("comments"));
    // The legacy explicit switch still parses and equals the default.
    assert_eq!(
        Criteria::parse_script("rules-based comments-unimportant").unwrap().rules,
        Criteria::parse_script("rules-based").unwrap().rules
    );
    let mut ignoring = Criteria::parse_script("rules-based comments-unimportant").unwrap();
    assert_eq!(
        ignoring.combine(CompareStatus::Same, Some(CompareStatus::SimilarUnimportant), None),
        CompareStatus::SimilarUnimportant
    );
    ignoring.ignore_unimportant = true;
    assert_eq!(
        ignoring.combine(CompareStatus::Same, Some(CompareStatus::SimilarUnimportant), None),
        CompareStatus::Same
    );
    // Line endings are unimportant by default: different bytes, "similar" result.
    let endings = pair("endings.cpp", b"int a;\r\nint b;\r\n", b"int a;\nint b;\n");
    assert_eq!(rules(endings).status, CompareStatus::SimilarUnimportant);
    let code = pair("code.cpp", b"int value = 1;\n", b"int value = 2;\n");
    assert_eq!(rules(code).status, CompareStatus::Different);
    let same = pair("same.cpp", b"int value = 1;\n", b"int value = 1;\n");
    assert_eq!(rules(same).status, CompareStatus::Same);
    // Binary input is not text: the byte result stands.
    let binary = pair("blob.bin", b"\0\x01\x02\0", b"\0\x01\x03\0");
    assert_eq!(rules(binary).status, CompareStatus::Different);
    assert_eq!(
        compare_readers(
            &mut Cursor::new(b"a\r\nb\r\n"),
            &mut Cursor::new(b"a\nb\n"),
            ContentCompare::RulesBased,
            &|| false
        )
        .unwrap(),
        (CompareStatus::SimilarUnimportant, [6, 4])
    );
    let cancelled = pair("cancel.cpp", b"a\n", b"b\n").compare(&|| true);
    assert!(matches!(cancelled, Err(VfsError::Cancelled)));
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
