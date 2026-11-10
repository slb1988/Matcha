//! On-demand CSV-backed sheet storage (experimental entry, default off).
//!
//! One private snapshot copy per side + read-only mmap + per-record byte-span
//! index built with the production `Records` parser. Cell values are parsed on
//! access through the same parser and `regional::interpret`; a qualifier-free
//! fast path borrows field slices directly from the mapping (no unsafe, no
//! borrow extension: slices live only as long as the `&self` borrow).
//!
//! Edits live in an identity-keyed sparse overlay (`Version`, copy-on-write):
//! rows/columns carry stable ids, so reorder/insert/remove remap order vectors
//! without touching the mapping, and undo/redo restores arc-shared versions.
//! Dirty cells and undo snapshots are never part of the bounded LRU viewport
//! cache; the cache holds display strings of physically-backed, unedited rows
//! only and is cleared on every mutation (revision bump).
//!
//! Why a private snapshot: a read-only map of the user file would reflect
//! external writes (page cache is shared) and break in-place save-as; the copy
//! gives stable bytes and lets save overwrite the original path. Snapshot copy
//! time and its mapped residency are real costs and are measured, not hidden.

use std::{
    cell::{Cell as TCell, RefCell},
    collections::{BTreeMap, HashMap, HashSet},
    io::{BufRead, Read},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        atomic::{AtomicU32, AtomicU8, Ordering as AtomicOrdering},
        Arc, Mutex,
    },
};

use bc_core::{cancel::CancelCheck, encoding::Encoding};
use bc_formats::model::{HeaderMode, RegionalSettings, TableType};
use memmap2::Mmap;

use crate::{
    data::{CellData, CellInfo, TableData},
    invalid,
    model::Style,
    source::{ReadOptions, Records},
    Cell, CellValue, Result, StyleId, TableOptions,
};

/// Row/column identity. Physical records/columns use their source index;
/// inserted rows/columns take fresh ids above the physical ranges.
pub type RowId = u32;
pub type ColId = u32;

/// Compare-phase supply mode (prototype-proven): typed values for
/// hashing/alignment/display, raw field text during compare where the
/// comparator's existing raw-text guard skips typing for identical fields and
/// `normalized_value` still types differing fields (rules equality preserved).
const SUPPLY_RAW: u8 = 0;
const SUPPLY_TYPED: u8 = 1;

/// Viewport row cache byte budget (prototype b6 value; stable-state account
/// includes the live cache, see tools/demand_proto.md).
const CACHE_CAP_BYTES: usize = 8 * 1024 * 1024;

/// Why a source cannot be served by demand storage.
#[derive(Debug)]
pub enum DemandError {
    /// Caller should fall back to the dense `read_path` load (visible log).
    Unsupported(String),
    /// Real I/O or parse failure; propagate like a dense load failure.
    Failed(bc_core::CoreError),
}
impl DemandError {
    fn unsupported(message: impl Into<String>) -> Self {
        Self::Unsupported(message.into())
    }
}
impl From<bc_core::CoreError> for DemandError {
    fn from(e: bc_core::CoreError) -> Self {
        Self::Failed(e)
    }
}
impl From<std::io::Error> for DemandError {
    fn from(e: std::io::Error) -> Self {
        Self::Failed(bc_core::CoreError::Io(e))
    }
}

/// BufRead over a byte slice that counts exactly the bytes the parser consumed
/// (logical record boundaries), never the readable-ahead remainder.
struct Counting<'a> {
    inner: &'a [u8],
    pos: Rc<TCell<usize>>,
}
impl Read for Counting<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = Read::read(&mut self.inner, buf)?;
        self.pos.set(self.pos.get() + n);
        Ok(n)
    }
}
impl BufRead for Counting<'_> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        Ok(self.inner)
    }
    fn consume(&mut self, n: usize) {
        self.inner = &self.inner[n..];
        self.pos.set(self.pos.get() + n);
    }
}

// Per-thread, per-table last-parsed physical record. The compare chain touches
// one row's cells consecutively; TLS keeps `DemandSheet: Sync` without locks
// while owned `Cow::Owned` text ensures no borrow escapes the cell. Entries for
// table ids no longer in use are bounded by clearing when many ids accumulate.
thread_local! {
    static ROW_SCRATCH: RefCell<HashMap<usize, (u32, Vec<String>)>> =
        RefCell::new(HashMap::new());
}
static TABLE_ID: AtomicU32 = AtomicU32::new(0);

/// Strip the record terminator the parser consumed (\r\n / \n / \r; absent at EOF).
fn strip_ending(mut body: &[u8]) -> &[u8] {
    if body.ends_with(b"\r\n") {
        body = &body[..body.len() - 2];
    } else if body.ends_with(b"\n") || body.ends_with(b"\r") {
        body = &body[..body.len() - 1];
    }
    body
}

/// The ending a record's byte span carries ("" at EOF without terminator).
fn ending_of(span: &[u8]) -> &str {
    if span.ends_with(b"\r\n") {
        "\r\n"
    } else if span.ends_with(b"\n") {
        "\n"
    } else if span.ends_with(b"\r") {
        "\r"
    } else {
        ""
    }
}

/// Span index products: record spans (absolute offsets), column ceiling and
/// line-ending vote. Both builders below produce identical triples; the
/// differential test at the bottom of this module proves it on synthetic and
/// real inputs.
type SpanIndex = (Vec<(u32, u32)>, usize, BTreeMap<String, usize>);

/// Build the record span index. The fast scan handles qualifier-free lines;
/// anything else (non-ASCII delimiter/qualifier alphabets, or a `ctrl_z_eof`
/// file that actually contains \x1A — pre-scanned, since the parser treats
/// \x1A as EOF even inside quotes) takes the production `Records` path.
fn build_span_index(
    map: &[u8],
    base: usize,
    format: &TableOptions,
    ctrl_z: bool,
    cancel: &dyn CancelCheck,
) -> Result<SpanIndex> {
    let body = &map[base..];
    if fast_span_eligible(format, ctrl_z, body) {
        // Same upfront options validation the Records constructor performs.
        Records::new(&body[..0], format.clone(), ctrl_z)?;
        fast_span_index(body, base, format, ctrl_z, cancel)
    } else {
        records_span_index(map, base, format, ctrl_z, cancel)
    }
}

fn fast_span_eligible(format: &TableOptions, ctrl_z: bool, body: &[u8]) -> bool {
    format.table_type == TableType::Delimited
        && format.qualifier.is_none_or(|q| q.is_ascii())
        && format.delimiters.iter().all(|d| d.is_ascii())
        && !(ctrl_z && body.contains(&0x1A))
}

