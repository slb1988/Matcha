//! Default-evaluation evidence: mixed storage backends (xlsx-dense ×
//! lazy-CSV) and regional settings on the demand store. No behavior changes;
//! these tests only add coverage.
use bc_core::cancel::NeverCancel;
use bc_engine_table::{
    demand::DemandSheet,
    map::TableStatistics,
    rules::KeySelection,
    source::{read_text, ReadOptions},
    store::SheetStore,
    workbook::Workbook,
    Sheet, TableMap, TableRules,
};
use bc_formats::model::{DateOrder, HeaderMode, RegionalSettings};
use std::{io::Cursor, sync::Arc};

#[path = "support/xlsx.rs"]
mod xlsx;

fn dense_csv(text: &str, header: HeaderMode) -> Sheet {
    let mut options = ReadOptions::default();
    options.header = Some(header);
    read_text(Cursor::new(text), &options, &NeverCancel)
        .unwrap()
        .sheet
}

fn lazy_csv(text: &str, header: HeaderMode) -> DemandSheet {
    let path = std::env::temp_dir().join(format!(
        "matcha-mixed-test-{}-{}.csv",
        std::process::id(),
        text.as_ptr() as usize
    ));
    std::fs::write(&path, text).unwrap();
    let mut options = ReadOptions::default();
    options.header = Some(header);
    let sheet = DemandSheet::open(&path, &options, &NeverCancel).unwrap();
    let _ = std::fs::remove_file(&path);
    sheet
}

fn stats(map: &TableMap) -> TableStatistics {
    map.statistics()
}

/// Sheet -> synthetic xlsx -> dense sheet via the production workbook reader.
fn via_xlsx(sheet: &Sheet) -> Sheet {
    let bytes = xlsx::workbook(&[sheet.clone()]);
    let mut wb = Workbook::from_reader(Cursor::new(bytes), Arc::new(NeverCancel)).unwrap();
    wb.load_sheet("Sheet1").unwrap()
}

/// id,name,amount with a few changed/inserted cells (typed values matter).
fn fixture() -> (String, String) {
    let mut left = String::new();
    let mut right = String::new();
    for i in 0..2000 {
        left.push_str(&format!("{i},item{i},{}.{:02}\n", i / 7, i % 100));
        let changed = i % 101 == 0;
        let name = if changed {
            "CHANGED".to_string()
        } else {
            format!("item{i}")
        };
        right.push_str(&format!(
            "{i},{name},{}{:02}\n",
            if changed { 9 } else { i / 7 },
            i % 100,
        ));
    }
    (left, right)
}

#[test]
fn mixed_xlsx_dense_and_lazy_csv_compare_equal_to_dense_dense() {
    let (left, right) = fixture();
    // Left: same content loaded as dense sheet directly and via xlsx.
    let xl_left = via_xlsx(&dense_csv(&left, HeaderMode::Data));
    let xl_right = via_xlsx(&dense_csv(&right, HeaderMode::Data));
    let rules = TableRules {
        keys: KeySelection::Manual(vec![0]),
        ..Default::default()
    };
    // Reference: dense xlsx × dense xlsx.
    let reference = TableMap::new(xl_left.clone(), xl_right.clone(), rules.clone(), &NeverCancel)
        .unwrap();
    // Mixed: dense xlsx × lazy CSV (the GUI's lazy-env mixed path).
    let mixed = TableMap::new_stores(
        SheetStore::Dense(xl_left),
        SheetStore::Lazy(lazy_csv(&right, HeaderMode::Data)),
        rules.clone(),
        &NeverCancel,
    )
    .unwrap();
    assert_eq!(stats(&mixed), stats(&reference));
    assert_eq!(mixed.row_count(), reference.row_count());
    for row in [0, 1, 100, 999, 1999] {
        for col in 0..3 {
            assert_eq!(
                mixed.cell_diff(row, col),
                reference.cell_diff(row, col),
                "cell diff mismatch at ({row}, {col})"
            );
        }
    }
    // And the mirror: lazy CSV × dense xlsx.
    let mirrored = TableMap::new_stores(
        SheetStore::Lazy(lazy_csv(&left, HeaderMode::Data)),
        SheetStore::Dense(xl_right),
        rules,
        &NeverCancel,
    )
    .unwrap();
    assert_eq!(stats(&mirrored), stats(&reference));
}

