//! Experimental demand-storage (MATCHA_TABLE_CSV_LAZY) GUI-path tests: the
//! real TableCompareView driven over Lazy stores. Minimal chains only — value
//! edit/undo/redo/save/reload, structural edit/sort undo/redo/save, copy and
//! export — complementing the dense matrix, never replacing it.
use super::*;
use bc_engine_table::{columns::Side, demand::DemandSheet, SheetStore};
use std::sync::atomic::{AtomicU32, Ordering};

static TAG: AtomicU32 = AtomicU32::new(0);

fn lazy_store(text: &str) -> SheetStore {
    let path = std::env::temp_dir().join(format!(
        "matcha-lazy-gui-test-{}-{}.csv",
        std::process::id(),
        TAG.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, text).unwrap();
    let store =
        SheetStore::Lazy(DemandSheet::open(&path, &ReadOptions::default(), &NeverCancel).unwrap());
    let _ = std::fs::remove_file(&path);
    store
}

fn lazy_view(left: &str, right: &str) -> TableCompareView {
    let mut v = TableCompareView::from_stores(
        lazy_store(left),
        lazy_store(right),
        TableRules {
            keys: KeySelection::Manual(vec![0]),
            strategy: AlignStrategy::Quick,
            ..Default::default()
        },
    );
    ready(&mut v);
    v
}

fn text(v: &TableCompareView, side: usize, row: usize, column: usize) -> String {
    let p = v.pair.as_ref().unwrap();
    let s = if side == 0 {
        p.map.left.borrow()
    } else {
        p.map.right.borrow()
    };
    s.display_at(row, column).unwrap()
}

fn lazy_fixture() -> (&'static str, &'static str) {
    (
        "id,name,price\n7,apple,1.0\n8,pear,2.0\n9,plum,3.0\n",
        "id,name,price\n7,apple,1.2\n8,PEAR,2.1\n9,plum,3.0\n",
    )
}

#[test]
fn lazy_value_edit_undo_redo_save_and_reload() {
    let (left, right) = lazy_fixture();
    let mut v = lazy_view(left, right);
    let ctx = egui::Context::default();
    // Render frames: grid preview reads go through the bounded row cache.
    frame(&ctx, &mut v, vec![]);
    assert_eq!(text(&v, 1, 0, 1), "apple");
    assert_eq!(text(&v, 1, 1, 1), "PEAR");
    // Edit a non-key cell on the right (lazy) side.
    v.begin_edit(1, Cursor { row: 1, column: 1 }, Some("pair".into()));
    v.commit_edit().unwrap();
    ready(&mut v);
    assert_eq!(text(&v, 1, 1, 1), "pair");
    frame(&ctx, &mut v, vec![]);
    // Undo / redo through the view's edit stack.
    v.undo(false);
    ready(&mut v);
    assert_eq!(text(&v, 1, 1, 1), "PEAR");
    v.undo(true);
    ready(&mut v);
    assert_eq!(text(&v, 1, 1, 1), "pair");
    // Save-as to a fresh path, then re-read the bytes.
    let out = std::env::temp_dir().join(format!(
        "matcha-lazy-gui-save-{}-{}.csv",
        std::process::id(),
        TAG.fetch_add(1, Ordering::Relaxed)
    ));
    v.save_to(1, &out, &Prefs::default(), false).unwrap();
    let bytes = std::fs::read_to_string(&out).unwrap();
    assert_eq!(bytes, "id,name,price\n7,apple,1.2\n8,pair,2.1\n9,plum,3.0\n");
    // Re-read: dense and lazy loads of the saved file show identical cells.
    let dense = sheet(&bytes);
    let lazy = lazy_store(&bytes);
    for r in 0..dense.row_count() {
        for c in 0..dense.column_count() {
            assert_eq!(dense.display_at(r, c), lazy.display_at(r, c));
        }
    }
    let _ = std::fs::remove_file(&out);
}

#[test]
fn lazy_structural_chain_with_undo_redo_and_save() {
    let (left, right) = lazy_fixture();
    let mut v = lazy_view(left, right);
    let ctx = egui::Context::default();
    v.grid.active = 1;
    v.grid.cursor = Cursor { row: 1, column: 1 };
    // Insert Row After (op 1), then Insert Column After (op 4) — same command
    // surface as the dense command matrix.
    v.structure(1).unwrap();
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().map.right.borrow().row_count(), 4);
    assert_eq!(text(&v, 1, 2, 0), "");
    v.structure(4).unwrap();
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().map.right.borrow().column_count(), 4);
    assert_eq!(text(&v, 1, 0, 2), "");
    // Undo both, redo both.
    v.undo(false);
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().map.right.borrow().column_count(), 3);
    v.undo(false);
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().map.right.borrow().row_count(), 3);
    v.undo(true);
    ready(&mut v);
    v.undo(true);
    ready(&mut v);
    assert_eq!(v.pair.as_ref().unwrap().map.right.borrow().row_count(), 4);
    assert_eq!(v.pair.as_ref().unwrap().map.right.borrow().column_count(), 4);
    // Sort the lazy side descending by id, then undo.
    v.grid.anchor = Some(Cursor { row: 0, column: 0 });
    v.grid.cursor = Cursor { row: 0, column: 0 };
    let ids = |v: &TableCompareView| {
        let p = v.pair.as_ref().unwrap();
        let s = p.map.right.borrow();
        (0..s.row_count())
            .map(|r| s.display_at(r, 0).unwrap())
            .collect::<Vec<_>>()
    };
    v.sort_selection(true).unwrap();
    ready(&mut v);
    assert_eq!(ids(&v), ["9", "8", "7", ""]);
    v.undo(false);
    ready(&mut v);
    assert_eq!(ids(&v), ["7", "8", "", "9"][..]);
    // Save the restructured side and verify bytes, then reload.
    let out = std::env::temp_dir().join(format!(
        "matcha-lazy-gui-struct-{}-{}.csv",
        std::process::id(),
        TAG.fetch_add(1, Ordering::Relaxed)
    ));
    v.save_to(1, &out, &Prefs::default(), false).unwrap();
    let bytes = std::fs::read_to_string(&out).unwrap();
    let dense = sheet(&bytes);
    let lazy = lazy_store(&bytes);
    assert_eq!(dense.row_count(), lazy.row_count());
    for r in 0..dense.row_count() {
        for c in 0..dense.column_count() {
            assert_eq!(dense.display_at(r, c), lazy.display_at(r, c));
        }
    }
    let _ = std::fs::remove_file(&out);
}

