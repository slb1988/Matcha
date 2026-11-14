//! Three-way table merge (left / center / right), GUI independent.
//!
//! Two layers, like the text merge (`bc-engine-text::merge`):
//!
//! * **Rows** are matched by key. Every row's key columns hash to a token that carries
//!   its occurrence number (the hashing of two-way key alignment), so the k-th row with
//!   key 7 on the left meets the k-th row with key 7 in the center. Rows the center does
//!   not have are matched between left and right by the same tokens: two sides adding
//!   the same key is one row (a double add, a conflict when the contents differ).
//!   Center rows keep their physical order; a row only one side has is placed after the
//!   row that precedes it on its own side (the buckets of `bc_core::align::reconcile3`).
//!   Row order is therefore the center's: reordering rows is not merged.
//! * **Cells** of a row present on all three sides are classified like text rows:
//!   unchanged, changed left, changed right, changed identically, changed differently
//!   (a conflict). A row missing on some side is structural: added, deleted, double add
//!   (conflict when contents differ), delete vs modify (conflict).
//!
//! Columns are matched by position over the widest of the three sheets; a side that is
//! narrower reads as empty cells there, so a column one side appended merges like any
//! other change.
use crate::{
    align::{hashes, KeyColumn},
    columns::{ColumnMapping, Side as PairSide},
    compare::{CellComparator, Importance},
    inference::infer_keys_data,
    invalid,
    model::{RawRow, SheetSource, TextSource},
    rules::{ColumnHandling, KeySelection, Layers, TableRules},
    Cell, Result, Sheet,
};
use bc_core::{cancel::NeverCancel, hash::ContentHash, Side};
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    sync::Arc,
};

/// Side taken for conflicts that consist of unimportant differences only (`/favorleft`,
/// `/favorright`, with ignore-unimportant).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Favor {
    #[default]
    None,
    Left,
    Right,
}

/// Merge settings decided when the merge opens.
#[derive(Debug, Clone, Default)]
pub struct TableMergeOptions {
    /// Explicit key columns (0-based comparison columns, priority order).
    pub keys: Option<Vec<usize>>,
    pub ignore_unimportant: bool,
    pub favor: Favor,
    /// Column handling by comparison column; missing entries use `default_handling`.
    pub handling: Vec<ColumnHandling>,
    pub default_handling: ColumnHandling,
}

/// Where the key columns came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource {
    Explicit,
    Inferred,
    /// No usable key: every column is key (row identity is the whole row).
    AllColumns,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeKeys {
    /// 0-based comparison columns.
    pub columns: Vec<usize>,
    pub source: KeySource,
}

impl MergeKeys {
    /// `1,3 (inferred)` style label (1-based, as `/keys=` takes them).
    pub fn label(&self) -> String {
        let columns: Vec<String> = self.columns.iter().map(|c| (c + 1).to_string()).collect();
        let source = match self.source {
            KeySource::Explicit => "explicit",
            KeySource::Inferred => "inferred",
            KeySource::AllColumns => "all columns",
        };
        if self.source == KeySource::AllColumns {
            source.into()
        } else {
            format!("{} ({source})", columns.join(","))
        }
    }
}

/// Change type of one cell relative to the center (same meaning as text merge rows).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CellChoice {
    #[default]
    Center,
    Left,
    Right,
    Both,
    Conflict,
}

/// Row structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// Present on all three sides; cells decide.
    Normal,
    LeftAdded,
    RightAdded,
    /// Both sides added the key with equal contents.
    BothAdded,
    /// Both sides added the key with different contents (conflict).
    AddConflict,
    /// Deleted on one side (other side unchanged) or on both.
    Deleted,
    /// Deleted on one side, modified on the other (conflict).
    DeleteConflict,
}

impl RowKind {
    pub fn is_structural_conflict(self) -> bool {
        matches!(self, RowKind::AddConflict | RowKind::DeleteConflict)
    }
}

/// One aligned row: source row indices (gaps are `None`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MergeRow {
    pub left: Option<usize>,
    pub center: Option<usize>,
    pub right: Option<usize>,
    pub kind: RowKind,
}

impl MergeRow {
    pub fn index(&self, side: Side) -> Option<usize> {
        match side {
            Side::Left => self.left,
            Side::Right => self.right,
            Side::Center => self.center,
            Side::Output => None,
        }
    }
}

/// Unresolved conflict section: consecutive rows with conflicts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub rows: Range<usize>,
    pub importance: Importance,
    /// The whole sheet: deleted on one side, modified on the other.
    pub sheet: bool,
}

/// Where an output cell comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellSource {
    Left,
    Center,
    Right,
    /// Typed into the output.
    Edited,
}

impl CellSource {
    fn of(side: Side) -> Self {
        match side {
            Side::Left => Self::Left,
            Side::Right => Self::Right,
            _ => Self::Center,
        }
    }
}

