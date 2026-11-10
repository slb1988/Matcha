//! Minimal boundary test for the prototype's on-demand access path:
//! a byte-buffer + record-span `TableData` implementation (same `Records`
//! parser, no mmap) must produce the identical compare digest/statistics as
//! the materialized `Sheet` path, including a quoted/escaped field case.

use bc_core::{align::AlignmentOverrides, cancel::NeverCancel};
use bc_engine_table::{
    columns::ColumnMapping,
    data::{CellData, CellInfo, TableData},
    inference::infer_keys_data,
    map::{compare_alignment_data, TableAlignOutput},
    rules::Layers,
    source::{detect_header_data, read_text, ReadOptions, Records},
    KeyAlign, TableOptions, TableRules,
};
use bc_formats::model::RegionalSettings;
use std::io::Cursor;
use std::sync::{Mutex, OnceLock};

struct BufferTable {
    bytes: Vec<u8>,
    spans: Vec<(u32, u32)>,
    header_rows: u32,
    headers: Option<Vec<String>>,
    columns: usize,
    options: TableOptions,
    regional: RegionalSettings,
    // tests are single-threaded per table; Mutex keeps Sync without TLS
    scratch: Mutex<(u32, Vec<String>)>,
}

static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Consumed-byte-counting reader wrapper (same pattern as the prototype):
/// record boundaries come from bytes the parser actually consumed.
struct Counting<'a> {
    inner: &'a [u8],
    pos: std::rc::Rc<std::cell::Cell<usize>>,
}
impl std::io::Read for Counting<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = std::io::Read::read(&mut self.inner, buf)?;
        self.pos.set(self.pos.get() + n);
        Ok(n)
    }
}
impl std::io::BufRead for Counting<'_> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        Ok(self.inner)
    }
    fn consume(&mut self, n: usize) {
        self.inner = &self.inner[n..];
        self.pos.set(self.pos.get() + n);
    }
}

impl BufferTable {
    fn build(text: &str) -> Self {
        let options = TableOptions::default();
        let bytes = text.as_bytes().to_vec();
        let mut spans = Vec::new();
        let mut columns = 0;
        let pos = std::rc::Rc::new(std::cell::Cell::new(0usize));
        let mut records = Records::new(
            Counting {
                inner: &bytes,
                pos: pos.clone(),
            },
            options.clone(),
            false,
        )
        .unwrap();
        let mut prev_end = 0usize;
        while let Some(record) = records.next_record(&NeverCancel).unwrap() {
            let end = pos.get();
            columns = columns.max(record.fields.len());
            spans.push((prev_end as u32, (end - prev_end) as u32));
            prev_end = end;
        }
        let mut table = Self {
            bytes,
            spans,
            header_rows: 0,
            headers: None,
            columns,
            options,
            regional: RegionalSettings::default(),
            scratch: Mutex::new((u32::MAX, Vec::new())),
        };
        if detect_header_data(&table) {
            table.headers = Some(table.parse_record(0));
            table.header_rows = 1;
        }
        table
    }
    fn parse_record(&self, index: usize) -> Vec<String> {
        let (off, len) = self.spans[index];
        let slice = &self.bytes[off as usize..(off + len) as usize];
        let mut records = Records::new(Cursor::new(slice), self.options.clone(), false).unwrap();
        records
            .next_record(&NeverCancel)
            .unwrap()
            .expect("record present")
            .fields
    }
}

impl TableData for BufferTable {
    fn row_count(&self) -> usize {
        self.spans.len() - self.header_rows as usize
    }
    fn column_count(&self) -> usize {
        self.columns
    }
    fn cell_info(&self, row: usize, column: usize) -> Option<CellInfo<'_>> {
        let mut guard = self.scratch.lock().ok()?;
        if guard.0 != row as u32 {
            guard.1 = self.parse_record(self.header_rows as usize + row);
            guard.0 = row as u32;
        }
        let text = guard.1.get(column)?;
        let value = bc_engine_table::regional::interpret(text, &self.regional);
        Some(CellInfo {
            value: CellData::from_owned_value(value),
            formula: None,
            number_format: None,
        })
    }
    fn display_into(&self, row: usize, column: usize, out: &mut String) {
        if let Some(info) = self.cell_info(row, column) {
            info.value.display_into(out);
        }
    }
    fn display_text(&self, row: usize, column: usize) -> String {
        let mut guard = self.scratch.lock().unwrap();
        if guard.0 != row as u32 {
            guard.1 = self.parse_record(self.header_rows as usize + row);
            guard.0 = row as u32;
        }
        guard.1.get(column).cloned().unwrap_or_default()
    }
    fn headers(&self) -> Option<&[String]> {
        self.headers.as_deref()
    }
    fn regional(&self) -> &RegionalSettings {
        &self.regional
    }
    fn revision(&self) -> u64 {
        0
    }
    fn collect_edited_rows(&self, _out: &mut std::collections::HashSet<usize>) {}
}

