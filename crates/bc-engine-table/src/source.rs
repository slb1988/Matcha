//! Streaming text records, encoding detection and lossless untouched-field saves.
use crate::{
    invalid,
    model::{RawRow, SheetSource, StringPool, TextSource},
    Cell, CellValue, Result, Sheet, TableModel,
};
use bc_core::{
    cancel::CancelCheck,
    encoding::{self, BomPolicy, Encoding},
};
use bc_formats::{
    model::{HeaderMode, RegionalSettings, TableType},
    TableOptions,
};
use std::{
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::Arc,
};

#[derive(Debug, Clone)]
pub struct ReadOptions {
    pub format: TableOptions,
    pub encoding: Option<Encoding>,
    pub default_code_page: u16,
    pub ctrl_z_eof: bool,
    pub header: Option<HeaderMode>,
    pub system_regional: RegionalSettings,
    /// Retain per-record lexical spans, not a second whole-file buffer.
    pub preserve_source: bool,
}
impl Default for ReadOptions {
    fn default() -> Self {
        Self {
            format: TableOptions::default(),
            encoding: None,
            default_code_page: 1252,
            ctrl_z_eof: false,
            header: None,
            system_regional: RegionalSettings::default(),
            preserve_source: true,
        }
    }
}
#[derive(Debug, Clone, Default)]
pub struct ReadDiagnostics {
    pub detected_delimiter: Option<char>,
    pub decoding_errors: usize,
    pub header_detected: bool,
    /// Mixed typed/text columns that a host can display as interpretation warnings.
    pub mixed_type_columns: Vec<usize>,
}
pub struct ReadOutput {
    pub sheet: Sheet,
    pub diagnostics: ReadDiagnostics,
}

