use super::cache::Cache;
use bc_core::{align::AlignmentOverrides, cancel::CancelCheck, CoreError};
use bc_engine_table::{
    jobs::TableAlignJob,
    map::TableAlignOutput,
    sheets::{SheetPair, SheetPairs},
    source::{detect_header, read_path, ReadOptions},
    workbook::Workbook,
    Sheet, TableRules,
};
use bc_formats::model::HeaderMode;
use bc_jobs::{CancelToken, Job, JobContext, JobError, JobId, JobResult, Progress, ProgressSink};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    thread::JoinHandle,
    time::{Duration, Instant},
};
pub struct Prepared {
    pub left: Sheet,
    pub right: Sheet,
    pub output: TableAlignOutput,
    pub cache: Cache,
}
pub struct Loaded {
    pub prepared: Prepared,
    pub names: [Vec<String>; 2],
    pub pairs: SheetPairs,
    pub times: [Duration; 2],
    pub bytes: [u64; 2],
    pub active: usize,
}
/// A side that must be served from memory (unedited in-memory sheet with its
/// sheet list), e.g. the untouched side of a single-side Open File.
pub struct MemorySheet {
    pub sheet: Sheet,
    pub names: Vec<String>,
    pub index: usize,
}
/// Pairing after a single-side replacement: by-name over the new side's sheets
/// and the preserved side's existing sheet list; the active pair keeps the
/// previously viewed preserved-side sheet (forcing a pairing with the new
/// side's first sheet when no name matches), so every preserved-side sheet
/// keeps a tab.
pub fn pair_for_replacement(
    new_names: &[String],
    other_names: &[String],
    new_side: usize,
    prefer_other: usize,
    left_source: bool,
) -> (SheetPairs, usize) {
    let (left, right) = if new_side == 0 {
        (new_names, other_names)
    } else {
        (other_names, new_names)
    };
    let mut pairs = if left.len() == 1 && right.len() == 1 {
        SheetPairs(vec![SheetPair {
            left: Some(0),
            right: Some(0),
        }])
    } else {
        SheetPairs::by_name(left, right, left_source)
    };
    let side_of = |p: &SheetPair| if new_side == 0 { p.left } else { p.right };
    let other_of = |p: &SheetPair| if new_side == 0 { p.right } else { p.left };
    let active = pairs
        .0
        .iter()
        .position(|p| other_of(p) == Some(prefer_other) && side_of(p).is_some())
        .or_else(|| {
            pairs
                .0
                .iter()
                .position(|p| p.left.is_some() && p.right.is_some())
        })
        .unwrap_or_else(|| {
            // No usable name match: compare the new side's first sheet against
            // the previously viewed sheet. A previous holder of the new side's
            // first sheet is removed when it is a pure orphan, demoted when it
            // pairs with another preserved sheet.
            if let Some(h) = pairs.0.iter().position(|p| side_of(p) == Some(0)) {
                if other_of(&pairs.0[h]).is_none() {
                    pairs.0.remove(h);
                } else if new_side == 0 {
                    pairs.0[h].left = None;
                } else {
                    pairs.0[h].right = None;
                }
            }
            if let Some(p) = pairs
                .0
                .iter_mut()
                .find(|p| other_of(p) == Some(prefer_other))
            {
                if new_side == 0 {
                    p.left = Some(0);
                } else {
                    p.right = Some(0);
                }
            } else {
                pairs.0.push(if new_side == 0 {
                    SheetPair {
                        left: Some(0),
                        right: Some(prefer_other),
                    }
                } else {
                    SheetPair {
                        left: Some(prefer_other),
                        right: Some(0),
                    }
                });
            }
            pairs
                .0
                .iter()
                .position(|p| side_of(p) == Some(0) && other_of(p) == Some(prefer_other))
                .unwrap_or(0)
        });
    (pairs, active)
}
pub enum Output {
    Loaded(Box<Loaded>),
    Aligned(Box<Prepared>),
    Projected(Box<Cache>, (u64, u64), TableRules, bool),
    Scanned(Vec<[bc_engine_table::map::TableStatistics; 3]>),
}
type SideLoad = (Result<(Sheet, Vec<String>), JobError>, Duration);
fn error(e: CoreError) -> JobError {
    match e {
        CoreError::Cancelled => JobError::Cancelled,
        e => JobError::Failed(e.to_string()),
    }
}
pub fn prepare(
    job: Box<dyn Job>,
    left: Sheet,
    right: Sheet,
    ctx: &JobContext,
) -> Result<Prepared, JobError> {
    let t = Instant::now();
    let output = *job
        .run(ctx)?
        .downcast::<TableAlignOutput>()
        .map_err(|_| JobError::Failed("unexpected table result".into()))?;
    crate::perf::span("worker:table-align-job", t);
    let t = Instant::now();
    let cache = Cache::build(&output, &left, &right, &ctx.cancel).map_err(error)?;
    crate::perf::span("worker:cache-build", t);
    Ok(Prepared {
        left,
        right,
        output,
        cache,
    })
}
pub struct AlignJob {
    pub job: Box<dyn Job>,
    pub left: Sheet,
    pub right: Sheet,
}
impl Job for AlignJob {
    fn name(&self) -> String {
        "Comparing table".into()
    }
    fn run(self: Box<Self>, ctx: &JobContext) -> JobResult {
        Ok(Box::new(Output::Aligned(Box::new(prepare(
            self.job, self.left, self.right, ctx,
        )?))))
    }
}
pub struct ProjectJob {
    pub fit_all: bool,
    pub output: TableAlignOutput,
    pub left: Sheet,
    pub right: Sheet,
}
impl Job for ProjectJob {
    fn name(&self) -> String {
        "Projecting table".into()
    }
    fn run(self: Box<Self>, ctx: &JobContext) -> JobResult {
        let mut cache =
            Cache::build(&self.output, &self.left, &self.right, &ctx.cancel).map_err(error)?;
        if self.fit_all {
            cache
                .fit_all(&self.output, &self.left, &self.right, &ctx.cancel)
                .map_err(error)?;
        }
        Ok(Box::new(Output::Projected(
            Box::new(cache),
            self.output.revisions,
            self.output.rules.clone(),
            self.fit_all,
        )))
    }
}
fn load(
    path: &Option<PathBuf>,
    options: &ReadOptions,
    index: Option<usize>,
    ctx: &JobContext,
) -> Result<(Sheet, Vec<String>), JobError> {
    let Some(path) = path else {
        return Ok((Sheet::default(), vec!["Clipboard".into()]));
    };
    CancelCheck::check(&ctx.cancel).map_err(error)?;
    let mut resolved = options.clone();
    if resolved.format.regional.use_system {
        resolved.system_regional = super::regional::host();
    }
    let options = &resolved;
    let workbook = path
        .extension()
        .is_some_and(|e| matches!(e.to_string_lossy().to_lowercase().as_str(), "xlsx" | "xlsm"));
    if workbook {
        let t = Instant::now();
        let names = {
            let book = Workbook::open(path, Arc::new(ctx.cancel.clone())).map_err(error)?;
            book.sheet_names().to_vec()
        };
        crate::perf::span("worker:workbook-open", t);
        let t = Instant::now();
        let mut sheet = if let Some(name) = index.and_then(|i| names.get(i)) {
            // Values/styles, formulas and merges are three independent
            // decompress+parse streams over the same zip entry; run them
            // concurrently (identical result to the serial sequence).
            bc_engine_table::workbook::load_sheet_parallel(
                path,
                name,
                Arc::new(ctx.cancel.clone()),
            )
            .map_err(error)?
        } else {
            Sheet::default()
        };
        crate::perf::span("worker:load-sheet", t);
        let t = Instant::now();
        sheet.regional = if options.format.regional.use_system {
            options.system_regional.clone()
        } else {
            options.format.regional.clone()
        };
        let header = options.header.unwrap_or(options.format.header);
        if header == HeaderMode::Names || (header == HeaderMode::Auto && detect_header(&sheet)) {
            sheet.take_header();
        }
        crate::perf::span("worker:header-detect", t);
        Ok((sheet, names))
    } else {
        let output = read_path(path, options, &ctx.cancel).map_err(error)?;
        let name = output.sheet.name.clone();
        Ok((
            if index.is_some() {
                output.sheet
            } else {
                Sheet::default()
            },
            vec![name],
        ))
    }
}
pub struct LoadJob {
    pub paths: [Option<PathBuf>; 2],
    pub options: [ReadOptions; 2],
    pub rules: TableRules,
    pub pair: Option<SheetPair>,
    pub left_source: bool,
    pub memory: [Option<MemorySheet>; 2],
    /// 用户在 Sheets 页排好的手动配对：逐字使用，不按名重新推导。
    pub fixed_pairs: Option<SheetPairs>,
}
impl Job for LoadJob {
    fn name(&self) -> String {
        "Loading table sources".into()
    }
    fn run(self: Box<Self>, ctx: &JobContext) -> JobResult {
        let mut times = [Duration::ZERO; 2];
        let mut bytes = [0; 2];
        let indices = self
            .pair
            .as_ref()
            .map(|p| [p.left, p.right])
            .unwrap_or([Some(0), Some(0)]);
        let mut sheets: [Option<Sheet>; 2] = [None, None];
        let mut names: [Vec<String>; 2] = [Vec::new(), Vec::new()];
        // Initial open reads both files from disk; the two parses are
        // independent, so run them concurrently. A side served from memory
        // (single-side replacement) keeps the original serial path.
        let mem_hit = |s: usize| matches!(&self.memory[s], Some(m) if indices[s] == Some(m.index));
        if !mem_hit(0) && !mem_hit(1) {
            let mut results: [Option<SideLoad>; 2] = [None, None];
            let this: &LoadJob = &self;
            std::thread::scope(|scope| {
                let mut handles = Vec::new();
                for (s, &index) in indices.iter().enumerate() {
                    let cancel = ctx.cancel.clone();
                    let handle = scope.spawn(move || {
                        let start = Instant::now();
                        let result = load(&this.paths[s], &this.options[s], index, ctx);
                        // A failing side cancels its sibling's parse early.
                        if result.is_err() {
                            cancel.cancel();
                        }
                        crate::perf::span(&format!("worker:load-side-{s}"), start);
                        (s, result, start.elapsed())
                    });
                    handles.push(handle);
                }
                for handle in handles {
                    let (s, result, elapsed) = handle
                        .join()
                        .unwrap_or_else(|_| panic!("table load thread panicked"));
                    results[s] = Some((result, elapsed));
                }
            });
            for s in 0..2 {
                let (result, elapsed) = results[s].take().unwrap();
                times[s] = elapsed;
                let (sheet, n) = result?;
                sheets[s] = Some(sheet);
                names[s] = n;
            }
        } else {
            for s in 0..2 {
                if let Some(memory) = &self.memory[s] {
                    names[s] = memory.names.clone();
                    if indices[s] == Some(memory.index) {
                        sheets[s] = Some(memory.sheet.clone());
                        continue;
                    }
                }
                let start = Instant::now();
                let (sheet, n) = load(&self.paths[s], &self.options[s], indices[s], ctx)?;
                times[s] = start.elapsed();
                crate::perf::span(&format!("worker:load-side-{s}"), start);
                sheets[s] = Some(sheet);
                if self.memory[s].is_none() {
                    names[s] = n;
                }
            }
        }
        let mut pairs = self
            .fixed_pairs
            .clone()
            .unwrap_or_else(|| SheetPairs::by_name(&names[0], &names[1], self.left_source));
        // Two single-sheet text sources compare regardless of their filenames.
        if self.fixed_pairs.is_none() && names[0].len() == 1 && names[1].len() == 1 {
            pairs = SheetPairs(vec![SheetPair {
                left: Some(0),
                right: Some(0),
            }]);
        }
        // Choose the active pair: an explicit pair (sheet switch) wins; with a
        // memory side the shared replacement-pairing helper keeps the preserved
        // sheet active and a tab for every preserved sheet; otherwise the first.
        let active = if let Some(p) = &self.pair {
            pairs.0.iter().position(|x| x == p).unwrap_or(0)
        } else if let Some((mem_side, mem)) = self
            .memory
            .iter()
            .enumerate()
            .find_map(|(s, m)| m.as_ref().map(|m| (s, m)))
        {
            let (new_pairs, chosen) = pair_for_replacement(
                &names[1 - mem_side],
                &mem.names,
                1 - mem_side,
                mem.index,
                self.left_source,
            );
            pairs = new_pairs;
            chosen
        } else {
            0
        };
        let chosen = pairs.0.get(active).cloned().unwrap_or(SheetPair {
            left: Some(0),
            right: Some(0),
        });
        // Reload whichever side the chosen pair does not already have loaded.
        for s in 0..2 {
            let want = if s == 0 { chosen.left } else { chosen.right };
            if let Some(memory) = &self.memory[s] {
                if want == Some(memory.index) {
                    sheets[s] = Some(memory.sheet.clone());
                    continue;
                }
            }
            if want != indices[s] || sheets[s].is_none() {
                let start = Instant::now();
                let (sheet, _) = load(&self.paths[s], &self.options[s], want, ctx)?;
                times[s] += start.elapsed();
                crate::perf::span(&format!("worker:reload-side-{s}"), start);
                sheets[s] = Some(sheet);
            }
        }
        for (s, path) in self.paths.iter().enumerate() {
            bytes[s] = path
                .as_ref()
                .and_then(|p| std::fs::metadata(p).ok())
                .map_or(0, |m| m.len());
        }
        let left = sheets[0].take().unwrap_or_default();
        let right = sheets[1].take().unwrap_or_default();
        let mut rules = self.rules;
        if rules
            .validate(left.column_count(), right.column_count())
            .is_err()
        {
            // A format/path change can invalidate the resolved mapping; fall
            // back to automatic mapping and key inference rather than failing
            // the whole reload (same contract as TableMap::replace_sheet).
            rules.columns.0.clear();
            rules.keys = bc_engine_table::rules::KeySelection::Automatic {
                columns: Vec::new(),
                score: 0.0,
            };
        }
        let job = TableAlignJob {
            left: left.clone(),
            right: right.clone(),
            rules,
            overrides: AlignmentOverrides::default(),
        };
        let prepared = prepare(Box::new(job), left, right, ctx)?;
        Ok(Box::new(Output::Loaded(Box::new(Loaded {
            prepared,
            names,
            pairs,
            times,
            bytes,
            active,
        }))))
    }
}
pub struct ScanJob {
    pub paths: [Option<PathBuf>; 2],
    pub options: [ReadOptions; 2],
    pub pairs: SheetPairs,
    pub rules: TableRules,
}
impl Job for ScanJob {
    fn name(&self) -> String {
        "Comparing other sheets".into()
    }
    fn run(self: Box<Self>, ctx: &JobContext) -> JobResult {
        let mut states = Vec::new();
        for pair in &self.pairs.0 {
            CancelCheck::check(&ctx.cancel).map_err(error)?;
            let left = load(&self.paths[0], &self.options[0], pair.left, ctx)?.0;
            let right = load(&self.paths[1], &self.options[1], pair.right, ctx)?.0;
            let mut rules = self.rules.clone();
            rules.columns.0.clear();
            rules.keys = bc_engine_table::rules::KeySelection::Automatic {
                columns: Vec::new(),
                score: 0.0,
            };
            let job = TableAlignJob {
                left,
                right,
                rules,
                overrides: Default::default(),
            };
            let output = *Box::new(job)
                .run(ctx)?
                .downcast::<TableAlignOutput>()
                .map_err(|_| JobError::Failed("unexpected sheet output".into()))?;
            states.push(
                [
                    bc_engine_table::rules::Layers::Values,
                    bc_engine_table::rules::Layers::ValuesAndFormulas,
                    bc_engine_table::rules::Layers::All,
                ]
                .map(|layer| output.data.statistics(layer, false)),
            );
        }
        Ok(Box::new(Output::Scanned(states)))
    }
}
#[derive(Default)]
struct State(Mutex<String>);
impl ProgressSink for State {
    fn report(&self, _: JobId, p: &Progress) {
        if let Ok(mut text) = self.0.lock() {
            text.clone_from(&p.message);
        }
    }
}
pub struct Worker {
    cancel: CancelToken,
    handle: Option<JoinHandle<Result<Output, JobError>>>,
    progress: Arc<State>,
}
impl Worker {
    pub fn spawn(job: Box<dyn Job>) -> Self {
        let cancel = CancelToken::new();
        let progress = Arc::new(State::default());
        let ctx = JobContext {
            id: JobId(1),
            cancel: cancel.clone(),
            progress: progress.clone(),
            interaction: None,
        };
        let handle = std::thread::spawn(move || {
            job.run(&ctx)?
                .downcast::<Output>()
                .map(|o| *o)
                .map_err(|_| JobError::Failed("unexpected worker payload".into()))
        });
        Self {
            cancel,
            progress,
            handle: Some(handle),
        }
    }
    pub fn poll(&mut self) -> Option<Result<Output, JobError>> {
        if !self.handle.as_ref()?.is_finished() {
            return None;
        }
        Some(
            self.handle
                .take()
                .unwrap()
                .join()
                .unwrap_or_else(|_| Err(JobError::Failed("table worker panicked".into()))),
        )
    }
    pub fn cancel(&self) {
        self.cancel.cancel();
    }
    pub fn progress(&self) -> String {
        self.progress
            .0
            .try_lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| "Comparing".into())
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
