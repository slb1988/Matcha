use super::*;
use bc_engine_table::{Cell, CellValue};

struct Dir(PathBuf);
impl Dir {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("matcha-table-merge-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn spec(dir: &Dir, l: &str, r: &str, c: &str, ext: &str) -> MergeSpec {
    MergeSpec {
        left: dir.file(&format!("left.{ext}"), l.as_bytes()),
        right: dir.file(&format!("right.{ext}"), r.as_bytes()),
        center: dir.file(&format!("base.{ext}"), c.as_bytes()),
        output: Some(dir.0.join(format!("out.{ext}"))),
    }
}

const BASE: &str = "id,name,qty\r\n1,apple,10\r\n2,pear,20\r\n3,plum,30\r\n";

#[test]
fn csv_auto_merge_writes_both_sides_changes() {
    let dir = Dir::new("csv-ok");
    let left = "id,name,qty\r\n1,apple,11\r\n2,pear,20\r\n3,plum,30\r\n";
    let right = "id,name,qty\r\n1,apple,10\r\n2,pear,20\r\n3,plum,30\r\n4,fig,40\r\n";
    let spec = spec(&dir, left, right, BASE, "csv");
    let report = auto_merge_tables(&spec, &TableMergeOptions::default()).unwrap();
    assert_eq!(report.code, ExitCode::Success);
    assert!(
        report.summary().contains("keys: 1 (inferred)"),
        "{}",
        report.summary()
    );
    let out = std::fs::read_to_string(spec.output.unwrap()).unwrap();
    assert_eq!(
        out,
        "id,name,qty\r\n1,apple,11\r\n2,pear,20\r\n3,plum,30\r\n4,fig,40\r\n"
    );
}

#[test]
fn csv_conflict_withholds_and_leaves_existing_output_alone() {
    let dir = Dir::new("csv-conflict");
    let left = BASE.replace("apple,10", "apple,12");
    let right = BASE.replace("apple,10", "apple,15");
    let spec = spec(&dir, &left, &right, BASE, "csv");
    let output = spec.output.clone().unwrap();
    std::fs::write(&output, "keep me").unwrap();
    let report = auto_merge_tables(&spec, &TableMergeOptions::default()).unwrap();
    assert_eq!(report.code, ExitCode::ConflictsMergeNotWritten);
    assert_eq!(report.conflicts, 1);
    assert!(!report.written);
    assert_eq!(std::fs::read_to_string(output).unwrap(), "keep me");
}

#[test]
fn explicit_key_out_of_range_is_an_error() {
    let dir = Dir::new("csv-keys");
    let spec = spec(&dir, BASE, BASE, BASE, "csv");
    let options = TableMergeOptions {
        keys: Some(vec![98]),
        ..Default::default()
    };
    let error = auto_merge_tables(&spec, &options).unwrap_err();
    assert!(error.contains("key column 99"), "{error}");
}

fn workbook(rows: &[(&str, f64)]) -> Vec<u8> {
    let mut sheet = bc_engine_table::Sheet::new("Items");
    sheet.push_row(vec![Cell::text("name"), Cell::text("qty")]);
    for (name, qty) in rows {
        sheet.push_row(vec![Cell::text(*name), Cell::number(*qty)]);
    }
    let mut extra = bc_engine_table::Sheet::new("Notes");
    extra.push_row(vec![Cell::text("hello")]);
    xlsx_write::workbook_bytes(&[sheet, extra], None).unwrap()
}

#[test]
fn workbook_merge_by_format_and_output_overwriting_an_input() {
    let dir = Dir::new("xlsx");
    let base = workbook(&[("apple", 10.0), ("pear", 20.0)]);
    let left = workbook(&[("apple", 11.0), ("pear", 20.0)]);
    let right = workbook(&[("apple", 10.0), ("pear", 20.0), ("fig", 40.0)]);
    // VCS temp names: the extension is not xlsx, the content is.
    let spec = MergeSpec {
        left: dir.file("left.xlsx#3", &left),
        right: dir.file("right.xlsx", &right),
        center: dir.file("base.xlsx", &base),
        output: Some(dir.0.join("right.xlsx")),
    };
    assert_eq!(merge_kind(&spec, false), Ok(MergeKind::Table));
    let report = auto_merge_tables(&spec, &TableMergeOptions::default()).unwrap();
    assert_eq!(report.code, ExitCode::Success, "{}", report.summary());
    // Written from the center package: no "without styles" fallback.
    assert!(
        !report.summary().contains("without styles"),
        "{}",
        report.summary()
    );
    let mut book = Workbook::open(&dir.0.join("right.xlsx"), Arc::new(NeverCancel)).unwrap();
    assert_eq!(book.sheet_names(), ["Items", "Notes"]);
    let items = book.load_sheet("Items").unwrap();
    assert_eq!(items.row_count(), 4);
    assert_eq!(items.cell(1, 1).unwrap().value, CellValue::Number(11.0));
    assert_eq!(
        items.cell(3, 0).unwrap().value,
        CellValue::Text("fig".into())
    );
}

#[test]
fn merge_kind_dispatch() {
    let dir = Dir::new("kind");
    let csv = spec(&dir, BASE, BASE, BASE, "csv");
    assert_eq!(merge_kind(&csv, false), Ok(MergeKind::Text));
    assert_eq!(merge_kind(&csv, true), Ok(MergeKind::Table));
    let mixed = MergeSpec {
        left: dir.file("a.xlsx", &workbook(&[])),
        ..csv.clone()
    };
    assert!(merge_kind(&mixed, false)
        .unwrap_err()
        .contains("different formats"));
    let xls = MergeSpec {
        left: dir.0.join("a.xls"),
        ..csv
    };
    assert!(merge_kind(&xls, true)
        .unwrap_err()
        .contains("not supported"));
}