/// One logical CSV record; storage is bounded by the longest logical record.
#[derive(Debug, Clone)]
pub struct Record {
    pub fields: Vec<String>,
    raw: RawRow,
}
/// UTF-8 record iterator. Encoding conversion is a separate streaming layer.
pub struct Records<R> {
    reader: R,
    options: TableOptions,
    line: String,
    offset: usize,
    eof: bool,
    ctrl_z: bool,
}
impl<R: BufRead> Records<R> {
    pub fn new(reader: R, options: TableOptions, ctrl_z: bool) -> Result<Self> {
        if options.table_type == TableType::Fixed
            && (options.column_widths.is_empty() || options.column_widths.contains(&0))
        {
            return Err(invalid("fixed widths must be nonzero"));
        }
        if options.delimiters.iter().any(|c| matches!(c, '\r' | '\n'))
            || options
                .qualifier
                .is_some_and(|q| options.delimiters.contains(&q))
        {
            return Err(invalid("invalid delimiter/qualifier combination"));
        }
        Ok(Self {
            reader,
            options,
            line: String::new(),
            offset: 0,
            eof: false,
            ctrl_z,
        })
    }
    fn peek(&mut self) -> Result<Option<char>> {
        if self.eof {
            return Ok(None);
        }
        if self.offset == self.line.len() {
            self.line.clear();
            self.offset = 0;
            if self.reader.read_line(&mut self.line)? == 0 {
                self.eof = true;
                return Ok(None);
            }
        }
        let c = self.line[self.offset..].chars().next();
        if self.ctrl_z && c == Some('\u{1a}') {
            self.eof = true;
            return Ok(None);
        }
        Ok(c)
    }
    fn take(&mut self) -> Result<Option<char>> {
        let c = self.peek()?;
        if let Some(c) = c {
            self.offset += c.len_utf8();
        }
        Ok(c)
    }
    pub fn next_record(&mut self, cancel: &dyn CancelCheck) -> Result<Option<Record>> {
        cancel.check()?;
        if self.peek()?.is_none() {
            return Ok(None);
        }
        let mut text = String::new();
        let mut fields = Vec::new();
        let mut spans = Vec::new();
        let mut start = 0usize;
        let mut quoted = false;
        let mut after_quote = false;
        let mut at_start = true;
        let mut ending = String::new();
        let fixed = self.options.table_type == TableType::Fixed;
        loop {
            if text.len().is_multiple_of(4096) {
                cancel.check()?;
            }
            let Some(c) = self.take()? else {
                break;
            };
            if !quoted && matches!(c, '\r' | '\n') {
                ending.push(c);
                if c == '\r' && self.peek()? == Some('\n') {
                    self.take()?;
                    ending.push('\n');
                }
                break;
            }
            if !fixed && self.options.qualifier == Some(c) {
                if quoted {
                    text.push(c);
                    if self.peek()? == Some(c) {
                        text.push(self.take()?.unwrap());
                    } else {
                        quoted = false;
                        after_quote = true;
                    }
                    continue;
                } else if at_start {
                    quoted = true;
                    at_start = false;
                    text.push(c);
                    continue;
                }
            }
            if !fixed && !quoted && self.options.delimiters.contains(&c) {
                spans.push((start as u32, text.len() as u32));
                fields.push(field_text(&text[start..], &self.options));
                text.push(c);
                if self.options.consecutive_delimiters {
                    while self
                        .peek()?
                        .is_some_and(|c| self.options.delimiters.contains(&c))
                    {
                        text.push(self.take()?.unwrap());
                    }
                }
                start = text.len();
                at_start = true;
                after_quote = false;
                continue;
            }
            if after_quote && !c.is_whitespace() {
                return Err(invalid("unexpected text after closing qualifier"));
            }
            if !(self.options.trim_surrounding_whitespace && c.is_whitespace()) {
                at_start = false;
            }
            text.push(c);
            if text.len() > u32::MAX as usize {
                return Err(invalid("record exceeds 4 GiB"));
            }
        }
        if quoted {
            return Err(invalid("unterminated quoted field"));
        }
        if fixed {
            let mut chars = text.char_indices();
            let mut offset = 0;
            for &width in &self.options.column_widths {
                let start = offset;
                for _ in 0..width {
                    if let Some((i, c)) = chars.next() {
                        offset = i + c.len_utf8();
                    } else {
                        break;
                    }
                }
                spans.push((start as u32, offset as u32));
                let value = &text[start..offset];
                fields.push(
                    if self.options.trim_surrounding_whitespace {
                        value.trim()
                    } else {
                        value
                    }
                    .to_string(),
                );
            }
        } else {
            spans.push((start as u32, text.len() as u32));
            fields.push(field_text(&text[start..], &self.options));
        }
        Ok(Some(Record {
            fields,
            raw: RawRow {
                text: text.into(),
                fields: spans,
                ending: ending.into(),
            },
        }))
    }
}
pub(crate) fn field_text(raw: &str, options: &TableOptions) -> String {
    let raw = if options.trim_surrounding_whitespace {
        raw.trim()
    } else {
        raw
    };
    if let Some(q) = options.qualifier {
        if raw.starts_with(q) && raw.ends_with(q) && raw.len() >= q.len_utf8() * 2 {
            return raw[q.len_utf8()..raw.len() - q.len_utf8()]
                .replace(&format!("{q}{q}"), &q.to_string());
        }
    }
    raw.into()
}