/// Resolution state; cloned whole for undo.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Resolution {
    /// Structural rows: side whose row the output takes (`Center` = the center row, or
    /// no row for a double add).
    rows: HashMap<usize, Side>,
    /// Cells (`row * columns + column`) taken from a side.
    cells: HashMap<usize, Side>,
    /// Cells typed into the output.
    edits: HashMap<usize, Cell>,
    /// Rows whose conflicts were accepted as they are.
    accepted: HashSet<usize>,
    /// Sheet-level conflict resolution.
    sheet: Option<Side>,
}

/// Merge of one sheet.
#[derive(Debug, Clone)]
pub struct SheetMerge {
    pub name: String,
    /// Sheet present on left / center / right.
    pub present: [bool; 3],
    /// Inputs padded to `columns` (missing sheets are empty).
    pub left: Sheet,
    pub center: Sheet,
    pub right: Sheet,
    pub columns: usize,
    pub keys: MergeKeys,
    /// Rows beyond the first with an already seen key, per left / center / right.
    pub duplicates: [usize; 3],
    pub rows: Vec<MergeRow>,
    /// Merged header names (text tables with a header row).
    pub headers: Option<Vec<String>>,
    cells: Vec<CellChoice>,
    importance: Vec<Importance>,
    /// Favor applied to ignored unimportant conflicts (`row * columns + column`).
    favored: HashMap<usize, Side>,
    /// Favor applied to an unimportant double add.
    favored_rows: HashMap<usize, Side>,
    /// One side deleted the sheet while the other changed it: the deleting side.
    sheet_deleted_by: Option<Side>,
    /// Deleted on both sides, or on one side with the other unchanged.
    dropped: bool,
    pub resolution: Resolution,
}

fn differs(importance: Importance, ignore: bool) -> bool {
    importance.projected(ignore) != Importance::Same
}

fn choice_of(raw: [Importance; 3], ignore: bool) -> CellChoice {
    match (differs(raw[0], ignore), differs(raw[1], ignore)) {
        (false, false) => CellChoice::Center,
        (true, false) => CellChoice::Left,
        (false, true) => CellChoice::Right,
        (true, true) if differs(raw[2], ignore) => CellChoice::Conflict,
        (true, true) => CellChoice::Both,
    }
}

fn pad(mut sheet: Sheet, columns: usize) -> Sheet {
    if sheet.column_count() < columns {
        let rows = sheet.row_count();
        sheet.ensure_size(rows, columns);
    }
    sheet
}

/// Comparison rules over `columns` positional columns.
fn rules_for(columns: usize, options: &TableMergeOptions) -> TableRules {
    let mut mapping = ColumnMapping::by_number(columns, columns);
    for (i, column) in mapping.0.iter_mut().enumerate() {
        column.handling = options
            .handling
            .get(i)
            .cloned()
            .unwrap_or_else(|| ColumnHandling {
                use_default: true,
                ..Default::default()
            });
    }
    TableRules {
        columns: mapping,
        default_handling: options.default_handling.clone(),
        layers: Layers::ValuesAndFormulas,
        ignore_unimportant: options.ignore_unimportant,
        ..TableRules::default()
    }
}

fn key_columns(rules: &TableRules, keys: &[usize], sheets: [&Sheet; 3]) -> Vec<KeyColumn> {
    let uncached: Vec<Vec<bool>> = sheets
        .iter()
        .map(|s| crate::data::TableData::uncached_formula_columns(*s))
        .collect();
    keys.iter()
        .map(|&c| {
            let by_formula = uncached
                .iter()
                .any(|side| side.get(c).copied().unwrap_or(false));
            (c, rules.handling(c), by_formula)
        })
        .collect()
}

/// Key tokens (key hash + occurrence number) in physical row order, and the number of
/// rows whose key was already seen.
fn tokens(
    sheet: &Sheet,
    rules: &TableRules,
    keys: &[KeyColumn],
) -> Result<(Vec<ContentHash>, usize)> {
    let order: Vec<usize> = (0..sheet.row_count()).collect();
    // Positional mapping: the left index equals the right index, either side reads alike.
    hashes(sheet, &order, PairSide::Left, rules, keys, &NeverCancel)
}

/// Inferred key columns that are unique on every non-empty side, else `None`.
fn infer(sheets: [&Sheet; 3], rules: &TableRules) -> Result<Option<Vec<usize>>> {
    let [left, center, right] = sheets;
    let mut candidates = Vec::new();
    let pairs: Vec<(&Sheet, &Sheet)> = if center.row_count() == 0 {
        vec![(left, right)]
    } else {
        vec![(center, left), (center, right)]
    };
    for (a, b) in pairs {
        if a.row_count() == 0 || b.row_count() == 0 {
            continue;
        }
        if let KeySelection::Automatic { columns, .. } =
            infer_keys_data(a, b, rules, &NeverCancel)?.selection
        {
            if !columns.is_empty() && !candidates.contains(&columns) {
                candidates.push(columns);
            }
        }
    }
    for columns in candidates {
        let keys = key_columns(rules, &columns, sheets);
        let mut unique = true;
        for sheet in sheets {
            if tokens(sheet, rules, &keys)?.1 > 0 {
                unique = false;
                break;
            }
        }
        if unique {
            return Ok(Some(columns));
        }
    }
    Ok(None)
}

