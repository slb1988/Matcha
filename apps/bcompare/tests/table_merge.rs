//! 端到端：真实的 `matcha` 以 P4V 形式（`%1 %2 %b %r`）无窗口表格合并 xlsx / CSV，检查退出码与输出文件。
//! `/automerge` 在发起进程内执行，不打开窗口、不转交实例。
#![cfg(windows)]
use bc_engine_table::{workbook::Workbook, xlsx_write, Cell, CellValue, Sheet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

struct Dir(PathBuf);
impl Dir {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("matcha-e2e-tmerge-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(dir: &Path, args: &[&str]) -> Option<i32> {
    Command::new(env!("CARGO_BIN_EXE_matcha"))
        .args(args)
        .current_dir(dir)
        .env(
            "MATCHA_INSTANCE",
            format!("Matcha-test-tmerge-{}", std::process::id()),
        )
        .status()
        .unwrap()
        .code()
}

fn book(path: &Path, rows: &[(f64, &str, f64)]) {
    let mut sheet = Sheet::new("Stock");
    sheet.push_row(vec![
        Cell::text("id"),
        Cell::text("item"),
        Cell::text("qty"),
    ]);
    for (id, item, qty) in rows {
        sheet.push_row(vec![
            Cell::number(*id),
            Cell::text(*item),
            Cell::number(*qty),
        ]);
    }
    std::fs::write(path, xlsx_write::workbook_bytes(&[sheet], None).unwrap()).unwrap();
}

#[test]
fn p4v_style_xlsx_auto_merge_exit_codes() {
    let dir = Dir::new("xlsx");
    let base = [
        (1.0, "apple", 10.0),
        (2.0, "pear", 20.0),
        (3.0, "plum", 30.0),
    ];
    book(&dir.path("base.xlsx"), &base);
    book(
        &dir.path("theirs.xlsx"),
        &[
            (1.0, "apple", 11.0),
            (2.0, "pear", 20.0),
            (3.0, "plum", 30.0),
        ],
    );
    book(
        &dir.path("yours.xlsx"),
        &[
            (1.0, "Apple", 10.0),
            (2.0, "pear", 20.0),
            (3.0, "plum", 30.0),
            (4.0, "fig", 40.0),
        ],
    );
    // %1 %2 %b %r
    let args = [
        "/automerge",
        "theirs.xlsx",
        "yours.xlsx",
        "base.xlsx",
        "result.xlsx",
    ];
    assert_eq!(run(&dir.0, &args), Some(0));
    let mut out = Workbook::open(
        &dir.path("result.xlsx"),
        Arc::new(bc_core::cancel::NeverCancel),
    )
    .unwrap();
    let sheet = out.load_sheet("Stock").unwrap();
    assert_eq!(sheet.row_count(), 5);
    assert_eq!(
        sheet.cell(1, 1).unwrap().value,
        CellValue::Text("Apple".into())
    );
    assert_eq!(sheet.cell(1, 2).unwrap().value, CellValue::Number(11.0));

    // Same cell changed differently: nothing written, 101.
    book(
        &dir.path("yours.xlsx"),
        &[
            (1.0, "apple", 12.0),
            (2.0, "pear", 20.0),
            (3.0, "plum", 30.0),
        ],
    );
    std::fs::write(dir.path("result.xlsx"), b"previous").unwrap();
    assert_eq!(run(&dir.0, &args), Some(101));
    assert_eq!(std::fs::read(dir.path("result.xlsx")).unwrap(), b"previous");

    // No conflict markers for tables; bad keys; mixed formats.
    assert_eq!(
        run(
            &dir.0,
            &[
                "/automerge",
                "/force",
                "theirs.xlsx",
                "yours.xlsx",
                "base.xlsx",
                "r.xlsx"
            ]
        ),
        Some(100)
    );
    assert_eq!(
        run(
            &dir.0,
            &[
                "/automerge",
                "/keys=9",
                "theirs.xlsx",
                "yours.xlsx",
                "base.xlsx",
                "r.xlsx"
            ]
        ),
        Some(100)
    );
    std::fs::write(dir.path("base.csv"), "id\n1\n").unwrap();
    assert_eq!(
        run(
            &dir.0,
            &[
                "/automerge",
                "theirs.xlsx",
                "yours.xlsx",
                "base.csv",
                "r.xlsx"
            ]
        ),
        Some(100)
    );
    assert!(!dir.path("r.xlsx").exists());
}

#[test]
fn csv_merges_as_table_only_with_the_switch() {
    let dir = Dir::new("csv");
    std::fs::write(dir.path("base.csv"), "id,item,qty\n1,apple,10\n2,pear,20\n").unwrap();
    std::fs::write(
        dir.path("theirs.csv"),
        "id,item,qty\n1,apple,11\n2,pear,20\n",
    )
    .unwrap();
    std::fs::write(
        dir.path("yours.csv"),
        "id,item,qty\n1,Apple,10\n2,pear,20\n",
    )
    .unwrap();
    let files = ["theirs.csv", "yours.csv", "base.csv", "out.csv"];
    // As text both sides changed the same line: conflict.
    let text: Vec<&str> = ["/automerge"].into_iter().chain(files).collect();
    assert_eq!(run(&dir.0, &text), Some(101));
    let table: Vec<&str> = ["/automerge", "/tablemerge"]
        .into_iter()
        .chain(files)
        .collect();
    assert_eq!(run(&dir.0, &table), Some(0));
    assert_eq!(
        std::fs::read_to_string(dir.path("out.csv")).unwrap(),
        "id,item,qty\n1,Apple,11\n2,pear,20\n"
    );
}