#[test]
fn lazy_copy_cell_and_export() {
    let (left, right) = lazy_fixture();
    let mut v = lazy_view(left, right);
    let ctx = egui::Context::default();
    // Select the differing left price cell and use the real copy command.
    v.grid.active = 0;
    v.grid.anchor = Some(Cursor { row: 0, column: 2 });
    v.grid.cursor = Cursor { row: 0, column: 2 };
    frame(&ctx, &mut v, vec![]); // settle panel geometry before button targeting
    let out = frame(&ctx, &mut v, vec![]);
    click(&ctx, &mut v, button(&out, "Copy cell → other"));
    ready(&mut v);
    assert_eq!(text(&v, 1, 0, 2), "1.0");
    assert_eq!(
        v.pair.as_ref().unwrap().map.statistics().important,
        1,
        "only the name case-difference remains important"
    );
    v.undo(false);
    ready(&mut v);
    assert_eq!(text(&v, 1, 0, 2), "1.2");
    // Export re-serializes the current view (no source-byte preservation).
    let out = std::env::temp_dir().join(format!(
        "matcha-lazy-gui-export-{}-{}.csv",
        std::process::id(),
        TAG.fetch_add(1, Ordering::Relaxed)
    ));
    v.save_to(1, &out, &Prefs::default(), true).unwrap();
    let bytes = std::fs::read_to_string(&out).unwrap();
    assert!(bytes.starts_with("id,name,price\r\n"));
    assert!(bytes.contains("7,apple,1.2\r\n"));
    let _ = std::fs::remove_file(&out);
}
