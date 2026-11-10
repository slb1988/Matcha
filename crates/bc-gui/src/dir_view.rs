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
use bc_core::Side;
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
#[derive(Clone)]
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
            view: ViewOptions::default(),
            criteria: Criteria::default(),
            handling: Default::default(),
            pairing: PairingOptions::default(),
            scan: ScanOptions {
                scan_orphans: sync || bench,
                expand_on_load: bench,
                ..Default::default()
            },
            clock: utc_clock(),
            sync_mode: sync.then_some((SyncMode::default(), false, false)),
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
                // 同步视图保持旧 UTC/占位文本；比较视图按参考用本地时间与空白占位。
                let compare = self.sync_mode.is_none();
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
            let mut center = if let Some((text, _)) = action {
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
            let mut conflict = action.is_some_and(|(_, c)| *c);
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
fn drive_letter(path: &str) -> Option<String> {
    let root = Path::new(path).components().next()?;
    let prefix = root.as_os_str().to_string_lossy();
    let drive = prefix.trim_end_matches(":\\").trim_end_matches(':');
    (!drive.is_empty()
        && !drive.starts_with("\\\\")
        && drive.chars().all(|c| c.is_ascii_alphabetic()))
    .then(|| drive.to_owned())
}
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
    compare_to: bool,
    align_to: bool,
    anchor: Option<(usize, usize)>,
    focus: Option<usize>,
    focus_side: Option<usize>,
    scroll_y: f32,
    pending_scroll: Option<usize>,
    pending_focus: Option<NodeId>,
    sort: (Column, usize, bool),
    rules_open: bool,
    criteria_text: String,
    pairing: PairingOptions,
    scan: ScanOptions,
    align_left: String,
    align_right: String,
    align_folder: String,
    align_regex: bool,
    /// 参考 chrome 布局状态（仅比较视图；同步视图保持旧布局）。
    menus: Vec<crate::chrome::menubar::MenuSpec>,
    home_requested: bool,
    tool_dropdown: Option<(String, egui::Pos2)>,
    show_files: bool,
    chrome_log: Vec<String>,
    chrome_log_logged: bool,
    log_save_open: bool,
    grid_lines: Option<(usize, Arc<Vec<RowLine>>)>,
    /// Status-bar selection totals keyed by snapshot and selection fingerprint.
    selection_totals: Option<(usize, (usize, u64), SelectionTotals)>,
}
impl DirCompareView {
    pub fn open(
        ctx: &egui::Context,
        left: &Path,
        right: &Path,
        sync_mode: bool,
        bench: bool,
    ) -> Self {
        let shared = Arc::new(Shared {
            latest: Mutex::new(None),
            progress: Mutex::new(Progress::default()),
            questions: Mutex::new(VecDeque::new()),
            notices: Mutex::new(VecDeque::new()),
            cancel: Mutex::new(CancelToken::new()),
            tokens: Mutex::new(BTreeMap::new()),
            next: AtomicU64::new(1),
            closed: AtomicBool::new(false),
            busy: AtomicBool::new(true),
            ctx: ctx.clone(),
            log: OpLog::default(),
        });
        let (tx, rx) = mpsc::channel();
        let copy = shared.clone();
        let worker = thread::spawn(move || run_worker(rx, copy, sync_mode, bench));
        let paths = [left.display().to_string(), right.display().to_string()];
        let token = CancelToken::new();
        shared.tokens.lock().unwrap().insert(0, token.clone());
        let _ = tx.send(Work {
            id: 0,
            command: Command::Load(paths.clone()),
            cancel: token,
        });
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
            paths: PathsDialog {
                left: paths[0].clone(),
                right: paths[1].clone(),
                recent: paths.to_vec(),
                ..Default::default()
            },
            history: vec![paths],
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
            message: "Loading folders".into(),
            quick: None,
            text: None,
            text_prefs: Prefs::default(),
            exit_requested: false,
            compare_to: false,
            align_to: false,
            anchor: None,
            focus: None,
            focus_side: None,
            scroll_y: 0.0,
            pending_scroll: None,
            pending_focus: None,
            sort: (Column::Name, 0, false),
            rules_open: false,
            criteria_text: Criteria::default().to_script(),
            pairing: PairingOptions::default(),
            scan: ScanOptions {
                scan_orphans: sync_mode || bench,
                expand_on_load: bench,
                ..Default::default()
            },
            align_left: String::new(),
            align_right: String::new(),
            align_folder: String::new(),
            align_regex: false,
            menus: folder_menu_inventory(),
            home_requested: false,
            tool_dropdown: None,
            show_files: true,
            chrome_log: Vec::new(),
            chrome_log_logged: false,
            log_save_open: false,
            grid_lines: None,
            selection_totals: None,
        };
        if !sync_mode {
            // 参考：比较视图过滤栏默认 *.*，日志区默认打开。
            view.filters.names = "*.*".into();
            view.log_panel.open = true;
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
            if self.snapshot.complete && !self.chrome_log_logged && !self.sync_mode {
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
                    if !self.sync_mode {
                        self.chrome_log.push(text.clone());
                        self.chrome_log.truncate(200);
                    }
                    self.message = text;
                }
                Notice::Clipboard(text) => ctx.copy_text(text),
                Notice::Quick(text) => self.quick = Some(text),
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
        self.cancel();
        self.selection.items.clear();
        self.focus = None;
        self.paths.left = paths[0].clone();
        self.paths.right = paths[1].clone();
        self.paths.remember(&paths);
        if remember {
            self.history.truncate(self.history_at + 1);
            self.history.push(paths.clone());
            self.history_at = self.history.len() - 1;
        }
        self.send(Command::Load(paths));
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
                    view.request_home();
                }
                true
            }
            _ => false,
        }
    }
    /// True once a text window confirmed Exit (the app then closes the window).
    pub fn take_exit_request(&mut self) -> bool {
        let child = self.child.as_mut().is_some_and(|c| c.take_exit_request());
        std::mem::take(&mut self.exit_requested) || child
    }
    fn new_folder_view(&mut self, paths: [PathBuf; 2]) {
        self.rules_open = false;
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
    fn columns_menu(&mut self, ui: &mut egui::Ui) {
        for column in Column::OPTIONAL {
            let mut enabled = self.columns.contains(&column);
            if ui.checkbox(&mut enabled, column.label()).changed() {
                if enabled {
                    self.columns.push(column);
                    if matches!(column, Column::Crc | Column::Version) {
                        self.send(Command::Details {
                            selection: (!self.selection.items.is_empty())
                                .then(|| self.selection.clone()),
                            crc: column == Column::Crc,
                            version: column == Column::Version,
                        });
                    }
                } else {
                    self.columns.retain(|c| *c != column);
                }
            }
        }
        ui.small("Owner/group/VCS require providers not shipped in this wave.");
    }
    fn action_menu(&mut self, ui: &mut egui::Ui) {
        if ui.button("Open folders in new view").clicked() {
            self.open_folder_selection();
            ui.close();
        }
        if ui.button("Open selected file pair").clicked() {
            self.open_pair();
            ui.close();
        }
        if ui.button("Compare to (pick another file)").clicked() {
            self.compare_to = true;
            self.align_to = false;
            ui.close();
        }
        if ui.button("Align with (pick other side)").clicked() {
            self.align_to = true;
            self.compare_to = false;
            ui.close();
        }
        ui.menu_button("Quick compare", |ui| {
            for (mode, label) in [
                (ContentCompare::Size, "Size"),
                (ContentCompare::Crc, "CRC"),
                (ContentCompare::Binary, "Binary"),
                (ContentCompare::RulesBased, "Rules-based"),
            ] {
                if ui.button(label).clicked() {
                    match self.selected_pair() {
                        Ok(paths) => self.send(Command::Quick(paths, mode)),
                        Err(e) => self.message = e,
                    }
                    ui.close();
                }
            }
        });
        ui.menu_button("Compare contents", |ui| {
            for (mode, label) in [
                (ContentCompare::Crc, "CRC"),
                (ContentCompare::Binary, "Binary"),
                (ContentCompare::RulesBased, "Rules-based"),
            ] {
                if ui.button(label).clicked() {
                    self.send(Command::Compare {
                        selection: self.selection.clone(),
                        mode,
                    });
                    ui.close();
                }
            }
        });
        ui.separator();
        for (label, op) in [
            (
                "Copy to right",
                DirOp::Copy {
                    from: Side::Left,
                    to: Side::Right,
                },
            ),
            (
                "Copy to left",
                DirOp::Copy {
                    from: Side::Right,
                    to: Side::Left,
                },
            ),
            ("Copy to other side", DirOp::CopyOther),
            (
                "Move to right",
                DirOp::Move {
                    from: Side::Left,
                    to: Side::Right,
                },
            ),
            (
                "Move to left",
                DirOp::Move {
                    from: Side::Right,
                    to: Side::Left,
                },
            ),
            ("Move to other side", DirOp::MoveOther),
            ("Exchange", DirOp::Exchange),
            ("Exclude", DirOp::Exclude),
            ("Mark ignored", DirOp::Ignored(true)),
            ("Clear ignored", DirOp::Ignored(false)),
            ("Copy filenames", DirOp::CopyFilename),
        ] {
            if ui.button(label).clicked() {
                if op == DirOp::CopyFilename {
                    self.operate(op, false);
                } else {
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
                ui.close();
            }
        }
        for (label, op) in [
            ("Copy to side...", DirOp::CopyOther),
            ("Move to side...", DirOp::MoveOther),
        ] {
            if ui.button(label).clicked() {
                self.scope = Some(dialogs::scope::ScopeDialog::new(
                    op,
                    self.handling.include_hidden,
                    true,
                ));
                ui.close();
            }
        }
        for (label, kind) in [
            ("Copy to folder...", Kind::CopyTo),
            ("Move to folder...", Kind::MoveTo),
            ("Delete...", Kind::Delete),
            ("Rename...", Kind::Rename),
            ("Attributes...", Kind::Attributes),
            ("Touch...", Kind::Touch),
            ("New folder...", Kind::NewFolder),
        ] {
            if ui.button(label).clicked() {
                let mut dialog = OperationDialog::new(kind);
                dialog.include_hidden =
                    self.handling.include_hidden || self.filters.options.suppress_filters;
                dialog.recent = self.paths.recent.clone();
                self.operation = Some(dialog);
                ui.close();
            }
        }
        ui.separator();
        for (label, expanded, recursive) in [
            ("Open folder", true, false),
            ("Close folder", false, false),
            ("Open subfolders", true, true),
            ("Close subfolders", false, true),
        ] {
            if ui.button(label).clicked() {
                self.expand_selection(expanded, recursive);
                ui.close();
            }
        }
        if ui.button("Set as base folder").clicked() {
            self.set_base(false, false);
            ui.close();
        }
        if ui.button("Set as base on other side").clicked() {
            self.set_base(true, false);
            ui.close();
        }
        if ui.button("Set as base folders").clicked() {
            self.set_base(false, true);
            ui.close();
        }
        if ui.button("Refresh selection").clicked() {
            self.send(Command::Refresh {
                selection: Some(self.selection.clone()),
                full: false,
            });
            ui.close();
        }
    }
    fn toolbar(&mut self, ui: &mut egui::Ui) -> Option<[PathBuf; 2]> {
        let mut switch = None;
        ui.horizontal(|ui| {
            ui.menu_button("Session", |ui| {
                if ui.button("Browse base folders...").clicked() {
                    self.paths.open = true;
                    ui.close();
                }
                if ui.button("Swap sides").clicked() {
                    self.reload([self.paths.right.clone(), self.paths.left.clone()], true);
                    ui.close();
                }
                for (label, target) in [
                    ("Up left", Target::Left),
                    ("Up right", Target::Right),
                    ("Up both", Target::All),
                ] {
                    if ui.button(label).clicked() {
                        self.up(target);
                        ui.close();
                    }
                }
                if ui.button("Compare parent folders (new view)").clicked() {
                    let paths = self
                        .paths()
                        .map(|p| p.parent().map(Path::to_path_buf).unwrap_or(p));
                    self.new_folder_view(paths);
                    ui.close();
                }
                if ui.button("Comparison rules / handling...").clicked() {
                    self.rules_open = true;
                    ui.close();
                }
                if ui
                    .button(if self.sync_mode {
                        "Compare base folders"
                    } else {
                        "Sync base folders"
                    })
                    .clicked()
                {
                    switch = Some(self.paths());
                    ui.close();
                }
            });
            ui.menu_button("Actions", |ui| self.action_menu(ui));
            ui.menu_button("Selection", |ui| {
                for (label, mask) in [
                    ("Select all", "all"),
                    ("Select all files", "files"),
                    ("Select newer left", "newer-left"),
                    ("Select newer right", "newer-right"),
                    ("Select newer both", "newer"),
                    ("Select orphans left", "orphan-left"),
                    ("Select orphans right", "orphan-right"),
                    ("Select orphans both", "orphans"),
                ] {
                    if ui.button(label).clicked() {
                        self.selection.items.clear();
                        self.select_visible(mask, false);
                        ui.close();
                    }
                }
                if ui.button("Invert selection").clicked() {
                    self.select_visible("all", true);
                    ui.close();
                }
            });
            ui.menu_button("View", |ui| {
                ui.checkbox(&mut self.toolbar, "Toolbar");
                ui.checkbox(&mut self.log_panel.open, "Log");
                ui.checkbox(&mut self.legend_open, "Legend");
                ui.checkbox(
                    &mut self.filters.show_none_option,
                    "Advanced: Show none filter",
                );
                self.columns_menu(ui);
            });
            if ui.button("Rules").clicked() {
                self.rules_open = true;
            }
            if ui.button("Find").clicked() {
                self.find.open = true;
            }
            if ui.button("Find next").clicked() {
                self.find_next(false);
            }
            if ui.button("Find previous").clicked() {
                self.find_next(true);
            }
        });
        if self.toolbar {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(self.history_at > 0, egui::Button::new("Back"))
                    .clicked()
                {
                    self.history_at -= 1;
                    self.reload(self.history[self.history_at].clone(), false);
                }
                if ui
                    .add_enabled(
                        self.history_at + 1 < self.history.len(),
                        egui::Button::new("Forward"),
                    )
                    .clicked()
                {
                    self.history_at += 1;
                    self.reload(self.history[self.history_at].clone(), false);
                }
                if ui.button("Expand all").clicked() {
                    self.send(Command::ExpandAll(true));
                }
                if ui.button("Collapse all").clicked() {
                    self.send(Command::ExpandAll(false));
                }
                if ui.button("Refresh").clicked() {
                    self.send(Command::Refresh {
                        selection: None,
                        full: false,
                    });
                }
                if ui.button("Full refresh").clicked() {
                    self.send(Command::Refresh {
                        selection: None,
                        full: true,
                    });
                }
                if ui.button("Previous difference").clicked() {
                    self.navigate(true, false);
                }
                if ui.button("Next difference").clicked() {
                    self.navigate(false, false);
                }
            });
        }
        switch
    }
    fn rules_dialog(&mut self, ctx: &egui::Context) {
        let mut open = self.rules_open;
        let mut apply = false;
        let mut align = false;
        let mut apply_filters = false;
        let mut save_presets = false;
        egui::Window::new("Folder comparison rules").open(&mut open).show(ctx,|ui|{
            // 旧过滤行的附加字段迁到这里（名称掩码在过滤栏组合框）。
            ui.label("Name filters are edited in the toolbar filter box; additional script filters:");
            ui.add(egui::TextEdit::singleline(&mut self.filters.other).desired_width(550.0));
            ui.label("UTF-8 content text (empty disables)");
            ui.text_edit_singleline(&mut self.filters.content);
            ui.checkbox(&mut self.filters.exclude_containing,"Exclude containing (otherwise exclude not containing)");
            if ui.button("Apply filters").clicked(){apply_filters=true;}
            ui.horizontal(|ui|{
                ui.text_edit_singleline(&mut self.filters.preset_name);
                if ui.button("Save named preset").clicked(){
                    match self.filters.presets.upsert(&self.filters.preset_name,&self.filters.names){
                        Ok(())=>{self.filters.error=None;save_presets=true;}
                        Err(e)=>self.filters.error=Some(e),
                    }
                }
            });
            ui.separator();
            ui.label("criteria arguments");ui.add(egui::TextEdit::singleline(&mut self.criteria_text).desired_width(550.0));
            ui.small("Examples: timestamp:2sec;IgnoreDST size / CRC / binary / rules-based / skip-quick-same. Rules-based uses the default text rules; add comments-unimportant to ignore comment edits.");
            ui.checkbox(&mut self.pairing.case_sensitive,"Case-sensitive pairing");ui.checkbox(&mut self.pairing.ignore_extension,"Align different extensions");ui.checkbox(&mut self.pairing.normalize_nfc,"Align Unicode normalization forms");
            ui.checkbox(&mut self.scan.automatic,"Automatic recursive scan");ui.checkbox(&mut self.scan.scan_orphans,"Scan top-level orphan folders");ui.checkbox(&mut self.scan.expand_on_load,"Expand folders on load");ui.checkbox(&mut self.scan.differences_only,"Expand differences only");ui.checkbox(&mut self.scan.auto_refresh,"Automatic filesystem refresh");
            ui.separator();
            ui.checkbox(&mut self.handling.copy_timestamps,"Preserve timestamps");
            ui.checkbox(&mut self.handling.copy_attributes,"Preserve attributes");
            ui.checkbox(&mut self.handling.copy_creation_time,"Preserve creation time");
            ui.checkbox(&mut self.handling.include_hidden,"Include hidden items by default");
            ui.add_enabled(false,egui::Checkbox::new(&mut self.handling.copy_ntfs_permissions,"NTFS permissions (deferred)"));
            if ui.button("Apply and reload").clicked(){apply=true;}
            ui.separator();ui.label("Alignment override (left pattern / right pattern / optional relative folder)");ui.text_edit_singleline(&mut self.align_left);ui.text_edit_singleline(&mut self.align_right);ui.text_edit_singleline(&mut self.align_folder);ui.checkbox(&mut self.align_regex,"Regex captures");if ui.button("Add alignment override").clicked(){align=true;}
        });
        self.rules_open = open;
        if apply_filters {
            self.apply_view_options();
        }
        if save_presets {
            self.send(Command::SavePresets(self.filters.presets.to_xml()));
        }
        if apply {
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
        if align {
            self.send(Command::AlignPattern {
                left: self.align_left.clone(),
                right: self.align_right.clone(),
                regex: self.align_regex,
                folder: (!self.align_folder.is_empty()).then(|| self.align_folder.clone()),
            });
        }
    }
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<FolderNavigation> {
        let ctx = ui.ctx().clone();
        self.poll(&ctx);
        let mut switch = None;
        let busy = self.busy();
        if self.sync_mode {
            // 同步视图本期不换肤：沿用旧行式布局。
            ui.add_enabled_ui(!busy && self.child.is_none(), |ui| {
                switch = self.toolbar(ui).map(|paths| (!self.sync_mode, paths));
                let change = self.filters.ui_legacy(ui, !self.sync_mode);
                if change.save_presets {
                    self.send(Command::SavePresets(self.filters.presets.to_xml()));
                }
                if change.apply {
                    self.send(Command::Filters {
                        names: self.filters.names.clone(),
                        other: self.filters.other.clone(),
                        content: self.filters.content.clone(),
                        exclude_containing: self.filters.exclude_containing,
                        view: self.filters.options,
                    });
                }
            });
            ui.horizontal(|ui| {
                ui.label(format!("{}  |  {}", self.paths.left, self.paths.right));
            });
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{} files, {} folders; {} visible rows; {} selected",
                    self.snapshot.files,
                    self.snapshot.folders,
                    self.row_count(),
                    self.selection.items.len()
                ));
                if busy {
                    let p = self.shared.progress.lock().unwrap().clone();
                    let fraction = p
                        .total
                        .filter(|n| *n > 0)
                        .map_or(0.0, |n| p.current as f32 / n as f32);
                    ui.add(
                        egui::ProgressBar::new(fraction)
                            .desired_width(180.0)
                            .text(&p.message),
                    );
                    if ui.button("Cancel task").clicked() {
                        self.cancel();
                    }
                } else {
                    ui.label(&self.message);
                }
            });
            if let Some(path) = self.log_panel.ui(ui, &self.shared.log) {
                self.send(Command::SaveLog(path.into()));
            }
            ui.add_enabled_ui(self.child.is_none(), |ui| self.draw_table(ui));
        } else {
            switch = self.chrome_ui(ui, busy);
        }
        if let Some(paths) = self.paths.show(&ctx) {
            self.reload(paths, true);
        }
        if self.find.show(&ctx) {
            self.find_next(self.find.backwards);
        }
        self.rules_dialog(&ctx);
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
            egui::Window::new("Quick compare")
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
            egui::Window::new("Text Compare — selected files")
                .id(egui::Id::unique(("folder_text", self.window_id)))
                .open(&mut open)
                .default_size([1150.0, 650.0])
                .show(&ctx, |ui| action = view.ui(ui, &mut self.text_prefs));
            if !open {
                // The close box goes through the text view's own save prompt.
                view.request_home();
            }
            match action {
                Some(ViewAction::Home) => self.text = None,
                Some(ViewAction::Exit) => {
                    self.text = None;
                    self.exit_requested = true;
                }
                // Only offered for clean text, so replacing the window loses nothing.
                Some(ViewAction::CompareParentFolders { left, right }) => {
                    self.new_folder_view([left, right]);
                }
                None => {}
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
            if !self.sync_mode {
                // 参考快捷键（比较视图）：导航/选择/刷新/查找。
                let pressed = |key: egui::Key, ctrl: bool, shift: bool, alt: bool| {
                    ctx.input(|i| {
                        i.key_pressed(key)
                            && i.modifiers.ctrl == ctrl
                            && i.modifiers.shift == shift
                            && i.modifiers.alt == alt
                    })
                };
                if pressed(egui::Key::N, true, false, false) {
                    self.navigate(false, false);
                }
                if pressed(egui::Key::P, true, false, false) {
                    self.navigate(true, false);
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
                if pressed(egui::Key::F2, false, false, false) && !busy {
                    let mut dialog = OperationDialog::new(Kind::Rename);
                    dialog.include_hidden =
                        self.handling.include_hidden || self.filters.options.suppress_filters;
                    dialog.recent = self.paths.recent.clone();
                    self.operation = Some(dialog);
                }
                if pressed(egui::Key::Insert, false, false, false) && !busy {
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
            egui::Window::new("Folder Compare — new view")
                .id(egui::Id::unique(("folder_child", self.window_id)))
                .open(&mut open)
                .default_size([1200.0, 720.0])
                .show(&ctx, |ui| {
                    if let Some(action) = child.ui(ui) {
                        switch = Some(action);
                    }
                });
            if !open {
                self.child = None;
                self.message = "Child view closed; Refresh if its files changed".into();
            }
        }
        switch
    }
    fn draw_table(&mut self, ui: &mut egui::Ui) {
        let columns = self.columns.clone();
        let data_width: f32 = columns.iter().map(|c| c.width()).sum();
        let center_width = if self.sync_mode { 88.0 } else { 48.0 };
        let width = ui
            .available_width()
            .max(2.0 * (data_width + 180.0) + center_width);
        let half = (width - center_width) / 2.0;
        let name_width = half - data_width;
        let mut sort = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for side in 0..2 {
                if side == 1 {
                    ui.add_space(center_width);
                }
                let header = ui.add_sized(
                    [name_width, 20.0],
                    egui::Button::new(if side == 0 { "Left name" } else { "Right name" }),
                );
                if header.clicked() {
                    sort = Some((Column::Name, side));
                }
                header.context_menu(|ui| self.columns_menu(ui));
                for column in &columns {
                    let header =
                        ui.add_sized([column.width(), 20.0], egui::Button::new(column.label()));
                    if header.clicked() {
                        sort = Some((*column, side));
                    }
                    header.context_menu(|ui| self.columns_menu(ui));
                }
            }
        });
        if let Some((column, side)) = sort {
            let reverse = self.sort.0 == column && self.sort.1 == side && !self.sort.2;
            self.sort = (column, side, reverse);
            self.send(Command::Sort(column, side, reverse));
        }
        if let Some(row) = self.pending_scroll.take() {
            self.scroll_y = row.min(self.row_count().saturating_sub(1)) as f32 * ROW_HEIGHT;
        }
        let snapshot = self.snapshot.clone();
        let mut drawn = 0;
        let mut first_drawn = None;
        let mut click = None;
        let mut context_command = None;
        let palette = self.palette;
        let selection = &self.selection;
        let automatic = self.scan.automatic;
        ui.spacing_mut().item_spacing.y = 0.0;
        let result = egui::ScrollArea::both()
            .id_salt("folder_rows")
            .auto_shrink([false, false])
            .vertical_scroll_offset(self.scroll_y)
            .show_rows(ui, ROW_HEIGHT, snapshot.rows.len(), |ui, range| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.set_min_width(width);
                for index in range {
                    first_drawn.get_or_insert(index);
                    let row = &snapshot.rows[index];
                    let (rect, response) =
                        ui.allocate_exact_size(egui::vec2(width, ROW_HEIGHT), egui::Sense::click());
                    drawn += 1;
                    ui.painter().rect_filled(
                        rect,
                        0.0,
                        if index % 2 == 0 {
                            egui::Color32::from_rgb(253, 253, 250)
                        } else {
                            egui::Color32::from_rgb(245, 247, 245)
                        },
                    );
                    for side in 0..2 {
                        let x = rect.left() + if side == 0 { 0.0 } else { half + center_width };
                        let cell_rect = egui::Rect::from_min_size(
                            egui::pos2(x, rect.top()),
                            egui::vec2(half, ROW_HEIGHT),
                        );
                        let Some(item) = &row.sides[side] else {
                            continue;
                        };
                        if selection.items.contains(&(item.node, side)) {
                            ui.painter().rect_filled(
                                cell_rect,
                                0.0,
                                egui::Color32::from_rgb(215, 230, 253),
                            );
                        }
                        let color = legend::side_color(palette, row.status, side, row.teal[side]);
                        let indent = row.depth as f32 * 14.0;
                        let text_x = x + 6.0 + indent;
                        if row.directory {
                            let p = egui::pos2(text_x, rect.center().y);
                            let folder_color = if !automatic && row.pending && !row.expanded {
                                legend::color(palette.unscanned)
                            } else {
                                color
                            };
                            let body = egui::Rect::from_min_size(
                                p + egui::vec2(0.0, -3.0),
                                egui::vec2(12.0, 8.0),
                            );
                            let tab = egui::Rect::from_min_size(
                                p + egui::vec2(1.0, -6.0),
                                egui::vec2(5.0, 3.0),
                            );
                            ui.painter().rect_filled(tab, 1.0, folder_color);
                            if row.pending && automatic {
                                ui.painter().rect_stroke(
                                    body,
                                    1.0,
                                    egui::Stroke::new(1.2, folder_color),
                                    egui::StrokeKind::Inside,
                                );
                            } else {
                                ui.painter().rect_filled(body, 1.0, folder_color);
                                if !automatic && row.pending {
                                    ui.painter().rect_stroke(
                                        body,
                                        1.0,
                                        egui::Stroke::new(1.0, egui::Color32::GRAY),
                                        egui::StrokeKind::Inside,
                                    );
                                }
                            }
                            if row.red {
                                ui.painter().rect_filled(
                                    egui::Rect::from_min_size(
                                        egui::pos2(text_x + 10.0, rect.top() + 2.0),
                                        egui::vec2(4.0, 4.0),
                                    ),
                                    0.0,
                                    legend::color(palette.different),
                                );
                            }
                            if row.purple {
                                ui.painter().rect_filled(
                                    egui::Rect::from_min_size(
                                        egui::pos2(text_x + 10.0, rect.bottom() - 6.0),
                                        egui::vec2(4.0, 4.0),
                                    ),
                                    0.0,
                                    legend::color(palette.orphan),
                                );
                            }
                        }
                        if item.entry.error.is_some() {
                            ui.painter().text(
                                egui::pos2(text_x + 4.0, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                "x",
                                egui::FontId::monospace(13.0),
                                egui::Color32::RED,
                            );
                        }
                        let clipped = ui.painter().with_clip_rect(egui::Rect::from_min_max(
                            egui::pos2(x, rect.top()),
                            egui::pos2(x + name_width - 3.0, rect.bottom()),
                        ));
                        clipped.text(
                            egui::pos2(
                                text_x + if row.directory { 19.0 } else { 4.0 },
                                rect.center().y,
                            ),
                            egui::Align2::LEFT_CENTER,
                            &item.label,
                            egui::FontId::monospace(13.0),
                            color,
                        );
                        let mut field_x = x + name_width;
                        for column in &columns {
                            let at = Column::OPTIONAL.iter().position(|c| c == column).unwrap();
                            let bounds = egui::Rect::from_min_size(
                                egui::pos2(field_x, rect.top()),
                                egui::vec2(column.width() - 3.0, ROW_HEIGHT),
                            );
                            ui.painter().with_clip_rect(bounds).text(
                                bounds.left_center(),
                                egui::Align2::LEFT_CENTER,
                                &item.cells[at],
                                egui::FontId::monospace(12.0),
                                color,
                            );
                            field_x += column.width();
                        }
                    }
                    let center = egui::Rect::from_min_size(
                        egui::pos2(rect.left() + half, rect.top()),
                        egui::vec2(center_width, ROW_HEIGHT),
                    );
                    ui.painter().text(
                        center.center(),
                        egui::Align2::CENTER_CENTER,
                        &row.center,
                        egui::FontId::monospace(12.0),
                        if row.conflict {
                            egui::Color32::RED
                        } else {
                            egui::Color32::DARK_GRAY
                        },
                    );
                    let response=response.on_hover_ui(|ui|{
                        for item in row.sides.iter().flatten(){ui.label(item.path.display().to_string());if let Some(error)=&item.entry.error{ui.colored_label(egui::Color32::RED,error);}}
                        if row.node.is_none(){ui.small("Flattened pair: operations use these actual paths; orphans retain their original relative path.");}
                    });
                    if response.clicked() || response.double_clicked() {
                        let x = response
                            .interact_pointer_pos()
                            .map_or(center.center().x, |p| p.x);
                        let side = if x < center.left() {
                            Some(0)
                        } else if x > center.right() {
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
                    response.context_menu(|ui| {
                        if ui.button("Select row").clicked() {
                            context_command = Some((index, 0));
                            ui.close();
                        }
                        if ui.button("Open file pair").clicked() {
                            context_command = Some((index, 1));
                            ui.close();
                        }
                        if row.directory && ui.button("Expand / collapse").clicked() {
                            context_command = Some((index, 2));
                            ui.close();
                        }
                    });
                }
            });
        self.scroll_y = result.state.clamped_offset().y;
        self.last_stats = DirFrameStats {
            rows_drawn: drawn,
            top_row: first_drawn.unwrap_or(0),
        };
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
const CHROME_SCROLLBAR: f32 = 18.0;

const TREE_STRIPE: egui::Color32 = egui::Color32::from_rgb(248, 250, 249);
const TREE_SELECTED: egui::Color32 = egui::Color32::from_rgb(168, 205, 241);
const TREE_LINE: egui::Color32 = egui::Color32::from_rgb(128, 128, 128);
const HEADER_SEP: egui::Color32 = egui::Color32::from_rgb(229, 229, 229);
const HEADER_EDGE: egui::Color32 = egui::Color32::from_rgb(160, 160, 160);
const TREE_EDGE: egui::Color32 = egui::Color32::from_rgb(192, 192, 192);
const LOG_FILL: egui::Color32 = egui::Color32::from_rgb(227, 231, 237);
const COMBO_EDGE: egui::Color32 = egui::Color32::from_rgb(141, 141, 141);

/// 每行树线标记：参考只画“父节点 → 直接子行”的肘形连接，无跨层贯通竖线。
/// `parent_up` = 本行到父行的行距（行数）；0 表示顶层行无连接线。
#[derive(Clone, Copy, Default)]
pub struct RowLine {
    parent_up: u16,
}

/// 正向一次扫描求每行父连接线的起点（祖先栈）。
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
    fn folder_command(&mut self, id: &str) -> Option<FolderNavigation> {
        use bc_engine_dir::filter::{DisplayFilter as D, FolderDisplay as F};
        match id {
            "Home" => {
                if !self.request_text_close(false) {
                    self.home_requested = true;
                }
            }
            "Session Settings" => self.rules_open = true,
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
            "browse-left" | "browse-right" => self.paths.open = true,
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
    fn chrome_dropdown(&mut self, ui: &mut egui::Ui) -> Option<FolderNavigation> {
        let (name, pos) = self.tool_dropdown.clone()?;
        let mut picked: Option<String> = None;
        let area = egui::Area::new(egui::Id::unique(("folder-tool-dropdown", self.window_id)))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .movable(false)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style())
                    .fill(egui::Color32::WHITE)
                    .stroke(egui::Stroke::new(1.0, crate::chrome::theme::BORDER))
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        match name.as_str() {
                            "Sessions" => {
                                for (label, id, enabled) in [
                                    ("Home", "Home", true),
                                    ("Session Settings...", "Session Settings", true),
                                    ("Open Session", "Open Session", false),
                                    ("Save Session", "Save Session", false),
                                ] {
                                    if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                                        picked = Some(id.to_string());
                                    }
                                }
                            }
                            "Show Differences" => {
                                use bc_engine_dir::filter::DisplayFilter as D;
                                for (label, id, mode) in [
                                    ("Show All", "Show All", D::All),
                                    ("Show Differences", "Show Differences", D::Mismatches),
                                    ("Show Same", "Show Same", D::Matches),
                                    ("Show No Orphans", "Show No Orphans", D::NoOrphans),
                                    (
                                        "Show Differences but No Orphans",
                                        "Show Differences but No Orphans",
                                        D::MismatchesNoOrphans,
                                    ),
                                    ("Show Orphans", "Show Orphans", D::Orphans),
                                    ("Show Left Newer", "Show Left Newer", D::LeftNewer),
                                    ("Show Right Newer", "Show Right Newer", D::RightNewer),
                                    (
                                        "Show Left Newer and Left Orphans",
                                        "Show Left Newer and Left Orphans",
                                        D::LeftNewerOrphans,
                                    ),
                                    (
                                        "Show Right Newer and Right Orphans",
                                        "Show Right Newer and Right Orphans",
                                        D::RightNewerOrphans,
                                    ),
                                    ("Show Left Orphans", "Show Left Orphans", D::LeftOrphans),
                                    ("Show Right Orphans", "Show Right Orphans", D::RightOrphans),
                                ] {
                                    if ui
                                        .selectable_label(
                                            self.filters.options.display == mode,
                                            label,
                                        )
                                        .clicked()
                                    {
                                        picked = Some(id.to_string());
                                    }
                                }
                            }
                            "Show Files" => {
                                if ui.selectable_label(self.show_files, "Show Files").clicked() {
                                    picked = Some("Show Files".to_string());
                                }
                                ui.separator();
                                for (label, id, mode) in [
                                    (
                                        "Always Show Folders",
                                        "Always Show Folders",
                                        FolderDisplay::AlwaysShow,
                                    ),
                                    (
                                        "Compare Files and Folder Structure",
                                        "Compare Files and Folder Structure",
                                        FolderDisplay::FilesAndStructure,
                                    ),
                                    (
                                        "Only Compare Files",
                                        "Only Compare Files",
                                        FolderDisplay::FilesOnly,
                                    ),
                                    (
                                        "Ignore Folder Structure",
                                        "Ignore Folder Structure",
                                        FolderDisplay::Flatten,
                                    ),
                                ] {
                                    if ui
                                        .selectable_label(
                                            self.filters.options.folders == mode,
                                            label,
                                        )
                                        .clicked()
                                    {
                                        picked = Some(id.to_string());
                                    }
                                }
                            }
                            _ => {}
                        }
                    });
            });
        let mut switch = None;
        if let Some(id) = picked {
            switch = self.folder_command(&id);
            self.tool_dropdown = None;
        }
        if ui.input(|i| {
            i.key_pressed(egui::Key::Escape)
                || (i.pointer.any_pressed()
                    && i.pointer
                        .interact_pos()
                        .is_some_and(|p| !area.response.rect.contains(p)))
        }) {
            self.tool_dropdown = None;
        }
        switch
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
            if let Some(action) = response.clicked {
                switch = self.folder_command(&action).or(switch);
            }
            if let Some(action) = response.dropdown {
                self.tool_dropdown = Some((
                    action,
                    ui.ctx().pointer_latest_pos().unwrap_or(ui.cursor().min),
                ));
            }
        }
        if let Some(action) = self.chrome_dropdown(ui) {
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
        let log_h = if self.log_panel.open { CHROME_LOG } else { 0.0 };
        let status_h = theme::STATUS_HEIGHT;
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
        if log_h > 0.0 {
            self.draw_log(ui, log_rect);
        }
        self.draw_status(ui, status, busy);
        ui.allocate_rect(full, egui::Sense::hover());
        switch
    }
    /// 窗口总宽 → (左树宽, 右树宽)；中缝与右侧滚动条固定。
    fn chrome_split(width: f32) -> (f32, f32) {
        let content = (width - CHROME_GAP - CHROME_SCROLLBAR).max(2.0);
        let left = (content / 2.0).ceil();
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
        let (left_w, _) = Self::chrome_split(bar.width());
        let split_x = bar.left() + left_w + CHROME_GAP;
        let mut reload: Option<[String; 2]> = None;
        for side in 0..2 {
            let origin = if side == 0 { bar.left() } else { split_x };
            let combo_w = if side == 0 { 592.0 } else { 542.0 };
            let combo = egui::Rect::from_min_size(
                egui::pos2(origin + 1.0, bar.top() + 1.0),
                egui::vec2(combo_w, 25.0),
            );
            painter.rect_filled(combo, 0.0, egui::Color32::WHITE);
            painter.rect_stroke(
                combo,
                0.0,
                egui::Stroke::new(1.0, COMBO_EDGE),
                egui::StrokeKind::Inside,
            );
            let edit_rect = egui::Rect::from_min_max(
                combo.min + egui::vec2(1.0, 1.0),
                egui::pos2(combo.right() - 15.0, combo.bottom() - 1.0),
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
                    let edit = ui.add(
                        egui::TextEdit::singleline(text)
                            .frame(egui::Frame::NONE)
                            .font(egui::FontId::proportional(crate::chrome::theme::UI_SIZE))
                            .margin(egui::vec2(2.0, 5.0)),
                    );
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
                egui::pos2(combo.right() - 15.0, combo.top()),
                egui::vec2(14.0, combo.height()),
            );
            icons::dropdown_triangle(
                &painter,
                chevron.center(),
                egui::Color32::from_rgb(110, 110, 110),
            );
            let chev = ui.interact(chevron, edit_id.with("chevron"), egui::Sense::click());
            let mut picked = None;
            egui::Popup::from_toggle_button_response(&chev)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    if self.paths.recent.is_empty() {
                        ui.add_enabled(false, egui::Label::new("No recent folders"));
                    }
                    for entry in &self.paths.recent {
                        if ui.selectable_label(false, entry).clicked() {
                            picked = Some(entry.clone());
                            ui.close();
                        }
                    }
                });
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
                    painter.rect_filled(slot, 0.0, crate::chrome::theme::HOVER_BLUE);
                    painter.rect_stroke(
                        slot,
                        0.0,
                        egui::Stroke::new(1.0, crate::chrome::theme::CLASSIC_BLUE),
                        egui::StrokeKind::Inside,
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
                            egui::Color32::BLACK
                        } else {
                            egui::Color32::from_rgb(150, 150, 150)
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
                        "browse" => self.paths.open = true,
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
    fn draw_grid(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let (left_w, right_w) = Self::chrome_split(rect.width());
        let painter = ui.painter().clone();
        let left = egui::Rect::from_min_size(rect.min, egui::vec2(left_w, rect.height()));
        let right = egui::Rect::from_min_size(
            egui::pos2(rect.left() + left_w + CHROME_GAP, rect.top()),
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
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(header.right() - 1.0, header.top() + 2.0),
                    egui::vec2(1.0, CHROME_HEADER - 2.0),
                ),
                0.0,
                HEADER_EDGE,
            );
            if side == 1 {
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
        if let Some((column, side)) = sort {
            let reverse = self.sort.0 == column && self.sort.1 == side && !self.sort.2;
            self.sort = (column, side, reverse);
            self.send(Command::Sort(column, side, reverse));
        }
        // 列头顶部两条高光线全宽贯通（参考 y141 #ECECEC / y142 #FEFEFE，覆盖中缝与滚动条槽）。
        painter.rect_filled(
            egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), 1.0)),
            0.0,
            egui::Color32::from_rgb(236, 236, 236),
        );
        painter.rect_filled(
            egui::Rect::from_min_size(
                rect.min + egui::vec2(0.0, 1.0),
                egui::vec2(rect.width(), 1.0),
            ),
            0.0,
            egui::Color32::from_rgb(254, 254, 254),
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
            egui::Color32::from_rgb(236, 236, 236),
        );
        painter.rect_filled(
            egui::Rect::from_min_size(
                rect.min + egui::vec2(1.0, 0.0),
                egui::vec2(1.0, rect.height()),
            ),
            0.0,
            egui::Color32::from_rgb(254, 254, 254),
        );
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(left.right() - 1.0, rows_area.top()),
                egui::vec2(1.0, content_bottom - rows_area.top()),
            ),
            0.0,
            TREE_EDGE,
        );
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(right.left(), rows_area.top()),
                egui::vec2(1.0, content_bottom - rows_area.top()),
            ),
            0.0,
            TREE_EDGE,
        );
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
        response.context_menu(|ui| {
            if let Some(pos) = response.interact_pointer_pos() {
                let index =
                    ((pos.y - rows_area.top() + self.scroll_y) / CHROME_ROW).floor() as usize;
                if index < total {
                    if ui.button("Select row").clicked() {
                        context_command = Some((index, 0));
                        ui.close();
                    }
                    if ui.button("Open file pair").clicked() {
                        context_command = Some((index, 1));
                        ui.close();
                    }
                    let is_dir = self.snapshot.rows[index].directory;
                    if is_dir && ui.button("Expand / collapse").clicked() {
                        context_command = Some((index, 2));
                        ui.close();
                    }
                }
            }
        });
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
            painter.rect_filled(thumb, 0.0, egui::Color32::from_rgb(192, 192, 192));
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
                egui::Color32::from_rgb(133, 133, 133),
            );
        }
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(scroll.right() - 2.0, rect.top()),
                egui::vec2(1.0, rect.height()),
            ),
            0.0,
            egui::Color32::from_rgb(247, 247, 247),
        );
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(scroll.right() - 1.0, rect.top()),
                egui::vec2(1.0, rect.height()),
            ),
            0.0,
            egui::Color32::from_rgb(230, 230, 230),
        );
    }
    /// 日志区（106 px）：左侧齿轮/保存图标，#E3E7ED 文本框，右侧面板灰条。
    fn draw_log(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let painter = ui.painter().clone();
        painter.rect_filled(rect, 0.0, crate::chrome::theme::PANEL_BG);
        painter.hline(
            rect.left()..=rect.right(),
            rect.top(),
            egui::Stroke::new(1.0, egui::Color32::from_rgb(254, 254, 254)),
        );
        painter.hline(
            rect.left()..=rect.right(),
            rect.top() + 1.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(131, 131, 131)),
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
            egui::Stroke::new(1.0, egui::Color32::from_rgb(236, 236, 236)),
            egui::StrokeKind::Inside,
        );
        // 底部边框行（参考 y934 #FEFEFE / y935 #838383）。
        painter.hline(
            rect.left()..=rect.right(),
            rect.bottom() - 2.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(254, 254, 254)),
        );
        painter.hline(
            rect.left()..=rect.right(),
            rect.bottom() - 1.0,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(131, 131, 131)),
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
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(192, 192, 192)),
                );
            }
        }
        if self.log_save_open {
            let mut save = None;
            egui::Window::new("Save log")
                .open(&mut self.log_save_open)
                .show(ui.ctx(), |ui| {
                    ui.label("Log file path");
                    ui.text_edit_singleline(&mut self.log_panel.save_path);
                    if ui.button("Save log").clicked() && !self.log_panel.save_path.is_empty() {
                        save = Some(self.log_panel.save_path.clone());
                    }
                });
            if let Some(path) = save {
                self.send(Command::SaveLog(path.into()));
            }
        }
    }
    /// 状态栏（25 px）：空闲显示 “Shows ...” 图例；纯文件选择时显示每侧文件数/字节与剩余空间。
    fn draw_status(&mut self, ui: &mut egui::Ui, rect: egui::Rect, busy: bool) {
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
            if any && only_files {
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
        // 参考几何：1384x961 下首行选中填充 y165、行高 20。
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
        let selected_fill = egui::Color32::from_rgb(168, 205, 241);
        let row0 = output.shapes.iter().any(|s| {
            matches!(&s.shape, egui::Shape::Rect(r)
                if r.fill == selected_fill
                    && (r.rect.top() - 165.0).abs() < 1.0
                    && (r.rect.height() - 20.0).abs() < 1.0)
        });
        assert!(row0, "selected first row at reference y=165 h=20");
        output.textures_delta.clear();
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
