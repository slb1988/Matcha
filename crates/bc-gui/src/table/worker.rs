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
}
pub enum Output {
    Loaded(Box<Loaded>),
    Aligned(Box<Prepared>),
    Projected(Box<Cache>, (u64, u64), TableRules, bool),
    Scanned(Vec<[bc_engine_table::map::TableStatistics; 3]>),
}
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
    let output = *job
        .run(ctx)?
        .downcast::<TableAlignOutput>()
        .map_err(|_| JobError::Failed("unexpected table result".into()))?;
    let cache = Cache::build(&output, &left, &right, &ctx.cancel).map_err(error)?;
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
        let mut book = Workbook::open(path, Arc::new(ctx.cancel.clone())).map_err(error)?;
        let names = book.sheet_names().to_vec();
        let mut sheet = if let Some(name) = index.and_then(|i| names.get(i)) {
            book.load_sheet(name).map_err(error)?
        } else {
            Sheet::default()
        };
        sheet.regional = if options.format.regional.use_system {
            options.system_regional.clone()
        } else {
            options.format.regional.clone()
        };
        let header = options.header.unwrap_or(options.format.header);
        if header == HeaderMode::Names || (header == HeaderMode::Auto && detect_header(&sheet)) {
            sheet.take_header();
        }
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
        let start = Instant::now();
        let (left, ln) = load(&self.paths[0], &self.options[0], indices[0], ctx)?;
        times[0] = start.elapsed();
        let start = Instant::now();
        let (mut right, rn) = load(&self.paths[1], &self.options[1], indices[1], ctx)?;
        times[1] = start.elapsed();
        let mut pairs = SheetPairs::by_name(&ln, &rn, self.left_source);
        // Two single-sheet text sources compare regardless of their filenames.
        if ln.len() == 1 && rn.len() == 1 {
            pairs = SheetPairs(vec![SheetPair {
                left: Some(0),
                right: Some(0),
            }]);
        }
        let mut left = left;
        if self.pair.is_none() {
            if let Some(pair) = pairs.0.first() {
                if pair.left != indices[0] {
                    left = load(&self.paths[0], &self.options[0], pair.left, ctx)?.0;
                }
                if pair.right != indices[1] {
                    right = load(&self.paths[1], &self.options[1], pair.right, ctx)?.0;
                }
            }
        }
        for (s, path) in self.paths.iter().enumerate() {
            bytes[s] = path
                .as_ref()
                .and_then(|p| std::fs::metadata(p).ok())
                .map_or(0, |m| m.len());
        }
        let job = TableAlignJob {
            left: left.clone(),
            right: right.clone(),
            rules: self.rules,
            overrides: AlignmentOverrides::default(),
        };
        let prepared = prepare(Box::new(job), left, right, ctx)?;
        Ok(Box::new(Output::Loaded(Box::new(Loaded {
            prepared,
            names: [ln, rn],
            pairs,
            times,
            bytes,
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
