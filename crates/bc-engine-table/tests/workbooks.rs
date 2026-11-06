use bc_core::{cancel::NeverCancel, hash::Crc32, CoreError};
use bc_engine_table::{
    source::{export_csv, write_delimited},
    workbook::Workbook,
    Cell, CellValue,
};
use std::{io::Cursor, sync::Arc};

// Minimal ZIP writer for stored entries: generated XML fixtures, no binary fixture
// or new dev dependency. Central directory CRC/size/offsets are real.
fn zip(entries: &[(&str, String)]) -> Vec<u8> {
    fn u16le(out: &mut Vec<u8>, v: u16) {
        out.extend(v.to_le_bytes());
    }
    fn u32le(out: &mut Vec<u8>, v: u32) {
        out.extend(v.to_le_bytes());
    }
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let offset = out.len() as u32;
        let mut crc = Crc32::new();
        crc.update(data.as_bytes());
        let crc = crc.finish();
        u32le(&mut out, 0x04034b50);
        u16le(&mut out, 20);
        for _ in 0..4 {
            u16le(&mut out, 0);
        }
        u32le(&mut out, crc);
        u32le(&mut out, data.len() as u32);
        u32le(&mut out, data.len() as u32);
        u16le(&mut out, name.len() as u16);
        u16le(&mut out, 0);
        out.extend(name.as_bytes());
        out.extend(data.as_bytes());
        u32le(&mut central, 0x02014b50);
        u16le(&mut central, 20);
        u16le(&mut central, 20);
        for _ in 0..4 {
            u16le(&mut central, 0);
        }
        u32le(&mut central, crc);
        u32le(&mut central, data.len() as u32);
        u32le(&mut central, data.len() as u32);
        u16le(&mut central, name.len() as u16);
        for _ in 0..4 {
            u16le(&mut central, 0);
        }
        u32le(&mut central, 0);
        u32le(&mut central, offset);
        central.extend(name.as_bytes());
    }
    let offset = out.len();
    let size = central.len();
    out.extend(central);
    u32le(&mut out, 0x06054b50);
    u16le(&mut out, 0);
    u16le(&mut out, 0);
    u16le(&mut out, entries.len() as u16);
    u16le(&mut out, entries.len() as u16);
    u32le(&mut out, size as u32);
    u32le(&mut out, offset as u32);
    u16le(&mut out, 0);
    out
}
fn workbook(epoch1904: bool, third: String) -> Vec<u8> {
    zip(&[
        ("[Content_Types].xml",r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/></Types>"#.into()),
        ("_rels/.rels",r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.into()),
        ("xl/workbook.xml",format!(r#"<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><workbookPr date1904="{}"/><sheets><sheet name="Data" sheetId="1" r:id="rId1"/><sheet name="Summary" sheetId="2" r:id="rId2"/><sheet name="Third" sheetId="3" r:id="rId3"/></sheets></workbook>"#,u8::from(epoch1904))),
        ("xl/_rels/workbook.xml.rels",r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet3.xml"/></Relationships>"#.into()),
        ("xl/styles.xml",r#"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><numFmts count="1"><numFmt numFmtId="164" formatCode="0.000"/></numFmts><cellXfs count="4"><xf numFmtId="0"/><xf numFmtId="14"/><xf numFmtId="2"/><xf numFmtId="164"/></cellXfs></styleSheet>"#.into()),
        ("xl/worksheets/sheet1.xml",r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><dimension ref="A1:H2"/><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>hello</t></is></c><c r="B1"><f>SUM(A2:A4)</f><v>6</v></c><c r="C1" s="1"><v>46023</v></c><c r="D1" t="b"><v>1</v></c><c r="E1" t="e"><v>#DIV/0!</v></c><c r="F1" s="2"><v>1</v></c><c r="G1" s="3"><v>1</v></c><c r="H1"><f>1+1</f></c></row><row r="2"><c r="A2"><v>0</v></c></row></sheetData><mergeCells count="1"><mergeCell ref="B2:C2"/></mergeCells></worksheet>"#.into()),
        ("xl/worksheets/sheet2.xml",r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><dimension ref="B2"/><sheetData><row r="2"><c r="B2" t="inlineStr"><is><t>second</t></is></c></row></sheetData></worksheet>"#.into()),
        ("xl/worksheets/sheet3.xml",third),
    ])
}
#[test]
fn native_workbook_types_formulas_styles_merges_and_lazy_sheets() {
    let mut wb = Workbook::from_reader(
        Cursor::new(workbook(false, "broken".into())),
        Arc::new(NeverCancel),
    )
    .unwrap();
    assert_eq!(wb.sheet_names(), &["Data", "Summary", "Third"]);
    let mut s = wb.load_sheet("Data").unwrap();
    assert_eq!(s.cell(0, 0).unwrap().display(), "hello");
    assert_eq!(s.cell(0, 1).unwrap().value, CellValue::Number(6.0));
    assert_eq!(
        s.cell(0, 1).unwrap().formula.as_deref(),
        Some("=SUM(A2:A4)")
    );
    assert_eq!(s.cell(0, 2).unwrap().display(), "2026-01-01");
    assert_eq!(s.cell(0, 3).unwrap().value, CellValue::Bool(true));
    assert_eq!(s.cell(0, 4).unwrap().display(), "#DIV/0!");
    assert_eq!(s.cell(0, 7).unwrap().formula.as_deref(), Some("=1+1"));
    assert_eq!(s.merged_ranges[0].start, (1, 1));
    assert_eq!(s.merged_ranges[0].end, (1, 2));
    let f = s.style(s.cell(0, 5).unwrap().style).unwrap();
    let g = s.style(s.cell(0, 6).unwrap().style).unwrap();
    assert_ne!(f, g);
    assert_eq!(g.number_format.as_deref(), Some("0.000"));
    let second = wb.load_sheet("Summary").unwrap();
    assert_eq!(second.cell(1, 1).unwrap().display(), "second");
    assert!(wb.load_sheet("Third").is_err()); // invalid sheet wasn't touched earlier
    s.set_cell(0, 0, Cell::text("changed")).unwrap();
    assert!(write_delimited(&s, Vec::new())
        .unwrap_err()
        .to_string()
        .contains("read-only"));
    let mut csv = Vec::new();
    export_csv(&s, &mut csv).unwrap();
    assert!(csv.starts_with(b"changed,6,2026-01-01"));
}
#[test]
fn epoch_1904_and_cancelled_load_do_not_return_partial_sheet() {
    let bytes = workbook(true, "broken".into());
    let mut wb = Workbook::from_reader(Cursor::new(&bytes), Arc::new(NeverCancel)).unwrap();
    let s = wb.load_sheet("Data").unwrap();
    assert_eq!(s.cell(0, 2).unwrap().display(), "2030-01-02");
    let cancel = bc_jobs::CancelToken::new();
    let mut wb = Workbook::from_reader(Cursor::new(bytes), Arc::new(cancel.clone())).unwrap();
    cancel.cancel();
    assert!(matches!(wb.load_sheet("Data"), Err(CoreError::Cancelled)));
    assert!(matches!(
        Workbook::from_reader(Cursor::new(Vec::<u8>::new()), Arc::new(cancel)),
        Err(CoreError::Cancelled)
    ));
}
#[test]
#[ignore = "100 MB lazy/cancellation stress; run explicitly in release"]
fn hundred_mb_workbook_is_lazy_and_io_is_cancellable() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let sheet = format!(
        "<worksheet><sheetData>{}</sheetData></worksheet>",
        " ".repeat(100_000_000)
    );
    let bytes = workbook(false, sheet);
    let enabled = Arc::new(AtomicBool::new(false));
    let reads = Arc::new(AtomicUsize::new(0));
    let flag = enabled.clone();
    let count = reads.clone();
    let check = Arc::new(move || {
        flag.load(Ordering::Relaxed) && count.fetch_add(1, Ordering::Relaxed) > 20
    });
    let mut wb = Workbook::from_reader(Cursor::new(bytes), check).unwrap();
    assert_eq!(wb.load_sheet("Data").unwrap().row_count(), 2);
    enabled.store(true, Ordering::Relaxed);
    assert!(matches!(wb.load_sheet("Third"), Err(CoreError::Cancelled)));
}
