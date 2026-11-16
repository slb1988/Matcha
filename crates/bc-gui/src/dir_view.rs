//! Folder view: worker-owned trees, latest-only immutable snapshots and virtual rows.
//! The UI never enumerates directories, compares contents or executes file operations.
use crate::{
    chrome::menubar::{MenuItem, MenuItemKind, MenuSpec},
    dialogs::{
        self,
        find::FindDialog,
        operation::{Kind, OperationDialog},
        paths::PathsDialog,
    },
    egui,
    filters_bar::FiltersBar,
    legend,
    log_panel::LogPanel,
    text::{TextCompareView, ViewAction},
};
use bc_engine_dir::{
    criteria::ContentCompare,
    filter::{ContentFilter, FilterClock, FolderDisplay},
    filter_script::{FilterPresets, FilterSettings},
    jobs::ContentCompareJob,
    op_exec::{self, ConfirmPolicy, OpLog},
    ops::{self, DirOp, OpPlan, PlanError, PlanOptions, Selection, Target},
    refresh::{self, AutoRefresh},
    sync::{SyncBaseline, SyncChoice, SyncMode, SyncOptions, SyncPlan, SyncStatistics},
    tree::{AlignmentRule, CompareStatus, DirTree, NodeId, PairingOptions},
    view::ViewOptions,
    Criteria,
};
use bc_jobs::{CancelToken, Confirmation, Interaction, JobContext, JobId, Progress, ProgressSink};
use bc_session::prefs::{FolderColors, Prefs};
use bc_vfs::{EntryKind, LocalFs, VfsEntry, VfsError, VfsManager, VfsPath};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering},
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const ROW_HEIGHT: f32 = 22.0;
static NEXT_VIEW: AtomicU64 = AtomicU64::new(1);
pub type FolderNavigation = (bool, [PathBuf; 2]);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    Name,
    Size,
    Modified,
    Crc,
    Version,
    Attrs,
    Owner,
    Group,
    Vcs,
}
impl Column {
    pub const OPTIONAL: [Self; 8] = [
        Self::Size,
        Self::Modified,
        Self::Crc,
        Self::Version,
        Self::Attrs,
        Self::Owner,
        Self::Group,
        Self::Vcs,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Size => "Size",
            Self::Modified => "Modified (UTC)",
            Self::Crc => "CRC32",
            Self::Version => "Version",
            Self::Attrs => "Attrs",
            Self::Owner => "Owner",
            Self::Group => "Group",
            Self::Vcs => "VCS",
        }
    }
    fn width(self) -> f32 {
        match self {
            Self::Modified => 155.0,
            Self::Version => 110.0,
            Self::Attrs => 50.0,
            _ => 85.0,
        }
    }
}
#[derive(Clone)]
pub struct RenderSide {
    pub node: NodeId,
    pub name: String,
    pub label: String,
    pub path: PathBuf,
    pub entry: VfsEntry,
    pub cells: [String; 8],
}
#[derive(Clone)]
pub struct RenderRow {
    pub node: Option<NodeId>,
    pub sides: [Option<RenderSide>; 2],
    pub depth: usize,
    pub status: CompareStatus,
    pub teal: [bool; 2],
    pub directory: bool,
    pub expanded: bool,
    pub pending: bool,
    pub red: bool,
    pub purple: bool,
    pub ignored: bool,
    pub center: String,
    pub conflict: bool,
    /// Sync plan marks of this row (`SYNC_*` bits), drawn in the arrow column.
    pub sync_mark: u8,
}
pub const SYNC_TO_RIGHT: u8 = 1;
pub const SYNC_TO_LEFT: u8 = 2;
pub const SYNC_DELETE_LEFT: u8 = 4;
pub const SYNC_DELETE_RIGHT: u8 = 8;
pub const SYNC_CONFLICT: u8 = 16;
/// Per-direction totals of the sync plan (operations area summary lines).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SyncSummary {
    /// (files, folders, bytes) copied to the left / right side.
    pub to_left: (usize, usize, u64),
    pub to_right: (usize, usize, u64),
    /// (files, folders) deleted on the left / right side.
    pub delete_left: (usize, usize),
    pub delete_right: (usize, usize),
    /// Unresolved conflicts (files / folders) that need a decision.
    pub prompts: (usize, usize),
}
#[derive(Clone)]
pub struct Snapshot {
    pub rows: Vec<RenderRow>,
    pub visible: BTreeSet<(NodeId, usize)>,
    pub positions: BTreeMap<(NodeId, usize), usize>,
    pub files: usize,
    pub folders: usize,
    pub paths: [String; 2],
    pub sync_stats: Option<SyncStatistics>,
    pub sync_summary: Option<SyncSummary>,
    pub clock: FilterClock,
    /// 每侧根所在卷的剩余空间标签（如 "50.6 GB free on C:\\"）；查询失败为 None。
    pub free: [Option<String>; 2],
    pub complete: bool,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            visible: BTreeSet::new(),
            positions: BTreeMap::new(),
            files: 0,
            folders: 0,
            paths: Default::default(),
            sync_stats: None,
            sync_summary: None,
            clock: utc_clock(),
            free: Default::default(),
            complete: false,
        }
    }
}
#[derive(Debug, Clone, Copy, Default)]
pub struct DirFrameStats {
    pub rows_drawn: usize,
    pub top_row: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanOptions {
    pub automatic: bool,
    pub scan_orphans: bool,
    pub expand_on_load: bool,
    pub differences_only: bool,
    pub auto_refresh: bool,
}
impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            automatic: true,
            scan_orphans: false,
            expand_on_load: false,
            differences_only: false,
            auto_refresh: false,
        }
    }
}
pub enum Command {
    Shutdown,
    Load([String; 2]),
    Filters {
        names: String,
        other: String,
        content: String,
        exclude_containing: bool,
        view: ViewOptions,
    },
    Rules {
        criteria: Criteria,
        handling: bc_engine_dir::rules::HandlingOptions,
        pairing: PairingOptions,
        scan: ScanOptions,
    },
    Expand {
        nodes: Vec<NodeId>,
        expanded: bool,
        recursive: bool,
    },
    ExpandAll(bool),
    Sort(Column, usize, bool),
    Operate {
        selection: Selection,
        op: DirOp,
        include_hidden: bool,
    },
    Refresh {
        selection: Option<Selection>,
        full: bool,
    },
    Compare {
        selection: Selection,
        mode: ContentCompare,
    },
    Quick([PathBuf; 2], ContentCompare),
    OpenText([PathBuf; 2]),
    Align {
        left: NodeId,
        right: NodeId,
    },
    AlignPattern {
        left: String,
        right: String,
        regex: bool,
        folder: Option<String>,
    },
    SyncMode(SyncMode, bool, bool),
    SyncOverride(NodeId, SyncChoice),
    SyncRun,
    FocusConflict {
        backwards: bool,
        current: Option<NodeId>,
    },
    SaveLog(PathBuf),
    SavePresets(String),
    /// 视图级“显示文件”开关（参考工具栏 Files 切换）；只在比较视图生效。
    ShowFiles(bool),
    Details {
        selection: Option<Selection>,
        crc: bool,
        version: bool,
    },
}
struct Work {
    id: u64,
    command: Command,
    cancel: CancelToken,
}
enum Notice {
    Message(String),
    Clipboard(String),
    Quick(String),
    /// Verified file pair; the formal text view holds Rc/RefCell, so it is built on the UI thread.
    Text([PathBuf; 2]),
    Presets(FilterPresets),
    Focus(NodeId),
}
struct Shared {
    latest: Mutex<Option<Arc<Snapshot>>>,
    progress: Mutex<Progress>,
    questions: Mutex<VecDeque<dialogs::Question>>,
    notices: Mutex<VecDeque<Notice>>,
    cancel: Mutex<CancelToken>,
    tokens: Mutex<BTreeMap<u64, CancelToken>>,
    next: AtomicU64,
    closed: AtomicBool,
    busy: AtomicBool,
    ctx: egui::Context,
    log: OpLog,
}
impl Shared {
    fn notify(&self, notice: Notice) {
        self.notices.lock().unwrap().push_back(notice);
        self.ctx.request_repaint();
    }
    fn token(&self) -> CancelToken {
        self.cancel.lock().unwrap().clone()
    }
}
impl ProgressSink for Shared {
    fn report(&self, _: JobId, p: &Progress) {
        *self.progress.lock().unwrap() = p.clone();
        self.ctx.request_repaint();
    }
}
impl Interaction for Shared {
    fn confirm(&self, text: &str) -> Option<Confirmation> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.questions.lock().unwrap().push_back(dialogs::Question {
            text: text.into(),
            cancel: self.token(),
            reply: tx,
        });
        self.ctx.request_repaint();
        loop {
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(answer) => return Some(answer),
                Err(mpsc::RecvTimeoutError::Disconnected) => return Some(Confirmation::No),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if self.closed.load(AtomicOrdering::Relaxed) || self.token().is_cancelled() {
                        return Some(Confirmation::No);
                    }
                }
            }
        }
    }
}
struct Model {
    tree: DirTree,
    vfs: Arc<VfsManager>,
    filters: FilterSettings,
    view: ViewOptions,
    criteria: Criteria,
    handling: bc_engine_dir::rules::HandlingOptions,
    pairing: PairingOptions,
    scan: ScanOptions,
    clock: FilterClock,
    sync_mode: Option<(SyncMode, bool, bool)>,
    sync_plan: Option<SyncPlan>,
    sync_baseline: Option<SyncBaseline>,
    watcher: Option<AutoRefresh>,
    shared: Arc<Shared>,
    last_publish: Instant,
    snapshots: Vec<Arc<Snapshot>>,
    sort: (Column, usize, bool),
    show_files: bool,
    free: [Option<String>; 2],
}
impl Model {
    fn new(shared: Arc<Shared>, sync: bool, bench: bool) -> Self {
        Self {
            tree: DirTree::new(),
            vfs: Arc::new(VfsManager::local_only()),
            filters: Default::default(),
            view: sync_view_options(sync),
            criteria: Criteria::default(),
            handling: Default::default(),
            pairing: PairingOptions::default(),
            scan: ScanOptions {
                scan_orphans: sync || bench,
                // Reference: the sync view opens fully expanded.
                expand_on_load: bench || sync,
                ..Default::default()
            },
            clock: utc_clock(),
            // Reference default sync operation: Update Both.
            sync_mode: sync.then_some((
                SyncMode::Update(bc_engine_dir::sync::SyncDirection::Both),
                false,
                false,
            )),
            sync_plan: None,
            sync_baseline: None,
            watcher: None,
            shared,
            last_publish: Instant::now(),
            snapshots: Vec::new(),
            sort: (Column::Name, 0, false),
            show_files: true,
            free: Default::default(),
        }
    }
    fn context(&self) -> JobContext {
        JobContext {
            id: JobId(1),
            cancel: self.shared.token(),
            progress: self.shared.clone(),
            interaction: Some(self.shared.clone()),
        }
    }
    fn check(&self) -> Result<(), String> {
        if self.shared.token().is_cancelled() || self.shared.closed.load(AtomicOrdering::Relaxed) {
            Err("Cancelled".into())
        } else {
            Ok(())
        }
    }
    fn load(&mut self, paths: [String; 2]) -> Result<(), String> {
        let previous: [String; 2] = std::array::from_fn(|s| {
            self.tree.bases[s]
                .as_ref()
                .map_or(String::new(), ToString::to_string)
        });
        if previous != paths {
            self.sync_baseline = None;
        }
        self.watcher = None;
        self.sync_plan = None;
        let alignment = self.tree.alignment_rules().to_vec();
        self.tree = DirTree::new();
        for rule in alignment {
            self.tree.add_alignment(rule).map_err(|e| e.to_string())?;
        }
        self.tree
            .set_pairing(self.pairing.clone())
            .map_err(|e| e.to_string())?;
        for (side, path) in paths.iter().enumerate() {
            let path = VfsPath::parse(path).map_err(|e| e.to_string())?;
            if self
                .vfs
                .resolve(&path)
                .map_err(|e| e.to_string())?
                .metadata(&path)
                .map_err(|e| e.to_string())?
                .kind
                != EntryKind::Dir
            {
                return Err("Both paths must be folders".into());
            }
            self.tree.bases[side] = Some(path);
        }
        // Local clock and free space come from one host query (~200 ms of
        // process start-up) that runs while the tree is scanned.
        let host = HostProbe::spawn(&paths);
        self.scan_one(0)?;
        self.apply_filters()?;
        self.publish(false);
        if self.scan.automatic {
            self.scan_descendants(0, false)?;
        }
        let (clock, free) = host.join();
        self.clock = clock.unwrap_or_else(|e| {
            self.shared.notify(Notice::Message(format!(
                "Local clock unavailable: {e}; UTC is shown explicitly"
            )));
            utc_clock()
        });
        self.free = free;
        self.tree.evaluate_quick(&self.criteria);
        if self.scan.expand_on_load {
            self.tree.set_expanded(None, true);
        }
        if self.scan.differences_only {
            let paths: Vec<_> = self
                .tree
                .nodes()
                .filter(|(_, n)| n.is_dir && n.status != CompareStatus::Same)
                .map(|(_, n)| n.relative.clone())
                .collect();
            self.tree.set_expanded(Some(&paths), true);
        }
        self.apply_filters()?;
        self.compare_contents(None, self.criteria.content, false)?;
        self.rebuild_sync()?;
        if self.scan.auto_refresh {
            self.watcher =
                Some(AutoRefresh::new(&self.tree, &self.vfs).map_err(|e| e.to_string())?);
        }
        self.publish(true);
        self.shared.notify(Notice::Message("Ready".into()));
        Ok(())
    }
    fn scan_one(&mut self, id: NodeId) -> Result<(), String> {
        self.check()?;
        for side in 0..2 {
            let Some(node) = self.tree.node(id) else {
                continue;
            };
            if id != 0 && node.sides[side].as_ref().map(|e| e.kind) != Some(EntryKind::Dir) {
                continue;
            }
            let place = ops::location(&self.tree, id, side).map_err(|e| e.to_string())?;
            let relative = place.relative.clone();
            let entries = (|| -> Result<Vec<VfsEntry>, String> {
                if let Some(error) = self.tree.node(id).unwrap().sides[side]
                    .as_ref()
                    .and_then(|e| e.error.as_ref())
                {
                    return Err(error.clone());
                }
                if self.criteria.follow_symlinks && id != 0 {
                    let target =
                        std::fs::canonicalize(place.path.to_local_path().ok_or("non-local scan")?)
                            .map_err(|e| e.to_string())?;
                    let mut parent = self.tree.node(id).unwrap().parent;
                    while let Some(ancestor) = parent {
                        let path = ops::location(&self.tree, ancestor, side)
                            .map_err(|e| e.to_string())?
                            .path
                            .to_local_path()
                            .ok_or("non-local scan")?;
                        if std::fs::canonicalize(path).map_err(|e| e.to_string())? == target {
                            return Err("Symbolic-link cycle reaches an ancestor".into());
                        }
                        parent = self.tree.node(ancestor).and_then(|n| n.parent);
                    }
                }
                self.vfs
                    .resolve(&place.path)
                    .map_err(|e| e.to_string())?
                    .list(&place.path)
                    .map_err(|e| e.to_string())
            })();
            let entries = match entries {
                Ok(entries) => entries,
                Err(error) if id != 0 => {
                    let mut entry = self.tree.node(id).unwrap().sides[side].clone().unwrap();
                    entry.error = Some(error);
                    self.tree
                        .update_entry(side, &place.relative, Some(entry))
                        .map_err(|e| e.to_string())?;
                    Vec::new()
                }
                Err(error) => return Err(error),
            };
            self.tree
                .apply_listing(side, &relative, entries)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    fn scan_descendants(&mut self, root: NodeId, force_orphans: bool) -> Result<(), String> {
        let mut queue: VecDeque<_> = self
            .tree
            .node(root)
            .unwrap()
            .children
            .iter()
            .copied()
            .collect();
        let mut seen = BTreeSet::new();
        let mut count = 0;
        while let Some(id) = queue.pop_front() {
            self.check()?;
            let Some(node) = self.tree.node(id) else {
                continue;
            };
            if !node.is_dir {
                continue;
            }
            if node.is_orphan() && !force_orphans && !self.scan.scan_orphans {
                continue;
            }
            if !seen.insert(id) {
                continue;
            }
            self.scan_one(id)?;
            count += 1;
            queue.extend(self.tree.node(id).unwrap().children.iter().copied());
            self.shared.report(
                JobId(1),
                &Progress {
                    current: count,
                    total: None,
                    message: format!("Scanning folders: {count}"),
                },
            );
            if self.last_publish.elapsed() > Duration::from_millis(150) {
                self.apply_filters()?;
                self.publish(false);
            }
        }
        Ok(())
    }
    fn apply_filters(&mut self) -> Result<(), String> {
        let mut content = BTreeMap::new();
        if !self.filters.other.content.is_empty() {
            for (id, node) in self.tree.nodes() {
                for side in 0..2 {
                    if node.sides[side]
                        .as_ref()
                        .is_some_and(|e| e.kind == EntryKind::File)
                    {
                        self.check()?;
                        let path = ops::location(&self.tree, id, side)
                            .map_err(|e| e.to_string())?
                            .path;
                        let mut reader = self
                            .vfs
                            .resolve(&path)
                            .map_err(|e| e.to_string())?
                            .open_read(&path)
                            .map_err(|e| e.to_string())?;
                        content.insert(
                            (id, side),
                            self.filters
                                .other
                                .matches_content(&mut reader, &self.shared.token())
                                .map_err(|e| e.to_string())?,
                        );
                    }
                }
            }
        }
        self.tree.apply_filters(
            &self.filters.names,
            &self.filters.other,
            self.clock,
            |id, side| content.get(&(id, side)).copied().unwrap_or(true),
        );
        Ok(())
    }
    fn compare_contents(
        &mut self,
        selection: Option<&Selection>,
        mode: ContentCompare,
        force: bool,
    ) -> Result<(), String> {
        if mode == ContentCompare::Size && !self.criteria.version {
            return Ok(());
        }
        if self.view.folders == FolderDisplay::Flatten {
            let rows = self.tree.view_rows(ViewOptions {
                display: bc_engine_dir::DisplayFilter::All,
                ..self.view
            });
            for row in rows {
                self.check()?;
                let (Some(left), Some(right)) = (row.sides[0], row.sides[1]) else {
                    continue;
                };
                if selection.is_some_and(|s| {
                    !s.items.contains(&(left, 0)) && !s.items.contains(&(right, 1))
                }) {
                    continue;
                }
                if self.tree.node(left).unwrap().sides[0]
                    .as_ref()
                    .unwrap()
                    .kind
                    != EntryKind::File
                    || self.tree.node(right).unwrap().sides[1]
                        .as_ref()
                        .unwrap()
                        .kind
                        != EntryKind::File
                {
                    continue;
                }
                let revisions = [
                    self.tree.node(left).unwrap().revision,
                    self.tree.node(right).unwrap().revision,
                ];
                let criteria = Criteria {
                    content: mode,
                    skip_content_if_quick_same: !force && self.criteria.skip_content_if_quick_same,
                    ..self.criteria.clone()
                };
                let quick = criteria.quick(
                    self.tree.node(left).unwrap().sides[0].as_ref(),
                    self.tree.node(right).unwrap().sides[1].as_ref(),
                );
                let job = ContentCompareJob {
                    vfs: self.vfs.clone(),
                    left: ops::location(&self.tree, left, 0)
                        .map_err(|e| e.to_string())?
                        .path,
                    right: ops::location(&self.tree, right, 1)
                        .map_err(|e| e.to_string())?
                        .path,
                    criteria,
                    quick_status: quick,
                    node: left,
                    revision: revisions[0],
                };
                match job.compare(&self.shared.token()) {
                    Ok(result) => {
                        self.tree.apply_virtual_content(
                            [left, right],
                            revisions,
                            (!result.skipped).then_some(result.status),
                            result.versions_equal,
                        );
                    }
                    Err(VfsError::Cancelled) => return Err("Cancelled".into()),
                    Err(error) => {
                        self.tree.apply_virtual_content(
                            [left, right],
                            revisions,
                            Some(CompareStatus::Error),
                            None,
                        );
                        self.shared.notify(Notice::Message(error.to_string()));
                    }
                }
            }
            return Ok(());
        }
        let ids: Vec<_> = self
            .tree
            .nodes()
            .filter(|(id, n)| {
                !n.is_dir
                    && n.sides[0]
                        .as_ref()
                        .is_some_and(|e| e.kind == EntryKind::File)
                    && n.sides[1]
                        .as_ref()
                        .is_some_and(|e| e.kind == EntryKind::File)
                    && ((!n.excluded && (!n.filtered[0] || !n.filtered[1]))
                        || (force && self.view.suppress_filters))
                    && selection.is_none_or(|s| s.items.iter().any(|(n, _)| n == id))
            })
            .map(|(id, _)| id)
            .collect();
        for (index, id) in ids.iter().copied().enumerate() {
            self.check()?;
            let node = self.tree.node(id).unwrap();
            let revision = node.revision;
            let job = ContentCompareJob {
                vfs: self.vfs.clone(),
                left: ops::location(&self.tree, id, 0)
                    .map_err(|e| e.to_string())?
                    .path,
                right: ops::location(&self.tree, id, 1)
                    .map_err(|e| e.to_string())?
                    .path,
                criteria: Criteria {
                    content: mode,
                    skip_content_if_quick_same: !force && self.criteria.skip_content_if_quick_same,
                    ..self.criteria.clone()
                },
                quick_status: node.quick_status,
                node: id,
                revision,
            };
            match job.compare(&self.shared.token()) {
                Ok(result) => {
                    result.apply(&mut self.tree);
                }
                Err(VfsError::Cancelled) => return Err("Cancelled".into()),
                Err(error) => {
                    self.tree
                        .apply_content(id, revision, CompareStatus::Error, None);
                    self.shared.notify(Notice::Message(error.to_string()));
                }
            }
            self.shared.report(
                JobId(1),
                &Progress {
                    current: (index + 1) as u64,
                    total: Some(ids.len() as u64),
                    message: format!("Comparing {}", self.tree.node(id).unwrap().relative),
                },
            );
            if self.last_publish.elapsed() > Duration::from_millis(150) {
                self.publish(false);
            }
        }
        Ok(())
    }
    fn rebuild_sync(&mut self) -> Result<(), String> {
        if let Some((mode, create_empty, visible_only)) = self.sync_mode {
            self.sync_plan = None;
            self.sync_plan = Some(
                SyncPlan::build_with(
                    &self.tree,
                    mode,
                    SyncOptions {
                        create_empty,
                        visible_only,
                        operation: PlanOptions::from_handling(&self.handling, self.view, None),
                        ..Default::default()
                    },
                    self.sync_baseline.as_ref(),
                )
                .map_err(|e| e.to_string())?,
            );
        }
        Ok(())
    }
    fn publish(&mut self, complete: bool) {
        let (column, side, reverse) = self.sort;
        let mut totals: BTreeMap<NodeId, [u64; 2]> = BTreeMap::new();
        for (_, node) in self.tree.nodes().filter(|(_, n)| !n.is_dir && !n.excluded) {
            if let Some(parent) = node.parent {
                for side in 0..2 {
                    if !node.filtered[side] {
                        totals.entry(parent).or_default()[side] +=
                            node.sides[side].as_ref().and_then(|e| e.size).unwrap_or(0);
                    }
                }
            }
        }
        let directories: Vec<_> = self
            .tree
            .nodes()
            .filter(|(_, n)| n.is_dir)
            .map(|(id, n)| (id, n.parent, n.relative.clone()))
            .collect();
        for (id, parent, _) in directories.iter().rev() {
            let value = totals.get(id).copied().unwrap_or_default();
            if let Some(parent) = parent {
                let sum = totals.entry(*parent).or_default();
                for side in 0..2 {
                    sum[side] += value[side];
                }
            }
        }
        let sizes: BTreeMap<_, _> = directories
            .iter()
            .map(|(id, _, path)| (path.clone(), totals.get(id).copied().unwrap_or_default()))
            .collect();
        self.tree.sort_children_by(|a, b| {
            let folders = b.is_dir.cmp(&a.is_dir);
            let order = if column == Column::Size {
                let size = |n: &bc_engine_dir::DirNode| {
                    if n.is_dir {
                        sizes.get(&n.relative).map(|s| s[side])
                    } else {
                        n.sides[side].as_ref().and_then(|e| e.size)
                    }
                };
                size(a).cmp(&size(b))
            } else {
                compare_entry(a.sides[side].as_ref(), b.sides[side].as_ref(), column)
            };
            folders.then(if reverse { order.reverse() } else { order })
        });
        let actions: BTreeMap<_, _> = self
            .sync_plan
            .as_ref()
            .map(|p| {
                p.items
                    .iter()
                    .map(|i| {
                        (
                            i.node,
                            (
                                choice_label(i.effective()).to_owned(),
                                i.unresolved_conflict(),
                                sync_mark(i),
                            ),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut rows = Vec::new();
        for row in self.tree.view_rows(self.view) {
            let node = row.node.and_then(|id| self.tree.node(id));
            let sides = std::array::from_fn(|side| {
                let id = row.sides[side]?;
                let mut entry = self.tree.node(id)?.sides[side].clone()?;
                if entry.kind == EntryKind::Dir
                    && !self.tree.node(id)?.summary.pending
                    && entry.error.is_none()
                {
                    entry.size = Some(totals.get(&id).map_or(0, |s| s[side]));
                }
                let path = ops::location(&self.tree, id, side)
                    .ok()?
                    .path
                    .to_local_path()?;
                let attrs = attrs_label(entry.attrs);
                // 比较与同步视图都按参考用本地时间与空白占位。
                let compare = true;
                let cells = [
                    entry
                        .size
                        .map_or(if compare { String::new() } else { "—".into() }, |s| {
                            s.to_string()
                        }),
                    entry.modified.map_or(
                        if compare { String::new() } else { "—".into() },
                        |t| {
                            if compare {
                                local_time_label(t, &self.clock)
                            } else {
                                time_label(t)
                            }
                        },
                    ),
                    entry.crc32.map_or("—".into(), |c| format!("{c:08X}")),
                    entry.version.clone().unwrap_or("—".into()),
                    attrs.clone(),
                    "—".into(),
                    "—".into(),
                    "—".into(),
                ];
                Some(RenderSide {
                    node: id,
                    label: format!(
                        "{}{}{}",
                        if entry.error.is_some() {
                            "  "
                        } else if entry.kind == EntryKind::Symlink {
                            "-> "
                        } else {
                            ""
                        },
                        entry.name,
                        if attrs.is_empty() {
                            String::new()
                        } else {
                            format!(" ({attrs})")
                        }
                    ),
                    name: entry.name.clone(),
                    path,
                    entry,
                    cells,
                })
            });
            let action = row.node.and_then(|id| actions.get(&id));
            let mut center = if let Some((text, _, _)) = action {
                text.clone()
            } else {
                let content = node.and_then(|n| n.content_status).or_else(|| {
                    if let (Some(left), Some(right)) = (row.sides[0], row.sides[1]) {
                        self.tree.virtual_content_status(left, right)
                    } else {
                        None
                    }
                });
                if let Some(content) = content {
                    status_icon(content).into()
                } else if matches!(
                    row.status,
                    CompareStatus::LeftOrphan | CompareStatus::RightOrphan | CompareStatus::Error
                ) {
                    status_icon(row.status).into()
                } else {
                    ".".into()
                }
            };
            let mut conflict = action.is_some_and(|(_, c, _)| *c);
            let sync_mark = action.map_or(0, |(_, _, m)| *m);
            if node.is_some_and(|n| n.is_dir) {
                if let Some(summary) = row
                    .node
                    .and_then(|id| self.sync_plan.as_ref()?.folder_actions.get(&id))
                {
                    let mut parts = Vec::new();
                    if summary.copy_left_to_right {
                        parts.push(">");
                    }
                    if summary.copy_right_to_left {
                        parts.push("<");
                    }
                    if summary.delete_left {
                        parts.push("xL");
                    }
                    if summary.delete_right {
                        parts.push("xR");
                    }
                    if summary.conflict {
                        parts.push("!");
                    }
                    if !parts.is_empty() {
                        center = parts.join(" ");
                    }
                    conflict |= summary.conflict;
                }
            }
            rows.push(RenderRow {
                node: row.node,
                sides,
                depth: row.depth,
                status: row.status,
                teal: row.teal,
                directory: node.is_some_and(|n| n.is_dir),
                expanded: node.is_some_and(|n| n.expanded),
                pending: node.is_some_and(|n| n.summary.pending),
                red: node.is_some_and(|n| n.summary.red),
                purple: node.is_some_and(|n| n.summary.purple),
                ignored: node.is_some_and(|n| n.ignored),
                center,
                conflict,
                sync_mark,
            });
        }
        // 比较视图的“Files”开关只隐藏文件行；同步视图保持全部行。
        if !self.show_files && self.sync_mode.is_none() {
            rows.retain(|row| row.directory);
        }
        if self.view.folders == FolderDisplay::Flatten {
            rows.sort_by(|a, b| {
                let order = compare_entry(
                    a.sides[side].as_ref().map(|s| &s.entry),
                    b.sides[side].as_ref().map(|s| &s.entry),
                    column,
                );
                if reverse {
                    order.reverse()
                } else {
                    order
                }
            });
        }
        let mut visible = BTreeSet::new();
        let mut positions = BTreeMap::new();
        for (index, row) in rows.iter().enumerate() {
            for (side, item) in row.sides.iter().enumerate() {
                if let Some(item) = item {
                    visible.insert((item.node, side));
                    positions.insert((item.node, side), index);
                }
            }
        }
        let snapshot = Snapshot {
            rows,
            visible,
            positions,
            files: self.tree.nodes().filter(|(_, n)| !n.is_dir).count(),
            folders: self
                .tree
                .nodes()
                .filter(|(id, n)| *id != 0 && n.is_dir)
                .count(),
            paths: std::array::from_fn(|s| {
                self.tree.bases[s]
                    .as_ref()
                    .map_or(String::new(), ToString::to_string)
            }),
            sync_stats: self.sync_plan.as_ref().map(SyncPlan::statistics),
            sync_summary: self.sync_plan.as_ref().map(sync_summary),
            clock: self.clock,
            free: self.free.clone(),
            complete,
        };
        let snapshot = Arc::new(snapshot);
        self.snapshots.retain(|s| Arc::strong_count(s) > 1);
        self.snapshots.push(snapshot.clone());
        *self.shared.latest.lock().unwrap() = Some(snapshot);
        self.shared.ctx.request_repaint();
        self.last_publish = Instant::now();
    }
    fn handle(&mut self, command: Command) -> Result<(), String> {
        self.check()?;
        match command {
            Command::Shutdown => return Ok(()),
            Command::Load(paths) => return self.load(paths),
            Command::Filters {
                names,
                other,
                content,
                exclude_containing,
                view,
            } => {
                let mut settings = FilterSettings {
                    names: bc_engine_dir::NameFilter::parse(&names)?,
                    ..Default::default()
                };
                settings.apply_script(&other, self.clock)?;
                if !content.is_empty() {
                    settings.other.content.push(ContentFilter {
                        text: content,
                        contains: exclude_containing,
                        case_sensitive: false,
                    });
                }
                let compare = self.filters != settings
                    || (self.view.folders == FolderDisplay::Flatten)
                        != (view.folders == FolderDisplay::Flatten);
                if self.sync_mode.is_some() && view.folders == FolderDisplay::Flatten {
                    return Err("Flatten is a compare view, not a sync preview".into());
                }
                self.filters = settings;
                self.view = view;
                if self.view.folders == FolderDisplay::Flatten
                    && self
                        .tree
                        .nodes()
                        .any(|(_, n)| n.is_dir && n.summary.pending)
                {
                    self.scan_descendants(0, true)?;
                }
                self.apply_filters()?;
                if compare {
                    self.compare_contents(None, self.criteria.content, false)?;
                }
                self.rebuild_sync()?;
                self.shared
                    .notify(Notice::Message("Filters applied".into()));
            }
            Command::Rules {
                criteria,
                handling,
                pairing,
                scan,
            } => {
                self.criteria = criteria;
                self.handling = handling;
                self.pairing = pairing;
                self.scan = scan;
                let mut manager = VfsManager::new();
                manager.register(Arc::new(LocalFs {
                    follow_symlinks: self.criteria.follow_symlinks,
                }));
                self.vfs = Arc::new(manager);
                let paths = std::array::from_fn(|s| {
                    self.tree.bases[s]
                        .as_ref()
                        .map_or(String::new(), ToString::to_string)
                });
                return self.load(paths);
            }
            Command::Expand {
                nodes,
                expanded,
                recursive,
            } => {
                let mut paths = Vec::new();
                for id in nodes {
                    if self.tree.node(id).is_none() {
                        continue;
                    }
                    if expanded && self.tree.node(id).unwrap().summary.pending {
                        self.scan_one(id)?;
                        if recursive || self.scan.automatic {
                            self.scan_descendants(id, true)?;
                        }
                    }
                    let mut pending = vec![id];
                    while let Some(id) = pending.pop() {
                        if let Some(node) = self.tree.node(id) {
                            if node.is_dir {
                                paths.push(node.relative.clone());
                                if recursive {
                                    pending.extend(node.children.iter().copied());
                                }
                            }
                        }
                    }
                }
                self.tree.set_expanded(Some(&paths), expanded);
                self.apply_filters()?;
            }
            Command::ExpandAll(expanded) => {
                let pending = self
                    .tree
                    .nodes()
                    .any(|(_, n)| n.is_dir && n.summary.pending);
                if expanded && pending {
                    self.scan_descendants(0, true)?;
                    self.apply_filters()?;
                    self.rebuild_sync()?;
                }
                self.tree.set_expanded(None, expanded);
            }
            Command::Sort(column, side, reverse) => self.sort = (column, side, reverse),
            Command::Operate {
                selection,
                op,
                include_hidden,
            } => {
                let plan = loop {
                    match OpPlan::build(
                        &self.tree,
                        &selection,
                        &op,
                        PlanOptions::from_handling(&self.handling, self.view, Some(include_hidden)),
                        SystemTime::now(),
                    ) {
                        Ok(plan) => break plan,
                        Err(PlanError::ScanRequired { node, .. }) => {
                            self.scan_one(node)?;
                            self.scan_descendants(node, true)?;
                            self.apply_filters()?;
                        }
                        Err(e) => return Err(e.to_string()),
                    }
                };
                let outcome = op_exec::execute(
                    &plan,
                    &self.vfs,
                    &self.context(),
                    ConfirmPolicy::Prompt,
                    &self.shared.log,
                );
                outcome.apply_session(&mut self.tree, &mut self.filters)?;
                if !outcome.filenames.is_empty() {
                    self.shared
                        .notify(Notice::Clipboard(outcome.filename_text()));
                }
                refresh::refresh_locations(
                    &mut self.tree,
                    &self.vfs,
                    &outcome.affected,
                    &bc_core::cancel::NeverCancel,
                )
                .map_err(|e| e.to_string())?;
                self.apply_filters()?;
                let affected = Selection {
                    items: outcome
                        .affected
                        .iter()
                        .filter_map(|p| {
                            p.side.and_then(|side| {
                                self.tree.find_side(&p.relative, side).map(|id| (id, side))
                            })
                        })
                        .collect(),
                };
                self.compare_contents(Some(&affected), self.criteria.content, false)?;
                self.rebuild_sync()?;
                self.shared.notify(Notice::Message(format!(
                    "Completed {}, skipped {}, failed {}, cancelled={}",
                    outcome.completed, outcome.skipped, outcome.failed, outcome.cancelled
                )));
            }
            Command::Refresh { selection, full } => {
                if full {
                    refresh::full_refresh(&mut self.tree, &self.vfs, &self.shared.token())
                        .map_err(|e| e.to_string())?;
                } else if let Some(selection) = selection {
                    refresh::refresh_selection(
                        &mut self.tree,
                        &self.vfs,
                        &selection,
                        &self.shared.token(),
                    )
                    .map_err(|e| e.to_string())?;
                } else {
                    let open: Vec<_> = self
                        .tree
                        .nodes()
                        .filter(|(_, n)| n.is_dir && n.expanded)
                        .map(|(id, _)| id)
                        .collect();
                    for id in open {
                        if self.tree.node(id).is_some() {
                            self.scan_one(id)?;
                        }
                    }
                }
                self.apply_filters()?;
                self.compare_contents(None, self.criteria.content, false)?;
                self.rebuild_sync()?;
            }
            Command::Compare { selection, mode } => {
                self.compare_contents(Some(&selection), mode, true)?;
                self.rebuild_sync()?;
            }
            Command::OpenText(paths) => {
                for path in &paths {
                    if !std::fs::metadata(path)
                        .map_err(|e| format!("{}: {e}", path.display()))?
                        .is_file()
                    {
                        return Err(format!("{} is not a file", path.display()));
                    }
                }
                self.shared.notify(Notice::Text(paths));
                return Ok(());
            }
            Command::Quick(paths, mode) => {
                let left = VfsPath::local(&paths[0]);
                let right = VfsPath::local(&paths[1]);
                let a = self
                    .vfs
                    .resolve(&left)
                    .map_err(|e| e.to_string())?
                    .metadata(&left)
                    .map_err(|e| e.to_string())?;
                let b = self
                    .vfs
                    .resolve(&right)
                    .map_err(|e| e.to_string())?
                    .metadata(&right)
                    .map_err(|e| e.to_string())?;
                let criteria = Criteria {
                    content: mode,
                    timestamp: None,
                    ..Criteria::default()
                };
                let quick = criteria.quick(Some(&a), Some(&b));
                let result = ContentCompareJob {
                    vfs: self.vfs.clone(),
                    left,
                    right,
                    criteria,
                    quick_status: quick,
                    node: 0,
                    revision: 0,
                }
                .compare(&self.shared.token())
                .map_err(|e| e.to_string())?;
                self.shared.notify(Notice::Quick(format!(
                    "{:?}\n{}\n{}\nRead {} + {} bytes",
                    result.status,
                    paths[0].display(),
                    paths[1].display(),
                    result.bytes_read[0],
                    result.bytes_read[1]
                )));
            }
            Command::Align { left, right } => {
                let a = self.tree.node(left).ok_or("stale selection")?;
                let b = self.tree.node(right).ok_or("stale selection")?;
                if a.parent != b.parent {
                    return Err("Align requires the same paired parent folder".into());
                }
                let folder = a
                    .parent
                    .and_then(|id| self.tree.node(id))
                    .map_or(String::new(), |n| n.relative.clone());
                let l = a.sides[0]
                    .as_ref()
                    .ok_or("select a left item")?
                    .name
                    .clone();
                let r = b.sides[1]
                    .as_ref()
                    .ok_or("select a right item")?
                    .name
                    .clone();
                self.tree
                    .align_with(&folder, &l, &r)
                    .map_err(|e| e.to_string())?;
                self.apply_filters()?;
                self.rebuild_sync()?;
            }
            Command::AlignPattern {
                left,
                right,
                regex,
                folder,
            } => {
                self.tree
                    .add_alignment(AlignmentRule::new(&left, &right, regex, folder)?)
                    .map_err(|e| e.to_string())?;
                self.apply_filters()?;
                self.rebuild_sync()?;
            }
            Command::SyncMode(mode, create_empty, visible) => {
                self.sync_mode = Some((mode, create_empty, visible));
                if self
                    .tree
                    .nodes()
                    .any(|(_, n)| n.is_dir && n.summary.pending)
                {
                    self.scan_descendants(0, true)?;
                }
                self.apply_filters()?;
                self.rebuild_sync()?;
            }
            Command::SyncOverride(node, choice) => {
                self.sync_plan
                    .as_mut()
                    .ok_or("No sync preview")?
                    .set_override(node, choice)
                    .map_err(|e| e.to_string())?;
            }
            Command::FocusConflict { backwards, current } => {
                let plan = self.sync_plan.as_ref().ok_or("No sync preview")?;
                let index = current.and_then(|id| plan.items.iter().position(|i| i.node == id));
                let target = if backwards {
                    plan.previous_conflict(index)
                } else {
                    plan.next_conflict(index)
                };
                if let Some(index) = target {
                    let item = &plan.items[index];
                    let node = item.node;
                    let paths: Vec<_> = item
                        .ancestors
                        .iter()
                        .filter_map(|id| self.tree.node(*id))
                        .map(|n| n.relative.clone())
                        .collect();
                    self.tree.set_expanded(Some(&paths), true);
                    if self
                        .tree
                        .view_rows(self.view)
                        .iter()
                        .any(|r| r.node == Some(node))
                    {
                        self.shared.notify(Notice::Focus(node));
                    } else {
                        self.shared.notify(Notice::Message(
                            "Conflict is hidden by the current display filter".into(),
                        ));
                    }
                } else {
                    self.shared
                        .notify(Notice::Message("No further conflict".into()));
                }
            }
            Command::SyncRun => {
                let context = self.context();
                let plan = self.sync_plan.take().ok_or("No sync preview")?;
                let result = plan
                    .execute_and_refresh(
                        &mut self.tree,
                        &self.vfs,
                        &context,
                        ConfirmPolicy::Prompt,
                        &self.shared.log,
                    )
                    .map_err(|e| e.to_string())?;
                if !result.operations.cancelled
                    && result.operations.failed == 0
                    && result.refresh_error.is_none()
                {
                    let captured = SyncBaseline::capture(&self.tree);
                    let baseline = self.sync_baseline.get_or_insert_with(SyncBaseline::default);
                    for item in &plan.items {
                        if item.is_dir {
                            continue;
                        }
                        if let Some(node) = self
                            .tree
                            .find(&item.relative)
                            .and_then(|id| self.tree.node(id))
                        {
                            if !node.ignored
                                && !node.excluded
                                && !item.unresolved_conflict()
                                && matches!(
                                    node.status,
                                    CompareStatus::Same
                                        | CompareStatus::LeftOrphan
                                        | CompareStatus::RightOrphan
                                )
                            {
                                if let Some(entry) = captured.entries.get(&item.relative) {
                                    baseline
                                        .entries
                                        .insert(item.relative.clone(), entry.clone());
                                }
                            }
                        } else if matches!(
                            item.effective(),
                            SyncChoice::DeleteLeft | SyncChoice::DeleteRight
                        ) {
                            baseline.entries.insert(item.relative.clone(), [None, None]);
                        }
                    }
                }
                self.shared.notify(Notice::Message(format!(
                    "Sync: {:?}; cancelled={}; refresh={:?}",
                    result.statistics, result.operations.cancelled, result.refresh_error
                )));
                self.apply_filters()?;
                self.rebuild_sync()?;
            }
            Command::SaveLog(path) => {
                if path.exists()
                    && !matches!(
                        self.shared.confirm("Overwrite the selected log file?"),
                        Some(Confirmation::Yes | Confirmation::YesToAll)
                    )
                {
                    return Ok(());
                }
                self.shared
                    .log
                    .write_to(std::fs::File::create(path).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            }
            Command::Details {
                selection,
                crc,
                version,
            } => {
                let items: Vec<_> = self
                    .tree
                    .nodes()
                    .flat_map(|(id, n)| {
                        (0..2)
                            .filter(|&s| {
                                n.sides[s]
                                    .as_ref()
                                    .is_some_and(|e| e.kind == EntryKind::File)
                            })
                            .map(move |s| (id, s, n.revision))
                    })
                    .filter(|(id, s, _)| {
                        selection
                            .as_ref()
                            .is_none_or(|selected| selected.items.contains(&(*id, *s)))
                    })
                    .collect();
                for (index, (id, side, revision)) in items.iter().copied().enumerate() {
                    self.check()?;
                    let path = ops::location(&self.tree, id, side)
                        .map_err(|e| e.to_string())?
                        .path;
                    let (checksum, version) = bc_engine_dir::jobs::read_entry_details(
                        &self.vfs,
                        &path,
                        crc,
                        version,
                        &self.shared.token(),
                    )
                    .map_err(|e| e.to_string())?;
                    self.tree
                        .cache_details(id, side, revision, checksum, version);
                    self.shared.report(
                        JobId(1),
                        &Progress {
                            current: (index + 1) as u64,
                            total: Some(items.len() as u64),
                            message: format!("Reading columns: {path}"),
                        },
                    );
                }
            }
            Command::SavePresets(xml) => {
                let path = presets_path();
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let temporary = path.with_extension("tmp");
                std::fs::write(&temporary, xml).map_err(|e| e.to_string())?;
                std::fs::rename(temporary, path).map_err(|e| e.to_string())?;
            }
            Command::ShowFiles(show) => {
                self.show_files = show;
            }
        }
        self.publish(true);
        Ok(())
    }
}
fn run_worker(receiver: Receiver<Work>, shared: Arc<Shared>, sync: bool, bench: bool) {
    let mut model = Model::new(shared.clone(), sync, bench);
    if let Ok(xml) = std::fs::read_to_string(presets_path()) {
        if let Ok(presets) = FilterPresets::from_xml(&xml) {
            shared.notify(Notice::Presets(presets));
        }
    }
    loop {
        model.snapshots.retain(|s| Arc::strong_count(s) > 1);
        if shared.closed.load(AtomicOrdering::Relaxed) {
            break;
        }
        match receiver.recv_timeout(Duration::from_millis(750)) {
            Ok(work) => {
                if matches!(work.command, Command::Shutdown) {
                    break;
                }
                *shared.cancel.lock().unwrap() = work.cancel.clone();
                shared.busy.store(true, AtomicOrdering::Relaxed);
                let invalidates = matches!(
                    &work.command,
                    Command::Load(_)
                        | Command::Filters { .. }
                        | Command::Rules { .. }
                        | Command::SyncMode(..)
                        | Command::Refresh { .. }
                        | Command::Align { .. }
                        | Command::AlignPattern { .. }
                        | Command::Compare { .. }
                );
                if let Err(error) = model.handle(work.command) {
                    if invalidates {
                        model.sync_plan = None;
                    }
                    shared.notify(Notice::Message(error));
                    model.publish(false);
                }
                shared.tokens.lock().unwrap().remove(&work.id);
                shared.busy.store(
                    !shared.tokens.lock().unwrap().is_empty(),
                    AtomicOrdering::Relaxed,
                );
                shared.ctx.request_repaint();
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(watcher) = &mut model.watcher {
                    if shared.token().is_cancelled() {
                        *shared.cancel.lock().unwrap() = CancelToken::new();
                    }
                    match watcher.poll(&mut model.tree, &model.vfs, &shared.token()) {
                        Ok(stats) if !stats.changed_nodes.is_empty() => {
                            let affected = Selection::both(stats.changed_nodes.iter().copied());
                            let result = model
                                .apply_filters()
                                .and_then(|_| {
                                    model.compare_contents(
                                        if stats.listings > 0 {
                                            None
                                        } else {
                                            Some(&affected)
                                        },
                                        model.criteria.content,
                                        false,
                                    )
                                })
                                .and_then(|_| model.rebuild_sync());
                            if let Err(error) = result {
                                shared.notify(Notice::Message(error));
                            }
                            model.publish(true);
                        }
                        Ok(_) => {}
                        Err(error) => shared.notify(Notice::Message(error.to_string())),
                    }
                }
            }
        }
    }
}
fn compare_entry(a: Option<&VfsEntry>, b: Option<&VfsEntry>, column: Column) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => {
            let order = match column {
                Column::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                Column::Size => a.size.cmp(&b.size),
                Column::Modified => a.modified.cmp(&b.modified),
                Column::Crc => a.crc32.cmp(&b.crc32),
                Column::Version => a.version.cmp(&b.version),
                Column::Attrs => attrs_label(a.attrs).cmp(&attrs_label(b.attrs)),
                _ => Ordering::Equal,
            };
            order.then(a.name.cmp(&b.name))
        }
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        _ => Ordering::Equal,
    }
}
pub fn status_icon(status: CompareStatus) -> &'static str {
    use CompareStatus::*;
    match status {
        Same => "=",
        Different => "!=",
        LeftNewer => ">",
        RightNewer => "<",
        LeftOrphan => "L",
        RightOrphan => "R",
        SimilarUnimportant => "~",
        Unknown => "?",
        Error => "!",
    }
}
/// Initial view options: the sync view lists only items that differ (the
/// reference preview hides identical files/folders); compare shows all.
fn sync_view_options(sync: bool) -> ViewOptions {
    let mut options = ViewOptions::default();
    if sync {
        options.display = bc_engine_dir::filter::DisplayFilter::Mismatches;
    }
    options
}
/// Arrow-column marks of one sync item.
fn sync_mark(item: &bc_engine_dir::sync::SyncItem) -> u8 {
    if item.unresolved_conflict() {
        return SYNC_CONFLICT;
    }
    match item.effective() {
        SyncChoice::CopyLeftToRight => SYNC_TO_RIGHT,
        SyncChoice::CopyRightToLeft => SYNC_TO_LEFT,
        SyncChoice::DeleteLeft => SYNC_DELETE_LEFT,
        SyncChoice::DeleteRight => SYNC_DELETE_RIGHT,
        _ => 0,
    }
}
/// Plan totals for the operations-area summary (copy counts/bytes, deletes,
/// prompts); bytes are the source side's file sizes.
fn sync_summary(plan: &SyncPlan) -> SyncSummary {
    let mut summary = SyncSummary::default();
    for item in &plan.items {
        let size = |side: usize| item.sides[side].as_ref().and_then(|e| e.size).unwrap_or(0);
        let count = |files: &mut usize, folders: &mut usize| {
            if item.is_dir {
                *folders += 1;
            } else {
                *files += 1;
            }
        };
        if item.unresolved_conflict() {
            let (f, d) = &mut summary.prompts;
            count(f, d);
            continue;
        }
        match item.effective() {
            SyncChoice::CopyLeftToRight => {
                let (f, d, b) = &mut summary.to_right;
                count(f, d);
                if !item.is_dir {
                    *b += size(0);
                }
            }
            SyncChoice::CopyRightToLeft => {
                let (f, d, b) = &mut summary.to_left;
                count(f, d);
                if !item.is_dir {
                    *b += size(1);
                }
            }
            SyncChoice::DeleteLeft => {
                let (f, d) = &mut summary.delete_left;
                count(f, d);
            }
            SyncChoice::DeleteRight => {
                let (f, d) = &mut summary.delete_right;
                count(f, d);
            }
            _ => {}
        }
    }
    summary
}
pub fn choice_label(choice: SyncChoice) -> &'static str {
    match choice {
        SyncChoice::LeaveAlone => "Leave",
        SyncChoice::CopyLeftToRight => "L -> R",
        SyncChoice::CopyRightToLeft => "R -> L",
        SyncChoice::DeleteLeft => "Delete L",
        SyncChoice::DeleteRight => "Delete R",
        SyncChoice::Exclude => "Exclude",
    }
}
fn attrs_label(attrs: bc_vfs::FileAttrs) -> String {
    [
        (attrs.readonly, 'r'),
        (attrs.hidden, 'h'),
        (attrs.archive, 'a'),
        (attrs.system, 's'),
    ]
    .into_iter()
    .filter_map(|(set, c)| set.then_some(c))
    .collect()
}
fn utc_clock() -> FilterClock {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    FilterClock {
        today_midnight: UNIX_EPOCH + Duration::from_secs(seconds / 86400 * 86400),
        utc_offset_seconds: 0,
    }
}
fn presets_path() -> PathBuf {
    std::env::var_os("MATCHA_SETTINGS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("APPDATA")
                .or_else(|| std::env::var_os("HOME"))
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join("Matcha")
        })
        .join("folder-filters.xml")
}
fn time_label(time: SystemTime) -> String {
    let seconds = time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (year, month, day, hour, minute, second) = civil(seconds);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}
/// 参考的 Modified 列：本地时区 `M/D/YYYY H:MM:SS AM/PM`（月/日/时不补零）。
fn local_time_label(time: SystemTime, clock: &FilterClock) -> String {
    let seconds = time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let local = seconds as i64 + i64::from(clock.utc_offset_seconds);
    if local < 0 {
        return time_label(time);
    }
    let (year, month, day, hour, minute, second) = civil(local as u64);
    let meridiem = if hour < 12 { "AM" } else { "PM" };
    let hour12 = match hour % 12 {
        0 => 12,
        h => h,
    };
    format!("{month}/{day}/{year} {hour12}:{minute:02}:{second:02} {meridiem}")
}
fn civil(seconds: u64) -> (i64, i64, i64, u64, u64, u64) {
    let days = seconds / 86400;
    let z = days as i64 + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (
        year,
        month,
        day,
        seconds % 86400 / 3600,
        seconds % 3600 / 60,
        seconds % 60,
    )
}
/// Local clock and per-side free space, resolved together by one host query
/// on a helper thread (each PowerShell start costs ~200 ms; the three former
/// serial queries held the descendant scan back by ~700 ms).
struct HostProbe(Option<thread::JoinHandle<HostInfo>>, [String; 2]);
type HostInfo = (Result<FilterClock, String>, [Option<String>; 2]);
impl HostProbe {
    fn spawn(paths: &[String; 2]) -> Self {
        let owned = paths.clone();
        let handle = thread::Builder::new()
            .name("matcha-host-probe".into())
            .spawn(move || {
                let t = Instant::now();
                let info = host_info(&owned);
                crate::perf::span("folder:host-probe", t);
                info
            })
            .ok();
        Self(handle, paths.clone())
    }
    fn join(self) -> HostInfo {
        self.0
            .and_then(|h| h.join().ok())
            .unwrap_or_else(|| host_info(&self.1))
    }
}
/// 根路径所在卷的盘符（UNC 与相对路径无剩余空间标签）。
#[cfg(windows)]
fn drive_letter(path: &str) -> Option<String> {
    let root = Path::new(path).components().next()?;
    let prefix = root.as_os_str().to_string_lossy();
    let drive = prefix.trim_end_matches(":\\").trim_end_matches(':');
    (!drive.is_empty()
        && !drive.starts_with("\\\\")
        && drive.chars().all(|c| c.is_ascii_alphabetic()))
    .then(|| drive.to_owned())
}
#[cfg(windows)]
fn free_label(drive: &str, bytes: f64) -> String {
    format!("{:.1} GB free on {}:\\", bytes / (1u64 << 30) as f64, drive)
}
fn host_info(paths: &[String; 2]) -> HostInfo {
    #[cfg(windows)]
    {
        let drives = paths.each_ref().map(|p| drive_letter(p));
        let mut script = String::from("$m=[DateTimeOffset]::new([DateTime]::Today); Write-Output ($m.ToUnixTimeSeconds().ToString() + ';' + ([int][DateTimeOffset]::Now.Offset.TotalSeconds).ToString())");
        for drive in &drives {
            match drive {
                Some(d) => script.push_str(&format!(
                    "; try {{ Write-Output ((Get-PSDrive {d} -ErrorAction Stop).Free) }} catch {{ Write-Output '' }}"
                )),
                None => script.push_str("; Write-Output ''"),
            }
        }
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .output();
        let output = match output {
            Ok(o) if o.status.success() => o,
            Ok(_) => return (Err("local time query failed".into()), Default::default()),
            Err(e) => return (Err(e.to_string()), Default::default()),
        };
        let text = String::from_utf8_lossy(&output.stdout);
        let mut lines = text.lines();
        let clock = parse_clock(lines.next().unwrap_or_default());
        let free = drives.map(|drive| {
            let line = lines.next()?;
            let bytes: f64 = line.trim().parse().ok()?;
            Some(free_label(drive.as_deref()?, bytes))
        });
        (clock, free)
    }
    #[cfg(not(windows))]
    {
        let _ = paths;
        (Ok(utc_clock()), Default::default())
    }
}
#[cfg_attr(not(windows), allow(dead_code))]
fn parse_clock(text: &str) -> Result<FilterClock, String> {
    let (midnight, offset) = text.trim().split_once(';').ok_or("invalid clock result")?;
    Ok(FilterClock {
        today_midnight: UNIX_EPOCH
            + Duration::from_secs(midnight.parse().map_err(|_| "invalid midnight")?),
        utc_offset_seconds: offset.parse().map_err(|_| "invalid offset")?,
    })
}

pub struct DirCompareView {
    window_id: u64,
    child: Option<Box<DirCompareView>>,
    shared: Arc<Shared>,
    commands: Sender<Work>,
    worker: Option<thread::JoinHandle<()>>,
    pub snapshot: Arc<Snapshot>,
    pub filters: FiltersBar,
    pub selection: Selection,
    pub last_stats: DirFrameStats,
    pub sync_mode: bool,
    paths: PathsDialog,
    history: Vec<[String; 2]>,
    history_at: usize,
    pub log_panel: LogPanel,
    columns: Vec<Column>,
    palette: FolderColors,
    legend_open: bool,
    toolbar: bool,
    find: FindDialog,
    operation: Option<OperationDialog>,
    scope: Option<dialogs::scope::ScopeDialog>,
    handling: bc_engine_dir::rules::HandlingOptions,
    question: Option<dialogs::Question>,
    message: String,
    quick: Option<String>,
    text: Option<Box<TextCompareView>>,
    text_prefs: Prefs,
    exit_requested: bool,
    /// 关闭标签页在等内嵌文本窗口的保存确认；确认关闭后转为 `close_requested`。
    close_after_text: bool,
    close_requested: bool,
    new_tab_requested: bool,
    compare_to: bool,
    align_to: bool,
    anchor: Option<(usize, usize)>,
    focus: Option<usize>,
    focus_side: Option<usize>,
    scroll_y: f32,
    pending_scroll: Option<usize>,
    pending_focus: Option<NodeId>,
    sort: (Column, usize, bool),
    /// Session Settings dialog (tabs per the reference help).
    settings: dialogs::folder_settings::FolderSettings,
    criteria_text: String,
    pairing: PairingOptions,
    scan: ScanOptions,
    /// 参考 chrome 布局状态（仅比较视图；同步视图保持旧布局）。
    menus: Vec<crate::chrome::menubar::MenuSpec>,
    home_requested: bool,
    show_files: bool,
    chrome_log: Vec<String>,
    chrome_log_logged: bool,
    log_save_open: bool,
    grid_lines: Option<(usize, Arc<Vec<RowLine>>)>,
    /// Row the right-click menu was opened on, and the side under the pointer.
    context_row: Option<usize>,
    context_side: Option<usize>,
    /// Folder-compare context menu choice, run by `chrome_ui` after the grid.
    context_action: Option<String>,
    /// Status-bar selection totals keyed by snapshot and selection fingerprint.
    selection_totals: Option<(usize, (usize, u64), SelectionTotals)>,
    /// Last-frame rects of the floating text / child-folder windows: an OS
    /// file drop inside one belongs to that window's own view.
    floating: [Option<egui::Rect>; 2],
}
impl DirCompareView {
    pub fn open(
        ctx: &egui::Context,
        left: &Path,
        right: &Path,
        sync_mode: bool,
        bench: bool,
    ) -> Self {
        let mut view = Self::empty(ctx, sync_mode, bench);
        let paths = [left.display().to_string(), right.display().to_string()];
        view.reload(paths.clone(), true);
        view.paths.recent = paths.to_vec();
        view.message = "Loading folders".into();
        view
    }
    /// 空会话：工作线程就绪但不加载；路径栏两侧都填上后经 `reload` 加载
    /// （与已打开会话改路径同一路径）。
    pub fn empty(ctx: &egui::Context, sync_mode: bool, bench: bool) -> Self {
        let shared = Arc::new(Shared {
            latest: Mutex::new(None),
            progress: Mutex::new(Progress::default()),
            questions: Mutex::new(VecDeque::new()),
            notices: Mutex::new(VecDeque::new()),
            cancel: Mutex::new(CancelToken::new()),
            tokens: Mutex::new(BTreeMap::new()),
            next: AtomicU64::new(1),
            closed: AtomicBool::new(false),
            busy: AtomicBool::new(false),
            ctx: ctx.clone(),
            log: OpLog::default(),
        });
        let (tx, rx) = mpsc::channel();
        let copy = shared.clone();
        let worker = thread::spawn(move || run_worker(rx, copy, sync_mode, bench));
        let mut view = Self {
            window_id: NEXT_VIEW.fetch_add(1, AtomicOrdering::Relaxed),
            child: None,
            shared,
            commands: tx,
            worker: Some(worker),
            snapshot: Arc::new(Snapshot::default()),
            filters: FiltersBar::default(),
            selection: Selection::default(),
            last_stats: Default::default(),
            sync_mode,
            paths: PathsDialog::default(),
            history: Vec::new(),
            history_at: 0,
            log_panel: LogPanel::default(),
            columns: vec![Column::Size, Column::Modified],
            palette: FolderColors::default(),
            legend_open: false,
            toolbar: true,
            find: FindDialog {
                side: 2,
                wrap: true,
                ..Default::default()
            },
            operation: None,
            scope: None,
            handling: Default::default(),
            question: None,
            message: String::new(),
            quick: None,
            text: None,
            text_prefs: Prefs::default(),
            exit_requested: false,
            close_after_text: false,
            close_requested: false,
            new_tab_requested: false,
            compare_to: false,
            align_to: false,
            anchor: None,
            focus: None,
            focus_side: None,
            scroll_y: 0.0,
            pending_scroll: None,
            pending_focus: None,
            sort: (Column::Name, 0, false),
            settings: Default::default(),
            criteria_text: Criteria::default().to_script(),
            pairing: PairingOptions::default(),
            scan: ScanOptions {
                scan_orphans: sync_mode || bench,
                expand_on_load: bench || sync_mode,
                ..Default::default()
            },
            menus: folder_menu_inventory(),
            home_requested: false,
            show_files: true,
            chrome_log: Vec::new(),
            chrome_log_logged: false,
            log_save_open: false,
            grid_lines: None,
            context_row: None,
            context_side: None,
            context_action: None,
            selection_totals: None,
            floating: [None; 2],
        };
        // 参考：过滤框默认 *.*，日志区默认打开（比较与同步视图相同）。
        view.filters.names = "*.*".into();
        view.log_panel.open = true;
        if sync_mode {
            view.filters.options = sync_view_options(true);
        }
        view
    }
    pub fn busy(&self) -> bool {
        self.shared.busy.load(AtomicOrdering::Relaxed)
    }
    pub fn send(&self, command: Command) {
        self.shared.busy.store(true, AtomicOrdering::Relaxed);
        let id = self.shared.next.fetch_add(1, AtomicOrdering::Relaxed);
        let token = CancelToken::new();
        self.shared.tokens.lock().unwrap().insert(id, token.clone());
        if self
            .commands
            .send(Work {
                id,
                command,
                cancel: token,
            })
            .is_err()
        {
            self.shared.busy.store(false, AtomicOrdering::Relaxed);
        }
    }
    pub fn cancel(&self) {
        self.shared.token().cancel();
        for token in self.shared.tokens.lock().unwrap().values() {
            token.cancel();
        }
    }
    pub fn row_count(&self) -> usize {
        self.snapshot.rows.len()
    }
    pub fn scroll_to_row(&mut self, row: usize) {
        self.pending_scroll = Some(row);
    }
    pub fn paths(&self) -> [PathBuf; 2] {
        [
            PathBuf::from(&self.paths.left),
            PathBuf::from(&self.paths.right),
        ]
    }
    pub fn poll(&mut self, ctx: &egui::Context) {
        if let Some(snapshot) = self.shared.latest.lock().unwrap().take() {
            let focus_key = self
                .focus
                .and_then(|index| self.snapshot.rows.get(index))
                .and_then(|row| {
                    row.sides
                        .iter()
                        .enumerate()
                        .filter_map(|(side, item)| item.as_ref().map(|i| (i.node, side)))
                        .find(|key| self.selection.items.contains(key))
                });
            self.selection.items = self
                .selection
                .items
                .intersection(&snapshot.visible)
                .copied()
                .collect();
            self.focus = focus_key.and_then(|key| snapshot.positions.get(&key).copied());
            if self.focus.is_none() {
                self.anchor = None;
            }
            self.snapshot = snapshot;
            // 空会话的空快照不算一次加载。
            let loaded = !self.paths.left.trim().is_empty() && !self.paths.right.trim().is_empty();
            if self.snapshot.complete && !self.chrome_log_logged && loaded {
                self.chrome_log_logged = true;
                self.chrome_log.push(format!(
                    "Load comparison: {} <-> {}",
                    self.paths.left, self.paths.right
                ));
            }
        }
        let notices: Vec<_> = self.shared.notices.lock().unwrap().drain(..).collect();
        for notice in notices {
            match notice {
                Notice::Message(text) => {
                    self.chrome_log.push(text.clone());
                    self.chrome_log.truncate(200);
                    self.message = text;
                }
                Notice::Clipboard(text) => ctx.copy_text(text),
                Notice::Quick(text) => self.quick = Some(text),
                // 二进制文件对打不开文本视图（内嵌窗口不支持 hex），给出可操作提示。
                Notice::Text(paths) if crate::home::hex_paths(&paths[0], &paths[1]) => {
                    self.message = format!(
                        "Cannot open text: {} is binary — open the pair in a Hex Compare session",
                        paths[0].display()
                    );
                }
                Notice::Text(paths) => match TextCompareView::open(&paths[0], &paths[1]) {
                    Ok(view) => self.text = Some(Box::new(view)),
                    Err(e) => self.message = format!("Cannot open text: {e}"),
                },
                Notice::Presets(presets) => self.filters.presets = presets,
                Notice::Focus(node) => self.pending_focus = Some(node),
            }
        }
        if let Some(node) = self.pending_focus {
            if let Some(index) = self.snapshot.rows.iter().position(|r| r.node == Some(node)) {
                self.focus_row(index, None);
                self.pending_focus = None;
            }
        }
        if self.question.is_none() {
            self.question = self.shared.questions.lock().unwrap().pop_front();
        }
    }
    fn reload(&mut self, paths: [String; 2], remember: bool) {
        self.paths.left = paths[0].clone();
        self.paths.right = paths[1].clone();
        // 空会话只填了一侧：等两侧都有路径再加载，避免先报"必须是文件夹"。
        if paths.iter().any(|p| p.trim().is_empty()) {
            return;
        }
        self.cancel();
        self.selection.items.clear();
        self.focus = None;
        self.paths.remember(&paths);
        if remember {
            self.history.truncate(self.history_at + 1);
            self.history.push(paths.clone());
            self.history_at = self.history.len() - 1;
        }
        self.send(Command::Load(paths));
    }
    /// OS file drop: a folder dropped on the left/right half (split at the
    /// centre gap, same geometry as the trees and path boxes) replaces that
    /// side's base folder. A drop over a floating text/child window is left to
    /// that window's view; any other drop is consumed here so an embedded view
    /// never acts on a drop that landed on the folder trees.
    fn handle_drop(&mut self, ctx: &egui::Context, full: egui::Rect) {
        use crate::chrome::drop;
        let dropped = drop::dropped_paths(ctx);
        let Some(path) = dropped.first() else {
            return;
        };
        let pos = drop::drop_pos(ctx);
        let windows = [
            self.floating[0].filter(|_| self.text.is_some()),
            self.floating[1].filter(|_| self.child.is_some()),
        ];
        if pos.is_some_and(|p| windows.iter().flatten().any(|r| r.contains(p))) {
            return;
        }
        ctx.input_mut(|i| i.raw.dropped_files.clear());
        let dialog = self.paths.open
            || self.find.open
            || self.settings.open
            || self.log_save_open
            || self.operation.is_some()
            || self.scope.is_some()
            || self.question.is_some();
        if dialog {
            return;
        }
        let gap = self.center_gap();
        let (left_w, _) = Self::chrome_split(full.width(), gap);
        let mid = full.left() + left_w + gap / 2.0;
        let halves = [
            egui::Rect::from_min_max(full.min, egui::pos2(mid, full.bottom())),
            egui::Rect::from_min_max(egui::pos2(mid, full.top()), full.max),
        ];
        let side = drop::side_at(pos, drop::panes(&halves), self.focus_side.unwrap_or(0));
        self.drop_path(side, path);
    }
    /// 系统原生「浏览文件夹」对话框：从该侧当前文件夹起步，选定后替换该侧根目录。
    fn browse_folder(&mut self, side: usize) {
        use crate::chrome::native_dialog::{side_title, Kind, Request};
        let current = self.path_texts()[side].clone();
        if let Some(path) = Request::new(Kind::PickFolder, side_title("Browse for Folder", side))
            .initial(current.trim())
            .show()
        {
            self.drop_path(side, &path);
        }
    }
    /// Replace one side's base folder with a dropped folder.
    pub(crate) fn drop_path(&mut self, side: usize, path: &Path) {
        if !path.is_dir() {
            self.message = format!("Drop a folder to replace a side: {}", path.display());
            return;
        }
        let mut paths = self.path_texts();
        paths[side] = path.display().to_string();
        self.reload(paths, true);
    }
    fn up(&mut self, target: Target) {
        let mut paths = [self.paths.left.clone(), self.paths.right.clone()];
        for (side, path) in paths.iter_mut().enumerate() {
            if target.contains(side) {
                if let Some(parent) = Path::new(path).parent() {
                    *path = parent.display().to_string();
                }
            }
        }
        self.reload(paths, true);
    }
    fn selected_sides(&self) -> Vec<(usize, RenderSide)> {
        let mut found = BTreeMap::new();
        for row in &self.snapshot.rows {
            for side in 0..2 {
                if let Some(item) = &row.sides[side] {
                    if self.selection.items.contains(&(item.node, side)) {
                        found.insert((item.node, side), item.clone());
                    }
                }
            }
        }
        found
            .into_iter()
            .map(|((_, side), item)| (side, item))
            .collect()
    }
    fn selected_pair(&self) -> Result<[PathBuf; 2], String> {
        let mut selected = self
            .selected_sides()
            .into_iter()
            .filter(|(_, s)| s.entry.kind == EntryKind::File)
            .collect::<Vec<_>>();
        if selected.len() == 2 {
            if selected[0].0 != selected[1].0 {
                selected.sort_by_key(|(side, _)| *side);
            }
            return Ok([selected[0].1.path.clone(), selected[1].1.path.clone()]);
        }
        if selected.len() == 1 {
            let (side, item) = &selected[0];
            if let Some(row) = self
                .snapshot
                .rows
                .iter()
                .find(|r| r.sides[*side].as_ref().is_some_and(|s| s.node == item.node))
            {
                if let Some(other) = &row.sides[1 - *side] {
                    return Ok(if *side == 0 {
                        [item.path.clone(), other.path.clone()]
                    } else {
                        [other.path.clone(), item.path.clone()]
                    });
                }
            }
        }
        Err("Select one paired file, or exactly two files (Ctrl-click may cross rows)".into())
    }
    fn open_pair(&mut self) {
        match self.selected_pair() {
            Ok(paths) => self.send(Command::OpenText(paths)),
            Err(error) => self.message = error,
        }
    }
    pub fn operate(&mut self, op: DirOp, include_hidden: bool) {
        if !self.snapshot.complete {
            self.message = "Refresh or resolve comparison errors before file operations".into();
            return;
        }
        if let DirOp::CopyTo { dest, .. } | DirOp::MoveTo { dest, .. } = &op {
            self.paths.remember(&[dest.clone(), dest.clone()]);
        }
        let mut selection = self.selection.clone();
        if selection.items.is_empty() && matches!(op, DirOp::NewFolder { .. }) {
            selection = Selection::both([0]);
        }
        self.send(Command::Operate {
            selection,
            op,
            include_hidden,
        });
    }
    /// Routes Home/close through an edited text window's save prompt (here or in a child).
    /// Returns true when that prompt now blocks the caller.
    pub fn request_text_close(&mut self, exit: bool) -> bool {
        if let Some(child) = &mut self.child {
            if child.request_text_close(exit) {
                return true;
            }
        }
        match &mut self.text {
            Some(view) if view.is_dirty() => {
                if exit {
                    view.request_exit();
                } else {
                    view.request_close();
                }
                true
            }
            _ => false,
        }
    }
    /// 内嵌文本窗口（含子视图）有未保存编辑。
    pub fn text_dirty(&self) -> bool {
        self.child.as_ref().is_some_and(|c| c.text_dirty())
            || self.text.as_ref().is_some_and(|t| t.is_dirty())
    }
    /// 内嵌文本窗口的保存确认框是否打开。
    pub fn prompt_open(&self) -> bool {
        self.child.as_ref().is_some_and(|c| c.prompt_open())
            || self.text.as_ref().is_some_and(|t| t.prompt_open())
    }
    /// Close Tab：内嵌文本有未保存编辑时先走其保存确认，确认后才关闭标签页。
    pub fn request_tab_close(&mut self) {
        if self.text.as_ref().is_some_and(|t| t.is_dirty()) {
            self.close_after_text = true;
            self.request_text_close(false);
        } else if self.child.as_ref().is_some_and(|c| c.text_dirty()) {
            // 子视图的文本窗口先确认；关闭标签页需用户再执行一次。
            self.request_text_close(false);
        } else {
            self.close_requested = true;
        }
    }
    /// True once Close Tab is confirmed (the app then removes this tab).
    pub fn take_close_request(&mut self) -> bool {
        std::mem::take(&mut self.close_requested)
    }
    /// True once Session > New Tab was chosen.
    pub fn take_new_tab_request(&mut self) -> bool {
        std::mem::take(&mut self.new_tab_requested)
    }
    /// True once a text window confirmed Exit (the app then closes the window).
    pub fn take_exit_request(&mut self) -> bool {
        let child = self.child.as_mut().is_some_and(|c| c.take_exit_request());
        std::mem::take(&mut self.exit_requested) || child
    }
    fn new_folder_view(&mut self, paths: [PathBuf; 2]) {
        self.settings.close();
        self.legend_open = false;
        self.find.open = false;
        self.operation = None;
        self.scope = None;
        self.text = None;
        self.child = Some(Box::new(Self::open(
            &self.shared.ctx,
            &paths[0],
            &paths[1],
            false,
            false,
        )));
    }
    fn open_folder_selection(&mut self) {
        let mut selected = self
            .selected_sides()
            .into_iter()
            .filter(|(_, s)| s.entry.kind == EntryKind::Dir)
            .collect::<Vec<_>>();
        if selected.len() == 2 {
            if selected[0].0 != selected[1].0 {
                selected.sort_by_key(|(side, _)| *side);
            }
            self.new_folder_view([selected[0].1.path.clone(), selected[1].1.path.clone()]);
            return;
        }
        if selected.len() == 1 {
            let (side, item) = &selected[0];
            if let Some(other) = self
                .snapshot
                .rows
                .iter()
                .find(|r| r.sides[*side].as_ref().is_some_and(|s| s.node == item.node))
                .and_then(|r| r.sides[1 - *side].as_ref())
                .filter(|s| s.entry.kind == EntryKind::Dir)
            {
                let paths = if *side == 0 {
                    [item.path.clone(), other.path.clone()]
                } else {
                    [other.path.clone(), item.path.clone()]
                };
                self.new_folder_view(paths);
                return;
            }
        }
        self.message = "Select two folders, or one paired folder, to open a new folder view".into();
    }
    fn set_base(&mut self, other: bool, both: bool) {
        let selected = self
            .selected_sides()
            .into_iter()
            .filter(|(_, s)| s.entry.kind == EntryKind::Dir)
            .collect::<Vec<_>>();
        let mut paths = [self.paths.left.clone(), self.paths.right.clone()];
        if both && selected.len() == 2 {
            paths = [
                selected[0].1.path.display().to_string(),
                selected[1].1.path.display().to_string(),
            ];
        } else if let Some((side, item)) = selected.first() {
            paths[if other { 1 - *side } else { *side }] = item.path.display().to_string();
        } else {
            self.message = "Select folder rows first".into();
            return;
        }
        self.reload(paths, true);
    }
    fn select_visible(&mut self, mask: &str, invert: bool) {
        for row in &self.snapshot.rows {
            for side in 0..2 {
                if let Some(item) = &row.sides[side] {
                    let matches = match mask {
                        "files" => !row.directory,
                        "newer-left" => side == 0 && row.status == CompareStatus::LeftNewer,
                        "newer-right" => side == 1 && row.status == CompareStatus::RightNewer,
                        "newer" => {
                            !row.directory
                                && ((side == 0 && row.status == CompareStatus::LeftNewer)
                                    || (side == 1 && row.status == CompareStatus::RightNewer))
                        }
                        "orphan-left" => side == 0 && row.status == CompareStatus::LeftOrphan,
                        "orphan-right" => side == 1 && row.status == CompareStatus::RightOrphan,
                        "orphans" => {
                            !row.directory
                                && matches!(
                                    row.status,
                                    CompareStatus::LeftOrphan | CompareStatus::RightOrphan
                                )
                        }
                        _ => true,
                    };
                    let key = (item.node, side);
                    if invert {
                        if !self.selection.items.remove(&key) {
                            self.selection.items.insert(key);
                        }
                    } else if matches {
                        self.selection.items.insert(key);
                    }
                }
            }
        }
    }
    pub fn navigate(&mut self, backwards: bool, conflicts: bool) {
        if conflicts {
            let current = self
                .focus
                .and_then(|i| self.snapshot.rows.get(i))
                .and_then(|r| r.node);
            self.send(Command::FocusConflict { backwards, current });
            return;
        }
        let start = self
            .focus
            .map_or(if backwards { self.row_count() } else { 0 }, |n| {
                if backwards {
                    n
                } else {
                    n + 1
                }
            });
        let indices: Box<dyn Iterator<Item = usize>> = if backwards {
            Box::new((0..start).rev())
        } else {
            Box::new(start..self.row_count())
        };
        if let Some(index) = indices.into_iter().find(|&i| {
            let row = &self.snapshot.rows[i];
            if conflicts {
                row.conflict
            } else {
                !matches!(row.status, CompareStatus::Same | CompareStatus::Unknown)
            }
        }) {
            self.focus_row(index, None);
        } else {
            self.message = "No further difference/conflict".into();
        }
    }
    fn focus_row(&mut self, index: usize, side: Option<usize>) {
        self.selection.items.clear();
        if let Some(row) = self.snapshot.rows.get(index) {
            for s in 0..2 {
                if side.is_none() || side == Some(s) {
                    if let Some(item) = &row.sides[s] {
                        self.selection.insert(item.node, s);
                    }
                }
            }
        }
        self.focus = Some(index);
        self.focus_side = side;
        self.scroll_to_row(index);
    }
    fn restrict_side(&mut self, side: usize) {
        self.selection
            .items
            .retain(|(_, selected)| *selected == side);
        self.focus_side = Some(side);
        if let Some(row) = self.focus {
            self.anchor = Some((row, side));
        }
    }
    fn find_next(&mut self, backwards: bool) {
        let pattern = match self.find.pattern() {
            Ok(p) => p,
            Err(e) => {
                self.find.error = Some(e);
                return;
            }
        };
        let count = self.row_count();
        if count == 0 {
            return;
        }
        let origin = self.focus.unwrap_or(if backwards { count } else { 0 });
        let mut found = None;
        let first = usize::from(self.focus.is_some());
        for step in first..count + first {
            let index = if backwards {
                if !self.find.wrap && step >= origin && self.focus.is_none() {
                    continue;
                }
                if !self.find.wrap && step > origin {
                    break;
                }
                (origin + count - step - usize::from(self.focus.is_none())) % count
            } else {
                if !self.find.wrap && origin + step >= count {
                    break;
                }
                (origin + step) % count
            };
            let row = &self.snapshot.rows[index];
            for side in 0..2 {
                if (self.find.side == 2 || self.find.side == side)
                    && row.sides[side]
                        .as_ref()
                        .is_some_and(|s| self.find.matches(&pattern, &s.name))
                {
                    found = Some((index, side));
                    break;
                }
            }
            if found.is_some() {
                break;
            }
        }
        if let Some((index, side)) = found {
            self.focus_row(index, Some(side));
            self.find.error = None;
        } else {
            self.find.error = Some("Filename not found".into());
        }
    }
    fn expand_selection(&self, expanded: bool, recursive: bool) {
        let nodes = self
            .selection
            .items
            .iter()
            .map(|(id, _)| *id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        self.send(Command::Expand {
            nodes,
            expanded,
            recursive,
        });
    }
    /// The Session Settings draft from the current view state.
    fn settings_draft(&self) -> dialogs::folder_settings::Draft {
        dialogs::folder_settings::Draft {
            paths: [self.paths.left.clone(), self.paths.right.clone()],
            names: self.filters.names.clone(),
            other: self.filters.other.clone(),
            content: self.filters.content.clone(),
            exclude_containing: self.filters.exclude_containing,
            criteria: self.criteria_text.clone(),
            pairing: self.pairing.clone(),
            scan: self.scan.clone(),
            handling: self.handling.clone(),
        }
    }
    pub(crate) fn open_settings(&mut self) {
        let draft = self.settings_draft();
        self.settings
            .show(draft, crate::chrome::dialog::Placement::Window);
    }
    /// Apply a Session Settings draft: only what changed is re-sent (filters re-filter,
    /// rules rescan, new folders reload).
    fn apply_settings(&mut self, draft: dialogs::folder_settings::Draft) {
        let before = self.settings_draft();
        if draft == before {
            return;
        }
        self.filters.names = draft.names.clone();
        self.filters.other = draft.other.clone();
        self.filters.content = draft.content.clone();
        self.filters.exclude_containing = draft.exclude_containing;
        self.criteria_text = draft.criteria.clone();
        self.pairing = draft.pairing.clone();
        self.scan = draft.scan.clone();
        self.handling = draft.handling.clone();
        let filters = (
            &draft.names,
            &draft.other,
            &draft.content,
            draft.exclude_containing,
        ) != (
            &before.names,
            &before.other,
            &before.content,
            before.exclude_containing,
        );
        let rules = (
            &draft.criteria,
            &draft.pairing,
            &draft.scan,
            &draft.handling,
        ) != (
            &before.criteria,
            &before.pairing,
            &before.scan,
            &before.handling,
        );
        if filters {
            self.apply_view_options();
        }
        if rules {
            match Criteria::parse_script(&self.criteria_text) {
                Ok(criteria) => self.send(Command::Rules {
                    criteria,
                    handling: self.handling.clone(),
                    pairing: self.pairing.clone(),
                    scan: self.scan.clone(),
                }),
                Err(e) => self.message = e,
            }
        }
        if draft.paths != before.paths {
            self.reload(draft.paths, true);
        }
    }
    fn settings_dialog(&mut self, ctx: &egui::Context) {
        use dialogs::folder_settings::Outcome;
        let title = if self.sync_mode {
            "Folder Sync"
        } else {
            "Folder Compare"
        };
        for outcome in self.settings.ui(ctx, title) {
            match outcome {
                Outcome::Apply(draft) => self.apply_settings(draft),
                Outcome::SavePreset { name, names } => {
                    match self.filters.presets.upsert(&name, &names) {
                        Ok(()) => {
                            self.settings.error = None;
                            self.send(Command::SavePresets(self.filters.presets.to_xml()));
                        }
                        Err(e) => self.settings.error = Some(e),
                    }
                }
                Outcome::AddAlignment {
                    left,
                    right,
                    folder,
                    regex,
                } => self.send(Command::AlignPattern {
                    left,
                    right,
                    regex,
                    folder,
                }),
            }
        }
    }
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<FolderNavigation> {
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
        let mut switch = None;
        let busy = self.busy();
        // The sync view draws its own chrome (sync_view.rs) from the shared
        // tree/log/status renderers before handing over for dialogs/keys.
        if !self.sync_mode {
            switch = self.chrome_ui(ui, busy);
        }
        self.handle_drop(&ctx, ui.max_rect());
        if let Some(paths) = self.paths.show(&ctx) {
            self.reload(paths, true);
        }
        if self.find.show(&ctx) {
            self.find_next(self.find.backwards);
        }
        self.settings_dialog(&ctx);
        legend::show(&ctx, &mut self.legend_open, self.palette);
        if let Some(dialog) = &mut self.operation {
            if let Some(result) = dialog.show(&ctx, self.snapshot.clock) {
                if let Ok((op, hidden)) = result {
                    self.operate(op, hidden);
                }
                self.operation = None;
            }
        }
        if let Some(scope) = &mut self.scope {
            if let Some(result) = scope.show(&ctx) {
                if let Ok((op, hidden)) = result {
                    self.operate(op, hidden);
                }
                self.scope = None;
            }
        }
        if let Some(question) = &self.question {
            if let Some(answer) = dialogs::confirmation(&ctx, question) {
                let _ = question.reply.send(answer);
                self.question = None;
            }
        }
        if let Some(text) = self.quick.clone() {
            let mut open = true;
            crate::chrome::window::Window::new("Quick compare")
                .open(&mut open)
                .show(&ctx, |ui| {
                    ui.label(text);
                });
            if !open {
                self.quick = None;
            }
        }
        if let Some(view) = &mut self.text {
            let mut open = true;
            let mut action = None;
            let id = egui::Id::unique(("folder_text", self.window_id));
            // Esc belongs to the embedded view (find bar, selection), not the window.
            self.floating[0] = crate::chrome::window::Window::new("Text Compare — selected files")
                .id(id)
                .open(&mut open)
                .default_size([1150.0, 650.0])
                .close_on_escape(false)
                .show(&ctx, |ui| action = view.ui(ui, &mut self.text_prefs))
                .and_then(|()| ctx.memory(|m| m.area_rect(id)));
            if !open {
                // The close box goes through the text view's own save prompt.
                view.request_close();
            }
            match action {
                Some(ViewAction::Close) => {
                    self.text = None;
                    self.close_requested = std::mem::take(&mut self.close_after_text);
                }
                Some(ViewAction::Home) => self.home_requested = true,
                Some(ViewAction::NewTab) => self.new_tab_requested = true,
                Some(ViewAction::Exit) => {
                    self.text = None;
                    self.exit_requested = true;
                }
                // Only offered for clean text, so replacing the window loses nothing.
                Some(ViewAction::CompareParentFolders { left, right }) => {
                    self.new_folder_view([left, right]);
                }
                // 内嵌文本窗口只有一个：未保存编辑时不替换，避免丢失修改。
                Some(ViewAction::CompareTexts { left, right }) => {
                    if self.text.as_ref().is_some_and(|t| t.is_dirty()) {
                        self.message =
                            "Save or discard edits before comparing the selection".into();
                    } else {
                        match TextCompareView::from_texts(&left, &right) {
                            Ok(view) => self.text = Some(Box::new(view)),
                            Err(e) => self.message = format!("Cannot compare selection: {e}"),
                        }
                    }
                }
                None => {}
                // 内嵌文本窗口不切到 hex（属主会话是文件夹比较；互切仅限主视图）。
                Some(ViewAction::CompareAsHex { .. }) => {}
            }
            // 保存确认被取消：放弃挂起的关闭标签页。
            if self.text.as_ref().is_some_and(|t| !t.prompt_open()) {
                self.close_after_text = false;
            }
        }
        // F12 is a local diagnostic capture, useful even when desktop capture cannot
        // read an OpenGL window. It captures this viewport only, never other windows.
        if self.child.is_none() && ctx.input(|i| i.key_pressed(egui::Key::F12)) {
            ctx.request_screenshot(|image| {
                thread::spawn(move || {
                    let path = std::env::temp_dir()
                        .join(format!("matcha-folder-view-{}.bmp", std::process::id()));
                    let temporary = path.with_extension("bmp.tmp");
                    if crate::bench_dir::write_bmp(&temporary, &image).is_ok() {
                        let _ = std::fs::rename(temporary, path);
                    }
                });
            });
        }
        // The text window has its own keyboard map; Enter/Delete must not reach the folder.
        if self.child.is_none() && self.text.is_none() && !ctx.egui_wants_keyboard_input() {
            if ctx.input(|i| i.key_pressed(egui::Key::F7)) {
                self.navigate(ctx.input(|i| i.modifiers.shift), self.sync_mode);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.open_pair();
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Delete)) && !busy {
                let mut dialog = OperationDialog::new(Kind::Delete);
                dialog.include_hidden =
                    self.handling.include_hidden || self.filters.options.suppress_filters;
                self.operation = Some(dialog);
            }
            for (key, backwards) in [(egui::Key::ArrowUp, true), (egui::Key::ArrowDown, false)] {
                if ctx.input(|i| i.key_pressed(key)) && self.row_count() > 0 {
                    let current = self.focus.unwrap_or(0);
                    let next = if backwards {
                        current.saturating_sub(1)
                    } else {
                        (current + 1).min(self.row_count() - 1)
                    };
                    if ctx.input(|i| i.modifiers.shift) {
                        self.clicked(
                            next,
                            self.focus_side,
                            false,
                            egui::Modifiers {
                                shift: true,
                                ..Default::default()
                            },
                        );
                        self.scroll_to_row(next);
                    } else {
                        self.focus_row(next, self.focus_side);
                    }
                }
            }
            if ctx.input(|i| i.modifiers.shift && i.key_pressed(egui::Key::ArrowLeft)) {
                self.restrict_side(0);
            }
            if ctx.input(|i| i.modifiers.shift && i.key_pressed(egui::Key::ArrowRight)) {
                self.restrict_side(1);
            }
            {
                // 参考快捷键：导航（同步视图为冲突）/选择/刷新/查找。
                let pressed = |key: egui::Key, ctrl: bool, shift: bool, alt: bool| {
                    ctx.input(|i| {
                        i.key_pressed(key)
                            && i.modifiers.ctrl == ctrl
                            && i.modifiers.shift == shift
                            && i.modifiers.alt == alt
                    })
                };
                if pressed(egui::Key::N, true, false, false) {
                    self.navigate(false, self.sync_mode);
                }
                if pressed(egui::Key::P, true, false, false) {
                    self.navigate(true, self.sync_mode);
                }
                if pressed(egui::Key::A, true, false, false) {
                    self.selection.items.clear();
                    self.select_visible("all", false);
                }
                if pressed(egui::Key::A, true, true, false) {
                    self.selection.items.clear();
                    self.select_visible("files", false);
                }
                if pressed(egui::Key::F, true, false, false) {
                    self.find.open = true;
                }
                if pressed(egui::Key::F3, false, false, false) {
                    self.find_next(false);
                }
                if pressed(egui::Key::F3, false, true, false) {
                    self.find_next(true);
                }
                if pressed(egui::Key::F5, false, false, false) {
                    self.send(Command::Refresh {
                        selection: None,
                        full: false,
                    });
                }
                if pressed(egui::Key::F5, true, false, false) {
                    self.send(Command::Refresh {
                        selection: None,
                        full: true,
                    });
                }
                if pressed(egui::Key::F5, false, true, false) {
                    self.send(Command::Refresh {
                        selection: Some(self.selection.clone()),
                        full: false,
                    });
                }
                if pressed(egui::Key::ArrowLeft, false, false, true) && self.history_at > 0 {
                    self.history_at -= 1;
                    self.reload(self.history[self.history_at].clone(), false);
                }
                if pressed(egui::Key::ArrowRight, false, false, true)
                    && self.history_at + 1 < self.history.len()
                {
                    self.history_at += 1;
                    self.reload(self.history[self.history_at].clone(), false);
                }
                if pressed(egui::Key::F2, false, false, false) && !busy && !self.sync_mode {
                    let mut dialog = OperationDialog::new(Kind::Rename);
                    dialog.include_hidden =
                        self.handling.include_hidden || self.filters.options.suppress_filters;
                    dialog.recent = self.paths.recent.clone();
                    self.operation = Some(dialog);
                }
                if pressed(egui::Key::Insert, false, false, false) && !busy && !self.sync_mode {
                    let mut dialog = OperationDialog::new(Kind::NewFolder);
                    dialog.include_hidden =
                        self.handling.include_hidden || self.filters.options.suppress_filters;
                    dialog.recent = self.paths.recent.clone();
                    self.operation = Some(dialog);
                }
            }
        }
        if let Some(child) = &mut self.child {
            let mut open = true;
            let id = egui::Id::unique(("folder_child", self.window_id));
            self.floating[1] = crate::chrome::window::Window::new("Folder Compare — new view")
                .id(id)
                .open(&mut open)
                .default_size([1200.0, 720.0])
                .close_on_escape(false)
                .show(&ctx, |ui| {
                    if let Some(action) = child.ui(ui) {
                        switch = Some(action);
                    }
                })
                .and_then(|()| ctx.memory(|m| m.area_rect(id)));
            if !open {
                self.child = None;
                self.message = "Child view closed; Refresh if its files changed".into();
            }
        }
        switch
    }
    fn clicked(
        &mut self,
        index: usize,
        side: Option<usize>,
        double: bool,
        modifiers: egui::Modifiers,
    ) {
        let row = self.snapshot.rows[index].clone();
        if let Some(side) = side.filter(|_| self.compare_to || self.align_to) {
            if let Some(item) = &row.sides[side] {
                let old = self.selected_sides();
                if let Some((old_side, old_item)) = old.first() {
                    if self.align_to {
                        let (left, right) = if *old_side == 0 {
                            (old_item.node, item.node)
                        } else {
                            (item.node, old_item.node)
                        };
                        self.send(Command::Align { left, right });
                    } else {
                        self.send(Command::OpenText([
                            old_item.path.clone(),
                            item.path.clone(),
                        ]));
                    }
                }
            }
            self.compare_to = false;
            self.align_to = false;
            return;
        }
        if !modifiers.ctrl && !modifiers.command {
            self.selection.items.clear();
        }
        let indices = if modifiers.shift {
            self.anchor.map_or(index..=index, |(anchor, _)| {
                anchor.min(index)..=anchor.max(index)
            })
        } else {
            index..=index
        };
        for at in indices {
            for s in 0..2 {
                if side.is_none() || side == Some(s) {
                    if let Some(item) = &self.snapshot.rows[at].sides[s] {
                        let key = (item.node, s);
                        if modifiers.ctrl && !modifiers.shift && self.selection.items.contains(&key)
                        {
                            self.selection.items.remove(&key);
                        } else {
                            self.selection.items.insert(key);
                        }
                    }
                }
            }
        }
        self.focus = Some(index);
        self.focus_side = side;
        if !modifiers.shift {
            self.anchor = Some((index, side.unwrap_or(0)));
        }
        if double {
            if row.directory {
                if let Some(node) = row.node {
                    self.send(Command::Expand {
                        nodes: vec![node],
                        expanded: !row.expanded,
                        recursive: false,
                    });
                }
            } else {
                self.open_pair();
            }
        }
    }
}
// ==================== 参考外观（文件夹比较 chrome 布局） ====================
// 以下实现对齐 docs/visual-parity/tokens.json 的 inventories.folder 与 folder-basic/
// folder-expanded/folder-diffs-filter 命名区域：菜单栏 27、工具栏 43、过滤栏 45、
// 路径栏 26、列头 24、行高 20、列宽 Name 437 / Size 78 / Modified 150、中缝 37、
// 右侧 18 px 平坦滚动条、日志区 106、状态栏 25。同步视图（sync_mode）保持旧布局。

const CHROME_ROW: f32 = 20.0;
const CHROME_HEADER: f32 = 24.0;
const CHROME_FILTERBAR: f32 = 45.0;
const CHROME_PATHBAR: f32 = 26.0;
const CHROME_LOG: f32 = 106.0;
const CHROME_GAP: f32 = 37.0;
/// Sync view arrow column (tokens `folder-sync` region `arrows`: 117 px) with
/// three sub-columns split at +38 / +77 / +116.
pub(crate) const SYNC_GAP: f32 = 117.0;
const SYNC_ARROW_LINES: [f32; 4] = [0.0, 38.0, 77.0, 116.0];
const CHROME_SCROLLBAR: f32 = 18.0;
/// Left / right tree split (fraction of the width left for both trees), persisted.
pub(crate) const FOLDER_SPLIT_X: crate::chrome::splitter::Split =
    crate::chrome::splitter::Split::ratio("folder.split.x", 0.5).min(120.0, 120.0);
/// Height of the bottom log panel (pixels above the status bar), persisted.
const FOLDER_LOG: crate::chrome::splitter::Split =
    crate::chrome::splitter::Split::pixels_from_end("folder.log.h", CHROME_LOG).min(80.0, 40.0);
/// Path-bar space right of each combo box (buttons), so combos follow the split.
const PATH_BUTTONS: [f32; 2] = [109.0, 139.0];

const TREE_STRIPE: egui::Color32 = egui::Color32::from_rgb(248, 250, 249);
const TREE_SELECTED: egui::Color32 = crate::chrome::theme::SELECTION;
const TREE_LINE: egui::Color32 = crate::chrome::theme::TEXT_FAINT;
const HEADER_SEP: egui::Color32 = crate::chrome::theme::LINE;
const HEADER_EDGE: egui::Color32 = crate::chrome::theme::LINE;
const TREE_EDGE: egui::Color32 = crate::chrome::theme::LINE;
const LOG_FILL: egui::Color32 = crate::chrome::theme::SURFACE_SUBTLE;

/// 每行树线标记：参考只画“父节点 → 直接子行”的肘形连接，无跨层贯通竖线。
/// `parent_up` = 本行到父行的行距（行数）；0 表示顶层行无连接线。
#[derive(Clone, Copy, Default)]
pub struct RowLine {
    parent_up: u16,
}

/// 正向一次扫描求每行父连接线的起点（祖先栈）。
/// Sync arrow column glyphs: copy arrows in the source side's sub-column
/// (left → right in the first, right → left in the third), "?" in both for an
/// unresolved conflict, a red "×" over the side being deleted. Original
/// vector drawing (block arrow, yellow fill, brown outline).
fn paint_sync_mark(painter: &egui::Painter, column_left: f32, row_top: f32, mark: u8) {
    let center = |sub: usize| {
        egui::pos2(
            (column_left + (SYNC_ARROW_LINES[sub] + SYNC_ARROW_LINES[sub + 1]) / 2.0).round() + 0.5,
            row_top + CHROME_ROW / 2.0,
        )
    };
    let fill = egui::Color32::from_rgb(250, 214, 92);
    let ink = egui::Color32::from_rgb(140, 100, 20);
    let arrow = |c: egui::Pos2, right: bool| {
        let s = if right { 1.0 } else { -1.0 };
        let points: Vec<egui::Pos2> = [
            (-6.0, -2.0),
            (0.0, -2.0),
            (0.0, -5.0),
            (6.0, 0.0),
            (0.0, 5.0),
            (0.0, 2.0),
            (-6.0, 2.0),
        ]
        .iter()
        .map(|(x, y)| c + egui::vec2(x * s, *y))
        .collect();
        // Head and shaft are filled separately (the outline is concave).
        painter.add(egui::Shape::convex_polygon(
            vec![points[2], points[3], points[4]],
            fill,
            egui::Stroke::NONE,
        ));
        painter.rect_filled(egui::Rect::from_two_pos(points[0], points[5]), 0.0, fill);
        let mut outline = points;
        outline.push(outline[0]);
        painter.add(egui::Shape::line(outline, egui::Stroke::new(1.0, ink)));
    };
    if mark & SYNC_CONFLICT != 0 {
        for sub in [0, 2] {
            painter.text(
                center(sub),
                egui::Align2::CENTER_CENTER,
                "?",
                egui::FontId::proportional(15.0),
                egui::Color32::BLACK,
            );
        }
    }
    if mark & SYNC_TO_RIGHT != 0 {
        arrow(center(0), true);
    }
    if mark & SYNC_TO_LEFT != 0 {
        arrow(center(2), false);
    }
    for (bit, sub) in [(SYNC_DELETE_LEFT, 0), (SYNC_DELETE_RIGHT, 2)] {
        if mark & bit != 0 {
            let c = center(sub);
            let stroke = egui::Stroke::new(2.0, egui::Color32::from_rgb(220, 30, 30));
            painter.line_segment(
                [c + egui::vec2(-4.0, -4.0), c + egui::vec2(4.0, 4.0)],
                stroke,
            );
            painter.line_segment(
                [c + egui::vec2(-4.0, 4.0), c + egui::vec2(4.0, -4.0)],
                stroke,
            );
        }
    }
}
fn row_lines(rows: &[RenderRow]) -> Vec<RowLine> {
    let mut out = Vec::with_capacity(rows.len());
    let mut ancestors: Vec<usize> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let d = row.depth;
        ancestors.truncate(d);
        let parent_up = if d > 0 && ancestors.len() == d {
            (i - ancestors[d - 1]) as u16
        } else {
            0
        };
        out.push(RowLine { parent_up });
        ancestors.push(i);
    }
    out
}

/// 文件夹图标着色：未扫描（参考黄）> 含差异（红）> 含孤儿（紫）> 其余（灰）。
fn folder_tint(row: &RenderRow) -> (egui::Color32, egui::Color32, egui::Color32, egui::Color32) {
    let rgb = |r, g, b| egui::Color32::from_rgb(r, g, b);
    if row.pending {
        (
            rgb(255, 232, 139),
            rgb(247, 215, 87),
            rgb(207, 196, 155),
            rgb(179, 168, 121),
        )
    } else if row.red {
        (
            rgb(255, 187, 187),
            rgb(246, 100, 100),
            rgb(207, 155, 155),
            rgb(91, 55, 55),
        )
    } else if row.purple {
        (
            rgb(160, 81, 251),
            rgb(154, 80, 230),
            rgb(169, 119, 221),
            rgb(74, 63, 85),
        )
    } else {
        (
            rgb(134, 134, 134),
            rgb(104, 104, 104),
            rgb(118, 118, 118),
            rgb(72, 72, 72),
        )
    }
}

fn paint_folder_icon(painter: &egui::Painter, rect: egui::Rect, row: &RenderRow) {
    let (top, bottom, tab, outline) = folder_tint(row);
    // 主体：四段垂直渐变 + 深色描边；左上角标签页。
    let body = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.top() + 3.0),
        rect.right_bottom(),
    );
    let bands = 4;
    for i in 0..bands {
        let y0 = body.top() + body.height() * i as f32 / bands as f32;
        let y1 = body.top() + body.height() * (i + 1) as f32 / bands as f32;
        let t = (i as f32 + 0.5) / bands as f32;
        let color = egui::Color32::from_rgb(
            (top.r() as f32 + (bottom.r() as f32 - top.r() as f32) * t) as u8,
            (top.g() as f32 + (bottom.g() as f32 - top.g() as f32) * t) as u8,
            (top.b() as f32 + (bottom.b() as f32 - top.b() as f32) * t) as u8,
        );
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(body.left(), y0), egui::pos2(body.right(), y1)),
            0.0,
            color,
        );
    }
    painter.rect_stroke(
        body,
        0.0,
        egui::Stroke::new(1.0, outline),
        egui::StrokeKind::Inside,
    );
    let tab_rect =
        egui::Rect::from_min_max(rect.min, egui::pos2(rect.left() + 6.5, rect.top() + 3.5));
    painter.rect_filled(tab_rect, 0.0, tab);
    painter.rect_stroke(
        tab_rect,
        0.0,
        egui::Stroke::new(1.0, outline),
        egui::StrokeKind::Inside,
    );
}