fn digest<L: TableData, R: TableData>(
    output: &TableAlignOutput,
    left: &L,
    right: &R,
) -> (u64, u64, u64, usize, [usize; 5]) {
    let mut h = 0xcbf29ce484222325u64;
    let mut hc = 0xcbf29ce484222325u64;
    let mut hv = 0xcbf29ce484222325u64;
    let put3 = |hh: &mut u64, b: u8| {
        *hh ^= b as u64;
        *hh = hh.wrapping_mul(0x100000001b3);
    };
    let mut stats = [0usize; 5];
    let mut rows = 0;
    let mut i = 0;
    let mut buf = String::new();
    while let Some(row) = output.data.row(i, Layers::All, false) {
        for v in [row.left, row.right] {
            for b in v.map_or(u32::MAX, |v| v as u32).to_le_bytes() {
                put3(&mut h, b);
            }
        }
        put3(&mut h, row.importance as u8);
        put3(&mut h, row.edited as u8);
        for c in 0..output.data.column_count() {
            let d = output.data.cell_diff(i, c).unwrap_or_default();
            put3(&mut hc, d.value as u8);
            put3(&mut hc, d.formula as u8);
            put3(&mut hc, d.format as u8);
        }
        for col in &output.rules.columns.0 {
            if let (Some(r), Some(c)) = (row.left, col.left) {
                buf.clear();
                left.display_into(r, c, &mut buf);
                for b in buf.as_bytes() {
                    put3(&mut hv, *b);
                }
            }
            if let (Some(r), Some(c)) = (row.right, col.right) {
                buf.clear();
                right.display_into(r, c, &mut buf);
                for b in buf.as_bytes() {
                    put3(&mut hv, *b);
                }
            }
        }
        match (row.left.is_some(), row.right.is_some()) {
            (true, true) if row.importance == bc_engine_table::compare::Importance::Same => {
                stats[0] += 1
            }
            (true, true) => {
                if row.importance == bc_engine_table::compare::Importance::Important {
                    stats[2] += 1
                } else {
                    stats[1] += 1
                }
            }
            (true, false) => stats[3] += 1,
            (false, true) => stats[4] += 1,
            _ => {}
        }
        rows += 1;
        i += 1;
    }
    (h, hc, hv, rows, stats)
}

fn chain<L: TableData + Sync>(left: &L, right: &L) -> TableAlignOutput {
    let mut rules = TableRules {
        columns: ColumnMapping::by_number(left.column_count(), right.column_count()),
        ..Default::default()
    };
    let suggestion = infer_keys_data(left, right, &rules, &NeverCancel).unwrap();
    assert!(suggestion.apply(&mut rules));
    let overrides = AlignmentOverrides::default();
    let alignment = KeyAlign
        .align_data(left, right, &rules, &overrides, &NeverCancel)
        .unwrap();
    compare_alignment_data(left, right, &rules, &overrides, alignment, &NeverCancel).unwrap()
}

/// Mirror of the prototype's fast splitter (examples/demand_proto.rs):
/// delimiter-bounded fields for a qualifier-free record body (ending stripped).
/// Any drift between this copy and the parser is caught by the differential
/// below; the example's copy is additionally guarded by the real-data digests.
fn fast_fields(body: &[u8], delim: u8) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for (i, &b) in body.iter().enumerate() {
        if b == delim {
            out.push(&body[start..i]);
            start = i + 1;
        }
    }
    out.push(&body[start..]);
    out
}