/// Validate all UTF-8 bytes without retaining them, matching core's whole-input
/// validation rather than guessing from an 8 KiB prefix. A seekable source allows
/// the actual parse to restart without spooling the file into memory.
fn detect<R: Read + Seek>(
    reader: &mut R,
    options: &ReadOptions,
    cancel: &dyn CancelCheck,
) -> Result<(Encoding, bool)> {
    let start = reader.stream_position()?;
    let mut first = vec![0; 8192];
    let mut len = 0;
    while len < first.len() {
        let n = reader.read(&mut first[len..])?;
        if n == 0 {
            break;
        }
        len += n;
    }
    first.truncate(len);
    let sniff =
        if options.ctrl_z_eof && !first.starts_with(b"\xff\xfe") && !first.starts_with(b"\xfe\xff")
        {
            &first[..first.iter().position(|&b| b == 0x1a).unwrap_or(first.len())]
        } else {
            &first
        };
    let detected = encoding::detect(sniff, options.default_code_page);
    if let Some(enc) = options.encoding {
        reader.seek(SeekFrom::Start(start))?;
        return Ok((
            enc,
            first.starts_with(b"\xef\xbb\xbf")
                || first.starts_with(b"\xff\xfe")
                || first.starts_with(b"\xfe\xff"),
        ));
    }
    if detected.has_bom || detected.encoding == Encoding::Binary {
        reader.seek(SeekFrom::Start(start))?;
        return Ok((detected.encoding, detected.has_bom));
    }
    let mut pending = Vec::new();
    let mut valid = true;
    let mut bytes = first;
    loop {
        cancel.check()?;
        let eof_marker = options
            .ctrl_z_eof
            .then(|| bytes.iter().position(|&b| b == 0x1a))
            .flatten();
        if let Some(end) = eof_marker {
            bytes.truncate(end);
        }
        pending.extend_from_slice(&bytes);
        match std::str::from_utf8(&pending) {
            Ok(_) => pending.clear(),
            Err(e) if e.error_len().is_none() => {
                pending.drain(..e.valid_up_to());
            }
            Err(_) => {
                valid = false;
                break;
            }
        }
        if eof_marker.is_some() {
            valid &= pending.is_empty();
            break;
        }
        bytes.resize(64 * 1024, 0);
        let n = reader.read(&mut bytes)?;
        bytes.truncate(n);
        if n == 0 {
            valid &= pending.is_empty();
            break;
        }
    }
    reader.seek(SeekFrom::Start(start))?;
    Ok((
        if valid {
            Encoding::Utf8
        } else {
            Encoding::CodePage(options.default_code_page)
        },
        false,
    ))
}
fn rs_encoding(encoding: Encoding) -> Result<&'static encoding_rs::Encoding> {
    use encoding_rs::{UTF_16BE, UTF_16LE, UTF_8};
    Ok(match encoding {
        Encoding::Utf8 | Encoding::CodePage(65001) => UTF_8,
        Encoding::Utf16Le | Encoding::CodePage(1200) => UTF_16LE,
        Encoding::Utf16Be | Encoding::CodePage(1201) => UTF_16BE,
        Encoding::Binary => return Err(invalid("binary source is not a text table")),
        Encoding::CodePage(page) => {
            let label = match page {
                866 => "ibm866",
                874 => "windows-874",
                932 => "shift_jis",
                936 => "gbk",
                949 => "euc-kr",
                950 => "big5",
                1250 => "windows-1250",
                1251 => "windows-1251",
                1252 => "windows-1252",
                1253 => "windows-1253",
                1254 => "windows-1254",
                1255 => "windows-1255",
                1256 => "windows-1256",
                1257 => "windows-1257",
                1258 => "windows-1258",
                20866 => "koi8-r",
                21866 => "koi8-u",
                54936 => "gb18030",
                _ => return Err(invalid(format!("unsupported code page {page}"))),
            };
            encoding_rs::Encoding::for_label(label.as_bytes()).unwrap()
        }
    })
}
struct Decoded<R> {
    reader: R,
    decoder: encoding_rs::Decoder,
    input: Vec<u8>,
    output: Vec<u8>,
    offset: usize,
    eof: bool,
    errors: usize,
}
impl<R: Read> Decoded<R> {
    fn new(reader: R, encoding: Encoding) -> Result<Self> {
        Ok(Self {
            reader,
            decoder: rs_encoding(encoding)?.new_decoder_with_bom_removal(),
            input: vec![0; 64 * 1024],
            output: Vec::new(),
            offset: 0,
            eof: false,
            errors: 0,
        })
    }
}
impl<R: Read> Read for Decoded<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        while self.offset == self.output.len() {
            if self.eof {
                return Ok(0);
            }
            let n = self.reader.read(&mut self.input)?;
            self.eof = n == 0;
            let mut text =
                String::with_capacity(self.decoder.max_utf8_buffer_length(n).unwrap_or(n * 4 + 16));
            let mut rest = &self.input[..n];
            loop {
                let (result, consumed) = self
                    .decoder
                    .decode_to_string_without_replacement(rest, &mut text, self.eof);
                rest = &rest[consumed..];
                match result {
                    encoding_rs::DecoderResult::InputEmpty => break,
                    encoding_rs::DecoderResult::OutputFull => text.reserve(rest.len() * 4 + 16),
                    encoding_rs::DecoderResult::Malformed(_, _) => {
                        text.push('\u{fffd}');
                        self.errors += 1;
                    }
                }
            }
            self.output = text.into_bytes();
            self.offset = 0;
        }
        let n = buffer.len().min(self.output.len() - self.offset);
        buffer[..n].copy_from_slice(&self.output[self.offset..self.offset + n]);
        self.offset += n;
        Ok(n)
    }
}
fn delimiter_score(sample: &str, delimiter: char, format: &TableOptions) -> usize {
    let mut format = format.clone();
    format.delimiters = vec![delimiter];
    format.detect_delimiter = false;
    let Ok(mut records) = Records::new(io::Cursor::new(sample.as_bytes()), format, false) else {
        return 0;
    };
    let mut counts = std::collections::HashMap::new();
    for _ in 0..32 {
        let Ok(Some(record)) = records.next_record(&bc_core::cancel::NeverCancel) else {
            break;
        };
        if record.fields.len() > 1 {
            *counts.entry(record.fields.len()).or_insert(0usize) += 1;
        }
    }
    counts
        .into_iter()
        .map(|(width, count)| count * 100 + width.min(99))
        .max()
        .unwrap_or(0)
}

