use crate::{
    columns::{finish_row_hash, row_hash_into, Side},
    data::TableData,
    invalid,
    rules::{ColumnHandling, KeySelection},
    Result, Sheet, TableRules,
};
use bc_core::{
    cancel::CancelCheck,
    hash::{ContentHash, U64KeyBuild},
};
use std::collections::{HashMap, VecDeque};

type KeyMap = HashMap<ContentHash, usize, U64KeyBuild>;

#[derive(Debug, Clone, PartialEq)]
pub struct KeyCandidate {
    pub columns: Vec<usize>,
    pub unique_left: f64,
    pub unique_right: f64,
    pub overlap: f64,
    pub stability: f64,
    pub position: f64,
    pub score: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct KeySuggestion {
    pub selection: KeySelection,
    pub candidates: Vec<KeyCandidate>,
}
fn values_into<S: TableData>(
    sheet: &S,
    columns: &[usize],
    rules: &TableRules,
    side: Side,
    set: &mut KeyMap,
    cancel: &dyn CancelCheck,
) -> Result<()> {
    let mapped: Vec<_> = columns
        .iter()
        .map(|&c| rules.columns.0[c].index(side).unwrap())
        .collect();
    let handling: Vec<_> = columns.iter().map(|&c| rules.handling(c)).collect();
    set.clear();
    let mut bytes = Vec::new();
    for row in 0..sheet.row_count() {
        if row.is_multiple_of(4096) {
            cancel.check()?;
        }
        row_hash_into(sheet, row, &mapped, &handling, &mut bytes);
        set.entry(finish_row_hash(&bytes)).or_insert(row);
    }
    Ok(())
}
struct ScoreScratch {
    l: KeyMap,
    r: KeyMap,
}
impl ScoreScratch {
    fn new<L: TableData, R: TableData>(left: &L, right: &R) -> Self {
        Self {
            l: KeyMap::with_capacity_and_hasher(left.row_count(), Default::default()),
            r: KeyMap::with_capacity_and_hasher(right.row_count(), Default::default()),
        }
    }
}
/// Score a wave of candidates. Large sheets score candidates concurrently in
/// order-preserving chunks (each thread gets its own preallocated scratch
/// maps); results concatenate in candidate order so the `best()` tie-break
/// observes exactly the sequential candidate sequence.
fn score_wave<S: TableData + Sync>(
    left: &S,
    right: &S,
    rules: &TableRules,
    columns_list: &[Vec<usize>],
    cancel: &dyn CancelCheck,
) -> Result<Vec<KeyCandidate>> {
    let big = left.row_count().max(right.row_count()) >= 4096;
    if columns_list.len() < 2 || !big {
        let mut scratch = ScoreScratch::new(left, right);
        let mut out = Vec::with_capacity(columns_list.len());
        for c in columns_list {
            out.push(score(left, right, rules, c.clone(), &mut scratch, cancel)?);
        }
        return Ok(out);
    }
    let threads = columns_list.len().min(4);
    let chunk = columns_list.len().div_ceil(threads);
    let mut results = Vec::new();
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for chunk in columns_list.chunks(chunk) {
            handles.push(scope.spawn(move || {
                let mut scratch = ScoreScratch::new(left, right);
                let mut out = Vec::with_capacity(chunk.len());
                for c in chunk {
                    out.push(score(left, right, rules, c.clone(), &mut scratch, cancel)?);
                }
                Ok(out)
            }));
        }
        for h in handles {
            results.push(h.join().unwrap_or_else(|_| Err(invalid("key inference panicked"))));
        }
    });
    let mut out = Vec::with_capacity(columns_list.len());
    for r in results {
        out.extend(r?);
    }
    Ok(out)
}
fn score<S: TableData>(
    left: &S,
    right: &S,
    rules: &TableRules,
    columns: Vec<usize>,
    scratch: &mut ScoreScratch,
    cancel: &dyn CancelCheck,
) -> Result<KeyCandidate> {
    values_into(left, &columns, rules, Side::Left, &mut scratch.l, cancel)?;
    values_into(right, &columns, rules, Side::Right, &mut scratch.r, cancel)?;
    score_maps(
        left.row_count(),
        right.row_count(),
        rules.columns.0.len(),
        &scratch.l,
        &scratch.r,
        columns,
        cancel,
    )
}
/// Scoring over the built key maps; the exact body shared by `score` (maps
/// from per-pass `values_into`) and `score_from_hashes` (maps rebuilt in row
/// order from the fused-pass arrays). The float accumulation order is the
/// left map's iteration order, which both builders reproduce identically
/// (deterministic `U64KeyBuild`, same capacity policy, same insertion
/// sequence).
#[allow(clippy::too_many_arguments)]
fn score_maps(
    left_rows: usize,
    right_rows: usize,
    total_columns: usize,
    l: &KeyMap,
    r: &KeyMap,
    columns: Vec<usize>,
    cancel: &dyn CancelCheck,
) -> Result<KeyCandidate> {
    let ul = l.len() as f64 / left_rows.max(1) as f64;
    let ur = r.len() as f64 / right_rows.max(1) as f64;
    let mut matches = 0;
    let mut displacement = 0.0;
    for (i, (hash, &lr)) in l.iter().enumerate() {
        if i.is_multiple_of(4096) {
            cancel.check()?;
        }
        if let Some(&rr) = r.get(hash) {
            matches += 1;
            displacement += (lr as f64 / left_rows.max(1) as f64
                - rr as f64 / right_rows.max(1) as f64)
                .abs();
        }
    }
    let overlap = matches as f64 / (l.len() + r.len() - matches).max(1) as f64;
    let stability = if matches == 0 {
        0.0
    } else {
        1.0 - displacement / matches as f64
    };
    let position = 1.0 - columns[0] as f64 / total_columns.max(1) as f64;
    let score = 0.2 * ul + 0.2 * ur + 0.35 * overlap + 0.2 * stability + 0.05 * position;
    Ok(KeyCandidate {
        columns,
        unique_left: ul,
        unique_right: ur,
        overlap,
        stability,
        position,
        score,
    })
}
/// One contiguous row range of the fused hash pass: for every candidate the
/// row hash is computed by the same `row_hash_into`/`finish_row_hash` pair
/// with the same single-column input `values_into` would use, then stored at
/// the row's index in the candidate's disjoint output slice.
fn fused_chunk<S: TableData>(
    sheet: &S,
    mapped: &[usize],
    handling: &[ColumnHandling],
    start_row: usize,
    bufs: &mut [&mut [u64]],
    cancel: &dyn CancelCheck,
) -> Result<()> {
    let rows = bufs.first().map_or(0, |b| b.len());
    let mut bytes = Vec::new();
    for local in 0..rows {
        if local.is_multiple_of(4096) {
            cancel.check()?;
        }
        let row = start_row + local;
        for (k, buf) in bufs.iter_mut().enumerate() {
            row_hash_into(sheet, row, &mapped[k..=k], &handling[k..=k], &mut bytes);
            buf[local] = finish_row_hash(&bytes).0;
        }
    }
    Ok(())
}
/// Fused single-pass per-(candidate, row) hashes for one side: rows chunked
/// across scoped workers (phase_threads budget) writing disjoint row ranges
/// of per-candidate arrays; one side at a time so staging never stacks.
fn fused_column_hashes<S: TableData + Sync>(
    sheet: &S,
    side: Side,
    rules: &TableRules,
    columns_list: &[Vec<usize>],
    cancel: &dyn CancelCheck,
) -> Result<Vec<Vec<u64>>> {
    let rows = sheet.row_count();
    let mapped: Vec<usize> = columns_list
        .iter()
        .map(|c| rules.columns.0[c[0]].index(side).unwrap())
        .collect();
    let handling: Vec<_> = columns_list.iter().map(|c| rules.handling(c[0])).collect();
    let mut out: Vec<Vec<u64>> = columns_list.iter().map(|_| vec![0; rows]).collect();
    let threads = crate::perf::phase_threads(rows);
    if threads < 2 {
        let mut bufs: Vec<&mut [u64]> = out.iter_mut().map(Vec::as_mut_slice).collect();
        fused_chunk(sheet, &mapped, &handling, 0, &mut bufs, cancel)?;
        return Ok(out);
    }
    let chunk = rows.div_ceil(threads).max(1);
    let mut lanes: Vec<VecDeque<&mut [u64]>> = out
        .iter_mut()
        .map(|a| a.chunks_mut(chunk).collect())
        .collect();
    let chunk_count = lanes.first().map_or(0, VecDeque::len);
    let mut jobs: Vec<(usize, Vec<&mut [u64]>)> = Vec::with_capacity(chunk_count);
    for i in 0..chunk_count {
        let bufs = lanes
            .iter_mut()
            .map(|q| q.pop_front().expect("lanes split evenly"))
            .collect();
        jobs.push((i * chunk, bufs));
    }
    let mapped = &mapped;
    let handling = &handling;
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for (start, mut bufs) in jobs {
            handles.push(scope.spawn(move || {
                fused_chunk(sheet, mapped, handling, start, &mut bufs, cancel)
            }));
        }
        for h in handles {
            h.join().unwrap_or_else(|_| Err(invalid("key inference panicked")))?;
        }
        Ok::<(), bc_core::CoreError>(())
    })?;
    Ok(out)
}
/// Rebuild one candidate's key maps from the fused arrays: ascending row
/// order, first-occurrence `or_insert`, same capacity policy (`ScoreScratch`),
/// same deterministic hasher — the resulting maps are layout-identical to
/// `values_into`'s, so `score_maps` observes the same iteration order.
fn score_from_hashes(
    left_rows: usize,
    right_rows: usize,
    total_columns: usize,
    lh: &[u64],
    rh: &[u64],
    columns: Vec<usize>,
    scratch: &mut ScoreScratch,
    cancel: &dyn CancelCheck,
) -> Result<KeyCandidate> {
    debug_assert_eq!(lh.len(), left_rows);
    debug_assert_eq!(rh.len(), right_rows);
    scratch.l.clear();
    for (row, &h) in lh.iter().enumerate() {
        if row.is_multiple_of(4096) {
            cancel.check()?;
        }
        scratch.l.entry(ContentHash(h)).or_insert(row);
    }
    scratch.r.clear();
    for (row, &h) in rh.iter().enumerate() {
        if row.is_multiple_of(4096) {
            cancel.check()?;
        }
        scratch.r.entry(ContentHash(h)).or_insert(row);
    }
    score_maps(
        left_rows,
        right_rows,
        total_columns,
        &scratch.l,
        &scratch.r,
        columns,
        cancel,
    )
}
/// Score a wave of single-column candidates from one fused hash pass per
/// side instead of one full pass per candidate. Small inputs take the same
/// sequential path `score_wave` uses; big inputs hash each side once
/// (chunked over scoped workers), then score candidates one at a time — the
/// staging arrays (2 sides × candidates × rows × 8B) plus one reused pair of
/// key maps stay below the old wave's 4-thread × 2-map scratch, and each
/// candidate's array pair is dropped right after scoring. Candidates come
/// back in list order, so `best()` tie-breaks see the identical sequence.
fn score_wave_fused<S: TableData + Sync>(
    left: &S,
    right: &S,
    rules: &TableRules,
    columns_list: &[Vec<usize>],
    cancel: &dyn CancelCheck,
) -> Result<Vec<KeyCandidate>> {
    let big = left.row_count().max(right.row_count()) >= 4096;
    if columns_list.len() < 2 || !big {
        let mut scratch = ScoreScratch::new(left, right);
        let mut out = Vec::with_capacity(columns_list.len());
        for c in columns_list {
            out.push(score(left, right, rules, c.clone(), &mut scratch, cancel)?);
        }
        return Ok(out);
    }
    let lh = fused_column_hashes(left, Side::Left, rules, columns_list, cancel)?;
    let rh = fused_column_hashes(right, Side::Right, rules, columns_list, cancel)?;
    let mut scratch = ScoreScratch::new(left, right);
    let mut out = Vec::with_capacity(columns_list.len());
    let total_columns = rules.columns.0.len();
    for (c, (l, r)) in columns_list
        .iter()
        .zip(lh.into_iter().zip(rh.into_iter()))
    {
        out.push(score_from_hashes(
            left.row_count(),
            right.row_count(),
            total_columns,
            &l,
            &r,
            c.clone(),
            &mut scratch,
            cancel,
        )?);
    }
    Ok(out)
}
/// Full-column uniqueness, Jaccard, relative position stability, left-position
/// prior; adjacent pairs only when no unique single column passes 0.8.
/// Infer keys on materialized sheets (original public entry; behavior unchanged).
pub fn infer_keys(
    left: &Sheet,
    right: &Sheet,
    rules: &TableRules,
    cancel: &dyn CancelCheck,
) -> Result<KeySuggestion> {
    infer_keys_data(left, right, rules, cancel)
}
/// Generic read-only entry over `TableData` (prototype/demand storage support).
pub fn infer_keys_data<S: TableData + Sync>(
    left: &S,
    right: &S,
    rules: &TableRules,
    cancel: &dyn CancelCheck,
) -> Result<KeySuggestion> {
    rules.validate(left.column_count(), right.column_count())?;
    let usable: Vec<_> = rules
        .columns
        .0
        .iter()
        .enumerate()
        .filter(|(_, c)| c.left.is_some() && c.right.is_some())
        .map(|(i, _)| i)
        .collect();
    let singles: Vec<Vec<usize>> = usable.iter().map(|&c| vec![c]).collect();
    // Singles take the fused one-pass-per-side scan; the pairs wave (rare,
    // only when no single column passes 0.8) keeps the per-pass `score_wave`.
    let mut candidates = score_wave_fused(left, right, rules, &singles, cancel)?;
    fn best(candidates: &[KeyCandidate]) -> Option<&KeyCandidate> {
        candidates
            .iter()
            .filter(|c| c.unique_left == 1.0 && c.unique_right == 1.0 && c.score >= 0.8)
            .max_by(|a, b| {
                a.score
                    .total_cmp(&b.score)
                    .then_with(|| b.columns.cmp(&a.columns))
            })
    }
    if best(&candidates).is_none() {
        let pairs: Vec<Vec<usize>> = usable
            .windows(2)
            .filter(|pair| pair[1] == pair[0] + 1)
            .map(|pair| pair.to_vec())
            .collect();
        candidates.extend(score_wave(left, right, rules, &pairs, cancel)?);
    }
    let selection =
        best(&candidates).map_or(KeySelection::AllColumns, |c| KeySelection::Automatic {
            columns: c.columns.clone(),
            score: c.score,
        });
    Ok(KeySuggestion {
        selection,
        candidates,
    })
}
impl KeySuggestion {
    /// A user edit locks the selection; background suggestions never overwrite it.
    pub fn apply(&self, rules: &mut TableRules) -> bool {
        if matches!(
            rules.keys,
            KeySelection::Manual(_) | KeySelection::ByColumnNumber | KeySelection::AllColumns
        ) {
            return false;
        }
        rules.keys = self.selection.clone();
        true
    }
}