/// Three-way key join; returns aligned rows (kind not yet set) in output order.
fn join(tokens: [&[ContentHash]; 3]) -> Vec<MergeRow> {
    let [lt, ct, rt] = tokens;
    let center_of: HashMap<ContentHash, usize> =
        ct.iter().enumerate().map(|(i, t)| (*t, i)).collect();
    let mut left_of = vec![None; ct.len()];
    let mut right_of = vec![None; ct.len()];
    let mut left_center = vec![None; lt.len()];
    let mut right_center = vec![None; rt.len()];
    for (l, token) in lt.iter().enumerate() {
        if let Some(&c) = center_of.get(token) {
            left_of[c] = Some(l);
            left_center[l] = Some(c);
        }
    }
    for (r, token) in rt.iter().enumerate() {
        if let Some(&c) = center_of.get(token) {
            right_of[c] = Some(r);
            right_center[r] = Some(c);
        }
    }
    // Rows the center lacks: match left and right by token.
    let left_orphans: HashMap<ContentHash, usize> = lt
        .iter()
        .enumerate()
        .filter(|(l, _)| left_center[*l].is_none())
        .map(|(l, t)| (*t, l))
        .collect();
    let mut partner_of_left = vec![None; lt.len()];
    let mut right_paired = vec![false; rt.len()];
    for (r, token) in rt.iter().enumerate() {
        if right_center[r].is_none() {
            if let Some(&l) = left_orphans.get(token) {
                partner_of_left[l] = Some(r);
                right_paired[r] = true;
            }
        }
    }
    // Buckets: an orphan goes after the center row its nearest preceding paired row
    // (on its own side) is paired with.
    let mut buckets: Vec<Vec<(Option<usize>, Option<usize>)>> = vec![Vec::new(); ct.len() + 1];
    let mut anchor = 0;
    for l in 0..lt.len() {
        match left_center[l] {
            Some(c) => anchor = c + 1,
            None => buckets[anchor].push((Some(l), partner_of_left[l])),
        }
    }
    anchor = 0;
    let mut right_only: Vec<Vec<usize>> = vec![Vec::new(); ct.len() + 1];
    for r in 0..rt.len() {
        match right_center[r] {
            Some(c) => anchor = c + 1,
            None if !right_paired[r] => right_only[anchor].push(r),
            None => {}
        }
    }
    let mut rows = Vec::with_capacity(ct.len().max(lt.len()).max(rt.len()));
    let row = |left, center, right| MergeRow {
        left,
        center,
        right,
        kind: RowKind::Normal,
    };
    for bucket in 0..=ct.len() {
        for &(l, r) in &buckets[bucket] {
            rows.push(row(l, None, r));
        }
        for &r in &right_only[bucket] {
            rows.push(row(None, None, Some(r)));
        }
        if bucket < ct.len() {
            rows.push(row(left_of[bucket], Some(bucket), right_of[bucket]));
        }
    }
    rows
}

/// Three-way pick of a header name (both changed differently keeps the center's).
fn pick<T: PartialEq + Clone>(left: &T, center: &T, right: &T) -> T {
    if left == center {
        right.clone()
    } else if right == center || left == right {
        left.clone()
    } else {
        center.clone()
    }
}

