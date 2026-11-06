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
pub(super) struct CellEditor {
    pub side: usize,
    pub source: Cursor,
    pub text: String,
    pub focus: bool,
}
pub(super) struct Pair {
    pub map: TableMap,
    pub history: [EditStack; 2],
    pub cache: Cache,
    pub projection: Projection,
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
    pub(super) scan: Option<Worker>,
    pub(super) expected_rules: Option<TableRules>,
    pub(super) edited_at: Option<Instant>,
    pub(super) grid: GridState,
    pub(super) filter: Filter,
    pub(super) hide_same: bool,
    pub(super) show_details: bool,
    pub(super) show_info: bool,
    pub(super) show_thumbnail: bool,
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
            scan: None,
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
        }
    }
    pub fn open(left: &Path, right: &Path) -> Result<Self, CoreError> {
        for path in [left, right] {
            if !std::fs::metadata(path)?.is_file() {
                return Err(CoreError::Invalid("Choose two files".into()));
            }
        }
        let mut view = Self::blank();
        view.files[0].path = Some(left.into());
        view.files[1].path = Some(right.into());
        for (s, path) in [left, right].iter().enumerate() {
            view.options[s].format.detect_delimiter = true;
            if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("tsv"))
            {
                view.options[s].format.delimiters = vec!['\t'];
                view.options[s].format.detect_delimiter = false;
            }
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
        self.files.iter().any(|f| f.clipboard_dirty)
            || self
                .pair
                .iter()
                .chain(self.cached.values())
                .any(|p| p.history.iter().any(EditStack::is_dirty))
    }
    pub fn is_ready(&self) -> bool {
        self.worker.is_none()
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
        drop(self.worker.take());
        drop(self.scan.take());
        self.edited_at = None;
        self.expected_rules = None;
    }
    pub(super) fn reload(&mut self) {
        self.cancel();
        self.pair = None;
        self.cached.clear();
        self.active_pair = 0;
        self.grid.scroll = egui::Vec2::ZERO;
        self.default_rules.columns.0.clear();
        self.worker = Some(Worker::spawn(Box::new(LoadJob {
            paths: self.paths(),
            options: self.options.clone(),
            rules: self.default_rules.clone(),
            pair: None,
            left_source: self.left_source,
        })));
        self.message = "Loading and comparing in background…".into();
    }
    fn install(&mut self, prepared: Prepared) -> Result<(), CoreError> {
        let Prepared {
            left,
            right,
            output,
            cache,
        } = prepared;
        if let Some(pair) = &mut self.pair {
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
        } else {
            let mut map = TableMap::from_output(left, right, output)?;
            map.set_deferred_edits(true);
            let projection = cache.project(self.filter);
            self.pair = Some(Pair {
                map,
                history: [EditStack::new(), EditStack::new()],
                cache,
                projection,
            });
        }
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
        Ok(())
    }
    pub fn poll(&mut self) {
        if let Some(result) = self.worker.as_mut().and_then(Worker::poll) {
            self.worker = None;
            match result {
                Ok(Output::Loaded(loaded)) => {
                    let loaded = *loaded;
                    if self.names[0].is_empty() {
                        self.names = loaded.names;
                        self.pairs = loaded.pairs;
                        self.states = vec![None; self.pairs.0.len()];
                        self.summaries = vec![None; self.pairs.0.len()];
                    }
                    for s in 0..2 {
                        self.files[s].byte_len = loaded.bytes[s];
                        self.files[s].load_time = loaded.times[s];
                    }
                    if let Err(e) = self.install(loaded.prepared) {
                        self.message = e.to_string();
                    }
                    if self.pairs.0.len() > 1 && self.scan.is_none() {
                        self.scan = Some(Worker::spawn(Box::new(ScanJob {
                            paths: self.paths(),
                            options: self.options.clone(),
                            pairs: self.pairs.clone(),
                            rules: self.default_rules.clone(),
                        })));
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
                Err(bc_jobs::JobError::Cancelled) => {}
                Err(e) => {
                    self.message = e.to_string();
                    self.edited_at = None;
                }
            }
        }
        if let Some(result) = self.scan.as_mut().and_then(Worker::poll) {
            self.scan = None;
            if let Ok(Output::Scanned(states)) = result {
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
        }
        if self.worker.is_none()
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
    pub(super) fn recompare(&mut self) {
        self.cancel();
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
        self.cancel();
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
                if i < self.states.len() {
                    self.states[i] = Some(p.map.output().data.importance(rules.layers, false));
                }
            }
            if self.active_pair < self.states.len() {
                self.states[self.active_pair] =
                    Some(p.map.output().data.importance(rules.layers, false));
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
            self.pair = Some(p);
            self.refresh_grid();
            if self.pair.as_ref().is_some_and(|p| p.map.is_stale()) {
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
            })));
        }
    }
    pub(super) fn goto_cell(&mut self, cursor: Cursor) {
        if let Some(pair) = &mut self.pair {
            self.grid.cursor = cursor;
            self.grid.anchor = None;
            if self.filter == Filter::Same || self.filter == Filter::None {
                self.filter = Filter::All;
                pair.projection = pair.cache.project(self.filter);
            }
            self.grid.scroll.y = pair.projection.display_for(cursor.row) as f32 * ROW_HEIGHT;
            if let Some(d) = self.grid.columns.iter().position(|&c| c == cursor.column) {
                self.grid.scroll.x = self.grid.offsets[d];
            }
        }
    }
    fn settings_open(&mut self) {
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
        self.keyboard(&ctx, prefs);
        self.toolbar(ui, prefs);
        if let Some(action) = self.find.ui(ui) {
            if let Err(e) = self.find_action(action) {
                self.message = e.to_string();
            }
        }
        if let Some(draft) = self.settings.ui(&ctx, &self.names) {
            self.request(Pending::Settings(Box::new(draft)));
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
            if let Some(p)=&self.pair{let s=p.cache.statistics;ui.label(format!("{} same · {} unimportant · {} important · {} left-only · {} right-only · {} columns · keys: {}{}",s.same,s.unimportant,s.important,s.left_only,s.right_only,s.columns,p.map.rules().keys.label(),if p.map.is_stale(){" · stale (recomparing)"}else{""}));}
            ui.label(self.worker.as_ref().map(Worker::progress).filter(|s|!s.is_empty()).unwrap_or_else(||self.message.clone()));
        });
        self.grid.rows_drawn = 0;
        self.grid.cells_drawn = 0;
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
                    if let Some(p) = &self.pair {
                        let row = self.goto[0]
                            .saturating_sub(1)
                            .min(p.cache.total.saturating_sub(1));
                        let column = self.goto[1]
                            .saturating_sub(1)
                            .min(p.map.column_count().saturating_sub(1));
                        self.goto_cell(Cursor { row, column });
                    }
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
                ui.menu_button("View", |ui| {
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
            });
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