#[test]
fn fast_split_matches_records_on_edge_fields() {
    let _guard = TEST_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    // (body, parser_input): fast path sees the record body with the ending
    // stripped; the parser sees the raw line including the ending.
    let cases: &[(&[u8], &[u8])] = &[
        (b"a,b,c", b"a,b,c\r\n"),                               // plain
        (b"a,,c", b"a,,c\r\n"),                                 // empty middle
        (b",b,c", b",b,c\r\n"),                                 // leading empty
        (b"a,b,", b"a,b,\r\n"),                                 // trailing empty
        (b"", b"\r\n"),                                         // blank record -> one empty field
        (b"x,y", b"x,y"),                                       // EOF without trailing newline
        (b"true,1.25,2026-01-01", b"true,1.25,2026-01-01\r\n"), // typed-looking stay raw
        (b"a , b ,c", b"a , b ,c\r\n"),                         // whitespace preserved (no trim)
        (b"item0,code0,0.25", b"item0,code0,0.25\r\n"),
    ];
    for (n, (body, input)) in cases.iter().enumerate() {
        let mut records =
            Records::new(Cursor::new(*input), TableOptions::default(), false).unwrap();
        let parsed = records
            .next_record(&NeverCancel)
            .unwrap()
            .map(|r| r.fields)
            .unwrap_or_default();
        let fast: Vec<String> = fast_fields(body, b',')
            .into_iter()
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .collect();
        assert_eq!(
            parsed,
            fast,
            "case {n}: {:?}",
            String::from_utf8_lossy(input)
        );
    }
    // quoted/escaped records must NOT take the fast path (caller gates on the
    // qualifier byte); Records remains the semantics source there.
    let quoted = "a,\"b, c\"\r\nd,\"say \"\"hi\"\"\"\r\n";
    let mut records = Records::new(
        Cursor::new(quoted.as_bytes()),
        TableOptions::default(),
        false,
    )
    .unwrap();
    let first = records.next_record(&NeverCancel).unwrap().unwrap().fields;
    assert_eq!(first, vec!["a".to_string(), "b, c".to_string()]);
    let second = records.next_record(&NeverCancel).unwrap().unwrap().fields;
    assert_eq!(second, vec!["d".to_string(), "say \"hi\"".to_string()]);
}

#[test]
fn numeric_text_variants_compare_equal_via_rules() {
    // "0.25" vs "0.250": textually different, numerically equal — the existing
    // rules path must judge them Same (not different-by-text). Guards the
    // semantics the prototype's raw-supply compare relies on: raw-text-different
    // fields still go through typed normalization.
    let _guard = TEST_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    let left = "id,v\n1,0.25\n";
    let right = "id,v\n1,0.250\n";
    let run = |l: &str, r: &str| {
        let ls = read_text(Cursor::new(l), &ReadOptions::default(), &NeverCancel)
            .unwrap()
            .sheet;
        let rs = read_text(Cursor::new(r), &ReadOptions::default(), &NeverCancel)
            .unwrap()
            .sheet;
        let out = chain(&ls, &rs);
        let row = out.data.row(0, Layers::All, false).unwrap();
        (
            row.importance == bc_engine_table::compare::Importance::Same,
            digest(&out, &ls, &rs),
        )
    };
    let (same_ref, digest_ref) = run(left, right);
    let (same_dem, digest_dem) = {
        let l = BufferTable::build(left);
        let r = BufferTable::build(right);
        let out = chain(&l, &r);
        (
            out.data.row(0, Layers::All, false).unwrap().importance
                == bc_engine_table::compare::Importance::Same,
            digest(&out, &l, &r),
        )
    };
    assert!(
        same_ref && same_dem,
        "numeric text variants must compare Same"
    );
    assert_eq!(digest_ref, digest_dem);
}

#[test]
fn demand_access_matches_sheet_path() {
    let _guard = TEST_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    let left = "id,name,amount,note\n1,alpha,1.25,\"quoted, comma\"\n2,beta,2.5,\"say \"\"hi\"\"\"\n3,gamma,3.75,plain\n";
    let right = "id,name,amount,note\n1,alpha,1.25,\"quoted, comma\"\n2,beta,2.5,\"say \"\"hi\"\"\"\n4,delta,4.0,\"new, row\"\n";
    let reference = {
        let l = read_text(Cursor::new(left), &ReadOptions::default(), &NeverCancel)
            .unwrap()
            .sheet;
        let r = read_text(Cursor::new(right), &ReadOptions::default(), &NeverCancel)
            .unwrap()
            .sheet;
        let out = chain(&l, &r);
        digest(&out, &l, &r)
    };
    let demand = {
        let l = BufferTable::build(left);
        let r = BufferTable::build(right);
        let out = chain(&l, &r);
        digest(&out, &l, &r)
    };
    assert_eq!(reference, demand);
}

// ---------------------------------------------------------------------------
// Engine DemandSheet (GUI store path) parity: same compare digests as dense
// through the real TableAlignJob/SheetStore chain, including post-edit
// recompare and byte-equal source-preserving save.
// ---------------------------------------------------------------------------

fn demand_store(text: &str, tag: &str) -> bc_engine_table::SheetStore {
    let path = std::env::temp_dir().join(format!(
        "matcha-demand-access-{}-{tag}.csv",
        std::process::id()
    ));
    std::fs::write(&path, text).unwrap();
    let sheet =
        bc_engine_table::demand::DemandSheet::open(&path, &ReadOptions::default(), &NeverCancel)
            .unwrap();
    let _ = std::fs::remove_file(&path);
    bc_engine_table::SheetStore::Lazy(sheet)
}