/// Production-parser span index (the original open() loop, verbatim).
fn records_span_index(
    map: &[u8],
    base: usize,
    format: &TableOptions,
    ctrl_z: bool,
    cancel: &dyn CancelCheck,
) -> Result<SpanIndex> {
    let mut spans = Vec::new();
    let mut columns = 0usize;
    let mut endings = BTreeMap::<String, usize>::new();
    let pos = Rc::new(TCell::new(0usize));
    let counting = Counting {
        inner: &map[base..],
        pos: pos.clone(),
    };
    let mut records = Records::new(counting, format.clone(), ctrl_z)?;
    let mut prev_end = 0usize;
    while let Some(record) = records.next_record(cancel)? {
        let end = pos.get();
        let span = &map[base + prev_end..base + end];
        *endings.entry(ending_of(span).to_string()).or_default() += 1;
        if record.fields.len() > columns {
            columns = record.fields.len();
        }
        // Offsets are relative to the post-BOM base.
        spans.push(((base + prev_end) as u32, (end - prev_end) as u32));
        prev_end = end;
    }
    Ok((spans, columns, endings))
}

/// Fast span index. A candidate record region without the qualifier byte ends
/// at its first '\r'/'\n' (the parser never engages quoting there, so the
/// line is the record); a region containing the qualifier byte re-parses
/// exactly one logical record with the production `Records` parser from the
/// record start (covering embedded newlines/delimiters and doubled escapes)
/// and the scan resumes at the record's logical end — the fallback always
/// starts on a true record boundary because preceding records were consumed
/// completely. Field counts replicate the parser's delimiter splitting
/// (quoting cannot engage without a qualifier byte); column identity is what
/// the column ceiling needs. Span ends replicate the Records path's reader
/// consumption: its `read_line` over-reads to the next '\n', so a record's
/// span end is the smallest just-after-'\n' position >= the record's logical
/// end (identity for '\n'/'\r\n' endings; the over-read only shows on lone
/// '\r' endings, mirrored here).
fn fast_span_index(
    body: &[u8],
    base: usize,
    format: &TableOptions,
    ctrl_z: bool,
    cancel: &dyn CancelCheck,
) -> Result<SpanIndex> {
    let qualifier = format.qualifier.filter(|q| q.is_ascii()).map(|q| q as u8);
    let delimiters: Vec<u8> = format.delimiters.iter().map(|&d| d as u8).collect();
    let consecutive = format.consecutive_delimiters;
    let mut spans = Vec::new();
    let mut columns = 0usize;
    let mut endings = BTreeMap::<String, usize>::new();
    let mut pos = 0usize; // logical start of the next record
    let mut span_prev = 0usize; // reader-consumption end of the previous record
    let len = body.len();
    while pos < len {
        cancel.check()?;
        // Scan the candidate record: terminator position, field count,
        // whether the qualifier byte appears before the terminator.
        let mut i = pos;
        let mut fields = 1usize;
        let mut has_qualifier = false;
        let mut term = len;
        while i < len {
            if i > pos && (i - pos).is_multiple_of(4096) {
                cancel.check()?;
            }
            let b = body[i];
            if b == b'\r' || b == b'\n' {
                term = i;
                break;
            }
            if Some(b) == qualifier {
                has_qualifier = true;
                break;
            }
            if delimiters.contains(&b) {
                fields += 1;
                if consecutive {
                    // Runs collapse into one separator (parser semantics).
                    while i + 1 < len && delimiters.contains(&body[i + 1]) {
                        i += 1;
                    }
                }
            }
            i += 1;
        }
        if has_qualifier {
            // Exactly one record from the production parser.
            let consumed = Rc::new(TCell::new(0usize));
            let counting = Counting {
                inner: &body[pos..],
                pos: consumed.clone(),
            };
            let mut records = Records::new(counting, format.clone(), ctrl_z)?;
            let record = records
                .next_record(cancel)?
                .ok_or_else(|| invalid("record parse failed at scan position"))?;
            let rec_end = pos + consumed.get();
            if record.fields.len() > columns {
                columns = record.fields.len();
            }
            push_span(body, base, &mut spans, &mut endings, span_prev, rec_end);
            // The next record parses from the logical end; its span starts at
            // the reader-consumption end (they differ only on over-reads).
            pos += record.raw.text.len() + record.raw.ending.len();
            span_prev = rec_end;
            continue;
        }
        // Logical end: past the terminator, consuming the '\n' of '\r\n'.
        let end = if term == len {
            len
        } else if body[term] == b'\r' && term + 1 < len && body[term + 1] == b'\n' {
            term + 2
        } else {
            term + 1
        };
        // Records errors on oversized record text (content, sans terminator).
        if term - pos > u32::MAX as usize {
            return Err(invalid("record exceeds 4 GiB"));
        }
        let rec_end = if end == len || body[end - 1] == b'\n' {
            end
        } else {
            // Lone-'\r' over-read: the parser's read_line consumed through the
            // next '\n' (or EOF) before the record break was observed.
            body[end..]
                .iter()
                .position(|&b| b == b'\n')
                .map_or(len, |j| end + j + 1)
        };
        if fields > columns {
            columns = fields;
        }
        push_span(body, base, &mut spans, &mut endings, span_prev, rec_end);
        pos = end;
        span_prev = rec_end;
    }
    Ok((spans, columns, endings))
}

fn push_span(
    body: &[u8],
    base: usize,
    spans: &mut Vec<(u32, u32)>,
    endings: &mut BTreeMap<String, usize>,
    start: usize,
    end: usize,
) {
    let span = &body[start..end];
    *endings.entry(ending_of(span).to_string()).or_default() += 1;
    spans.push(((base + start) as u32, (end - start) as u32));
}

/// Fast-path gate mirroring the prototype evidence: the Records parser only
/// engages quote handling when the qualifier byte appears, so delimiter
/// splitting reproduces fields exactly for qualifier-free records.
fn fast_path_ok(options: &TableOptions) -> bool {
    options.table_type == TableType::Delimited
        && !options.trim_surrounding_whitespace
        && !options.consecutive_delimiters
        && options.qualifier.is_some_and(|q| q.is_ascii())
        && !options.delimiters.is_empty()
        && options.delimiters.iter().all(|d| d.is_ascii())
}

/// Byte-budgeted LRU row cache for viewport materialization. Entries are
/// rebuildable from the source, so eviction is always safe; the byte total is
/// accounted and capped, never growing with access count. Dirty/undo data is
/// never cached here.
#[derive(Debug)]
struct BoundedRowCache {
    cap: usize,
    map: HashMap<RowId, (Arc<Vec<String>>, usize, u64)>,
    ticks: BTreeMap<u64, RowId>,
    bytes: usize,
    tick: u64,
    max_bytes: usize,
    hits: u64,
    misses: u64,
    evictions: u64,
}
impl BoundedRowCache {
    fn new(cap: usize) -> Self {
        Self {
            cap,
            map: HashMap::new(),
            ticks: BTreeMap::new(),
            bytes: 0,
            tick: 0,
            max_bytes: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
        }
    }
    fn entry_bytes(v: &[String]) -> usize {
        v.iter().map(|s| s.len() + 24).sum::<usize>() + 40
    }
    fn get_or_load(
        &mut self,
        key: RowId,
        load: impl FnOnce() -> Arc<Vec<String>>,
    ) -> Arc<Vec<String>> {
        if let Some(e) = self.map.get_mut(&key) {
            self.tick += 1;
            self.ticks.remove(&e.2);
            e.2 = self.tick;
            self.ticks.insert(self.tick, key);
            self.hits += 1;
            return e.0.clone();
        }
        let v = load();
        let b = Self::entry_bytes(&v);
        self.tick += 1;
        self.map.insert(key, (v.clone(), b, self.tick));
        self.ticks.insert(self.tick, key);
        self.bytes += b;
        self.misses += 1;
        while self.bytes > self.cap {
            let (&t, &k) = self.ticks.iter().next().expect("entry present");
            let e = self.map.remove(&k).expect("entry present");
            self.ticks.remove(&t);
            self.bytes -= e.1;
            self.evictions += 1;
        }
        self.max_bytes = self.max_bytes.max(self.bytes);
        v
    }
    fn clear(&mut self) {
        self.map.clear();
        self.ticks.clear();
        self.bytes = 0;
    }
}

