//! On-demand table storage harness (GUI-integration phase: measures the real
//! engine storage, not a prototype-local copy).
//!
//! reference mode: existing read_path -> Sheet -> infer/align/compare (full Cell
//! materialization), the production engine path.
//! demand mode: bc_engine_table::demand::DemandSheet (private snapshot +
//! read-only mmap + per-record spans + on-demand parse + bounded LRU viewport
//! cache) driven through the same `TableData` infer/align/compare chain.
//!
//! Both modes print the same machine-checkable summary: key selection, statistics,
//! and an FNV-1a digest over the aligned row stream (left,right,kind,importance,
//! edited) so modes can be diffed offline.
//!
//! Rerun: cargo run --release -p bc-engine-table --example demand_proto -- ^
//!   --mode=demand --left <left.csv> --right <right.csv> [--hold=12]

use bc_core::{align::AlignmentOverrides, cancel::NeverCancel};
use bc_engine_table::{
    columns::ColumnMapping,
    compare::Importance,
    data::TableData,
    demand::DemandSheet,
    map::{compare_alignment_data, TableAlignOutput},
    rules::Layers,
    source::{read_path, ReadOptions},
    KeyAlign, TableRules,
};
use std::{path::PathBuf, sync::OnceLock, time::Instant};

static T0: OnceLock<Instant> = OnceLock::new();
fn mark(phase: &str) {
    let t = T0.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0;
    eprintln!("[proto] t={t:>9.1}ms {phase}");
}

struct Args {
    mode: String,
    left: PathBuf,
    right: PathBuf,
    hold: f64,
}

fn parse_args() -> Result<Args, String> {
    let mut mode = None;
    let mut left = None;
    let mut right = None;
    let mut hold = 12.0;
    for arg in std::env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--mode=") {
            mode = Some(v.to_string());
        } else if let Some(v) = arg.strip_prefix("--left=") {
            left = Some(PathBuf::from(v));
        } else if let Some(v) = arg.strip_prefix("--right=") {
            right = Some(PathBuf::from(v));
        } else if let Some(v) = arg.strip_prefix("--hold=") {
            hold = v.parse().map_err(|_| "bad --hold")?;
        } else if arg.starts_with("--cache-cap-mib=") {
            // Prototype-era knob; the engine store fixes the viewport cache at
            // 8 MiB (cache_stats reports the live cap).
        } else {
            return Err(format!("unknown arg {arg}"));
        }
    }
    Ok(Args {
        mode: mode.ok_or("missing --mode")?,
        left: left.ok_or("missing --left")?,
        right: right.ok_or("missing --right")?,
        hold,
    })
}