#[test]
fn store_chain_matches_dense_and_save_bytes_equal() {
    let _guard = TEST_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    let left = "id,name,amount,note\n1,alpha,1.25,\"quoted, comma\"\n2,beta,2.5,\"say \"\"hi\"\"\"\n3,gamma,3.75,plain\n";
    let right = "id,name,amount,note\n1,alpha,1.25,\"quoted, comma\"\n2,beta,2.5,\"say \"\"hi\"\"\"\n4,delta,4.0,\"new, row\"\n";
    let dense = || {
        bc_engine_table::SheetStore::Dense(
            read_text(Cursor::new(left), &ReadOptions::default(), &NeverCancel)
                .unwrap()
                .sheet,
        )
    };
    let dense_r = || {
        bc_engine_table::SheetStore::Dense(
            read_text(Cursor::new(right), &ReadOptions::default(), &NeverCancel)
                .unwrap()
                .sheet,
        )
    };
    let job = |l: &bc_engine_table::SheetStore, r: &bc_engine_table::SheetStore| {
        bc_engine_table::jobs::TableAlignJob {
            left: l.clone(),
            right: r.clone(),
            rules: TableRules::default(),
            overrides: AlignmentOverrides::default(),
        }
        .execute(&NeverCancel)
        .unwrap()
    };
    let dense_out = job(&dense(), &dense_r());
    let lazy_out = job(&demand_store(left, "l"), &demand_store(right, "r"));
    // Full row stream + per-cell three-layer classification equality.
    let mut i = 0;
    while let Some(row) = dense_out.data.row(i, Layers::All, false) {
        let lazy_row = lazy_out.data.row(i, Layers::All, false).unwrap();
        assert_eq!(row.left, lazy_row.left);
        assert_eq!(row.right, lazy_row.right);
        assert_eq!(row.importance, lazy_row.importance);
        assert_eq!(row.edited, lazy_row.edited);
        for c in 0..dense_out.data.column_count() {
            let a = dense_out.data.cell_diff(i, c).unwrap();
            let b = lazy_out.data.cell_diff(i, c).unwrap();
            assert_eq!(
                (a.value, a.formula, a.format),
                (b.value, b.formula, b.format)
            );
        }
        i += 1;
    }
    assert!(lazy_out.data.row(i, Layers::All, false).is_none());
    // Display equality per aligned cell through the store façade.
    let dl = dense();
    let ll = demand_store(left, "l2");
    let mut buf_a = String::new();
    let mut buf_b = String::new();
    for r in 0..dl.row_count() {
        for c in 0..dl.column_count() {
            buf_a.clear();
            buf_b.clear();
            bc_engine_table::TableData::display_into(&dl, r, c, &mut buf_a);
            bc_engine_table::TableData::display_into(&ll, r, c, &mut buf_b);
            assert_eq!(buf_a, buf_b, "typed display differs at ({r},{c})");
            assert_eq!(dl.display_at(r, c), ll.display_at(r, c));
        }
    }
    // Post-edit recompare parity through TableMap, and save byte-equality.
    let mut dense_map = bc_engine_table::TableMap::new_stores(
        dense(),
        dense_r(),
        TableRules::default(),
        &NeverCancel,
    )
    .unwrap();
    let mut lazy_map = bc_engine_table::TableMap::new_stores(
        demand_store(left, "l3"),
        demand_store(right, "r3"),
        TableRules::default(),
        &NeverCancel,
    )
    .unwrap();
    for map in [&mut dense_map, &mut lazy_map] {
        map.set_deferred_edits(true);
        let edit = map
            .edit_sheet(
                bc_engine_table::columns::Side::Left,
                "Modify cell",
                |sheet, _, _| sheet.set_cell(1, 2, bc_engine_table::Cell::text("9.75")),
            )
            .unwrap();
        drop(edit);
        map.recompare(&NeverCancel).unwrap();
    }
    let ds = dense_map.statistics();
    let ls = lazy_map.statistics();
    assert_eq!(
        (
            ds.same,
            ds.unimportant,
            ds.important,
            ds.left_only,
            ds.right_only
        ),
        (
            ls.same,
            ls.unimportant,
            ls.important,
            ls.left_only,
            ls.right_only
        )
    );
    let mut dense_bytes = Vec::new();
    let mut lazy_bytes = Vec::new();
    bc_engine_table::store::write_store_delimited(&dense_map.left.borrow(), &mut dense_bytes)
        .unwrap();
    bc_engine_table::store::write_store_delimited(&lazy_map.left.borrow(), &mut lazy_bytes)
        .unwrap();
    assert_eq!(dense_bytes, lazy_bytes);
    assert!(String::from_utf8_lossy(&lazy_bytes).contains("2,beta,9.75"));
}
