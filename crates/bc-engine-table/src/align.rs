//! Key preparation only; all sequence alignment is delegated to bc-core.
use crate::{
    columns::Side,
    compare::{equal_values, normalized_value, raw_text_equal},
    data::{CellData, TableData},
    invalid,
    rules::ColumnHandling,
    Result, Sheet, TableRules,
};
use bc_core::{
    align::{
        AlignInput, AlignMap, AlignRow, AlignStrategy, AlignmentOverrides, QuickAlign, SmartAlign,
    },
    cancel::CancelCheck,
    diff::{DiffOp, PatienceDiff},
    hash::{ContentHash, LineHasher, U64KeyBuild},
    line::LineList,
    DiffKind,
};
use std::{
    cmp::Ordering,
    collections::{BTreeSet, HashMap},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DuplicateKeys {
    pub left: usize,
    pub right: usize,
}
#[derive(Debug)]
pub struct KeyAlignment {
    /// Core indices address these permutations, NOT original source indices.
    pub map: AlignMap,
    pub left_order: Vec<usize>,
    pub right_order: Vec<usize>,
    pub duplicates: DuplicateKeys,
}
#[derive(Debug, Default)]
pub struct KeyAlign;
/// One key column: rules column index, its handling, and whether formula
/// cells in it hash by formula text (some cell on either side has no cached
/// result, so values cannot be trusted to line up).
pub(crate) type KeyColumn = (usize, ColumnHandling, bool);
/// Formula text of a by-formula key cell (`None`: hash/compare the value).
fn key_formula<S: TableData>(sheet: &S, row: usize, column: Option<usize>) -> Option<String> {
    let info = sheet.cell_info(row, column?)?;
    let formula = crate::compare::formula_text(Some(&info));
    (!formula.is_empty()).then(|| formula.to_owned())
}
fn atom_bytes(value: &CellData, out: &mut Vec<u8>) {
    match value {
        CellData::Empty => out.push(0),
        CellData::Text(t) => {
            out.push(1);
            out.extend(t.as_bytes());
        }
        CellData::Number(n) => {
            out.push(2);
            out.extend(if *n == 0.0 { 0.0f64 } else { *n }.to_bits().to_le_bytes());
        }
        CellData::Bool(b) => {
            out.push(3);
            out.push(u8::from(*b));
        }
        CellData::Date(d) => {
            out.push(4);
            out.extend(d.to_le_bytes());
        }
        CellData::Error(t) => {
            out.push(5);
            out.extend(t.as_bytes());
        }
    }
}
fn hash_row<S: TableData>(
    sheet: &S,
    row: usize,
    side: Side,
    rules: &TableRules,
    keys: &[KeyColumn],
    exact_only: bool,
    bytes: &mut Vec<u8>,
) -> ContentHash {
    bytes.clear();
    for (column, handling, by_formula) in keys {
        if exact_only && (handling.numeric_tolerance > 0.0 || handling.date_tolerance.0 > 0) {
            continue;
        }
        let column = rules.columns.0[*column].index(side);
        let start = bytes.len();
        bytes.extend([0; 8]);
        match by_formula
            .then(|| key_formula(sheet, row, column))
            .flatten()
        {
            Some(formula) => {
                bytes.push(6);
                bytes.extend(formula.as_bytes());
            }
            None => atom_bytes(&normalized_value(sheet, row, column, handling), bytes),
        }
        let len = (bytes.len() - start - 8) as u64;
        bytes[start..start + 8].copy_from_slice(&len.to_le_bytes());
    }
    LineHasher::default().hash_line(bytes)
}
fn keys_equal<S: TableData>(
    left: &S,
    right: &S,
    l: usize,
    r: usize,
    rules: &TableRules,
    keys: &[KeyColumn],
) -> bool {
    let same_regional = left.regional() == right.regional();
    keys.iter().all(|(c, h, by_formula)| {
        if *by_formula {
            let lf = key_formula(left, l, rules.columns.0[*c].left);
            let rf = key_formula(right, r, rules.columns.0[*c].right);
            if lf.is_some() || rf.is_some() {
                return lf == rf;
            }
        }
        let lc = rules.columns.0[*c].left.and_then(|col| left.cell_info(l, col));
        let rc = rules.columns.0[*c].right.and_then(|col| right.cell_info(r, col));
        if same_regional
            && h.column_type != crate::rules::ColumnType::Text
            && raw_text_equal(lc.as_ref(), rc.as_ref())
        {
            return true;
        }
        equal_values(
            &normalized_value(left, l, rules.columns.0[*c].left, h),
            &normalized_value(right, r, rules.columns.0[*c].right, h),
            h,
        )
    })
}
pub(crate) fn value_cmp(l: &CellData, r: &CellData) -> Ordering {
    fn rank(v: &CellData) -> u8 {
        match v {
            CellData::Empty => 0,
            CellData::Number(_) => 1,
            CellData::Date(_) => 2,
            CellData::Bool(_) => 3,
            CellData::Text(_) => 4,
            CellData::Error(_) => 5,
        }
    }
    match (l, r) {
        (CellData::Number(l), CellData::Number(r)) => l.total_cmp(r),
        (CellData::Date(l), CellData::Date(r)) => l.cmp(r),
        (CellData::Bool(l), CellData::Bool(r)) => l.cmp(r),
        (CellData::Text(l), CellData::Text(r)) | (CellData::Error(l), CellData::Error(r)) => {
            l.as_ref().cmp(r.as_ref())
        }
        _ => rank(l).cmp(&rank(r)),
    }
}
fn order<S: TableData>(
    sheet: &S,
    side: Side,
    rules: &TableRules,
    keys: &[KeyColumn],
    cancel: &dyn CancelCheck,
) -> Result<Vec<usize>> {
    let mut order: Vec<_> = (0..sheet.row_count()).collect();
    if rules.sort_by_key {
        order.sort_by(|&a, &b| {
            if cancel.is_cancelled() {
                return Ordering::Equal;
            }
            for (c, h, _) in keys {
                let col = rules.columns.0[*c].index(side);
                let cmp = value_cmp(
                    &normalized_value(sheet, a, col, h),
                    &normalized_value(sheet, b, col, h),
                );
                if cmp != Ordering::Equal {
                    return cmp;
                }
            }
            Ordering::Equal
        });
    }
    cancel.check()?;
    Ok(order)
}
/// Key hashes of one contiguous visit-order range. `hash_row` is
/// deterministic, so the concatenation of chunk outputs in visit order is
/// exactly the single-pass hash sequence.
fn hashes_chunk<S: TableData>(
    sheet: &S,
    rows: &[usize],
    side: Side,
    rules: &TableRules,
    keys: &[KeyColumn],
    cancel: &dyn CancelCheck,
) -> Result<Vec<u64>> {
    let mut out = Vec::with_capacity(rows.len());
    let mut scratch = Vec::new();
    for (i, &row) in rows.iter().enumerate() {
        if i.is_multiple_of(1024) {
            cancel.check()?;
        }
        out.push(hash_row(sheet, row, side, rules, keys, false, &mut scratch).0);
    }
    Ok(out)
}
pub(crate) fn hashes<S: TableData + Sync>(
    sheet: &S,
    order: &[usize],
    side: Side,
    rules: &TableRules,
    keys: &[KeyColumn],
    cancel: &dyn CancelCheck,
) -> Result<(Vec<ContentHash>, usize)> {
    // Pass 1: per-row key hashes, chunked across scoped workers.
    let threads = crate::perf::phase_threads(order.len());
    let hashes: Vec<u64> = if threads < 2 {
        hashes_chunk(sheet, order, side, rules, keys, cancel)?
    } else {
        let chunk = order.len().div_ceil(threads).max(1);
        let results = std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for rows in order.chunks(chunk) {
                handles.push(scope.spawn(|| hashes_chunk(sheet, rows, side, rules, keys, cancel)));
            }
            handles
                .into_iter()
                .map(|h| h.join().unwrap_or_else(|_| Err(invalid("hashing panicked"))))
                .collect::<Vec<_>>()
        });
        let mut out = Vec::with_capacity(order.len());
        for result in results {
            out.extend(result?);
        }
        out
    };
    // Pass 2: occurrence numbering and outer hashing, the original sequential
    // loop verbatim — byte-identical output for the same hash sequence.
    let mut counts: HashMap<ContentHash, u64, U64KeyBuild> = HashMap::default();
    let mut duplicates = 0;
    let mut result = Vec::with_capacity(hashes.len());
    for (i, hash) in hashes.into_iter().enumerate() {
        if i.is_multiple_of(1024) {
            cancel.check()?;
        }
        let hash = ContentHash(hash);
        let count = counts.entry(hash).or_insert(0u64);
        if *count > 0 {
            duplicates += 1;
        }
        let mut bytes = [0; 16];
        bytes[..8].copy_from_slice(&hash.0.to_le_bytes());
        bytes[8..].copy_from_slice(&count.to_le_bytes());
        result.push(LineHasher::default().hash_line(&bytes));
        *count += 1;
    }
    Ok((result, duplicates))
}
fn bucket<S: TableData>(
    sheet: &S,
    row: usize,
    side: Side,
    rules: &TableRules,
    first: &KeyColumn,
) -> i64 {
    let (c, h, _) = first;
    let value = normalized_value(sheet, row, rules.columns.0[*c].index(side), h);
    match &value {
        CellData::Number(n) if h.numeric_tolerance > 0.0 => {
            (*n / h.numeric_tolerance).floor() as i64
        }
        CellData::Date(ms) if h.date_tolerance.0 > 0 => ms.div_euclid(h.date_tolerance.0),
        _ => i64::MIN,
    }
}
struct KeyContext<'a, S: TableData> {
    left: &'a S,
    right: &'a S,
    rules: &'a TableRules,
    keys: &'a [KeyColumn],
}
/// Positive tolerances are not transitive, so quantizing alone is incorrect.
/// Search neighboring buckets and verify every candidate against the actual
/// tolerance; assign shared pair IDs in stable occurrence order.
fn tolerant_tokens<S: TableData>(
    ctx: &KeyContext<'_, S>,
    orders: (&[usize], &[usize]),
    cancel: &dyn CancelCheck,
) -> Result<(Vec<ContentHash>, Vec<ContentHash>)> {
    let KeyContext {
        left,
        right,
        rules,
        keys,
    } = *ctx;
    let (lo, ro) = orders;
    let first = keys
        .iter()
        .find(|(_, h, _)| h.numeric_tolerance > 0.0 || h.date_tolerance.0 > 0)
        .unwrap();
    let mut index: HashMap<(ContentHash, i64), BTreeSet<usize>> = HashMap::new();
    let mut scratch = Vec::new();
    for (i, &row) in ro.iter().enumerate() {
        if i.is_multiple_of(1024) {
            cancel.check()?;
        }
        index
            .entry((
                hash_row(right, row, Side::Right, rules, keys, true, &mut scratch),
                bucket(right, row, Side::Right, rules, first),
            ))
            .or_default()
            .insert(i);
    }
    let mut lt = Vec::with_capacity(lo.len());
    let rt = (0..ro.len()).map(|i| ContentHash(i as u64 + 1)).collect();
    for (i, &row) in lo.iter().enumerate() {
        if i.is_multiple_of(1024) {
            cancel.check()?;
        }
        let hash = hash_row(left, row, Side::Left, rules, keys, true, &mut scratch);
        let b = bucket(left, row, Side::Left, rules, first);
        let mut best = None;
        for candidate_bucket in [b.saturating_sub(1), b, b.saturating_add(1)] {
            if let Some(candidates) = index.get(&(hash, candidate_bucket)) {
                for (n, &r) in candidates.iter().enumerate() {
                    if n.is_multiple_of(1024) {
                        cancel.check()?;
                    }
                    if best.is_some_and(|(prev, _)| r >= prev) {
                        break;
                    }
                    if keys_equal(left, right, row, ro[r], rules, keys) {
                        best = Some((r, candidate_bucket));
                        break;
                    }
                }
            }
        }
        if let Some((r, b)) = best {
            index.get_mut(&(hash, b)).unwrap().remove(&r);
            lt.push(ContentHash(r as u64 + 1));
        } else {
            lt.push(ContentHash(ro.len() as u64 + i as u64 + 1));
        }
    }
    Ok((lt, rt))
}
/// Without overrides, identical ordered hashes are one common prefix in the
/// Myers skeleton. There is no orphan block for Smart's closeness resolver
/// to score. This says nothing about the non-key cells (or hash collisions):
/// key verification and the full cell comparison still run.
fn may_have_orphan_blocks(
    left: &[ContentHash],
    right: &[ContentHash],
    overrides: &AlignmentOverrides,
    cancel: &dyn CancelCheck,
) -> Result<bool> {
    if !overrides.is_empty() || left.len() != right.len() {
        return Ok(true);
    }
    for (l, r) in left.chunks(1024).zip(right.chunks(1024)) {
        cancel.check()?;
        if l != r {
            return Ok(true);
        }
    }
    Ok(false)
}