/// 文件图标：白底 8x8 方块 + 5 px 语义色实心芯（参考的“未知类型”小方块）。
fn paint_file_icon(painter: &egui::Painter, left: f32, row_top: f32, color: egui::Color32) {
    let outer = egui::Rect::from_min_size(egui::pos2(left, row_top + 7.0), egui::vec2(8.0, 8.0));
    painter.rect_filled(outer, 0.0, egui::Color32::WHITE);
    painter.rect_filled(
        egui::Rect::from_min_size(
            egui::pos2(outer.left() + 1.0, outer.top() + 1.0),
            egui::vec2(5.0, 6.0),
        ),
        0.0,
        color,
    );
}

/// 点线（1 px 点隔 1 px 空），用于树线与焦点虚框。
impl DirCompareView {
    /// Folder-compare right-click menu (reference `folder-context`): entries, order and
    /// separators follow the captured menu; actions reuse the menu-bar command ids. Items
    /// without an implementation (Synchronize, Open With > Folder Sync) are disabled.
    fn folder_context_items(&self, directory: bool) -> Vec<crate::chrome::menubar::MenuItem> {
        use crate::chrome::menubar::{MenuItem, MenuItemKind};
        let any = !self.selection.items.is_empty();
        let to = if self.context_side == Some(1) {
            "Left"
        } else {
            "Right"
        };
        let item = |label: &str, shortcut: Option<&str>, id: &str, enabled: bool| MenuItem {
            shortcut: shortcut.map(str::to_owned),
            enabled,
            ..MenuItem::action(label, id)
        };
        let sub = |label: &str, children: Vec<MenuItem>| {
            let enabled = children
                .iter()
                .any(|c| c.enabled && !matches!(c.kind, MenuItemKind::Separator));
            MenuItem {
                kind: MenuItemKind::Submenu(children),
                enabled,
                ..MenuItem::action(label, label)
            }
        };
        let sep = crate::chrome::popup::item::separator;
        vec![
            if directory {
                item("Open Folder", None, "Open Folder", true)
            } else {
                item("Open", None, "Open Pair", true)
            },
            item("Open Subfolders", None, "Open Subfolders", directory),
            item("Close Subfolders", None, "Close Subfolders", directory),
            item("Set as Base Folder", None, "Set as Base Folder", directory),
            item(
                "Set as Base on Other Side",
                None,
                "Set as Base on Other Side",
                directory,
            ),
            item("Open in New View", None, "Open in New View", directory),
            sub(
                "Open With",
                vec![
                    item("Associated Application", None, "open-with-app", any),
                    sep(),
                    item("Folder Compare", None, "Open in New View", directory),
                    item("Folder Sync", None, "open-with-sync", false),
                ],
            ),
            item("Compare To...", Some("F7"), "Compare To", any),
            sep(),
            sub(
                "Compare Contents",
                vec![
                    item("CRC", None, "compare-crc", any),
                    item("Binary", None, "compare-binary", any),
                    item("Rules-based", None, "compare-rules", any),
                ],
            ),
            item(
                &format!("Copy to {to}"),
                Some("Ctrl+R"),
                "Copy to Side",
                any,
            ),
            item(&format!("Move to {to}..."), None, "Move to Side", any),
            item("Copy to Folder...", None, "Copy to Folder", any),
            item("Move to Folder...", None, "Move to Folder", any),
            item("Delete...", None, "Delete", any),
            item("Rename", Some("F2"), "Rename", any),
            item("Attributes...", None, "Attributes", any),
            item("Touch...", None, "Touch", any),
            item("Exclude", None, "Exclude", any),
            item("New Folder...", Some("Ins"), "New Folder", true),
            item("Copy Filename", None, "Copy Filename", any),
            item("Ignored", None, "Ignored", any),
            item(
                "Refresh Selection",
                Some("Shift+F5"),
                "Refresh Selection",
                any,
            ),
            sep(),
            sub(
                "Synchronize",
                vec![
                    item("Update Right...", None, "sync-update-right", false),
                    item("Update Left...", None, "sync-update-left", false),
                    item("Update Both...", None, "sync-update-both", false),
                    item(
                        "Mirror to Right...",
                        Some("Shift+Ctrl+R"),
                        "sync-mirror-right",
                        false,
                    ),
                    item(
                        "Mirror to Left...",
                        Some("Shift+Ctrl+L"),
                        "sync-mirror-left",
                        false,
                    ),
                ],
            ),
            sep(),
            item("Explorer", None, "Explorer", any),
        ]
    }
}

