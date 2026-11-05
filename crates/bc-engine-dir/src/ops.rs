//! Pure, reviewable file-operation plans. Traversal and filtering happen before I/O.
use crate::{tree::NodeId, view::ViewOptions, DirTree};
use bc_core::{pattern::Pattern, Side};
use bc_vfs::{CopyOptions, EntryKind, FileAttrs, VfsEntry, VfsPath};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::SystemTime,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PathMode {
    #[default]
    None,
    Relative,
    Base,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Target {
    Left,
    Right,
    #[default]
    All,
}
impl Target {
    pub fn contains(self, side: usize) -> bool {
        matches!(
            (self, side),
            (Self::All, 0 | 1) | (Self::Left, 0) | (Self::Right, 1)
        )
    }
}
pub fn side_index(side: Side) -> Result<usize, PlanError> {
    match side {
        Side::Left => Ok(0),
        Side::Right => Ok(1),
        _ => Err(PlanError::Invalid("two-sided operation required".into())),
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameSpec {
    Dos { new_mask: String },
    Regex { old_mask: String, new_mask: String },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TouchSpec {
    Copy {
        from: Side,
        to: Side,
    },
    Set {
        target: Target,
        when: Option<SystemTime>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirOp {
    Copy {
        from: Side,
        to: Side,
    },
    Move {
        from: Side,
        to: Side,
    },
    CopyOther,
    MoveOther,
    Exchange,
    CopyTo {
        target: Target,
        path_mode: PathMode,
        dest: String,
    },
    MoveTo {
        target: Target,
        path_mode: PathMode,
        dest: String,
    },
    Delete {
        target: Target,
        recycle_bin: bool,
    },
    Rename(RenameSpec),
    Touch(TouchSpec),
    Attrib {
        set: FileAttrs,
        clear: FileAttrs,
    },
    Exclude,
    Ignored(bool),
    NewFolder {
        target: Target,
        name: String,
    },
    CopyFilename,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selection {
    pub items: BTreeSet<(NodeId, usize)>,
}
impl Selection {
    pub fn insert(&mut self, node: NodeId, side: usize) {
        if side < 2 {
            self.items.insert((node, side));
        }
    }
    pub fn both(nodes: impl IntoIterator<Item = NodeId>) -> Self {
        let mut selected = Self::default();
        for node in nodes {
            selected.insert(node, 0);
            selected.insert(node, 1);
        }
        selected
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    ScanRequired { node: NodeId, side: usize },
    Invalid(String),
    Collision(String),
}
impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ScanRequired { node, side } => {
                write!(f, "scan required for node {node}, side {side}")
            }
            Self::Invalid(s) => f.write_str(s),
            Self::Collision(s) => write!(f, "destination collision: {s}"),
        }
    }
}
impl std::error::Error for PlanError {}
#[derive(Debug, Clone, Default)]
pub struct PlanOptions {
    pub include_hidden: bool,
    pub view: ViewOptions,
    pub copy: CopyOptions,
    pub copy_permissions: bool,
}
impl PlanOptions {
    /// Per-confirmation include-hidden override wins over the session preference.
    pub fn from_handling(
        handling: &crate::rules::HandlingOptions,
        view: ViewOptions,
        include_hidden_override: Option<bool>,
    ) -> Self {
        Self {
            include_hidden: include_hidden_override.unwrap_or(handling.include_hidden),
            view,
            copy: CopyOptions {
                timestamps: handling.copy_timestamps,
                attributes: handling.copy_attributes,
                created: handling.copy_creation_time,
                overwrite: false,
            },
            copy_permissions: handling.copy_ntfs_permissions,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfirmKind {
    Overwrite,
    OverwriteNewer,
    ReadOnly,
    Move,
    Delete,
    Exchange,
    PermanentFallback,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub path: VfsPath,
    pub side: Option<usize>,
    pub relative: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpAction {
    Copy,
    Move,
    Rename,
    Exchange,
    Delete { recycle: bool },
    EnsureDir,
    PreserveDir,
    RemoveEmptyDir { recycle: bool },
    Touch(SystemTime),
    Attrib { set: FileAttrs, clear: FileAttrs },
    Exclude,
    Ignored(bool),
    CopyFilename,
}
#[derive(Debug, Clone)]
pub struct OpItem {
    pub node: NodeId,
    pub revision: u64,
    pub action: OpAction,
    pub source: Option<Location>,
    pub destination: Option<Location>,
    pub source_entry: Option<VfsEntry>,
    pub target_entry: Option<VfsEntry>,
    pub target_known: bool,
    pub confirmations: Vec<ConfirmKind>,
}
impl OpItem {
    pub fn bytes(&self) -> u64 {
        self.source_entry.as_ref().and_then(|e| e.size).unwrap_or(0)
            + if self.action == OpAction::Exchange {
                self.target_entry.as_ref().and_then(|e| e.size).unwrap_or(0)
            } else {
                0
            }
    }
}
#[derive(Debug, Clone)]
pub struct OpPlan {
    pub items: Vec<OpItem>,
    pub options: PlanOptions,
    pub hidden_skipped: usize,
}
impl OpPlan {
    pub fn build(
        tree: &DirTree,
        selection: &Selection,
        op: &DirOp,
        options: PlanOptions,
        now: SystemTime,
    ) -> Result<Self, PlanError> {
        if options.copy_permissions {
            return Err(PlanError::Invalid(
                "NTFS permission copying is Unsupported".into(),
            ));
        }
        let mut plan = Self {
            items: Vec::new(),
            options,
            hidden_skipped: 0,
        };
        let mut selection = selection.clone();
        if matches!(op, DirOp::Exchange) {
            for (id, side) in selection.items.clone() {
                selection.insert(id, 1 - side);
            }
        }
        let recursive = matches!(
            op,
            DirOp::Copy { .. }
                | DirOp::Move { .. }
                | DirOp::CopyOther
                | DirOp::MoveOther
                | DirOp::CopyTo { .. }
                | DirOp::MoveTo { .. }
                | DirOp::Delete { .. }
                | DirOp::Exchange
                | DirOp::Attrib { .. }
                | DirOp::Touch(_)
                | DirOp::Ignored(_)
        );
        let mut selected = BTreeSet::new();
        for &(id, side) in &selection.items {
            let active = match op {
                DirOp::Copy { from, .. } | DirOp::Move { from, .. } => side == side_index(*from)?,
                DirOp::CopyTo { target, .. }
                | DirOp::MoveTo { target, .. }
                | DirOp::Delete { target, .. }
                | DirOp::NewFolder { target, .. } => target.contains(side),
                DirOp::Touch(TouchSpec::Set { target, .. }) => target.contains(side),
                DirOp::Touch(TouchSpec::Copy { to, .. }) => side == side_index(*to)?,
                _ => true,
            };
            if !active {
                continue;
            }
            let mut pending = vec![id];
            while let Some(id) = pending.pop() {
                let node = tree
                    .node(id)
                    .ok_or_else(|| PlanError::Invalid("stale selection".into()))?;
                if id == 0 && !matches!(op, DirOp::NewFolder { .. } | DirOp::CopyFilename) {
                    return Err(PlanError::Invalid(
                        "base folder cannot be a destructive selection".into(),
                    ));
                }
                if side > 1 || (node.sides[side].is_none() && id != 0) {
                    continue;
                }
                let hidden = node.excluded
                    || node.filtered[side]
                    || (!node.is_dir
                        && !plan.options.view.display.shows_with_importance(
                            node.status,
                            plan.options.view.ignore_unimportant,
                        ));
                if hidden && !plan.options.include_hidden && !plan.options.view.suppress_filters {
                    plan.hidden_skipped += 1;
                    continue;
                }
                if !selected.insert((id, side)) {
                    continue;
                }
                if recursive
                    && node.sides[side]
                        .as_ref()
                        .is_some_and(|e| e.kind == EntryKind::Dir)
                {
                    if !node.loaded[side] {
                        return Err(PlanError::ScanRequired { node: id, side });
                    }
                    pending.extend(node.children.iter().copied());
                }
            }
        }
        // The minimum relative structure is based on selected roots, not descendants.
        let roots: Vec<_> = selection
            .items
            .iter()
            .map(|&(id, s)| tree.side_relative(id, s))
            .collect();
        let prefix = common_parent(&roots);
        let mut exchange_done = BTreeSet::new();
        for (id, side) in selected {
            let node = tree.node(id).unwrap();
            let entry = node.sides[side].clone();
            let source = location(tree, id, side)?;
            let mut item = OpItem {
                node: id,
                revision: node.revision,
                action: OpAction::Copy,
                source: Some(source.clone()),
                destination: None,
                source_entry: entry.clone(),
                target_entry: None,
                target_known: true,
                confirmations: Vec::new(),
            };
            let is_dir = entry.as_ref().is_some_and(|e| e.kind == EntryKind::Dir);
            match op {
                DirOp::Copy { from, to } | DirOp::Move { from, to } => {
                    if side != side_index(*from)? {
                        continue;
                    }
                    let to = side_index(*to)?;
                    if side == to {
                        return Err(PlanError::Invalid("same-side transfer".into()));
                    }
                    item.destination = Some(location(tree, id, to)?);
                    item.target_entry = node.sides[to].clone();
                    item.action = if is_dir {
                        OpAction::EnsureDir
                    } else if matches!(op, DirOp::Move { .. }) {
                        OpAction::Move
                    } else {
                        OpAction::Copy
                    };
                }
                DirOp::CopyOther | DirOp::MoveOther => {
                    item.destination = Some(location(tree, id, 1 - side)?);
                    item.target_entry = node.sides[1 - side].clone();
                    item.action = if is_dir {
                        OpAction::EnsureDir
                    } else if matches!(op, DirOp::MoveOther) {
                        OpAction::Move
                    } else {
                        OpAction::Copy
                    };
                }
                DirOp::CopyTo {
                    target,
                    path_mode,
                    dest,
                }
                | DirOp::MoveTo {
                    target,
                    path_mode,
                    dest,
                } => {
                    if !target.contains(side) {
                        continue;
                    }
                    let base =
                        VfsPath::parse(dest).map_err(|e| PlanError::Invalid(e.to_string()))?;
                    let relative = match path_mode {
                        PathMode::Base => source.relative.clone(),
                        PathMode::Relative => source
                            .relative
                            .strip_prefix(&prefix)
                            .unwrap_or(&source.relative)
                            .to_owned(),
                        PathMode::None => {
                            // Keep descendants inside a selected folder, but discard the roots' parents.
                            let root = selection
                                .items
                                .iter()
                                .filter(|(_, s)| *s == side)
                                .map(|(root, _)| tree.side_relative(*root, side))
                                .filter(|root| {
                                    source.relative == *root
                                        || source.relative.starts_with(&format!("{root}/"))
                                })
                                .min_by_key(String::len)
                                .unwrap_or_else(|| source.relative.clone());
                            let parent = root.rsplit_once('/').map_or("", |(p, _)| p);
                            source
                                .relative
                                .strip_prefix(parent)
                                .unwrap_or(&source.relative)
                                .trim_start_matches('/')
                                .to_owned()
                        }
                    };
                    item.destination = Some(Location {
                        path: base.join(&relative),
                        side: None,
                        relative,
                    });
                    item.target_known = false;
                    item.action = if is_dir {
                        OpAction::EnsureDir
                    } else if matches!(op, DirOp::MoveTo { .. }) {
                        OpAction::Move
                    } else {
                        OpAction::Copy
                    };
                }
                DirOp::Exchange => {
                    if !exchange_done.insert(id) {
                        continue;
                    }
                    let to = 1 - side;
                    if node.sides[to].is_some()
                        && (node.filtered[to] || node.excluded)
                        && !plan.options.include_hidden
                        && !plan.options.view.suppress_filters
                    {
                        plan.hidden_skipped += 1;
                        continue;
                    }
                    if node.sides[to]
                        .as_ref()
                        .is_some_and(|e| e.kind == EntryKind::Dir)
                        && !node.loaded[to]
                    {
                        return Err(PlanError::ScanRequired { node: id, side: to });
                    }
                    item.destination = Some(location(tree, id, to)?);
                    item.target_entry = node.sides[to].clone();
                    item.action = if is_dir {
                        OpAction::EnsureDir
                    } else if item.target_entry.is_none() {
                        OpAction::Move
                    } else {
                        OpAction::Exchange
                    };
                    if item.action == OpAction::Exchange
                        && item
                            .target_entry
                            .as_ref()
                            .is_some_and(|e| e.kind == EntryKind::Dir)
                    {
                        return Err(PlanError::Invalid("cannot exchange file and folder".into()));
                    }
                }
                DirOp::Delete {
                    target,
                    recycle_bin,
                } => {
                    if !target.contains(side) {
                        continue;
                    }
                    item.action = if is_dir {
                        OpAction::RemoveEmptyDir {
                            recycle: *recycle_bin,
                        }
                    } else {
                        OpAction::Delete {
                            recycle: *recycle_bin,
                        }
                    };
                }
                DirOp::Rename(spec) => {
                    let name = rename_name(&entry.as_ref().unwrap().name, spec)?;
                    let parent = source
                        .path
                        .parent()
                        .ok_or_else(|| PlanError::Invalid("rename needs a parent".into()))?;
                    let relative = source
                        .relative
                        .rsplit_once('/')
                        .map_or_else(|| name.clone(), |(p, _)| format!("{p}/{name}"));
                    item.destination = Some(Location {
                        path: parent.join(&name),
                        side: Some(side),
                        relative,
                    });
                    item.target_known = false;
                    item.action = OpAction::Rename;
                }
                DirOp::Touch(TouchSpec::Copy { from, to }) => {
                    if side != side_index(*to)? {
                        continue;
                    }
                    let from = side_index(*from)?;
                    let when = node.sides[from]
                        .as_ref()
                        .and_then(|e| e.modified)
                        .ok_or_else(|| PlanError::Invalid("opposite timestamp missing".into()))?;
                    item.action = OpAction::Touch(when);
                }
                DirOp::Touch(TouchSpec::Set { target, when }) => {
                    if !target.contains(side) {
                        continue;
                    }
                    item.action = OpAction::Touch(when.unwrap_or(now));
                }
                DirOp::Attrib { set, clear } => {
                    item.action = OpAction::Attrib {
                        set: *set,
                        clear: *clear,
                    }
                }
                DirOp::Exclude => item.action = OpAction::Exclude,
                DirOp::Ignored(value) => item.action = OpAction::Ignored(*value),
                DirOp::NewFolder { target, name } => {
                    if !target.contains(side) {
                        continue;
                    }
                    if !node.is_dir {
                        return Err(PlanError::Invalid(
                            "new folder requires a parent folder".into(),
                        ));
                    }
                    valid_name(name)?;
                    let relative = if source.relative.is_empty() {
                        name.clone()
                    } else {
                        format!("{}/{name}", source.relative)
                    };
                    item.destination = Some(Location {
                        path: source.path.join(name),
                        side: Some(side),
                        relative,
                    });
                    item.source = None;
                    item.source_entry = None;
                    item.action = OpAction::EnsureDir;
                    item.target_known = false;
                }
                DirOp::CopyFilename => item.action = OpAction::CopyFilename,
            }
            if item
                .destination
                .as_ref()
                .is_some_and(|d| path_key(&d.path) == path_key(&source.path))
            {
                if item.action == OpAction::Rename {
                    if item.destination.as_ref().unwrap().path == source.path {
                        continue;
                    }
                    // A case-only rename is valid; execution verifies both spellings
                    // resolve to the same existing object before allowing it.
                } else {
                    return Err(PlanError::Invalid("source and destination coincide".into()));
                }
            }
            item.confirmations = confirmations(&item);
            let cleanup = is_dir
                && (matches!(
                    op,
                    DirOp::Move { .. } | DirOp::MoveOther | DirOp::MoveTo { .. }
                ) || (matches!(op, DirOp::Exchange) && node.sides[1 - side].is_none()));
            plan.items.push(item.clone());
            if is_dir
                && item.action == OpAction::EnsureDir
                && !matches!(op, DirOp::Exchange | DirOp::NewFolder { .. })
                && (plan.options.copy.timestamps
                    || plan.options.copy.attributes
                    || plan.options.copy.created)
            {
                let mut metadata = item.clone();
                metadata.action = OpAction::PreserveDir;
                metadata.target_known = false;
                metadata.confirmations.clear();
                plan.items.push(metadata);
            }
            if cleanup {
                item.destination = None;
                item.target_entry = None;
                item.action = OpAction::RemoveEmptyDir { recycle: false };
                item.confirmations = vec![ConfirmKind::Move];
                plan.items.push(item);
            }
        }
        plan.finish()?;
        Ok(plan)
    }
    pub(crate) fn finish(&mut self) -> Result<(), PlanError> {
        self.items.sort_by_key(|item| match item.action {
            OpAction::EnsureDir => (
                0,
                item.destination
                    .as_ref()
                    .map_or(0, |p| p.relative.matches('/').count()),
            ),
            OpAction::PreserveDir => (
                2,
                usize::MAX
                    - item
                        .destination
                        .as_ref()
                        .map_or(0, |p| p.relative.matches('/').count()),
            ),
            OpAction::RemoveEmptyDir { .. } => (
                3,
                usize::MAX
                    - item
                        .source
                        .as_ref()
                        .map_or(0, |p| p.relative.matches('/').count()),
            ),
            _ => (1, 0),
        });
        let sources: BTreeMap<_, _> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| {
                !matches!(
                    i.action,
                    OpAction::EnsureDir | OpAction::PreserveDir | OpAction::RemoveEmptyDir { .. }
                )
            })
            .filter_map(|(index, i)| i.source.as_ref().map(|p| (path_key(&p.path), index)))
            .collect();
        let mut destinations = BTreeMap::new();
        for (index, item) in self.items.iter().enumerate() {
            if let Some(dest) = &item.destination {
                if item.action == OpAction::PreserveDir {
                    continue;
                }
                let key = path_key(&dest.path);
                if sources.get(&key).is_some_and(|other| *other != index) {
                    return Err(PlanError::Collision(key));
                }
                if let Some(source) = &item.source {
                    if item
                        .source_entry
                        .as_ref()
                        .is_some_and(|e| e.kind == EntryKind::Dir)
                        && key.starts_with(&format!(
                            "{}/",
                            path_key(&source.path).trim_end_matches('/')
                        ))
                    {
                        return Err(PlanError::Invalid(
                            "cannot transfer a folder into itself".into(),
                        ));
                    }
                }
                if destinations.insert(key.clone(), item.node).is_some() {
                    return Err(PlanError::Collision(key));
                }
            }
        }
        Ok(())
    }
    pub fn total_bytes(&self) -> u64 {
        self.items.iter().map(OpItem::bytes).sum()
    }
}
pub fn location(tree: &DirTree, id: NodeId, side: usize) -> Result<Location, PlanError> {
    let base = tree
        .bases
        .get(side)
        .and_then(Option::as_ref)
        .ok_or_else(|| PlanError::Invalid("base folder missing".into()))?;
    let relative = tree.side_relative(id, side);
    Ok(Location {
        path: if relative.is_empty() {
            base.clone()
        } else {
            base.join(&relative)
        },
        side: Some(side),
        relative,
    })
}
pub fn path_key(path: &VfsPath) -> String {
    let key = path.to_local_path().map_or_else(
        || path.to_string(),
        |p| p.to_string_lossy().replace('\\', "/"),
    );
    if cfg!(windows) && path.is_local() {
        key.to_lowercase()
    } else {
        key
    }
}
fn common_parent(paths: &[String]) -> String {
    let mut prefix: Vec<_> = paths
        .first()
        .map_or("", String::as_str)
        .split('/')
        .collect();
    prefix.pop();
    for path in paths.iter().skip(1) {
        let mut parts: Vec<_> = path.split('/').collect();
        parts.pop();
        let n = prefix
            .iter()
            .zip(parts)
            .take_while(|(a, b)| **a == *b)
            .count();
        prefix.truncate(n);
    }
    if prefix.is_empty() {
        String::new()
    } else {
        format!("{}/", prefix.join("/"))
    }
}
pub fn valid_name(name: &str) -> Result<(), PlanError> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name
            .chars()
            .any(|c| c.is_control() || "/\\:*?\"<>|".contains(c))
        || name.ends_with([' ', '.'])
    {
        return Err(PlanError::Invalid(format!("invalid filename: {name:?}")));
    }
    Ok(())
}
pub fn rename_name(name: &str, spec: &RenameSpec) -> Result<String, PlanError> {
    let renamed = match spec {
        RenameSpec::Regex { old_mask, new_mask } => Pattern::new(old_mask, true)
            .map_err(|e| PlanError::Invalid(e.to_string()))?
            .replace_all(name, new_mask),
        RenameSpec::Dos { new_mask } => {
            fn part(old: &str, mask: &str) -> String {
                let chars: Vec<_> = old.chars().collect();
                let mut index = 0;
                let mut result = String::new();
                for c in mask.chars() {
                    match c {
                        '*' => {
                            result.extend(&chars[index..]);
                            index = chars.len();
                        }
                        '?' => {
                            if let Some(c) = chars.get(index) {
                                result.push(*c);
                                index += 1;
                            }
                        }
                        _ => {
                            result.push(c);
                            index = (index + 1).min(chars.len());
                        }
                    }
                }
                result
            }
            let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
            if let Some((new_stem, new_ext)) = new_mask.rsplit_once('.') {
                let a = part(stem, new_stem);
                let b = part(ext, new_ext);
                if b.is_empty() {
                    a
                } else {
                    format!("{a}.{b}")
                }
            } else {
                part(name, new_mask)
            }
        }
    };
    valid_name(&renamed)?;
    Ok(renamed)
}
pub(crate) fn confirmations(item: &OpItem) -> Vec<ConfirmKind> {
    let mut result = Vec::new();
    match item.action {
        OpAction::Move | OpAction::Rename => result.push(ConfirmKind::Move),
        OpAction::Delete { .. } | OpAction::RemoveEmptyDir { .. } => {
            result.push(ConfirmKind::Delete)
        }
        OpAction::Exchange => result.push(ConfirmKind::Exchange),
        _ => {}
    }
    if item.target_entry.is_some()
        && matches!(
            item.action,
            OpAction::Copy | OpAction::Move | OpAction::Rename
        )
    {
        let newer = match (
            item.source_entry.as_ref().and_then(|e| e.modified),
            item.target_entry.as_ref().and_then(|e| e.modified),
        ) {
            (Some(a), Some(b)) => b > a,
            _ => false,
        };
        result.push(if newer {
            ConfirmKind::OverwriteNewer
        } else {
            ConfirmKind::Overwrite
        });
    }
    if item.target_entry.as_ref().is_some_and(|e| e.attrs.readonly)
        || (!matches!(
            item.action,
            OpAction::Copy | OpAction::CopyFilename | OpAction::Exclude | OpAction::Ignored(_)
        ) && item.source_entry.as_ref().is_some_and(|e| e.attrs.readonly))
    {
        result.push(ConfirmKind::ReadOnly);
    }
    result
}
