//! Default-evaluation evidence (iteration 7): find-on-lazy equivalence,
//! fit-all cost, and 1M-row structural-op latency through the real view
//! chain. Large-scale cases are gated on MATCHA_EVAL_LARGE=1 plus the real
//! audit inputs being present; they are measurement harnesses, not CI tests.
use super::*;
use crate::table::find::Action;
use bc_engine_table::{demand::DemandSheet, store::SheetStore};
use std::time::Instant;

fn lazy_store_at(path: &std::path::Path) -> SheetStore {
    SheetStore::Lazy(DemandSheet::open(path, &ReadOptions::default(), &NeverCancel).unwrap())
}

fn large_enabled() -> bool {
    std::env::var_os("MATCHA_EVAL_LARGE").is_some()
}

fn real_inputs() -> Option<(std::path::PathBuf, std::path::PathBuf)> {
    let dir = std::path::Path::new("../../target/table-wave7/audit/inputs");
    let left = dir.join("table-left.csv");
    let right = dir.join("table-right.csv");
    (left.is_file() && right.is_file()).then_some((left, right))
}

fn lazy_view_of(left: &std::path::Path, right: &std::path::Path) -> TableCompareView {
    let mut v = TableCompareView::from_stores(
        lazy_store_at(left),
        lazy_store_at(right),
        TableRules::default(),
    );
    ready(&mut v);
    v
}

fn dense_view_of(left: &std::path::Path, right: &std::path::Path) -> TableCompareView {
    let mut v = TableCompareView::open(left, right).unwrap();
    ready(&mut v);
    v
}

fn needle_fixture(rows: usize) -> String {
    let mut left = String::from("id,name,code\n");
    let mut right = left.clone();
    for i in 0..rows {
        left.push_str(&format!("{i},item{i},code{i}\n"));
        right.push_str(&format!(
            "{i},item{i},code{}\n",
            i + usize::from(i % 997 == 0)
        ));
    }
    left
}

/// Find Next/Previous/Replace-all on lazy stores: identical hit sequence and
/// replacement count to dense, per-press cost reported.
#[test]
fn lazy_find_matches_dense_hit_for_hit() {
    let rows = 30_000;
    let left = needle_fixture(rows);
    let path = std::env::temp_dir().join(format!("matcha-eval-find-{}.csv", std::process::id()));
    std::fs::write(&path, &left).unwrap();
    let mut lazy = {
        let mut v = TableCompareView::from_stores(
            lazy_store_at(&path),
            lazy_store_at(&path),
            TableRules {
                keys: KeySelection::Manual(vec![0]),
                ..Default::default()
            },
        );
        ready(&mut v);
        v
    };
    let mut dense = {
        let mut v = TableCompareView::from_sheets(
            sheet(&left),
            sheet(&left),
            TableRules {
                keys: KeySelection::Manual(vec![0]),
                ..Default::default()
            },
        );
        ready(&mut v);
        v
    };
    for v in [&mut lazy, &mut dense] {
        v.find.query = "item123".into();
    }
    let run = |v: &mut TableCompareView, action| {
        let t = Instant::now();
        v.find_action(action).unwrap();
        t.elapsed()
    };
    // Three Nexts + one Previous: identical landing positions.
    let mut lazy_cost = std::time::Duration::ZERO;
    let mut dense_cost = std::time::Duration::ZERO;
    for action in [Action::Next, Action::Next, Action::Next, Action::Previous] {
        lazy_cost += run(&mut lazy, action);
        dense_cost += run(&mut dense, action);
        assert_eq!(lazy.find.last, dense.find.last, "hit divergence");
    }
    // Replace all: identical replacement count.
    for v in [&mut lazy, &mut dense] {
        v.find.replacement = "REPLACED".into();
    }
    run(&mut lazy, Action::All);
    run(&mut dense, Action::All);
    let count = |v: &TableCompareView, s| {
        let p = v.pair.as_ref().unwrap();
        let sheet = if s == 0 {
            p.map.left.borrow()
        } else {
            p.map.right.borrow()
        };
        (0..sheet.row_count())
            .map(|r| {
                (0..sheet.column_count())
                    .filter(|&c| {
                        sheet
                            .display_at(r, c)
                            .is_some_and(|t| t.contains("REPLACED"))
                    })
                    .count()
            })
            .sum::<usize>()
    };
    assert_eq!(count(&lazy, 0), count(&dense, 0));
    assert!(count(&lazy, 0) > 0);
    eprintln!("[eval] find {rows} rows: lazy 4 presses {lazy_cost:?}, dense {dense_cost:?}");
    let _ = std::fs::remove_file(&path);
}

