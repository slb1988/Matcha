use bc_core::hash::Crc32;
use bc_engine_table::{CellValue, Sheet};
pub fn stored_zip(entries: &[(&str, String)]) -> Vec<u8> {
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
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
pub fn workbook(sheets: &[Sheet]) -> Vec<u8> {
    let names = sheets
        .iter()
        .enumerate()
        .map(|(i, s)| {
            format!(
                "<sheet name=\"{}\" sheetId=\"{}\" r:id=\"rId{}\"/>",
                escape(&s.name),
                i + 1,
                i + 1
            )
        })
        .collect::<String>();
    let rels=(1..=sheets.len()).map(|i|format!("<Relationship Id=\"rId{i}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet{i}.xml\"/>")).collect::<String>();
    let mut entries=vec![
        ("[Content_Types].xml".to_string(),"<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"xml\" ContentType=\"application/xml\"/></Types>".to_string()),
        ("_rels/.rels".into(),"<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>".into()),
        ("xl/workbook.xml".into(),format!("<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets>{names}</sheets></workbook>")),
        ("xl/_rels/workbook.xml.rels".into(),format!("<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{rels}</Relationships>")),
    ];
    for (index, sheet) in sheets.iter().enumerate() {
        let mut xml=String::from("<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>");
        for row in 0..sheet.row_count() {
            xml.push_str(&format!("<row r=\"{}\">", row + 1));
            for col in 0..sheet.column_count() {
                let mut n = col + 1;
                let mut letters = Vec::new();
                while n > 0 {
                    letters.push((b'A' + ((n - 1) % 26) as u8) as char);
                    n = (n - 1) / 26;
                }
                let letters = letters.iter().rev().collect::<String>();
                let pos = format!("{letters}{}", row + 1);
                let cell = sheet.cell(row, col).unwrap();
                let value = sheet.display_at(row, col).unwrap_or_default();
                let body=match &cell.value{
                    CellValue::Number(n)=>format!("<c r=\"{pos}\"><v>{n}</v></c>"),
                    CellValue::Bool(b)=>format!("<c r=\"{pos}\" t=\"b\"><v>{}</v></c>",u8::from(*b)),
                    _ if ["#N/A","#VALUE!"].contains(&value.as_str())=>format!("<c r=\"{pos}\" t=\"e\"><v>{}</v></c>",escape(&value)),
                    _=>format!("<c r=\"{pos}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>",escape(&value)),
                };
                xml.push_str(&body);
            }
            xml.push_str("</row>");
        }
        xml.push_str("</sheetData></worksheet>");
        entries.push((format!("xl/worksheets/sheet{}.xml", index + 1), xml));
    }
    stored_zip(
        &entries
            .iter()
            .map(|(n, v)| (n.as_str(), v.clone()))
            .collect::<Vec<_>>(),
    )
}