pub fn read_path(
    path: &Path,
    options: &ReadOptions,
    cancel: &dyn CancelCheck,
) -> Result<ReadOutput> {
    let mut result = read_text(File::open(path)?, options, cancel)?;
    result.sheet.name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    Ok(result)
}
pub fn read_text<R: Read + Seek>(
    mut reader: R,
    options: &ReadOptions,
    cancel: &dyn CancelCheck,
) -> Result<ReadOutput> {
    let (encoding, has_bom) = detect(&mut reader, options, cancel)?;
    let mut decoder = BufReader::new(Decoded::new(reader, encoding)?);
    let mut prefix = Vec::new();
    let mut format = options.format.clone();
    let mut diagnostics = ReadDiagnostics::default();
    if format.detect_delimiter && format.table_type == TableType::Delimited {
        decoder.by_ref().take(64 * 1024).read_to_end(&mut prefix)?;
        let sample = String::from_utf8_lossy(&prefix);
        let candidates = [',', '\t', ';', '|', ' '];
        let mut best = (0, ',');
        for c in candidates {
            let score = delimiter_score(&sample, c, &format);
            if score > best.0 {
                best = (score, c);
            }
        }
        format.delimiters = vec![best.1];
        diagnostics.detected_delimiter = Some(best.1);
    }
    let mut records = Records::new(
        BufReader::new(io::Cursor::new(prefix).chain(decoder)),
        format.clone(),
        options.ctrl_z_eof,
    )?;
    let mut sheet = Sheet {
        source: SheetSource::Text,
        regional: if format.regional.use_system {
            options.system_regional.clone()
        } else {
            format.regional.clone()
        },
        ..Sheet::default()
    };
    let mut raw_rows = Vec::new();
    let mut pool = StringPool::default();
    let mut typed = Vec::<bool>::new();
    let mut textual = Vec::<bool>::new();
    let mut endings = std::collections::BTreeMap::<String, usize>::new();
    while let Some(record) = records.next_record(cancel)? {
        if !record.raw.ending.is_empty() {
            *endings.entry(record.raw.ending.to_string()).or_default() += 1;
        }
        let cells = record
            .fields
            .iter()
            .enumerate()
            .map(|(i, value)| {
                let mut parsed = crate::regional::interpret(value, &sheet.regional);
                typed.resize(typed.len().max(i + 1), false);
                textual.resize(textual.len().max(i + 1), false);
                if let CellValue::Text(s) = &mut parsed {
                    *s = pool.intern(value);
                    if sheet.row_count() > 0 {
                        textual[i] = true;
                    }
                } else if !matches!(parsed, CellValue::Empty) {
                    typed[i] = true;
                }
                Cell::new(parsed)
            })
            .collect();
        sheet.push_row(cells);
        if options.preserve_source {
            raw_rows.push(record.raw);
        }
    }
    let header_mode = options.header.unwrap_or(format.header);
    let header = header_mode == HeaderMode::Names
        || (header_mode == HeaderMode::Auto && detect_header(&sheet));
    if header {
        sheet.take_header();
        diagnostics.header_detected = true;
    }
    diagnostics.mixed_type_columns = (0..typed.len())
        .filter(|&i| typed[i] && textual[i])
        .collect();
    diagnostics.decoding_errors = records
        .reader
        .into_inner()
        .into_inner()
        .1
        .into_inner()
        .errors;
    let default_ending = endings
        .into_iter()
        .max_by_key(|(ending, count)| {
            (
                *count,
                match ending.as_str() {
                    "\r\n" => 3,
                    "\n" => 2,
                    _ => 1,
                },
            )
        })
        .map(|(e, _)| e)
        .unwrap_or_else(|| "\r\n".into());
    let header_raw = if header && !raw_rows.is_empty() {
        Some(raw_rows.remove(0))
    } else {
        None
    };
    sheet.text_source = Some(TextSource {
        options: format,
        encoding,
        has_bom,
        header: header_raw,
        rows: Arc::new(raw_rows),
        default_ending: default_ending.into(),
    });
    Ok(ReadOutput { sheet, diagnostics })
}
pub fn detect_header(sheet: &Sheet) -> bool {
    sheet.row_count() > 1
        && sheet.column_count() > 0
        && (0..sheet.column_count())
            .all(|c| matches!(sheet.cell(0, c).map(|c| &c.value), Some(CellValue::Text(_))))
        && (0..sheet.column_count()).any(|c| {
            (1..sheet.row_count().min(33))
                .filter(|&r| {
                    !matches!(
                        sheet.cell(r, c).map(|c| &c.value),
                        Some(CellValue::Text(_) | CellValue::Empty)
                    )
                })
                .count()
                * 2
                >= (sheet.row_count() - 1).min(32)
        })
}
impl TableModel {
    pub fn parse_delimited(
        text: &str,
        options: &TableOptions,
        first_row_is_header: bool,
    ) -> Result<Self> {
        let options = ReadOptions {
            format: options.clone(),
            header: Some(if first_row_is_header {
                HeaderMode::Names
            } else {
                HeaderMode::Data
            }),
            ..Default::default()
        };
        Ok(Self::new(
            read_text(
                io::Cursor::new(text),
                &options,
                &bc_core::cancel::NeverCancel,
            )?
            .sheet,
        ))
    }
}
fn quoted_field(value: &str, options: &TableOptions) -> Result<String> {
    let needs = value.chars().any(|c| {
        matches!(c, '\r' | '\n') || options.delimiters.contains(&c) || options.qualifier == Some(c)
    }) || (options.trim_surrounding_whitespace && value.trim() != value);
    if !needs {
        return Ok(value.into());
    }
    let q = options
        .qualifier
        .ok_or_else(|| invalid("field needs a qualifier but none is configured"))?;
    Ok(format!("{q}{}{q}", value.replace(q, &format!("{q}{q}"))))
}
/// Streaming writer. Does not truncate an existing destination on validation error;
/// callers writing files should use save_as for staged atomic replacement.
pub fn write_delimited(sheet: &Sheet, mut writer: impl Write) -> Result<()> {
    if matches!(sheet.source, SheetSource::Workbook) {
        return Err(invalid("workbook is read-only; export as CSV instead"));
    }
    let source = sheet.text_source.as_ref();
    let default_options = TableOptions::default();
    let options = source.map(|s| &s.options).unwrap_or(&default_options);
    if options.table_type == TableType::Fixed {
        return Err(invalid(
            "fixed-width write-back is not supported; export as CSV",
        ));
    }
    let encoding = source.map_or(Encoding::Utf8, |s| s.encoding);
    let has_bom = source.is_some_and(|s| s.has_bom);
    writer.write_all(&encoding::encode(
        "",
        encoding,
        BomPolicy::Preserve,
        has_bom,
    )?)?;
    let delimiter = options
        .delimiters
        .first()
        .copied()
        .unwrap_or(',')
        .to_string();
    let ending = source.map_or("\r\n", |s| s.default_ending.as_ref());
    if let Some(headers) = &sheet.headers {
        let text = if let Some(raw) = source.and_then(|s| s.header.as_ref()) {
            format!("{}{}", raw.text, raw.ending)
        } else {
            format!(
                "{}{}",
                headers
                    .iter()
                    .map(|h| quoted_field(h, options))
                    .collect::<Result<Vec<_>>>()?
                    .join(&delimiter),
                ending
            )
        };
        writer.write_all(&encoding::encode(&text, encoding, BomPolicy::Never, false)?)?;
    }
    for row in 0..sheet.row_count() {
        let mut text = String::new();
        if let Some(raw) = source
            .and_then(|s| s.rows.get(row))
            .filter(|r| !r.fields.is_empty())
        {
            let mut offset = 0;
            for (col, &(start, end)) in raw.fields.iter().enumerate() {
                text.push_str(&raw.text[offset..start as usize]);
                if sheet.changed.contains(&(row, col)) {
                    text.push_str(&quoted_field(
                        &sheet.display_at(row, col).unwrap(),
                        options,
                    )?);
                } else {
                    text.push_str(&raw.text[start as usize..end as usize]);
                }
                offset = end as usize;
            }
            text.push_str(&raw.text[offset..]);
            for col in raw.fields.len()..sheet.column_count() {
                if sheet.changed.contains(&(row, col))
                    || (col..sheet.column_count()).any(|c| sheet.changed.contains(&(row, c)))
                {
                    text.push_str(&delimiter);
                    text.push_str(&quoted_field(
                        &sheet.display_at(row, col).unwrap(),
                        options,
                    )?);
                }
            }
            text.push_str(&raw.ending);
        } else {
            for col in 0..sheet.column_count() {
                if col > 0 {
                    text.push_str(&delimiter);
                }
                text.push_str(&quoted_field(
                    &sheet.display_at(row, col).unwrap(),
                    options,
                )?);
            }
            text.push_str(ending);
        }
        writer.write_all(&encoding::encode(&text, encoding, BomPolicy::Never, false)?)?;
    }
    Ok(())
}
pub fn export_csv(sheet: &Sheet, writer: impl Write) -> Result<()> {
    let mut exported = sheet.clone();
    exported.source = SheetSource::Memory;
    exported.text_source = None;
    write_delimited(&exported, writer)
}
pub fn save_as(sheet: &Sheet, path: &Path) -> Result<()> {
    if matches!(sheet.source, SheetSource::Workbook) {
        return Err(invalid("workbook is read-only; export as CSV instead"));
    }
    let name = path
        .file_name()
        .ok_or_else(|| invalid("destination has no filename"))?
        .to_string_lossy();
    let temp = path.with_file_name(format!(
        ".{name}.matcha-{}-{}.tmp",
        std::process::id(),
        crate::model::revision()
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        write_delimited(sheet, &mut file)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}
