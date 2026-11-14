//! Writing merged sheets as a new xlsx workbook (`rust_xlsxwriter`).
//!
//! Kept: cell values, formulas (with the cached result), number formats, sheet names
//! and order, and a VBA project handed in by the caller (`.xlsm`). Not kept: fonts,
//! fills, borders, column widths, merged cells, charts, images, defined names.
use crate::{invalid, model::CellValue, Result, Sheet};
use rust_xlsxwriter::{Format, Formula, Workbook};
use std::{collections::HashMap, io::Read, path::Path};

/// Excel serial day of the Unix epoch (1970-01-01) in the 1900 date system.
const UNIX_EPOCH_SERIAL: f64 = 25569.0;
const DAY_MS: f64 = 86_400_000.0;

fn error(e: impl std::fmt::Display) -> bc_core::CoreError {
    invalid(format!("cannot write workbook: {e}"))
}

/// `vbaProject.bin` of an xlsm workbook, if it has one.
pub fn vba_project(path: &Path) -> Result<Option<Vec<u8>>> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(std::io::BufReader::new(file))
        .map_err(|e| invalid(format!("{}: {e}", path.display())))?;
    let Ok(mut entry) = archive.by_name("xl/vbaProject.bin") else {
        return Ok(None);
    };
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut bytes)?;
    Ok(Some(bytes))
}

fn format_for(code: &str) -> Format {
    match code.strip_prefix("builtin:").map(str::parse::<u8>) {
        Some(Ok(index)) => Format::new().set_num_format_index(index),
        _ => Format::new().set_num_format(code),
    }
}

/// Serialize `sheets` as an xlsx workbook. `vba` makes it a macro workbook (`.xlsm`).
pub fn workbook_bytes(sheets: &[Sheet], vba: Option<&[u8]>) -> Result<Vec<u8>> {
    let mut book = Workbook::new();
    let mut vba_file = None;
    if let Some(bytes) = vba {
        // rust_xlsxwriter reads the project from a file.
        let path = std::env::temp_dir().join(format!(
            "matcha-vba-{}-{}.bin",
            std::process::id(),
            crate::model::revision()
        ));
        std::fs::write(&path, bytes)?;
        let added = book.add_vba_project(&path).map(|_| ()).map_err(error);
        vba_file = Some(path);
        if let Err(e) = added {
            let _ = std::fs::remove_file(vba_file.as_ref().unwrap());
            return Err(e);
        }
    }
    let result = (|| {
        for sheet in sheets {
            let worksheet = book.add_worksheet();
            worksheet.set_name(&sheet.name).map_err(error)?;
            let mut formats: HashMap<String, Format> = HashMap::new();
            let date_format = Format::new().set_num_format("yyyy-mm-dd");
            let datetime_format = Format::new().set_num_format("yyyy-mm-dd hh:mm:ss");
            let mut write_row = |row: u32,
                                 cells: &mut dyn Iterator<Item = (u16, &crate::Cell)>|
             -> Result<()> {
                for (column, cell) in cells {
                    let code = sheet
                        .style(cell.style)
                        .and_then(|s| s.number_format.as_deref())
                        .map(str::to_owned);
                    let format = code.map(|code| {
                        formats
                            .entry(code.clone())
                            .or_insert_with(|| format_for(&code))
                            .clone()
                    });
                    let format = match (&format, &cell.value) {
                        (Some(f), _) => Some(f.clone()),
                        (None, CellValue::Date(ms)) if ms.rem_euclid(DAY_MS as i64) == 0 => {
                            Some(date_format.clone())
                        }
                        (None, CellValue::Date(_)) => Some(datetime_format.clone()),
                        _ => None,
                    };
                    if let Some(text) = cell.formula.as_deref().filter(|f| !f.trim().is_empty()) {
                        let mut formula = Formula::new(text);
                        if !matches!(cell.value, CellValue::Empty) {
                            formula = formula.set_result(cell.display());
                        }
                        match &format {
                            Some(f) => worksheet.write_formula_with_format(row, column, formula, f),
                            None => worksheet.write_formula(row, column, formula),
                        }
                        .map_err(error)?;
                        continue;
                    }
                    match &cell.value {
                        CellValue::Empty => {
                            if let Some(f) = &format {
                                worksheet.write_blank(row, column, f).map_err(error)?;
                            }
                        }
                        CellValue::Text(text) | CellValue::Error(text) => {
                            match &format {
                                Some(f) => worksheet.write_string_with_format(
                                    row,
                                    column,
                                    text.as_ref(),
                                    f,
                                ),
                                None => worksheet.write_string(row, column, text.as_ref()),
                            }
                            .map_err(error)?;
                        }
                        CellValue::Number(n) => {
                            match &format {
                                Some(f) => worksheet.write_number_with_format(row, column, *n, f),
                                None => worksheet.write_number(row, column, *n),
                            }
                            .map_err(error)?;
                        }
                        CellValue::Bool(b) => {
                            match &format {
                                Some(f) => worksheet.write_boolean_with_format(row, column, *b, f),
                                None => worksheet.write_boolean(row, column, *b),
                            }
                            .map_err(error)?;
                        }
                        CellValue::Date(ms) => {
                            let serial = *ms as f64 / DAY_MS + UNIX_EPOCH_SERIAL;
                            worksheet
                                .write_number_with_format(
                                    row,
                                    column,
                                    serial,
                                    format.as_ref().unwrap_or(&date_format),
                                )
                                .map_err(error)?;
                        }
                    }
                }
                Ok(())
            };
            let offset = u32::from(sheet.headers.is_some());
            if let Some(headers) = &sheet.headers {
                let cells: Vec<crate::Cell> = headers
                    .iter()
                    .map(|h| crate::Cell::text(h.as_str()))
                    .collect();
                write_row(
                    0,
                    &mut cells.iter().enumerate().map(|(c, cell)| (c as u16, cell)),
                )?;
            }
            for row in 0..sheet.row_count() {
                let r = u32::try_from(row)
                    .ok()
                    .and_then(|r| r.checked_add(offset))
                    .ok_or_else(|| invalid("too many rows for a worksheet"))?;
                let mut cells = (0..sheet.column_count()).filter_map(|c| {
                    let cell = sheet.cell(row, c)?;
                    let keep = !matches!(cell.value, CellValue::Empty)
                        || cell.formula.is_some()
                        || cell.style.is_some();
                    (keep && c <= u16::MAX as usize).then_some((c as u16, cell))
                });
                write_row(r, &mut cells)?;
            }
        }
        book.save_to_buffer().map_err(error)
    })();
    if let Some(path) = vba_file {
        let _ = std::fs::remove_file(path);
    }
    result
}