/// FNV-1a 64 over the aligned row stream; deterministic across runs/platforms.
struct FnV(u64);
impl FnV {
    fn put(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
    fn u32(&mut self, v: u32) {
        self.put(&v.to_le_bytes());
    }
    fn u8(&mut self, v: u8) {
        self.put(&[v]);
    }
}

fn digest_output(output: &TableAlignOutput) -> (String, usize, [usize; 5]) {
    let mut h = FnV(0xcbf29ce484222325);
    let mut stats = [0usize; 5]; // same, unimportant, important, left_only, right_only
    let mut rows = 0;
    let mut i = 0;
    while let Some(row) = output.data.row(i, Layers::All, false) {
        // DiffKind is crate-private; row() derives it from these public fields, so
        // deriving here yields the identical value: 0=Same,1=Changed,2=LeftOnly,3=RightOnly.
        let kind = match (row.left.is_some(), row.right.is_some()) {
            (true, true) if row.importance == Importance::Same => 0u8,
            (true, true) => 1,
            (true, false) => 2,
            (false, true) => 3,
            (false, false) => 4, // gap-gap should not exist; keep visible in digest
        };
        h.u32(row.left.map_or(u32::MAX, |v| v as u32));
        h.u32(row.right.map_or(u32::MAX, |v| v as u32));
        h.u8(kind);
        h.u8(row.importance as u8);
        h.u8(row.edited as u8);
        match kind {
            0 => stats[0] += 1,
            1 => {
                if row.importance == Importance::Important {
                    stats[2] += 1
                } else {
                    stats[1] += 1
                }
            }
            2 => stats[3] += 1,
            3 => stats[4] += 1,
            _ => {}
        }
        rows += 1;
        i += 1;
    }
    (format!("{:016x}", h.0), rows, stats)
}

/// Per-cell classification digest over the full aligned grid (existing public
/// accessor `ComparedData::cell_diff`): three 2-bit layers per cell.
fn cell_digest(output: &TableAlignOutput) -> String {
    let mut h = FnV(0xcbf29ce484222325);
    let rows = output.data.row_count();
    let cols = output.data.column_count();
    for r in 0..rows {
        for c in 0..cols {
            match output.data.cell_diff(r, c) {
                Some(d) => {
                    h.u8(d.value as u8);
                    h.u8(d.formula as u8);
                    h.u8(d.format as u8);
                }
                None => h.u8(0xff),
            }
        }
    }
    format!("{:016x}", h.0)
}

/// Access-layer value consistency digest: for every aligned row and every
/// mapped column pair, hash the typed display text produced through each
/// side's `TableData` (identical call path in both modes).
fn value_digest<L, R>(output: &TableAlignOutput, left: &L, right: &R, rules: &TableRules) -> String
where
    L: bc_engine_table::data::TableData,
    R: bc_engine_table::data::TableData,
{
    let mut h = FnV(0xcbf29ce484222325);
    let mut buf = String::new();
    let mut i = 0;
    while let Some(row) = output.data.row(i, Layers::All, false) {
        for col in &rules.columns.0 {
            if let Some(r) = row.left {
                if let Some(c) = col.left {
                    buf.clear();
                    left.display_into(r, c, &mut buf);
                    h.u32(buf.len() as u32);
                    h.put(buf.as_bytes());
                }
            }
            if let Some(r) = row.right {
                if let Some(c) = col.right {
                    buf.clear();
                    right.display_into(r, c, &mut buf);
                    h.u32(buf.len() as u32);
                    h.put(buf.as_bytes());
                }
            }
        }
        i += 1;
    }
    format!("{:016x}", h.0)
}

/// Column-mapping + key-selection summary digest (comparison scope identity).
fn columns_digest(rules: &TableRules) -> String {
    let mut h = FnV(0xcbf29ce484222325);
    for col in &rules.columns.0 {
        h.u32(col.left.map_or(u32::MAX, |v| v as u32));
        h.u32(col.right.map_or(u32::MAX, |v| v as u32));
    }
    for k in rules.key_columns() {
        h.u32(k as u32);
    }
    format!("{:016x}", h.0)
}

fn print_summary<L: TableData, R: TableData>(
    mode: &str,
    output: &TableAlignOutput,
    left: &L,
    right: &R,
) {
    let (rows_digest, rows, stats) = digest_output(output);
    mark("digest:rows");
    println!(
        "summary mode={mode} rows={} same={} unimportant={} important={} left_only={} right_only={} rows_digest={}",
        rows, stats[0], stats[1], stats[2], stats[3], stats[4], rows_digest
    );
    let cd = cell_digest(output);
    mark("digest:cells");
    let vd = value_digest(output, left, right, &output.rules);
    mark("digest:values");
    let cold = columns_digest(&output.rules);
    println!(
        "cells mode={mode} cell_digest={} value_digest={} columns_digest={}",
        cd, vd, cold
    );
}

fn infer_and_align<S: TableData + Sync>(
    left: &S,
    right: &S,
) -> Result<
    (
        TableRules,
        bc_engine_table::align::KeyAlignment,
        AlignmentOverrides,
    ),
    Box<dyn std::error::Error>,
> {
    let mut rules = TableRules {
        columns: ColumnMapping::by_number(left.column_count(), right.column_count()),
        ..Default::default()
    };
    let suggestion =
        bc_engine_table::inference::infer_keys_data(left, right, &rules, &NeverCancel)?;
    assert!(suggestion.apply(&mut rules));
    mark("infer_keys");
    eprintln!("[proto] key_selection={:?}", rules.keys);
    let overrides = AlignmentOverrides::default();
    let alignment = KeyAlign.align_data(left, right, &rules, &overrides, &NeverCancel)?;
    mark("align");
    Ok((rules, alignment, overrides))
}

fn run_reference(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let options = ReadOptions::default();
    let left = read_path(&args.left, &options, &NeverCancel)?.sheet;
    mark("load:left");
    let right = read_path(&args.right, &options, &NeverCancel)?.sheet;
    mark("load:right");
    let (rules, alignment, overrides) = infer_and_align(&left, &right)?;
    let output =
        compare_alignment_data(&left, &right, &rules, &overrides, alignment, &NeverCancel)?;
    mark("compare");
    print_summary("reference", &output, &left, &right);
    hold_with_live_state(args.hold, || {
        std::hint::black_box((&left, &right, &output));
    });
    Ok(())
}

// ---------------------------------------------------------------------------
// demand mode (engine storage; GUI store variant SheetStore::Lazy)
// ---------------------------------------------------------------------------

/// Deterministic viewport access driver: sequential sweeps + far jumps +
/// revisits through the store's bounded row cache (same access path the GUI
/// grid uses via preview/display).
fn viewport_driver(left: &DemandSheet, right: &DemandSheet, rows: usize) -> u64 {
    let mut accesses = 0u64;
    let mut access_log: Vec<(u8, u32)> = Vec::new();
    // sequential sweep: window of 50 rows every 977 rows
    let mut start = 0usize;
    while start < rows {
        for row in start..(start + 50).min(rows) {
            for (side, t) in [(0u8, left), (1u8, right)] {
                std::hint::black_box(t.cached_row_displays(row));
                access_log.push((side, row as u32));
                accesses += 1;
            }
        }
        start += 977;
    }
    // far jumps (xorshift64)
    let mut state = 0x9E3779B97F4A7C15u64;
    for _ in 0..4096 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let row = (state % rows as u64) as u32;
        for (_side, t) in [(0u8, left), (1u8, right)] {
            std::hint::black_box(t.cached_row_displays(row as usize));
            accesses += 1;
        }
    }
    // revisit the first accesses (exercise hits after eviction pressure)
    for &(side, row) in access_log.iter().take(2048) {
        let t = if side == 0 { left } else { right };
        std::hint::black_box(t.cached_row_displays(row as usize));
        accesses += 1;
    }
    accesses
}

