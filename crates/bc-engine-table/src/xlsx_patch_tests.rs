use super::*;
use crate::merge::{MergeInputs, TableMergeOptions};
use bc_core::cancel::NeverCancel;
use rust_xlsxwriter::{Chart, ChartType, Color, Format, Workbook};
use std::sync::Arc;

/// id, item, qty, price; `extra` adds rows; `price_fill` colors one price cell.
fn styled_book(ids: &[(u32, &str, f64, f64)], red_price_of: Option<u32>) -> Vec<u8> {
    let mut book = Workbook::new();
    let header = Format::new()
        .set_bold()
        .set_background_color(Color::RGB(0xDDEBF7));
    let money = Format::new().set_num_format("#,##0.00");
    let red_money = Format::new()
        .set_num_format("#,##0.00")
        .set_background_color(Color::RGB(0xFF0000));
    let total = Format::new().set_italic();
    let sheet = book.add_worksheet();
    sheet.set_name("Data").unwrap();
    sheet.set_column_width(1, 30).unwrap();
    sheet.set_freeze_panes(1, 0).unwrap();
    for (c, name) in ["id", "item", "qty", "price"].iter().enumerate() {
        sheet
            .write_string_with_format(0, c as u16, *name, &header)
            .unwrap();
    }
    for (i, (id, item, qty, price)) in ids.iter().enumerate() {
        let r = i as u32 + 1;
        sheet.write_number(r, 0, *id as f64).unwrap();
        sheet.write_string(r, 1, *item).unwrap();
        sheet.write_number(r, 2, *qty).unwrap();
        let fmt = if red_price_of == Some(*id) {
            &red_money
        } else {
            &money
        };
        sheet.write_number_with_format(r, 3, *price, fmt).unwrap();
    }
    let last = ids.len() as u32 + 1;
    sheet
        .merge_range(last, 0, last, 1, "Total", &total)
        .unwrap();
    sheet
        .write_formula(last, 2, format!("=SUM(C2:C{last})").as_str())
        .unwrap();
    let mut chart = Chart::new(ChartType::Column);
    chart
        .add_series()
        .set_values(format!("Data!$C$2:$C${last}").as_str());
    sheet.insert_chart(1, 6, &chart).unwrap();
    book.define_name("Qty", &format!("=Data!$C$2:$C${last}"))
        .unwrap();
    let notes = book.add_worksheet();
    notes.set_name("Notes").unwrap();
    notes.write_string(0, 0, "keep me").unwrap();
    book.save_to_buffer().unwrap()
}

fn load(bytes: &[u8]) -> Vec<Sheet> {
    let mut book =
        crate::workbook::Workbook::from_reader(Cursor::new(bytes.to_vec()), Arc::new(NeverCancel))
            .unwrap();
    let names = book.sheet_names().to_vec();
    names.iter().map(|n| book.load_sheet(n).unwrap()).collect()
}

fn entry(bytes: &[u8], name: &str) -> String {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    read_text(&mut zip, name).unwrap_or_else(|| panic!("no {name}"))
}

/// Raw `<xf>` of the style a cell uses, and the font/fill it points at.
fn cell_style(bytes: &[u8], sheet: &str, at: &str) -> (String, String, String) {
    let xml = entry(bytes, sheet);
    let span = spans(&xml, "c")
        .into_iter()
        .find(|s| parse_tag(&xml[s.clone()]).get("r") == Some(at))
        .unwrap_or_else(|| panic!("no cell {at}"));
    let index: usize = parse_tag(&xml[span])
        .get("s")
        .unwrap_or("0")
        .parse()
        .unwrap();
    let styles = Styles::parse(&entry(bytes, "xl/styles.xml"));
    let xf = parse_tag(&styles.xfs[index]);
    let font = styles.fonts[xf.get("fontId").unwrap_or("0").parse::<usize>().unwrap()].clone();
    let fill = styles.fills[xf.get("fillId").unwrap_or("0").parse::<usize>().unwrap()].clone();
    (styles.xfs[index].clone(), font, fill)
}