/// Bounded-cache counters for measurement surfaces (prototype harness).
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub max_bytes: usize,
    pub live_bytes: usize,
    pub cap_bytes: usize,
}

/// Mutable logical state; snapshot clones share it by Arc until an edit
/// copy-on-writes. Undo/redo restore captured versions wholesale.
#[derive(Debug)]
struct Version {
    /// Logical row order over row identities.
    rows: Vec<RowId>,
    /// Logical column order over column identities.
    cols: Vec<ColId>,
    /// Dirty/inserted cell values (never evicted, never in the LRU cache).
    edits: HashMap<(RowId, ColId), Cell>,
    /// Override display text (copy semantics parity with Sheet).
    display_overrides: HashMap<(RowId, ColId), Arc<str>>,
    /// Rows carrying edit marks (identity-based; stable across reorders).
    edited_rows: HashSet<RowId>,
    /// Rows with at least one overlay cell (save splice / cache bypass).
    dirty_rows: HashSet<RowId>,
    /// Column insert/remove marks every row edited (Sheet parity) without
    /// materializing a million-entry set.
    all_rows_edited: bool,
    headers: Option<Vec<String>>,
    styles: Vec<Style>,
    revision: u64,
}
impl Clone for Version {
    fn clone(&self) -> Self {
        Self {
            rows: self.rows.clone(),
            cols: self.cols.clone(),
            edits: self.edits.clone(),
            display_overrides: self.display_overrides.clone(),
            edited_rows: self.edited_rows.clone(),
            dirty_rows: self.dirty_rows.clone(),
            all_rows_edited: self.all_rows_edited,
            headers: self.headers.clone(),
            styles: self.styles.clone(),
            revision: self.revision,
        }
    }
}

/// Immutable physical side: snapshot mapping + record spans + format options.
/// Shared by every snapshot clone of the sheet.
#[derive(Debug)]
struct DemandInner {
    // Option so Drop can unmap before deleting the snapshot file (Windows
    // refuses to delete a mapped file). Accessors unwrap; never None until drop.
    map: Option<Mmap>,
    id: usize,
    snapshot_path: PathBuf,
    spans: Vec<(u32, u32)>, // (offset, len) per data record; header span below
    /// Physical span of the promoted header record (verbatim save).
    header_span: Option<(u32, u32)>,
    header_rows: u32,
    columns: u32,
    options: TableOptions,
    regional: RegionalSettings,
    has_bom: bool,
    default_ending: String,
    /// Source file name (tab title parity with `read_path`).
    name: String,
    /// (qualifier byte, delimiter bytes) when the borrow-from-mapping split is
    /// allowed; None => always re-parse via Records.
    fast: Option<(u8, Vec<u8>)>,
    next_row: AtomicU32,
    next_col: AtomicU32,
    cache_cap: usize,
}
impl DemandInner {
    fn bytes(&self) -> &[u8] {
        self.map.as_deref().unwrap_or(&[])
    }
    fn record_bytes(&self, record: usize) -> &[u8] {
        let (off, len) = self.spans[record];
        &self.bytes()[off as usize..(off + len) as usize]
    }
}
impl Drop for DemandInner {
    fn drop(&mut self) {
        // Unmap first: Windows cannot delete a file with live mappings.
        self.map.take();
        let _ = std::fs::remove_file(&self.snapshot_path);
    }
}

/// On-demand CSV sheet. See module docs for the ownership/editing model.
#[derive(Debug)]
pub struct DemandSheet {
    inner: Arc<DemandInner>,
    version: Arc<Version>,
    /// Bounded viewport row cache (Mutex: Sync without a compare-hot-path lock;
    /// compare reads go through TLS scratch instead).
    cache: Mutex<BoundedRowCache>,
    /// Per-clone supply mode: a worker snapshot flips its own copy without
    /// touching the interactive store's display semantics.
    supply: AtomicU8,
}
impl Clone for DemandSheet {
    fn clone(&self) -> Self {
        // Snapshot clone: shares the mapping/index and the current version
        // (copy-on-write on edit); gets a fresh empty bounded cache.
        Self {
            inner: self.inner.clone(),
            version: self.version.clone(),
            cache: Mutex::new(BoundedRowCache::new(self.inner.cache_cap)),
            supply: AtomicU8::new(self.supply.load(AtomicOrdering::Relaxed)),
        }
    }
}