/// Sync-layout row right-click menu (same entries as before the re-skin).
fn row_menu(directory: bool) -> Vec<crate::chrome::menubar::MenuItem> {
    use crate::chrome::menubar::MenuItem;
    let mut items = vec![
        MenuItem::action("Select row", "row:select"),
        MenuItem::action("Open file pair", "row:open"),
    ];
    if directory {
        items.push(MenuItem::action("Expand / collapse", "row:expand"));
    }
    items
}
/// Row menu action -> the view's context command kind.
fn row_command(action: &str) -> Option<usize> {
    match action {
        "row:select" => Some(0),
        "row:open" => Some(1),
        "row:expand" => Some(2),
        _ => None,
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SelectionTotals {
    /// Per side: selected files and their bytes.
    counts: [(usize, u64); 2],
    any: bool,
    only_files: bool,
}
fn selection_totals(snapshot: &Snapshot, selection: &Selection) -> SelectionTotals {
    let mut counts = [(0usize, 0u64); 2];
    let mut any = false;
    let mut only_files = true;
    if !selection.items.is_empty() {
        for row in &snapshot.rows {
            for (side, count) in counts.iter_mut().enumerate() {
                if let Some(item) = &row.sides[side] {
                    if selection.items.contains(&(item.node, side)) {
                        any = true;
                        if item.entry.kind == EntryKind::File {
                            count.0 += 1;
                            count.1 += item.entry.size.unwrap_or(0);
                        } else {
                            only_files = false;
                        }
                    }
                }
            }
        }
    }
    SelectionTotals {
        counts,
        any,
        only_files,
    }
}
/// Order-independent selection fingerprint (count plus a sum of mixed
/// items): O(selection) integer work, cheap enough to take every frame.
fn selection_fingerprint(selection: &Selection) -> (usize, u64) {
    let sum = selection.items.iter().fold(0u64, |acc, &(node, side)| {
        // splitmix64 finalizer over (node, side).
        let mut z = (node as u64).wrapping_mul(2).wrapping_add(side as u64);
        z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        acc.wrapping_add(z ^ (z >> 31))
    });
    (selection.items.len(), sum)
}
/// First dot of a 2 px-period run starting at `start` that can touch
/// `[min, ..)`: skipping fully clipped dots keeps the emitted shapes O(clip)
/// instead of O(run length) without moving the dot phase.
fn dot_start(start: f32, min: f32) -> f32 {
    if min - 1.0 > start {
        start + ((min - 1.0 - start) / 2.0).floor() * 2.0
    } else {
        start
    }
}
fn dotted_v(painter: &egui::Painter, x: f32, y0: f32, y1: f32, color: egui::Color32) {
    let clip = painter.clip_rect();
    let mut y = dot_start(y0, clip.top());
    let y1 = y1.min(clip.bottom() + 1.0);
    while y < y1 {
        painter.rect_filled(
            egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(1.0, 1.0)),
            0.0,
            color,
        );
        y += 2.0;
    }
}
fn dotted_h(painter: &egui::Painter, x0: f32, x1: f32, y: f32, color: egui::Color32) {
    let clip = painter.clip_rect();
    let mut x = dot_start(x0, clip.left());
    let x1 = x1.min(clip.right() + 1.0);
    while x < x1 {
        painter.rect_filled(
            egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(1.0, 1.0)),
            0.0,
            color,
        );
        x += 2.0;
    }
}