impl SheetMerge {
    /// Merge one sheet. `None` sheets are absent on that side.
    pub fn new(
        name: impl Into<String>,
        sheets: [Option<Sheet>; 3],
        options: &TableMergeOptions,
    ) -> Result<Self> {
        let present = [
            sheets[0].is_some(),
            sheets[1].is_some(),
            sheets[2].is_some(),
        ];
        let columns = sheets
            .iter()
            .flatten()
            .map(Sheet::column_count)
            .max()
            .unwrap_or(0);
        let [left, center, right] =
            sheets.map(|s| pad(s.unwrap_or_else(|| Sheet::new("")), columns));
        let rules = rules_for(columns, options);
        let keys = match &options.keys {
            Some(keys) => {
                if keys.is_empty() {
                    return Err(invalid("no key columns given"));
                }
                if let Some(bad) = keys.iter().find(|&&k| k >= columns) {
                    return Err(invalid(format!(
                        "key column {} is out of range (the table has {columns} column(s))",
                        bad + 1
                    )));
                }
                MergeKeys {
                    columns: keys.clone(),
                    source: KeySource::Explicit,
                }
            }
            None => match infer([&left, &center, &right], &rules)? {
                Some(columns) => MergeKeys {
                    columns,
                    source: KeySource::Inferred,
                },
                None => MergeKeys {
                    columns: (0..columns).collect(),
                    source: KeySource::AllColumns,
                },
            },
        };
        let key_cols = key_columns(&rules, &keys.columns, [&left, &center, &right]);
        let (lt, ld) = tokens(&left, &rules, &key_cols)?;
        let (ct, cd) = tokens(&center, &rules, &key_cols)?;
        let (rt, rd) = tokens(&right, &rules, &key_cols)?;
        let rows = join([&lt, &ct, &rt]);
        let headers = match (&left.headers, &center.headers, &right.headers) {
            (None, None, None) => None,
            (l, c, r) => {
                let get = |h: &Option<Vec<String>>, i: usize| {
                    h.as_ref()
                        .and_then(|h| h.get(i))
                        .cloned()
                        .unwrap_or_default()
                };
                Some(
                    (0..columns)
                        .map(|i| pick(&get(l, i), &get(c, i), &get(r, i)))
                        .collect(),
                )
            }
        };
        let mut merge = Self {
            name: name.into(),
            present,
            left,
            center,
            right,
            columns,
            keys,
            duplicates: [ld, cd, rd],
            rows,
            headers,
            cells: Vec::new(),
            importance: Vec::new(),
            favored: HashMap::new(),
            favored_rows: HashMap::new(),
            sheet_deleted_by: None,
            dropped: false,
            resolution: Resolution::default(),
        };
        merge.classify(&rules, options);
        merge.sheet_presence();
        Ok(merge)
    }

    fn sheet(&self, side: Side) -> &Sheet {
        match side {
            Side::Left => &self.left,
            Side::Right => &self.right,
            _ => &self.center,
        }
    }

    /// Raw importance of each column between two rows.
    fn compare_rows(
        &self,
        rules: &TableRules,
        a: (Side, usize),
        b: (Side, usize),
        out: &mut [Importance],
    ) {
        let (sa, sb) = (self.sheet(a.0), self.sheet(b.0));
        for (column, slot) in out.iter_mut().enumerate() {
            let handling = rules.handling(column);
            let comparator = CellComparator {
                handling: &handling,
                format_important: false,
            };
            *slot = comparator
                .compare_data(sa, sb, (Some(a.1), Some(b.1)), (Some(column), Some(column)))
                .importance(Layers::ValuesAndFormulas, false);
        }
    }

    fn classify(&mut self, rules: &TableRules, options: &TableMergeOptions) {
        let n = self.columns;
        let ignore = options.ignore_unimportant;
        let favored = match options.favor {
            Favor::None => None,
            Favor::Left => Some(Side::Left),
            Favor::Right => Some(Side::Right),
        };
        let mut cells = vec![CellChoice::Center; self.rows.len() * n];
        let mut importance = vec![Importance::Same; self.rows.len()];
        let (mut lc, mut cr, mut lr) = (
            vec![Importance::Same; n],
            vec![Importance::Same; n],
            vec![Importance::Same; n],
        );
        let rows = std::mem::take(&mut self.rows);
        let mut out_rows = Vec::with_capacity(rows.len());
        for (index, mut row) in rows.into_iter().enumerate() {
            let cells = &mut cells[index * n..(index + 1) * n];
            let changed = |raw: &[Importance]| raw.iter().any(|i| differs(*i, ignore));
            let max = |raw: &[Importance]| raw.iter().copied().max().unwrap_or(Importance::Same);
            row.kind = match (row.left, row.center, row.right) {
                (Some(l), Some(c), Some(r)) => {
                    self.compare_rows(rules, (Side::Left, l), (Side::Center, c), &mut lc);
                    self.compare_rows(rules, (Side::Center, c), (Side::Right, r), &mut cr);
                    self.compare_rows(rules, (Side::Left, l), (Side::Right, r), &mut lr);
                    for column in 0..n {
                        let raw = [lc[column], cr[column], lr[column]];
                        cells[column] = choice_of(raw, ignore);
                        if let (true, Some(side)) = (ignore, favored) {
                            if choice_of(raw, false) == CellChoice::Conflict
                                && cells[column] != CellChoice::Conflict
                            {
                                self.favored.insert(index * n + column, side);
                            }
                        }
                    }
                    importance[index] = max(&lc).max(max(&cr)).max(max(&lr));
                    RowKind::Normal
                }
                (Some(l), None, Some(r)) => {
                    self.compare_rows(rules, (Side::Left, l), (Side::Right, r), &mut lr);
                    importance[index] = max(&lr);
                    let conflict = changed(&lr);
                    for column in 0..n {
                        cells[column] = if differs(lr[column], ignore) {
                            CellChoice::Conflict
                        } else {
                            CellChoice::Both
                        };
                    }
                    if !conflict && changed_raw(&lr) {
                        if let Some(side) = favored {
                            self.favored_rows.insert(index, side);
                        }
                    }
                    if conflict {
                        RowKind::AddConflict
                    } else {
                        RowKind::BothAdded
                    }
                }
                (Some(_), None, None) => {
                    cells.fill(CellChoice::Left);
                    RowKind::LeftAdded
                }
                (None, None, Some(_)) => {
                    cells.fill(CellChoice::Right);
                    RowKind::RightAdded
                }
                (None, Some(c), Some(r)) => {
                    self.compare_rows(rules, (Side::Center, c), (Side::Right, r), &mut cr);
                    importance[index] = Importance::Important;
                    for column in 0..n {
                        cells[column] = if differs(cr[column], ignore) {
                            CellChoice::Right
                        } else {
                            CellChoice::Center
                        };
                    }
                    if changed(&cr) {
                        RowKind::DeleteConflict
                    } else {
                        RowKind::Deleted
                    }
                }
                (Some(l), Some(c), None) => {
                    self.compare_rows(rules, (Side::Left, l), (Side::Center, c), &mut lc);
                    importance[index] = Importance::Important;
                    for column in 0..n {
                        cells[column] = if differs(lc[column], ignore) {
                            CellChoice::Left
                        } else {
                            CellChoice::Center
                        };
                    }
                    if changed(&lc) {
                        RowKind::DeleteConflict
                    } else {
                        RowKind::Deleted
                    }
                }
                (None, Some(_), None) => RowKind::Deleted,
                (None, None, None) => unreachable!("join never yields empty rows"),
            };
            out_rows.push(row);
        }
        self.rows = out_rows;
        self.cells = cells;
        self.importance = importance;
    }