impl DemandSheet {
    /// Open a delimited text source as on-demand storage: snapshot copy,
    /// read-only mapping, span index via the production parser, header handling
    /// identical to `read_text`. `Unsupported` reasons ask the caller to fall
    /// back to the dense load (non-UTF8 encodings, fixed-width layouts).
    pub fn open(
        path: &Path,
        options: &ReadOptions,
        cancel: &dyn CancelCheck,
    ) -> std::result::Result<Self, DemandError> {
        if options.format.table_type == TableType::Fixed {
            return Err(DemandError::unsupported(
                "fixed-width layout is not demand-backed; dense load applies",
            ));
        }
        let mut probe = std::fs::File::open(path)?;
        let (encoding, has_bom) = crate::source::detect(&mut probe, options, cancel)?;
        if encoding != Encoding::Utf8 {
            return Err(DemandError::unsupported(format!(
                "encoding {encoding:?} is not demand-backed; dense load applies"
            )));
        }
        drop(probe);
        let mut format = options.format.clone();
        if format.detect_delimiter && format.table_type == TableType::Delimited {
            // Same 64 KiB sample + scoring as read_text.
            let mut f = std::fs::File::open(path)?;
            let mut prefix = Vec::new();
            f.by_ref().take(64 * 1024).read_to_end(&mut prefix)?;
            drop(f);
            let sample = String::from_utf8_lossy(&prefix);
            let candidates = [',', '\t', ';', '|', ' '];
            let mut best = (0, ',');
            for c in candidates {
                let score = crate::source::delimiter_score(&sample, c, &format);
                if score > best.0 {
                    best = (score, c);
                }
            }
            format.delimiters = vec![best.1];
        }
        if format.delimiters.is_empty() {
            return Err(DemandError::unsupported("no delimiters configured"));
        }
        // Private snapshot: stable bytes under our own ownership.
        let snapshot_path = std::env::temp_dir().join(format!(
            "matcha-demand-{}-{}.csv",
            std::process::id(),
            TABLE_ID.fetch_add(1, AtomicOrdering::Relaxed)
        ));
        std::fs::copy(path, &snapshot_path)?;
        let snapshot = std::fs::File::open(&snapshot_path)?;
        let map = bc_core::mmap::map_readonly(&snapshot)?;
        drop(snapshot);
        let base = if has_bom { 3 } else { 0 };
        let (spans, columns, endings) =
            build_span_index(&map, base, &format, options.ctrl_z_eof, cancel)?;
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
        let regional = if format.regional.use_system {
            options.system_regional.clone()
        } else {
            format.regional.clone()
        };
        let fast = if fast_path_ok(&format) {
            let q = format.qualifier.unwrap() as u8;
            let delims = format.delimiters.iter().map(|&d| d as u8).collect();
            Some((q, delims))
        } else {
            None
        };
        let id = TABLE_ID.fetch_add(1, AtomicOrdering::Relaxed) as usize;
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let inner = Arc::new(DemandInner {
            map: Some(map),
            id,
            snapshot_path,
            spans,
            header_span: None,
            header_rows: 0,
            columns: columns as u32,
            options: format,
            regional,
            has_bom,
            default_ending,
            name,
            fast,
            next_row: AtomicU32::new(0),
            next_col: AtomicU32::new(0),
            cache_cap: CACHE_CAP_BYTES,
        });
        // Identity counters start above the physical ranges.
        inner
            .next_row
            .store(inner.spans.len() as u32, AtomicOrdering::Relaxed);
        inner.next_col.store(inner.columns, AtomicOrdering::Relaxed);
        let version = Arc::new(Version {
            rows: (0..inner.spans.len() as u32).collect(),
            cols: (0..inner.columns).collect(),
            edits: HashMap::new(),
            display_overrides: HashMap::new(),
            edited_rows: HashSet::new(),
            dirty_rows: HashSet::new(),
            all_rows_edited: false,
            headers: None,
            styles: Vec::new(),
            revision: crate::model::revision(),
        });
        let mut sheet = Self {
            inner,
            version,
            cache: Mutex::new(BoundedRowCache::new(CACHE_CAP_BYTES)),
            supply: AtomicU8::new(SUPPLY_TYPED),
        };
        // Header handling identical to read_text: Names always promotes row 0,
        // Auto runs the shared detector, Data keeps data rows.
        let header_mode = options.header.unwrap_or(sheet.inner.options.header);
        let promote = header_mode == HeaderMode::Names
            || (header_mode == HeaderMode::Auto && crate::source::detect_header_data(&sheet));
        if promote && !sheet.inner.spans.is_empty() {
            let headers = sheet.parse_record(0).fields;
            sheet.inner_mut_header_promotion(headers);
        }
        Ok(sheet)
    }

    fn inner_mut_header_promotion(&mut self, headers: Vec<String>) {
        // Build-time only: no snapshot/version sharing exists yet.
        let inner = Arc::get_mut(&mut self.inner).expect("build-time exclusive");
        inner.header_rows = 1;
        inner.header_span = Some(inner.spans[0]);
        inner.spans.remove(0);
        inner
            .next_row
            .store(inner.spans.len() as u32, AtomicOrdering::Relaxed);
        let version = Arc::make_mut(&mut self.version);
        version.rows = (0..inner.spans.len() as u32).collect();
        version.headers = Some(headers);
    }

    /// Sheet/tab name (source file name, `read_path` parity).
    pub fn name(&self) -> &str {
        &self.inner.name
    }
    /// Worker-snapshot supply mode for the compare phase (see SUPPLY_RAW).
    pub fn set_compare_raw_supply(&self, raw: bool) {
        self.supply.store(
            if raw { SUPPLY_RAW } else { SUPPLY_TYPED },
            AtomicOrdering::Relaxed,
        );
    }

    /// Mapped virtual bytes of the snapshot (virtual size, not residency).
    pub fn mapped_virtual_bytes(&self) -> u64 {
        self.inner.bytes().len() as u64
    }

    fn version_mut(&mut self) -> &mut Version {
        // Every mutation bumps the revision and clears the viewport cache:
        // cached display strings must never outlive a content change.
        self.cache.lock().expect("cache poisoned").clear();
        let v = Arc::make_mut(&mut self.version);
        v.revision = crate::model::revision();
        v
    }

    fn parse_record(&self, record: usize) -> crate::source::Record {
        let slice = self.inner.record_bytes(record);
        let mut records = Records::new(slice, self.inner.options.clone(), false)
            .expect("options already validated");
        match records.next_record(&bc_core::cancel::NeverCancel) {
            Ok(Some(record)) => record,
            other => panic!("record re-parse failed at {record}: {other:?}"),
        }
    }

    /// Production-parser field of a physical record (fallback path), TLS-cached
    /// per row per thread.
    fn parsed_field(&self, record: usize, column: usize) -> Option<String> {
        ROW_SCRATCH.with(|s| {
            let mut map = s.borrow_mut();
            if !map.contains_key(&self.inner.id) && map.len() >= 4 {
                map.clear();
            }
            let entry = map
                .entry(self.inner.id)
                .or_insert_with(|| (u32::MAX, Vec::new()));
            if entry.0 != record as u32 {
                entry.1 = self.parse_record(record).fields;
                entry.0 = record as u32;
            }
            entry.1.get(column).cloned()
        })
    }