/// 状态栏“Shows ...”文本：由显示过滤器推出可见类别。
fn shows_legend(display: bc_engine_dir::filter::DisplayFilter) -> String {
    use bc_engine_dir::filter::DisplayFilter;
    if display == DisplayFilter::None {
        return "Shows no items.".into();
    }
    let mut parts = Vec::new();
    for (label, status) in [
        ("same", CompareStatus::Same),
        ("left orphan", CompareStatus::LeftOrphan),
        ("right orphan", CompareStatus::RightOrphan),
        ("left newer", CompareStatus::LeftNewer),
        ("right newer", CompareStatus::RightNewer),
        ("difference", CompareStatus::Different),
    ] {
        if display.shows(status) {
            parts.push(label);
        }
    }
    if parts.is_empty() {
        return "Shows no items.".into();
    }
    let mut text = String::from("Shows ");
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            text.push_str(if i + 1 == parts.len() { " and " } else { ", " });
        }
        text.push_str(part);
    }
    text.push_str(" items.");
    text
}

fn menu_entry(
    label: &str,
    mnemonic: Option<char>,
    shortcut: Option<&str>,
    children: Vec<MenuItem>,
    id: &str,
) -> MenuItem {
    MenuItem {
        label: label.into(),
        mnemonic,
        shortcut: shortcut.map(str::to_owned),
        kind: if label.is_empty() {
            MenuItemKind::Separator
        } else if children.is_empty() {
            MenuItemKind::Action
        } else {
            MenuItemKind::Submenu(children)
        },
        enabled: false,
        action_id: id.into(),
    }
}

/// 完整转录 inventories.folder 的七个菜单；未实现项保持禁用（外观库存，不是功能承诺）。
fn folder_menu_inventory() -> Vec<MenuSpec> {
    use crate::chrome::menubar::{MenuItem as MI, MenuSpec};
    fn item(label: &str, mnemonic: Option<char>, shortcut: Option<&str>, id: &str) -> MI {
        menu_entry(label, mnemonic, shortcut, vec![], id)
    }
    fn sep() -> MI {
        menu_entry("", None, None, vec![], "")
    }
    fn sub(label: &str, mnemonic: Option<char>, children: Vec<MI>, id: &str) -> MI {
        menu_entry(label, mnemonic, None, children, id)
    }
    let menu = |title: &str, mnemonic: char, items: Vec<MI>| MenuSpec {
        title: title.into(),
        mnemonic: Some(mnemonic),
        items,
    };
    vec![
        menu(
            "Session",
            's',
            vec![
                sub(
                    "New Session",
                    Some('n'),
                    vec![
                        item("Folder Compare", Some('f'), None, "new-folder"),
                        item("Folder Sync", Some('o'), None, "new-sync"),
                        item("Text Compare", Some('t'), None, "new-text"),
                        item("Hex Compare", Some('h'), None, "new-hex"),
                        item("Media Compare", Some('m'), None, "new-media"),
                        item("Picture Compare", Some('p'), None, "new-picture"),
                        item("Table Compare", Some('b'), None, "new-table"),
                        item("Version Compare", Some('v'), None, "new-version"),
                    ],
                    "New Session",
                ),
                item("New Tab", Some('t'), Some("Ctrl+T"), "New Tab"),
                item("New Window", Some('w'), None, "New Window"),
                item(
                    "Open Session",
                    Some('o'),
                    Some("Shift+Ctrl+O"),
                    "Open Session",
                ),
                sep(),
                sub(
                    "Load Workspace",
                    Some('k'),
                    vec![item(
                        "Manage Workspaces...",
                        None,
                        None,
                        "Manage Workspaces",
                    )],
                    "Load Workspace",
                ),
                item("Save Workspace As...", Some('v'), None, "Save Workspace As"),
                sep(),
                item(
                    "Save Session",
                    Some('s'),
                    Some("Shift+Ctrl+S"),
                    "Save Session",
                ),
                item("Save Session As...", Some('a'), None, "Save Session As"),
                item("Session Settings...", Some('e'), None, "Session Settings"),
                item("Locked", Some('l'), None, "Locked"),
                item(
                    "Clear Session",
                    Some('c'),
                    Some("Shift+Ctrl+C"),
                    "Clear Session",
                ),
                item("Close Tab", None, Some("Ctrl+W"), "Close Tab"),
                sep(),
                item("Swap Sides", Some('d'), None, "Swap Sides"),
                item("Back", Some('b'), Some("Alt+Left"), "Back"),
                item("Forward", Some('f'), Some("Alt+Right"), "Forward"),
                sub(
                    "Browse for Folder",
                    Some('o'),
                    vec![
                        item("Left Side", Some('l'), None, "browse-left"),
                        item("Right Side", Some('r'), None, "browse-right"),
                    ],
                    "Browse for Folder",
                ),
                sub(
                    "Up One Level",
                    Some('u'),
                    vec![
                        item("Left Side", Some('l'), None, "up-left"),
                        item("Right Side", Some('r'), None, "up-right"),
                        item("Both Sides", Some('b'), None, "up-both"),
                    ],
                    "Up One Level",
                ),
                sep(),
                item(
                    "Folder Compare Report...",
                    Some('p'),
                    None,
                    "Folder Compare Report",
                ),
                item(
                    "Folder Compare Info",
                    Some('i'),
                    Some("Ctrl+I"),
                    "Folder Compare Info",
                ),
                sep(),
                item("Sync Base Folders", Some('y'), None, "Sync Base Folders"),
                item(
                    "Compare Parent Folders",
                    Some('f'),
                    None,
                    "Compare Parent Folders",
                ),
                sep(),
                item("Exit", Some('x'), Some("Ctrl+Q"), "Exit"),
            ],
        ),
        menu(
            "Actions",
            'a',
            vec![
                item("Open Folder", Some('o'), None, "Open Folder"),
                item("Open Subfolders", Some('s'), None, "Open Subfolders"),
                item("Close Subfolders", Some('c'), None, "Close Subfolders"),
                item(
                    "Set as Base Folders",
                    Some('b'),
                    None,
                    "Set as Base Folders",
                ),
                item("Open in New View", Some('o'), None, "Open in New View"),
                sub(
                    "Open With",
                    Some('w'),
                    vec![
                        item("Associated Application", None, None, "open-with-app"),
                        sep(),
                        item("Folder Compare", Some('f'), None, "open-with-folder"),
                        item("Folder Sync", Some('o'), None, "open-with-sync"),
                        sep(),
                    ],
                    "Open With",
                ),
                sub(
                    "Compare Contents",
                    Some('c'),
                    vec![
                        item("CRC", Some('c'), None, "compare-crc"),
                        item("Binary", Some('b'), None, "compare-binary"),
                        item("Rules-based", Some('r'), None, "compare-rules"),
                    ],
                    "Compare Contents",
                ),
                item("Copy to Side...", Some('y'), None, "Copy to Side"),
                item("Move to Side...", Some('m'), None, "Move to Side"),
                item("Copy to Folder...", Some('f'), None, "Copy to Folder"),
                item("Move to Folder...", Some('v'), None, "Move to Folder"),
                item("Delete...", Some('d'), None, "Delete"),
                item("Rename...", Some('r'), Some("F2"), "Rename"),
                item("Attributes...", Some('a'), None, "Attributes"),
                item("Touch...", Some('t'), None, "Touch"),
                item("Exclude", Some('e'), None, "Exclude"),
                item("New Folder...", Some('n'), Some("Ins"), "New Folder"),
                item("Copy Filename", Some('i'), None, "Copy Filename"),
                item("Ignored", Some('g'), None, "Ignored"),
                item(
                    "Refresh Selection",
                    Some('s'),
                    Some("Shift+F5"),
                    "Refresh Selection",
                ),
                sep(),
                sub(
                    "Synchronize",
                    Some('z'),
                    vec![
                        item("Update Right...", Some('r'), None, "sync-update-right"),
                        item("Update Left...", Some('l'), None, "sync-update-left"),
                        item("Update Both...", Some('b'), None, "sync-update-both"),
                        item(
                            "Mirror to Right...",
                            Some('g'),
                            Some("Shift+Ctrl+R"),
                            "sync-mirror-right",
                        ),
                        item(
                            "Mirror to Left...",
                            Some('f'),
                            Some("Shift+Ctrl+L"),
                            "sync-mirror-left",
                        ),
                    ],
                    "Synchronize",
                ),
                sep(),
                sub("Explorer", Some('x'), vec![], "Explorer"),
            ],
        ),
        menu(
            "Edit",
            'e',
            vec![
                item("Expand All", Some('e'), None, "Expand All"),
                item("Collapse All", Some('c'), None, "Collapse All"),
                sep(),
                item("Select All", Some('a'), Some("Ctrl+A"), "Select All"),
                item(
                    "Select All Files",
                    Some('f'),
                    Some("Shift+Ctrl+A"),
                    "Select All Files",
                ),
                sub(
                    "Select Newer",
                    Some('n'),
                    vec![
                        item("Left Side", Some('l'), None, "select-newer-left"),
                        item("Right Side", Some('r'), None, "select-newer-right"),
                        item("Both Sides", Some('b'), None, "select-newer-both"),
                    ],
                    "Select Newer",
                ),
                sub(
                    "Select Orphans",
                    Some('o'),
                    vec![
                        item("Left Side", Some('l'), None, "select-orphan-left"),
                        item("Right Side", Some('r'), None, "select-orphan-right"),
                        item("Both Sides", Some('b'), None, "select-orphan-both"),
                    ],
                    "Select Orphans",
                ),
                item("Invert Selection", Some('i'), None, "Invert Selection"),
                item("Refresh", Some('r'), Some("F5"), "Refresh"),
                item("Full Refresh", Some('u'), Some("Ctrl+F5"), "Full Refresh"),
            ],
        ),
        menu(
            "Search",
            's',
            vec![
                item(
                    "Next Difference",
                    Some('n'),
                    Some("Ctrl+N"),
                    "Next Difference",
                ),
                item(
                    "Previous Difference",
                    Some('p'),
                    Some("Ctrl+P"),
                    "Previous Difference",
                ),
                item(
                    "Find Filename...",
                    Some('f'),
                    Some("Ctrl+F"),
                    "Find Filename",
                ),
                item(
                    "Find Next Filename",
                    Some('x'),
                    Some("F3"),
                    "Find Next Filename",
                ),
                item(
                    "Find Previous Filename",
                    Some('v'),
                    Some("Shift+F3"),
                    "Find Previous Filename",
                ),
            ],
        ),
        menu(
            "View",
            'v',
            vec![
                item("Show All", None, None, "Show All"),
                item("Show Differences", None, None, "Show Differences"),
                item("Show Same", None, None, "Show Same"),
                item("Show No Orphans", None, None, "Show No Orphans"),
                item(
                    "Show Differences but No Orphans",
                    None,
                    None,
                    "Show Differences but No Orphans",
                ),
                item("Show Orphans", None, None, "Show Orphans"),
                item("Show Left Newer", None, None, "Show Left Newer"),
                item("Show Right Newer", None, None, "Show Right Newer"),
                item(
                    "Show Left Newer and Left Orphans",
                    None,
                    None,
                    "Show Left Newer and Left Orphans",
                ),
                item(
                    "Show Right Newer and Right Orphans",
                    None,
                    None,
                    "Show Right Newer and Right Orphans",
                ),
                item("Show Left Orphans", None, None, "Show Left Orphans"),
                item("Show Right Orphans", None, None, "Show Right Orphans"),
                sep(),
                item(
                    "Always Show Folders",
                    Some('a'),
                    None,
                    "Always Show Folders",
                ),
                item(
                    "Compare Files and Folder Structure",
                    Some('d'),
                    None,
                    "Compare Files and Folder Structure",
                ),
                item("Only Compare Files", Some('f'), None, "Only Compare Files"),
                item(
                    "Ignore Folder Structure",
                    Some('i'),
                    None,
                    "Ignore Folder Structure",
                ),
                sep(),
                item(
                    "Ignore Unimportant Differences",
                    Some('u'),
                    None,
                    "Ignore Unimportant Differences",
                ),
                item("Suppress Filters", Some('s'), None, "Suppress Filters"),
                sep(),
                sub(
                    "Columns",
                    Some('c'),
                    vec![
                        item("Ext", Some('e'), None, "column-ext"),
                        item("Status", Some('s'), None, "column-status"),
                        item("Revision", Some('i'), None, "column-revision"),
                        item("Version", Some('v'), None, "column-version"),
                        item("Size", Some('z'), None, "column-size"),
                        item("CRC", Some('c'), None, "column-crc"),
                        item("Modified", Some('m'), None, "column-modified"),
                        item("Attributes", Some('a'), None, "column-attrs"),
                        item("Owner", Some('o'), None, "column-owner"),
                        item("Group", Some('g'), None, "column-group"),
                    ],
                    "Columns",
                ),
                item("Legend", Some('l'), Some("Ctrl+Alt+L"), "Legend"),
                item("Log", Some('o'), None, "Log"),
                item("Toolbar", Some('t'), None, "Toolbar"),
            ],
        ),
        menu(
            "Tools",
            't',
            vec![
                item("Options...", Some('o'), None, "Options"),
                item("File Formats...", Some('f'), None, "File Formats"),
                item("Profiles...", Some('p'), None, "Profiles"),
                sep(),
                item("Export Settings...", Some('e'), None, "Export Settings"),
                item("Import Settings...", Some('i'), None, "Import Settings"),
                item(
                    "Restore Factory Defaults...",
                    Some('r'),
                    None,
                    "Restore Factory Defaults",
                ),
                sep(),
                item("Save Snapshot...", Some('h'), None, "Save Snapshot"),
                item("Edit Text File", Some('d'), None, "Edit Text File"),
                item("View Patch...", Some('a'), None, "View Patch"),
            ],
        ),
        menu(
            "Help",
            'h',
            vec![
                item("Contents", Some('c'), None, "Contents"),
                item(
                    "Context Sensitive Help",
                    Some('h'),
                    Some("F1"),
                    "Context Sensitive Help",
                ),
                sep(),
                item("Project Website", Some('w'), None, "Project Website"),
                item("Check for Updates", Some('u'), None, "Check for Updates"),
                item("Support", Some('s'), None, "Support"),
                item("Enter Key...", Some('e'), None, "Enter Key"),
                sep(),
                item("About Matcha", Some('a'), None, "About Matcha"),
            ],
        ),
    ]
}

