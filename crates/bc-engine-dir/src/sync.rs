//! Snapshot sync previews, explicit overrides and the operation-plan execution bridge.
use crate::{
    op_exec::{ConfirmPolicy, OpLog, OpOutcome, OpState},
    ops::*,
    tree::NodeId,
    CompareStatus, DirTree,
};
use bc_jobs::JobContext;
use bc_vfs::{FileAttrs, VfsEntry, VfsManager};
use std::{collections::BTreeMap, time::SystemTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyncDirection {
    LeftToRight,
    RightToLeft,
    Both,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SyncChoice {
    #[default]
    LeaveAlone,
    CopyLeftToRight,
    CopyRightToLeft,
    DeleteLeft,
    DeleteRight,
    Exclude,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CustomSyncRules {
    pub left_newer: SyncChoice,
    pub right_newer: SyncChoice,
    pub left_orphan: SyncChoice,
    pub right_orphan: SyncChoice,
    pub different: SyncChoice,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyncMode {
    Update(SyncDirection),
    Mirror(SyncDirection),
    Custom(CustomSyncRules),
}
impl Default for SyncMode {
    fn default() -> Self {
        Self::Update(SyncDirection::LeftToRight)
    }
}
#[derive(Debug, Clone)]
pub struct SyncOptions {
    pub create_empty: bool,
    pub visible_only: bool,
    pub operation: PlanOptions,
    pub recycle_bin: bool,
}
impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            create_empty: false,
            visible_only: false,
            operation: PlanOptions::default(),
            recycle_bin: true,
        }
    }
}
/// Metadata, not a content-history database. Existing CRCs improve the fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncFingerprint {
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
    pub attrs: FileAttrs,
    pub crc: Option<u32>,
}
impl From<&VfsEntry> for SyncFingerprint {
    fn from(e: &VfsEntry) -> Self {
        Self {
            size: e.size,
            modified: e.modified,
            attrs: e.attrs,
            crc: e.crc32,
        }
    }
}
#[derive(Debug, Clone, Default)]
pub struct SyncBaseline {
    pub entries: BTreeMap<String, [Option<SyncFingerprint>; 2]>,
}
impl SyncBaseline {
    pub fn capture(tree: &DirTree) -> Self {
        Self {
            entries: tree
                .nodes()
                .filter(|(_, n)| !n.is_dir)
                .map(|(_, n)| {
                    (
                        n.relative.clone(),
                        std::array::from_fn(|s| n.sides[s].as_ref().map(SyncFingerprint::from)),
                    )
                })
                .collect(),
        }
    }
    fn both_changed(&self, relative: &str, entries: &[Option<VfsEntry>; 2]) -> bool {
        self.entries.get(relative).is_some_and(|old| {
            (0..2).all(|s| match (&old[s], entries[s].as_ref()) {
                (None, None) => false,
                (Some(old), Some(now)) => {
                    old.size != now.size
                        || old.modified != now.modified
                        || old.attrs != now.attrs
                        || old.crc.zip(now.crc32).is_some_and(|(a, b)| a != b)
                }
                _ => true,
            })
        })
    }
}
#[derive(Debug, Clone)]
pub struct SyncItem {
    pub node: NodeId,
    pub revision: u64,
    pub relative: String,
    pub is_dir: bool,
    pub status: CompareStatus,
    pub sides: [Option<VfsEntry>; 2],
    pub locations: [Location; 2],
    pub ancestors: Vec<NodeId>,
    pub choice: SyncChoice,
    pub override_choice: Option<SyncChoice>,
    pub conflict: bool,
    pub reason: String,
    allowed: [bool; 2],
}
impl SyncItem {
    pub fn effective(&self) -> SyncChoice {
        self.override_choice.unwrap_or(self.choice)
    }
    pub fn unresolved_conflict(&self) -> bool {
        self.conflict && self.override_choice.is_none() && self.effective() != SyncChoice::Exclude
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FolderActions {
    pub copy_left_to_right: bool,
    pub copy_right_to_left: bool,
    pub delete_left: bool,
    pub delete_right: bool,
    pub conflict: bool,
}
impl FolderActions {
    fn add(&mut self, item: &SyncItem) {
        match item.effective() {
            SyncChoice::CopyLeftToRight => self.copy_left_to_right = true,
            SyncChoice::CopyRightToLeft => self.copy_right_to_left = true,
            SyncChoice::DeleteLeft => self.delete_left = true,
            SyncChoice::DeleteRight => self.delete_right = true,
            _ => {}
        }
        self.conflict |= item.unresolved_conflict();
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncStatistics {
    pub copies: usize,
    pub deletes: usize,
    pub creates: usize,
    pub skipped: usize,
    pub failed: usize,
    pub conflicts: usize,
}
#[derive(Debug, Clone)]
pub struct SyncPlan {
    pub mode: SyncMode,
    pub options: SyncOptions,
    pub items: Vec<SyncItem>,
    pub folder_actions: BTreeMap<NodeId, FolderActions>,
}
#[derive(Debug, Clone)]
pub struct SyncRunResult {
    pub operations: OpOutcome,
    pub statistics: SyncStatistics,
    pub refresh: crate::refresh::RefreshStats,
    pub refresh_error: Option<String>,
}
fn choice_for(mode: SyncMode, status: CompareStatus) -> SyncChoice {
    use CompareStatus::*;
    use SyncChoice::*;
    use SyncDirection::*;
    match mode {
        SyncMode::Update(LeftToRight) => match status {
            LeftNewer | LeftOrphan => CopyLeftToRight,
            _ => LeaveAlone,
        },
        SyncMode::Update(RightToLeft) => match status {
            RightNewer | RightOrphan => CopyRightToLeft,
            _ => LeaveAlone,
        },
        SyncMode::Update(Both) => match status {
            LeftNewer | LeftOrphan => CopyLeftToRight,
            RightNewer | RightOrphan => CopyRightToLeft,
            _ => LeaveAlone,
        },
        SyncMode::Mirror(LeftToRight) => match status {
            RightOrphan => DeleteRight,
            Same | Unknown | Error => LeaveAlone,
            _ => CopyLeftToRight,
        },
        SyncMode::Mirror(RightToLeft) => match status {
            LeftOrphan => DeleteLeft,
            Same | Unknown | Error => LeaveAlone,
            _ => CopyRightToLeft,
        },
        SyncMode::Mirror(Both) => LeaveAlone, // rejected before planning
        SyncMode::Custom(rules) => match status {
            LeftNewer => rules.left_newer,
            RightNewer => rules.right_newer,
            LeftOrphan => rules.left_orphan,
            RightOrphan => rules.right_orphan,
            Different | SimilarUnimportant => rules.different,
            _ => LeaveAlone,
        },
    }
}
fn required_side(choice: SyncChoice) -> Option<usize> {
    match choice {
        SyncChoice::CopyLeftToRight | SyncChoice::DeleteLeft => Some(0),
        SyncChoice::CopyRightToLeft | SyncChoice::DeleteRight => Some(1),
        _ => None,
    }
}
impl SyncPlan {
    pub fn build(
        tree: &DirTree,
        mode: SyncMode,
        create_empty: bool,
        visible_only: bool,
    ) -> Result<Self, PlanError> {
        Self::build_with(
            tree,
            mode,
            SyncOptions {
                create_empty,
                visible_only,
                recycle_bin: true,
                ..Default::default()
            },
            None,
        )
    }
    pub fn build_with(
        tree: &DirTree,
        mode: SyncMode,
        options: SyncOptions,
        baseline: Option<&SyncBaseline>,
    ) -> Result<Self, PlanError> {
        if mode == SyncMode::Mirror(SyncDirection::Both) {
            return Err(PlanError::Invalid("mirror:all is invalid".into()));
        }
        if options.operation.copy_permissions {
            return Err(PlanError::Invalid(
                "NTFS permission copying is Unsupported".into(),
            ));
        }
        let mut items = Vec::new();
        for (id, node) in tree.nodes().filter(|(id, _)| *id != 0) {
            let mut allowed = std::array::from_fn(|s| {
                node.sides[s].is_some()
                    && (options.operation.include_hidden
                        || options.operation.view.suppress_filters
                        || (!node.excluded && !node.filtered[s]))
            });
            if options.visible_only
                && !options.operation.view.suppress_filters
                && !options
                    .operation
                    .view
                    .display
                    .shows_with_importance(node.status, options.operation.view.ignore_unimportant)
            {
                allowed = [false; 2];
            }
            // Sync previews depend on the entire participating snapshot, not current expansion.
            for (side, entry) in node.sides.iter().take(2).enumerate() {
                if allowed[side]
                    && entry
                        .as_ref()
                        .is_some_and(|e| e.kind == bc_vfs::EntryKind::Dir)
                    && !node.loaded[side]
                {
                    return Err(PlanError::ScanRequired { node: id, side });
                }
            }
            let sides = [node.sides[0].clone(), node.sides[1].clone()];
            let unreadable = sides.iter().flatten().any(|e| e.error.is_some());
            let status = if unreadable {
                CompareStatus::Error
            } else if options.operation.view.ignore_unimportant
                && node.status == CompareStatus::SimilarUnimportant
            {
                CompareStatus::Same
            } else {
                node.status
            };
            let conflict = mode == SyncMode::Update(SyncDirection::Both)
                && !node.is_dir
                && allowed.iter().any(|v| *v)
                && (baseline.is_some_and(|b| b.both_changed(&node.relative, &sides))
                    || matches!(
                        status,
                        CompareStatus::Different | CompareStatus::SimilarUnimportant
                    ));
            let mut choice = if conflict {
                SyncChoice::LeaveAlone
            } else {
                choice_for(mode, status)
            };
            let mut reason = if unreadable {
                "unreadable or cyclic entry".into()
            } else if conflict {
                "both sides changed or no unambiguous direction".into()
            } else {
                String::new()
            };
            if node.is_dir {
                // Folder rows aggregate descendants. Only explicit empty-folder creation
                // and mirror orphan cleanup add an operation on the container itself.
                if matches!(
                    choice,
                    SyncChoice::CopyLeftToRight | SyncChoice::CopyRightToLeft
                ) && (!options.create_empty || !node.is_orphan())
                {
                    choice = SyncChoice::LeaveAlone;
                }
            }
            if let Some(side) = required_side(choice) {
                if sides[side].is_none() {
                    return Err(PlanError::Invalid(format!(
                        "sync source missing: {}",
                        node.relative
                    )));
                }
                if !allowed[side] {
                    choice = SyncChoice::LeaveAlone;
                    reason = "filtered or not visible".into();
                }
            }
            let mut ancestors = Vec::new();
            let mut parent = node.parent;
            while let Some(p) = parent {
                ancestors.push(p);
                parent = tree.node(p).and_then(|n| n.parent);
            }
            items.push(SyncItem {
                node: id,
                revision: node.revision,
                relative: node.relative.clone(),
                is_dir: node.is_dir,
                status,
                sides,
                locations: [location(tree, id, 0)?, location(tree, id, 1)?],
                ancestors,
                choice,
                override_choice: None,
                conflict,
                reason,
                allowed,
            });
        }
        // Nonempty orphan parents are required by selected file copies even when
        // create-empty is off. Plan them explicitly so their metadata is preserved.
        let needed: Vec<_> = items
            .iter()
            .filter(|i| !i.is_dir)
            .filter_map(|i| match i.choice {
                SyncChoice::CopyLeftToRight | SyncChoice::CopyRightToLeft => {
                    Some((i.ancestors.clone(), i.choice))
                }
                _ => None,
            })
            .collect();
        for item in &mut items {
            if item.is_dir
                && item.choice == SyncChoice::LeaveAlone
                && matches!(
                    item.status,
                    CompareStatus::LeftOrphan | CompareStatus::RightOrphan
                )
            {
                if let Some((_, choice)) = needed
                    .iter()
                    .find(|(ancestors, _)| ancestors.contains(&item.node))
                {
                    if let Some(side) = required_side(*choice) {
                        if item.allowed[side] {
                            item.choice = *choice;
                        }
                    }
                }
            }
        }
        items.sort_by(|a, b| a.relative.cmp(&b.relative));
        let mut plan = Self {
            mode,
            options,
            items,
            folder_actions: BTreeMap::new(),
        };
        plan.rebuild_summaries();
        Ok(plan)
    }
    pub fn set_override(&mut self, node: NodeId, choice: SyncChoice) -> Result<(), PlanError> {
        let root = self
            .items
            .iter()
            .find(|i| i.node == node)
            .ok_or_else(|| PlanError::Invalid("sync node missing".into()))?;
        let directory = root.is_dir;
        let indices: Vec<_> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.node == node || (directory && i.ancestors.contains(&node)))
            .map(|(i, _)| i)
            .collect();
        // Validate the whole override before updating any row.
        for &index in &indices {
            let item = &self.items[index];
            if let Some(side) = required_side(choice) {
                if item.sides[side].is_none() {
                    return Err(PlanError::Invalid(format!(
                        "override source missing: {}",
                        item.relative
                    )));
                }
                if item.sides[side].as_ref().is_some_and(|e| e.error.is_some()) {
                    return Err(PlanError::Invalid(
                        "unreadable source cannot be overridden; refresh first".into(),
                    ));
                }
                if !item.allowed[side] {
                    return Err(PlanError::Invalid(
                        "override requires rebuilding with include-hidden/visible settings".into(),
                    ));
                }
            }
        }
        for index in indices {
            self.items[index].override_choice = Some(choice);
        }
        self.rebuild_summaries();
        Ok(())
    }
    pub fn exclude(&mut self, node: NodeId) -> Result<(), PlanError> {
        self.set_override(node, SyncChoice::Exclude)
    }
    pub fn clear_override(&mut self, node: NodeId) -> Result<(), PlanError> {
        let directory = self
            .items
            .iter()
            .find(|i| i.node == node)
            .ok_or_else(|| PlanError::Invalid("sync node missing".into()))?
            .is_dir;
        for item in &mut self.items {
            if item.node == node || (directory && item.ancestors.contains(&node)) {
                item.override_choice = None;
            }
        }
        self.rebuild_summaries();
        Ok(())
    }
    pub fn next_conflict(&self, after: Option<usize>) -> Option<usize> {
        self.items
            .iter()
            .enumerate()
            .find(|(i, item)| after.is_none_or(|a| *i > a) && item.unresolved_conflict())
            .map(|(i, _)| i)
    }
    pub fn previous_conflict(&self, before: Option<usize>) -> Option<usize> {
        self.items
            .iter()
            .enumerate()
            .rev()
            .find(|(i, item)| before.is_none_or(|a| *i < a) && item.unresolved_conflict())
            .map(|(i, _)| i)
    }
    fn rebuild_summaries(&mut self) {
        self.folder_actions.clear();
        for item in &self.items {
            for &ancestor in &item.ancestors {
                self.folder_actions.entry(ancestor).or_default().add(item);
            }
            if item.is_dir {
                self.folder_actions.entry(item.node).or_default().add(item);
            }
        }
    }
    pub fn statistics(&self) -> SyncStatistics {
        let mut stats = SyncStatistics::default();
        for item in &self.items {
            match item.effective() {
                SyncChoice::CopyLeftToRight | SyncChoice::CopyRightToLeft => {
                    if item.is_dir {
                        stats.creates += 1
                    } else {
                        stats.copies += 1
                    }
                }
                SyncChoice::DeleteLeft | SyncChoice::DeleteRight => stats.deletes += 1,
                _ => stats.skipped += 1,
            }
            stats.conflicts += usize::from(item.unresolved_conflict());
        }
        stats
    }
    pub fn summary(&self) -> (usize, usize, usize, usize) {
        let s = self.statistics();
        (s.copies, s.deletes, s.creates, s.skipped)
    }
    pub fn to_op_plan(&self) -> Result<OpPlan, PlanError> {
        let mut plan = OpPlan {
            items: Vec::new(),
            options: self.options.operation.clone(),
            hidden_skipped: 0,
        };
        for row in &self.items {
            let choice = row.effective();
            let Some(side) = required_side(choice) else {
                continue;
            };
            let copy = matches!(
                choice,
                SyncChoice::CopyLeftToRight | SyncChoice::CopyRightToLeft
            );
            let destination = copy.then(|| row.locations[1 - side].clone());
            let action = if copy {
                if row.is_dir {
                    OpAction::EnsureDir
                } else {
                    OpAction::Copy
                }
            } else if row.is_dir {
                OpAction::RemoveEmptyDir {
                    recycle: self.options.recycle_bin,
                }
            } else {
                OpAction::Delete {
                    recycle: self.options.recycle_bin,
                }
            };
            let mut item = OpItem {
                node: row.node,
                revision: row.revision,
                action,
                source: Some(row.locations[side].clone()),
                destination,
                source_entry: row.sides[side].clone(),
                target_entry: if copy {
                    row.sides[1 - side].clone()
                } else {
                    None
                },
                target_known: true,
                confirmations: Vec::new(),
            };
            item.confirmations = crate::ops::confirmations(&item);
            plan.items.push(item.clone());
            if copy
                && row.is_dir
                && (plan.options.copy.timestamps
                    || plan.options.copy.attributes
                    || plan.options.copy.created)
            {
                item.action = OpAction::PreserveDir;
                item.target_known = false;
                item.confirmations.clear();
                plan.items.push(item);
            }
        }
        plan.finish()?;
        Ok(plan)
    }
    /// Executes exactly the preview snapshot and refreshes affected entries, retaining
    /// outcomes even when cancellation or refresh errors occur. Does not advance baseline.
    pub fn execute_and_refresh(
        &self,
        tree: &mut DirTree,
        vfs: &VfsManager,
        ctx: &JobContext,
        policy: ConfirmPolicy,
        log: &OpLog,
    ) -> Result<SyncRunResult, PlanError> {
        let plan = self.to_op_plan()?;
        let start = log.snapshot().len();
        let operations = crate::op_exec::execute(&plan, vfs, ctx, policy, log);
        let preview = self.statistics();
        let mut statistics = SyncStatistics {
            skipped: preview.skipped + operations.skipped,
            failed: operations.failed,
            conflicts: preview.conflicts,
            ..Default::default()
        };
        for event in log
            .snapshot()
            .into_iter()
            .skip(start)
            .filter(|e| e.state == OpState::Completed)
        {
            match event.action {
                OpAction::Copy => statistics.copies += 1,
                OpAction::Delete { .. } | OpAction::RemoveEmptyDir { .. } => {
                    statistics.deletes += 1
                }
                OpAction::EnsureDir => statistics.creates += 1,
                _ => {}
            }
        }
        if operations.cancelled {
            statistics.skipped +=
                plan.items.len() - operations.completed - operations.skipped - operations.failed;
        }
        let refreshed = crate::refresh::refresh_locations(
            tree,
            vfs,
            &operations.affected,
            &bc_core::cancel::NeverCancel,
        );
        let (refresh, refresh_error) = match refreshed {
            Ok(stats) => (stats, None),
            Err(e) => (Default::default(), Some(e.to_string())),
        };
        Ok(SyncRunResult {
            operations,
            statistics,
            refresh,
            refresh_error,
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncCommand {
    pub mode: SyncMode,
    pub visible_only: bool,
    pub create_empty: bool,
}
impl SyncCommand {
    pub fn parse_script(input: &str) -> Result<Self, PlanError> {
        let words = crate::filter_script::words(input).map_err(PlanError::Invalid)?;
        let mut mode = None;
        let mut visible_only = false;
        let mut create_empty = false;
        for word in words.iter().skip(usize::from(
            words
                .first()
                .is_some_and(|w| w.eq_ignore_ascii_case("sync")),
        )) {
            let lower = word.to_ascii_lowercase();
            match lower.as_str() {
                "visible" => visible_only = true,
                "create-empty" => create_empty = true,
                _ => {
                    let (kind, direction) = lower
                        .split_once(':')
                        .ok_or_else(|| PlanError::Invalid("invalid sync argument".into()))?;
                    let direction = match direction {
                        "left->right" => SyncDirection::LeftToRight,
                        "right->left" => SyncDirection::RightToLeft,
                        "all" => SyncDirection::Both,
                        _ => return Err(PlanError::Invalid("invalid sync direction".into())),
                    };
                    let value = match kind {
                        "update" => SyncMode::Update(direction),
                        "mirror" if direction != SyncDirection::Both => SyncMode::Mirror(direction),
                        _ => {
                            return Err(PlanError::Invalid(
                                "invalid sync mode (mirror:all is forbidden)".into(),
                            ))
                        }
                    };
                    if mode.replace(value).is_some() {
                        return Err(PlanError::Invalid("multiple sync modes".into()));
                    }
                }
            }
        }
        Ok(Self {
            mode: mode.ok_or_else(|| PlanError::Invalid("sync mode missing".into()))?,
            visible_only,
            create_empty,
        })
    }
    pub fn to_script(self) -> Result<String, PlanError> {
        if self.mode == SyncMode::Mirror(SyncDirection::Both) {
            return Err(PlanError::Invalid("mirror:all is invalid".into()));
        }
        let (kind, direction) = match self.mode {
            SyncMode::Update(d) => ("update", d),
            SyncMode::Mirror(d) => ("mirror", d),
            SyncMode::Custom(_) => {
                return Err(PlanError::Invalid(
                    "custom sync rules have no script spelling".into(),
                ))
            }
        };
        Ok(format!(
            "sync {}{}{kind}:{}",
            if self.visible_only { "visible " } else { "" },
            if self.create_empty {
                "create-empty "
            } else {
                ""
            },
            match direction {
                SyncDirection::LeftToRight => "left->right",
                SyncDirection::RightToLeft => "right->left",
                SyncDirection::Both => "all",
            }
        ))
    }
}