    /// Raw field text of a physical record: borrowed slice for qualifier-free
    /// records (fast path), owned parse via the production parser otherwise.
    fn raw_field_phys(&self, record: usize, column: usize) -> Option<std::borrow::Cow<'_, str>> {
        let body = strip_ending(self.inner.record_bytes(record));
        if let Some((q, ref delims)) = self.inner.fast {
            if !body.contains(&q) {
                let mut start = 0usize;
                let mut idx = 0usize;
                for (i, &b) in body.iter().enumerate() {
                    if delims.contains(&b) {
                        if idx == column {
                            return std::str::from_utf8(&body[start..i])
                                .ok()
                                .map(std::borrow::Cow::Borrowed);
                        }
                        idx += 1;
                        start = i + 1;
                    }
                }
                return if idx == column {
                    std::str::from_utf8(&body[start..])
                        .ok()
                        .map(std::borrow::Cow::Borrowed)
                } else {
                    None
                };
            }
        }
        self.parsed_field(record, column)
            .map(std::borrow::Cow::Owned)
    }

    /// Same interpretation as the dense load path: textual values keep the
    /// borrowed source text, typed values interpret freshly.
    fn value_of<'a>(&self, text: &'a str) -> CellData<'a> {
        match crate::regional::interpret(text, &self.inner.regional) {
            CellValue::Text(_) => CellData::Text(std::borrow::Cow::Borrowed(text)),
            other => CellData::from_owned_value(other),
        }
    }

    fn ids(&self, row: usize, column: usize) -> Option<(RowId, ColId)> {
        let v = &self.version;
        Some((*v.rows.get(row)?, *v.cols.get(column)?))
    }

    fn overlay(&self, row: RowId, col: ColId) -> Option<&Cell> {
        self.version.edits.get(&(row, col))
    }

    /// Physical record index for a row identity still backed by the source.
    fn physical_record(&self, row: RowId) -> Option<usize> {
        let idx = row as usize;
        (idx < self.inner.spans.len()).then_some(idx)
    }

    /// Physical column index for a column identity still backed by the source.
    fn physical_column(&self, col: ColId) -> Option<usize> {
        (col < self.inner.columns).then_some(col as usize)
    }

    /// Typed cell value as the dense load would have materialized it. In-range
    /// cells without physical backing (inserted rows/columns, ragged short
    /// records) read as Empty, matching dense columnar fill.
    fn typed_value(&self, row: usize, column: usize) -> Option<CellData<'_>> {
        let (rid, cid) = self.ids(row, column)?;
        if let Some(cell) = self.overlay(rid, cid) {
            return Some(CellData::from(&cell.value));
        }
        let text = match (self.physical_record(rid), self.physical_column(cid)) {
            (Some(record), Some(column)) => match self.raw_field_phys(record, column) {
                Some(text) => text,
                None => return Some(CellData::Empty),
            },
            _ => return Some(CellData::Empty),
        };
        if self.supply.load(AtomicOrdering::Relaxed) == SUPPLY_RAW {
            // Compare phase only; see set_compare_raw_supply.
            return Some(CellData::Text(text));
        }
        match text {
            std::borrow::Cow::Borrowed(s) => Some(self.value_of(s)),
            std::borrow::Cow::Owned(s) => Some(CellData::from_owned_value(
                crate::regional::interpret(&s, &self.inner.regional),
            )),
        }
    }

    /// Silent size growth (Dense `ensure_size` parity): fresh identities, no
    /// edit marks, no header changes, no revision bump; the viewport cache is
    /// cleared because cached row strings span the old column count.
    pub(crate) fn ensure_size(&mut self, rows: usize, columns: usize) {
        if self.column_count() >= columns && self.row_count() >= rows {
            return;
        }
        self.cache.lock().expect("cache poisoned").clear();
        let v = Arc::make_mut(&mut self.version);
        while v.cols.len() < columns {
            let cid = self.inner.next_col.fetch_add(1, AtomicOrdering::Relaxed);
            v.cols.push(cid);
        }
        while v.rows.len() < rows {
            let rid = self.inner.next_row.fetch_add(1, AtomicOrdering::Relaxed);
            v.rows.push(rid);
        }
    }

    /// Viewport cache counters (hits/misses/evictions/bytes) for measurement.
    pub fn cache_stats(&self) -> CacheStats {
        let c = self.cache.lock().expect("cache poisoned");
        CacheStats {
            hits: c.hits,
            misses: c.misses,
            evictions: c.evictions,
            max_bytes: c.max_bytes,
            live_bytes: c.bytes,
            cap_bytes: c.cap,
        }
    }

    /// `Sheet::display_at` semantics: override text wins, edited cells show
    /// their typed display, untouched physical cells keep the source spelling.
    pub fn display_text_at(&self, row: usize, column: usize) -> Option<String> {
        let (rid, cid) = self.ids(row, column)?;
        if let Some(text) = self.version.display_overrides.get(&(rid, cid)) {
            return Some(text.to_string());
        }
        if let Some(cell) = self.overlay(rid, cid) {
            return Some(cell.display());
        }
        let text = match (self.physical_record(rid), self.physical_column(cid)) {
            (Some(record), Some(column)) => self.raw_field_phys(record, column),
            _ => None,
        };
        Some(text.map(|t| t.into_owned()).unwrap_or_default())
    }

    /// `Sheet::preview` semantics: bounded rendering without cloning large text.
    pub fn preview(&self, row: usize, column: usize, start: usize, limit: usize) -> Option<String> {
        let (rid, cid) = self.ids(row, column)?;
        if let Some(text) = self.version.display_overrides.get(&(rid, cid)) {
            return Some(text.chars().skip(start).take(limit).collect());
        }
        if let Some(cell) = self.overlay(rid, cid) {
            return Some(match &cell.value {
                CellValue::Text(t) | CellValue::Error(t) => {
                    t.chars().skip(start).take(limit).collect()
                }
                _ => cell.display().chars().skip(start).take(limit).collect(),
            });
        }
        let text = match (self.physical_record(rid), self.physical_column(cid)) {
            (Some(record), Some(column)) => self.raw_field_phys(record, column),
            _ => None,
        };
        let text = text.unwrap_or_default();
        Some(text.chars().skip(start).take(limit).collect())
    }

    /// Owned typed cell snapshot (`Sheet::cell(...).cloned()` equivalent).
    pub fn cell_snapshot(&self, row: usize, column: usize) -> Option<Cell> {
        let (rid, cid) = self.ids(row, column)?;
        if let Some(cell) = self.overlay(rid, cid) {
            return Some(cell.clone());
        }
        let text = match (self.physical_record(rid), self.physical_column(cid)) {
            (Some(record), Some(column)) => self.raw_field_phys(record, column),
            _ => None,
        };
        match text {
            Some(text) => Some(Cell::new(crate::regional::interpret(
                &text,
                &self.inner.regional,
            ))),
            None => Some(Cell::default()),
        }
    }

    pub fn column_name(&self, column: usize) -> String {
        self.version
            .headers
            .as_ref()
            .and_then(|h| h.get(column))
            .cloned()
            .unwrap_or_else(|| (column + 1).to_string())
    }
    pub fn row_count(&self) -> usize {
        self.version.rows.len()
    }
    pub fn column_count(&self) -> usize {
        self.version.cols.len()
    }
    pub fn revision(&self) -> u64 {
        self.version.revision
    }
    pub fn fresh_revision(&mut self) {
        self.version_mut();
    }
    pub fn is_edited(&self, row: usize) -> bool {
        let v = &self.version;
        if v.all_rows_edited {
            return row < v.rows.len();
        }
        v.rows.get(row).is_some_and(|id| v.edited_rows.contains(id))
    }
    pub fn style(&self, id: Option<StyleId>) -> Option<&Style> {
        self.version.styles.get(id?.0 as usize)
    }
    pub fn intern_style(&mut self, style: Style) -> StyleId {
        if let Some(i) = self.version.styles.iter().position(|s| *s == style) {
            return StyleId(i as u32);
        }
        let v = self.version_mut();
        let i = v.styles.len();
        v.styles.push(style);
        StyleId(i as u32)
    }
    pub(crate) fn insert_display_override(&mut self, row: usize, column: usize, text: Arc<str>) {
        if let Some((rid, cid)) = self.ids(row, column) {
            self.version_mut()
                .display_overrides
                .insert((rid, cid), text);
        }
    }
    pub fn source_is_workbook(&self) -> bool {
        false
    }

    pub fn set_cell(&mut self, row: usize, column: usize, cell: Cell) -> Result<()> {
        let Some((rid, cid)) = self.ids(row, column) else {
            return Err(invalid("cell out of range"));
        };
        if cell.style.is_some_and(|id| self.style(Some(id)).is_none()) {
            return Err(invalid("unknown style id"));
        }
        let v = self.version_mut();
        v.edits.insert((rid, cid), cell);
        v.display_overrides.remove(&(rid, cid));
        v.edited_rows.insert(rid);
        v.dirty_rows.insert(rid);
        Ok(())
    }

    /// Insert a logical row; provided cells land in the overlay (Sheet parity:
    /// an inserted row counts as edited on every column).
    pub fn insert_row(&mut self, at: usize, cells: Vec<Cell>) -> Result<()> {
        if at > self.row_count() {
            return Err(invalid("row insertion out of range"));
        }
        if cells
            .iter()
            .any(|c| c.style.is_some_and(|id| self.style(Some(id)).is_none()))
        {
            return Err(invalid("unknown style id"));
        }
        // ensure_size parity: grow columns to fit the provided cells.
        while self.column_count() < cells.len() {
            let cid = self.inner.next_col.fetch_add(1, AtomicOrdering::Relaxed);
            self.version_mut().cols.push(cid);
        }
        let rid = self.inner.next_row.fetch_add(1, AtomicOrdering::Relaxed);
        let v = self.version_mut();
        v.rows.insert(at, rid);
        for (i, cell) in cells.into_iter().enumerate() {
            if let Some(&cid) = v.cols.get(i) {
                v.edits.insert((rid, cid), cell);
            }
        }
        v.edited_rows.insert(rid);
        v.dirty_rows.insert(rid);
        Ok(())
    }

    pub fn remove_row_indices(&mut self, rows: &[usize]) -> Result<()> {
        let count = self.row_count();
        if rows.iter().any(|&r| r >= count) {
            return Err(invalid("row deletion out of bounds"));
        }
        if rows.is_empty() {
            return Ok(());
        }
        let deleted: HashSet<usize> = rows.iter().copied().collect();
        let min_deleted = rows.iter().copied().min().unwrap_or(0);
        let v = self.version_mut();
        v.rows = v
            .rows
            .iter()
            .enumerate()
            .filter_map(|(i, &id)| (!deleted.contains(&i)).then_some(id))
            .collect();
        // Sheet marks the slot the deletion collapsed onto (index semantics);
        // with identity marks we mark the identity now at that position.
        if !v.rows.is_empty() {
            let at = min_deleted.min(v.rows.len() - 1);
            v.edited_rows.insert(v.rows[at]);
        }
        Ok(())
    }

    pub fn remove_rows(&mut self, range: std::ops::Range<usize>) -> Result<()> {
        if range.start > range.end || range.end > self.row_count() {
            return Err(invalid("row range out of bounds"));
        }
        self.remove_row_indices(&range.collect::<Vec<_>>())
    }

    /// Reorder logical rows by source-row permutation (Sheet::reorder_rows).
    pub fn reorder_rows(&mut self, order: &[usize]) -> Result<()> {
        let count = self.row_count();
        let seen: HashSet<_> = order.iter().copied().collect();
        if seen.len() != order.len() || order.iter().any(|&r| r >= count) {
            return Err(invalid("invalid row permutation"));
        }
        let v = self.version_mut();
        let old = std::mem::take(&mut v.rows);
        v.rows = order.iter().map(|&r| old[r]).collect();
        for (new, &old_row) in order.iter().enumerate() {
            if new != old_row {
                v.edited_rows.insert(v.rows[new]);
            }
        }
        Ok(())
    }

    /// Plan a stable lexicographic sort (shared generic implementation).
    pub fn sorted_row_order(
        &self,
        rows: &[usize],
        keys: &[(usize, crate::ColumnHandling)],
        descending: bool,
    ) -> Result<Option<Vec<usize>>> {
        crate::editing::sorted_row_order_data(self, rows, keys, descending)
    }

    pub fn insert_column(&mut self, at: usize, name: String) -> Result<()> {
        if at > self.column_count() {
            return Err(invalid("column insertion out of bounds"));
        }
        let cid = self.inner.next_col.fetch_add(1, AtomicOrdering::Relaxed);
        let v = self.version_mut();
        v.cols.insert(at, cid);
        if let Some(headers) = &mut v.headers {
            headers.resize(v.cols.len() - 1, String::new());
            headers.insert(at, name);
        }
        v.all_rows_edited = true;
        Ok(())
    }

    pub fn remove_column(&mut self, at: usize) -> Result<()> {
        if at >= self.column_count() {
            return Err(invalid("column deletion out of bounds"));
        }
        let v = self.version_mut();
        v.cols.remove(at);
        if let Some(headers) = &mut v.headers {
            if at < headers.len() {
                headers.remove(at);
            }
        }
        v.all_rows_edited = true;
        Ok(())
    }

    /// One atomic rectangular paste; callers supply decoded cell values.
    pub fn paste_cells(&mut self, row: usize, col: usize, values: Vec<Vec<Cell>>) -> Result<()> {
        let width = values.iter().map(Vec::len).max().unwrap_or(0);
        while self.column_count() < col + width {
            self.insert_column(self.column_count(), String::new())?;
        }
        while self.row_count() < row + values.len() {
            self.insert_row(self.row_count(), Vec::new())?;
        }
        for (r, values) in values.into_iter().enumerate() {
            for (c, value) in values.into_iter().enumerate() {
                self.set_cell(row + r, col + c, value)?;
            }
        }
        Ok(())
    }

    /// Display strings of one logical row, bounded-cached for clean physical
    /// rows; dirty/inserted rows are rebuilt each time and never cached.
    pub fn cached_row_displays(&self, row: usize) -> Option<Arc<Vec<String>>> {
        let v = &self.version;
        let rid = *v.rows.get(row)?;
        let clean = !v.dirty_rows.contains(&rid)
            && self.physical_record(rid).is_some()
            && !v.display_overrides.keys().any(|&(r, _)| r == rid);
        if !clean {
            let mut strings = Vec::with_capacity(v.cols.len());
            for c in 0..v.cols.len() {
                strings.push(self.display_text_at(row, c).unwrap_or_default());
            }
            return Some(Arc::new(strings));
        }
        let mut cache = self.cache.lock().expect("cache poisoned");
        Some(cache.get_or_load(rid, || {
            let mut strings = Vec::with_capacity(self.version.cols.len());
            for c in 0..self.version.cols.len() {
                strings.push(self.display_text_at(row, c).unwrap_or_default());
            }
            Arc::new(strings)
        }))
    }

    /// Streaming write-back byte-equal to dense `write_delimited`: clean rows
    /// copy source bytes verbatim, dirty rows splice serialized edits into the
    /// re-parsed record spans, inserted rows serialize fully; a changed column
    /// structure re-serializes every row like the dense fallback branch.
    pub fn write_delimited(&self, mut writer: impl std::io::Write) -> Result<()> {
        if self.inner.has_bom {
            writer.write_all(b"\xEF\xBB\xBF")?;
        }
        let options = &self.inner.options;
        let delimiter = options
            .delimiters
            .first()
            .copied()
            .unwrap_or(',')
            .to_string();
        let identity_cols = self
            .version
            .cols
            .iter()
            .enumerate()
            .all(|(i, &c)| c as usize == i)
            && self.version.cols.len() == self.inner.columns as usize;
        if let Some(headers) = &self.version.headers {
            if identity_cols {
                // Verbatim header record bytes (promoted span, includes ending).
                if let Some((off, len)) = self.inner.header_span {
                    writer.write_all(&self.inner.bytes()[off as usize..(off + len) as usize])?;
                }
            } else {
                let mut text = String::new();
                for (i, h) in headers.iter().enumerate() {
                    if i > 0 {
                        text.push_str(&delimiter);
                    }
                    text.push_str(&quoted_field(h, options)?);
                }
                text.push_str(&self.inner.default_ending);
                writer.write_all(text.as_bytes())?;
            }
        }
        for row in 0..self.row_count() {
            let rid = self.version.rows[row];
            match self.physical_record(rid) {
                None => {
                    // Inserted row: full serialization, default ending.
                    let mut text = String::new();
                    for c in 0..self.column_count() {
                        if c > 0 {
                            text.push_str(&delimiter);
                        }
                        text.push_str(&quoted_field(
                            &self.display_text_at(row, c).unwrap_or_default(),
                            options,
                        )?);
                    }
                    text.push_str(&self.inner.default_ending);
                    writer.write_all(text.as_bytes())?;
                }
                Some(record) if identity_cols && !self.version.dirty_rows.contains(&rid) => {
                    // Untouched row: source bytes verbatim (includes ending).
                    writer.write_all(self.inner.record_bytes(record))?;
                }
                Some(record) if identity_cols => {
                    // Dirty row, identity columns: dense raw-branch splice.
                    let parsed = self.parse_record(record);
                    let raw = &parsed.raw;
                    let mut text = String::new();
                    let mut offset = 0usize;
                    for (col, &(start, end)) in raw.fields.iter().enumerate() {
                        text.push_str(&raw.text[offset..start as usize]);
                        let cid = self.version.cols[col];
                        if self.version.edits.contains_key(&(rid, cid)) {
                            text.push_str(&quoted_field(
                                &self.display_text_at(row, col).unwrap_or_default(),
                                options,
                            )?);
                        } else {
                            text.push_str(&raw.text[start as usize..end as usize]);
                        }
                        offset = end as usize;
                    }
                    text.push_str(&raw.text[offset..]);
                    text.push_str(&raw.ending);
                    writer.write_all(text.as_bytes())?;
                }
                Some(record) => {
                    // Column structure changed: dense fallback branch (full
                    // re-serialization of the current logical view).
                    let parsed = self.parse_record(record);
                    let ending = if parsed.raw.ending.is_empty() {
                        self.inner.default_ending.as_str()
                    } else {
                        parsed.raw.ending.as_ref()
                    };
                    let mut text = String::new();
                    for c in 0..self.column_count() {
                        if c > 0 {
                            text.push_str(&delimiter);
                        }
                        text.push_str(&quoted_field(
                            &self.display_text_at(row, c).unwrap_or_default(),
                            options,
                        )?);
                    }
                    text.push_str(ending);
                    writer.write_all(text.as_bytes())?;
                }
            }
        }
        Ok(())
    }

    /// Export semantics of dense `export_csv`: no source-byte preservation,
    /// every cell re-serialized from the current view.
    pub fn export_delimited(&self, mut writer: impl std::io::Write) -> Result<()> {
        let options = &TableOptions::default();
        let delimiter = ",".to_string();
        let ending = "\r\n";
        if let Some(headers) = &self.version.headers {
            let mut text = String::new();
            for (i, h) in headers.iter().enumerate() {
                if i > 0 {
                    text.push_str(&delimiter);
                }
                text.push_str(&quoted_field(h, options)?);
            }
            text.push_str(ending);
            writer.write_all(text.as_bytes())?;
        }
        for row in 0..self.row_count() {
            let mut text = String::new();
            for c in 0..self.column_count() {
                if c > 0 {
                    text.push_str(&delimiter);
                }
                text.push_str(&quoted_field(
                    &self.display_text_at(row, c).unwrap_or_default(),
                    options,
                )?);
            }
            text.push_str(ending);
            writer.write_all(text.as_bytes())?;
        }
        Ok(())
    }

    /// Staged atomic replacement, same protocol as dense `save_as`.
    pub fn save_as(&self, path: &Path) -> Result<()> {
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
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            self.write_delimited(&mut file)?;
            file.sync_all()?;
            drop(file);
            std::fs::rename(&temp, path)?;
            Ok::<_, bc_core::CoreError>(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        result
    }
}