/// 每帧同步菜单启用与勾选状态；子菜单容器有子项即启用，未实现的叶子保持禁用。
struct MenuFlags<'a> {
    view: ViewOptions,
    columns: &'a [Column],
    legend: bool,
    log: bool,
    toolbar: bool,
    back: bool,
    forward: bool,
}
fn update_folder_items(items: &mut [MenuItem], flags: &MenuFlags) {
    use bc_engine_dir::filter::{DisplayFilter as D, FolderDisplay as F};
    let view = flags.view;
    let columns = flags.columns;
    for item in items {
        if let MenuItemKind::Submenu(children) = &mut item.kind {
            item.enabled = !children.is_empty();
            update_folder_items(children, flags);
            continue;
        }
        if matches!(item.kind, MenuItemKind::Separator) {
            continue;
        }
        let mut enabled = true;
        match item.action_id.as_str() {
            "Session Settings"
            | "Swap Sides"
            | "browse-left"
            | "browse-right"
            | "up-left"
            | "up-right"
            | "up-both"
            | "Sync Base Folders"
            | "Compare Parent Folders"
            | "New Tab"
            | "Close Tab"
            | "Exit" => {}
            "Back" => enabled = flags.back,
            "Forward" => enabled = flags.forward,
            "Open Folder"
            | "Open Subfolders"
            | "Close Subfolders"
            | "Set as Base Folders"
            | "Open in New View"
            | "compare-crc"
            | "compare-binary"
            | "compare-rules"
            | "Copy to Side"
            | "Move to Side"
            | "Copy to Folder"
            | "Move to Folder"
            | "Delete"
            | "Rename"
            | "Attributes"
            | "Touch"
            | "Exclude"
            | "New Folder"
            | "Copy Filename"
            | "Ignored"
            | "Refresh Selection" => {}
            "Expand All"
            | "Collapse All"
            | "Select All"
            | "Select All Files"
            | "select-newer-left"
            | "select-newer-right"
            | "select-newer-both"
            | "select-orphan-left"
            | "select-orphan-right"
            | "select-orphan-both"
            | "Invert Selection"
            | "Refresh"
            | "Full Refresh" => {}
            "Next Difference"
            | "Previous Difference"
            | "Find Filename"
            | "Find Next Filename"
            | "Find Previous Filename" => {}
            "Show All" => {
                item.kind = MenuItemKind::Radio {
                    selected: view.display == D::All,
                }
            }
            "Show Differences" => {
                item.kind = MenuItemKind::Radio {
                    selected: view.display == D::Mismatches,
                };
            }
            "Show Same" => {
                item.kind = MenuItemKind::Radio {
                    selected: view.display == D::Matches,
                }
            }
            "Show No Orphans" => {
                item.kind = MenuItemKind::Radio {
                    selected: view.display == D::NoOrphans,
                };
            }
            "Show Differences but No Orphans" => {
                item.kind = MenuItemKind::Radio {
                    selected: view.display == D::MismatchesNoOrphans,
                };
            }
            "Show Orphans" => {
                item.kind = MenuItemKind::Radio {
                    selected: view.display == D::Orphans,
                }
            }
            "Show Left Newer" => {
                item.kind = MenuItemKind::Radio {
                    selected: view.display == D::LeftNewer,
                };
            }
            "Show Right Newer" => {
                item.kind = MenuItemKind::Radio {
                    selected: view.display == D::RightNewer,
                };
            }
            "Show Left Newer and Left Orphans" => {
                item.kind = MenuItemKind::Radio {
                    selected: view.display == D::LeftNewerOrphans,
                };
            }
            "Show Right Newer and Right Orphans" => {
                item.kind = MenuItemKind::Radio {
                    selected: view.display == D::RightNewerOrphans,
                };
            }
            "Show Left Orphans" => {
                item.kind = MenuItemKind::Radio {
                    selected: view.display == D::LeftOrphans,
                };
            }
            "Show Right Orphans" => {
                item.kind = MenuItemKind::Radio {
                    selected: view.display == D::RightOrphans,
                };
            }
            "Always Show Folders" => {
                item.kind = MenuItemKind::Radio {
                    selected: view.folders == F::AlwaysShow,
                };
            }
            "Compare Files and Folder Structure" => {
                item.kind = MenuItemKind::Radio {
                    selected: view.folders == F::FilesAndStructure,
                };
            }
            "Only Compare Files" => {
                item.kind = MenuItemKind::Radio {
                    selected: view.folders == F::FilesOnly,
                };
            }
            "Ignore Folder Structure" => {
                item.kind = MenuItemKind::Radio {
                    selected: view.folders == F::Flatten,
                };
            }
            "Ignore Unimportant Differences" => {
                item.kind = MenuItemKind::Check {
                    checked: view.ignore_unimportant,
                };
            }
            "Suppress Filters" => {
                item.kind = MenuItemKind::Check {
                    checked: view.suppress_filters,
                };
            }
            "column-size" => {
                item.kind = MenuItemKind::Check {
                    checked: columns.contains(&Column::Size),
                };
            }
            "column-modified" => {
                item.kind = MenuItemKind::Check {
                    checked: columns.contains(&Column::Modified),
                };
            }
            "column-crc" => {
                item.kind = MenuItemKind::Check {
                    checked: columns.contains(&Column::Crc),
                };
            }
            "column-version" => {
                item.kind = MenuItemKind::Check {
                    checked: columns.contains(&Column::Version),
                };
            }
            "column-attrs" => {
                item.kind = MenuItemKind::Check {
                    checked: columns.contains(&Column::Attrs),
                };
            }
            "column-ext" | "column-status" | "column-revision" | "column-owner"
            | "column-group" => enabled = false,
            "Legend" => {
                item.kind = MenuItemKind::Check {
                    checked: flags.legend,
                }
            }
            "Log" => item.kind = MenuItemKind::Check { checked: flags.log },
            "Toolbar" => {
                item.kind = MenuItemKind::Check {
                    checked: flags.toolbar,
                }
            }
            _ => enabled = false,
        }
        item.enabled = enabled;
    }
}

/// 参考列宽：Size 78 / Modified 150；Name 吸收剩余宽度。附加列沿用旧宽度。
fn chrome_column_width(column: Column) -> f32 {
    match column {
        Column::Size => 78.0,
        Column::Modified => 150.0,
        other => other.width().max(60.0),
    }
}