fn run_demand(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let options = ReadOptions::default();
    let left = DemandSheet::open(&args.left, &options, &NeverCancel)
        .map_err(|e| format!("left: {e:?}"))?;
    mark("index:left");
    let right = DemandSheet::open(&args.right, &options, &NeverCancel)
        .map_err(|e| format!("right: {e:?}"))?;
    mark("index:right");
    eprintln!(
        "[proto] mapped_bytes={} spans_left={} spans_right={}",
        left.mapped_virtual_bytes() + right.mapped_virtual_bytes(),
        left.row_count(),
        right.row_count()
    );
    // Hashing/alignment phases need typed values; the compare phase supplies
    // raw field text (engine store flag, same as the worker snapshot path).
    let (rules, alignment, overrides) = infer_and_align(&left, &right)?;
    left.set_compare_raw_supply(true);
    right.set_compare_raw_supply(true);
    let output =
        compare_alignment_data(&left, &right, &rules, &overrides, alignment, &NeverCancel)?;
    left.set_compare_raw_supply(false);
    right.set_compare_raw_supply(false);
    mark("compare");
    print_summary("demand", &output, &left, &right);
    let accesses = viewport_driver(&left, &right, left.row_count());
    mark("viewport_driver");
    let lc = left.cache_stats();
    let rc = right.cache_stats();
    println!(
        "viewport accesses={} hits={} misses={} evictions={} max_cache_bytes={} live_cache_bytes={} cap_bytes={}",
        accesses,
        lc.hits + rc.hits,
        lc.misses + rc.misses,
        lc.evictions + rc.evictions,
        lc.max_bytes + rc.max_bytes,
        lc.live_bytes + rc.live_bytes,
        lc.cap_bytes + rc.cap_bytes
    );
    // The filled bounded cache stays alive through the stable window so the
    // sampled stable memory includes it; it is dropped only after the hold.
    hold_with_live_state(args.hold, || {
        std::hint::black_box((&left, &right, &output));
    });
    Ok(())
}

/// Stable-window hold: sleep with the compared state still alive (values drop
/// only after this returns).
fn hold_with_live_state(hold: f64, keep: impl FnOnce()) {
    if hold > 0.0 {
        mark("hold:begin");
        std::thread::sleep(std::time::Duration::from_secs_f64(hold));
        keep();
        mark("hold:end");
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    T0.set(Instant::now()).ok();
    mark("main:enter");
    let args = parse_args()?;
    eprintln!(
        "[proto] mode={} left={} right={} hold={}",
        args.mode,
        args.left.display(),
        args.right.display(),
        args.hold
    );
    match args.mode.as_str() {
        "reference" => run_reference(&args)?,
        "demand" => run_demand(&args)?,
        other => return Err(format!("unknown mode {other}").into()),
    }
    Ok(())
}
