use crate::{
    align::{DuplicateKeys, KeyAlign, KeyAlignment},
    columns::{align_columns, ColumnMapping, Side},
    compare::{CellComparator, CellDiff, Importance, PackedStates},
    invalid,
    rules::{ColumnHandling, ColumnMode, KeySelection, Layers},
    store::SheetStore,
    Cell, Result, Sheet, TableRules,
};
use bc_core::{
    align::{AlignRow, AlignmentOverrides},
    cancel::{CancelCheck, NeverCancel},
    edit::EditItem,
    DiffKind,
};
use std::{
    cell::{Cell as Parity, Ref, RefCell, RefMut},
    rc::Rc,
    sync::Arc,
};

/// Live edit target per side. The name predates the storage bridge: it now
/// holds the `SheetStore` enum (dense default / experimental demand-backed).
pub type SharedSheet = Rc<RefCell<SheetStore>>;
const GAP: u32 = u32::MAX;
#[derive(Debug, Clone, Copy)]
pub(crate) struct CompactRow {
    left: u32,
    right: u32,
    aggregate: u8,
    edited: bool,
}
impl CompactRow {
    fn left(self) -> Option<usize> {
        (self.left != GAP).then_some(self.left as usize)
    }
    fn right(self) -> Option<usize> {
        (self.right != GAP).then_some(self.right as usize)
    }
}
#[derive(Debug)]
pub struct ComparedData {
    pub(crate) rows: Vec<CompactRow>,
    pub(crate) states: PackedStates,
    pub(crate) columns: usize,
    column_states: Vec<CellDiff>,
    summary: CellDiff,
    pub duplicates: DuplicateKeys,
}
impl ComparedData {
    pub fn row(&self, index: usize, layers: Layers, ignore: bool) -> Option<TableMapRow> {
        let row = *self.rows.get(index)?;
        let importance = CellDiff::unpack(row.aggregate).importance(layers, ignore);
        let kind = match (row.left(), row.right()) {
            (Some(_), Some(_)) => {
                if importance == Importance::Same {
                    DiffKind::Same
                } else {
                    DiffKind::Changed
                }
            }
            (Some(_), None) => DiffKind::LeftOnly,
            _ => DiffKind::RightOnly,
        };
        Some(TableMapRow {
            left: row.left(),
            right: row.right(),
            kind,
            importance,
            edited: row.edited,
        })
    }
    pub fn cell_diff(&self, row: usize, col: usize) -> Option<CellDiff> {
        if col >= self.columns || row >= self.rows.len() {
            return None;
        }
        self.states.get(row * self.columns + col)
    }
    pub fn column_diff(&self, col: usize) -> Option<CellDiff> {
        self.column_states.get(col).copied()
    }
    pub fn column_count(&self) -> usize {
        self.columns
    }
    pub fn importance(&self, layers: Layers, ignore: bool) -> Importance {
        self.summary.importance(layers, ignore)
    }
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }
    pub fn state_bytes(&self) -> usize {
        self.states.len_bytes()
    }
    pub fn compact_row_bytes(&self) -> usize {
        self.rows.len() * std::mem::size_of::<CompactRow>()
    }
    /// Worker-safe counts; structural annotations are metadata supplied by TableMap.
    pub fn statistics(&self, layers: Layers, ignore_unimportant: bool) -> TableStatistics {
        let mut stats = TableStatistics {
            columns: self.columns,
            ..Default::default()
        };
        for row in &self.rows {
            match (row.left(), row.right()) {
                (Some(_), None) => stats.left_only += 1,
                (None, Some(_)) => stats.right_only += 1,
                _ => match CellDiff::unpack(row.aggregate).importance(layers, ignore_unimportant) {
                    Importance::Same => stats.same += 1,
                    Importance::Unimportant => stats.unimportant += 1,
                    Importance::Important => stats.important += 1,
                },
            }
        }
        stats
    }
}
/// One compare chunk's output: packed per-cell states and per-row aggregates
/// in aligned-row order, plus the chunk-local column/summary folds.
/// `CellDiff::merge` is per-field max (associative and commutative), so the
/// sequential assembly reproduces the historical single-loop totals exactly.
#[derive(Debug)]
struct CompareChunk {
    /// `CellDiff::packed()` per compared cell, row-major over the chunk.
    cells: Vec<u8>,
    /// Per-row packed aggregate (after the structural Important override).
    aggregates: Vec<u8>,
    column_states: Vec<CellDiff>,
    summary: CellDiff,
}
/// Compare one contiguous range of aligned rows (cancel cadence no coarser
/// than the historical 1024-row step). Both sheets, the rules and the visit
/// orders are read-only shares.
fn compare_rows_chunk<S: crate::data::TableData>(
    rows: &[AlignRow],
    left: &S,
    right: &S,
    orders: (&[usize], &[usize]),
    rules: &TableRules,
    handling: &[ColumnHandling],
    cancel: &dyn CancelCheck,
) -> Result<CompareChunk> {
    let (left_order, right_order) = orders;
    let columns = handling.len();
    let mut chunk = CompareChunk {
        cells: Vec::with_capacity(rows.len() * columns),
        aggregates: Vec::with_capacity(rows.len()),
        column_states: vec![CellDiff::default(); columns],
        summary: CellDiff::default(),
    };
    for (i, row) in rows.iter().enumerate() {
        if i.is_multiple_of(1024) {
            cancel.check()?;
        }
        let l = row.left.map(|r| left_order[r]);
        let r = row.right.map(|r| right_order[r]);
        let mut aggregate = CellDiff::default();
        for (c, handling) in handling.iter().enumerate() {
            let col = &rules.columns.0[c];
            let diff = CellComparator {
                handling,
                format_important: rules.format_important,
            }
            .compare_data(left, right, (l, r), (col.left, col.right));
            aggregate = aggregate.merge(diff);
            chunk.column_states[c] = chunk.column_states[c].merge(diff);
            chunk.cells.push(diff.packed());
        }
        if l.is_none() || r.is_none() {
            aggregate.value = Importance::Important;
        }
        chunk.summary = chunk.summary.merge(aggregate);
        chunk.aggregates.push(aggregate.packed());
    }
    Ok(chunk)
}
#[derive(Debug, Clone)]
pub struct TableAlignOutput {
    pub data: Arc<ComparedData>,
    pub revisions: (u64, u64),
    pub rules: TableRules,
    pub overrides: AlignmentOverrides,
}
/// Public split phase used by the benchmark: does not repeat alignment.
/// Compare on materialized sheets (original public entry; behavior unchanged).
pub fn compare_alignment(
    left: &Sheet,
    right: &Sheet,
    rules: &TableRules,
    overrides: &AlignmentOverrides,
    alignment: KeyAlignment,
    cancel: &dyn CancelCheck,
) -> Result<TableAlignOutput> {
    compare_alignment_data(left, right, rules, overrides, alignment, cancel)
}
/// Generic read-only entry over `TableData` (prototype/demand storage support).
pub fn compare_alignment_data<S: crate::data::TableData + Sync>(
    left: &S,
    right: &S,
    rules: &TableRules,
    overrides: &AlignmentOverrides,
    alignment: KeyAlignment,
    cancel: &dyn CancelCheck,
) -> Result<TableAlignOutput> {
    cancel.check()?;
    if left.row_count() >= GAP as usize || right.row_count() >= GAP as usize {
        return Err(invalid("table exceeds compact row index limit"));
    }
    let columns = rules.columns.0.len();
    let cells = alignment
        .map
        .len()
        .checked_mul(columns)
        .filter(|n| n.checked_mul(6).is_some())
        .ok_or_else(|| invalid("cell state size overflow"))?;
    let mut states = PackedStates::new(cells);
    let mut column_states = vec![CellDiff::default(); columns];
    let mut summary = CellDiff::default();
    let mut rows = Vec::with_capacity(alignment.map.len());
    let handling: Vec<_> = (0..columns).map(|c| rules.handling(c)).collect();
    let mut le = std::collections::HashSet::new();
    left.collect_edited_rows(&mut le);
    let mut re = std::collections::HashSet::new();
    right.collect_edited_rows(&mut re);
    let le = le;
    let re = re;
    let map_rows = alignment.map.into_rows();
    // Compare each aligned-row range on scoped workers; staging is position-
    // indexed packed bytes, so results never depend on scheduling.
    let threads = crate::perf::phase_threads(map_rows.len());
    let chunks = if threads < 2 {
        vec![compare_rows_chunk(
            &map_rows,
            left,
            right,
            (&alignment.left_order, &alignment.right_order),
            rules,
            &handling,
            cancel,
        )?]
    } else {
        let chunk = map_rows.len().div_ceil(threads).max(1);
        let results = std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for slice in map_rows.chunks(chunk) {
                handles.push(scope.spawn(|| {
                    compare_rows_chunk(
                        slice,
                        left,
                        right,
                        (&alignment.left_order, &alignment.right_order),
                        rules,
                        &handling,
                        cancel,
                    )
                }));
            }
            handles
                .into_iter()
                .map(|h| h.join().unwrap_or_else(|_| Err(invalid("compare panicked"))))
                .collect::<Vec<_>>()
        });
        let mut chunks = Vec::with_capacity(results.len());
        for result in results {
            chunks.push(result?);
        }
        chunks
    };
    // Sequential assembly in aligned-row order: PackedStates/rows stay
    // position-exact; column_states/summary folds are per-field max
    // (associative and commutative), so chunk order cannot change them.
    let mut base_row = 0usize;
    for chunk in chunks {
        for (c, state) in chunk.column_states.iter().enumerate() {
            column_states[c] = column_states[c].merge(*state);
        }
        summary = summary.merge(chunk.summary);
        for (r, &aggregate) in chunk.aggregates.iter().enumerate() {
            let row = map_rows[base_row + r];
            let l = row.left.map(|i| alignment.left_order[i]);
            let rr = row.right.map(|i| alignment.right_order[i]);
            let first = (base_row + r) * columns;
            for (c, &packed) in chunk.cells[r * columns..(r + 1) * columns].iter().enumerate() {
                states.set(first + c, CellDiff::unpack(packed));
            }
            rows.push(CompactRow {
                left: l.map_or(GAP, |r| r as u32),
                right: rr.map_or(GAP, |r| r as u32),
                aggregate,
                edited: l.is_some_and(|r| le.contains(&r))
                    || rr.is_some_and(|r| re.contains(&r)),
            });
        }
        base_row += chunk.aggregates.len();
    }
    cancel.check()?;
    Ok(TableAlignOutput {
        data: Arc::new(ComparedData {
            rows,
            states,
            columns,
            column_states,
            summary,
            duplicates: alignment.duplicates,
        }),
        revisions: (left.revision(), right.revision()),
        rules: rules.clone(),
        overrides: overrides.clone(),
    })
}
#[derive(Debug, Clone, Default)]
pub(crate) struct Settings {
    pub rules: TableRules,
    pub overrides: AlignmentOverrides,
    /// Bumped by replace_sheet: edits captured before a side replacement must
    /// not resurrect anchors that addressed the replaced content.
    pub anchors_generation: u64,
}
fn transpose_overrides(overrides: &AlignmentOverrides) -> AlignmentOverrides {
    let mut out = AlignmentOverrides::default();
    for &(left, right) in overrides.pairs() {
        let _ = out.insert(right, left);
    }
    out
}
impl Settings {
    fn transpose(&self) -> Self {
        let mut rules = self.rules.clone();
        rules.transpose();
        Self {
            rules,
            overrides: transpose_overrides(&self.overrides),
            anchors_generation: self.anchors_generation,
        }
    }
}
impl TableAlignOutput {
    /// Mirror image of this comparison: rows, revisions, rules and anchors
    /// exchange sides; per-column cell states are keyed by comparison column
    /// and stay put.
    fn transpose(&self) -> Self {
        let data = &self.data;
        let mut rules = self.rules.clone();
        rules.transpose();
        Self {
            data: Arc::new(ComparedData {
                rows: data
                    .rows
                    .iter()
                    .map(|row| CompactRow {
                        left: row.right,
                        right: row.left,
                        ..*row
                    })
                    .collect(),
                states: data.states.clone(),
                columns: data.columns,
                column_states: data.column_states.clone(),
                summary: data.summary,
                duplicates: DuplicateKeys {
                    left: data.duplicates.right,
                    right: data.duplicates.left,
                },
            }),
            revisions: (self.revisions.1, self.revisions.0),
            rules,
            overrides: transpose_overrides(&self.overrides),
        }
    }
}
pub(crate) fn same_comparison(a: &TableRules, b: &TableRules) -> bool {
    let mut a = a.clone();
    let mut b = b.clone();
    a.ignore_unimportant = false;
    b.ignore_unimportant = false;
    a.layers = Layers::Values;
    b.layers = Layers::Values;
    a == b
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableMapRow {
    pub left: Option<usize>,
    pub right: Option<usize>,
    pub kind: DiffKind,
    pub importance: Importance,
    pub edited: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TableStatistics {
    pub same: usize,
    pub unimportant: usize,
    pub important: usize,
    pub left_only: usize,
    pub right_only: usize,
    pub columns: usize,
    pub structural_columns: usize,
}
impl TableStatistics {
    pub fn importance(self) -> Importance {
        if self.important + self.left_only + self.right_only > 0 {
            Importance::Important
        } else if self.unimportant > 0 {
            Importance::Unimportant
        } else {
            Importance::Same
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShowRows {
    All,
    Differences,
    Same,
    None,
}
#[derive(Debug)]
pub struct TableMap {
    pub left: SharedSheet,
    pub right: SharedSheet,
    pub(crate) settings: Rc<RefCell<Settings>>,
    pub(crate) state: Rc<RefCell<TableAlignOutput>>,
    deferred_edits: bool,
    /// Swap Sides parity shared with captured edits: `false` = as compared.
    pub(crate) orientation: Rc<Parity<bool>>,
}
pub(crate) fn prepare_rules(
    left: &Sheet,
    right: &Sheet,
    rules: &mut TableRules,
    cancel: &dyn CancelCheck,
) -> Result<()> {
    cancel.check()?;
    if rules.columns.0.is_empty() {
        rules.columns = if rules.column_mode == ColumnMode::Automatic
            && !matches!(rules.keys, KeySelection::ByColumnNumber)
        {
            align_columns(left, right, rules.left_is_source, cancel)?
        } else {
            ColumnMapping::by_number(left.column_count(), right.column_count())
        };
    }
    if matches!(&rules.keys,KeySelection::Automatic{columns,..} if columns.is_empty()) {
        rules.keys = crate::inference::infer_keys(left, right, rules, cancel)?.selection;
    }
    rules.validate(left.column_count(), right.column_count())
}
/// Generic read-only variant over `TableData` (store-bridged worker path);
/// identical steps to `prepare_rules`.
pub(crate) fn prepare_rules_data<S: crate::data::TableData + Sync>(
    left: &S,
    right: &S,
    rules: &mut TableRules,
    cancel: &dyn CancelCheck,
) -> Result<()> {
    cancel.check()?;
    if rules.columns.0.is_empty() {
        rules.columns = if rules.column_mode == ColumnMode::Automatic
            && !matches!(rules.keys, KeySelection::ByColumnNumber)
        {
            crate::columns::align_columns_data(left, right, rules.left_is_source, cancel)?
        } else {
            ColumnMapping::by_number(left.column_count(), right.column_count())
        };
    }
    if matches!(&rules.keys,KeySelection::Automatic{columns,..} if columns.is_empty()) {
        rules.keys = crate::inference::infer_keys_data(left, right, rules, cancel)?.selection;
    }
    rules.validate(left.column_count(), right.column_count())
}
impl TableMap {
    pub fn new(
        left: Sheet,
        right: Sheet,
        mut rules: TableRules,
        cancel: &dyn CancelCheck,
    ) -> Result<Self> {
        prepare_rules(&left, &right, &mut rules, cancel)?;
        let overrides = AlignmentOverrides::default();
        let alignment = KeyAlign.align(&left, &right, &rules, &overrides, cancel)?;
        let output = compare_alignment(&left, &right, &rules, &overrides, alignment, cancel)?;
        Self::from_output(left, right, output)
    }
    /// Install an initial worker result without repeating any heavy work on the UI
    /// thread. The live edit targets are created only here, never sent to workers.
    /// Dense-sheet entry preserved for existing callers/tests.
    pub fn from_output(left: Sheet, right: Sheet, output: TableAlignOutput) -> Result<Self> {
        Self::from_stores(
            SheetStore::Dense(left),
            SheetStore::Dense(right),
            output,
        )
    }
    /// Fresh comparison over store-bridged sides (identical chain to `new`,
    /// generic over `TableData`; dense stores take the same code path whose
    /// digest-equality the prototype established).
    pub fn new_stores(
        left: SheetStore,
        right: SheetStore,
        mut rules: TableRules,
        cancel: &dyn CancelCheck,
    ) -> Result<Self> {
        prepare_rules_data(&left, &right, &mut rules, cancel)?;
        let overrides = AlignmentOverrides::default();
        let alignment = KeyAlign.align_data(&left, &right, &rules, &overrides, cancel)?;
        let output = compare_alignment_data(&left, &right, &rules, &overrides, alignment, cancel)?;
        Self::from_stores(left, right, output)
    }
    /// Store-bridged install (dense default / experimental demand-backed sides).
    pub fn from_stores(
        left: SheetStore,
        right: SheetStore,
        output: TableAlignOutput,
    ) -> Result<Self> {
        if output.revisions != (left.revision(), right.revision()) {
            return Err(invalid("stale initial table result"));
        }
        output
            .rules
            .validate(left.column_count(), right.column_count())?;
        let rules = output.rules.clone();
        let overrides = output.overrides.clone();
        Ok(Self {
            left: Rc::new(RefCell::new(left)),
            right: Rc::new(RefCell::new(right)),
            settings: Rc::new(RefCell::new(Settings {
                rules,
                overrides,
                anchors_generation: 0,
            })),
            state: Rc::new(RefCell::new(output)),
            deferred_edits: false,
            orientation: Rc::new(Parity::new(false)),
        })
    }
    /// GUI edit commands never synchronously align. Keep stale state until a
    /// revision-matching TableRealignJob has been installed.
    pub fn set_deferred_edits(&mut self, enabled: bool) {
        self.deferred_edits = enabled;
    }
    pub fn output(&self) -> TableAlignOutput {
        self.state.borrow().clone()
    }
    pub fn edit_sheet(
        &self,
        side: Side,
        description: &str,
        edit: impl FnOnce(&mut SheetStore, &mut TableRules, &mut AlignmentOverrides) -> Result<()>,
    ) -> Result<Box<dyn EditItem>> {
        self.apply_edit(side, description, |sheet, settings| {
            edit(sheet, &mut settings.rules, &mut settings.overrides)
        })
    }
    pub fn rules(&self) -> Ref<'_, TableRules> {
        Ref::map(self.settings.borrow(), |s| &s.rules)
    }
    pub fn rules_mut(&self) -> RefMut<'_, TableRules> {
        RefMut::map(self.settings.borrow_mut(), |s| &mut s.rules)
    }
    pub fn overrides(&self) -> AlignmentOverrides {
        self.settings.borrow().overrides.clone()
    }
    pub fn set_overrides(&self, overrides: AlignmentOverrides) {
        self.settings.borrow_mut().overrides = overrides;
    }
    /// Swap Sides in place: exchange the sheets and mirror rules, anchors and the
    /// installed comparison. The transposed snapshot is exactly the mirrored
    /// comparison, so the map stays aligned; captured edits consult the shared
    /// orientation parity to restore correctly across the swap.
    pub fn swap_sides(&mut self) {
        self.orientation.set(!self.orientation.get());
        std::mem::swap(&mut self.left, &mut self.right);
        let settings = self.settings.borrow().transpose();
        *self.settings.borrow_mut() = settings;
        let state = self.state.borrow().transpose();
        *self.state.borrow_mut() = state;
    }
    /// Dense-sheet replacement entry preserved for existing callers/tests.
    pub fn replace_sheet(&self, side: Side, sheet: Sheet) {
        self.replace_store(side, SheetStore::Dense(sheet));
    }
    /// Replace one side's content in place (Open File/Clipboard on one side).
    /// The other side keeps its sheet object, edit history and captured
    /// snapshots; the new sheet's revision marks the map stale for realignment.
    /// The caller clears the replaced side's own edit stack: its coordinates
    /// described the previous content. The resolved mapping is re-matched
    /// against the new source: an incompatible layout or changed header names
    /// fall back to automatic mapping and key inference instead of silently
    /// comparing the wrong columns. Manual anchors bind row pairs across both
    /// sides, so they are dropped and the anchor generation bumped.
    pub fn replace_store(&self, side: Side, sheet: SheetStore) {
        let target = self.target(side);
        let rematch = {
            let settings = self.settings.borrow();
            let old = target.borrow();
            // ByNumber mapping is positional on purpose; Automatic mapping is
            // header/content-matched and must re-match a changed layout.
            settings.rules.column_mode == ColumnMode::Automatic
                && settings
                    .rules
                    .columns
                    .0
                    .iter()
                    .filter_map(|c| c.index(side))
                    .any(|i| {
                        i >= sheet.column_count() || old.column_name(i) != sheet.column_name(i)
                    })
        };
        *target.borrow_mut() = sheet;
        let (left_columns, right_columns) = (
            self.left.borrow().column_count(),
            self.right.borrow().column_count(),
        );
        let mut settings = self.settings.borrow_mut();
        if rematch
            || settings
                .rules
                .validate(left_columns, right_columns)
                .is_err()
        {
            settings.rules.columns.0.clear();
            settings.rules.keys = crate::rules::KeySelection::Automatic {
                columns: Vec::new(),
                score: 0.0,
            };
        }
        settings.overrides = AlignmentOverrides::default();
        settings.anchors_generation += 1;
    }
    pub fn is_stale(&self) -> bool {
        let state = self.state.borrow();
        let settings = self.settings.borrow();
        state.revisions
            != (
                self.left.borrow().revision(),
                self.right.borrow().revision(),
            )
            || !same_comparison(&state.rules, &settings.rules)
            || state.overrides != settings.overrides
    }
    pub fn set_alignment(&self, output: TableAlignOutput) -> Result<()> {
        // Resolve the request rules into a local candidate and validate
        // everything before publishing: a rejected stale result must leave
        // rules/keys/overrides/state completely untouched. An empty mapping is
        // an unresolved automatic request that adopts the worker-resolved
        // columns/keys exactly as prepare_rules produced them.
        let mut candidate = {
            let settings = self.settings.borrow();
            let mut candidate = settings.rules.clone();
            if candidate.columns.0.is_empty() && !output.rules.columns.0.is_empty() {
                candidate.columns = output.rules.columns.clone();
            }
            if matches!(&candidate.keys, KeySelection::Automatic { columns, .. } if columns.is_empty())
            {
                candidate.keys = output.rules.keys.clone();
            }
            if output.revisions
                != (
                    self.left.borrow().revision(),
                    self.right.borrow().revision(),
                )
                || !same_comparison(&output.rules, &candidate)
                || output.overrides != settings.overrides
            {
                return Err(invalid("stale table alignment result"));
            }
            candidate
        };
        self.settings.borrow_mut().rules = std::mem::take(&mut candidate);
        *self.state.borrow_mut() = output;
        Ok(())
    }
    pub fn recompare(&self, cancel: &dyn CancelCheck) -> Result<()> {
        let output = crate::jobs::TableAlignJob::from_map(self).execute(cancel)?;
        self.set_alignment(output)
    }
    pub fn row_count(&self) -> usize {
        self.state.borrow().data.rows.len()
    }
    pub fn column_count(&self) -> usize {
        self.state.borrow().data.columns
    }
    pub fn row(&self, index: usize) -> Option<TableMapRow> {
        let state = self.state.borrow();
        let row = *state.data.rows.get(index)?;
        let rules = self.rules();
        let importance =
            CellDiff::unpack(row.aggregate).importance(rules.layers, rules.ignore_unimportant);
        let kind = match (row.left(), row.right()) {
            (Some(_), Some(_)) => {
                if importance == Importance::Same {
                    DiffKind::Same
                } else {
                    DiffKind::Changed
                }
            }
            (Some(_), None) => DiffKind::LeftOnly,
            _ => DiffKind::RightOnly,
        };
        Some(TableMapRow {
            left: row.left(),
            right: row.right(),
            kind,
            importance,
            edited: row.edited,
        })
    }
    /// Derived rows; no second expanded map is retained.
    pub fn rows(&self) -> impl Iterator<Item = TableMapRow> + '_ {
        (0..self.row_count()).map(|i| self.row(i).unwrap())
    }
    pub fn cell_diff(&self, row: usize, column: usize) -> Option<CellDiff> {
        let state = self.state.borrow();
        if column >= state.data.columns || row >= state.data.rows.len() {
            return None;
        }
        state.data.states.get(row * state.data.columns + column)
    }
    pub fn cell_importance(&self, row: usize, column: usize) -> Option<Importance> {
        let diff = self.cell_diff(row, column)?;
        let rules = self.rules();
        Some(diff.importance(rules.layers, rules.ignore_unimportant))
    }
    pub fn statistics(&self) -> TableStatistics {
        let rules = self.rules();
        let mut stats = self
            .state
            .borrow()
            .data
            .statistics(rules.layers, rules.ignore_unimportant);
        stats.structural_columns = rules
            .columns
            .0
            .iter()
            .filter(|c| !c.changes.is_empty())
            .count();
        stats
    }
    pub fn duplicate_keys(&self) -> DuplicateKeys {
        self.state.borrow().data.duplicates
    }
    pub fn visible_rows(&self, show: ShowRows) -> Vec<usize> {
        (0..self.row_count())
            .filter(|&i| match show {
                ShowRows::All => true,
                ShowRows::None => false,
                ShowRows::Differences => self.row(i).unwrap().importance != Importance::Same,
                ShowRows::Same => self.row(i).unwrap().importance == Importance::Same,
            })
            .collect()
    }
    pub fn different_columns(&self) -> Vec<usize> {
        (0..self.column_count())
            .filter(|&c| {
                (0..self.row_count()).any(|r| self.cell_importance(r, c) != Some(Importance::Same))
            })
            .collect()
    }
    pub fn next_difference_row(&self, from: Option<usize>) -> Option<usize> {
        (from.map_or(0, |r| r.saturating_add(1))..self.row_count())
            .find(|&r| self.row(r).unwrap().importance != Importance::Same)
    }
    pub fn previous_difference_row(&self, from: Option<usize>) -> Option<usize> {
        (0..from.unwrap_or(self.row_count()).min(self.row_count()))
            .rev()
            .find(|&r| self.row(r).unwrap().importance != Importance::Same)
    }
    pub fn next_difference_cell(&self, from: Option<(usize, usize)>) -> Option<(usize, usize)> {
        let columns = self.column_count();
        let start = from.map_or(0, |(r, c)| {
            r.saturating_mul(columns)
                .saturating_add(c)
                .saturating_add(1)
        });
        (start..self.row_count() * columns)
            .find(|&i| self.cell_importance(i / columns, i % columns) != Some(Importance::Same))
            .map(|i| (i / columns, i % columns))
    }
    pub fn previous_difference_cell(&self, from: Option<(usize, usize)>) -> Option<(usize, usize)> {
        let columns = self.column_count();
        let end = from
            .map_or(self.row_count() * columns, |(r, c)| {
                r.saturating_mul(columns).saturating_add(c)
            })
            .min(self.row_count() * columns);
        (0..end)
            .rev()
            .find(|&i| self.cell_importance(i / columns, i % columns) != Some(Importance::Same))
            .map(|i| (i / columns, i % columns))
    }
    pub fn next_edit(&self, from: Option<usize>) -> Option<usize> {
        (from.map_or(0, |r| r.saturating_add(1))..self.row_count())
            .find(|&r| self.row(r).unwrap().edited)
    }
    pub fn previous_edit(&self, from: Option<usize>) -> Option<usize> {
        (0..from.unwrap_or(self.row_count()).min(self.row_count()))
            .rev()
            .find(|&r| self.row(r).unwrap().edited)
    }
    pub fn goto(&self, row: usize, column: usize) -> Option<(usize, usize)> {
        (row < self.row_count() && column < self.column_count()).then_some((row, column))
    }
    fn target(&self, side: Side) -> SharedSheet {
        match side {
            Side::Left => self.left.clone(),
            Side::Right => self.right.clone(),
        }
    }
    fn apply_edit(
        &self,
        side: Side,
        description: &str,
        edit: impl FnOnce(&mut SheetStore, &mut Settings) -> Result<()>,
    ) -> Result<Box<dyn EditItem>> {
        let before_aligned = !self.is_stale();
        if !before_aligned && !self.deferred_edits {
            return Err(invalid("recompare before editing through table map"));
        }
        let target = self.target(side);
        let before = target.borrow().clone();
        let before_state = self.state.borrow().clone();
        let before_settings = self.settings.borrow().clone();
        let result = edit(&mut target.borrow_mut(), &mut self.settings.borrow_mut());
        let result = result.and_then(|_| {
            if self.deferred_edits {
                Ok(())
            } else {
                self.recompare(&NeverCancel)
            }
        });
        if let Err(error) = result {
            *target.borrow_mut() = before;
            *self.settings.borrow_mut() = before_settings;
            return Err(error);
        }
        let after = target.borrow().clone();
        let after_state = self.state.borrow().clone();
        let after_settings = self.settings.borrow().clone();
        Ok(Box::new(TableEdit {
            description: description.into(),
            target,
            state: self.state.clone(),
            settings: self.settings.clone(),
            orientation: self.orientation.clone(),
            captured_orientation: self.orientation.get(),
            before,
            after,
            before_state,
            after_state,
            before_settings,
            after_settings,
            before_aligned,
            after_aligned: !self.deferred_edits,
            side,
        }))
    }
    /// Already applied; push the returned item onto the destination EditStack.
    pub fn modify_cell(
        &self,
        side: Side,
        row: usize,
        column: usize,
        cell: Cell,
    ) -> Result<Box<dyn EditItem>> {
        self.apply_edit(side, "Modify cell", move |sheet, _| {
            sheet.set_cell(row, column, cell)
        })
    }
    pub fn copy_cell(&self, row: usize, column: usize, from: Side) -> Result<Box<dyn EditItem>> {
        self.copy_cells(&[(row, column)], from)
    }
    pub fn copy_row(&self, row: usize, from: Side) -> Result<Box<dyn EditItem>> {
        self.copy_rows(&[row], from)
    }
}
#[derive(Debug)]
struct TableEdit {
    description: String,
    target: SharedSheet,
    state: Rc<RefCell<TableAlignOutput>>,
    settings: Rc<RefCell<Settings>>,
    orientation: Rc<Parity<bool>>,
    captured_orientation: bool,
    before: SheetStore,
    after: SheetStore,
    before_state: TableAlignOutput,
    after_state: TableAlignOutput,
    before_settings: Settings,
    after_settings: Settings,
    before_aligned: bool,
    after_aligned: bool,
    side: Side,
}
impl TableEdit {
    /// Snapshots were captured in the map's orientation at edit time; a later
    /// Swap Sides flips that orientation, so restoring across it must first
    /// transpose the captured rules/state/anchors into the current orientation.
    fn restore(&self, sheet: &SheetStore, state: &TableAlignOutput, settings: &Settings, aligned: bool) {
        let flipped = self.orientation.get() != self.captured_orientation;
        let side = if flipped {
            self.side.other()
        } else {
            self.side
        };
        let transposed_state;
        let state = if flipped {
            transposed_state = state.transpose();
            &transposed_state
        } else {
            state
        };
        let transposed_settings;
        let settings = if flipped {
            transposed_settings = settings.transpose();
            &transposed_settings
        } else {
            settings
        };
        let mut sheet = sheet.clone();
        sheet.fresh_revision();
        let revision = if aligned { sheet.revision() } else { 0 };
        *self.target.borrow_mut() = sheet;
        let mut state = state.clone();
        match side {
            Side::Left => state.revisions.0 = revision,
            Side::Right => state.revisions.1 = revision,
        }
        *self.state.borrow_mut() = state;
        let (projection, current_rules, current_generation) = {
            let s = self.settings.borrow();
            (
                (s.rules.layers, s.rules.ignore_unimportant),
                s.rules.clone(),
                s.anchors_generation,
            )
        };
        let mut restored = settings.clone();
        if restored.anchors_generation != current_generation {
            // A side was replaced since this edit was captured: anchors and the
            // resolved mapping address the replaced content. Restore only the
            // sheet content; the session rules in force stay in force and are
            // never clobbered by an undo across the boundary.
            restored.overrides = AlignmentOverrides::default();
            restored.anchors_generation = current_generation;
            restored.rules = current_rules;
        }
        *self.settings.borrow_mut() = restored;
        let mut s = self.settings.borrow_mut();
        s.rules.layers = projection.0;
        s.rules.ignore_unimportant = projection.1;
    }
}
impl EditItem for TableEdit {
    fn description(&self) -> String {
        self.description.clone()
    }
    fn undo(&mut self) {
        self.restore(
            &self.before,
            &self.before_state,
            &self.before_settings,
            self.before_aligned,
        );
    }
    fn redo(&mut self) {
        self.restore(
            &self.after,
            &self.after_state,
            &self.after_settings,
            self.after_aligned,
        );
    }
}