impl DirCompareView {
    pub fn take_home_request(&mut self) -> bool {
        std::mem::take(&mut self.home_requested)
    }
    /// Base folder texts of both sides (sync paths area).
    pub(crate) fn path_texts(&self) -> [String; 2] {
        [self.paths.left.clone(), self.paths.right.clone()]
    }
    /// Load new base folders (sync paths area Enter / history).
    pub(crate) fn open_paths(&mut self, paths: [String; 2]) {
        self.reload(paths, true);
    }
    /// Applied file-name filter text (sync Filters box).
    pub(crate) fn filter_names(&self) -> &str {
        &self.filters.names
    }
    /// Apply a new file-name filter through the existing view-options path.
    pub(crate) fn set_filter_names(&mut self, names: String) {
        self.filters.names = names;
        self.apply_view_options();
    }
    pub(crate) fn display(&self) -> bc_engine_dir::filter::DisplayFilter {
        self.filters.options.display
    }
    pub(crate) fn toolbar_visible(&self) -> bool {
        self.toolbar
    }
    pub(crate) fn log_visible(&self) -> bool {
        self.log_panel.open
    }
    /// Check state of shared view commands (menus/toolbar toggles).
    pub(crate) fn view_checked(&self, id: &str) -> Option<bool> {
        Some(match id {
            "Ignore Unimportant Differences" => self.filters.options.ignore_unimportant,
            "Suppress Filters" => self.filters.options.suppress_filters,
            "Legend" => self.legend_open,
            "Log" => self.log_panel.open,
            "Toolbar" => self.toolbar,
            "column-size" => self.columns.contains(&Column::Size),
            "column-modified" => self.columns.contains(&Column::Modified),
            "column-crc" => self.columns.contains(&Column::Crc),
            "column-version" => self.columns.contains(&Column::Version),
            "column-attrs" => self.columns.contains(&Column::Attrs),
            _ => return None,
        })
    }
    /// 快照预置：与参考操作等价（folder-expanded = Ctrl+A + Expand All；
    /// folder-diffs-filter = 工具栏 Diffs；folder-basic = 折叠 + 首行选中）。
    pub fn snapshot_preset(&mut self, scene: &str) {
        match scene {
            "folder-expanded" => {
                // 参考操作序：Ctrl+A（只选中可见的顶层行）后 Edit > Expand All。
                self.selection.items.clear();
                for row in &self.snapshot.rows {
                    if row.depth == 0 {
                        for side in 0..2 {
                            if let Some(item) = &row.sides[side] {
                                self.selection.insert(item.node, side);
                            }
                        }
                    }
                }
                if self.row_count() > 0 {
                    self.focus = Some(0);
                }
                self.send(Command::ExpandAll(true));
            }
            "folder-diffs-filter" => {
                self.filters.options.display = bc_engine_dir::filter::DisplayFilter::Mismatches;
                self.apply_view_options();
                if self.row_count() > 0 {
                    self.focus_row(0, None);
                }
            }
            "dlg-folder-session-settings" => {
                let draft = self.settings_draft();
                self.settings
                    .show(draft, crate::chrome::dialog::Placement::ClientOnly);
            }
            _ => {
                if self.row_count() > 0 {
                    self.focus_row(0, None);
                }
            }
        }
    }
    /// 以当前 filters 状态通知工作线程重算（显示过滤/文件夹模式/不重要/Peek）。
    fn apply_view_options(&mut self) {
        self.send(Command::Filters {
            names: self.filters.effective_names(),
            other: self.filters.other.clone(),
            content: self.filters.content.clone(),
            exclude_containing: self.filters.exclude_containing,
            view: self.filters.options,
        });
    }
    fn open_operation(&mut self, kind: Kind) {
        let mut dialog = OperationDialog::new(kind);
        dialog.include_hidden =
            self.handling.include_hidden || self.filters.options.suppress_filters;
        dialog.recent = self.paths.recent.clone();
        self.operation = Some(dialog);
    }
    fn open_scope(&mut self, op: DirOp) {
        let both = self
            .selection
            .items
            .iter()
            .map(|(_, s)| *s)
            .collect::<BTreeSet<_>>()
            .len()
            > 1;
        let direction = both && matches!(op, DirOp::CopyOther | DirOp::MoveOther);
        self.scope = Some(dialogs::scope::ScopeDialog::new(
            op,
            self.handling.include_hidden || self.filters.options.suppress_filters,
            direction,
        ));
    }
    fn toggle_column(&mut self, column: Column) {
        if self.columns.contains(&column) {
            self.columns.retain(|c| *c != column);
        } else {
            self.columns.push(column);
            if matches!(column, Column::Crc | Column::Version) {
                self.send(Command::Details {
                    selection: (!self.selection.items.is_empty()).then(|| self.selection.clone()),
                    crc: column == Column::Crc,
                    version: column == Column::Version,
                });
            }
        }
    }
    /// Ignored 为切换：所选全部已忽略时清除，否则标记。
    fn toggle_ignored(&mut self) {
        let nodes: BTreeSet<_> = self.selection.items.iter().map(|(n, _)| *n).collect();
        let all_ignored = !nodes.is_empty()
            && self
                .snapshot
                .rows
                .iter()
                .filter(|r| r.node.is_some_and(|n| nodes.contains(&n)))
                .all(|r| r.ignored);
        self.open_scope(DirOp::Ignored(!all_ignored));
    }
    fn set_display(&mut self, display: bc_engine_dir::filter::DisplayFilter) {
        self.filters.options.display = display;
        self.apply_view_options();
    }
    fn set_folder_display(&mut self, folders: FolderDisplay) {
        self.filters.options.folders = folders;
        self.apply_view_options();
    }
    fn update_menus(&mut self) {
        let flags = MenuFlags {
            view: self.filters.options,
            columns: &self.columns,
            legend: self.legend_open,
            log: self.log_panel.open,
            toolbar: self.toolbar,
            back: self.history_at > 0,
            forward: self.history_at + 1 < self.history.len(),
        };
        let mut menus = std::mem::take(&mut self.menus);
        for menu in &mut menus {
            update_folder_items(&mut menu.items, &flags);
        }
        self.menus = menus;
    }
    /// 菜单与工具栏统一动作入口；返回 Some 表示要切换比较/同步视图。
    pub(crate) fn folder_command(&mut self, id: &str) -> Option<FolderNavigation> {
        use bc_engine_dir::filter::{DisplayFilter as D, FolderDisplay as F};
        match id {
            // Home 只切换标签页，内嵌文本窗口保持打开。
            "Home" => self.home_requested = true,
            "Close Tab" => self.request_tab_close(),
            "New Tab" => self.new_tab_requested = true,
            "Session Settings" => self.open_settings(),
            "Swap Sides" => self.reload([self.paths.right.clone(), self.paths.left.clone()], true),
            "Back" => {
                if self.history_at > 0 {
                    self.history_at -= 1;
                    self.reload(self.history[self.history_at].clone(), false);
                }
            }
            "Forward" => {
                if self.history_at + 1 < self.history.len() {
                    self.history_at += 1;
                    self.reload(self.history[self.history_at].clone(), false);
                }
            }
            "browse-left" => self.browse_folder(0),
            "browse-right" => self.browse_folder(1),
            "up-left" => self.up(Target::Left),
            "up-right" => self.up(Target::Right),
            "up-both" => self.up(Target::All),
            "Sync Base Folders" => return Some((true, self.paths())),
            "Compare Parent Folders" => {
                let paths = self
                    .paths()
                    .map(|p| p.parent().map(Path::to_path_buf).unwrap_or(p));
                self.new_folder_view(paths);
            }
            "Exit" => self.exit_requested = true,
            "Expand All" => self.send(Command::ExpandAll(true)),
            "Collapse All" => self.send(Command::ExpandAll(false)),
            "Select All" => {
                self.selection.items.clear();
                self.select_visible("all", false);
            }
            "Select All Files" => {
                self.selection.items.clear();
                self.select_visible("files", false);
            }
            "select-newer-left" | "select-newer-right" | "select-newer-both" => {
                self.selection.items.clear();
                let mask = match id {
                    "select-newer-left" => "newer-left",
                    "select-newer-right" => "newer-right",
                    _ => "newer",
                };
                self.select_visible(mask, false);
            }
            "select-orphan-left" | "select-orphan-right" | "select-orphan-both" => {
                self.selection.items.clear();
                let mask = match id {
                    "select-orphan-left" => "orphan-left",
                    "select-orphan-right" => "orphan-right",
                    _ => "orphans",
                };
                self.select_visible(mask, false);
            }
            "Invert Selection" => self.select_visible("all", true),
            "Refresh" => self.send(Command::Refresh {
                selection: None,
                full: false,
            }),
            "Full Refresh" => self.send(Command::Refresh {
                selection: None,
                full: true,
            }),
            "Refresh Selection" => self.send(Command::Refresh {
                selection: Some(self.selection.clone()),
                full: false,
            }),
            "Next Difference" => self.navigate(false, false),
            "Previous Difference" => self.navigate(true, false),
            "Find Filename" => self.find.open = true,
            "Find Next Filename" => self.find_next(false),
            "Find Previous Filename" => self.find_next(true),
            "Show All" => self.set_display(D::All),
            "Show Differences" => self.set_display(D::Mismatches),
            "Show Same" => self.set_display(D::Matches),
            "Show No Orphans" => self.set_display(D::NoOrphans),
            "Show Differences but No Orphans" => self.set_display(D::MismatchesNoOrphans),
            "Show Orphans" => self.set_display(D::Orphans),
            "Show Left Newer" => self.set_display(D::LeftNewer),
            "Show Right Newer" => self.set_display(D::RightNewer),
            "Show Left Newer and Left Orphans" => self.set_display(D::LeftNewerOrphans),
            "Show Right Newer and Right Orphans" => self.set_display(D::RightNewerOrphans),
            "Show Left Orphans" => self.set_display(D::LeftOrphans),
            "Show Right Orphans" => self.set_display(D::RightOrphans),
            "Always Show Folders" => self.set_folder_display(F::AlwaysShow),
            "Compare Files and Folder Structure" => self.set_folder_display(F::FilesAndStructure),
            "Only Compare Files" => self.set_folder_display(F::FilesOnly),
            "Ignore Folder Structure" => self.set_folder_display(F::Flatten),
            "Ignore Unimportant Differences" => {
                self.filters.options.ignore_unimportant = !self.filters.options.ignore_unimportant;
                self.apply_view_options();
            }
            "Suppress Filters" => {
                self.filters.options.suppress_filters = !self.filters.options.suppress_filters;
                self.apply_view_options();
            }
            "column-size" => self.toggle_column(Column::Size),
            "column-modified" => self.toggle_column(Column::Modified),
            "column-crc" => self.toggle_column(Column::Crc),
            "column-version" => self.toggle_column(Column::Version),
            "column-attrs" => self.toggle_column(Column::Attrs),
            "Legend" => self.legend_open = !self.legend_open,
            "Log" => self.log_panel.open = !self.log_panel.open,
            "Toolbar" => self.toolbar = !self.toolbar,
            "Open Folder" => self.expand_selection(true, false),
            "Close Folder" => self.expand_selection(false, false),
            "Open Pair" => self.open_pair(),
            "Compare To" => {
                self.compare_to = true;
                self.align_to = false;
            }
            "Align With" => {
                self.align_to = true;
                self.compare_to = false;
            }
            "quick-size" | "quick-crc" | "quick-binary" | "quick-rules" => {
                let mode = match id {
                    "quick-size" => ContentCompare::Size,
                    "quick-crc" => ContentCompare::Crc,
                    "quick-binary" => ContentCompare::Binary,
                    _ => ContentCompare::RulesBased,
                };
                match self.selected_pair() {
                    Ok(paths) => self.send(Command::Quick(paths, mode)),
                    Err(e) => self.message = e,
                }
            }
            "Exchange" => self.open_scope(DirOp::Exchange),
            "Set as Base Folder" => self.set_base(false, false),
            "Set as Base on Other Side" => self.set_base(true, false),
            "Open Subfolders" => self.expand_selection(true, true),
            "Close Subfolders" => self.expand_selection(false, true),
            "Set as Base Folders" => self.set_base(false, true),
            "Open in New View" => self.open_folder_selection(),
            "compare-crc" => self.send(Command::Compare {
                selection: self.selection.clone(),
                mode: ContentCompare::Crc,
            }),
            "compare-binary" => self.send(Command::Compare {
                selection: self.selection.clone(),
                mode: ContentCompare::Binary,
            }),
            "compare-rules" => self.send(Command::Compare {
                selection: self.selection.clone(),
                mode: ContentCompare::RulesBased,
            }),
            "Copy to Side" => self.open_scope(DirOp::CopyOther),
            "Move to Side" => self.open_scope(DirOp::MoveOther),
            "Copy to Folder" => self.open_operation(Kind::CopyTo),
            "Move to Folder" => self.open_operation(Kind::MoveTo),
            "Delete" => self.open_operation(Kind::Delete),
            "Rename" => self.open_operation(Kind::Rename),
            "Attributes" => self.open_operation(Kind::Attributes),
            "Touch" => self.open_operation(Kind::Touch),
            "Exclude" => self.open_scope(DirOp::Exclude),
            "New Folder" => self.open_operation(Kind::NewFolder),
            "Copy Filename" => self.operate(DirOp::CopyFilename, false),
            "open-with-app" | "Explorer" => match self.selected_sides().first() {
                Some((_, item)) => {
                    if let Err(e) = crate::text::view::shell_open(&item.path, id == "Explorer") {
                        self.message = format!("Cannot open {}: {e}", item.path.display());
                    }
                }
                None => self.message = "Select an item first".into(),
            },
            "Ignored" => self.toggle_ignored(),
            "Stop" => self.cancel(),
            "Show Files" => {
                self.show_files = !self.show_files;
                self.send(Command::ShowFiles(self.show_files));
            }
            _ => {}
        }
        None
    }
    fn chrome_toolbar_items(&self, busy: bool) -> Vec<crate::chrome::toolbar::ToolItem> {
        use crate::chrome::{
            icons::Icon,
            toolbar::{ToolItem, ToolItemKind as Kind},
        };
        use bc_engine_dir::filter::DisplayFilter as D;
        let display = self.filters.options.display;
        let mk = |icon: Icon,
                  label: &str,
                  id: &str,
                  kind: Kind,
                  group: &str,
                  enabled: bool,
                  selected: bool| {
            ToolItem {
                label: label.into(),
                action_id: id.into(),
                kind,
                enabled,
                selected,
                icon,
                group: Some(group.into()),
            }
        };
        vec![
            mk(
                Icon::Home,
                "Home",
                "Home",
                Kind::Normal,
                "home",
                true,
                false,
            ),
            mk(
                Icon::Sessions,
                "Sessions",
                "Sessions",
                Kind::Dropdown,
                "home",
                true,
                false,
            ),
            mk(
                Icon::All,
                "All",
                "Show All",
                Kind::Toggle,
                "filter",
                true,
                display == D::All,
            ),
            mk(
                Icon::Diffs,
                "Diffs",
                "Show Differences",
                Kind::Split,
                "filter",
                true,
                display == D::Mismatches,
            ),
            mk(
                Icon::Same,
                "Same",
                "Show Same",
                Kind::Toggle,
                "filter",
                true,
                display == D::Matches,
            ),
            mk(
                Icon::Files,
                "Files",
                "Show Files",
                Kind::Split,
                "minor",
                true,
                self.show_files,
            ),
            mk(
                Icon::Minor,
                "Minor",
                "Ignore Unimportant Differences",
                Kind::Toggle,
                "minor",
                true,
                self.filters.options.ignore_unimportant,
            ),
            mk(
                Icon::Rules,
                "Rules",
                "Session Settings",
                Kind::Normal,
                "rules",
                true,
                false,
            ),
            mk(
                Icon::Copy,
                "Copy",
                "Copy to Side",
                Kind::Normal,
                "copy",
                true,
                false,
            ),
            mk(
                Icon::Expand,
                "Expand",
                "Expand All",
                Kind::Normal,
                "tree",
                true,
                false,
            ),
            mk(
                Icon::Collapse,
                "Collapse",
                "Collapse All",
                Kind::Normal,
                "tree",
                true,
                false,
            ),
            mk(
                Icon::Select,
                "Select",
                "Select All",
                Kind::Normal,
                "tree",
                true,
                false,
            ),
            mk(
                Icon::Files,
                "Files",
                "Select All Files",
                Kind::Normal,
                "files",
                true,
                false,
            ),
            mk(
                Icon::Refresh,
                "Refresh",
                "Refresh",
                Kind::Normal,
                "files",
                true,
                false,
            ),
            mk(
                Icon::Swap,
                "Swap",
                "Swap Sides",
                Kind::Normal,
                "swap",
                true,
                false,
            ),
            mk(
                Icon::Stop,
                "Stop",
                "Stop",
                Kind::Normal,
                "swap",
                busy,
                false,
            ),
        ]
    }
    /// 工具栏下拉（Sessions / Diffs / Files）：Area 弹出，选择后映射到同一动作入口。
    /// Toolbar drop-down menus (Sessions / Show Differences / Show Files) in
    /// the shared popup style, opened under the clicked toolbar item.
    fn chrome_dropdown(
        &mut self,
        ui: &mut egui::Ui,
        response: &crate::chrome::toolbar::ToolbarResponse,
    ) -> Option<FolderNavigation> {
        use crate::chrome::{popup::item, toolbar};
        use bc_engine_dir::filter::DisplayFilter as D;
        let id = egui::Id::unique(("folder-tool-dropdown", self.window_id));
        let display = self.filters.options.display;
        let folders = self.filters.options.folders;
        let show_files = self.show_files;
        let picked = toolbar::dropdown_menu(ui.ctx(), id, response, |name| match name {
            "Sessions" => vec![
                item::action("Home", "Home", true),
                item::action("Session Settings...", "Session Settings", true),
                item::action("Open Session", "Open Session", false),
                item::action("Save Session", "Save Session", false),
            ],
            "Show Differences" => [
                ("Show All", D::All),
                ("Show Differences", D::Mismatches),
                ("Show Same", D::Matches),
                ("Show No Orphans", D::NoOrphans),
                ("Show Differences but No Orphans", D::MismatchesNoOrphans),
                ("Show Orphans", D::Orphans),
                ("Show Left Newer", D::LeftNewer),
                ("Show Right Newer", D::RightNewer),
                ("Show Left Newer and Left Orphans", D::LeftNewerOrphans),
                ("Show Right Newer and Right Orphans", D::RightNewerOrphans),
                ("Show Left Orphans", D::LeftOrphans),
                ("Show Right Orphans", D::RightOrphans),
            ]
            .into_iter()
            .map(|(label, mode)| item::radio(label, label, display == mode))
            .collect(),
            "Show Files" => {
                let mut items = vec![
                    item::check("Show Files", "Show Files", show_files),
                    item::separator(),
                ];
                items.extend(
                    [
                        ("Always Show Folders", FolderDisplay::AlwaysShow),
                        (
                            "Compare Files and Folder Structure",
                            FolderDisplay::FilesAndStructure,
                        ),
                        ("Only Compare Files", FolderDisplay::FilesOnly),
                        ("Ignore Folder Structure", FolderDisplay::Flatten),
                    ]
                    .into_iter()
                    .map(|(label, mode)| item::radio(label, label, folders == mode)),
                );
                items
            }
            _ => Vec::new(),
        })?;
        self.folder_command(&picked)
    }
    fn chrome_ui(&mut self, ui: &mut egui::Ui, busy: bool) -> Option<FolderNavigation> {
        use crate::chrome::{menubar, theme, toolbar};
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
        let mut switch = None;
        // 绝对布局：子 ui 会推进父游标，所有条带改为从 full 直接计算。
        let full = ui.available_rect_before_wrap();
        let mut y = full.top();
        // 菜单栏 27 px。
        self.update_menus();
        let menu_rect = egui::Rect::from_min_size(
            egui::pos2(full.left(), y),
            egui::vec2(full.width(), theme::MENU_HEIGHT),
        );
        let mut menu_action = None;
        ui.scope_builder(egui::UiBuilder::new().max_rect(menu_rect), |ui| {
            menu_action = menubar::show(ui, &self.menus);
        });
        y = menu_rect.bottom();
        if let Some(action) = menu_action {
            switch = self.folder_command(&action);
        }
        // 工具栏 43 px。
        let mut tool_response = toolbar::ToolbarResponse::default();
        if self.toolbar {
            let tool_rect = egui::Rect::from_min_size(
                egui::pos2(full.left(), y),
                egui::vec2(full.width(), theme::TOOLBAR_HEIGHT),
            );
            let items = self.chrome_toolbar_items(busy);
            let mut response = toolbar::ToolbarResponse::default();
            ui.scope_builder(egui::UiBuilder::new().max_rect(tool_rect), |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                response = toolbar::show(ui, &items);
            });
            y = tool_rect.bottom();
            if let Some(action) = response.clicked.clone() {
                switch = self.folder_command(&action).or(switch);
            }
            tool_response = response;
        }
        if let Some(action) = self.chrome_dropdown(ui, &tool_response) {
            switch = Some(action);
        }
        // 过滤栏 45 px。
        let bar = egui::Rect::from_min_size(
            egui::pos2(full.left(), y),
            egui::vec2(full.width(), CHROME_FILTERBAR),
        );
        y = bar.bottom();
        let change = self.filters.chrome(ui, bar);
        if change.save_presets {
            self.send(Command::SavePresets(self.filters.presets.to_xml()));
        }
        if change.apply {
            self.apply_view_options();
        }
        // 路径栏 26 px。
        let bar = egui::Rect::from_min_size(
            egui::pos2(full.left(), y),
            egui::vec2(full.width(), CHROME_PATHBAR),
        );
        y = bar.bottom();
        self.path_bar(ui, bar);
        // 剩余：双树 + 日志 + 状态栏。
        let avail = egui::Rect::from_min_max(egui::pos2(full.left(), y), full.max);
        let status_h = theme::STATUS_HEIGHT;
        let log_h = if self.log_panel.open {
            FOLDER_LOG.last(avail.height() - status_h)
        } else {
            0.0
        };
        let grid_h = (avail.height() - log_h - status_h).max(80.0);
        let grid = egui::Rect::from_min_size(avail.min, egui::vec2(avail.width(), grid_h));
        let log_rect = egui::Rect::from_min_size(
            egui::pos2(avail.left(), grid.bottom()),
            egui::vec2(avail.width(), log_h),
        );
        let status = egui::Rect::from_min_size(
            egui::pos2(avail.left(), grid.bottom() + log_h),
            egui::vec2(avail.width(), status_h),
        );
        self.draw_grid(ui, grid);
        if let Some(action) = self.context_action.take() {
            switch = self.folder_command(&action).or(switch);
        }
        if log_h > 0.0 {
            self.draw_log(ui, log_rect);
        }
        self.draw_status(ui, status, busy);
        ui.allocate_rect(full, egui::Sense::hover());
        if log_h > 0.0 {
            // 日志面板上沿可上下拖动调整高度（持久化，双击复位）。
            crate::chrome::splitter::horizontal_line(
                ui,
                &FOLDER_LOG,
                ("folder-log", self.window_id),
                log_rect.top(),
                log_rect.x_range(),
                egui::Rangef::new(avail.top(), status.top()),
            );
        }
        switch
    }
    /// 窗口总宽 → (左树宽, 右树宽)；中缝与右侧滚动条固定，分隔位置按 [`FOLDER_SPLIT_X`]。
    pub(crate) fn chrome_split(width: f32, gap: f32) -> (f32, f32) {
        let content = (width - gap - CHROME_SCROLLBAR).max(2.0);
        let left = FOLDER_SPLIT_X.first(content).ceil().min(content);
        (left, content - left)
    }
    /// 参考路径栏（26 px）：可编辑组合框 + 内嵌历史下拉 + 小图标按钮。
    /// 按钮接线全部复用既有动作：回退/浏览/上一级/两侧同级（左），右侧另有带回退列表的前进/回退。
    fn path_bar(&mut self, ui: &mut egui::Ui, bar: egui::Rect) {
        use crate::chrome::{icons, icons::Icon};
        let painter = ui.painter().clone();
        // 顶部 1 px 白色高光（参考 y115）。
        painter.hline(
            bar.left()..=bar.right(),
            bar.top(),
            egui::Stroke::new(1.0, egui::Color32::WHITE),
        );
        let (left_w, _) = Self::chrome_split(bar.width(), CHROME_GAP);
        let split_x = bar.left() + left_w + CHROME_GAP;
        let mut reload: Option<[String; 2]> = None;
        for side in 0..2 {
            let origin = if side == 0 { bar.left() } else { split_x };
            // 组合框宽度跟随分隔线（默认窗口下即参考的 592 / 542 px），右侧留给按钮。
            let region = if side == 0 {
                split_x - bar.left()
            } else {
                bar.right() - split_x
            };
            let combo_w = (region - 1.0 - PATH_BUTTONS[side]).max(80.0);
            let combo = egui::Rect::from_min_size(
                egui::pos2(origin + 1.0, bar.top() + 1.0),
                egui::vec2(combo_w, 25.0),
            );
            let palette = crate::chrome::theme::palette(ui.ctx());
            painter.rect_filled(combo, crate::chrome::theme::RADIUS_MD, palette.surface);
            painter.rect_stroke(
                combo,
                crate::chrome::theme::RADIUS_MD,
                egui::Stroke::new(1.0, palette.line_strong),
                egui::StrokeKind::Inside,
            );
            let edit_rect = egui::Rect::from_min_max(
                combo.min + egui::vec2(5.0, 1.0),
                egui::pos2(combo.right() - 21.0, combo.bottom() - 1.0),
            );
            let mut submitted = false;
            let edit_id = egui::Id::unique(("folder-path-edit", self.window_id, side));
            let text = if side == 0 {
                &mut self.paths.left
            } else {
                &mut self.paths.right
            };
            ui.scope_builder(
                egui::UiBuilder::new().max_rect(edit_rect).id_salt(edit_id),
                |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                    // 选区蓝底，与 chrome::pathbar 一致。
                    ui.visuals_mut().selection.bg_fill = crate::chrome::theme::TEXT_SELECTION;
                    ui.visuals_mut().selection.stroke =
                        egui::Stroke::new(1.0, crate::chrome::theme::TEXT);
                    let text_id = edit_id.with("text");
                    crate::chrome::pathbar::clean_paste_events(ui.ctx(), text_id);
                    let edit = ui.add(
                        egui::TextEdit::singleline(text)
                            .id(text_id)
                            .frame(egui::Frame::NONE)
                            .font(egui::FontId::proportional(crate::chrome::theme::UI_SIZE))
                            .margin(egui::vec2(2.0, 5.0)),
                    );
                    crate::chrome::pathbar::select_all_on_first_focus(ui, &edit, text);
                    if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        submitted = true;
                    }
                },
            );
            if submitted {
                reload = Some([self.paths.left.clone(), self.paths.right.clone()]);
            }
            // 内嵌历史下拉箭头。
            let chevron = egui::Rect::from_min_size(
                egui::pos2(combo.right() - 21.0, combo.top()),
                egui::vec2(20.0, combo.height()),
            );
            let c = chevron.center();
            painter.add(egui::Shape::line(
                vec![
                    c + egui::vec2(-3.5, -1.5),
                    c + egui::vec2(0.0, 2.0),
                    c + egui::vec2(3.5, -1.5),
                ],
                egui::Stroke::new(1.4, palette.text_muted),
            ));
            let chev = ui.interact(chevron, edit_id.with("chevron"), egui::Sense::click());
            // 历史列表：共享弹出菜单，锚在整个组合框下方。
            let menu_id = chev.id.with("menu-below");
            if chev.clicked() {
                crate::chrome::popup::toggle_menu_below(ui.ctx(), menu_id, combo);
            }
            let recent = &self.paths.recent;
            let current = if side == 0 {
                &self.paths.left
            } else {
                &self.paths.right
            };
            let picked = crate::chrome::popup::menu(ui.ctx(), menu_id, combo.width(), || {
                use crate::chrome::popup::item;
                if recent.is_empty() {
                    return vec![item::action("No recent folders", "", false)];
                }
                recent
                    .iter()
                    .enumerate()
                    .map(|(i, entry)| item::radio(entry.clone(), i.to_string(), entry == current))
                    .collect()
            })
            .and_then(|i| i.parse::<usize>().ok())
            .and_then(|i| self.paths.recent.get(i).cloned());
            if let Some(path) = picked {
                if side == 0 {
                    self.paths.left = path;
                } else {
                    self.paths.right = path;
                }
                reload = Some([self.paths.left.clone(), self.paths.right.clone()]);
            }
            // 小图标按钮：位置按参考实测偏移（相对组合框右缘）。
            let back = self.history_at > 0;
            let forward = self.history_at + 1 < self.history.len();
            let mut buttons = if side == 0 {
                vec![
                    (Icon::UpLevel, 5.0, "back", back, false),
                    (Icon::Browse, 28.0, "browse", true, false),
                    (Icon::FolderUp, 51.0, "up", true, false),
                    (Icon::FolderUpBoth, 81.0, "up-both", true, false),
                ]
            } else {
                vec![
                    (Icon::UpLevel, 12.0, "back", back, false),
                    (Icon::Browse, 27.0, "browse", true, false),
                    (Icon::FolderUp, 51.0, "up", true, false),
                    (Icon::BackCircle, 76.0, "back", back, true),
                    (Icon::ForwardCircle, 113.0, "forward", forward, true),
                ]
            };
            for (icon, rel, id, enabled, chevron) in buttons.drain(..) {
                let slot = egui::Rect::from_min_size(
                    egui::pos2(combo.right() + rel, bar.top() + 1.0),
                    egui::vec2(22.0, 25.0),
                );
                let response = ui.interact(
                    slot,
                    edit_id.with(("btn", id)),
                    if enabled {
                        egui::Sense::click()
                    } else {
                        egui::Sense::hover()
                    },
                );
                if response.hovered() && enabled {
                    painter.rect_filled(
                        slot.shrink(1.0),
                        crate::chrome::theme::RADIUS_MD,
                        crate::chrome::theme::HOVER,
                    );
                }
                let mut icon_painter = painter.clone();
                if !enabled {
                    icon_painter.set_opacity(0.35);
                }
                icons::paint(
                    &icon_painter,
                    egui::Rect::from_center_size(
                        egui::pos2(slot.left() + 8.0, slot.center().y),
                        egui::vec2(16.0, 16.0),
                    ),
                    icon,
                );
                if chevron {
                    icons::dropdown_triangle(
                        &painter,
                        egui::pos2(slot.left() + 20.0, slot.center().y),
                        if enabled {
                            crate::chrome::theme::TEXT_MUTED
                        } else {
                            crate::chrome::theme::TEXT_FAINT
                        },
                    );
                }
                let tip = match icon {
                    Icon::UpLevel => "Back",
                    Icon::Browse => "Browse for folder",
                    Icon::FolderUp => "Up one level",
                    Icon::FolderUpBoth => "Up one level (both sides)",
                    Icon::BackCircle => "Back (with history)",
                    _ => "Forward (with history)",
                };
                if response.clicked() {
                    match id {
                        "back" => {
                            if back {
                                self.history_at -= 1;
                                reload = Some(self.history[self.history_at].clone());
                            }
                        }
                        "forward" => {
                            if forward {
                                self.history_at += 1;
                                reload = Some(self.history[self.history_at].clone());
                            }
                        }
                        "browse" => self.browse_folder(side),
                        "up" => self.up(if side == 0 {
                            Target::Left
                        } else {
                            Target::Right
                        }),
                        _ => self.up(Target::All),
                    }
                }
                response.on_hover_text(tip);
            }
        }
        if let Some(paths) = reload {
            let remember = !self
                .history
                .get(self.history_at)
                .is_some_and(|current| *current == paths);
            self.reload(paths, remember);
        }
    }
    /// 双树 + 列头 + 中缝 + 右侧平坦滚动条。绘制只访问可见行。
    /// Width of the column between the trees: compare 37 px, sync 117 px.
    fn center_gap(&self) -> f32 {
        if self.sync_mode {
            SYNC_GAP
        } else {
            CHROME_GAP
        }
    }
    pub(crate) fn draw_grid(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let gap = self.center_gap();
        let (left_w, right_w) = Self::chrome_split(rect.width(), gap);
        let painter = ui.painter().clone();
        let left = egui::Rect::from_min_size(rect.min, egui::vec2(left_w, rect.height()));
        let right = egui::Rect::from_min_size(
            egui::pos2(rect.left() + left_w + gap, rect.top()),
            egui::vec2(right_w, rect.height()),
        );
        let scroll = egui::Rect::from_min_size(
            egui::pos2(right.right(), rect.top()),
            egui::vec2(CHROME_SCROLLBAR, rect.height()),
        );
        // 列头（24 px）。
        let mut sort = None;
        for side in 0..2 {
            let tree = if side == 0 { left } else { right };
            let header =
                egui::Rect::from_min_size(tree.min, egui::vec2(tree.width(), CHROME_HEADER));
            painter.rect_filled(header, 0.0, egui::Color32::WHITE);
            // 列布局：Name 吸收剩余宽度；Size 78 / Modified 150（参考列宽）。
            let mut columns: Vec<(Column, f32, f32)> = Vec::new(); // (列, 左缘, 宽)
            let optional: f32 = self.columns.iter().map(|c| chrome_column_width(*c)).sum();
            let name_w = (tree.width() - optional).max(80.0);
            let mut x = tree.left();
            columns.push((Column::Name, x, name_w));
            x += name_w;
            for column in &self.columns {
                let w = chrome_column_width(*column);
                columns.push((*column, x, w));
                x += w;
            }
            for (index, (column, cx, cw)) in columns.iter().enumerate() {
                let cell = egui::Rect::from_min_size(
                    egui::pos2(*cx, header.top()),
                    egui::vec2(*cw, CHROME_HEADER),
                );
                let label = match column {
                    Column::Name => "Name",
                    Column::Modified => "Modified",
                    other => other.label(),
                };
                painter.text(
                    egui::pos2(cell.left() + 8.0, cell.center().y),
                    egui::Align2::LEFT_CENTER,
                    label,
                    egui::FontId::proportional(crate::chrome::theme::UI_SIZE),
                    egui::Color32::BLACK,
                );
                // 排序箭头（小三角，居中于列）。
                if self.sort.0 == *column {
                    let c = egui::pos2(cell.center().x, header.top() + 7.0);
                    let up = !self.sort.2;
                    let (a, b) = if up { (7.0, -3.0) } else { (-7.0, 3.0) };
                    painter.line_segment(
                        [
                            c + egui::vec2(-3.5, a * 0.29 + 2.0),
                            c + egui::vec2(0.0, b * 0.29 + 2.0),
                        ],
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(90, 90, 90)),
                    );
                    painter.line_segment(
                        [
                            c + egui::vec2(0.0, b * 0.29 + 2.0),
                            c + egui::vec2(3.5, a * 0.29 + 2.0),
                        ],
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(90, 90, 90)),
                    );
                }
                let response = ui.interact(
                    cell,
                    egui::Id::unique(("folder-header", self.window_id, side, index)),
                    egui::Sense::click(),
                );
                if response.clicked() {
                    sort = Some((*column, side));
                }
                // 列分隔线（参考：Name|Size 之间、Size 尾前一像素）。
                if index > 0 {
                    painter.rect_filled(
                        egui::Rect::from_min_size(
                            egui::pos2(cell.left(), header.top() + 2.0),
                            egui::vec2(1.0, CHROME_HEADER - 2.0),
                        ),
                        0.0,
                        HEADER_SEP,
                    );
                }
            }
            // 外缘：块右边 #A0A0A0；右树左缘同（起于顶两条高光之下）。
            // 同步视图的内侧边线由箭头列自己绘制。
            if !(self.sync_mode && side == 0) {
                painter.rect_filled(
                    egui::Rect::from_min_size(
                        egui::pos2(header.right() - 1.0, header.top() + 2.0),
                        egui::vec2(1.0, CHROME_HEADER - 2.0),
                    ),
                    0.0,
                    HEADER_EDGE,
                );
            }
            if side == 1 && !self.sync_mode {
                painter.rect_filled(
                    egui::Rect::from_min_size(
                        egui::pos2(header.left(), header.top() + 2.0),
                        egui::vec2(1.0, CHROME_HEADER - 2.0),
                    ),
                    0.0,
                    HEADER_EDGE,
                );
            }
        }
        if self.sync_mode {
            // Arrow column header: three empty cells (#E5E5E5 | #A0A0A0 lines).
            for (i, dx) in SYNC_ARROW_LINES.iter().enumerate() {
                let x = left.right() + dx;
                if i > 0 {
                    painter.rect_filled(
                        egui::Rect::from_min_size(
                            egui::pos2(x - 1.0, rect.top() + 2.0),
                            egui::vec2(1.0, CHROME_HEADER - 2.0),
                        ),
                        0.0,
                        HEADER_SEP,
                    );
                }
                painter.rect_filled(
                    egui::Rect::from_min_size(
                        egui::pos2(x, rect.top() + 2.0),
                        egui::vec2(1.0, CHROME_HEADER - 2.0),
                    ),
                    0.0,
                    HEADER_EDGE,
                );
            }
        }
        if let Some((column, side)) = sort {
            let reverse = self.sort.0 == column && self.sort.1 == side && !self.sort.2;
            self.sort = (column, side, reverse);
            self.send(Command::Sort(column, side, reverse));
        }
        // 列头顶部两条高光线全宽贯通（参考 y141 #ECECEC / y142 #FEFEFE，覆盖中缝与滚动条槽）。
        painter.rect_filled(
            egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), 1.0)),
            0.0,
            crate::chrome::theme::LINE,
        );
        painter.rect_filled(
            egui::Rect::from_min_size(
                rect.min + egui::vec2(0.0, 1.0),
                egui::vec2(rect.width(), 1.0),
            ),
            0.0,
            crate::chrome::theme::SURFACE_CHROME,
        );
        // 行区。
        let rows_area = egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.top() + CHROME_HEADER),
            egui::pos2(right.right(), rect.bottom()),
        );
        let rows_h = rows_area.height();
        let total = self.snapshot.rows.len();
        if let Some(row) = self.pending_scroll.take() {
            self.scroll_y = row.min(total.saturating_sub(1)) as f32 * CHROME_ROW;
        }
        let max_scroll = (total as f32 * CHROME_ROW - rows_h).max(0.0);
        self.scroll_y = self.scroll_y.clamp(0.0, max_scroll);
        let first = (self.scroll_y / CHROME_ROW).floor().max(0.0) as usize;
        let end = (first + (rows_h / CHROME_ROW).ceil() as usize + 1).min(total);
        // 树线缓存按快照重建（O(n) 一次；滚动帧不重算）。
        let key = Arc::as_ptr(&self.snapshot) as usize;
        if self.grid_lines.as_ref().map(|(k, _)| *k) != Some(key) {
            self.grid_lines = Some((key, Arc::new(row_lines(&self.snapshot.rows))));
        }
        let lines = self
            .grid_lines
            .as_ref()
            .map(|(_, l)| l.clone())
            .unwrap_or_default();
        // 行区白底（含中缝），滚动条槽保持面板灰。
        painter.rect_filled(rows_area, 0.0, egui::Color32::WHITE);
        painter.rect_filled(
            egui::Rect::from_min_max(scroll.min, egui::pos2(scroll.right(), rect.bottom())),
            0.0,
            crate::chrome::theme::PANEL_BG,
        );
        let font = egui::FontId::proportional(crate::chrome::theme::UI_SIZE);
        // 树线用整列行区裁剪（竖段跨行）；行内容用行带裁剪。
        let tree_clip = [
            painter.with_clip_rect(egui::Rect::from_min_max(
                egui::pos2(left.left(), rows_area.top()),
                egui::pos2(left.right(), rows_area.bottom()),
            )),
            painter.with_clip_rect(egui::Rect::from_min_max(
                egui::pos2(right.left(), rows_area.top()),
                egui::pos2(right.right(), rows_area.bottom()),
            )),
        ];
        let mut drawn = 0;
        // Vertical guide runs already painted this frame per side, as
        // (x, painted up to y): sibling rows share their parent's run, so
        // each row only adds the part below what an earlier row drew.
        let mut guides: [Vec<(f32, f32)>; 2] = Default::default();
        for index in first..end {
            let row = &self.snapshot.rows[index];
            let y = rows_area.top() + index as f32 * CHROME_ROW - self.scroll_y;
            let band = egui::Rect::from_min_max(
                egui::pos2(left.left(), y),
                egui::pos2(right.right(), y + CHROME_ROW),
            );
            if index % 2 == 1 {
                painter.rect_filled(band, 0.0, TREE_STRIPE);
            }
            let selected = row.sides.iter().enumerate().any(|(s, item)| {
                item.as_ref()
                    .is_some_and(|item| self.selection.items.contains(&(item.node, s)))
            });
            if selected {
                painter.rect_filled(band, 0.0, TREE_SELECTED);
            }
            let flags = lines.get(index).copied().unwrap_or_default();
            for (side, tree_clip) in tree_clip.iter().enumerate() {
                let tree = if side == 0 { left } else { right };
                // 树线：只画“父节点 → 直接子行”的肘形，且该侧有条目才画。
                let depth = row.depth;
                if depth > 0 && row.sides[side].is_some() && flags.parent_up > 0 {
                    let x = tree.left() + 15.0 + 21.0 * (depth - 1) as f32;
                    let mid = y + CHROME_ROW / 2.0 - 0.5;
                    // 竖段从父行图标下缘到本行行中；横段到图标左缘。
                    let top = y - flags.parent_up as f32 * CHROME_ROW + 17.0;
                    // Same x ⇒ same dot phase (row tops differ by whole rows).
                    let start = match guides[side].iter_mut().find(|(gx, _)| *gx == x) {
                        Some((_, until)) => {
                            let start = if *until > top {
                                dot_start(top, *until + 1.0)
                            } else {
                                top
                            };
                            *until = until.max(mid);
                            start
                        }
                        None => {
                            guides[side].push((x, mid));
                            top
                        }
                    };
                    dotted_v(tree_clip, x, start, mid, TREE_LINE);
                    let icon_left = tree.left()
                        + 6.0
                        + 19.0 * depth as f32
                        + if row.directory { 0.0 } else { 2.0 };
                    dotted_h(tree_clip, x, icon_left - 1.0, mid, TREE_LINE);
                }
                let clip = painter.with_clip_rect(egui::Rect::from_min_max(
                    egui::pos2(tree.left(), y),
                    egui::pos2(tree.right(), y + CHROME_ROW),
                ));
                let Some(item) = &row.sides[side] else {
                    continue;
                };
                // 参考着色：文件夹名恒黑、其 Size/Modified 恒灰 #969696（颜色由图标承载）；
                // 文件名与单元格用语义色（选中也不反色）。
                let semantic = legend::side_color(self.palette, row.status, side, row.teal[side]);
                let (name_color, cell_color) = if row.directory {
                    (egui::Color32::BLACK, egui::Color32::from_rgb(150, 150, 150))
                } else {
                    (semantic, semantic)
                };
                let indent = 19.0 * depth as f32;
                let mid = y + CHROME_ROW / 2.0;
                if row.directory {
                    paint_folder_icon(
                        &clip,
                        egui::Rect::from_min_size(
                            egui::pos2(tree.left() + 6.0 + indent, y + 4.0),
                            egui::vec2(15.0, 13.0),
                        ),
                        row,
                    );
                    clip.text(
                        egui::pos2(tree.left() + 26.0 + indent, mid),
                        egui::Align2::LEFT_CENTER,
                        &item.name,
                        font.clone(),
                        name_color,
                    );
                } else {
                    paint_file_icon(&clip, tree.left() + 8.0 + indent, y, semantic);
                    clip.text(
                        egui::pos2(tree.left() + 18.0 + indent, mid),
                        egui::Align2::LEFT_CENTER,
                        &item.name,
                        font.clone(),
                        name_color,
                    );
                }
                // 列。
                let optional: f32 = self.columns.iter().map(|c| chrome_column_width(*c)).sum();
                let name_w = (tree.width() - optional).max(80.0);
                let mut cx = tree.left() + name_w;
                for column in &self.columns {
                    let w = chrome_column_width(*column);
                    let cell =
                        egui::Rect::from_min_size(egui::pos2(cx, y), egui::vec2(w, CHROME_ROW));
                    let at = Column::OPTIONAL.iter().position(|c| c == column).unwrap();
                    let text = &item.cells[at];
                    if !text.is_empty() {
                        if *column == Column::Size {
                            clip.text(
                                egui::pos2(cell.right() - 5.0, mid),
                                egui::Align2::RIGHT_CENTER,
                                text,
                                font.clone(),
                                cell_color,
                            );
                        } else {
                            clip.text(
                                egui::pos2(cell.left() + 4.0, mid),
                                egui::Align2::LEFT_CENTER,
                                text,
                                font.clone(),
                                cell_color,
                            );
                        }
                    }
                    cx += w;
                }
            }
            if self.sync_mode && row.sync_mark != 0 {
                paint_sync_mark(&painter, left.right(), y, row.sync_mark);
            }
            // 焦点虚框（跨双树与中缝）。
            if self.focus == Some(index) && selected {
                let r = egui::Rect::from_min_max(
                    egui::pos2(rect.left() + 2.0, y),
                    egui::pos2(right.right() - 1.0, y + CHROME_ROW),
                );
                let ink = egui::Color32::BLACK;
                dotted_h(&painter, r.left(), r.right(), r.top(), ink);
                dotted_h(&painter, r.left(), r.right(), r.bottom() - 1.0, ink);
                dotted_v(&painter, r.left(), r.top(), r.bottom(), ink);
                dotted_v(&painter, r.right(), r.top(), r.bottom(), ink);
            }
            drawn += 1;
        }
        self.last_stats = DirFrameStats {
            rows_drawn: drawn,
            top_row: first,
        };
        // 树区边线：左缘 #ECECEC/#FEFEFE 贯通整列；两树接缝 #C0C0C0 只到内容末行（参考如此）。
        let content_bottom = (rows_area.top() + total as f32 * CHROME_ROW).min(rect.bottom());
        painter.rect_filled(
            egui::Rect::from_min_size(rect.min, egui::vec2(1.0, rect.height())),
            0.0,
            crate::chrome::theme::LINE,
        );
        painter.rect_filled(
            egui::Rect::from_min_size(
                rect.min + egui::vec2(1.0, 0.0),
                egui::vec2(1.0, rect.height()),
            ),
            0.0,
            crate::chrome::theme::SURFACE_CHROME,
        );
        let seams: Vec<f32> = if self.sync_mode {
            SYNC_ARROW_LINES
                .iter()
                .map(|dx| left.right() + dx)
                .collect()
        } else {
            vec![left.right() - 1.0, right.left()]
        };
        for x in seams {
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(x, rows_area.top()),
                    egui::vec2(1.0, content_bottom - rows_area.top()),
                ),
                0.0,
                TREE_EDGE,
            );
        }
        // 交互：点击/双击/右键/滚轮。
        let response = ui.interact(
            rows_area,
            egui::Id::unique(("folder-grid", self.window_id)),
            egui::Sense::click(),
        );
        let mut click = None;
        if response.clicked() || response.double_clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let index =
                    ((pos.y - rows_area.top() + self.scroll_y) / CHROME_ROW).floor() as usize;
                if index < total {
                    let side = if pos.x < left.right() {
                        Some(0)
                    } else if pos.x >= right.left() {
                        Some(1)
                    } else {
                        None
                    };
                    click = Some((
                        index,
                        side,
                        response.double_clicked(),
                        ui.input(|i| i.modifiers),
                    ));
                }
            }
        }
        let mut context_command = None;
        // The row under the pointer when the menu opened.
        if response.secondary_clicked() {
            let pos = response.interact_pointer_pos();
            self.context_row = pos.and_then(|pos| {
                let index =
                    ((pos.y - rows_area.top() + self.scroll_y) / CHROME_ROW).floor() as usize;
                (index < total).then_some(index)
            });
            self.context_side = pos.and_then(|pos| {
                if pos.x < left.right() {
                    Some(0)
                } else if pos.x >= right.left() {
                    Some(1)
                } else {
                    None
                }
            });
            // Folder compare: right click outside the selection selects the row (the
            // clicked side only) first, like a file manager; inside it keeps it.
            if let (Some(index), false) = (self.context_row, self.sync_mode) {
                let row = &self.snapshot.rows[index];
                let selected = row.sides.iter().enumerate().any(|(s, item)| {
                    item.as_ref()
                        .is_some_and(|i| self.selection.items.contains(&(i.node, s)))
                });
                if !selected {
                    let side = self
                        .context_side
                        .filter(|&s| self.snapshot.rows[index].sides[s].is_some());
                    self.focus_row(index, side);
                }
            }
        }
        if let Some(index) = self.context_row.filter(|i| *i < total) {
            let directory = self.snapshot.rows[index].directory;
            if self.sync_mode {
                if let Some(kind) =
                    crate::chrome::popup::context_menu(&response, || row_menu(directory))
                        .and_then(|a| row_command(&a))
                {
                    context_command = Some((index, kind));
                }
            } else if let Some(action) = crate::chrome::popup::context_menu(&response, || {
                self.folder_context_items(directory)
            }) {
                self.context_action = Some(action);
            }
        }
        if let Some((index, side, double, modifiers)) = click {
            self.clicked(index, side, double, modifiers);
        }
        if let Some((index, kind)) = context_command {
            self.focus_row(index, None);
            if kind == 1 {
                self.open_pair();
            } else if kind == 2 {
                let row = &self.snapshot.rows[index];
                if let Some(node) = row.node {
                    self.send(Command::Expand {
                        nodes: vec![node],
                        expanded: !row.expanded,
                        recursive: false,
                    });
                }
            }
        }
        if response.hovered() {
            // Raw wheel, 1:1 this frame (egui's smoothed delta glides on).
            let delta = crate::chrome::wheel::delta(ui.ctx(), &[rows_area]).y;
            if delta != 0.0 {
                self.scroll_y = (self.scroll_y - delta).clamp(0.0, max_scroll);
            }
        }
        // 平坦滚动条（18 px）：内容放得下时只画轨道与顶端小灰块（参考如此）。
        let track = egui::Rect::from_min_max(
            egui::pos2(scroll.left(), rows_area.top()),
            egui::pos2(scroll.right(), rect.bottom()),
        );
        if total as f32 * CHROME_ROW > rows_h {
            let thumb_h = (rows_h * rows_h / (total as f32 * CHROME_ROW)).max(20.0);
            let travel = (rows_h - thumb_h).max(1.0);
            let frac = if max_scroll > 0.0 {
                self.scroll_y / max_scroll
            } else {
                0.0
            };
            let thumb = egui::Rect::from_min_size(
                egui::pos2(track.left() + 1.0, track.top() + travel * frac),
                egui::vec2(track.width() - 2.0, thumb_h),
            );
            painter.rect_filled(
                thumb.shrink2(egui::vec2(2.0, 1.0)),
                crate::chrome::theme::RADIUS_SM,
                crate::chrome::theme::SCROLL_THUMB,
            );
            let bar_response = ui.interact(
                track,
                egui::Id::unique(("folder-scrollbar", self.window_id)),
                egui::Sense::click_and_drag(),
            );
            if bar_response.dragged() || bar_response.clicked() {
                if let Some(pos) = bar_response.interact_pointer_pos() {
                    let f = ((pos.y - track.top() - thumb_h / 2.0) / travel).clamp(0.0, 1.0);
                    self.scroll_y = f * max_scroll;
                }
            }
        } else {
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(track.left() + 7.0, track.top() + 17.0),
                    egui::vec2(2.0, 17.0),
                ),
                0.0,
                crate::chrome::theme::TEXT_FAINT,
            );
        }
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(scroll.right() - 2.0, rect.top()),
                egui::vec2(1.0, rect.height()),
            ),
            0.0,
            crate::chrome::theme::SURFACE_CHROME,
        );
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(scroll.right() - 1.0, rect.top()),
                egui::vec2(1.0, rect.height()),
            ),
            0.0,
            crate::chrome::theme::LINE,
        );
        // 中缝两侧边缘都可左右拖动，移动左右树的分界（中缝宽度不变）。
        let span = egui::Rangef::new(rect.left(), rect.left() + left_w + right_w);
        for (edge, x) in [left.right(), right.left()].into_iter().enumerate() {
            crate::chrome::splitter::vertical_line(
                ui,
                &FOLDER_SPLIT_X,
                ("folder-tree", self.window_id, edge),
                x,
                rect.y_range(),
                span,
            );
        }
    }
    /// 日志区（106 px）：左侧齿轮/保存图标，#E3E7ED 文本框，右侧面板灰条。
    pub(crate) fn draw_log(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let painter = ui.painter().clone();
        painter.rect_filled(rect, 0.0, crate::chrome::theme::PANEL_BG);
        painter.hline(
            rect.left()..=rect.right(),
            rect.top(),
            egui::Stroke::new(1.0, crate::chrome::theme::SURFACE_CHROME),
        );
        painter.hline(
            rect.left()..=rect.right(),
            rect.top() + 1.0,
            egui::Stroke::new(1.0, crate::chrome::theme::LINE),
        );
        // 左侧小图标（原创）：齿轮 = 收起日志；文档 = 保存日志。
        let gear =
            egui::Rect::from_min_size(rect.min + egui::vec2(6.0, 12.0), egui::vec2(16.0, 16.0));
        let doc =
            egui::Rect::from_min_size(rect.min + egui::vec2(6.0, 38.0), egui::vec2(16.0, 16.0));
        let gray = egui::Color32::from_rgb(79, 79, 79);
        let c = gear.center();
        for i in 0..6 {
            let v = egui::Vec2::angled(i as f32 * std::f32::consts::PI / 3.0);
            painter.line_segment([c + v * 3.2, c + v * 5.4], egui::Stroke::new(1.4, gray));
        }
        painter.circle_stroke(c, 3.2, egui::Stroke::new(1.4, gray));
        let d = doc.center();
        painter.rect_stroke(
            egui::Rect::from_center_size(d, egui::vec2(10.0, 12.0)),
            0.0,
            egui::Stroke::new(1.2, gray),
            egui::StrokeKind::Inside,
        );
        for i in 0..3 {
            let y = d.y - 3.0 + i as f32 * 3.0;
            painter.line_segment(
                [egui::pos2(d.x - 3.0, y), egui::pos2(d.x + 3.0, y)],
                egui::Stroke::new(1.0, gray),
            );
        }
        let gear_r = ui.interact(
            gear,
            egui::Id::unique(("folder-log-gear", self.window_id)),
            egui::Sense::click(),
        );
        if gear_r.clicked() {
            self.log_panel.open = false;
        }
        gear_r.on_hover_text("Close log panel");
        let doc_r = ui.interact(
            doc,
            egui::Id::unique(("folder-log-save", self.window_id)),
            egui::Sense::click(),
        );
        if doc_r.clicked() {
            self.log_save_open = !self.log_save_open;
        }
        doc_r.on_hover_text("Save log");
        // 文本框。
        let text_box = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 29.0, rect.top() + 6.0),
            egui::pos2(rect.right() - 19.0, rect.bottom() - 3.0),
        );
        painter.rect_filled(text_box, 0.0, LOG_FILL);
        painter.rect_stroke(
            text_box,
            0.0,
            egui::Stroke::new(1.0, crate::chrome::theme::LINE),
            egui::StrokeKind::Inside,
        );
        // 底部边框行（参考 y934 #FEFEFE / y935 #838383）。
        painter.hline(
            rect.left()..=rect.right(),
            rect.bottom() - 2.0,
            egui::Stroke::new(1.0, crate::chrome::theme::SURFACE_CHROME),
        );
        painter.hline(
            rect.left()..=rect.right(),
            rect.bottom() - 1.0,
            egui::Stroke::new(1.0, crate::chrome::theme::LINE),
        );
        let mut lines = self.chrome_log.clone();
        for entry in self.shared.log.snapshot() {
            lines.push(format!(
                "{:?} {:?}: {} -> {} {}",
                entry.state,
                entry.action,
                entry
                    .source
                    .as_ref()
                    .map_or(String::new(), ToString::to_string),
                entry
                    .destination
                    .as_ref()
                    .map_or(String::new(), ToString::to_string),
                entry.message
            ));
        }
        let clip = painter.with_clip_rect(text_box.shrink(1.0));
        let pitch = 21.0;
        let capacity = ((text_box.height() - 8.0) / pitch).max(1.0) as usize;
        let start = lines.len().saturating_sub(capacity);
        for (i, line) in lines[start..].iter().enumerate() {
            let ly = text_box.top() + 7.0 + i as f32 * pitch;
            clip.text(
                egui::pos2(text_box.left() + 6.0, ly),
                egui::Align2::LEFT_TOP,
                line,
                egui::FontId::proportional(crate::chrome::theme::UI_SIZE),
                egui::Color32::BLACK,
            );
            // 首行（会话信息）下方的细分隔线。
            if i == 0 && lines.len() > 1 {
                clip.hline(
                    text_box.left()..=text_box.right(),
                    ly + 15.0,
                    egui::Stroke::new(1.0, crate::chrome::theme::LINE),
                );
            }
        }
        if self.log_save_open {
            let mut save = None;
            use crate::chrome::{controls::Role, widgets};
            crate::chrome::window::Window::new("Save log")
                .open(&mut self.log_save_open)
                .show(ui.ctx(), |ui| {
                    ui.label("Log file path");
                    widgets::text_field_width(ui, &mut self.log_panel.save_path, 360.0);
                    if widgets::dialog_buttons(ui, &[("Save log", Role::Primary)]) == Some(0)
                        && !self.log_panel.save_path.is_empty()
                    {
                        save = Some(self.log_panel.save_path.clone());
                    }
                });
            if let Some(path) = save {
                self.send(Command::SaveLog(path.into()));
            }
        }
    }
    /// 状态栏（25 px）：空闲显示 “Shows ...” 图例；纯文件选择时显示每侧文件数/字节与剩余空间。
    pub(crate) fn draw_status(&mut self, ui: &mut egui::Ui, rect: egui::Rect, busy: bool) {
        use crate::chrome::statusbar::{self, StatusField};
        let fields: Vec<StatusField>;
        if busy {
            let p = self.shared.progress.lock().unwrap().clone();
            let text = if p.message.is_empty() {
                self.message.clone()
            } else if let Some(total) = p.total.filter(|n| *n > 0) {
                format!("{} ({}/{})", p.message, p.current, total)
            } else {
                p.message
            };
            fields = vec![StatusField::flexible(text)];
        } else {
            // Walking every row against the selection costs ~10 ms per frame
            // at 100k rows with everything selected; recompute only when the
            // snapshot or the selection changes.
            let key = Arc::as_ptr(&self.snapshot) as usize;
            let print = selection_fingerprint(&self.selection);
            let totals = match self.selection_totals {
                Some((k, p, totals)) if k == key && p == print => totals,
                _ => {
                    let totals = selection_totals(&self.snapshot, &self.selection);
                    self.selection_totals = Some((key, print, totals));
                    totals
                }
            };
            let SelectionTotals {
                counts,
                any,
                only_files,
            } = totals;
            if self.sync_mode {
                // Sync: per-side file totals (selection when files are
                // selected, otherwise every listed file) and free space.
                let counts = if any && only_files {
                    counts
                } else {
                    let mut all = [(0usize, 0u64); 2];
                    for row in self.snapshot.rows.iter().filter(|r| !r.directory) {
                        for (side, item) in row.sides.iter().enumerate() {
                            if let Some(item) = item {
                                all[side].0 += 1;
                                all[side].1 += item.entry.size.unwrap_or(0);
                            }
                        }
                    }
                    all
                };
                let free = |side: usize| self.snapshot.free[side].clone().unwrap_or_default();
                let selected =
                    |side: usize| format!("{} file(s), {} bytes", counts[side].0, counts[side].1);
                let (left_w, _) = Self::chrome_split(rect.width(), SYNC_GAP);
                let half = (left_w / 2.0).floor();
                fields = vec![
                    StatusField::fixed(selected(0), half),
                    StatusField::fixed(free(0), left_w - half),
                    StatusField::fixed("", SYNC_GAP),
                    StatusField::fixed(selected(1), half + 1.0),
                    StatusField::flexible(free(1)),
                ];
            } else if any && only_files {
                let free = |side: usize| {
                    self.snapshot.free[side]
                        .clone()
                        .unwrap_or_else(|| "—".into())
                };
                let selected =
                    |side: usize| format!("{} file(s), {} bytes", counts[side].0, counts[side].1);
                fields = vec![
                    StatusField::fixed(selected(0), 265.0),
                    StatusField::fixed("", 131.0),
                    StatusField::fixed(free(0), 268.0),
                    StatusField::fixed("", CHROME_GAP),
                    StatusField::fixed(selected(1), 265.0),
                    StatusField::fixed("", 131.0),
                    StatusField::flexible(free(1)),
                ];
            } else {
                fields = vec![StatusField::flexible(shows_legend(
                    self.filters.options.display,
                ))];
            }
        }
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            statusbar::show(ui, &fields);
        });
        // 右下角缩放把手（参考：三条斜向 2x2 灰点，#BFBFBF）。
        let painter = ui.painter().clone();
        let dot = egui::Color32::from_rgb(191, 191, 191);
        for (dx, dy) in [(4, 15), (7, 18), (4, 18), (10, 21), (7, 21), (4, 21)] {
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(rect.right() - dx as f32 - 2.0 + 2.0, rect.top() + dy as f32),
                    egui::vec2(2.0, 2.0),
                ),
                0.0,
                dot,
            );
        }
    }
    fn drop_impl(&mut self) {
        self.shared.closed.store(true, AtomicOrdering::Relaxed);
        self.cancel();
        let _ = self.commands.send(Work {
            id: u64::MAX,
            command: Command::Shutdown,
            cancel: self.shared.token(),
        });
        if let Some(question) = self.question.take() {
            let _ = question.reply.send(Confirmation::No);
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl Drop for DirCompareView {
    fn drop(&mut self) {
        self.drop_impl();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bc_core::Side;
    use std::sync::atomic::AtomicUsize;
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "matcha-folder-gui-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, AtomicOrdering::Relaxed)
            ));
            for side in ["left", "right"] {
                std::fs::create_dir_all(root.join(side).join("nested")).unwrap();
            }
            std::fs::write(root.join("left/a.txt"), b"123456789").unwrap();
            std::fs::write(root.join("right/a.txt"), b"ABCDEFGH").unwrap();
            for side in ["left", "right"] {
                std::fs::write(root.join(side).join("b.txt"), b"same\n").unwrap();
                std::fs::write(root.join(side).join("nested/x.txt"), b"nested\n").unwrap();
            }
            Self(root)
        }
        fn view(&self, ctx: &egui::Context, sync: bool) -> DirCompareView {
            DirCompareView::open(
                ctx,
                &self.0.join("left"),
                &self.0.join("right"),
                sync,
                false,
            )
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn wait(view: &mut DirCompareView, ctx: &egui::Context) {
        let started = Instant::now();
        loop {
            view.poll(ctx);
            if !view.busy() {
                assert!(view.snapshot.complete, "{}", view.message);
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "{}",
                view.message
            );
            thread::sleep(Duration::from_millis(5));
        }
    }
    fn frame(view: &mut DirCompareView, ctx: &egui::Context) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            ..Default::default()
        };
        ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                view.ui(ui);
            });
        })
        .drop_without_applying_deltas();
    }
    #[derive(Debug)]
    struct Dropped(PathBuf);
    impl egui::DroppedFile for Dropped {
        fn path(&self) -> &Path {
            &self.0
        }
        fn bytes(&self) -> Result<Vec<u8>, String> {
            Err("dropped folders are never read".into())
        }
    }
    fn drop_frame(view: &mut DirCompareView, ctx: &egui::Context, pos: egui::Pos2, path: &Path) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            events: vec![egui::Event::PointerMoved(pos)],
            dropped_files: vec![Arc::new(Dropped(path.to_path_buf()))],
            ..Default::default()
        };
        ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                view.ui(ui);
            });
        })
        .drop_without_applying_deltas();
    }
    #[test]
    fn os_drop_replaces_the_half_under_the_pointer_in_compare_and_sync() {
        for sync in [false, true] {
            let temp = Temp::new();
            let other = temp.0.join("other");
            std::fs::create_dir_all(&other).unwrap();
            let ctx = egui::Context::default();
            let mut view = temp.view(&ctx, sync);
            wait(&mut view, &ctx);
            frame(&mut view, &ctx);
            let left = temp.0.join("left").display().to_string();
            let right = temp.0.join("right").display().to_string();
            let other_s = other.display().to_string();
            // Right half (path box / tree area) replaces the right folder.
            drop_frame(&mut view, &ctx, egui::pos2(1100.0, 400.0), &other);
            wait(&mut view, &ctx);
            assert_eq!(view.path_texts(), [left.clone(), other_s.clone()]);
            // Left half replaces the left folder.
            drop_frame(
                &mut view,
                &ctx,
                egui::pos2(200.0, 400.0),
                &temp.0.join("right"),
            );
            wait(&mut view, &ctx);
            assert_eq!(view.path_texts(), [right.clone(), other_s.clone()]);
            // A file is not a base folder: paths stay, a message explains.
            drop_frame(
                &mut view,
                &ctx,
                egui::pos2(1100.0, 400.0),
                &temp.0.join("left/a.txt"),
            );
            assert_eq!(view.path_texts(), [right.clone(), other_s.clone()]);
            assert!(
                view.message.starts_with("Drop a folder"),
                "{}",
                view.message
            );
            // A drop over the floating text window belongs to that window.
            view.text = Some(Box::new(TextCompareView::from_texts("a", "b").unwrap()));
            view.floating[0] = Some(egui::Rect::from_min_size(
                egui::pos2(900.0, 300.0),
                egui::vec2(300.0, 300.0),
            ));
            let left_dir = temp.0.join("left");
            let consumed = std::cell::Cell::new(false);
            ctx.run_ui(
                egui::RawInput {
                    events: vec![egui::Event::PointerMoved(egui::pos2(1000.0, 400.0))],
                    dropped_files: vec![Arc::new(Dropped(left_dir))],
                    ..Default::default()
                },
                |ui| {
                    let full =
                        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1400.0, 900.0));
                    view.handle_drop(ui.ctx(), full);
                    consumed.set(ui.input(|i| i.raw.dropped_files.is_empty()));
                },
            )
            .drop_without_applying_deltas();
            assert!(
                !consumed.get(),
                "drop over the text window was taken by the folder view"
            );
            assert_eq!(view.path_texts(), [right, other_s]);
        }
    }
    fn row(view: &DirCompareView, name: &str) -> usize {
        view.snapshot
            .rows
            .iter()
            .position(|r| r.sides.iter().flatten().any(|s| s.name == name))
            .unwrap()
    }

    #[test]
    fn viewport_only_materializes_visible_rows_at_a_hundred_thousand_rows() {
        let temp = Temp::new();
        let ctx = egui::Context::default();
        let mut view = temp.view(&ctx, false);
        wait(&mut view, &ctx);
        let source = view.snapshot.rows[row(&view, "a.txt")].clone();
        let rows = (0..100_000)
            .map(|index| {
                let mut row = source.clone();
                row.node = Some(index + 1);
                for side in row.sides.iter_mut().flatten() {
                    side.node = index + 1;
                    side.label = format!("file{index:06}.txt");
                }
                row
            })
            .collect();
        view.snapshot = Arc::new(Snapshot {
            rows,
            files: 100_000,
            complete: true,
            ..Snapshot::default()
        });
        frame(&mut view, &ctx);
        assert!(view.last_stats.rows_drawn > 0 && view.last_stats.rows_drawn < 80);
        view.scroll_to_row(90_000);
        frame(&mut view, &ctx);
        frame(&mut view, &ctx);
        assert!(view.last_stats.top_row >= 89_999);
        assert!(view.last_stats.rows_drawn < 80);
    }
    fn frame_with(view: &mut DirCompareView, ctx: &egui::Context, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            events,
            ..Default::default()
        };
        ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                view.ui(ui);
            });
        })
        .drop_without_applying_deltas();
    }
    #[test]
    fn row_context_menu_follows_the_reference_and_runs_menu_commands() {
        // Inventory: the captured reference menu (folder row), with our CRC/Binary/
        // Rules-based submenu standing in for the reference "Compare Contents..." dialog.
        let tree: serde_json::Value = serde_json::from_str(
            std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../docs/visual-parity/supplemental/folder-context.tree.json"),
            )
            .unwrap()
            .trim_start_matches('\u{feff}'),
        )
        .unwrap();
        let reference: Vec<String> = tree["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| {
                if i["separator"].as_bool() == Some(true) || i["type"] == "separator" {
                    "----".to_string()
                } else {
                    i["name"]
                        .as_str()
                        .unwrap()
                        .replace("Compare Contents...", "Compare Contents")
                }
            })
            .collect();
        let temp = Temp::new();
        let ctx = egui::Context::default();
        let mut view = temp.view(&ctx, false);
        wait(&mut view, &ctx);
        frame(&mut view, &ctx);
        let labels = |items: Vec<crate::chrome::menubar::MenuItem>| -> Vec<String> {
            items
                .into_iter()
                .map(|i| match i.kind {
                    crate::chrome::menubar::MenuItemKind::Separator => "----".to_string(),
                    _ => i.label,
                })
                .collect()
        };
        assert_eq!(labels(view.folder_context_items(true)), reference);
        // Files open the pair instead of a folder; unimplemented entries are disabled.
        let file_items = view.folder_context_items(false);
        assert_eq!(file_items[0].label, "Open");
        assert!(file_items
            .iter()
            .any(|i| i.label == "Synchronize" && !i.enabled));

        let press = |pos, button, pressed| egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: Default::default(),
        };
        let pos = egui::pos2(300.0, 200.0);
        view.selection.items.clear();
        frame_with(
            &mut view,
            &ctx,
            vec![
                egui::Event::PointerMoved(pos),
                press(pos, egui::PointerButton::Secondary, true),
                press(pos, egui::PointerButton::Secondary, false),
            ],
        );
        let index = view
            .context_row
            .expect("right-click picks the row under the pointer");
        // Right click outside the selection selects the clicked side of that row.
        assert_eq!(view.context_side, Some(0));
        let row = &view.snapshot.rows[index];
        let left = row.sides[0]
            .as_ref()
            .expect("left item under the pointer")
            .node;
        assert_eq!(view.selection.items, BTreeSet::from([(left, 0)]));
        // egui lays a new popup area out once before it takes input.
        frame(&mut view, &ctx);
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1400.0, 900.0),
                )),
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    view.ui(ui);
                });
            },
        );
        out.textures_delta.clear();
        let target = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == "Compare To..." => {
                    Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center())
                }
                _ => None,
            })
            .next_back()
            .expect("menu shows Compare To...");
        assert!(out.shapes.iter().any(
            |s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "Copy to Right")
        ));
        frame_with(
            &mut view,
            &ctx,
            vec![
                egui::Event::PointerMoved(target),
                press(target, egui::PointerButton::Primary, true),
                press(target, egui::PointerButton::Primary, false),
            ],
        );
        assert!(view.compare_to, "the menu runs the menu-bar command");
        // The sync layout keeps its own short row menu.
        assert_eq!(row_menu(false).len(), 2);
        assert_eq!(row_menu(true).len(), 3);
        assert_eq!(row_command("row:expand"), Some(2));
    }
    #[test]
    fn session_settings_draft_applies_only_on_ok_and_only_what_changed() {
        let temp = Temp::new();
        let ctx = egui::Context::default();
        let mut view = temp.view(&ctx, false);
        wait(&mut view, &ctx);
        view.folder_command("Session Settings");
        assert!(view.settings.open);
        assert_eq!(view.settings.draft, Some(view.settings_draft()));
        // Cancel-like close discards the draft; the view is untouched.
        view.settings.draft.as_mut().unwrap().names = "*.txt".into();
        view.settings.close();
        assert_eq!(view.filters.names, view.settings_draft().names);
        assert_ne!(view.filters.names, "*.txt");
        // Apply a changed name mask and handling option: both land in the view;
        // the folders did not change, so nothing reloads.
        view.open_settings();
        let mut draft = view.settings.draft.clone().unwrap();
        draft.names = "*.txt".into();
        draft.handling.copy_attributes = !draft.handling.copy_attributes;
        let paths = view.paths();
        view.apply_settings(draft.clone());
        assert_eq!(view.filters.names, "*.txt");
        assert_eq!(
            view.handling.copy_attributes,
            draft.handling.copy_attributes
        );
        assert_eq!(view.paths(), paths);
        assert_eq!(view.settings_draft(), draft);
    }
    #[test]
    fn session_settings_dialog_validates_on_ok_and_reports_actions() {
        use dialogs::folder_settings::{Draft, FolderSettings, Outcome};
        let draft = Draft {
            paths: ["a".into(), "b".into()],
            names: String::new(),
            other: String::new(),
            content: String::new(),
            exclude_containing: false,
            criteria: "size crc".into(),
            pairing: Default::default(),
            scan: ScanOptions::default(),
            handling: Default::default(),
        };
        let mut dialog = FolderSettings::default();
        dialog.show(draft, crate::chrome::dialog::Placement::ClientOnly);
        let ctx = egui::Context::default();
        let run = |dialog: &mut FolderSettings, events: Vec<egui::Event>| {
            let mut out = Vec::new();
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(583.0, 563.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| out = dialog.ui(ui.ctx(), "Folder Compare"),
            )
            .drop_without_applying_deltas();
            out
        };
        let click = |pos: egui::Pos2| {
            [true, false].map(|pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            })
        };
        // A new Area takes input only after two layout frames.
        run(&mut dialog, vec![]);
        run(&mut dialog, vec![]);
        // OK (268..362 x 523..552): invalid criteria keep the dialog open with an error.
        let ok = egui::pos2(315.0, 537.0);
        let mut events = vec![egui::Event::PointerMoved(ok)];
        events.extend(click(ok));
        assert!(run(&mut dialog, events).is_empty());
        assert!(dialog.open);
        assert!(dialog
            .error
            .as_deref()
            .unwrap_or_default()
            .starts_with("Comparison"));
        // Fixed criteria: Apply hands the draft over and keeps the dialog open.
        dialog.draft.as_mut().unwrap().criteria = "size".into();
        let apply = egui::pos2(525.0, 537.0);
        let mut events = vec![egui::Event::PointerMoved(apply)];
        events.extend(click(apply));
        let out = run(&mut dialog, events);
        assert!(matches!(out.as_slice(), [Outcome::Apply(d)] if d.criteria == "size"));
        assert!(dialog.open && dialog.error.is_none());
        // Esc discards.
        run(
            &mut dialog,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert!(!dialog.open && dialog.draft.is_none());
        assert_eq!(dialogs::folder_settings::TABS.len(), 6);
    }
    #[test]
    fn wheel_scroll_tracks_input_immediately() {
        let temp = Temp::new();
        let ctx = egui::Context::default();
        let mut view = temp.view(&ctx, false);
        wait(&mut view, &ctx);
        let source = view.snapshot.rows[row(&view, "a.txt")].clone();
        let rows = (0..10_000)
            .map(|index| {
                let mut row = source.clone();
                row.node = Some(index + 1);
                for side in row.sides.iter_mut().flatten() {
                    side.node = index + 1;
                    side.label = format!("file{index:06}.txt");
                }
                row
            })
            .collect();
        view.snapshot = Arc::new(Snapshot {
            rows,
            files: 10_000,
            complete: true,
            ..Snapshot::default()
        });
        frame(&mut view, &ctx);
        let pos = egui::pos2(300.0, 400.0);
        frame_with(&mut view, &ctx, vec![egui::Event::PointerMoved(pos)]);
        assert_eq!(view.scroll_y, 0.0);
        let wheel = |lines: f32, modifiers| egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, lines),
            modifiers,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
        };
        let mut events = vec![egui::Event::PointerMoved(pos)];
        events.extend((0..10).map(|_| wheel(-3.0, egui::Modifiers::default())));
        frame_with(&mut view, &ctx, events);
        let mut trajectory = vec![view.scroll_y];
        for _ in 0..20 {
            frame_with(&mut view, &ctx, vec![egui::Event::PointerMoved(pos)]);
            trajectory.push(view.scroll_y);
        }
        eprintln!("folder wheel trajectory: {trajectory:?}");
        assert!(
            trajectory.iter().all(|&y| y == 1200.0),
            "full delta applies in the arrival frame without glide: {trajectory:?}"
        );
        // Ctrl+wheel never scrolls; wheel up past the top clamps at zero.
        frame_with(
            &mut view,
            &ctx,
            vec![
                egui::Event::PointerMoved(pos),
                wheel(-3.0, egui::Modifiers::COMMAND),
            ],
        );
        assert_eq!(view.scroll_y, 1200.0);
        frame_with(
            &mut view,
            &ctx,
            vec![
                egui::Event::PointerMoved(pos),
                wheel(50.0, egui::Modifiers::default()),
            ],
        );
        assert_eq!(view.scroll_y, 0.0);
    }
    #[test]
    fn clipped_dot_runs_keep_phase_and_cover_the_clip() {
        for start in [-40_017.5f32, -17.0, -3.5, 0.0, 0.5, 12.0] {
            for min in [-1000.0f32, -18.0, 0.0, 0.5, 1.0, 7.25, 300.0] {
                let first = dot_start(start, min);
                assert_eq!((first - start).rem_euclid(2.0), 0.0, "{start} {min}");
                assert!(first >= start);
                // No dot that can touch [min, ..) is skipped.
                assert!(first == start || first + 2.0 + 1.0 > min && first + 1.0 <= min);
            }
        }
    }
    #[test]
    fn status_totals_follow_selection_changes_of_equal_size() {
        let temp = Temp::new();
        let ctx = egui::Context::default();
        let mut view = temp.view(&ctx, false);
        wait(&mut view, &ctx);
        frame(&mut view, &ctx);
        let node = |view: &DirCompareView, name: &str| {
            view.snapshot.rows[row(view, name)].sides[0]
                .as_ref()
                .unwrap()
                .node
        };
        let a = node(&view, "a.txt");
        let b = node(&view, "b.txt");
        view.selection.items = [(a, 0)].into();
        frame(&mut view, &ctx);
        let totals = view.selection_totals.unwrap().2;
        assert_eq!(totals.counts[0], (1, 9));
        assert!(totals.any && totals.only_files);
        // Same size, different item: the cached totals must not be reused.
        view.selection.items = [(b, 0)].into();
        frame(&mut view, &ctx);
        assert_eq!(view.selection_totals.unwrap().2.counts[0], (1, 5));
        view.selection.items.clear();
        frame(&mut view, &ctx);
        assert!(!view.selection_totals.unwrap().2.any);
    }
    /// Interactive-latency probe over a real tree (not part of the default
    /// run): `cargo test --release -p bc-gui --lib folder_interaction_latency
    /// -- --ignored --nocapture` with `MATCHA_PERF_LEFT`/`MATCHA_PERF_RIGHT`
    /// (default: the 100k-file `dir_ui_fixture` under %TEMP%). Frames are real
    /// egui passes plus tessellation; worker round trips are timed to the
    /// first frame that shows the result.
    #[test]
    #[ignore]
    fn folder_interaction_latency() {
        let fixture = std::env::temp_dir().join("matcha-wave3-fixture");
        let path = |var: &str, side: &str| {
            std::env::var_os(var)
                .map(PathBuf::from)
                .unwrap_or_else(|| fixture.join(side))
        };
        let (left, right) = (
            path("MATCHA_PERF_LEFT", "left"),
            path("MATCHA_PERF_RIGHT", "right"),
        );
        let ctx = egui::Context::default();
        let timed = |view: &mut DirCompareView, events: Vec<egui::Event>| {
            let t = Instant::now();
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1400.0, 900.0),
                )),
                events,
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    view.ui(ui);
                });
            });
            out.textures_delta.clear();
            let _ = ctx.tessellate(out.shapes, out.pixels_per_point);
            t.elapsed().as_secs_f64() * 1e3
        };
        // Drive frames until the worker is idle; report the round trip and
        // the slowest frame seen meanwhile (a UI-thread stall shows there).
        let settle = |label: &str, view: &mut DirCompareView| {
            let t = Instant::now();
            let mut worst: f64 = 0.0;
            let mut frames = 0;
            loop {
                worst = worst.max(timed(view, vec![]));
                frames += 1;
                if !view.busy() && frames > 1 {
                    break;
                }
                thread::sleep(Duration::from_millis(1));
                assert!(t.elapsed() < Duration::from_secs(300), "{label}");
            }
            eprintln!(
                "[latency] folder {label:<22} round-trip={:>8.1}ms frames={frames:>4} worst-frame={worst:>7.2}ms rows={}",
                t.elapsed().as_secs_f64() * 1e3,
                view.snapshot.rows.len()
            );
        };
        let t = Instant::now();
        let mut view = DirCompareView::open(&ctx, &left, &right, false, false);
        eprintln!(
            "[latency] folder open() call         {:.1}ms",
            t.elapsed().as_secs_f64() * 1e3
        );
        settle("load", &mut view);
        let pos = egui::pos2(300.0, 400.0);
        let wheel = egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, -3.0),
            modifiers: Default::default(),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
        };
        for command in ["Expand All", "Show Differences", "Show Same", "Show All"] {
            view.folder_command(command);
            settle(command, &mut view);
            let mut ms: Vec<f64> = (0..60)
                .map(|_| {
                    timed(
                        &mut view,
                        vec![egui::Event::PointerMoved(pos), wheel.clone()],
                    )
                })
                .collect();
            ms.sort_by(f64::total_cmp);
            eprintln!(
                "[latency] folder   wheel x60 after {command:<16} p50={:.2}ms max={:.2}ms",
                ms[30], ms[59]
            );
        }
        view.scroll_to_row(view.snapshot.rows.len() / 2);
        eprintln!(
            "[latency] folder jump to middle frame {:.2}ms",
            timed(&mut view, vec![])
        );
        view.select_visible("all", false);
        eprintln!(
            "[latency] folder select all frame {:.2}ms",
            timed(&mut view, vec![])
        );
        let mut ms: Vec<f64> = (0..60)
            .map(|_| {
                timed(
                    &mut view,
                    vec![egui::Event::PointerMoved(pos), wheel.clone()],
                )
            })
            .collect();
        ms.sort_by(f64::total_cmp);
        eprintln!(
            "[latency] folder   wheel x60 with all selected p50={:.2}ms max={:.2}ms",
            ms[30], ms[59]
        );
        view.folder_command("Collapse All");
        settle("Collapse All", &mut view);
    }
    #[test]
    fn selection_find_sort_expand_filters_and_cross_row_open_use_snapshot_identities() {
        let temp = Temp::new();
        let ctx = egui::Context::default();
        let mut view = temp.view(&ctx, false);
        wait(&mut view, &ctx);
        frame(&mut view, &ctx);
        view.select_visible("all", false);
        let left_count = view.selection.items.iter().filter(|(_, s)| *s == 0).count();
        view.restrict_side(0);
        assert_eq!(view.selection.items.len(), left_count);
        assert!(left_count > 1);
        let a = row(&view, "a.txt");
        let b = row(&view, "b.txt");
        view.clicked(b, Some(0), false, egui::Modifiers::default());
        view.clicked(
            a,
            Some(1),
            false,
            egui::Modifiers {
                ctrl: true,
                ..Default::default()
            },
        );
        view.open_pair();
        wait(&mut view, &ctx);
        let text = view.text.as_ref().unwrap();
        let [left, right] = text.paths();
        assert!(left.unwrap().ends_with("left/b.txt"));
        assert!(right.unwrap().ends_with("right/a.txt"));
        // The formal view classifies the cross-row pair, so the difference is highlighted.
        while !view.text.as_ref().unwrap().is_ready() {
            frame(&mut view, &ctx);
            thread::sleep(Duration::from_millis(5));
        }
        let text = view.text.as_ref().unwrap();
        assert!(text.map.statistics().important > 0);
        let parents = text.parent_folders().unwrap();
        assert!(parents[0].ends_with("left") && parents[1].ends_with("right"));
        frame(&mut view, &ctx);
        assert!(!view.request_text_close(false));
        let text = view.text.as_mut().unwrap();
        text.content_mut(0).replace_line(0, b"edited").unwrap();
        // Unsaved text edits block Home/close until the text view's own prompt answers.
        assert!(view.request_text_close(false));
        frame(&mut view, &ctx);
        assert!(view.text.is_some() && !view.take_exit_request());
        view.text = None;
        view.find.text = "A.TXT".into();
        view.find.case_sensitive = false;
        view.find.wrap = false;
        view.focus = None;
        view.find_next(false);
        assert_eq!(view.focus, Some(a));
        view.find.regex = true;
        view.find.text = r"\.txt$".into();
        view.find.whole_word = false;
        view.find.wrap = true;
        view.find_next(true);
        assert!(view.find.error.is_none());
        let node = view.snapshot.rows[row(&view, "nested")].node.unwrap();
        view.send(Command::Expand {
            nodes: vec![node],
            expanded: true,
            recursive: false,
        });
        wait(&mut view, &ctx);
        assert!(view.snapshot.rows.iter().any(|r| r.depth == 1));
        view.send(Command::Sort(Column::Size, 0, true));
        wait(&mut view, &ctx);
        view.send(Command::Filters {
            names: String::new(),
            other: "size:>8B".into(),
            content: String::new(),
            exclude_containing: false,
            view: ViewOptions::default(),
        });
        wait(&mut view, &ctx);
        assert_eq!(view.snapshot.rows[row(&view, "a.txt")].teal, [true, false]);
        view.send(Command::Filters {
            names: "-*.txt".into(),
            other: String::new(),
            content: String::new(),
            exclude_containing: false,
            view: ViewOptions {
                suppress_filters: true,
                ..Default::default()
            },
        });
        wait(&mut view, &ctx);
        assert!(view
            .snapshot
            .rows
            .iter()
            .filter(|r| !r.directory)
            .all(|r| r.teal.iter().all(|t| *t)));
    }
    #[test]
    fn interaction_bridge_waits_for_ui_and_cancel_does_not_grant_overwrite() {
        let temp = Temp::new();
        let ctx = egui::Context::default();
        let mut view = temp.view(&ctx, false);
        wait(&mut view, &ctx);
        view.focus_row(row(&view, "a.txt"), Some(0));
        view.operate(
            DirOp::Copy {
                from: Side::Left,
                to: Side::Right,
            },
            false,
        );
        let started = Instant::now();
        while view.question.is_none() {
            view.poll(&ctx);
            assert!(started.elapsed() < Duration::from_secs(10));
            thread::sleep(Duration::from_millis(5));
        }
        frame(&mut view, &ctx);
        assert_eq!(
            std::fs::read(temp.0.join("right/a.txt")).unwrap(),
            b"ABCDEFGH"
        );
        let question = view.question.take().unwrap();
        assert!(question.text.contains("Overwrite"));
        question.reply.send(Confirmation::YesToAll).unwrap();
        wait(&mut view, &ctx);
        assert_eq!(
            std::fs::read(temp.0.join("right/a.txt")).unwrap(),
            b"123456789"
        );
        view.log_panel.open = true;
        frame(&mut view, &ctx);
        view.operate(
            DirOp::Delete {
                target: Target::Left,
                recycle_bin: false,
            },
            false,
        );
        let started = Instant::now();
        while view.question.is_none() {
            view.poll(&ctx);
            assert!(started.elapsed() < Duration::from_secs(10));
            thread::sleep(Duration::from_millis(5));
        }
        view.cancel();
        frame(&mut view, &ctx);
        wait(&mut view, &ctx);
        assert!(temp.0.join("left/a.txt").exists());
        assert!(view
            .shared
            .log
            .snapshot()
            .iter()
            .any(|e| e.state == bc_engine_dir::op_exec::OpState::Cancelled));
    }
    #[test]
    fn successful_sync_keeps_a_baseline_and_conflict_navigation_expands_ancestors() {
        use bc_vfs::Vfs;
        let temp = Temp::new();
        let ctx = egui::Context::default();
        let mut view = temp.view(&ctx, true);
        wait(&mut view, &ctx);
        view.send(Command::SyncMode(
            SyncMode::Mirror(bc_engine_dir::sync::SyncDirection::LeftToRight),
            false,
            false,
        ));
        wait(&mut view, &ctx);
        view.send(Command::SyncRun);
        let started = Instant::now();
        let mut confirmations = 0;
        while view.busy() {
            view.poll(&ctx);
            if let Some(question) = view.question.take() {
                confirmations += 1;
                question.reply.send(Confirmation::YesToAll).unwrap();
            }
            assert!(started.elapsed() < Duration::from_secs(20));
            thread::sleep(Duration::from_millis(5));
        }
        wait(&mut view, &ctx);
        assert!(confirmations > 0);
        for (side, seconds, bytes) in [
            ("left", 100u64, b"left changed".as_slice()),
            ("right", 200u64, b"right change".as_slice()),
        ] {
            let path = temp.0.join(side).join("nested/x.txt");
            std::fs::write(&path, bytes).unwrap();
            LocalFs::new()
                .set_modified(
                    &VfsPath::local(&path),
                    UNIX_EPOCH + Duration::from_secs(1_700_000_000 + seconds),
                )
                .unwrap();
        }
        view.send(Command::Refresh {
            selection: None,
            full: true,
        });
        wait(&mut view, &ctx);
        view.send(Command::SyncMode(
            SyncMode::Update(bc_engine_dir::sync::SyncDirection::Both),
            false,
            false,
        ));
        wait(&mut view, &ctx);
        assert_eq!(view.snapshot.sync_stats.unwrap().conflicts, 1);
        view.navigate(false, true);
        wait(&mut view, &ctx);
        let index = row(&view, "x.txt");
        assert_eq!(view.focus, Some(index));
        assert!(view.snapshot.rows[index].conflict);
        let node = view.snapshot.rows[index].node.unwrap();
        view.send(Command::SyncOverride(node, SyncChoice::LeaveAlone));
        wait(&mut view, &ctx);
        view.send(Command::ExpandAll(false));
        wait(&mut view, &ctx);
        view.send(Command::ExpandAll(true));
        wait(&mut view, &ctx);
        assert_eq!(view.snapshot.rows[row(&view, "x.txt")].center, "Leave");
    }
    #[cfg(windows)]
    #[test]
    fn following_a_junction_cycle_is_bounded_and_cannot_plan_destructive_work() {
        let temp = Temp::new();
        let link = temp.0.join("left").join("loop");
        let target = temp.0.join("left");
        assert!(std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(&target)
            .output()
            .unwrap()
            .status
            .success());
        let ctx = egui::Context::default();
        let mut view = temp.view(&ctx, false);
        wait(&mut view, &ctx);
        view.send(Command::Rules {
            criteria: Criteria {
                follow_symlinks: true,
                ..Default::default()
            },
            handling: Default::default(),
            pairing: Default::default(),
            scan: ScanOptions {
                scan_orphans: true,
                ..Default::default()
            },
        });
        wait(&mut view, &ctx);
        assert_eq!(view.snapshot.files, 3);
        let index = row(&view, "loop");
        assert!(view.snapshot.rows[index].sides[0]
            .as_ref()
            .unwrap()
            .entry
            .error
            .as_ref()
            .unwrap()
            .contains("cycle"));
        view.focus_row(index, Some(0));
        view.operate(
            DirOp::Copy {
                from: Side::Left,
                to: Side::Right,
            },
            false,
        );
        let started = Instant::now();
        while view.busy() {
            view.poll(&ctx);
            assert!(started.elapsed() < Duration::from_secs(10));
            thread::sleep(Duration::from_millis(5));
        }
        view.poll(&ctx);
        assert!(view.message.contains("unreadable source"));
        assert!(!temp.0.join("right/loop").exists());
        drop(view);
        std::fs::remove_dir(link).unwrap();
    }
    #[test]
    fn flattened_content_and_copy_use_virtual_partners_and_revision_safe_results() {
        let temp = Temp::new();
        std::fs::create_dir_all(temp.0.join("left/alpha")).unwrap();
        std::fs::create_dir_all(temp.0.join("right/beta")).unwrap();
        std::fs::write(temp.0.join("left/alpha/same.txt"), b"AAAA").unwrap();
        std::fs::write(temp.0.join("right/beta/same.txt"), b"BBBB").unwrap();
        let ctx = egui::Context::default();
        let mut view = temp.view(&ctx, false);
        wait(&mut view, &ctx);
        view.send(Command::Filters {
            names: String::new(),
            other: String::new(),
            content: String::new(),
            exclude_containing: false,
            view: ViewOptions {
                folders: FolderDisplay::Flatten,
                ..Default::default()
            },
        });
        wait(&mut view, &ctx);
        let index = row(&view, "same.txt");
        let pair = &view.snapshot.rows[index];
        assert!(pair.sides[0].is_some() && pair.sides[1].is_some());
        view.focus_row(index, None);
        view.send(Command::Compare {
            selection: view.selection.clone(),
            mode: ContentCompare::Binary,
        });
        wait(&mut view, &ctx);
        assert_eq!(view.snapshot.rows[row(&view, "same.txt")].center, "!=");
        view.focus_row(row(&view, "same.txt"), Some(0));
        view.operate(
            DirOp::Copy {
                from: Side::Left,
                to: Side::Right,
            },
            false,
        );
        let started = Instant::now();
        let mut confirmed = false;
        while view.busy() {
            view.poll(&ctx);
            if let Some(question) = view.question.take() {
                assert!(question.text.contains("beta"));
                confirmed = true;
                question.reply.send(Confirmation::YesToAll).unwrap();
            }
            assert!(started.elapsed() < Duration::from_secs(20));
            thread::sleep(Duration::from_millis(5));
        }
        wait(&mut view, &ctx);
        assert!(confirmed);
        assert_eq!(
            std::fs::read(temp.0.join("right/beta/same.txt")).unwrap(),
            b"AAAA"
        );
        assert!(!temp.0.join("right/alpha/same.txt").exists());
        view.focus_row(row(&view, "same.txt"), Some(0));
        view.send(Command::Filters {
            names: "-*.txt".into(),
            other: String::new(),
            content: String::new(),
            exclude_containing: false,
            view: ViewOptions::default(),
        });
        wait(&mut view, &ctx);
        assert!(view.selection.items.is_empty());
        view.navigate(true, false);
        frame(&mut view, &ctx);
    }
    #[test]
    fn crc_columns_quick_compare_and_sync_preview_stay_in_background() {
        let temp = Temp::new();
        let ctx = egui::Context::default();
        let mut view = temp.view(&ctx, true);
        wait(&mut view, &ctx);
        // The sync view lists differences only by default; show the same
        // file too for the quick-compare checks below.
        view.filters.options.display = bc_engine_dir::filter::DisplayFilter::All;
        view.apply_view_options();
        wait(&mut view, &ctx);
        view.send(Command::Details {
            selection: None,
            crc: true,
            version: false,
        });
        wait(&mut view, &ctx);
        assert_eq!(
            view.snapshot.rows[row(&view, "a.txt")].sides[0]
                .as_ref()
                .unwrap()
                .cells[2],
            "CBF43926"
        );
        view.focus_row(row(&view, "a.txt"), None);
        view.send(Command::Quick(
            view.selected_pair().unwrap(),
            ContentCompare::Binary,
        ));
        wait(&mut view, &ctx);
        assert!(view.quick.as_ref().unwrap().contains("Different"));
        for (name, expected) in [("a.txt", "Different"), ("b.txt", "Same")] {
            view.focus_row(row(&view, name), None);
            view.send(Command::Quick(
                view.selected_pair().unwrap(),
                ContentCompare::RulesBased,
            ));
            wait(&mut view, &ctx);
            assert!(view.quick.as_ref().unwrap().starts_with(expected));
        }
        view.focus_row(row(&view, "a.txt"), None);
        view.send(Command::SyncMode(
            SyncMode::Update(bc_engine_dir::sync::SyncDirection::Both),
            true,
            false,
        ));
        wait(&mut view, &ctx);
        assert!(view.snapshot.sync_stats.is_some());
        frame(&mut view, &ctx);
        let node = view.snapshot.rows[row(&view, "a.txt")].node.unwrap();
        view.send(Command::SyncOverride(node, SyncChoice::LeaveAlone));
        wait(&mut view, &ctx);
        assert_eq!(view.snapshot.rows[row(&view, "a.txt")].center, "Leave");
    }
    #[test]
    fn opening_folders_in_a_new_view_preserves_the_original_bases() {
        let temp = Temp::new();
        let ctx = egui::Context::default();
        let mut view = temp.view(&ctx, false);
        wait(&mut view, &ctx);
        let original = view.paths();
        view.focus_row(row(&view, "nested"), None);
        view.open_folder_selection();
        let child = view.child.as_mut().unwrap();
        wait(child, &ctx);
        assert!(child.paths()[0].ends_with("left/nested"));
        frame(&mut view, &ctx);
        assert_eq!(view.paths(), original);
        view.child = None;
        assert_eq!(view.paths(), original);
    }
    #[test]
    fn folder_history_up_swap_paths_and_drop_action_are_available() {
        let temp = Temp::new();
        let ctx = egui::Context::default();
        let mut view = temp.view(&ctx, false);
        wait(&mut view, &ctx);
        let initial = view.paths();
        view.reload(
            [
                initial[1].display().to_string(),
                initial[0].display().to_string(),
            ],
            true,
        );
        wait(&mut view, &ctx);
        assert_eq!(view.history.len(), 2);
        assert_eq!(view.paths()[0], initial[1]);
        view.history_at = 0;
        view.reload(view.history[0].clone(), false);
        wait(&mut view, &ctx);
        assert_eq!(view.paths(), initial);
        #[derive(Debug)]
        struct DropPath(PathBuf);
        impl egui::DroppedFile for DropPath {
            fn path(&self) -> &Path {
                &self.0
            }
            fn bytes(&self) -> Result<Vec<u8>, String> {
                Err("folder bytes must never be read".into())
            }
        }
        let mut home = crate::home::HomeView::default();
        let mut action = None;
        ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1400.0, 900.0),
                )),
                dropped_files: initial
                    .into_iter()
                    .map(|p| Arc::new(DropPath(p)) as egui::DroppedFileHandle)
                    .collect(),
                ..Default::default()
            },
            |ui| {
                if let Some(next) = home.ui(ui) {
                    action = Some(next);
                }
            },
        )
        .drop_without_applying_deltas();
        assert!(matches!(
            action,
            Some(crate::home::HomeAction::OpenFolderCompare { .. })
        ));
    }
    #[test]
    fn dialog_fields_cover_operation_variants_and_find_word_boundaries() {
        let clock = utc_clock();
        for kind in [
            Kind::CopyTo,
            Kind::MoveTo,
            Kind::Rename,
            Kind::Attributes,
            Kind::Touch,
            Kind::NewFolder,
            Kind::Delete,
        ] {
            let mut dialog = OperationDialog::new(kind);
            dialog.text = match kind {
                Kind::Attributes => "h",
                Kind::Touch => "2026-01-01 00:00:00",
                _ => "target",
            }
            .into();
            assert!(dialog.operation(clock).is_ok());
        }
        let mut find = FindDialog {
            text: "main".into(),
            whole_word: true,
            ..Default::default()
        };
        assert!(find.matches(&find.pattern().unwrap(), "main.rs"));
        assert!(!find.matches(&find.pattern().unwrap(), "domain.rs"));
        find.regex = true;
        find.text = "[".into();
        assert!(find.pattern().is_err());
    }

    fn menu_enabled(view: &DirCompareView, id: &str) -> Option<bool> {
        fn walk(items: &[MenuItem], id: &str) -> Option<bool> {
            for item in items {
                if item.action_id == id {
                    return Some(item.enabled);
                }
                if let MenuItemKind::Submenu(children) = &item.kind {
                    if let Some(found) = walk(children, id) {
                        return Some(found);
                    }
                }
            }
            None
        }
        view.menus.iter().find_map(|m| walk(&m.items, id))
    }

    #[test]
    fn tree_split_follows_the_persisted_splitter() {
        FOLDER_SPLIT_X.reset();
        // Default: even split (ceil), identical to the reference layout.
        assert_eq!(DirCompareView::chrome_split(1384.0, CHROME_GAP), (665.0, 664.0));
        FOLDER_SPLIT_X.store(400.0, 1329.0);
        let (left, right) = DirCompareView::chrome_split(1384.0, CHROME_GAP);
        assert_eq!((left, left + right), (400.0, 1329.0));
        // Same fraction in sync mode (wider centre column).
        let (left, right) = DirCompareView::chrome_split(1384.0, SYNC_GAP);
        assert!((left - (400.0_f32 / 1329.0 * 1249.0).ceil()).abs() < 0.5, "{left} {right}");
        // Each tree keeps its minimum width.
        FOLDER_SPLIT_X.store(5.0, 1329.0);
        assert_eq!(DirCompareView::chrome_split(1384.0, CHROME_GAP).0, 120.0);
        FOLDER_SPLIT_X.reset();
    }
    #[test]
    fn chrome_layout_menu_inventory_toolbar_states_and_reference_geometry() {
        let temp = Temp::new();
        let ctx = egui::Context::default();
        let mut view = temp.view(&ctx, false);
        wait(&mut view, &ctx);
        // 七个顶层菜单按参考顺序；未实现项禁用、已有动作启用。
        let titles: Vec<_> = view.menus.iter().map(|m| m.title.as_str()).collect();
        assert_eq!(
            titles,
            ["Session", "Actions", "Edit", "Search", "View", "Tools", "Help"]
        );
        frame(&mut view, &ctx);
        assert_eq!(menu_enabled(&view, "Options"), Some(false));
        assert_eq!(menu_enabled(&view, "Swap Sides"), Some(true));
        assert_eq!(menu_enabled(&view, "Expand All"), Some(true));
        assert_eq!(menu_enabled(&view, "Show Differences"), Some(true));
        assert_eq!(menu_enabled(&view, "Back"), Some(false));
        // 工具栏切换态反映当前显示过滤（默认 All 选中、Diffs 未选中、Stop 禁用）。
        let items = view.chrome_toolbar_items(false);
        let by_id = |id: &str| items.iter().find(|i| i.action_id == id).unwrap();
        assert!(by_id("Show All").selected && !by_id("Show Differences").selected);
        assert!(!by_id("Stop").enabled);
        assert!(by_id("Show Files").selected);
        view.filters.options.display = bc_engine_dir::filter::DisplayFilter::Mismatches;
        let items = view.chrome_toolbar_items(false);
        assert!(
            items
                .iter()
                .find(|i| i.action_id == "Show Differences")
                .unwrap()
                .selected
        );
        // 参考状态栏图例文本逐字一致。
        assert_eq!(
            shows_legend(bc_engine_dir::filter::DisplayFilter::All),
            "Shows same, left orphan, right orphan, left newer, right newer and difference items."
        );
        assert_eq!(
            shows_legend(bc_engine_dir::filter::DisplayFilter::Mismatches),
            "Shows left orphan, right orphan, left newer, right newer and difference items."
        );
        // 几何：1384x961 下首行选中填充行高 20，位于外框之下。
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1384.0, 961.0),
            )),
            ..Default::default()
        };
        view.focus_row(0, None);
        let mut output = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(crate::chrome::theme::PANEL_BG))
                .show(ui, |ui| {
                    view.ui(ui);
                });
        });
        let selected_fill = crate::chrome::theme::SELECTION;
        // The modern chrome is taller than the classic reference, so only
        // the row height and "below the chrome" are pinned, not y=165.
        let row0 = output.shapes.iter().any(|s| {
            matches!(&s.shape, egui::Shape::Rect(r)
                if r.fill == selected_fill
                    && r.rect.top() > 120.0
                    && r.rect.top() < 240.0
                    && (r.rect.height() - 20.0).abs() < 1.0)
        });
        output.textures_delta.clear();
        assert!(row0, "selected first row below the chrome, h=20");
    }

    #[test]
    fn chrome_menu_commands_drive_filters_expand_swap_and_home() {
        let temp = Temp::new();
        let ctx = egui::Context::default();
        let mut view = temp.view(&ctx, false);
        wait(&mut view, &ctx);
        let before = view.row_count();
        // View > Show Differences 过滤到差异行。
        assert!(view.folder_command("Show Differences").is_none());
        wait(&mut view, &ctx);
        assert!(view.row_count() < before);
        assert!(view
            .snapshot
            .rows
            .iter()
            .all(|r| !matches!(r.status, CompareStatus::Same)));
        // View > Show All 恢复。
        view.folder_command("Show All");
        wait(&mut view, &ctx);
        assert_eq!(view.row_count(), before);
        // Edit > Expand All 展开出子行。
        view.folder_command("Expand All");
        wait(&mut view, &ctx);
        assert!(view.snapshot.rows.iter().any(|r| r.depth == 1));
        // Session > Swap Sides 交换两侧。
        view.folder_command("Swap Sides");
        wait(&mut view, &ctx);
        assert!(view.paths()[0].ends_with("right"));
        assert!(view.paths()[1].ends_with("left"));
        // Home 进入待回 Home 状态（由 app 层处理）。
        assert!(!view.take_home_request());
        view.folder_command("Home");
        assert!(view.take_home_request());
        // View > Log 关闭日志区；View > Toolbar 保留。
        assert!(view.log_panel.open);
        view.folder_command("Log");
        assert!(!view.log_panel.open);
        view.folder_command("Log");
        assert!(view.log_panel.open);
        // Actions > Ignored 打开范围确认窗（与旧 Actions 菜单同路径），确认后标记；再次切换清除。
        view.focus_row(row(&view, "a.txt"), Some(0));
        view.folder_command("Ignored");
        let scope = view
            .scope
            .take()
            .expect("Ignored opens the scope review dialog");
        assert!(matches!(scope.op, DirOp::Ignored(true)));
        view.operate(scope.op, scope.include_hidden);
        let started = Instant::now();
        while view.busy() {
            view.poll(&ctx);
            if let Some(question) = view.question.take() {
                question.reply.send(Confirmation::YesToAll).unwrap();
            }
            assert!(started.elapsed() < Duration::from_secs(10));
            thread::sleep(Duration::from_millis(5));
        }
        wait(&mut view, &ctx);
        assert!(view.snapshot.rows[row(&view, "a.txt")].ignored);
        view.folder_command("Ignored");
        let scope = view
            .scope
            .take()
            .expect("Ignored toggle re-opens the dialog");
        assert!(matches!(scope.op, DirOp::Ignored(false)));
        view.operate(scope.op, scope.include_hidden);
        let started = Instant::now();
        while view.busy() {
            view.poll(&ctx);
            if let Some(question) = view.question.take() {
                question.reply.send(Confirmation::YesToAll).unwrap();
            }
            assert!(started.elapsed() < Duration::from_secs(10));
            thread::sleep(Duration::from_millis(5));
        }
        wait(&mut view, &ctx);
        assert!(!view.snapshot.rows[row(&view, "a.txt")].ignored);
    }

    #[test]
    fn snapshot_presets_match_reference_operations() {
        let temp = Temp::new();
        let ctx = egui::Context::default();
        // folder-expanded：先选中顶层行，再 Expand All；子行不追加选择（参考 Ctrl+A 先于展开）。
        let mut view = temp.view(&ctx, false);
        wait(&mut view, &ctx);
        let collapsed_rows = view.row_count();
        view.snapshot_preset("folder-expanded");
        wait(&mut view, &ctx);
        frame(&mut view, &ctx);
        assert!(view.row_count() > collapsed_rows);
        assert!(view.snapshot.rows.iter().any(|r| r.depth == 1));
        for (index, r) in view.snapshot.rows.iter().enumerate() {
            for side in 0..2 {
                if let Some(s) = &r.sides[side] {
                    let selected = view.selection.items.contains(&(s.node, side));
                    if r.depth == 0 {
                        assert!(selected, "root row {index} must stay selected");
                    } else {
                        assert!(!selected, "child row {index} must not be selected");
                    }
                }
            }
        }
        assert_eq!(view.focus, Some(0));
        // folder-diffs-filter：只留差异行（本语料首行 nested 为 Same，过滤后焦点清除属预期）。
        let mut view = temp.view(&ctx, false);
        wait(&mut view, &ctx);
        view.snapshot_preset("folder-diffs-filter");
        wait(&mut view, &ctx);
        assert!(view.row_count() > 0);
        assert!(view
            .snapshot
            .rows
            .iter()
            .all(|r| !matches!(r.status, CompareStatus::Same)));
        assert_eq!(
            view.filters.options.display,
            bc_engine_dir::filter::DisplayFilter::Mismatches
        );
        // folder-basic：折叠、首行双侧选中。
        let mut view = temp.view(&ctx, false);
        wait(&mut view, &ctx);
        view.snapshot_preset("folder-basic");
        frame(&mut view, &ctx);
        assert!(!view.snapshot.rows.iter().any(|r| r.depth > 0));
        assert_eq!(view.focus, Some(0));
        assert_eq!(view.selection.items.len(), 2);
    }

    #[test]
    fn local_time_label_matches_reference_format() {
        // 语料 mtime 由 set-corpus-mtime.ps1 固定为本地 2026-01-02 03:04:05。
        let clock = FilterClock {
            today_midnight: UNIX_EPOCH,
            utc_offset_seconds: 8 * 3600,
        };
        let t = UNIX_EPOCH + Duration::from_secs(1_767_294_245); // 2026-01-01 19:04:05 UTC
        assert_eq!(local_time_label(t, &clock), "1/2/2026 3:04:05 AM");
        let older = t - Duration::from_secs(86400);
        assert_eq!(local_time_label(older, &clock), "1/1/2026 3:04:05 AM");
        let noon = UNIX_EPOCH + Duration::from_secs(1_767_268_800); // 2026-01-01 12:00:00 UTC
        assert_eq!(local_time_label(noon, &clock), "1/1/2026 8:00:00 PM");
    }
}