/// Find press cost on the real 1M-row inputs, both storage paths.
#[test]
fn find_press_cost_on_real_inputs() {
    if !large_enabled() {
        eprintln!("[eval] skipped: set MATCHA_EVAL_LARGE=1");
        return;
    }
    let Some((left, right)) = real_inputs() else {
        eprintln!("[eval] skipped: real inputs missing");
        return;
    };
    for (name, mut v) in [
        ("dense", dense_view_of(&left, &right)),
        ("lazy", lazy_view_of(&left, &right)),
    ] {
        v.find.query = "812345".into();
        let t = Instant::now();
        v.find_action(Action::Next).unwrap();
        let first = t.elapsed();
        let t = Instant::now();
        v.find_action(Action::Next).unwrap();
        let second = t.elapsed();
        eprintln!(
            "[eval] find-next 1M {name}: first={first:?} second={second:?} hit={:?}",
            v.find.last
        );
        assert!(v.find.last.is_some());
    }
}

/// fit-all column-width scan cost on the real 1M-row inputs.
#[test]
fn fit_all_cost_on_real_inputs() {
    if !large_enabled() {
        eprintln!("[eval] skipped: set MATCHA_EVAL_LARGE=1");
        return;
    }
    let Some((left, right)) = real_inputs() else {
        eprintln!("[eval] skipped: real inputs missing");
        return;
    };
    for (name, v) in [
        ("dense", dense_view_of(&left, &right)),
        ("lazy", lazy_view_of(&left, &right)),
    ] {
        let p = v.pair.as_ref().unwrap();
        let output = p.map.output();
        let l = p.map.left.borrow().clone();
        let r = p.map.right.borrow().clone();
        let t = Instant::now();
        let mut cache = crate::table::cache::Cache::build(&output, &l, &r, &NeverCancel).unwrap();
        let build = t.elapsed();
        let t = Instant::now();
        cache.fit_all(&output, &l, &r, &NeverCancel).unwrap();
        eprintln!(
            "[eval] fit-all 1M {name}: build={build:?} fit_all={:?}",
            t.elapsed()
        );
    }
}

/// Structural-op latency through the real view chain at 1M rows.
#[test]
fn structural_ops_latency_on_real_inputs() {
    if !large_enabled() {
        eprintln!("[eval] skipped: set MATCHA_EVAL_LARGE=1");
        return;
    }
    let Some((left, right)) = real_inputs() else {
        eprintln!("[eval] skipped: real inputs missing");
        return;
    };
    for (name, mut v) in [
        ("dense", dense_view_of(&left, &right)),
        ("lazy", lazy_view_of(&left, &right)),
    ] {
        v.key_focus = true;
        // Insert a row below the cursor, then delete it again.
        v.grid.cursor = Cursor { row: 0, column: 0 };
        let t = Instant::now();
        v.structure(1).unwrap();
        ready(&mut v);
        eprintln!("[eval] {name} insert-row+realign: {:?}", t.elapsed());
        let t = Instant::now();
        v.grid.anchor = None;
        v.grid.cursor = Cursor { row: 0, column: 0 };
        v.structure(2).unwrap();
        ready(&mut v);
        eprintln!("[eval] {name} delete-row+realign: {:?}", t.elapsed());
        // Paste a 1000-row block at the end of the left side.
        let mut block = String::new();
        for i in 0..1000 {
            block.push_str(&format!("pasted{i},a,b,c,d,e,f,g,h,{i}\n"));
        }
        v.grid.cursor = Cursor {
            row: v.row_count() - 1,
            column: 0,
        };
        let t = Instant::now();
        frame(
            &egui::Context::default(),
            &mut v,
            vec![egui::Event::Paste(block)],
        );
        ready(&mut v);
        eprintln!("[eval] {name} paste-1000-rows+realign: {:?}", t.elapsed());
        // Sort by the id column over the whole side (anchor+cursor = full range).
        v.grid.anchor = Some(Cursor { row: 0, column: 0 });
        v.grid.cursor = Cursor {
            row: v.row_count() - 1,
            column: 9,
        };
        let t = Instant::now();
        v.sort_selection(false).unwrap();
        ready(&mut v);
        eprintln!("[eval] {name} sort-column+realign: {:?}", t.elapsed());
        // Undo/redo the sort.
        let t = Instant::now();
        v.undo(false);
        ready(&mut v);
        let undo = t.elapsed();
        let t = Instant::now();
        v.undo(true);
        ready(&mut v);
        eprintln!(
            "[eval] {name} undo-sort {:?} redo-sort {:?}",
            undo,
            t.elapsed()
        );
    }
}
