//! Folder view: worker-owned trees, latest-only immutable snapshots and virtual rows.
//! The UI never enumerates directories, compares contents or executes file operations.
use crate::{
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
        self.scan_one(0)?;
        self.apply_filters()?;
        self.publish(false);
        self.clock = system_clock().unwrap_or_else(|e| {
            self.shared.notify(Notice::Message(format!(
                "Local clock unavailable: {e}; UTC is shown explicitly"
            )));
            utc_clock()
        });
        if self.scan.automatic {
            self.scan_descendants(0, false)?;
        }
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
        if mode == ContentCompare::RulesBased {
            return Err("Rules comparison is unavailable until text-engine integration; quick results are not content results".into());
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
                let cells = [
                    entry.size.map_or("—".into(), |s| s.to_string()),
                    entry.modified.map_or("—".into(), time_label),
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
                center,
                conflict,
            });
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
fn system_clock() -> Result<FilterClock, String> {
    #[cfg(windows)]
    {
        let output=std::process::Command::new("powershell").args(["-NoProfile","-Command","$m=[DateTimeOffset]::new([DateTime]::Today); Write-Output ($m.ToUnixTimeSeconds().ToString() + ';' + ([int][DateTimeOffset]::Now.Offset.TotalSeconds).ToString())"]).output().map_err(|e|e.to_string())?;
        if !output.status.success() {
            return Err("local time query failed".into());
        }
        let text = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
        let (midnight, offset) = text.trim().split_once(';').ok_or("invalid clock result")?;
        Ok(FilterClock {
            today_midnight: UNIX_EPOCH
                + Duration::from_secs(midnight.parse().map_err(|_| "invalid midnight")?),
            utc_offset_seconds: offset.parse().map_err(|_| "invalid offset")?,
        })
    }
    #[cfg(not(windows))]
    {
        Ok(utc_clock())
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
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}",
        seconds % 86400 / 3600,
        seconds % 3600 / 60,
        seconds % 60
    )
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
        Self {
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
        }
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
        }
        let notices: Vec<_> = self.shared.notices.lock().unwrap().drain(..).collect();
        for notice in notices {
            match notice {
                Notice::Message(text) => self.message = text,
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
                (ContentCompare::RulesBased, "Rules (unavailable)"),
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
        egui::Window::new("Folder comparison rules").open(&mut open).show(ctx,|ui|{
            ui.label("criteria arguments");ui.add(egui::TextEdit::singleline(&mut self.criteria_text).desired_width(550.0));
            ui.small("Examples: timestamp:2sec;IgnoreDST size / CRC / binary / skip-quick-same. Rules-based remains Unsupported until merge.");
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
        ui.add_enabled_ui(!busy && self.child.is_none(), |ui| {
            switch = self.toolbar(ui).map(|paths| (!self.sync_mode, paths));
            let change = self.filters.ui(ui, !self.sync_mode);
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
impl Drop for DirCompareView {
    fn drop(&mut self) {
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
}