/// One row's tab-joined display bytes: per-cell typed display, 256-character
/// cap, CR/LF flattened to spaces, '\n' terminator. Shared by the sequential
/// and chunked row-text paths so both emit byte-identical lines.
fn row_text_row<S: TableData>(sheet: &S, row: usize, cell_buf: &mut String, text: &mut Vec<u8>) {
    let mut count = 0;
    for column in 0..sheet.column_count() {
        if column > 0 {
            text.push(b'\t');
        }
        cell_buf.clear();
        sheet.display_into(row, column, cell_buf);
        if cell_buf.is_ascii() {
            // ASCII fast path: chars are bytes, the 256-char cap counts bytes,
            // and CR/LF flattening is byte-wise — identical output to the
            // char loop without decode/encode round-trips.
            for &b in cell_buf.as_bytes() {
                if count >= 256 {
                    break;
                }
                text.push(if matches!(b, b'\r' | b'\n') { b' ' } else { b });
                count += 1;
            }
        } else {
            for c in cell_buf.chars() {
                if count >= 256 {
                    break;
                }
                let c = if matches!(c, '\r' | '\n') { ' ' } else { c };
                text.extend(c.encode_utf8(&mut [0; 4]).as_bytes());
                count += 1;
            }
        }
        if count >= 256 {
            break;
        }
    }
    text.push(b'\n');
}
/// Row text for a contiguous range of the visit order (cancel cadence no
/// coarser than the historical 1024-row step).
fn row_text_chunk<S: TableData>(
    sheet: &S,
    rows: &[usize],
    cancel: &dyn CancelCheck,
) -> Result<Vec<u8>> {
    let mut text = Vec::with_capacity(rows.len() * 32);
    let mut cell_buf = String::new();
    for (i, &row) in rows.iter().enumerate() {
        if i.is_multiple_of(1024) {
            cancel.check()?;
        }
        row_text_row(sheet, row, &mut cell_buf, &mut text);
    }
    Ok(text)
}
/// Line text of one side: row-range chunks on scoped workers, concatenated in
/// chunk order — byte-identical to the historical single-buffer build.
fn row_text<S: TableData + Sync>(sheet: &S, order: &[usize], cancel: &dyn CancelCheck) -> Result<LineList> {
    let threads = crate::perf::phase_threads(order.len());
    if threads < 2 {
        return Ok(LineList::from_bytes(row_text_chunk(sheet, order, cancel)?));
    }
    let chunk = order.len().div_ceil(threads).max(1);
    let results = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for rows in order.chunks(chunk) {
            handles.push(scope.spawn(|| row_text_chunk(sheet, rows, cancel)));
        }
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|_| Err(invalid("row text panicked"))))
            .collect::<Vec<_>>()
    });
    let mut total = 0;
    for result in &results {
        total += result.as_ref().map_or(0, Vec::len);
    }
    let mut text = Vec::with_capacity(total);
    for result in results {
        // Each chunk buffer is dropped right after its copy, keeping the peak
        // at final-text plus the not-yet-assembled chunks.
        text.extend_from_slice(&result?);
    }
    Ok(LineList::from_bytes(text))
}
/// Verify-filter one contiguous range of aligned rows: in keyed mode (key
/// columns are a proper subset), unforced pairs whose hashes or keys disagree
/// split into a left/right-only pair; everything else passes through. In
/// content mode (no explicit keys — every column feeds the row hash) the split
/// would fire on ANY cell change, so closeness-paired rows are kept and their
/// cell differences surface as changed rows instead.
/// Concatenating chunk outputs in row order reproduces the
/// historical sequential filter exactly; `keys_equal`/`anchors` are read-only
/// shares, so the result never depends on scheduling.
#[allow(clippy::too_many_arguments)]
fn verify_filter_chunk<S: TableData>(
    rows: &[AlignRow],
    left: &S,
    right: &S,
    orders: (&[usize], &[usize]),
    rules: &TableRules,
    keys: &[KeyColumn],
    keyed: bool,
    hashes: (&[ContentHash], &[ContentHash]),
    anchors: &AlignmentOverrides,
    cancel: &dyn CancelCheck,
) -> Result<Vec<AlignRow>> {
    let (lo, ro) = orders;
    let (lh, rh) = hashes;
    let mut out = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().copied().enumerate() {
        if i.is_multiple_of(1024) {
            cancel.check()?;
        }
        if let (Some(l), Some(r)) = (row.left, row.right) {
            let forced = rules.strategy == AlignStrategy::None
                || anchors.pairs().binary_search(&(l, r)).is_ok();
            if keyed
                && !forced
                && (lh[l] != rh[r] || !keys_equal(left, right, lo[l], ro[r], rules, keys))
            {
                out.push(AlignRow {
                    left: Some(l),
                    kind: DiffKind::LeftOnly,
                    ..Default::default()
                });
                out.push(AlignRow {
                    right: Some(r),
                    kind: DiffKind::RightOnly,
                    ..Default::default()
                });
                continue;
            }
        }
        out.push(row);
    }
    Ok(out)
}
/// Upper bound on `left rows × right rows × key columns` cell comparisons for
/// one difference run; larger runs keep the line-text closeness result.
const CELL_CLOSENESS_BUDGET: usize = 50_000_000;
/// Per key column atom hash of one row; 0 marks an empty cell.
fn cell_atoms<S: TableData>(
    sheet: &S,
    row: usize,
    side: Side,
    rules: &TableRules,
    keys: &[KeyColumn],
    bytes: &mut Vec<u8>,
) -> Vec<u64> {
    keys.iter()
        .map(|(column, handling, by_formula)| {
            let column = rules.columns.0[*column].index(side);
            bytes.clear();
            match by_formula
                .then(|| key_formula(sheet, row, column))
                .flatten()
            {
                Some(formula) => {
                    bytes.push(6);
                    bytes.extend(formula.as_bytes());
                }
                None => match normalized_value(sheet, row, column, handling) {
                    CellData::Empty => return 0,
                    value => atom_bytes(&value, bytes),
                },
            }
            LineHasher::default().hash_line(bytes).0.max(1)
        })
        .collect()
}
/// Table analogue of the line closeness score: the same shape as the
/// byte-histogram formula, but counting per-column agreement of non-empty
/// cells. Byte histograms ignore columns, so a sparse row scores the same
/// (clamped) against every long row and ties fall to the last candidate.
fn cell_score(a: &[u64], b: &[u64]) -> i64 {
    let len_a = a.iter().filter(|&&h| h != 0).count() as i64;
    let len_b = b.iter().filter(|&&h| h != 0).count() as i64;
    if len_a == 0 && len_b == 0 {
        return -10;
    }
    if len_a == 0 || len_b == 0 {
        return (-4 * (len_a + len_b) - 1).clamp(-40, -11);
    }
    let common = a.iter().zip(b).filter(|(x, y)| **x != 0 && x == y).count() as i64;
    let t = 3 * common - len_a - len_b;
    if t < 1 {
        (4 * t - 1).max(-40)
    } else {
        25 * t / len_a.min(len_b)
    }
}
/// One emitted row of a refined run.
fn run_row(
    l: Option<usize>,
    r: Option<usize>,
    hashes: (&[ContentHash], &[ContentHash]),
) -> AlignRow {
    let kind = match (l, r) {
        (Some(l), Some(r)) if hashes.0[l] == hashes.1[r] => DiffKind::Same,
        (Some(_), Some(_)) => DiffKind::Changed,
        (Some(_), None) => DiffKind::LeftOnly,
        _ => DiffKind::RightOnly,
    };
    AlignRow {
        left: l,
        right: r,
        center: None,
        kind,
    }
}
/// Content alignment only: re-resolve every run of non-Same rows between
/// fixed rows (hash-equal pairs and manual anchors) with a monotone
/// cell-closeness DP. Same tie rules as the core closeness DP: the pair wins
/// ties, and a gap must be strictly better.
#[allow(clippy::too_many_arguments)]
fn refine_content_runs<S: TableData>(
    rows: Vec<AlignRow>,
    left: &S,
    right: &S,
    orders: (&[usize], &[usize]),
    rules: &TableRules,
    keys: &[KeyColumn],
    hashes: (&[ContentHash], &[ContentHash]),
    anchors: &AlignmentOverrides,
    cancel: &dyn CancelCheck,
) -> Result<Vec<AlignRow>> {
    const PAIR: u8 = 0;
    const LEFT_GAP: u8 = 1;
    const RIGHT_GAP: u8 = 2;
    let gap = bc_core::align::CLOSENESS_GAP_PENALTY;
    let (lo, ro) = orders;
    let (lh, rh) = hashes;
    let fixed = |row: &AlignRow| match (row.left, row.right) {
        (Some(l), Some(r)) => lh[l] == rh[r] || anchors.pairs().binary_search(&(l, r)).is_ok(),
        _ => false,
    };
    let mut out = Vec::with_capacity(rows.len());
    let mut bytes = Vec::new();
    let mut start = 0;
    while start < rows.len() {
        if start.is_multiple_of(4096) {
            cancel.check()?;
        }
        if fixed(&rows[start]) {
            out.push(rows[start]);
            start += 1;
            continue;
        }
        let mut end = start;
        while end < rows.len() && !fixed(&rows[end]) {
            end += 1;
        }
        let run = &rows[start..end];
        start = end;
        let ls: Vec<usize> = run.iter().filter_map(|r| r.left).collect();
        let rs: Vec<usize> = run.iter().filter_map(|r| r.right).collect();
        if ls.is_empty()
            || rs.is_empty()
            || ls
                .len()
                .saturating_mul(rs.len())
                .saturating_mul(keys.len().max(1))
                > CELL_CLOSENESS_BUDGET
        {
            out.extend_from_slice(run);
            continue;
        }
        cancel.check()?;
        let la: Vec<_> = ls
            .iter()
            .map(|&l| cell_atoms(left, lo[l], Side::Left, rules, keys, &mut bytes))
            .collect();
        let ra: Vec<_> = rs
            .iter()
            .map(|&r| cell_atoms(right, ro[r], Side::Right, rules, keys, &mut bytes))
            .collect();
        let cols = rs.len() + 1;
        let mut directions = vec![PAIR; (ls.len() + 1) * cols];
        directions[1..cols].fill(RIGHT_GAP);
        let mut previous: Vec<i64> = (0..cols as i64).map(|j| j * gap).collect();
        let mut current = vec![0i64; cols];
        for (i, a) in la.iter().enumerate() {
            cancel.check()?;
            let i = i + 1;
            current[0] = i as i64 * gap;
            directions[i * cols] = LEFT_GAP;
            for (j, b) in ra.iter().enumerate() {
                let j = j + 1;
                let mut best = previous[j - 1] + cell_score(a, b);
                let mut direction = PAIR;
                if current[j - 1] + gap > best {
                    best = current[j - 1] + gap;
                    direction = RIGHT_GAP;
                }
                if previous[j] + gap > best {
                    best = previous[j] + gap;
                    direction = LEFT_GAP;
                }
                current[j] = best;
                directions[i * cols + j] = direction;
            }
            std::mem::swap(&mut previous, &mut current);
        }
        let (mut i, mut j) = (ls.len(), rs.len());
        let mut path = Vec::with_capacity(i + j);
        while i > 0 || j > 0 {
            let direction = directions[i * cols + j];
            path.push(direction);
            match direction {
                PAIR => {
                    i -= 1;
                    j -= 1;
                }
                LEFT_GAP => i -= 1,
                _ => j -= 1,
            }
        }
        let (mut i, mut j) = (0, 0);
        for &direction in path.iter().rev() {
            let (l, r) = match direction {
                PAIR => (Some(ls[i]), Some(rs[j])),
                LEFT_GAP => (Some(ls[i]), None),
                _ => (None, Some(rs[j])),
            };
            i += usize::from(l.is_some());
            j += usize::from(r.is_some());
            out.push(run_row(l, r, hashes));
        }
    }
    Ok(out)
}
fn patience(
    l: AlignInput<'_>,
    r: AlignInput<'_>,
    overrides: &AlignmentOverrides,
    cancel: &dyn CancelCheck,
) -> Result<AlignMap> {
    let mut rows = Vec::new();
    for op in PatienceDiff.diff_cancellable(l.hashes, r.hashes, cancel)? {
        match op {
            DiffOp::Equal {
                a_start,
                b_start,
                len,
            } => {
                for i in 0..len {
                    if i.is_multiple_of(4096) {
                        cancel.check()?;
                    }
                    rows.push(AlignRow {
                        left: Some(a_start + i),
                        right: Some(b_start + i),
                        kind: DiffKind::Same,
                        center: None,
                    });
                }
            }
            DiffOp::Delete { a_start, len, .. } => {
                for i in 0..len {
                    if i.is_multiple_of(4096) {
                        cancel.check()?;
                    }
                    rows.push(AlignRow {
                        left: Some(a_start + i),
                        kind: DiffKind::LeftOnly,
                        ..Default::default()
                    });
                }
            }
            DiffOp::Insert { b_start, len, .. } => {
                for i in 0..len {
                    if i.is_multiple_of(4096) {
                        cancel.check()?;
                    }
                    rows.push(AlignRow {
                        right: Some(b_start + i),
                        kind: DiffKind::RightOnly,
                        ..Default::default()
                    });
                }
            }
        }
    }
    let mut map = AlignMap::from_rows_cancellable(rows, cancel)?;
    for &(l, r) in overrides.pairs() {
        cancel.check()?;
        map.force_pair(l, r)?;
    }
    Ok(map)
}
impl KeyAlign {
    /// Align materialized sheets (original public entry; behavior unchanged).
    pub fn align(
        &self,
        left: &Sheet,
        right: &Sheet,
        rules: &TableRules,
        overrides: &AlignmentOverrides,
        cancel: &dyn CancelCheck,
    ) -> Result<KeyAlignment> {
        self.align_data(left, right, rules, overrides, cancel)
    }
    /// Generic read-only entry over `TableData` (prototype/demand storage support).
    pub fn align_data<S: TableData + Sync>(
        &self,
        left: &S,
        right: &S,
        rules: &TableRules,
        overrides: &AlignmentOverrides,
        cancel: &dyn CancelCheck,
    ) -> Result<KeyAlignment> {
        cancel.check()?;
        rules.validate(left.column_count(), right.column_count())?;
        let uncached = (
            left.uncached_formula_columns(),
            right.uncached_formula_columns(),
        );
        let keys: Vec<KeyColumn> = rules
            .key_columns()
            .into_iter()
            .map(|c| {
                let column = &rules.columns.0[c];
                let flagged = |side: &[bool], index: Option<usize>| {
                    index.is_some_and(|i| side.get(i).copied().unwrap_or(false))
                };
                let by_formula =
                    flagged(&uncached.0, column.left) || flagged(&uncached.1, column.right);
                (c, rules.handling(c), by_formula)
            })
            .collect();
        let t = std::time::Instant::now();
        let lo = order(left, Side::Left, rules, &keys, cancel)?;
        let ro = order(right, Side::Right, rules, &keys, cancel)?;
        crate::perf::span("key-align:order", t);
        let t = std::time::Instant::now();
        // Each side chunks internally under the shared phase budget; sides
        // run one at a time so staging never stacks (row_text pattern).
        let (mut lh, ld) = hashes(left, &lo, Side::Left, rules, &keys, cancel)?;
        let (mut rh, rd) = hashes(right, &ro, Side::Right, rules, &keys, cancel)?;
        crate::perf::span("key-align:hashes", t);
        if keys
            .iter()
            .any(|(_, h, _)| h.numeric_tolerance > 0.0 || h.date_tolerance.0 > 0)
        {
            (lh, rh) = tolerant_tokens(
                &KeyContext {
                    left,
                    right,
                    rules,
                    keys: &keys,
                },
                (&lo, &ro),
                cancel,
            )?;
        }
        let mut anchors = AlignmentOverrides::default();
        if !overrides.pairs().is_empty() {
            let mut li = vec![0; lo.len()];
            let mut ri = vec![0; ro.len()];
            for (i, &r) in lo.iter().enumerate() {
                li[r] = i;
            }
            for (i, &r) in ro.iter().enumerate() {
                ri[r] = i;
            }
            for &(l, r) in overrides.pairs() {
                let l = *li
                    .get(l)
                    .ok_or_else(|| invalid("left anchor out of range"))?;
                let r = *ri
                    .get(r)
                    .ok_or_else(|| invalid("right anchor out of range"))?;
                anchors.insert(l, r)?;
            }
        }
        let t = std::time::Instant::now();
        let text = if rules.strategy == AlignStrategy::Smart
            && rules.use_closeness
            && may_have_orphan_blocks(&lh, &rh, overrides, cancel)?
        {
            // The sides chunk internally under the shared phase budget; built
            // one side at a time so their staging buffers never overlap.
            Some((row_text(left, &lo, cancel)?, row_text(right, &ro, cancel)?))
        } else {
            None
        };
        crate::perf::span("key-align:row-text", t);
        let l = AlignInput {
            hashes: &lh,
            lines: text.as_ref().map(|t| &t.0),
            ..Default::default()
        };
        let r = AlignInput {
            hashes: &rh,
            lines: text.as_ref().map(|t| &t.1),
            ..Default::default()
        };
        let t = std::time::Instant::now();
        let map = match rules.strategy {
            AlignStrategy::Quick => {
                QuickAlign.align2_unpaired_cancellable(l, r, &anchors, cancel)?
            }
            AlignStrategy::Smart if rules.use_closeness => {
                SmartAlign::default().align2_cancellable(l, r, &anchors, cancel)?
            }
            AlignStrategy::Smart => {
                QuickAlign.align2_unpaired_cancellable(l, r, &anchors, cancel)?
            }
            AlignStrategy::Patience => patience(l, r, &anchors, cancel)?,
            AlignStrategy::None => {
                let mut map = AlignMap::unaligned(&lh, &rh);
                for &(l, r) in anchors.pairs() {
                    cancel.check()?;
                    map.force_pair(l, r)?;
                }
                map
            }
        };
        // row_text's line buffers are only inputs to the strategy alignment
        // above; release them before verify-filter instead of at fn return.
        drop(text);
        crate::perf::span("key-align:strategy-align", t);
        let t = std::time::Instant::now();
        let mut map_rows = map.into_rows();
        // Content alignment pairs rows by cell agreement, not line text.
        if rules.content_aligned() && rules.strategy == AlignStrategy::Smart && rules.use_closeness
        {
            map_rows = refine_content_runs(
                map_rows,
                left,
                right,
                (&lo, &ro),
                rules,
                &keys,
                (&lh, &rh),
                &anchors,
                cancel,
            )?;
        }
        crate::perf::span("key-align:cell-closeness", t);
        let t = std::time::Instant::now();
        let threads = crate::perf::phase_threads(map_rows.len());
        // Same definition as the report's `key:` line: content alignment (no
        // real key columns) must not verify-split closeness pairs.
        let keyed = !rules.content_aligned();
        let mut filtered = Vec::with_capacity(map_rows.len());
        if threads < 2 {
            filtered.extend(verify_filter_chunk(
                &map_rows,
                left,
                right,
                (&lo, &ro),
                rules,
                &keys,
                keyed,
                (&lh, &rh),
                &anchors,
                cancel,
            )?);
        } else {
            let chunk = map_rows.len().div_ceil(threads).max(1);
            let results = std::thread::scope(|scope| {
                let mut handles = Vec::new();
                for slice in map_rows.chunks(chunk) {
                    handles.push(scope.spawn(|| {
                        verify_filter_chunk(
                            slice,
                            left,
                            right,
                            (&lo, &ro),
                            rules,
                            &keys,
                            keyed,
                            (&lh, &rh),
                            &anchors,
                            cancel,
                        )
                    }));
                }
                handles
                    .into_iter()
                    .map(|h| h.join().unwrap_or_else(|_| Err(invalid("verify filter panicked"))))
                    .collect::<Vec<_>>()
            });
            for result in results {
                filtered.extend(result?);
            }
        }
        crate::perf::span("key-align:verify-filter", t);
        Ok(KeyAlignment {
            map: AlignMap::from_rows_cancellable(filtered, cancel)?,
            left_order: lo,
            right_order: ro,
            duplicates: DuplicateKeys {
                left: ld,
                right: rd,
            },
        })
    }
}
