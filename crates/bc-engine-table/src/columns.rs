use crate::{
    data::{CellData, CellInfo, TableData},
    invalid, rules::ColumnHandling, Result, Sheet,
};
use bc_core::{
    cancel::CancelCheck,
    hash::{ContentHash, LineHasher},
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Left,
    Right,
}
impl Side {
    pub fn other(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnChange {
    Inserted,
    Deleted,
    Renamed,
    Moved,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparisonColumn {
    pub left: Option<usize>,
    pub right: Option<usize>,
    pub handling: ColumnHandling,
    pub changes: Vec<ColumnChange>,
}
impl ComparisonColumn {
    pub fn new(left: Option<usize>, right: Option<usize>) -> Self {
        Self {
            left,
            right,
            handling: ColumnHandling {
                use_default: true,
                ..Default::default()
            },
            changes: Vec::new(),
        }
    }
    pub fn index(&self, side: Side) -> Option<usize> {
        match side {
            Side::Left => self.left,
            Side::Right => self.right,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ColumnMapping(pub Vec<ComparisonColumn>);
impl ColumnMapping {
    pub fn by_number(left: usize, right: usize) -> Self {
        Self(
            (0..left.max(right))
                .map(|i| {
                    let mut column =
                        ComparisonColumn::new((i < left).then_some(i), (i < right).then_some(i));
                    if i >= left {
                        column.changes.push(ColumnChange::Inserted);
                    }
                    if i >= right {
                        column.changes.push(ColumnChange::Deleted);
                    }
                    column
                })
                .collect(),
        )
    }
    /// A None side moves the whole comparison column, including Handling.
    pub fn move_to(&mut self, from: usize, to: usize, side: Option<Side>) -> Result<()> {
        if from >= self.0.len() || to >= self.0.len() {
            return Err(invalid("mapping index out of range"));
        }
        if let Some(side) = side {
            let value = self.0[from].index(side);
            if from < to {
                for i in from..to {
                    let next = self.0[i + 1].index(side);
                    set_side(&mut self.0[i], side, next);
                }
            } else {
                for i in (to + 1..=from).rev() {
                    let prev = self.0[i - 1].index(side);
                    set_side(&mut self.0[i], side, prev);
                }
            }
            set_side(&mut self.0[to], side, value);
            for column in &mut self.0 {
                column.changes.clear();
            }
        } else {
            let column = self.0.remove(from);
            self.0.insert(to, column);
        }
        Ok(())
    }
    pub fn tidy(&mut self) {
        self.0.retain(|c| c.left.is_some() || c.right.is_some());
    }
    /// Swap Sides: every comparison column exchanges its left/right source, and
    /// directional structure annotations flip with it (a column inserted on the
    /// right is a column deleted from the new right after the swap).
    pub fn transpose(&mut self) {
        for c in &mut self.0 {
            std::mem::swap(&mut c.left, &mut c.right);
            for change in &mut c.changes {
                *change = match change {
                    ColumnChange::Inserted => ColumnChange::Deleted,
                    ColumnChange::Deleted => ColumnChange::Inserted,
                    ColumnChange::Renamed => ColumnChange::Renamed,
                    ColumnChange::Moved => ColumnChange::Moved,
                };
            }
        }
    }
    pub fn validate(&self, left: usize, right: usize) -> Result<()> {
        for (side, len) in [(Side::Left, left), (Side::Right, right)] {
            let mut seen = HashSet::new();
            for c in &self.0 {
                if let Some(i) = c.index(side) {
                    if i >= len || !seen.insert(i) {
                        return Err(invalid("duplicate or out-of-range mapped column"));
                    }
                }
            }
        }
        Ok(())
    }
}
fn set_side(c: &mut ComparisonColumn, side: Side, value: Option<usize>) {
    match side {
        Side::Left => c.left = value,
        Side::Right => c.right = value,
    }
}
pub fn normalized_name(name: &str) -> String {
    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Canonical exact value bytes, length-delimited by the caller for composite keys.
pub(crate) fn value_bytes(cell: Option<&CellInfo>, handling: &ColumnHandling) -> Vec<u8> {
    let mut bytes = Vec::new();
    value_bytes_write(cell, handling, &mut bytes);
    bytes
}
fn value_bytes_write(cell: Option<&CellInfo>, handling: &ColumnHandling, out: &mut Vec<u8>) {
    let Some(cell) = cell else {
        out.push(0);
        return;
    };
    if handling.column_type == crate::rules::ColumnType::Text {
        out.push(1);
        let mut text = String::new();
        cell.value.display_into(&mut text);
        out.extend(handling.normalize_text(&text).as_bytes());
        return;
    }
    match &cell.value {
        CellData::Empty => out.push(0),
        CellData::Text(s) => {
            out.push(1);
            if !handling.ignore_case && !handling.ignore_whitespace {
                // Identity normalization: copy without the String round-trip.
                out.extend(s.as_bytes());
            } else {
                out.extend(handling.normalize_text(s).as_bytes());
            }
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
        CellData::Error(e) => {
            out.push(5);
            out.extend(e.as_bytes());
        }
    }
}
/// Same hash as the historical allocating `row_hash`, reusing caller scratch
/// instead of allocating per row. Hot for full-column key inference over
/// large sheets.
pub(crate) fn row_hash_into<S: TableData>(
    sheet: &S,
    row: usize,
    columns: &[usize],
    handling: &[ColumnHandling],
    bytes: &mut Vec<u8>,
) {
    bytes.clear();
    for (&column, handling) in columns.iter().zip(handling) {
        let start = bytes.len();
        bytes.extend([0; 8]);
        let info = sheet.cell_info(row, column);
        value_bytes_write(info.as_ref(), handling, bytes);
        let len = (bytes.len() - start - 8) as u64;
        bytes[start..start + 8].copy_from_slice(&len.to_le_bytes());
    }
}
/// Hash of a scratch-filled row encoding produced by `row_hash_into`.
pub(crate) fn finish_row_hash(bytes: &[u8]) -> ContentHash {
    LineHasher::default().hash_line(bytes)
}
fn column_sample<S: TableData>(sheet: &S, column: usize) -> (HashSet<ContentHash>, [usize; 6]) {
    let mut set = HashSet::new();
    let mut types = [0; 6];
    for i in 0..sheet.row_count().min(1000) {
        let row = i * sheet.row_count() / sheet.row_count().clamp(1, 1000);
        let info = sheet.cell_info(row, column).unwrap();
        let kind = match info.value {
            CellData::Empty => 0,
            CellData::Text(_) => 1,
            CellData::Number(_) => 2,
            CellData::Bool(_) => 3,
            CellData::Date(_) => 4,
            CellData::Error(_) => 5,
        };
        types[kind] += 1;
        set.insert(
            LineHasher::default().hash_line(&value_bytes(Some(&info), &ColumnHandling::default())),
        );
    }
    (set, types)
}
/// Header-first matching, then deterministic greedy maximum similarity among
/// unmatched columns (1000 uniformly sampled values, threshold 0.9).
/// Align columns of materialized sheets (original public entry; behavior unchanged).
pub fn align_columns(
    left: &Sheet,
    right: &Sheet,
    left_is_source: bool,
    cancel: &dyn CancelCheck,
) -> Result<ColumnMapping> {
    align_columns_data(left, right, left_is_source, cancel)
}
/// Generic read-only entry over `TableData` (prototype/demand storage support).
pub fn align_columns_data<S: TableData>(
    left: &S,
    right: &S,
    left_is_source: bool,
    cancel: &dyn CancelCheck,
) -> Result<ColumnMapping> {
    let mut pairs = vec![None; left.column_count()];
    let mut used = HashSet::new();
    if let (Some(lh), Some(rh)) = (left.headers(), right.headers()) {
        for (l, name) in lh.iter().enumerate().take(left.column_count()) {
            cancel.check()?;
            let name = normalized_name(name);
            if name.is_empty() {
                continue;
            }
            if let Some(r) = rh
                .iter()
                .enumerate()
                .take(right.column_count())
                .find(|(r, n)| !used.contains(r) && normalized_name(n) == name)
                .map(|(r, _)| r)
            {
                pairs[l] = Some(r);
                used.insert(r);
            }
        }
    }
    let mut candidates = Vec::new();
    let ls: HashMap<_, _> = (0..left.column_count())
        .filter(|&l| pairs[l].is_none())
        .map(|l| (l, column_sample(left, l)))
        .collect();
    let rs: HashMap<_, _> = (0..right.column_count())
        .filter(|r| !used.contains(r))
        .map(|r| (r, column_sample(right, r)))
        .collect();
    for (&l, (lv, lt)) in &ls {
        for (&r, (rv, rt)) in &rs {
            cancel.check()?;
            let union = lv.union(rv).count();
            if union == 0 {
                continue;
            }
            let j = lv.intersection(rv).count() as f64 / union as f64;
            let total = lt.iter().sum::<usize>().max(rt.iter().sum()).max(1) as f64;
            let types = lt.iter().zip(rt).map(|(a, b)| a.min(b)).sum::<usize>() as f64 / total;
            let score = 0.9 * j + 0.1 * types;
            if score >= 0.9 {
                candidates.push((l, r, score));
            }
        }
    }
    candidates.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)).then(a.1.cmp(&b.1)));
    for (l, r, _) in candidates {
        if pairs[l].is_none() && used.insert(r) {
            pairs[l] = Some(r);
        }
    }
    let mut result = Vec::new();
    for (l, r) in pairs.into_iter().enumerate() {
        let mut c = ComparisonColumn::new(Some(l), r);
        match r {
            None => c.changes.push(ColumnChange::Deleted),
            Some(r) => {
                if let (Some(lh), Some(rh)) = (left.headers(), right.headers()) {
                    if normalized_name(&lh[l]) != normalized_name(&rh[r]) {
                        c.changes.push(ColumnChange::Renamed);
                    } else if l != r {
                        c.changes.push(ColumnChange::Moved);
                    }
                }
            }
        }
        result.push(c);
    }
    for r in 0..right.column_count() {
        if !used.contains(&r) {
            let mut c = ComparisonColumn::new(None, Some(r));
            c.changes.push(ColumnChange::Inserted);
            // Place new columns near their following paired column, not always at the end.
            let at = result
                .iter()
                .position(|c| c.right.is_some_and(|i| i > r))
                .unwrap_or(result.len());
            result.insert(at, c);
        }
    }
    if !left_is_source {
        result.sort_by_key(|c| c.right.unwrap_or(usize::MAX));
    }
    Ok(ColumnMapping(result))
}