#[test]
fn merged_workbook_keeps_layout_styles_and_repoints_rows() {
    let base_rows = [
        (1, "apple", 10.0, 1.5),
        (2, "pear", 20.0, 2.0),
        (3, "plum", 30.0, 3.0),
        (4, "fig", 40.0, 4.0),
        (5, "kiwi", 50.0, 5.0),
    ];
    let center = styled_book(&base_rows, None);
    // Left deletes id 2; right changes id 4's price with a new red style and adds id 9.
    let left_rows: Vec<_> = base_rows.iter().copied().filter(|r| r.0 != 2).collect();
    let left = styled_book(&left_rows, None);
    let mut right_rows = base_rows.to_vec();
    right_rows[3].3 = 4.5;
    right_rows.push((9, "lime", 90.0, 9.0));
    let right = styled_book(&right_rows, Some(4));

    let inputs = MergeInputs {
        left: load(&left),
        center: load(&center),
        right: load(&right),
    };
    let options = TableMergeOptions {
        keys: Some(vec![0]),
        ..Default::default()
    };
    let merge = TableMerge::new(inputs, &options).unwrap();
    // The total row's formula reads C2:C6 / C2:C5 / C2:C7 on center / left / right, but
    // re-pointed into the output only the right changed it (its range took in id 9).
    assert_eq!(merge.unresolved(), 0, "{:?}", merge.sheets[0].conflicts());
    let sheets = merge.output();
    let (bytes, warnings) =
        patch_workbook(&merge, &sheets, [&left, &center, &right], true).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    if let Some(path) = std::env::var_os("MATCHA_PATCH_KEEP") {
        std::fs::write(path, &bytes).unwrap();
    }

    // Values.
    let out = load(&bytes);
    let data = &out[0];
    let ids: Vec<String> = (0..data.row_count())
        .map(|r| data.cell(r, 0).map(|c| c.display()).unwrap_or_default())
        .collect();
    assert_eq!(ids, ["id", "1", "3", "4", "5", "9", "Total"]);
    assert_eq!(data.cell(3, 3).unwrap().value, CellValue::Number(4.5));
    // The right's total (its range grew over id 9) re-pointed: the left's row 3 is gone.
    assert_eq!(
        data.cell(6, 2).unwrap().formula.as_deref(),
        Some("=SUM(C2:C6)")
    );
    assert_eq!(out[1].cell(0, 0).unwrap().display(), "keep me");

    let sheet_xml = entry(&bytes, "xl/worksheets/sheet1.xml");
    // Layout kept: column width, frozen pane, merged total row (moved with its row).
    assert!(sheet_xml.contains("<cols>"), "{sheet_xml}");
    assert!(sheet_xml.contains("<pane"), "{sheet_xml}");
    assert!(
        sheet_xml.contains("<mergeCell ref=\"A7:B7\"/>"),
        "{sheet_xml}"
    );
    assert!(sheet_xml.contains("<drawing"), "chart anchor kept");
    // Styles: bold filled header, number format on prices, the right's new red fill.
    let (_, font, fill) = cell_style(&bytes, "xl/worksheets/sheet1.xml", "A1");
    assert!(font.contains("<b/>"), "{font}");
    assert!(fill.contains("DDEBF7"), "{fill}");
    let (xf, _, fill) = cell_style(&bytes, "xl/worksheets/sheet1.xml", "D4");
    assert!(
        fill.contains("FF0000"),
        "right's style carried over: {fill}"
    );
    assert!(
        xf.contains("numFmtId=\"164\"") || xf.contains("numFmtId=\"4\""),
        "{xf}"
    );
    let (_, font, _) = cell_style(&bytes, "xl/worksheets/sheet1.xml", "A7");
    assert!(font.contains("<i/>"), "{font}");
    // Defined name and chart series follow the rows; recalculation forced; no calc chain.
    let workbook = entry(&bytes, "xl/workbook.xml");
    assert!(workbook.contains("Data!$C$2:$C$5"), "{workbook}");
    assert!(workbook.contains("fullCalcOnLoad=\"1\""), "{workbook}");
    let chart = entry(&bytes, "xl/charts/chart1.xml");
    assert!(chart.contains("Data!$C$2:$C$5"), "{chart}");
}

#[test]
fn sheet_only_one_side_has_is_added_and_deleted_sheet_dropped() {
    let rows = [(1, "apple", 10.0, 1.5)];
    let center = styled_book(&rows, None);
    let mut book = Workbook::new();
    let bold = Format::new().set_bold();
    let a = book.add_worksheet();
    a.set_name("Data").unwrap();
    a.write_string(0, 0, "id").unwrap();
    let extra = book.add_worksheet();
    extra.set_name("Extra").unwrap();
    extra
        .write_string_with_format(0, 0, "new sheet", &bold)
        .unwrap();
    let left = book.save_to_buffer().unwrap();
    // Left: Data rewritten (conflicts resolved to center below), Notes deleted, Extra added.
    let inputs = MergeInputs {
        left: load(&left),
        center: load(&center),
        right: load(&center),
    };
    let mut merge = TableMerge::new(inputs, &TableMergeOptions::default()).unwrap();
    for sheet in &mut merge.sheets {
        sheet.take(0..sheet.rows.len(), Side::Right);
    }
    let sheets = merge.output();
    let names: Vec<_> = sheets.iter().map(|s| s.name.clone()).collect();
    assert_eq!(names, ["Data", "Extra"]);
    let (bytes, _) = patch_workbook(&merge, &sheets, [&left, &center, &center], true).unwrap();
    let mut book =
        crate::workbook::Workbook::from_reader(Cursor::new(bytes.clone()), Arc::new(NeverCancel))
            .unwrap();
    assert_eq!(book.sheet_names(), ["Data", "Extra"]);
    assert_eq!(
        book.load_sheet("Extra")
            .unwrap()
            .cell(0, 0)
            .unwrap()
            .display(),
        "new sheet"
    );
    let types = entry(&bytes, "[Content_Types].xml");
    assert!(types.contains("sheet_merged1.xml"), "{types}");
    assert!(!types.contains("sheet2.xml"), "Notes part removed: {types}");
}