#[test]
fn regional_settings_on_lazy_match_dense() {
    // European layout: ';' delimiter, ',' decimal, '.' date separator, DMY.
    let mut left = String::new();
    let mut right = String::new();
    for i in 0..300 {
        let (tens, day) = (i % 10, i % 28 + 1);
        left.push_str(&format!("{i};1{tens},5;{day:02}.02.2024\n"));
        let amount = if i % 50 == 0 { "9,9" } else { "1,5" };
        right.push_str(&format!("{i};x{tens}{amount};{day:02}.02.2024\n"));
    }
    let regional = RegionalSettings {
        use_system: false,
        decimal: ',',
        thousands: None,
        date_order: DateOrder::Dmy,
        date_separator: '.',
    };
    let mut options = ReadOptions::default();
    options.header = Some(HeaderMode::Data);
    options.format.delimiters = vec![';'];
    options.format.regional = regional.clone();
    let dense_l = read_text(Cursor::new(&left), &options, &NeverCancel)
        .unwrap()
        .sheet;
    let dense_r = read_text(Cursor::new(&right), &options, &NeverCancel)
        .unwrap()
        .sheet;
    let path = std::env::temp_dir().join(format!(
        "matcha-regional-test-{}.csv",
        std::process::id()
    ));
    std::fs::write(&path, &right).unwrap();
    let lazy_r = DemandSheet::open(&path, &options, &NeverCancel).unwrap();
    let _ = std::fs::remove_file(&path);
    let rules = TableRules {
        keys: KeySelection::Manual(vec![0]),
        ..Default::default()
    };
    let dense = TableMap::new(dense_l, dense_r, rules.clone(), &NeverCancel).unwrap();
    let lazy = TableMap::new_stores(
        SheetStore::Dense(read_text(Cursor::new(&left), &options, &NeverCancel).unwrap().sheet),
        SheetStore::Lazy(lazy_r),
        rules,
        &NeverCancel,
    )
    .unwrap();
    assert_eq!(stats(&lazy), stats(&dense));
    assert!(stats(&dense).important > 0 || stats(&dense).same > 0);
}

/// 100k-row mixed comparison: correctness at scale + timing evidence.
#[test]
fn mixed_xlsx_and_lazy_csv_at_100k_rows() {
    let mut left = String::new();
    let mut right = String::new();
    for i in 0..100_000usize {
        left.push_str(&format!("{i},name{i},{},01.02.2024\n", i % 977));
        right.push_str(&format!(
            "{i},name{},{},01.02.2024\n",
            if i % 10_000 == 7 { i + 1 } else { i },
            (i % 977) + usize::from(i % 10_000 == 7),
        ));
    }
    let t = std::time::Instant::now();
    let xl = via_xlsx(&dense_csv(&left, HeaderMode::Data));
    eprintln!("[eval] build+parse 100k xlsx: {:?}", t.elapsed());
    let t = std::time::Instant::now();
    let lazy = lazy_csv(&right, HeaderMode::Data);
    eprintln!("[eval] lazy open 100k csv: {:?}", t.elapsed());
    let rules = TableRules {
        keys: KeySelection::Manual(vec![0]),
        ..Default::default()
    };
    let t = std::time::Instant::now();
    let mixed = TableMap::new_stores(
        SheetStore::Dense(xl),
        SheetStore::Lazy(lazy),
        rules.clone(),
        &NeverCancel,
    )
    .unwrap();
    eprintln!("[eval] mixed 100k align+compare: {:?}", t.elapsed());
    let reference = TableMap::new(
        dense_csv(&left, HeaderMode::Data),
        dense_csv(&right, HeaderMode::Data),
        rules,
        &NeverCancel,
    )
    .unwrap();
    assert_eq!(stats(&mixed), stats(&reference));
    assert_eq!(stats(&mixed).important, 10);
}