    /// Sheet-level structure: a sheet deleted on one side.
    fn sheet_presence(&mut self) {
        let [l, c, r] = self.present;
        if !c {
            return;
        }
        let deleted_by = match (l, r) {
            (false, false) => {
                self.dropped = true;
                return;
            }
            (false, true) => Side::Left,
            (true, false) => Side::Right,
            (true, true) => return,
        };
        // The other side changed the sheet when it modified, added or deleted a row.
        let kept = if deleted_by == Side::Left {
            Side::Right
        } else {
            Side::Left
        };
        let changed = self.rows.iter().any(|row| match row.kind {
            RowKind::DeleteConflict | RowKind::LeftAdded | RowKind::RightAdded => true,
            RowKind::Deleted => row.index(kept).is_none(),
            _ => false,
        });
        if changed {
            self.sheet_deleted_by = Some(deleted_by);
        } else {
            self.dropped = true;
        }
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Cell type (meaningful for the row kinds that carry cells).
    pub fn choice(&self, row: usize, column: usize) -> CellChoice {
        self.cells
            .get(row * self.columns + column)
            .copied()
            .unwrap_or_default()
    }

    /// The row has a conflict (resolved or not).
    pub fn row_conflicted(&self, row: usize) -> bool {
        let kind = self.rows[row].kind;
        kind.is_structural_conflict()
            || (kind == RowKind::Normal
                && (0..self.columns).any(|c| self.choice(row, c) == CellChoice::Conflict))
    }

    /// The cell is a conflict nobody decided yet.
    pub fn cell_unresolved(&self, row: usize, column: usize) -> bool {
        let index = row * self.columns + column;
        self.rows[row].kind == RowKind::Normal
            && self.choice(row, column) == CellChoice::Conflict
            && !self.resolution.accepted.contains(&row)
            && !self.resolution.cells.contains_key(&index)
            && !self.resolution.edits.contains_key(&index)
    }

    /// The row still has an undecided conflict.
    pub fn row_unresolved(&self, row: usize) -> bool {
        if self.sheet_conflict_unresolved() {
            return true;
        }
        if self.resolution.accepted.contains(&row) {
            return false;
        }
        match self.rows[row].kind {
            RowKind::AddConflict | RowKind::DeleteConflict => {
                !self.resolution.rows.contains_key(&row)
            }
            RowKind::Normal => (0..self.columns).any(|c| self.cell_unresolved(row, c)),
            _ => false,
        }
    }

    fn sheet_conflict_unresolved(&self) -> bool {
        self.sheet_deleted_by.is_some() && self.resolution.sheet.is_none()
    }

    /// Side that deleted the sheet while the other changed it.
    pub fn sheet_deleted_by(&self) -> Option<Side> {
        self.sheet_deleted_by
    }

    /// The sheet is left out of the output.
    pub fn is_dropped(&self) -> bool {
        self.dropped
            || match (self.sheet_deleted_by, self.resolution.sheet) {
                (Some(by), Some(side)) => by == side,
                _ => false,
            }
    }

    /// Every conflict section (maximal run of conflicted rows) and whether it is still
    /// unresolved.
    pub fn sections(&self) -> Vec<(Range<usize>, bool)> {
        let mut out = Vec::new();
        let mut row = 0;
        while row < self.rows.len() {
            if !self.row_conflicted(row) {
                row += 1;
                continue;
            }
            let start = row;
            let mut unresolved = false;
            while row < self.rows.len() && self.row_conflicted(row) {
                unresolved |= self.row_unresolved(row);
                row += 1;
            }
            out.push((start..row, unresolved));
        }
        out
    }

    /// Unresolved conflict sections. A sheet-level conflict is one section over all rows.
    pub fn conflicts(&self) -> Vec<Conflict> {
        if self.sheet_conflict_unresolved() {
            return vec![Conflict {
                rows: 0..self.rows.len(),
                importance: Importance::Important,
                sheet: true,
            }];
        }
        self.sections()
            .into_iter()
            .filter(|(_, unresolved)| *unresolved)
            .map(|(rows, _)| Conflict {
                importance: self.importance[rows.clone()]
                    .iter()
                    .copied()
                    .max()
                    .unwrap_or(Importance::Same),
                rows,
                sheet: false,
            })
            .collect()
    }

    /// Conflict section containing `row`, if any.
    pub fn section_at(&self, row: usize) -> Option<Range<usize>> {
        self.sections()
            .into_iter()
            .map(|(rows, _)| rows)
            .find(|rows| rows.contains(&row))
    }

    /// Side whose row the output row is based on; `None` = no output row.
    pub fn row_base(&self, row: usize) -> Option<Side> {
        if let Some(side) = self.sheet_verbatim() {
            return self.rows[row].index(side).map(|_| side);
        }
        let merge_row = &self.rows[row];
        if let Some(&side) = self.resolution.rows.get(&row) {
            return merge_row.index(side).map(|_| side);
        }
        match merge_row.kind {
            RowKind::Normal | RowKind::DeleteConflict => Some(Side::Center),
            RowKind::LeftAdded => Some(Side::Left),
            RowKind::RightAdded => Some(Side::Right),
            RowKind::BothAdded => Some(self.favored_rows.get(&row).copied().unwrap_or(Side::Left)),
            RowKind::AddConflict | RowKind::Deleted => None,
        }
    }

    /// Sheet output copied whole from one side (sheet-level conflict).
    fn sheet_verbatim(&self) -> Option<Side> {
        self.sheet_deleted_by
            .map(|_| self.resolution.sheet.unwrap_or(Side::Center))
    }

    /// Where the output cell comes from; `None` when the row is not in the output.
    pub fn cell_source(&self, row: usize, column: usize) -> Option<CellSource> {
        let base = self.row_base(row)?;
        if self.sheet_verbatim().is_some() {
            return Some(CellSource::of(base));
        }
        let index = row * self.columns + column;
        if self.resolution.edits.contains_key(&index) {
            return Some(CellSource::Edited);
        }
        let merge_row = &self.rows[row];
        if let Some(&side) = self.resolution.cells.get(&index) {
            if merge_row.index(side).is_some() {
                return Some(CellSource::of(side));
            }
        }
        if merge_row.kind != RowKind::Normal || base != Side::Center {
            return Some(CellSource::of(base));
        }
        if let Some(&side) = self.favored.get(&index) {
            return Some(CellSource::of(side));
        }
        Some(match self.choice(row, column) {
            CellChoice::Center | CellChoice::Conflict => CellSource::Center,
            CellChoice::Left | CellChoice::Both => CellSource::Left,
            CellChoice::Right => CellSource::Right,
        })
    }

    /// Input cell of `side` on aligned row `row` (`None` on a gap).
    pub fn input_cell(&self, side: Side, row: usize, column: usize) -> Option<&Cell> {
        let index = self.rows[row].index(side)?;
        self.sheet(side).cell(index, column)
    }

    /// Display text of an input cell (keeps the source spelling of text tables).
    pub fn input_text(&self, side: Side, row: usize, column: usize) -> Option<String> {
        let index = self.rows[row].index(side)?;
        self.sheet(side).display_at(index, column)
    }

    /// Output cell and its display text; `None` when the row is not in the output.
    pub fn output_cell(&self, row: usize, column: usize) -> Option<(Cell, String, CellSource)> {
        let source = self.cell_source(row, column)?;
        Some(match source {
            CellSource::Edited => {
                let cell = self.resolution.edits[&(row * self.columns + column)].clone();
                let text = cell.display();
                (cell, text, source)
            }
            _ => {
                let side = match source {
                    CellSource::Left => Side::Left,
                    CellSource::Right => Side::Right,
                    _ => Side::Center,
                };
                let cell = self
                    .input_cell(side, row, column)
                    .cloned()
                    .unwrap_or_default();
                let text = self.input_text(side, row, column).unwrap_or_default();
                (cell, text, source)
            }
        })
    }

    /// Take `side` for the conflicts of `rows`: structural rows take that side's row,
    /// other rows take it on their conflict cells. A sheet-level conflict section takes
    /// the side's whole sheet.
    pub fn take(&mut self, rows: Range<usize>, side: Side) {
        if self.sheet_deleted_by.is_some() {
            self.resolution.sheet = Some(side);
            return;
        }
        for row in rows {
            self.take_row(row, side);
        }
    }

    fn take_row(&mut self, row: usize, side: Side) {
        match self.rows[row].kind {
            RowKind::AddConflict | RowKind::DeleteConflict => {
                self.resolution.rows.insert(row, side);
            }
            RowKind::Normal => {
                for column in 0..self.columns {
                    if self.choice(row, column) == CellChoice::Conflict {
                        let index = row * self.columns + column;
                        self.resolution.edits.remove(&index);
                        self.resolution.cells.insert(index, side);
                    }
                }
            }
            _ => {}
        }
    }

    /// Take `side` for one cell (a conflict cell, or any cell of a present row).
    pub fn take_cell(&mut self, row: usize, column: usize, side: Side) {
        if self.sheet_deleted_by.is_some() {
            self.resolution.sheet = Some(side);
            return;
        }
        if self.rows[row].kind.is_structural_conflict() && !self.resolution.rows.contains_key(&row)
        {
            self.take_row(row, side);
            return;
        }
        let index = row * self.columns + column;
        self.resolution.edits.remove(&index);
        self.resolution.cells.insert(index, side);
    }

    /// Mark the conflicts of `rows` resolved without changing the output.
    pub fn accept(&mut self, rows: Range<usize>) {
        if self.sheet_deleted_by.is_some() {
            self.resolution.sheet = Some(Side::Center);
            return;
        }
        for row in rows {
            if self.row_conflicted(row) {
                self.resolution.accepted.insert(row);
            }
        }
    }

    /// Type a value into an output cell; resolves a conflict there. `false` when the row
    /// is not in the output.
    pub fn edit(&mut self, row: usize, column: usize, cell: Cell) -> bool {
        if self.row_base(row).is_none() || column >= self.columns || self.sheet_verbatim().is_some()
        {
            return false;
        }
        self.resolution
            .edits
            .insert(row * self.columns + column, cell);
        true
    }

    /// Number of unresolved conflict sections.
    pub fn unresolved(&self) -> usize {
        self.conflicts().len()
    }

    /// The merged sheet, or `None` when the sheet is dropped. Text tables keep the base
    /// side's records: unchanged fields, delimiters and line endings stay as they were.
    pub fn output_sheet(&self) -> Option<Sheet> {
        if self.is_dropped() {
            return None;
        }
        if let Some(side) = self.sheet_verbatim() {
            return Some(self.sheet(side).clone());
        }
        let base = if self.present[1] {
            Side::Center
        } else if self.present[0] {
            Side::Left
        } else {
            Side::Right
        };
        let base_sheet = self.sheet(base);
        let mut out = Sheet::new(if base_sheet.name.is_empty() {
            self.name.clone()
        } else {
            base_sheet.name.clone()
        });
        out.regional = base_sheet.regional.clone();
        out.source = match base_sheet.source {
            SheetSource::Workbook => SheetSource::Workbook,
            SheetSource::Text => SheetSource::Text,
            SheetSource::Memory => SheetSource::Memory,
        };
        out.headers = self.headers.clone();
        let base_text = base_sheet.text_source.as_ref();
        let same_layout = |side: Side| match (base_text, self.sheet(side).text_source.as_ref()) {
            (Some(a), Some(b)) => {
                a.options.delimiters.first() == b.options.delimiters.first()
                    && a.options.qualifier == b.options.qualifier
            }
            _ => false,
        };
        let mut raw_rows = Vec::new();
        let mut changed = Vec::new();
        let mut overrides = Vec::new();
        for row in 0..self.rows.len() {
            let Some(row_side) = self.row_base(row) else {
                continue;
            };
            let out_row = out.row_count();
            let mut cells = Vec::with_capacity(self.columns);
            let mut texts = Vec::with_capacity(self.columns);
            let mut sources = Vec::with_capacity(self.columns);
            for column in 0..self.columns {
                let (mut cell, text, source) =
                    self.output_cell(row, column).expect("row is in the output");
                let from = match source {
                    CellSource::Left => Some(Side::Left),
                    CellSource::Right => Some(Side::Right),
                    CellSource::Center => Some(Side::Center),
                    CellSource::Edited => None,
                };
                if let (Some(style), Some(side)) = (cell.style, from) {
                    cell.style = self
                        .sheet(side)
                        .style(Some(style))
                        .cloned()
                        .map(|s| out.intern_style(s));
                }
                cells.push(cell);
                texts.push(text);
                sources.push(from);
            }
            out.push_row(cells);
            if base_text.is_none() {
                continue;
            }
            let raw = self.rows[row]
                .index(row_side)
                .filter(|_| same_layout(row_side))
                .and_then(|index| {
                    self.sheet(row_side)
                        .text_source
                        .as_ref()
                        .and_then(|t| t.rows.get(index))
                        .cloned()
                });
            match raw {
                Some(raw) => {
                    for (column, from) in sources.iter().enumerate() {
                        if *from != Some(row_side) {
                            changed.push((out_row, column));
                            overrides.push(((out_row, column), texts[column].clone()));
                        }
                    }
                    raw_rows.push(raw);
                }
                None => {
                    for (column, text) in texts.into_iter().enumerate() {
                        overrides.push(((out_row, column), text));
                    }
                    raw_rows.push(RawRow::default());
                }
            }
        }
        if let Some(source) = base_text {
            let header = source
                .header
                .clone()
                .filter(|_| self.headers == base_sheet.headers);
            out.text_source = Some(TextSource {
                options: source.options.clone(),
                encoding: source.encoding,
                has_bom: source.has_bom,
                header,
                rows: Arc::new(raw_rows),
                default_ending: source.default_ending.clone(),
            });
            out.changed.extend(changed);
            for (at, text) in overrides {
                out.display_overrides.insert(at, text.into());
            }
        }
        out.fresh_revision();
        Some(out)
    }
}

fn changed_raw(raw: &[Importance]) -> bool {
    raw.iter().any(|i| *i != Importance::Same)
}

/// The inputs of a merge: named sheets per side.
#[derive(Debug, Clone, Default)]
pub struct MergeInputs {
    pub left: Vec<Sheet>,
    pub center: Vec<Sheet>,
    pub right: Vec<Sheet>,
}

/// Merge of a whole table file (one sheet for text tables, every sheet of a workbook).
#[derive(Debug, Clone)]
pub struct TableMerge {
    pub sheets: Vec<SheetMerge>,
    pub warnings: Vec<String>,
}

/// Outcome of an automatic merge.
#[derive(Debug, Clone)]
pub struct AutoTableMerge {
    /// Unresolved conflict sections per sheet name.
    pub conflicts: Vec<(String, Conflict)>,
    /// Output sheets; empty when withheld.
    pub sheets: Vec<Sheet>,
    /// Conflicts remain: nothing to write.
    pub withheld: bool,
}

impl TableMerge {
    /// Pair sheets by name (a single sheet per side pairs regardless of name) and merge
    /// each pair. Output order: the center's sheets, then sheets new on the left, then
    /// sheets new on the right.
    pub fn new(inputs: MergeInputs, options: &TableMergeOptions) -> Result<Self> {
        let MergeInputs {
            left,
            center,
            right,
        } = inputs;
        let single = left.len() <= 1 && center.len() <= 1 && right.len() <= 1;
        let mut names: Vec<String> = Vec::new();
        for sheet in center.iter().chain(&left).chain(&right) {
            if !names.contains(&sheet.name) {
                names.push(sheet.name.clone());
            }
        }
        if single {
            names.truncate(1);
        }
        let take = |sheets: &[Sheet], name: &str| -> Option<Sheet> {
            if single {
                sheets.first().cloned()
            } else {
                sheets.iter().find(|s| s.name == name).cloned()
            }
        };
        let mut sheets = Vec::with_capacity(names.len());
        for name in names {
            let merge = SheetMerge::new(
                name.clone(),
                [
                    take(&left, &name),
                    take(&center, &name),
                    take(&right, &name),
                ],
                options,
            )
            .map_err(|e| invalid(format!("sheet {name:?}: {e}")))?;
            sheets.push(merge);
        }
        let mut warnings = Vec::new();
        for sheet in &sheets {
            let [l, c, r] = sheet.duplicates;
            if l + c + r > 0 {
                warnings.push(format!(
                    "sheet {:?}: duplicate keys (left {l}, center {c}, right {r}) pair by order",
                    sheet.name
                ));
            }
        }
        Ok(Self { sheets, warnings })
    }

