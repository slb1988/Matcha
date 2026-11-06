//! Key preparation only; all sequence alignment is delegated to bc-core.
use crate::{
    columns::Side,
    compare::{equal_values, normalized_value},
    invalid,
    rules::ColumnHandling,
    CellValue, Result, Sheet, TableRules,
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
fn atom_bytes(value: &CellValue, out: &mut Vec<u8>) {
    match value {
        CellValue::Empty => out.push(0),
        CellValue::Text(t) => {
            out.push(1);
            out.extend(t.as_bytes());
        }
        CellValue::Number(n) => {
            out.push(2);
            out.extend(if *n == 0.0 { 0.0f64 } else { *n }.to_bits().to_le_bytes());
        }
        CellValue::Bool(b) => {
            out.push(3);
            out.push(u8::from(*b));
        }
        CellValue::Date(d) => {
            out.push(4);
            out.extend(d.to_le_bytes());
        }
        CellValue::Error(t) => {
            out.push(5);
            out.extend(t.as_bytes());
        }
    }
}
fn hash_row(
    sheet: &Sheet,
    row: usize,
    side: Side,
    rules: &TableRules,
    keys: &[(usize, ColumnHandling)],
    exact_only: bool,
    bytes: &mut Vec<u8>,
) -> ContentHash {
    bytes.clear();
    for (column, handling) in keys {
        if exact_only && (handling.numeric_tolerance > 0.0 || handling.date_tolerance.0 > 0) {
            continue;
        }
        let column = rules.columns.0[*column].index(side);
        let value = normalized_value(sheet, row, column, handling);
        let start = bytes.len();
        bytes.extend([0; 8]);
        atom_bytes(&value, bytes);
        let len = (bytes.len() - start - 8) as u64;
        bytes[start..start + 8].copy_from_slice(&len.to_le_bytes());
    }
    LineHasher::default().hash_line(bytes)
}
fn keys_equal(
    left: &Sheet,
    right: &Sheet,
    l: usize,
    r: usize,
    rules: &TableRules,
    keys: &[(usize, ColumnHandling)],
) -> bool {
    keys.iter().all(|(c, h)| {
        equal_values(
            &normalized_value(left, l, rules.columns.0[*c].left, h),
            &normalized_value(right, r, rules.columns.0[*c].right, h),
            h,
        )
    })
}
pub(crate) fn value_cmp(l: &CellValue, r: &CellValue) -> Ordering {
    fn rank(v: &CellValue) -> u8 {
        match v {
            CellValue::Empty => 0,
            CellValue::Number(_) => 1,
            CellValue::Date(_) => 2,
            CellValue::Bool(_) => 3,
            CellValue::Text(_) => 4,
            CellValue::Error(_) => 5,
        }
    }
    match (l, r) {
        (CellValue::Number(l), CellValue::Number(r)) => l.total_cmp(r),
        (CellValue::Date(l), CellValue::Date(r)) => l.cmp(r),
        (CellValue::Bool(l), CellValue::Bool(r)) => l.cmp(r),
        (CellValue::Text(l), CellValue::Text(r)) | (CellValue::Error(l), CellValue::Error(r)) => {
            l.cmp(r)
        }
        _ => rank(l).cmp(&rank(r)),
    }
}
fn order(
    sheet: &Sheet,
    side: Side,
    rules: &TableRules,
    keys: &[(usize, ColumnHandling)],
    cancel: &dyn CancelCheck,
) -> Result<Vec<usize>> {
    let mut order: Vec<_> = (0..sheet.row_count()).collect();
    if rules.sort_by_key {
        order.sort_by(|&a, &b| {
            if cancel.is_cancelled() {
                return Ordering::Equal;
            }
            for (c, h) in keys {
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
fn hashes(
    sheet: &Sheet,
    order: &[usize],
    side: Side,
    rules: &TableRules,
    keys: &[(usize, ColumnHandling)],
    cancel: &dyn CancelCheck,
) -> Result<(Vec<ContentHash>, usize)> {
    let mut counts: HashMap<ContentHash, u64, U64KeyBuild> = HashMap::default();
    let mut duplicates = 0;
    let mut result = Vec::with_capacity(order.len());
    let mut scratch = Vec::new();
    for (i, &row) in order.iter().enumerate() {
        if i.is_multiple_of(1024) {
            cancel.check()?;
        }
        let hash = hash_row(sheet, row, side, rules, keys, false, &mut scratch);
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
fn bucket(
    sheet: &Sheet,
    row: usize,
    side: Side,
    rules: &TableRules,
    first: &(usize, ColumnHandling),
) -> i64 {
    let (c, h) = first;
    let value = normalized_value(sheet, row, rules.columns.0[*c].index(side), h);
    match &*value {
        CellValue::Number(n) if h.numeric_tolerance > 0.0 => {
            (*n / h.numeric_tolerance).floor() as i64
        }
        CellValue::Date(ms) if h.date_tolerance.0 > 0 => ms.div_euclid(h.date_tolerance.0),
        _ => i64::MIN,
    }
}
struct KeyContext<'a> {
    left: &'a Sheet,
    right: &'a Sheet,
    rules: &'a TableRules,
    keys: &'a [(usize, ColumnHandling)],
}
/// Positive tolerances are not transitive, so quantizing alone is incorrect.
/// Search neighboring buckets and verify every candidate against the actual
/// tolerance; assign shared pair IDs in stable occurrence order.
fn tolerant_tokens(
    ctx: &KeyContext<'_>,
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
        .find(|(_, h)| h.numeric_tolerance > 0.0 || h.date_tolerance.0 > 0)
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
fn row_text(sheet: &Sheet, order: &[usize], cancel: &dyn CancelCheck) -> Result<LineList> {
    let mut text = Vec::with_capacity(order.len() * 32);
    let mut cell_buf = String::new();
    for (i, &row) in order.iter().enumerate() {
        if i.is_multiple_of(1024) {
            cancel.check()?;
        }
        let mut count = 0;
        for column in 0..sheet.column_count() {
            if column > 0 {
                text.push(b'\t');
            }
            cell_buf.clear();
            sheet.cell(row, column).unwrap().display_into(&mut cell_buf);
            for c in cell_buf.chars() {
                if count >= 256 {
                    break;
                }
                let c = if matches!(c, '\r' | '\n') { ' ' } else { c };
                text.extend(c.encode_utf8(&mut [0; 4]).as_bytes());
                count += 1;
            }
            if count >= 256 {
                break;
            }
        }
        text.push(b'\n');
    }
    Ok(LineList::from_bytes(text))
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
    pub fn align(
        &self,
        left: &Sheet,
        right: &Sheet,
        rules: &TableRules,
        overrides: &AlignmentOverrides,
        cancel: &dyn CancelCheck,
    ) -> Result<KeyAlignment> {
        cancel.check()?;
        rules.validate(left.column_count(), right.column_count())?;
        let keys: Vec<_> = rules
            .key_columns()
            .into_iter()
            .map(|c| (c, rules.handling(c)))
            .collect();
        let t = std::time::Instant::now();
        let lo = order(left, Side::Left, rules, &keys, cancel)?;
        let ro = order(right, Side::Right, rules, &keys, cancel)?;
        crate::perf::span("key-align:order", t);
        let t = std::time::Instant::now();
        let (mut lh, ld) = hashes(left, &lo, Side::Left, rules, &keys, cancel)?;
        let (mut rh, rd) = hashes(right, &ro, Side::Right, rules, &keys, cancel)?;
        crate::perf::span("key-align:hashes", t);
        if keys
            .iter()
            .any(|(_, h)| h.numeric_tolerance > 0.0 || h.date_tolerance.0 > 0)
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
        let text = if rules.strategy == AlignStrategy::Smart && rules.use_closeness {
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
        crate::perf::span("key-align:strategy-align", t);
        let t = std::time::Instant::now();
        let mut filtered = Vec::with_capacity(map.len());
        for (i, row) in map.into_rows().into_iter().enumerate() {
            if i.is_multiple_of(1024) {
                cancel.check()?;
            }
            if let (Some(l), Some(r)) = (row.left, row.right) {
                let forced = rules.strategy == AlignStrategy::None
                    || anchors.pairs().binary_search(&(l, r)).is_ok();
                if !forced
                    && (lh[l] != rh[r] || !keys_equal(left, right, lo[l], ro[r], rules, &keys))
                {
                    filtered.push(AlignRow {
                        left: Some(l),
                        kind: DiffKind::LeftOnly,
                        ..Default::default()
                    });
                    filtered.push(AlignRow {
                        right: Some(r),
                        kind: DiffKind::RightOnly,
                        ..Default::default()
                    });
                    continue;
                }
            }
            filtered.push(row);
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
