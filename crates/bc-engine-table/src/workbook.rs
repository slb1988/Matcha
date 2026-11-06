//! Lazy, cancellable xlsx/xlsm loading; no formula evaluation or workbook writes.
use crate::{
    invalid,
    model::{SheetSource, StringPool},
    Cell, CellValue, MergedRange, Result, Sheet, Style,
};
use bc_core::cancel::CancelCheck;
use calamine::{DataRef, Reader, Xlsx};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
    sync::Arc,
};

struct CheckedReader<R> {
    inner: R,
    cancel: Arc<dyn CancelCheck>,
}
impl<R: Read> Read for CheckedReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.cancel.is_cancelled() {
            return Err(io::Error::other("cancelled"));
        }
        self.inner.read(buffer)
    }
}
impl<R: Seek> Seek for CheckedReader<R> {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        if self.cancel.is_cancelled() {
            return Err(io::Error::other("cancelled"));
        }
        self.inner.seek(from)
    }
}
pub struct Workbook<R: Read + Seek = File> {
    reader: Xlsx<CheckedReader<R>>,
    cancel: Arc<dyn CancelCheck>,
    names: Vec<String>,
}
fn mapped(error: impl std::fmt::Display, cancel: &dyn CancelCheck) -> bc_core::CoreError {
    if cancel.is_cancelled() {
        bc_core::CoreError::Cancelled
    } else {
        invalid(error.to_string())
    }
}
impl Workbook<io::BufReader<File>> {
    pub fn open(path: &Path, cancel: Arc<dyn CancelCheck>) -> Result<Self> {
        // Zip entry streams are read sequentially in small chunks; buffer the
        // file so decompression does not pay a syscall per chunk.
        Self::from_reader(io::BufReader::with_capacity(4 << 20, File::open(path)?), cancel)
    }
}
impl<R: Read + Seek> Workbook<R> {
    pub fn from_reader(reader: R, cancel: Arc<dyn CancelCheck>) -> Result<Self> {
        cancel.check()?;
        let t_open = std::time::Instant::now();
        let reader = Xlsx::new(CheckedReader {
            inner: reader,
            cancel: cancel.clone(),
        })
        .map_err(|e| mapped(e, &*cancel))?;
        crate::perf::span("workbook:open(xlsx meta+shared strings)", t_open);
        let names = reader.sheet_names();
        Ok(Self {
            reader,
            cancel,
            names,
        })
    }
    pub fn sheet_names(&self) -> &[String] {
        &self.names
    }
    /// Values+styles pass only; see `load_sheet_parallel` for the full sheet.
    fn load_sheet_values(&mut self, name: &str) -> Result<Sheet> {
        self.cancel.check()?;
        let mut sheet = Sheet::new(name);
        sheet.source = SheetSource::Workbook;
        let mut pool = StringPool::default();
        let t_values = std::time::Instant::now();
        {
            let mut cells = self
                .reader
                .worksheet_cells_reader(name)
                .map_err(|e| mapped(e, &*self.cancel))?;
            let style_ids: Vec<_> = cells
                .styles()
                .iter()
                .map(|s| {
                    let format = s.number_format.as_ref().map(|f| {
                        if f.format_code.is_empty() {
                            format!("builtin:{}", f.format_id.unwrap_or(0))
                        } else {
                            f.format_code.clone()
                        }
                    });
                    format.filter(|f| f != "General").map(|f| {
                        sheet.intern_style(Style {
                            number_format: Some(f.into()),
                        })
                    })
                })
                .collect();
            while let Some((data, style)) = cells
                .next_cell_with_style_id()
                .map_err(|e| mapped(e, &*self.cancel))?
            {
                self.cancel.check()?;
                let (r, c) = data.get_position();
                let value = match data.get_value() {
                    DataRef::Empty => CellValue::Empty,
                    DataRef::Int(v) => CellValue::Number(*v as f64),
                    DataRef::Float(v) => CellValue::Number(*v),
                    DataRef::String(v) => CellValue::Text(pool.intern(v)),
                    DataRef::SharedString(v) => CellValue::Text(pool.intern(v)),
                    DataRef::Bool(v) => CellValue::Bool(*v),
                    DataRef::Error(v) => CellValue::Error(v.to_string().into()),
                    DataRef::DateTime(v) if v.is_datetime() => {
                        let (y, m, d, h, min, s, ms) = v.to_ymd_hms_milli();
                        let date = crate::regional::date_ms(y as i32, m as i32, d as i32)
                            .ok_or_else(|| {
                                invalid(
                                    "unsupported workbook date (including fictitious 1900-02-29)",
                                )
                            })?;
                        CellValue::Date(
                            date + i64::from(h) * 3_600_000
                                + i64::from(min) * 60_000
                                + i64::from(s) * 1000
                                + i64::from(ms),
                        )
                    }
                    DataRef::DateTime(v) => CellValue::Number(v.as_f64()),
                    DataRef::DateTimeIso(v) => crate::regional::parse_date(v, &sheet.regional)
                        .map(CellValue::Date)
                        .unwrap_or_else(|| CellValue::Text(pool.intern(v))),
                    DataRef::DurationIso(v) => CellValue::Text(pool.intern(v)),
                };
                sheet.put_loaded(
                    r as usize,
                    c as usize,
                    Cell {
                        value,
                        formula: None,
                        style: style_ids.get(style).copied().flatten(),
                    },
                );
            }
        }
        crate::perf::span("workbook:values-pass", t_values);
        Ok(sheet)
    }
    /// Formula pass over the same sheet, collecting expanded formula text keyed
    /// by position. Formula cells without a cached value still appear here.
    fn sheet_formulas(&mut self, name: &str) -> Result<Vec<(u32, u32, String)>> {
        // Formula iterator expands shared formulas and polls between cells,
        // avoiding an additional dense Range<String> allocation.
        let t_formulas = std::time::Instant::now();
        let mut out = Vec::new();
        {
            let mut formulas = self
                .reader
                .worksheet_cells_reader(name)
                .map_err(|e| mapped(e, &*self.cancel))?;
            while let Some(formula) = formulas
                .next_formula()
                .map_err(|e| mapped(e, &*self.cancel))?
            {
                self.cancel.check()?;
                if !formula.get_value().is_empty() {
                    let (r, c) = formula.get_position();
                    out.push((
                        r,
                        c,
                        format!("={}", formula.get_value().trim_start_matches('=')),
                    ));
                }
            }
        }
        crate::perf::span("workbook:formulas-pass", t_formulas);
        Ok(out)
    }
    /// Merged ranges of one sheet.
    fn sheet_merges(&mut self, name: &str) -> Result<Vec<MergedRange>> {
        let t_merges = std::time::Instant::now();
        let ranges = self
            .reader
            .merge_cells_by_sheet_name(name)
            .map_err(|e| mapped(e, &*self.cancel))?
            .into_iter()
            .map(|r| MergedRange {
                start: (r.start.0 as usize, r.start.1 as usize),
                end: (r.end.0 as usize, r.end.1 as usize),
            })
            .collect();
        crate::perf::span("workbook:merges", t_merges);
        Ok(ranges)
    }
    /// Each invocation loads only the requested sheet. Workbook metadata/shared
    /// strings are owned by calamine; ranges from unrelated sheets are never read.
    pub fn load_sheet(&mut self, name: &str) -> Result<Sheet> {
        let mut sheet = self.load_sheet_values(name)?;
        let formulas = self.sheet_formulas(name)?;
        apply_formulas(&mut sheet, formulas);
        sheet.merged_ranges = self.sheet_merges(name)?;
        self.cancel.check()?;
        sheet.fresh_revision();
        Ok(sheet)
    }
}
fn apply_formulas(sheet: &mut Sheet, formulas: Vec<(u32, u32, String)>) {
    for (r, c, text) in formulas {
        let mut cell = sheet
            .cell(r as usize, c as usize)
            .cloned()
            .unwrap_or_default();
        cell.formula = Some(text.into());
        sheet.put_loaded(r as usize, c as usize, cell);
    }
}
/// Full sheet load that decompresses/parses the sheet XML streams concurrently:
/// values+styles on the calling thread, formulas and merges on two helper
/// threads with their own handles over the same file. The resulting `Sheet`
/// is identical to the serial `load_sheet` sequence.
pub fn load_sheet_parallel(
    path: &Path,
    name: &str,
    cancel: Arc<dyn CancelCheck>,
) -> Result<Sheet> {
    let mut main_book = Workbook::open(path, cancel.clone())?;
    let aux = |kind: AuxPass, cancel: Arc<dyn CancelCheck>| -> Result<Aux> {
        let mut book = Workbook::open(path, cancel)?;
        match kind {
            AuxPass::Formulas => Ok(Aux::Formulas(book.sheet_formulas(name)?)),
            AuxPass::Merges => Ok(Aux::Merges(book.sheet_merges(name)?)),
        }
    };
    let (mut sheet, formulas, merges) = std::thread::scope(|scope| {
        let f = scope.spawn(|| aux(AuxPass::Formulas, cancel.clone()));
        let m = scope.spawn(|| aux(AuxPass::Merges, cancel.clone()));
        let sheet = main_book.load_sheet_values(name);
        let joined = match (f.join(), m.join()) {
            (Ok(f), Ok(m)) => Ok((f?, m?)),
            (Err(_), _) | (_, Err(_)) => Err(invalid("xlsx aux reader panicked")),
        };
        match (sheet, joined) {
            (Ok(sheet), Ok((Aux::Formulas(formulas), Aux::Merges(merges)))) => {
                Ok((sheet, formulas, merges))
            }
            (Err(e), _) => Err(e),
            (_, Err(e)) => Err(e),
            _ => Err(invalid("xlsx aux reader mismatch")),
        }
    })?;
    apply_formulas(&mut sheet, formulas);
    sheet.merged_ranges = merges;
    sheet.fresh_revision();
    Ok(sheet)
}
enum AuxPass {
    Formulas,
    Merges,
}
enum Aux {
    Formulas(Vec<(u32, u32, String)>),
    Merges(Vec<MergedRange>),
}
