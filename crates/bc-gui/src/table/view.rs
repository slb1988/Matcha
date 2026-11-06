use super::{
    cache::{Cache, Filter},
    find::FindPanel,
    grid::{self, Cursor, GridState, ROW_HEIGHT},
    settings_panel::{Draft, SettingsPanel},
    worker::{AlignJob, LoadJob, Output, Prepared, ProjectJob, ScanJob, Worker},
};
use crate::text::projection::{DisplayFilter, Projection};
use bc_core::{edit::EditStack, CoreError};
use bc_engine_table::{
    compare::Importance,
    jobs::{TableAlignJob, TableRealignJob},
    rules::Layers,
    sheets::SheetPairs,
    source::ReadOptions,
    Sheet, TableMap, TableRules,
};
use bc_session::prefs::Prefs;
use eframe::egui;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
#[derive(Debug, Clone)]
pub enum TableAction {
    Home,
    Exit,
    Text { left: PathBuf, right: PathBuf },
}
#[derive(Clone)]
pub(super) enum Pending {
    Home,
    Exit,
    Reload,
    Open(usize, PathBuf),
    Clipboard(usize, String),
    Settings(Box<Draft>),
    Text,
}
/// A destructive load staged on the worker: files/options/rules/pairs of the
/// running session are replaced only after the load succeeds; a failure keeps
/// the previous comparison untouched.
pub(super) enum Staged {
    Reload,
    /// side, new path, whether the in-memory other side is preserved, routed options
    Open(usize, PathBuf, bool, ReadOptions),
    Settings(Box<Draft>),
    /// Load the replacement side sheet for a preserved pair being visited.
    PairSide(usize),
}
#[derive(Clone, Copy)]
pub(super) enum FileMode {
    Open(usize),
    Save(usize),
    Export(usize),
    Summary(bool),
}
pub(super) struct PathDialog {
    pub mode: FileMode,
    pub path: String,
}
#[derive(Clone)]
pub(super) struct CellEditor {
    pub side: usize,
    pub source: Cursor,
    pub display: Cursor,
    pub original: String,
    pub original_cell: bc_engine_table::Cell,
    pub text: String,
    pub focus: bool,
}
impl CellEditor {
    pub fn changed(&self) -> bool {
        self.text != self.original
    }
}
pub(super) struct Pair {
    pub map: TableMap,
    pub history: [EditStack; 2],
    pub cache: Cache,
    pub projection: Projection,
    /// A preserved pair whose replacement side sheet is still loading:
    /// (side, sheet index) to fetch before it can be compared.
    pub pending_side: Option<(usize, usize)>,
}
#[derive(Default, Clone)]
pub struct FileInfo {
    pub path: Option<PathBuf>,
    pub target: Option<PathBuf>,
    pub byte_len: u64,
    pub load_time: Duration,
    pub readonly: bool,
    pub clipboard_dirty: bool,
}
pub struct TableCompareView {
    pub(super) pair: Option<Pair>,
    pub(super) cached: HashMap<usize, Pair>,
    pub(super) active_pair: usize,
    pub(super) names: [Vec<String>; 2],
    pub(super) pairs: SheetPairs,
    pub(super) states: Vec<Option<Importance>>,
    pub(super) summaries: Vec<Option<[bc_engine_table::map::TableStatistics; 3]>>,
    pub files: [FileInfo; 2],
    pub(super) options: [ReadOptions; 2],
    pub(super) default_rules: TableRules,
    pub(super) left_source: bool,
    pub(super) worker: Option<Worker>,
    pub(super) staged: Option<Staged>,
    pub(super) held_output: Option<Result<Output, bc_jobs::JobError>>,
    pub(super) scan: Option<Worker>,
    /// A completed scan attempt that failed; cleared by the next user action
    /// (cancel), so pending sheets never look Same and never busy-restart.
    pub(super) scan_failed: bool,
    pub(super) expected_rules: Option<TableRules>,
    pub(super) edited_at: Option<Instant>,
    pub(super) grid: GridState,
    pub(super) filter: Filter,
    pub(super) hide_same: bool,
    pub(super) show_details: bool,
    pub(super) show_info: bool,
    pub(super) show_thumbnail: bool,
    pub(super) show_toolbar: bool,
    pub(super) find: FindPanel,
    pub(super) settings: SettingsPanel,
    pub(super) editor: Option<CellEditor>,
    pub(super) dialog: Option<PathDialog>,
    pub(super) pending: Option<Pending>,
    pub(super) action: Option<TableAction>,
    pub(super) message: String,
    pub(super) show_statistics: bool,
    pub(super) goto: [usize; 2],
    pub(super) align_with: Option<(usize, usize)>,
    pub(super) clipboard_side: Option<usize>,
    pub(super) paste_requested: bool,
    pub(super) focus_grid: Option<usize>,
}
impl TableCompareView {
    pub(super) fn blank() -> Self {
        Self {
            pair: None,
            cached: HashMap::new(),
            active_pair: 0,
            names: [Vec::new(), Vec::new()],
            pairs: Default::default(),
            states: Vec::new(),
            summaries: Vec::new(),
            files: Default::default(),
            options: Default::default(),
            default_rules: Default::default(),
            left_source: true,
            worker: None,
            staged: None,
            held_output: None,
            scan: None,
            scan_failed: false,
            expected_rules: None,
            edited_at: None,
            grid: GridState {
                row_numbers: true,
                ..Default::default()
            },
            filter: Filter::All,
            hide_same: false,
            show_details: false,
            show_info: true,
            show_thumbnail: true,
            show_toolbar: true,
            find: Default::default(),
            settings: Default::default(),
            editor: None,
            dialog: None,
            pending: None,
            action: None,
            message: String::new(),
            show_statistics: false,
            goto: [1, 1],
            align_with: None,
            clipboard_side: None,
            paste_requested: false,
            focus_grid: None,
        }
    }
    pub fn open(left: &Path, right: &Path) -> Result<Self, CoreError> {
        Self::open_with_registry(left, right, &super::formats::load_registry()?)
    }
    pub(crate) fn open_with_registry(
        left: &Path,
        right: &Path,
        registry: &bc_formats::registry::FormatRegistry,
    ) -> Result<Self, CoreError> {
        let mut view = Self::blank();
        for (side, path) in [left, right].iter().enumerate() {
            let metadata = std::fs::metadata(path)?;
            if !metadata.is_file() {
                return Err(CoreError::Invalid("Choose two files".into()));
            }
            view.files[side].path = Some((*path).into());
            view.files[side].readonly = metadata.permissions().readonly();
            view.options[side] = super::formats::options(path, registry)?;
        }
        view.reload();
        Ok(view)
    }
    pub fn from_sheets(left: Sheet, right: Sheet, rules: TableRules) -> Self {
        let mut view = Self::blank();
        view.names = [vec![left.name.clone()], vec![right.name.clone()]];
        view.pairs = SheetPairs(vec![bc_engine_table::sheets::SheetPair {
            left: Some(0),
            right: Some(0),
        }]);
        view.states = vec![None];
        view.summaries = vec![None];
        view.default_rules = rules.clone();
        let job = TableAlignJob {
            left: left.clone(),
            right: right.clone(),
            rules,
            overrides: Default::default(),
        };
        view.worker = Some(Worker::spawn(Box::new(AlignJob {
            job: Box::new(job),
            left,
            right,
        })));
        view
    }
    pub fn paths(&self) -> [Option<PathBuf>; 2] {
        [self.files[0].path.clone(), self.files[1].path.clone()]
    }
    pub fn is_dirty(&self) -> bool {
        self.editor.as_ref().is_some_and(CellEditor::changed)
            || self.files.iter().any(|f| f.clipboard_dirty)
            || self
                .pair
                .iter()
                .chain(self.cached.values())
                .any(|p| p.history.iter().any(EditStack::is_dirty))
    }
    pub fn is_ready(&self) -> bool {
        self.worker.is_none()
            && self.held_output.is_none()
            && self.edited_at.is_none()
            && self.pair.as_ref().is_some_and(|p| !p.map.is_stale())
    }
    pub fn row_count(&self) -> usize {
        self.pair.as_ref().map_or(0, |p| p.projection.len())
    }
    pub fn top_row(&self, _height: f32) -> usize {
        (self.grid.scroll.y / ROW_HEIGHT).max(0.0) as usize
    }
    pub fn scroll_to_row(&mut self, row: usize) {
        self.grid.scroll.y = row.min(self.row_count().saturating_sub(1)) as f32 * ROW_HEIGHT;
    }
    pub fn scroll_to_column(&mut self, column: usize) {
        if let Some(d) = self.grid.columns.iter().position(|&c| c == column) {
            self.grid.scroll.x = self.grid.offsets[d];
        }
    }
    pub fn index_bytes(&self) -> usize {
        self.pair.as_ref().map_or(0, |p| {
            p.map.output().data.compact_row_bytes() + p.map.output().data.state_bytes()
        })
    }
    pub fn rows_drawn(&self) -> usize {
        self.grid.rows_drawn
    }
    pub fn cells_drawn(&self) -> usize {
        self.grid.cells_drawn
    }
    pub fn request_home(&mut self) {
        self.request(Pending::Home);
    }
    pub fn request_exit(&mut self) {
        self.request(Pending::Exit);
    }
    pub(super) fn cancel(&mut self) {
        self.align_with = None;
        self.cancel_comparison();
        drop(self.scan.take());
        // A user action is the retry path for a failed sheet scan.
        self.scan_failed = false;
    }
    pub(super) fn cancel_comparison(&mut self) {
        drop(self.worker.take());
        self.staged = None;
        self.held_output = None;
        self.edited_at = None;
        self.expected_rules = None;
    }
    pub(super) fn reload(&mut self) {
        self.cancel();
        // Staged: the running comparison, names and options stay until the
        // worker finishes; a failed reload keeps the previous session.
        let mut rules = self.default_rules.clone();
        rules.columns.0.clear();
        self.staged = Some(Staged::Reload);
        self.worker = Some(Worker::spawn(Box::new(LoadJob {
            paths: self.paths(),
            options: self.options.clone(),
            rules,
            pair: None,
            left_source: self.left_source,
            memory: [None, None],
            fixed_pairs: None,
        })));
        self.message = "Loading and comparing in background…".into();
    }
    pub(super) fn install(&mut self, prepared: Prepared) -> Result<(), CoreError> {
        let Prepared {
            left,
            right,
            output,
            cache,
        } = prepared;
        if self.pair.is_some() {
            self.install_aligned(output, cache)
        } else {
            let mut map = TableMap::from_output(left, right, output)?;
            map.set_deferred_edits(true);
            let projection = cache.project(self.filter);
            self.pair = Some(Pair {
                map,
                history: [EditStack::new(), EditStack::new()],
                cache,
                projection,
                pending_side: None,
            });
            self.finish_install();
            Ok(())
        }
    }
    /// Publish a worker result into the live pair: revisions must match the
    /// current sheets (a single-side replacement swaps its sheet in first).
    pub(super) fn install_aligned(
        &mut self,
        output: bc_engine_table::map::TableAlignOutput,
        cache: Cache,
    ) -> Result<(), CoreError> {
        let pair = self
            .pair
            .as_mut()
            .ok_or_else(|| CoreError::Invalid("Table is loading".into()))?;
        if self
            .expected_rules
            .as_ref()
            .is_some_and(|r| *r != *pair.map.rules())
            || output.revisions
                != (
                    pair.map.left.borrow().revision(),
                    pair.map.right.borrow().revision(),
                )
        {
            return Err(CoreError::Invalid("stale table result discarded".into()));
        }
        // Auto column/key discovery resolves the exact requested draft on the worker.
        *pair.map.rules_mut() = output.rules.clone();
        pair.map.set_alignment(output)?;
        pair.cache = cache;
        pair.projection = pair.cache.project(self.filter);
        self.finish_install();
        Ok(())
    }
    fn finish_install(&mut self) {
        self.edited_at = None;
        self.expected_rules = None;
        self.refresh_grid();
        if let Some(p) = &self.pair {
            if self.active_pair < self.states.len() {
                self.states[self.active_pair] =
                    Some(p.map.output().data.importance(p.map.rules().layers, false));
            }
        }
        self.summaries.resize(self.states.len(), None);
        if let Some(p) = &self.pair {
            if self.active_pair < self.summaries.len() {
                self.summaries[self.active_pair] = Some(p.cache.summaries);
            }
        }
        self.message = "Comparison complete".into();
    }
    fn maybe_spawn_scan(&mut self) {
        // Never scan mid-transaction: a scan spawned with the outgoing
        // paths/pairing has no generation guard and could publish into the
        // committed structure by index.
        if self.staged.is_some() {
            return;
        }
        // Only pairs this ScanJob can publish for — non-active and not cached —
        // justify a scan; retained pending cached pairs must not loop the
        // scanner, and a failed scan waits for a user action (cancel clears it).
        let fillable =
            self.states.iter().enumerate().any(|(i, s)| {
                s.is_none() && i != self.active_pair && !self.cached.contains_key(&i)
            });
        if self.pairs.0.len() > 1 && self.scan.is_none() && !self.scan_failed && fillable {
            self.scan = Some(Worker::spawn(Box::new(ScanJob {
                paths: self.paths(),
                options: self.options.clone(),
                pairs: self.pairs.clone(),
                rules: self.default_rules.clone(),
            })));
        }
    }
    pub fn poll(&mut self) {
        // Do not move comparison coordinates underneath an uncommitted cell edit.
        if self.held_output.is_none() {
            if let Some(result) = self.worker.as_mut().and_then(Worker::poll) {
                self.worker = None;
                self.held_output = Some(result);
            }
        }
        let result = if self.editor.is_none() {
            self.held_output.take()
        } else {
            None
        };
        if let Some(result) = result {
            self.worker = None;
            match result {
                Ok(Output::Loaded(loaded)) => {
                    let loaded = *loaded;
                    let staged = self.staged.take();
                    // Open commits its own names/pairs and re-hangs cached pairs;
                    // the other full replacements reset the whole structure here.
                    let full = matches!(staged, Some(Staged::Reload | Staged::Settings(_)))
                        || (staged.is_none() && self.names[0].is_empty());
                    if full {
                        self.names = loaded.names.clone();
                        self.pairs = loaded.pairs.clone();
                        self.states = vec![None; self.pairs.0.len()];
                        self.summaries = vec![None; self.pairs.0.len()];
                        self.cached.clear();
                        self.grid.scroll = egui::Vec2::ZERO;
                    }
                    match staged {
                        Some(Staged::PairSide(s)) => {
                            // Visiting a preserved pair whose replacement side
                            // sheet just arrived: swap it in and realign.
                            let super::worker::Prepared {
                                left,
                                right,
                                output,
                                cache,
                            } = loaded.prepared;
                            if self.pair.is_some() {
                                {
                                    let p = self.pair.as_mut().unwrap();
                                    p.map.replace_sheet(
                                        super::commands::side(s),
                                        if s == 0 { left } else { right },
                                    );
                                }
                                match self.install_aligned(output, cache) {
                                    Ok(()) => {
                                        // Only a successful publish clears the
                                        // flag; failure keeps it for retry.
                                        if let Some(p) = &mut self.pair {
                                            p.pending_side = None;
                                        }
                                    }
                                    Err(e) => self.message = e.to_string(),
                                }
                            } else if let Err(e) = self.install(super::worker::Prepared {
                                left,
                                right,
                                output,
                                cache,
                            }) {
                                self.message = e.to_string();
                            }
                        }
                        Some(Staged::Open(s, path, preserve, options)) => {
                            self.commit_open_side(s, path, preserve, options, loaded);
                        }
                        Some(Staged::Settings(draft)) => {
                            self.default_rules = draft.rules.clone();
                            self.default_rules.left_is_source = draft.left_source;
                            self.options = draft.options;
                            self.left_source = draft.left_source;
                            for s in 0..2 {
                                self.files[s].readonly = draft.readonly[s];
                                self.files[s].path = (!draft.paths[s].is_empty())
                                    .then(|| draft.paths[s].clone().into());
                                self.files[s].target = None;
                                self.files[s].clipboard_dirty = false;
                                self.files[s].byte_len = loaded.bytes[s];
                                self.files[s].load_time = loaded.times[s];
                            }
                            self.pair = None;
                            // The worker resolved the active pair exactly (the
                            // fixed manual pairing or a re-derived pairing for
                            // changed sources); always publish it.
                            self.active_pair =
                                loaded.active.min(self.pairs.0.len().saturating_sub(1));
                            if let Err(e) = self.install(loaded.prepared) {
                                self.message = e.to_string();
                            }
                        }
                        other => {
                            if matches!(other, Some(Staged::Reload)) {
                                self.pair = None;
                                self.active_pair = 0;
                                self.default_rules.columns.0.clear();
                            }
                            for s in 0..2 {
                                self.files[s].byte_len = loaded.bytes[s];
                                self.files[s].load_time = loaded.times[s];
                            }
                            if let Err(e) = self.install(loaded.prepared) {
                                self.message = e.to_string();
                            }
                        }
                    }
                }
                Ok(Output::Aligned(prepared)) => {
                    if let Err(e) = self.install(*prepared) {
                        self.message = e.to_string();
                    }
                }
                Ok(Output::Projected(cache, revisions, rules, fit_all)) => {
                    if let Some(p) = &mut self.pair {
                        if revisions
                            == (
                                p.map.left.borrow().revision(),
                                p.map.right.borrow().revision(),
                            )
                            && rules == *p.map.rules()
                        {
                            p.cache = *cache;
                            if fit_all {
                                self.grid.widths = p.cache.widths.clone();
                            }
                            p.projection = p.cache.project(self.filter);
                        }
                    }
                    self.refresh_grid();
                }
                Ok(Output::Scanned(_)) => {}
                Err(bc_jobs::JobError::Cancelled) => {
                    self.staged = None;
                }
                Err(e) => {
                    self.message = e.to_string();
                    self.edited_at = None;
                    // A failed staged load keeps the previous session untouched.
                    self.staged = None;
                }
            }
        }
        self.maybe_spawn_scan();
        if let Some(result) = self.scan.as_mut().and_then(Worker::poll) {
            self.scan = None;
            match result {
                Ok(Output::Scanned(states)) => {
                    for (i, state) in states.into_iter().enumerate() {
                        if i != self.active_pair
                            && !self.cached.contains_key(&i)
                            && i < self.states.len()
                        {
                            self.states[i] = Some(
                                state[super::cache::layer_index(self.default_rules.layers)]
                                    .importance(),
                            );
                            self.summaries.resize(self.states.len(), None);
                            self.summaries[i] = Some(state);
                        }
                    }
                }
                Ok(_) => {}
                Err(bc_jobs::JobError::Cancelled) => {}
                Err(e) => {
                    // Honest pending dots plus an explicit message; the next
                    // user action (tab switch/Reload/…) allows the retry.
                    self.scan_failed = true;
                    self.message =
                        format!("Sheet scan failed: {e}; switch sheets or Reload to retry");
                }
            }
        }
        if self.worker.is_none()
            && self.editor.is_none()
            && self.held_output.is_none()
            && self
                .edited_at
                .is_some_and(|t| t.elapsed() >= Duration::from_millis(350))
        {
            if let Some(p) = &self.pair {
                self.expected_rules = Some(p.map.rules().clone());
                self.worker = Some(Worker::spawn(Box::new(AlignJob {
                    job: Box::new(TableRealignJob::from_map(&p.map)),
                    left: p.map.left.borrow().clone(),
                    right: p.map.right.borrow().clone(),
                })));
            }
        }
    }
    pub(super) fn edited(&mut self) {
        self.align_with = None;
        if let Some(w) = &self.worker {
            w.cancel();
        }
        self.edited_at = Some(Instant::now());
        self.find.last = None;
        if let Some(p) = &mut self.pair {
            let rows = p
                .map
                .left
                .borrow()
                .row_count()
                .max(p.map.right.borrow().row_count());
            p.projection = Projection::build(rows, &[], DisplayFilter::All, 0, &[]);
            self.grid
                .rebuild_columns(p.map.rules().columns.0.len(), &[], false);
        }
        self.message = "Edited — background realignment pending".into();
    }
    /// (Re)start the background load of the active pair's pending replacement
    /// side sheet. Shared by select_sheet, Swap, Recompare and projection so a
    /// placeholder is never compared or reported as ready.
    pub(super) fn load_pending_pair(&mut self) {
        let Some((s, _sheet)) = self.pair.as_ref().and_then(|p| p.pending_side) else {
            return;
        };
        let index = self.active_pair;
        let p = self.pair.as_ref().unwrap();
        let other = 1 - s;
        let mut memory = [None, None];
        memory[other] = Some(super::worker::MemorySheet {
            sheet: if other == 0 {
                p.map.left.borrow().clone()
            } else {
                p.map.right.borrow().clone()
            },
            names: self.names[other].clone(),
            index: if other == 0 {
                self.pairs.0[index].left
            } else {
                self.pairs.0[index].right
            }
            .unwrap_or(0),
        });
        let mut rules = self.default_rules.clone();
        rules.columns.0.clear();
        self.staged = Some(Staged::PairSide(s));
        self.worker = Some(Worker::spawn(Box::new(LoadJob {
            paths: self.paths(),
            options: self.options.clone(),
            rules,
            pair: Some(self.pairs.0[index].clone()),
            left_source: self.left_source,
            memory,
            fixed_pairs: Some(self.pairs.clone()),
        })));
        self.message = "Loading the replaced side's sheet…".into();
    }
    /// 键列标注：自动/手动推断结果带比较列名，如 `id（自动）`。
    pub(super) fn keys_description(&self) -> String {
        let Some(p) = &self.pair else {
            return String::new();
        };
        let rules = p.map.rules();
        let label = rules.keys.label();
        match &rules.keys {
            bc_engine_table::rules::KeySelection::Automatic { .. }
            | bc_engine_table::rules::KeySelection::Manual(_) => {
                let names = rules
                    .key_columns()
                    .iter()
                    .map(|&c| {
                        let column = &rules.columns.0[c];
                        column
                            .left
                            .map(|i| p.map.left.borrow().column_name(i))
                            .or_else(|| column.right.map(|i| p.map.right.borrow().column_name(i)))
                            .unwrap_or_else(|| format!("Column {}", c + 1))
                    })
                    .collect::<Vec<_>>()
                    .join("+");
                if names.is_empty() {
                    label.into()
                } else {
                    format!("{names}（{label}）")
                }
            }
            _ => label.into(),
        }
    }
    pub(super) fn recompare(&mut self) {
        self.cancel();
        if self.pair.as_ref().is_some_and(|p| p.pending_side.is_some()) {
            // A pending placeholder is never compared: restart the side load.
            self.load_pending_pair();
            return;
        }
        if let Some(p) = &self.pair {
            self.expected_rules = Some(p.map.rules().clone());
            self.worker = Some(Worker::spawn(Box::new(AlignJob {
                job: Box::new(TableAlignJob::from_map(&p.map)),
                left: p.map.left.borrow().clone(),
                right: p.map.right.borrow().clone(),
            })));
        }
    }
    pub(super) fn refresh_grid(&mut self) {
        if let Some(p) = &mut self.pair {
            if self.grid.widths.len() != p.cache.widths.len() {
                self.grid.widths = p.cache.widths.clone();
            }
            self.grid.rebuild_columns(
                p.map.rules().columns.0.len(),
                &p.cache.different_columns,
                self.hide_same,
            );
            self.grid.cursor.row = self.grid.cursor.row.min(p.cache.total.saturating_sub(1));
            self.grid.cursor.column = self
                .grid
                .cursor
                .column
                .min(p.map.column_count().saturating_sub(1));
        }
    }
    pub(super) fn project(&mut self) {
        self.project_with_fit(false);
    }
    pub(super) fn project_with_fit(&mut self, fit_all: bool) {
        // Sheet scans return raw statistics for all three layers. A display-only
        // change must not cancel them and leave unvisited sheet tabs pending.
        self.cancel_comparison();
        if let Some(p) = &self.pair {
            let rules = p.map.rules().clone();
            self.default_rules.layers = rules.layers;
            self.default_rules.ignore_unimportant = rules.ignore_unimportant;
            for (i, summary) in self.summaries.iter().enumerate() {
                if let Some(summary) = summary {
                    self.states[i] =
                        Some(summary[super::cache::layer_index(rules.layers)].importance());
                }
            }
            for (&i, p) in &self.cached {
                p.map.rules_mut().layers = rules.layers;
                p.map.rules_mut().ignore_unimportant = rules.ignore_unimportant;
                // A stale or still-loading pair keeps its pending dot; never
                // publish a state computed from an outdated snapshot.
                if i < self.states.len() && !p.map.is_stale() && p.pending_side.is_none() {
                    self.states[i] = Some(p.map.output().data.importance(rules.layers, false));
                }
            }
            if self.active_pair < self.states.len() && p.pending_side.is_none() {
                self.states[self.active_pair] =
                    Some(p.map.output().data.importance(rules.layers, false));
            }
            if p.pending_side.is_some() {
                // A display-layer change does not compare a placeholder; the
                // replacement side load restarts with the updated rules.
                self.load_pending_pair();
                return;
            }
            if p.map.is_stale() {
                self.recompare();
                return;
            }
            let mut output = p.map.output();
            output.rules = rules;
            self.worker = Some(Worker::spawn(Box::new(ProjectJob {
                fit_all,
                output,
                left: p.map.left.borrow().clone(),
                right: p.map.right.borrow().clone(),
            })));
        }
    }
    pub(super) fn select_sheet(&mut self, index: usize) {
        if index == self.active_pair || index >= self.pairs.0.len() {
            return;
        }
        self.cancel();
        if let Some(p) = self.pair.take() {
            self.cached.insert(self.active_pair, p);
        }
        self.active_pair = index;
        self.grid = GridState {
            row_numbers: self.grid.row_numbers,
            whitespace: self.grid.whitespace,
            stacked: self.grid.stacked,
            ..Default::default()
        };
        if let Some(p) = self.cached.remove(&index) {
            let pending = p.pending_side;
            self.pair = Some(p);
            self.refresh_grid();
            if pending.is_some() {
                // A preserved pair whose replacement side sheet has not loaded
                // yet: fetch it in the background, then replace + realign.
                self.load_pending_pair();
            } else if self.pair.as_ref().is_some_and(|p| p.map.is_stale()) {
                self.recompare();
            } else {
                self.project();
            }
        } else {
            let mut rules = self.default_rules.clone();
            rules.columns.0.clear();
            rules.keys = bc_engine_table::rules::KeySelection::Automatic {
                columns: Vec::new(),
                score: 0.0,
            };
            self.worker = Some(Worker::spawn(Box::new(LoadJob {
                paths: self.paths(),
                options: self.options.clone(),
                rules,
                pair: Some(self.pairs.0[index].clone()),
                left_source: self.left_source,
                memory: [None, None],
                // The current pairing (possibly manually arranged) is the
                // source of truth for tab indices; never re-derive it here.
                fixed_pairs: Some(self.pairs.clone()),
            })));
        }
    }
    pub(super) fn goto_cell(&mut self, mut cursor: Cursor) {
        if let Some(pair) = &mut self.pair {
            let stale = pair.map.is_stale();
            let count = if stale {
                pair.map
                    .left
                    .borrow()
                    .row_count()
                    .max(pair.map.right.borrow().row_count())
            } else {
                pair.cache.total
            };
            if count == 0 {
                self.message = "Table has no rows".into();
                return;
            }
            let columns = pair.map.rules().columns.0.len();
            cursor.row = cursor.row.min(count - 1);
            cursor.column = cursor.column.min(columns.saturating_sub(1));
            let display = pair.projection.display_for(cursor.row);
            if super::cache::projected(&pair.projection, display) != Some(cursor.row) {
                self.filter = Filter::All;
                pair.projection = if stale {
                    Projection::build(count, &[], DisplayFilter::All, 0, &[])
                } else {
                    pair.cache.project(self.filter)
                };
            }
            if self.hide_same && !self.grid.columns.contains(&cursor.column) {
                self.hide_same = false;
                self.grid.rebuild_columns(columns, &[], false);
            }
            self.grid.cursor = cursor;
            self.grid.anchor = None;
            self.grid.scroll.y = pair.projection.display_for(cursor.row) as f32 * ROW_HEIGHT;
            if let Some(d) = self.grid.columns.iter().position(|&c| c == cursor.column) {
                self.grid.scroll.x = self.grid.offsets[d];
            }
            self.message = format!("Row {}, column {}", cursor.row + 1, cursor.column + 1);
        }
    }
    fn go_to(&mut self) {
        if !self.is_ready() || self.editor.is_some() {
            self.message = "Wait for comparison / finish editing before Go To".into();
            return;
        }
        let pair = self.pair.as_ref().unwrap();
        let (rows, columns) = (pair.map.row_count(), pair.map.column_count());
        if rows == 0 || columns == 0 {
            self.message = "Table has no cells".into();
            return;
        }
        let cursor = Cursor {
            row: self.goto[0].saturating_sub(1).min(rows - 1),
            column: self.goto[1].saturating_sub(1).min(columns - 1),
        };
        self.goto_cell(cursor);
        self.goto = [cursor.row + 1, cursor.column + 1];
        self.focus_grid = Some(self.grid.active);
    }
    pub(super) fn settings_open(&mut self) {
        if let Some(pair) = &self.pair {
            self.settings.draft = Some(Draft {
                rules: pair.map.rules().clone(),
                options: self.options.clone(),
                pairs: self.pairs.clone(),
                paths: self
                    .paths()
                    .map(|p| p.map_or(String::new(), |p| p.display().to_string())),
                readonly: [self.files[0].readonly, self.files[1].readonly],
                left_source: self.left_source,
            });
            self.settings.open = true;
        }
    }
    pub fn ui(&mut self, ui: &mut egui::Ui, prefs: &mut Prefs) -> Option<TableAction> {
        self.poll();
        let ctx = ui.ctx().clone();
        if self.worker.is_some() || self.scan.is_some() || self.edited_at.is_some() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        if self.pending.is_some() || self.dialog.is_some() || self.editor.is_some() {
            ui.disable();
        }
        self.restore_grid_focus(&ctx);
        self.keyboard(&ctx, prefs);
        self.toolbar(ui, prefs);
        if let Some(action) = self.find.ui(ui) {
            if let Err(e) = self.find_action(action) {
                self.message = e.to_string();
            }
        }
        if self.editor.is_none() && self.pending.is_none() {
            let key_hint = self.keys_description();
            if let Some(draft) = self.settings.ui(&ctx, &self.names, &key_hint) {
                self.request(Pending::Settings(Box::new(draft)));
            }
        }
        if self.show_details {
            if let Some(p) = &self.pair {
                super::details::ui(ui, &p.map, self.grid.cursor);
            }
        }
        let tab_states = self
            .states
            .iter()
            .map(|s| s.map(|s| s.projected(self.default_rules.ignore_unimportant)))
            .collect::<Vec<_>>();
        if let Some(index) = super::sheet_tabs::ui(
            ui,
            &self.names,
            &self.pairs,
            &tab_states,
            self.active_pair,
            &prefs.colors,
        ) {
            self.select_sheet(index);
        }
        egui::Panel::bottom("table_status").show(ui,|ui|{
            if let Some(p)=&self.pair{let s=p.cache.statistics;ui.label(format!("{} same · {} unimportant · {} important · {} left-only · {} right-only · {} columns · keys: {}{}",s.same,s.unimportant,s.important,s.left_only,s.right_only,s.columns,self.keys_description(),if p.map.is_stale(){" · stale (recomparing)"}else{""}));}
            ui.label(self.worker.as_ref().map(Worker::progress).filter(|s|!s.is_empty()).unwrap_or_else(||self.message.clone()));
        });
        self.grid.rows_drawn = 0;
        self.grid.cells_drawn = 0;
        self.grid.edit_target = self.editor.as_ref().map(|e| (e.side, e.display));
        self.grid.edit_rect = None;
        let mut grid_action = None;
        let mut nav_target = None;
        if let Some(p) = &self.pair {
            ui.horizontal_top(|ui| {
                if self.show_thumbnail {
                    nav_target = super::details::thumbnail(
                        ui,
                        &p.cache.bins,
                        p.cache.total,
                        self.grid.visible.clone(),
                        &prefs.colors,
                    );
                }
                let available = ui.available_size();
                ui.allocate_ui_with_layout(
                    available,
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        if self.grid.stacked {
                            for s in 0..2 {
                                ui.allocate_ui(
                                    egui::vec2(available.x, (available.y - 8.0) / 2.0),
                                    |ui| {
                                        if let Some(a) = grid::pane(
                                            ui,
                                            &p.map,
                                            &p.projection,
                                            &mut self.grid,
                                            s,
                                            (available.y - 8.0) / 2.0,
                                            &prefs.colors,
                                        ) {
                                            grid_action = Some(a);
                                        }
                                    },
                                );
                            }
                        } else {
                            ui.columns(2, |columns| {
                                for (s, ui) in columns.iter_mut().enumerate() {
                                    if let Some(a) = grid::pane(
                                        ui,
                                        &p.map,
                                        &p.projection,
                                        &mut self.grid,
                                        s,
                                        available.y,
                                        &prefs.colors,
                                    ) {
                                        grid_action = Some(a);
                                    }
                                }
                            });
                        }
                    },
                );
            });
        } else if self.worker.is_none() {
            // Initial load failed: show the error with an explicit retry entry
            // instead of spinning forever.
            ui.label(format!("Load failed: {}", self.message));
            if ui.button("Retry load").clicked() {
                self.reload();
            }
        } else {
            ui.spinner();
            ui.label("Loading table sources on worker…");
        }
        if let Some(row) = nav_target {
            self.goto_cell(Cursor {
                row,
                column: self.grid.cursor.column,
            });
        }
        if let Some(action) = grid_action {
            self.grid_action(action);
        }
        self.dialogs(&ctx, prefs);
        self.restore_grid_focus(&ctx);
        if let Some(path) =
            ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()))
        {
            let side = ctx
                .input(|i| i.pointer.hover_pos())
                .and_then(|p| self.grid.pane_rects.iter().position(|r| r.contains(p)))
                .unwrap_or(self.grid.active);
            self.request(Pending::Open(side, path));
        }
        self.action.take()
    }
    fn restore_grid_focus(&mut self, ctx: &egui::Context) {
        if self.editor.is_none() && self.pending.is_none() && self.dialog.is_none() {
            if let Some(side) = self.focus_grid.take() {
                ctx.memory_mut(|m| m.request_focus(egui::Id::unique(("table_grid_focus", side))));
            }
        }
    }
    fn view_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("View", |ui| {
            ui.checkbox(&mut self.show_toolbar, "Toolbar");
            ui.checkbox(&mut self.grid.row_numbers, "Row Numbers");
            ui.checkbox(&mut self.grid.whitespace, "Visible Whitespace");
            ui.checkbox(&mut self.show_details, "Text Details");
            ui.checkbox(&mut self.show_info, "File Info");
            ui.checkbox(&mut self.show_thumbnail, "Thumbnail");
            if ui.button("Resize Columns to Fit").clicked() {
                self.project_with_fit(true);
                ui.close();
            }
        });
    }
    fn toolbar(&mut self, ui: &mut egui::Ui, prefs: &Prefs) {
        egui::Panel::top("table_commands").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.menu_button("Session", |ui| {
                    ui.add_enabled(false, egui::Button::new("Save Session — session-and-cli"));
                    ui.add_enabled(
                        false,
                        egui::Button::new("Save Session As — session-and-cli"),
                    );
                    if ui.button("Session Settings").clicked() {
                        self.settings_open();
                        ui.close();
                    }
                    if ui.button("Swap Sides").clicked() {
                        self.swap();
                        ui.close();
                    }
                    if ui.button("Reload Files").clicked() {
                        self.request(Pending::Reload);
                        ui.close();
                    }
                    if ui.button("Recompare Files").clicked() {
                        self.recompare();
                        ui.close();
                    }
                    if ui.button("Table Compare Info").clicked() {
                        self.show_statistics = true;
                        ui.close();
                    }
                    if ui.button("Compare Files Using Text Compare").clicked() {
                        self.request(Pending::Text);
                        ui.close();
                    }
                    ui.add_enabled(
                        false,
                        egui::Button::new("Compare Parent Folders — pending folder integration"),
                    );
                    if ui.button("Export summary (Text)").clicked() {
                        self.dialog = Some(PathDialog {
                            mode: FileMode::Summary(false),
                            path: String::new(),
                        });
                        ui.close();
                    }
                    if ui.button("Export summary (CSV)").clicked() {
                        self.dialog = Some(PathDialog {
                            mode: FileMode::Summary(true),
                            path: String::new(),
                        });
                        ui.close();
                    }
                });
                ui.menu_button("File", |ui| {
                    for (s, name) in [(0, "Left"), (1, "Right")] {
                        ui.menu_button(name, |ui| {
                            if ui.button("Open File…").clicked() {
                                self.dialog = Some(PathDialog {
                                    mode: FileMode::Open(s),
                                    path: String::new(),
                                });
                                ui.close();
                            }
                            if ui
                                .button("Open Clipboard (paste after closing menu)")
                                .clicked()
                            {
                                self.clipboard_side = Some(s);
                                ui.ctx()
                                    .send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                                ui.close();
                            }
                            let workbook = self.pair.as_ref().is_some_and(|p| {
                                if s == 0 {
                                    p.map.left.borrow().source_is_workbook()
                                } else {
                                    p.map.right.borrow().source_is_workbook()
                                }
                            });
                            if ui
                                .add_enabled(!workbook, egui::Button::new("Save"))
                                .on_disabled_hover_text("Workbook is read-only; export as CSV")
                                .clicked()
                            {
                                if let Err(e) = self.save(s, prefs) {
                                    self.message = e.to_string();
                                }
                                ui.close();
                            }
                            if ui
                                .add_enabled(!workbook, egui::Button::new("Save As…"))
                                .clicked()
                            {
                                self.dialog = Some(PathDialog {
                                    mode: FileMode::Save(s),
                                    path: String::new(),
                                });
                                ui.close();
                            }
                            if ui.button("Export as CSV…").clicked() {
                                self.dialog = Some(PathDialog {
                                    mode: FileMode::Export(s),
                                    path: String::new(),
                                });
                                ui.close();
                            }
                        });
                    }
                });
                self.edit_menu(ui);
                ui.menu_button("Search", |ui| {
                    for (name, forward, kind) in [
                        ("Next Difference Cell", true, 0),
                        ("Previous Difference Cell", false, 0),
                        ("Next Difference Row", true, 1),
                        ("Previous Difference Row", false, 1),
                        ("Next Edit", true, 2),
                        ("Previous Edit", false, 2),
                    ] {
                        if ui.button(name).clicked() {
                            self.navigate(forward, kind);
                            ui.close();
                        }
                    }
                    if ui.button("Find / Replace").clicked() {
                        self.find.open = true;
                        ui.close();
                    }
                });
                self.view_menu(ui);
            });
            if self.show_toolbar {
                ui.horizontal_wrapped(|ui| {
                    if ui.button("◀ Difference").clicked() {
                        self.navigate(false, 0);
                    }
                    if ui.button("Difference ▶").clicked() {
                        self.navigate(true, 0);
                    }
                    if ui.button("Copy cell → other").clicked() {
                        self.copy(false);
                    }
                    if ui.button("Settings").clicked() {
                        self.settings_open();
                    }
                    ui.label("Go To");
                    ui.add(egui::DragValue::new(&mut self.goto[0]).range(1..=usize::MAX));
                    ui.add(egui::DragValue::new(&mut self.goto[1]).range(1..=usize::MAX));
                    if ui.button("Go").clicked() {
                        self.go_to();
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    let old = self.filter;
                    for f in [Filter::All, Filter::Differences, Filter::Same, Filter::None] {
                        ui.selectable_value(&mut self.filter, f, format!("{f:?}"));
                    }
                    if old != self.filter {
                        if let Some(p) = &mut self.pair {
                            p.projection = p.cache.project(self.filter);
                        }
                        self.grid.scroll.y = 0.0;
                    }
                    let mut changed = false;
                    if let Some(p) = &self.pair {
                        let mut rules = p.map.rules_mut();
                        changed |= ui
                            .checkbox(&mut rules.ignore_unimportant, "Ignore Unimportant")
                            .changed();
                        for (layer, label) in [
                            (Layers::Values, "Values"),
                            (Layers::ValuesAndFormulas, "+ Formulas"),
                            (Layers::All, "+ Number formats"),
                        ] {
                            changed |= ui
                                .selectable_value(&mut rules.layers, layer, label)
                                .changed();
                        }
                    }
                    if changed {
                        self.project();
                    }
                    if ui
                        .checkbox(&mut self.hide_same, "Hide Same Columns")
                        .changed()
                    {
                        self.refresh_grid();
                    }
                    ui.checkbox(&mut self.grid.stacked, "Over-Under");
                });
            }
            if self.show_info {
                for s in 0..2 {
                    let dirty = self.pair.as_ref().is_some_and(|p| p.history[s].is_dirty());
                    ui.label(format!(
                        "{}: {} · {} B · {}{}",
                        if s == 0 { "L" } else { "R" },
                        self.files[s]
                            .path
                            .as_ref()
                            .map_or("Clipboard".into(), |p| p.display().to_string()),
                        self.files[s].byte_len,
                        if self.files[s].readonly {
                            "editing disabled"
                        } else {
                            "editable"
                        },
                        if dirty { " · modified" } else { "" }
                    ));
                }
            }
        });
    }
}
