mod common;
use bc_core::Side;
use bc_engine_dir::{
    filter::{FilterClock, OtherFilters},
    op_exec::{ConfirmPolicy, OpState},
    ops::*,
    refresh, NameFilter,
};
use bc_jobs::{CancelToken, Confirmation, Job, JobId, Progress, ProgressSink};
use bc_vfs::{LocalFs, Vfs, VfsPath};
use common::*;
use std::{
    fs,
    sync::{atomic::Ordering, Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
fn clock() -> FilterClock {
    FilterClock {
        today_midnight: UNIX_EPOCH + Duration::from_secs(2_000_000_000),
        utc_offset_seconds: 0,
    }
}
fn copy() -> DirOp {
    DirOp::Copy {
        from: Side::Left,
        to: Side::Right,
    }
}
#[test]
fn planning_all_operations_filters_scan_guard_and_paths() {
    let temp = Temp::new();
    temp.write("left", "src/a.txt", b"aaa");
    temp.write("left", "src/hide.obj", b"hidden");
    temp.write("right", "src/a.txt", b"bbb");
    let vfs = manager(Arc::new(ProbeFs::default()));
    let mut tree = temp.tree(&vfs);
    tree.apply_filters(
        &NameFilter::parse("-*.obj").unwrap(),
        &OtherFilters::default(),
        clock(),
        |_, _| true,
    );
    let select = selected(&tree, &["src"], 0);
    let plan = OpPlan::build(
        &tree,
        &select,
        &copy(),
        Default::default(),
        SystemTime::now(),
    )
    .unwrap();
    assert_eq!(plan.items.len(), 2);
    assert_eq!(plan.hidden_skipped, 1);
    assert!(plan
        .items
        .iter()
        .all(|i| !i.source.as_ref().unwrap().relative.ends_with(".obj")));
    let all = OpPlan::build(
        &tree,
        &select,
        &copy(),
        PlanOptions {
            include_hidden: true,
            ..Default::default()
        },
        SystemTime::now(),
    )
    .unwrap();
    assert_eq!(all.items.len(), 3);
    let mut unscanned = bc_engine_dir::DirTree::new();
    unscanned.bases = tree.bases.clone();
    for side in 0..2 {
        unscanned
            .apply_listing(
                side,
                "",
                vfs.resolve(unscanned.bases[side].as_ref().unwrap())
                    .unwrap()
                    .list(unscanned.bases[side].as_ref().unwrap())
                    .unwrap(),
            )
            .unwrap();
    }
    assert!(matches!(
        OpPlan::build(
            &unscanned,
            &selected(&unscanned, &["src"], 0),
            &copy(),
            Default::default(),
            SystemTime::now()
        ),
        Err(PlanError::ScanRequired { .. })
    ));
    for op in [
        DirOp::Move {
            from: Side::Left,
            to: Side::Right,
        },
        DirOp::CopyOther,
        DirOp::MoveOther,
        DirOp::Exchange,
        DirOp::Rename(RenameSpec::Dos {
            new_mask: "*.bak".into(),
        }),
        DirOp::Rename(RenameSpec::Regex {
            old_mask: "a".into(),
            new_mask: "b".into(),
        }),
        DirOp::Touch(TouchSpec::Set {
            target: Target::Left,
            when: None,
        }),
        DirOp::Touch(TouchSpec::Copy {
            from: Side::Right,
            to: Side::Left,
        }),
        DirOp::Attrib {
            set: Default::default(),
            clear: Default::default(),
        },
        DirOp::Exclude,
        DirOp::Ignored(true),
        DirOp::CopyFilename,
    ] {
        assert!(!common::plan(&tree, &["src/a.txt"], 0, op).items.is_empty());
    }
    for (mode, expected) in [
        (PathMode::Base, "src/a.txt"),
        (PathMode::Relative, "a.txt"),
        (PathMode::None, "a.txt"),
    ] {
        let p = common::plan(
            &tree,
            &["src/a.txt"],
            0,
            DirOp::CopyTo {
                target: Target::Left,
                path_mode: mode,
                dest: temp.0.join("external").to_string_lossy().into(),
            },
        );
        assert!(p.items[0]
            .destination
            .as_ref()
            .unwrap()
            .path
            .to_local_path()
            .unwrap()
            .ends_with(expected));
    }
    assert_eq!(
        rename_name(
            "abc1.txt",
            &RenameSpec::Dos {
                new_mask: "xyz?.bak".into()
            }
        )
        .unwrap(),
        "xyz1.bak"
    );
    assert_eq!(
        rename_name(
            "img_01.png",
            &RenameSpec::Regex {
                old_mask: r"img_(\d+)".into(),
                new_mask: "photo_$1".into()
            }
        )
        .unwrap(),
        "photo_01.png"
    );
    assert!(rename_name(
        "a",
        &RenameSpec::Dos {
            new_mask: "../escape".into()
        }
    )
    .is_err());
}
#[test]
fn copy_touch_attrib_move_exchange_rename_and_session_effects() {
    let temp = Temp::new();
    temp.write("left", "a", b"left");
    temp.write("right", "a", b"right");
    let vfs = manager(Arc::new(ProbeFs::default()));
    let ctx = context(None);
    let mut tree = temp.tree(&vfs);
    let exchange = plan(&tree, &["a"], 0, DirOp::Exchange);
    let (o, _) = execute(&exchange, &vfs, &ctx, ConfirmPolicy::YesToAll);
    assert_eq!(o.failed, 0);
    assert_eq!(fs::read(temp.path("left", "a")).unwrap(), b"right");
    assert_eq!(fs::read(temp.path("right", "a")).unwrap(), b"left");
    refresh::refresh_locations(&mut tree, &vfs, &o.affected, &|| false).unwrap();
    let timestamp = UNIX_EPOCH + Duration::from_secs(1234567890);
    let p = plan(
        &tree,
        &["a"],
        0,
        DirOp::Touch(TouchSpec::Set {
            target: Target::Left,
            when: Some(timestamp),
        }),
    );
    let (o, _) = execute(&p, &vfs, &ctx, ConfirmPolicy::YesToAll);
    assert_eq!(o.completed, 1);
    refresh::refresh_locations(&mut tree, &vfs, &o.affected, &|| false).unwrap();
    let p = plan(
        &tree,
        &["a"],
        0,
        DirOp::Attrib {
            set: bc_vfs::FileAttrs {
                readonly: true,
                ..Default::default()
            },
            clear: Default::default(),
        },
    );
    let (o, _) = execute(&p, &vfs, &ctx, ConfirmPolicy::YesToAll);
    assert_eq!(o.completed, 1);
    refresh::refresh_locations(&mut tree, &vfs, &o.affected, &|| false).unwrap();
    let p = plan(&tree, &["a"], 0, copy());
    let (o, _) = execute(&p, &vfs, &ctx, ConfirmPolicy::YesToAll);
    assert_eq!(o.failed, 0);
    let copied = LocalFs::new()
        .metadata(&VfsPath::local(&temp.path("right", "a")))
        .unwrap();
    assert_eq!(copied.modified, Some(timestamp));
    assert!(copied.attrs.readonly);
    refresh::refresh_locations(&mut tree, &vfs, &o.affected, &|| false).unwrap();
    let mut settings = Default::default();
    for op in [
        DirOp::Ignored(true),
        DirOp::Ignored(false),
        DirOp::CopyFilename,
        DirOp::Exclude,
    ] {
        let p = plan(&tree, &["a"], 0, op.clone());
        let (o, _) = execute(&p, &vfs, &ctx, ConfirmPolicy::YesToAll);
        o.apply_session(&mut tree, &mut settings).unwrap();
        if op == DirOp::CopyFilename {
            assert!(o.filename_text().contains("left"));
        }
    }
    assert!(tree.node(tree.find("a").unwrap()).unwrap().excluded);
    let root = Selection::both([0]);
    let p = OpPlan::build(
        &tree,
        &root,
        &DirOp::NewFolder {
            target: Target::Right,
            name: "new".into(),
        },
        Default::default(),
        SystemTime::now(),
    )
    .unwrap();
    let (o, _) = execute(&p, &vfs, &ctx, ConfirmPolicy::YesToAll);
    assert_eq!(o.failed, 0);
    assert!(temp.path("right", "new").is_dir());
    temp.write("left", "move.txt", b"move");
    tree = temp.tree(&vfs);
    let p = plan(
        &tree,
        &["move.txt"],
        0,
        DirOp::MoveTo {
            target: Target::Left,
            path_mode: PathMode::None,
            dest: temp.0.join("elsewhere").to_string_lossy().into(),
        },
    );
    let (o, _) = execute(&p, &vfs, &ctx, ConfirmPolicy::YesToAll);
    assert_eq!(o.completed, 1);
    assert!(!temp.path("left", "move.txt").exists());
    temp.write("left", "rename.txt", b"rename");
    tree = temp.tree(&vfs);
    let p = plan(
        &tree,
        &["rename.txt"],
        0,
        DirOp::Rename(RenameSpec::Dos {
            new_mask: "*.bak".into(),
        }),
    );
    let (o, _) = execute(&p, &vfs, &ctx, ConfirmPolicy::YesToAll);
    assert_eq!(o.completed, 1);
    assert!(temp.path("left", "rename.bak").exists());
}
#[test]
fn confirmations_are_class_scoped_and_permanent_fallback_is_independent() {
    let temp = Temp::new();
    for name in ["a", "b"] {
        temp.write("left", name, b"left");
        temp.write("right", name, b"right");
    }
    let vfs = manager(Arc::new(ProbeFs::default()));
    for name in ["a", "b"] {
        for side in ["left", "right"] {
            LocalFs::new()
                .set_modified(
                    &VfsPath::local(&temp.path(side, name)),
                    UNIX_EPOCH + Duration::from_secs(1_700_000_000),
                )
                .unwrap();
        }
    }
    let tree = temp.tree(&vfs);
    let p = plan(&tree, &["a", "b"], 0, copy());
    let answers = Answers::new([Confirmation::NoToAll]);
    let (o, _) = execute(
        &p,
        &vfs,
        &context(Some(answers.clone())),
        ConfirmPolicy::Prompt,
    );
    assert_eq!(o.skipped, 2);
    assert_eq!(answers.questions.lock().unwrap().len(), 1);
    let answers = Answers::new([Confirmation::YesToAll]);
    let (o, _) = execute(
        &p,
        &vfs,
        &context(Some(answers.clone())),
        ConfirmPolicy::Prompt,
    );
    assert_eq!(o.completed, 2);
    assert_eq!(answers.questions.lock().unwrap().len(), 1);
    let probe = Arc::new(ProbeFs {
        no_recycle: true,
        ..Default::default()
    });
    let vfs = manager(probe.clone());
    let tree = temp.tree(&vfs);
    let p = plan(
        &tree,
        &["a", "b"],
        0,
        DirOp::Delete {
            target: Target::Left,
            recycle_bin: true,
        },
    );
    let answers = Answers::new([Confirmation::YesToAll, Confirmation::No, Confirmation::Yes]);
    let (o, log) = execute(
        &p,
        &vfs,
        &context(Some(answers.clone())),
        ConfirmPolicy::Prompt,
    );
    assert_eq!((o.completed, o.skipped), (1, 1));
    assert!(temp.path("left", "a").exists());
    assert!(!temp.path("left", "b").exists());
    assert_eq!(answers.questions.lock().unwrap().len(), 3);
    assert_eq!(probe.permanent.load(Ordering::Relaxed), 1);
    assert!(log
        .snapshot()
        .iter()
        .any(|e| e.message.contains("recycle unavailable")));
    let p = plan(
        &temp.tree(&vfs),
        &["a"],
        0,
        DirOp::Delete {
            target: Target::Left,
            recycle_bin: true,
        },
    );
    let (o, _) = execute(&p, &vfs, &context(None), ConfirmPolicy::YesToAll);
    assert_eq!(o.skipped, 1);
    assert!(temp.path("left", "a").exists());
}
#[test]
fn newer_and_readonly_prompts_are_distinct_failures_continue_and_stale_targets_are_safe() {
    let temp = Temp::new();
    for name in ["a", "b", "c"] {
        temp.write("left", name, b"left");
        temp.write("right", name, b"right");
    }
    for name in ["a", "b", "c"] {
        for side in ["left", "right"] {
            LocalFs::new()
                .set_modified(
                    &VfsPath::local(&temp.path(side, name)),
                    UNIX_EPOCH + Duration::from_secs(1_700_000_000),
                )
                .unwrap();
        }
    }
    LocalFs::new()
        .set_modified(
            &VfsPath::local(&temp.path("right", "b")),
            UNIX_EPOCH + Duration::from_secs(1_700_000_060),
        )
        .unwrap();
    LocalFs::new()
        .set_attrs(
            &VfsPath::local(&temp.path("right", "c")),
            bc_vfs::FileAttrs {
                readonly: true,
                ..Default::default()
            },
        )
        .unwrap();
    let vfs = manager(Arc::new(ProbeFs::default()));
    let tree = temp.tree(&vfs);
    let p = plan(&tree, &["a", "b", "c"], 0, copy());
    let answers = Answers::new([
        Confirmation::YesToAll,
        Confirmation::NoToAll,
        Confirmation::No,
    ]);
    let (o, _) = execute(
        &p,
        &vfs,
        &context(Some(answers.clone())),
        ConfirmPolicy::Prompt,
    );
    assert_eq!((o.completed, o.skipped), (1, 2));
    let questions = answers.questions.lock().unwrap().join("\n");
    assert!(questions.contains("OverwriteNewer"));
    assert!(questions.contains("ReadOnly"));
    let probe = Arc::new(ProbeFs {
        deny_delete: Some("b".into()),
        ..Default::default()
    });
    let vfs = manager(probe);
    let p = plan(
        &temp.tree(&vfs),
        &["a", "b", "c"],
        0,
        DirOp::Delete {
            target: Target::Left,
            recycle_bin: false,
        },
    );
    let (o, log) = execute(&p, &vfs, &context(None), ConfirmPolicy::YesToAll);
    assert_eq!((o.completed, o.failed), (2, 1));
    assert!(temp.path("left", "b").exists());
    let mut output = Vec::new();
    log.write_to(&mut output).unwrap();
    assert!(String::from_utf8(output).unwrap().contains("Failed"));
    let p = plan(&temp.tree(&vfs), &["b"], 0, copy());
    temp.write("right", "b", b"changed after planning");
    let (o, _) = execute(&p, &vfs, &context(None), ConfirmPolicy::YesToAll);
    assert_eq!(o.failed, 1);
    assert_eq!(
        fs::read(temp.path("right", "b")).unwrap(),
        b"changed after planning"
    );
}
#[test]
fn filtered_folder_deletion_preserves_hidden_and_new_contents() {
    let temp = Temp::new();
    temp.write("left", "d/keep", b"x");
    temp.write("left", "d/hidden.obj", b"x");
    let vfs = manager(Arc::new(ProbeFs::default()));
    let mut tree = temp.tree(&vfs);
    tree.apply_filters(
        &NameFilter::parse("-*.obj").unwrap(),
        &Default::default(),
        clock(),
        |_, _| true,
    );
    let p = plan(
        &tree,
        &["d"],
        0,
        DirOp::Delete {
            target: Target::Left,
            recycle_bin: false,
        },
    );
    let (o, _) = execute(&p, &vfs, &context(None), ConfirmPolicy::YesToAll);
    assert_eq!(o.completed, 1);
    assert!(temp.path("left", "d/hidden.obj").exists());
    assert!(temp.path("left", "d").is_dir());
}
#[test]
fn localized_refresh_touches_one_row_in_a_hundred_thousand_node_model() {
    let temp = Temp::new();
    temp.write("left", "f050000", b"real");
    let probe = Arc::new(ProbeFs::default());
    let vfs = manager(probe.clone());
    let mut tree = bc_engine_dir::DirTree::new();
    tree.bases = [
        Some(VfsPath::local(&temp.path("left", ""))),
        Some(VfsPath::local(&temp.path("right", ""))),
        None,
    ];
    let real = LocalFs::new()
        .metadata(&VfsPath::local(&temp.path("left", "f050000")))
        .unwrap();
    let entries: Vec<_> = (0..100_000)
        .map(|i| bc_vfs::VfsEntry {
            name: format!("f{i:06}"),
            ..real.clone()
        })
        .collect();
    tree.apply_listing(0, "", entries.clone()).unwrap();
    tree.apply_listing(
        1,
        "",
        entries
            .into_iter()
            .filter(|e| e.name != "f050000")
            .collect(),
    )
    .unwrap();
    let untouched = tree.find("f000000").unwrap();
    let revision = tree.node(untouched).unwrap().revision;
    let p = plan(&tree, &["f050000"], 0, copy());
    let (o, _) = execute(&p, &vfs, &context(None), ConfirmPolicy::YesToAll);
    assert_eq!(o.completed, 1);
    probe.reads.store(0, Ordering::Relaxed);
    probe.lists.store(0, Ordering::Relaxed);
    let stats = refresh::refresh_locations(&mut tree, &vfs, &o.affected, &|| false).unwrap();
    assert_eq!(stats.changed_nodes.len(), 1);
    assert_eq!(stats.metadata_reads, 2);
    assert_eq!(probe.reads.load(Ordering::Relaxed), 2);
    assert_eq!(probe.lists.load(Ordering::Relaxed), 0);
    assert_eq!(tree.node(untouched).unwrap().revision, revision);
}
#[test]
fn watcher_driven_refresh_and_full_refresh_detect_external_changes() {
    let temp = Temp::new();
    let vfs = manager(Arc::new(ProbeFs::default()));
    let mut tree = temp.tree(&vfs);
    let mut watcher = refresh::AutoRefresh::new(&tree, &vfs).unwrap();
    temp.write("left", "external", b"x");
    let start = Instant::now();
    while tree.find("external").is_none() {
        watcher.poll(&mut tree, &vfs, &|| false).unwrap();
        assert!(start.elapsed() < Duration::from_secs(3));
        std::thread::sleep(Duration::from_millis(5));
    }
    temp.write("left", "sub/new", b"y");
    refresh::full_refresh(&mut tree, &vfs, &|| false).unwrap();
    assert!(tree.find("sub/new").is_some());
    temp.write("left", "ordinary", b"z");
    refresh::refresh_loaded(&mut tree, &vfs, &|| false).unwrap();
    assert!(tree.find("ordinary").is_some());
}
#[test]
fn directory_exchange_includes_both_sides_orphans_and_cleans_empty_source() {
    let temp = Temp::new();
    temp.write("left", "d/pair", b"left");
    temp.write("right", "d/pair", b"right");
    temp.write("left", "d/left-only", b"L");
    temp.write("right", "d/right-only", b"R");
    temp.write("left", "only/f", b"only");
    let vfs = manager(Arc::new(ProbeFs::default()));
    let tree = temp.tree(&vfs);
    let p = plan(&tree, &["d", "only"], 0, DirOp::Exchange);
    let (o, log) = execute(&p, &vfs, &context(None), ConfirmPolicy::YesToAll);
    assert_eq!(o.failed, 0, "{:?}", log.snapshot());
    assert_eq!(o.skipped, 0);
    assert_eq!(fs::read(temp.path("left", "d/pair")).unwrap(), b"right");
    assert_eq!(fs::read(temp.path("right", "d/pair")).unwrap(), b"left");
    assert!(temp.path("left", "d/right-only").exists());
    assert!(temp.path("right", "d/left-only").exists());
    assert!(!temp.path("left", "d/left-only").exists());
    assert!(!temp.path("right", "d/right-only").exists());
    assert!(!temp.path("left", "only").exists());
    assert!(temp.path("right", "only/f").exists());
}

#[test]
fn cancellation_inside_one_large_copy_leaves_no_partial_target() {
    let temp = Temp::new();
    fs::File::create(temp.path("left", "large"))
        .unwrap()
        .set_len(4 * 1024 * 1024)
        .unwrap();
    let vfs = manager(Arc::new(ProbeFs::default()));
    let tree = temp.tree(&vfs);
    let p = plan(&tree, &["large"], 0, copy());
    struct CancelBytes(CancelToken);
    impl ProgressSink for CancelBytes {
        fn report(&self, _: JobId, p: &Progress) {
            if p.message.contains("bytes=65536") {
                self.0.cancel();
            }
        }
    }
    let mut ctx = context(None);
    ctx.progress = Arc::new(CancelBytes(ctx.cancel.clone()));
    let (o, _) = execute(&p, &vfs, &ctx, ConfirmPolicy::YesToAll);
    assert!(o.cancelled);
    assert_eq!(o.completed, 0);
    assert_eq!(o.bytes_written, 65536);
    assert_eq!(fs::read_dir(temp.path("right", "")).unwrap().count(), 0);
}

#[test]
fn five_thousand_file_job_cancels_halfway_with_partial_results_and_progress() {
    let temp = Temp::new();
    for i in 0..5000 {
        temp.write("left", &format!("f{i:04}"), b"data");
    }
    let vfs = manager(Arc::new(ProbeFs::default()));
    let tree = temp.tree(&vfs);
    let selection =
        Selection::parse_script("select left.files", &tree, Default::default()).unwrap();
    let p = OpPlan::build(
        &tree,
        &selection,
        &copy(),
        Default::default(),
        SystemTime::now(),
    )
    .unwrap();
    assert_eq!(p.items.len(), 5000);
    struct Sink {
        cancel: CancelToken,
        at: Mutex<Option<Instant>>,
        last: Mutex<Option<Progress>>,
    }
    impl ProgressSink for Sink {
        fn report(&self, _: JobId, p: &Progress) {
            *self.last.lock().unwrap() = Some(p.clone());
            if p.current == 2500 && !self.cancel.is_cancelled() {
                *self.at.lock().unwrap() = Some(Instant::now());
                self.cancel.cancel();
            }
        }
    }
    let mut ctx = context(None);
    let sink = Arc::new(Sink {
        cancel: ctx.cancel.clone(),
        at: Mutex::new(None),
        last: Mutex::new(None),
    });
    ctx.progress = sink.clone();
    let log = bc_engine_dir::op_exec::OpLog::default();
    let worker_log = log.clone();
    let task = bc_engine_dir::jobs::FileOpJob {
        vfs: vfs.clone(),
        plan: p,
        confirmation: ConfirmPolicy::YesToAll,
        log: worker_log,
    };
    let started = Instant::now();
    let worker = std::thread::spawn(move || Box::new(task).run(&ctx));
    let result = worker
        .join()
        .unwrap()
        .unwrap()
        .downcast::<bc_engine_dir::op_exec::OpOutcome>()
        .unwrap();
    let cancel_ms = sink.at.lock().unwrap().unwrap().elapsed().as_secs_f64() * 1000.0;
    assert!(result.cancelled);
    assert_eq!(result.completed, 2500);
    assert_eq!(result.bytes_written, 10_000);
    assert_eq!(fs::read_dir(temp.path("right", "")).unwrap().count(), 2500);
    assert!(cancel_ms < 1000.0);
    assert_eq!(
        sink.last.lock().unwrap().as_ref().unwrap().total,
        Some(5000)
    );
    assert!(log.snapshot().iter().any(|e| e.state == OpState::Cancelled));
    println!(
        "operations.completed=2500/5000; bytes=10000; elapsed_s={:.6}; cancel_ms={cancel_ms:.3}",
        started.elapsed().as_secs_f64()
    );
}