    pub fn unresolved(&self) -> usize {
        self.sheets.iter().map(SheetMerge::unresolved).sum()
    }

    /// Output sheets (dropped sheets left out).
    pub fn output(&self) -> Vec<Sheet> {
        self.sheets
            .iter()
            .filter_map(SheetMerge::output_sheet)
            .collect()
    }

    /// Key columns per sheet, for result lines (`Sheet1: 1 (inferred)`).
    pub fn keys_label(&self) -> String {
        if let [sheet] = self.sheets.as_slice() {
            return sheet.keys.label();
        }
        self.sheets
            .iter()
            .map(|s| format!("{}: {}", s.name, s.keys.label()))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// Automatic merge: conflicts are listed and the output withheld while any remain
/// (a table cell cannot carry conflict markers).
pub fn auto_merge(merge: &TableMerge) -> AutoTableMerge {
    let conflicts: Vec<(String, Conflict)> = merge
        .sheets
        .iter()
        .flat_map(|s| s.conflicts().into_iter().map(|c| (s.name.clone(), c)))
        .collect();
    let withheld = !conflicts.is_empty();
    AutoTableMerge {
        sheets: if withheld { Vec::new() } else { merge.output() },
        conflicts,
        withheld,
    }
}

#[cfg(test)]
#[path = "merge_tests.rs"]
mod tests;
