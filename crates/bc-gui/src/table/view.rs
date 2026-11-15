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
    /// 切到 Home 标签页（不关闭本会话）。
    Home,
    /// 关闭本标签页（未保存编辑已确认）。
    Close,
    /// Session > New Tab。
    NewTab,
    Exit,
    Text {
        left: PathBuf,
        right: PathBuf,
    },
    /// Using: Hex Compare 原位换会话类型。
    Hex {
        left: PathBuf,
        right: PathBuf,
    },
}
#[derive(Clone)]
pub(super) enum Pending {
    Close,
    Exit,
    Reload,
    Open(usize, PathBuf),
    Clipboard(usize, String),
    Settings(Box<Draft>),
    Text,
    Hex,
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
    /// Sticky "grid owns the keyboard" intent: set on grid clicks, cleared by
    /// clicks outside the panes, Escape, or opening Find. egui's own focus
    /// cannot carry this: its arrow-key focus navigation would move focus off
    /// the pane after one press (and `set_focus_lock_filter` cannot be armed
    /// before the first frame of focus), freezing arrow navigation.
    pub(super) key_focus: bool,
    pub(super) menus: Vec<crate::chrome::menubar::MenuSpec>,
    pub(super) show_goto: bool,
    pub(super) path_edits: [String; 2],
    pub(super) path_sources: [Option<PathBuf>; 2],
    pub(super) labels: [super::chrome_ui::FileLabels; 2],
    /// Snapshot scenes reproduce the reference "path selected on open" state.
    pub(super) snapshot_select_paths: bool,
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
            show_details: true,
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
            key_focus: false,
            menus: Vec::new(),
            show_goto: false,
            path_edits: Default::default(),
            path_sources: Default::default(),
            labels: Default::default(),
            snapshot_select_paths: false,
        }
    }
    /// 空会话：两侧无文件、无比较；路径栏提交经 `Pending::Open` 逐侧暂存加载
    /// （一侧为空时按空表比较，与已打开会话换文件同一路径）。
    pub fn empty() -> Self {
        Self::blank()
    }
    /// 预填一侧文件（空会话拖入单个文件时）。
    pub fn open_path(&mut self, side: usize, path: PathBuf) {
        self.request(Pending::Open(side, path));
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
        Self::from_stores(
            bc_engine_table::SheetStore::Dense(left),
            bc_engine_table::SheetStore::Dense(right),
            rules,
        )
    }
    /// Store-bridged constructor: dense default path, or the experimental
    /// demand-backed stores when MATCHA_TABLE_CSV_LAZY loaded the sources.
    pub fn from_stores(
        left: bc_engine_table::SheetStore,
        right: bc_engine_table::SheetStore,
        rules: TableRules,
    ) -> Self {
        let mut view = Self::blank();
        view.names = [vec![left.name()], vec![right.name()]];
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
    /// Statistics of the active pair (launch-perf fingerprinting).
    pub fn statistics(&self) -> Option<bc_engine_table::map::TableStatistics> {
        self.pair.as_ref().map(|p| p.cache.statistics)
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
    /// 关闭标签页：有未提交/未保存编辑时先弹确认，确认后产出 [`TableAction::Close`]。
    pub fn request_close(&mut self) {
        self.request(Pending::Close);
    }
    /// 确认框是否打开（标签页关闭/退出流程据此识别"取消"）。
    pub fn prompt_open(&self) -> bool {
        self.pending.is_some()
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
        crate::perf::mark("view:reload-spawned");
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
            let t = std::time::Instant::now();
            let mut map = TableMap::from_stores(left, right, output)?;
            map.set_deferred_edits(true);
            crate::perf::span("view:install-from-output", t);
            let t = std::time::Instant::now();
            let projection = cache.project(self.filter);
            crate::perf::span("view:install-project", t);
            self.pair = Some(Pair {
                map,
                history: [EditStack::new(), EditStack::new()],
                cache,
                projection,
                pending_side: None,
            });
            let t = std::time::Instant::now();
            self.finish_install();
            crate::perf::span("view:finish-install", t);
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
                crate::perf::mark("view:worker-finished");
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
                                    p.map.replace_store(
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
    /// 键列标注：自动/手动推断结果带比较列名，如 `id（自动）`；无表头行的
    /// 侧退化为 Excel 列字母（与网格表头一致）。
    pub(super) fn keys_description(&self) -> String {
        let Some(p) = &self.pair else {
            return String::new();
        };
        let rules = p.map.rules();
        let label = rules.keys.label();
        let name_of = |side: usize, i: Option<usize>, c: usize| -> Option<String> {
            let sheet = if side == 0 {
                p.map.left.borrow()
            } else {
                p.map.right.borrow()
            };
            i.map(|i| {
                if bc_engine_table::TableData::headers(&*sheet).is_some() {
                    sheet.column_name(i)
                } else {
                    super::grid::column_letter(c)
                }
            })
        };
        match &rules.keys {
            bc_engine_table::rules::KeySelection::Automatic { .. }
            | bc_engine_table::rules::KeySelection::Manual(_) => {
                let names = rules
                    .key_columns()
                    .iter()
                    .map(|&c| {
                        let column = &rules.columns.0[c];
                        name_of(0, column.left, c)
                            .or_else(|| name_of(1, column.right, c))
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
    pub(super) fn go_to(&mut self) {
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
        if self.worker.is_some()
            || self.scan.is_some()
            || self.edited_at.is_some()
            || self.labels_pending()
        {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        let blocked = self.pending.is_some() || self.dialog.is_some() || self.editor.is_some();
        if blocked {
            ui.disable();
        }
        self.restore_grid_focus(&ctx);
        if !blocked && !self.settings.open {
            self.shortcuts(&ctx, prefs);
        }
        self.keyboard(&ctx, prefs);
        ui.spacing_mut().item_spacing.y = 0.0;
        self.chrome_top(ui, prefs);
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
        self.goto_window(&ctx);
        self.grid.rows_drawn = 0;
        self.grid.cells_drawn = 0;
        self.grid.edit_target = self.editor.as_ref().map(|e| (e.side, e.display));
        self.grid.edit_rect = None;
        // Mouse wheel over the grid scrolls immediately (egui's wheel
        // smoothing reads as lag on Windows Line-unit notches); gated like
        // `keyboard` on modal/editor state.
        grid::wheel(
            &ctx,
            &mut self.grid,
            self.editor.is_none() && self.pending.is_none() && self.dialog.is_none(),
        );
        // Reference layout below the toolbar: 52 px thumbnail column, two
        // panes (path bar 26 + info 27 + header/grid + pane status 19), the
        // 81 px details strip and the 25 px status bar.
        let available = ui.available_rect_before_wrap();
        let status = egui::Rect::from_min_max(
            egui::pos2(available.left(), available.bottom() - 25.0),
            available.max,
        );
        // Workbooks with several sheets: a tab strip between the grids and the
        // status bar (single-sheet sources such as CSV show none).
        let sheet_strip = if self.pairs.0.len() > 1 {
            crate::chrome::sheet_tabs::HEIGHT
        } else {
            0.0
        };
        let tabs_rect = egui::Rect::from_min_max(
            egui::pos2(available.left(), status.top() - sheet_strip),
            egui::pos2(available.right(), status.top()),
        );
        let thumb = if self.show_thumbnail { 52.0 } else { 0.0 };
        let details_height = if self.show_details && self.pair.is_some() {
            super::details::HEIGHT
        } else {
            0.0
        };
        let details = egui::Rect::from_min_max(
            egui::pos2(available.left() + thumb, tabs_rect.top() - details_height),
            egui::pos2(available.right(), tabs_rect.top()),
        );
        if sheet_strip > 0.0 {
            let tab_states = self
                .states
                .iter()
                .map(|s| s.map(|s| s.projected(self.default_rules.ignore_unimportant)))
                .collect::<Vec<_>>();
            if let Some(index) = super::sheet_tabs::ui(
                ui,
                tabs_rect,
                &self.names,
                &self.pairs,
                &tab_states,
                self.active_pair,
                &prefs.colors,
            ) {
                self.select_sheet(index);
            }
        }
        let body =
            egui::Rect::from_min_max(available.min, egui::pos2(available.right(), details.top()));
        let panes = egui::Rect::from_min_max(body.min + egui::vec2(thumb, 0.0), body.max);
        let mut grid_action = None;
        let mut menu_owners: Vec<(usize, egui::Response)> = Vec::with_capacity(2);
        let mut nav_target = None;
        if let Some(p) = &self.pair {
            if self.show_thumbnail {
                nav_target = super::details::thumbnail(
                    ui,
                    egui::Rect::from_min_max(body.min, egui::pos2(panes.left(), body.bottom())),
                    &p.cache.bins,
                    p.cache.total,
                    self.grid.visible.clone(),
                    &prefs.colors,
                );
            }
        }
        let width = (panes.width() - 4.0) / 2.0 + 2.0;
        let half = (panes.height() - 4.0) / 2.0;
        for s in 0..2 {
            let rect = if self.grid.stacked {
                egui::Rect::from_min_size(
                    panes.min + egui::vec2(0.0, s as f32 * (half + 4.0)),
                    egui::vec2(panes.width(), half),
                )
            } else if s == 0 {
                egui::Rect::from_min_size(panes.min, egui::vec2(width, panes.height()))
            } else {
                egui::Rect::from_min_max(panes.min + egui::vec2(width + 4.0, 0.0), panes.max)
            };
            let header = if self.show_info { 53.0 } else { 0.0 };
            if self.show_info {
                self.file_header(ui, rect, s, prefs);
            }
            let pane_status =
                egui::Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - 19.0), rect.max);
            let grid_rect = egui::Rect::from_min_max(
                rect.min + egui::vec2(0.0, header),
                egui::pos2(rect.right(), pane_status.top()),
            );
            if let Some(p) = &self.pair {
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .id_salt(("table-pane", s))
                        .max_rect(grid_rect),
                    |ui| {
                        ui.set_clip_rect(grid_rect.intersect(ui.clip_rect()));
                        let (a, response) = grid::pane(
                            ui,
                            grid_rect,
                            &p.map,
                            &p.projection,
                            &mut self.grid,
                            s,
                            &p.cache.different_columns,
                            &prefs.colors,
                        );
                        if a.is_some() {
                            grid_action = a;
                        }
                        menu_owners.push((s, response));
                    },
                );
            } else if s == 0 && (self.worker.is_some() || self.paths() != [None, None]) {
                // 空会话（两侧都未选文件、也没有在加载）保持空白窗格。
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(grid_rect.shrink(8.0)),
                    |ui| {
                        if self.worker.is_none() {
                            // Initial load failed: show the error with an explicit
                            // retry entry instead of spinning forever.
                            ui.label(format!("Load failed: {}", self.message));
                            if crate::chrome::widgets::button(ui, "Retry load").clicked() {
                                self.reload();
                            }
                        } else {
                            ui.spinner();
                            ui.label("Loading table sources on worker…");
                        }
                    },
                );
            }
            self.pane_status(ui, pane_status, s);
        }
        if details_height > 0.0 {
            if let Some(p) = &self.pair {
                super::details::paint(
                    ui,
                    details,
                    &p.map,
                    &self.grid,
                    &p.cache.different_columns,
                    &prefs.colors,
                );
            }
        }
        self.status_bar(ui, status);
        ui.allocate_rect(available, egui::Sense::hover());
        if let Some(row) = nav_target {
            self.goto_cell(Cursor {
                row,
                column: self.grid.cursor.column,
            });
        }
        if let Some(action) = grid_action {
            self.grid_action(action);
        }
        // 网格右键菜单（参考 table-context）：作用于被点击的窗格与选区。
        for (s, owner) in menu_owners {
            if let Some(action) =
                crate::chrome::popup::context_menu(&owner, || self.context_items(s))
            {
                self.grid.active = s;
                self.focus_grid = Some(s);
                self.command(&action, &ctx, prefs);
            }
        }
        self.dialogs(&ctx, prefs);
        self.restore_grid_focus(&ctx);
        // A drop while a confirmation, dialog, cell editor or the settings
        // draft is open would overwrite that pending state; ignore it.
        if let Some((side, path)) = crate::chrome::drop::dropped_on(
            &ctx,
            crate::chrome::drop::panes(&self.grid.pane_rects),
            self.grid.active,
        )
        .filter(|_| !blocked && !self.settings.open)
        {
            self.request(Pending::Open(side, path));
        }
        self.action.take()
    }
    /// Snapshot scenes: the reference shows the header row as data with
    /// lettered columns and aligns rows without a key column. The GUI worker
    /// already defaults to that lettered/data look (only an explicit "Column
    /// names" setting promotes row 0); snapshots pin it explicitly, while
    /// inferred keys remain Matcha's own default outside snapshots.
    pub fn snapshot_options(&mut self) {
        for options in &mut self.options {
            options.header = Some(bc_formats::model::HeaderMode::Data);
        }
        // Reference rows pair positionally (no inferred key column).
        self.default_rules.keys = bc_engine_table::rules::KeySelection::Manual(Vec::new());
        self.default_rules.strategy = bc_core::align::AlignStrategy::None;
        self.reload();
    }
    /// Deterministic state of the table snapshot scenes.
    pub fn snapshot_preset(&mut self, scene: &str) {
        self.scroll_to_row(0);
        self.grid.active = 0;
        self.grid.anchor = None;
        if scene == "table-cursor-on-diff" {
            // Reference operation: click the left grid's differing cell (C3).
            self.grid.cursor = Cursor { row: 2, column: 2 };
            self.grid.selection_visible = true;
        } else {
            self.grid.cursor = Cursor::default();
            self.snapshot_select_paths = true;
        }
        if scene == "dlg-table-session-settings" {
            self.settings_open();
            self.settings.placement = Some(crate::chrome::dialog::Placement::ClientOnly);
        }
    }
    fn restore_grid_focus(&mut self, ctx: &egui::Context) {
        if self.editor.is_none() && self.pending.is_none() && self.dialog.is_none() {
            if let Some(side) = self.focus_grid.take() {
                ctx.memory_mut(|m| m.request_focus(egui::Id::unique(("table_grid_focus", side))));
                // Every "focus the grid" flow (go-to, navigate, sort, edit
                // commit/cancel, ...) also hands it the keyboard.
                self.key_focus = true;
            }
        }
    }
}