/// Dense `quoted_field` lives in source.rs; re-implemented here over the same
/// rule so demand save stays byte-equal without widening source.rs visibility.
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

impl TableData for DemandSheet {
    fn row_count(&self) -> usize {
        self.row_count()
    }
    fn column_count(&self) -> usize {
        self.column_count()
    }
    fn cell_info(&self, row: usize, column: usize) -> Option<CellInfo<'_>> {
        let (rid, cid) = self.ids(row, column)?;
        if let Some(cell) = self.overlay(rid, cid) {
            let number_format = self
                .style(cell.style)
                .and_then(|s| s.number_format.as_deref());
            return Some(CellInfo {
                value: CellData::from(&cell.value),
                formula: cell.formula.as_deref(),
                number_format,
            });
        }
        Some(CellInfo {
            value: self.typed_value(row, column)?,
            formula: None,
            number_format: None,
        })
    }
    fn display_into(&self, row: usize, column: usize, out: &mut String) {
        if let Some(value) = self.typed_value(row, column) {
            value.display_into(out);
        }
    }
    fn display_text(&self, row: usize, column: usize) -> String {
        self.display_text_at(row, column).unwrap_or_default()
    }
    fn headers(&self) -> Option<&[String]> {
        self.version.headers.as_deref()
    }
    fn regional(&self) -> &RegionalSettings {
        &self.inner.regional
    }
    fn revision(&self) -> u64 {
        self.revision()
    }
    fn collect_edited_rows(&self, out: &mut HashSet<usize>) {
        let v = &self.version;
        if v.all_rows_edited {
            out.extend(0..v.rows.len());
            return;
        }
        for (i, id) in v.rows.iter().enumerate() {
            if v.edited_rows.contains(id) {
                out.insert(i);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bc_core::cancel::NeverCancel;

    fn write_temp(rows: &[&str]) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "matcha-demand-test-{}-{}.csv",
            std::process::id(),
            TABLE_ID.fetch_add(1, AtomicOrdering::Relaxed)
        ));
        std::fs::write(&path, rows.join("\r\n") + "\r\n").unwrap();
        path
    }

    fn open(path: &Path) -> DemandSheet {
        let options = ReadOptions {
            header: Some(HeaderMode::Data),
            ..Default::default()
        };
        DemandSheet::open(path, &options, &NeverCancel).unwrap()
    }

    #[test]
    fn values_display_and_edits_roundtrip() {
        let path = write_temp(&["a,1,0.250", "b,2,x", "c,3,\"q,\"\"q\""]);
        let mut sheet = open(&path);
        assert_eq!(sheet.row_count(), 3);
        assert_eq!(sheet.column_count(), 3);
        // Typed access mirrors dense interpret.
        assert_eq!(sheet.typed_value(0, 1), Some(CellData::Number(1.0)));
        // Display keeps the source spelling (trailing zero).
        assert_eq!(sheet.display_text_at(0, 2).as_deref(), Some("0.250"));
        // Quoted field via production parser fallback.
        assert_eq!(sheet.display_text_at(2, 2).as_deref(), Some("q,\"q"));
        // Value edit through the overlay.
        sheet.set_cell(1, 1, Cell::text("9")).unwrap();
        assert_eq!(sheet.display_text_at(1, 1).as_deref(), Some("9"));
        assert!(sheet.is_edited(1));
        assert!(!sheet.is_edited(0));
        // Insert/reorder/remove keep identities stable.
        sheet.insert_row(1, vec![Cell::text("new")]).unwrap();
        assert_eq!(sheet.display_text_at(1, 0).as_deref(), Some("new"));
        sheet.reorder_rows(&[0, 2, 1, 3]).unwrap();
        assert_eq!(sheet.display_text_at(1, 0).as_deref(), Some("b"));
        assert_eq!(sheet.display_text_at(2, 0).as_deref(), Some("new"));
        sheet.remove_row_indices(&[0]).unwrap();
        assert_eq!(sheet.row_count(), 3);
        assert_eq!(sheet.display_text_at(0, 0).as_deref(), Some("b"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_splices_edits_and_preserves_clean_bytes() {
        let path = write_temp(&["a,1,0.250", "b,2,x", "c,3,y"]);
        let mut sheet = open(&path);
        sheet.set_cell(1, 1, Cell::text("20")).unwrap();
        let mut out = Vec::new();
        sheet.write_delimited(&mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text, "a,1,0.250\r\nb,20,x\r\nc,3,y\r\n");
        // Inserted row serializes; removed row disappears.
        sheet
            .insert_row(1, vec![Cell::text("i"), Cell::text("5"), Cell::text("z")])
            .unwrap();
        sheet.remove_row_indices(&[2]).unwrap();
        let mut out = Vec::new();
        sheet.write_delimited(&mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text, "a,1,0.250\r\ni,5,z\r\nc,3,y\r\n");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn bounded_cache_caps_bytes_and_bypasses_dirty_rows() {
        let path = write_temp(&["a,1", "b,2", "c,3"]);
        let mut sheet = open(&path);
        let a = sheet.cached_row_displays(0).unwrap();
        assert_eq!(a.as_slice(), ["a".to_string(), "1".to_string()]);
        sheet.set_cell(0, 0, Cell::text("z")).unwrap();
        let b = sheet.cached_row_displays(0).unwrap();
        assert_eq!(b.as_slice(), ["z".to_string(), "1".to_string()]);
        let _ = std::fs::remove_file(&path);
    }

    fn assert_index_equal(body: &[u8], format: &TableOptions) {
        let via_records = records_span_index(body, 0, format, false, &NeverCancel);
        let eligible = fast_span_eligible(format, false, body);
        let via_fast = eligible.then(|| fast_span_index(body, 0, format, false, &NeverCancel));
        match (via_records, via_fast) {
            (Ok(a), Some(Ok(b))) => assert_eq!(
                (a.0, a.1, a.2),
                (b.0, b.1, b.2),
                "span index divergence on {:?}",
                String::from_utf8_lossy(body)
            ),
            (Err(a), Some(Err(b))) => assert_eq!(
                a.to_string(),
                b.to_string(),
                "error divergence on {:?}",
                String::from_utf8_lossy(body)
            ),
            (expected, None) => {
                // Ineligible alphabet: fast scan must not run; nothing to compare.
                drop(expected);
            }
            (a, Some(b)) => panic!(
                "path divergence on {:?}: records={a:?} fast={b:?}",
                String::from_utf8_lossy(body)
            ),
        }
    }

    #[test]
    fn fast_span_index_matches_records_parser_on_synthetic_corpus() {
        let comma = TableOptions::default();
        let consecutive = TableOptions {
            consecutive_delimiters: true,
            ..TableOptions::default()
        };
        let multi = TableOptions {
            delimiters: vec![',', '\t'],
            ..TableOptions::default()
        };
        let no_qualifier = TableOptions {
            qualifier: None,
            ..TableOptions::default()
        };
        // Legal per Records::new validation: '\r' can never engage quoting
        // (terminator check runs first), and the scan never falls back on it.
        let cr_qualifier = TableOptions {
            qualifier: Some('\r'),
            ..TableOptions::default()
        };
        let formats = [&comma, &consecutive, &multi, &no_qualifier, &cr_qualifier];
        let corpus = [
            "a,b,c\r\n1,2,3\r\n",
            "a,b\n1,2\n",
            "a,b\r1,2\r",
            "a,b\r\nc,d\ne,f\rg,h\r\n",
            "a,\"x\ny\",c\r\n1,2,3\r\n",
            "a,\"x,y\",c\r\n",
            "a,\"x\"\"y\",c\r\n",
            "a,b\"c,d\r\n",
            "a,b,c\r\n1,2\r\nx\r\n",
            "a,b\r\n\r\n1,2\r\n",
            "a,b\r\n1,2",
            "a,b\r\n\u{1a}1,2\r\n",
            "a,\"x\n1,2",
            "a,,b\r\n,,,\r\n",
            "a\r\"x\ny\"\nb\r",
            "",
            "\r\n",
            "\"\"\r\n",
            "a,\"x\"y,c\r\n",
            "\u{1a}",
            "\n\n\n",
            "\r\r\r",
            "a\rb\rc\n",
            "\"x\"",
            "a,\"x\r\ny\",b\r\n",
            "\r\n1,2\r\n",
            "a,\"x\ny\"\r\n\"p\",2\r\n",
            "1,000.50,abc\r\n",
            "a\tb\r\nc\td\r\n",
            "a,b,c,",
            ",,,",
        ];
        for body in corpus {
            for format in formats {
                assert_index_equal(body.as_bytes(), format);
            }
        }
        // Long lines exercise the in-record cancel cadence and 4 KiB steps.
        let long = format!("a,{}\r\n1,2\r\n", "x".repeat(5000));
        for format in formats {
            assert_index_equal(long.as_bytes(), format);
        }
        // ctrl-z eligibility: a \x1A byte anywhere forces the full parser path.
        assert!(!fast_span_eligible(&comma, true, b"a\x1ab"));
        assert!(fast_span_eligible(&comma, true, b"a,b\r\n"));
        // With ctrl-z off, \x1A is ordinary content.
        assert!(fast_span_eligible(&comma, false, b"a\x1ab"));
        // Non-ASCII alphabets are never eligible.
        let wide = TableOptions {
            delimiters: vec!['α'],
            ..TableOptions::default()
        };
        assert!(!fast_span_eligible(&wide, false, "aαb".as_bytes()));
    }

    #[test]
    fn fast_span_index_matches_records_parser_on_real_inputs() {
        let dir = std::path::Path::new("../../target/table-wave7/audit/inputs");
        if !dir.is_dir() {
            eprintln!("skipping real-input differential: inputs not present");
            return;
        }
        let format = TableOptions::default();
        for name in ["table-left.csv", "table-right.csv"] {
            let bytes = std::fs::read(dir.join(name)).unwrap();
            assert!(fast_span_eligible(&format, false, &bytes));
            let expected = records_span_index(&bytes, 0, &format, false, &NeverCancel).unwrap();
            let fast = fast_span_index(&bytes, 0, &format, false, &NeverCancel).unwrap();
            assert_eq!(
                (expected.0, expected.1, expected.2),
                (fast.0, fast.1, fast.2),
                "span index divergence on {name}"
            );
        }
    }
}
