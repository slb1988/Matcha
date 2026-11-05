//! Arena tree: stable row identities, per-folder name tables and cached visible rows.
use crate::{CompareStatus::*, Criteria, DisplayFilter};
use bc_core::pattern::Pattern;
use bc_vfs::{EntryKind, LocalFs, VfsEntry, VfsResult};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CompareStatus {
    #[default]
    Unknown,
    Same,
    Different,
    LeftNewer,
    RightNewer,
    LeftOrphan,
    RightOrphan,
    SimilarUnimportant,
    Error,
}
pub type NodeId = usize;
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FolderSummary {
    pub red: bool,
    pub purple: bool,
    pub error: bool,
    pub pending: bool,
}
impl FolderSummary {
    fn status(status: CompareStatus) -> Self {
        Self {
            red: matches!(
                status,
                Different | LeftNewer | RightNewer | SimilarUnimportant
            ),
            purple: matches!(status, LeftOrphan | RightOrphan),
            error: status == Error,
            pending: status == Unknown,
        }
    }
    fn merge(&mut self, rhs: Self) {
        self.red |= rhs.red;
        self.purple |= rhs.purple;
        self.error |= rhs.error;
        self.pending |= rhs.pending;
    }
}
#[derive(Debug, Clone, Default)]
pub struct DirNode {
    pub name: String,
    pub relative: String,
    pub parent: Option<NodeId>,
    pub sides: [Option<VfsEntry>; 3],
    pub status: CompareStatus,
    pub quick_status: CompareStatus,
    pub content_status: Option<CompareStatus>,
    pub children: Vec<NodeId>,
    pub expanded: bool,
    pub excluded: bool,
    pub filtered: [bool; 2],
    pub is_dir: bool,
    pub summary: FolderSummary,
    pub loaded: [bool; 3],
    pub revision: u64,
    pub alive: bool,
    child_table: BTreeMap<String, Vec<NodeId>>,
}
impl DirNode {
    pub fn is_orphan(&self) -> bool {
        matches!(self.status, LeftOrphan | RightOrphan)
    }
    pub fn hollow(&self) -> bool {
        self.is_dir && self.summary.pending
    }
    pub fn yellow(&self, automatic_scan: bool) -> bool {
        self.is_dir && !automatic_scan && !self.expanded && self.summary.pending
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingOptions {
    pub case_sensitive: bool,
    pub normalize_nfc: bool,
    pub ignore_extension: bool,
}
impl Default for PairingOptions {
    fn default() -> Self {
        Self {
            case_sensitive: !cfg!(windows),
            normalize_nfc: false,
            ignore_extension: false,
        }
    }
}
#[derive(Debug, Clone)]
pub struct AlignmentRule {
    pub folder: Option<String>,
    left: Pattern,
    right: Pattern,
}
impl AlignmentRule {
    pub fn new(
        left: &str,
        right: &str,
        regex: bool,
        folder: Option<String>,
    ) -> Result<Self, String> {
        let compile = |s: &str| {
            let source = if regex {
                s.to_owned()
            } else {
                let mut result = String::new();
                for c in s.chars() {
                    match c {
                        '*' => result.push_str("(.*)"),
                        '?' => result.push_str("(.)"),
                        _ => {
                            if ".+()[]{}^$|\\".contains(c) {
                                result.push('\\');
                            }
                            result.push(c);
                        }
                    }
                }
                result
            };
            Pattern::new(&format!("^(?:{source})$"), false).map_err(|e| e.to_string())
        };
        Ok(Self {
            folder: folder.map(|s| normalize_path(&s)),
            left: compile(left)?,
            right: compile(right)?,
        })
    }
    fn key(&self, side: usize, parent: &str, name: &str) -> Option<String> {
        if self
            .folder
            .as_deref()
            .is_some_and(|s| !s.eq_ignore_ascii_case(parent))
        {
            return None;
        }
        let found = if side == 0 { &self.left } else { &self.right }.match_at(name, 0)?;
        Some(
            found
                .captures
                .iter()
                .skip(1)
                .map(|r| {
                    r.as_ref()
                        .map(|r| name[r.clone()].to_lowercase())
                        .unwrap_or_default()
                })
                .map(|s| format!("{}:{s}", s.len()))
                .collect::<Vec<_>>()
                .join("|"),
        )
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowRef {
    pub node: NodeId,
    pub depth: usize,
}
#[derive(Debug, Default)]
struct VisibleCache {
    key: Option<(u64, DisplayFilter)>,
    rows: Vec<RowRef>,
}

#[derive(Debug)]
pub struct DirTree {
    nodes: Vec<DirNode>,
    pub bases: [Option<bc_vfs::VfsPath>; 3],
    pairing: PairingOptions,
    align_overrides: Vec<AlignmentRule>,
    criteria: Criteria,
    dirty: BTreeSet<NodeId>,
    pub(crate) generation: u64,
    visible: RefCell<VisibleCache>,
    pub(crate) view_cache:
        RefCell<Option<(u64, crate::view::ViewOptions, Vec<crate::view::ViewRow>)>>,
}
impl Default for DirTree {
    fn default() -> Self {
        Self {
            nodes: vec![DirNode {
                is_dir: true,
                expanded: true,
                alive: true,
                ..DirNode::default()
            }],
            bases: Default::default(),
            pairing: PairingOptions::default(),
            align_overrides: Vec::new(),
            criteria: Criteria::default(),
            dirty: BTreeSet::new(),
            generation: 0,
            visible: RefCell::default(),
            view_cache: RefCell::default(),
        }
    }
}
pub fn normalize_path(relative: &str) -> String {
    relative
        .replace('\\', "/")
        .trim_matches('/')
        .trim_start_matches("./")
        .into()
}
impl DirTree {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn root(&self) -> &DirNode {
        &self.nodes[0]
    }
    pub fn node(&self, id: NodeId) -> Option<&DirNode> {
        self.nodes.get(id).filter(|n| n.alive)
    }
    pub fn nodes(&self) -> impl Iterator<Item = (NodeId, &DirNode)> {
        self.nodes.iter().enumerate().filter(|(_, n)| n.alive)
    }
    pub fn pairing(&self) -> &PairingOptions {
        &self.pairing
    }
    pub fn find(&self, relative: &str) -> Option<NodeId> {
        let mut current = 0;
        for part in normalize_path(relative)
            .split('/')
            .filter(|p| !p.is_empty())
        {
            current = *self.nodes[current].children.iter().find(|&&id| {
                let n = &self.nodes[id];
                n.alive
                    && (n.name == part
                        || n.sides.iter().flatten().any(|e| e.name == part)
                        || (!self.pairing.case_sensitive
                            && n.name.to_lowercase() == part.to_lowercase()))
            })?;
        }
        Some(current)
    }
    pub fn set_pairing(&mut self, options: PairingOptions) -> VfsResult<()> {
        if options.normalize_nfc {
            LocalFs::normalize_nfc("test")?;
        }
        self.pairing = options;
        self.realign()
    }
    pub fn add_alignment(&mut self, rule: AlignmentRule) -> VfsResult<()> {
        self.align_overrides.push(rule);
        self.realign()
    }
    pub fn align_with(&mut self, folder: &str, left: &str, right: &str) -> VfsResult<()> {
        let literal = |s: &str| {
            let mut escaped = String::new();
            for c in s.chars() {
                if ".+*?()[]{}^$|\\".contains(c) {
                    escaped.push('\\');
                }
                escaped.push(c);
            }
            escaped
        };
        let rule = AlignmentRule::new(&literal(left), &literal(right), true, Some(folder.into()))
            .map_err(bc_vfs::VfsError::Other)?;
        self.add_alignment(rule)
    }
    fn realign(&mut self) -> VfsResult<()> {
        // Replay listings top-down; overrides survive future refreshes.
        let snapshots: Vec<_> = self
            .nodes()
            .filter(|(_, n)| n.is_dir)
            .map(|(_, n)| {
                let listings: [Vec<VfsEntry>; 3] = std::array::from_fn(|side| {
                    n.children
                        .iter()
                        .filter_map(|&id| self.nodes[id].sides[side].clone())
                        .collect()
                });
                (n.relative.clone(), n.loaded, listings)
            })
            .collect();
        // Retire old ids so in-flight content results can never hit a newly paired row.
        for node in &mut self.nodes[1..] {
            node.alive = false;
        }
        self.nodes[0].children.clear();
        self.nodes[0].child_table.clear();
        self.nodes[0].loaded = [false; 3];
        self.generation += 1;
        for (relative, loaded, listings) in snapshots {
            if self.find(&relative).is_none() {
                continue;
            }
            for (side, entries) in listings.into_iter().enumerate() {
                if loaded[side] {
                    self.apply_listing(side, &relative, entries)?;
                }
            }
        }
        Ok(())
    }
    fn name_key(&self, side: usize, parent: &str, entry: &VfsEntry) -> VfsResult<String> {
        if side < 2 {
            for (index, rule) in self.align_overrides.iter().enumerate() {
                if let Some(key) = rule.key(side, parent, &entry.name) {
                    return Ok(format!("override:{index}:{key}"));
                }
            }
        }
        let mut name = if self.pairing.normalize_nfc {
            LocalFs::normalize_nfc(&entry.name)?
        } else {
            entry.name.clone()
        };
        if self.pairing.ignore_extension && entry.kind == EntryKind::File {
            if let Some((stem, _)) = name.rsplit_once('.') {
                if !stem.is_empty() {
                    name = stem.to_owned();
                }
            }
        }
        if !self.pairing.case_sensitive {
            name = name.to_lowercase();
        }
        Ok(format!("name:{name}"))
    }

    pub fn apply_listing(
        &mut self,
        side: usize,
        relative: &str,
        mut entries: Vec<VfsEntry>,
    ) -> VfsResult<()> {
        if side > 2 {
            return Err(bc_vfs::VfsError::Other("invalid side".into()));
        }
        let parent = self
            .find(relative)
            .ok_or_else(|| bc_vfs::VfsError::Other(format!("folder not found: {relative}")))?;
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        let relative = self.nodes[parent].relative.clone();
        let keyed = entries
            .into_iter()
            .map(|entry| Ok((self.name_key(side, &relative, &entry)?, entry)))
            .collect::<VfsResult<Vec<_>>>()?;
        let children = self.nodes[parent].children.clone();
        // Rebuild the small per-folder index using the current pairing policy.
        let mut table: BTreeMap<String, Vec<NodeId>> = BTreeMap::new();
        for &id in &children {
            for (s, entry) in self.nodes[id]
                .sides
                .iter()
                .enumerate()
                .filter_map(|(s, e)| e.as_ref().map(|e| (s, e)))
            {
                let key = self.name_key(s, &relative, entry)?;
                let ids = table.entry(key).or_default();
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
        // Release this side before pairing the replacement listing. Otherwise a
        // case-only rename can incorrectly leave two opposite-side orphans.
        let previous: BTreeMap<_, _> = children
            .iter()
            .filter_map(|&id| self.nodes[id].sides[side].take().map(|e| (id, e)))
            .collect();
        let mut used = BTreeSet::new();
        let mut new_children = children.clone();
        for (key, entry) in keyed {
            let candidates = table.entry(key).or_default();
            let id = candidates
                .iter()
                .copied()
                .find(|id| {
                    !used.contains(id) && previous.get(id).is_some_and(|e| e.name == entry.name)
                })
                .or_else(|| {
                    candidates
                        .iter()
                        .copied()
                        .find(|id| !used.contains(id) && self.nodes[*id].sides[side].is_none())
                });
            let id = id.unwrap_or_else(|| {
                let id = self.nodes.len();
                self.nodes.push(DirNode {
                    name: entry.name.clone(),
                    relative: if relative.is_empty() {
                        entry.name.clone()
                    } else {
                        format!("{relative}/{}", entry.name)
                    },
                    parent: Some(parent),
                    alive: true,
                    ..DirNode::default()
                });
                candidates.push(id);
                new_children.push(id);
                id
            });
            used.insert(id);
            if previous
                .get(&id)
                .is_some_and(|old| old.kind != entry.kind || old.name != entry.name)
            {
                for child in self.nodes[id].children.clone() {
                    self.remove_side(child, side);
                }
                self.nodes[id].loaded[side] = false;
            }
            let node = &mut self.nodes[id];
            node.alive = true;
            node.sides[side] = Some(entry);
            node.revision += 1;
            node.content_status = None;
            node.is_dir = node
                .sides
                .iter()
                .flatten()
                .any(|e| e.kind == EntryKind::Dir);
            node.quick_status = self
                .criteria
                .quick(node.sides[0].as_ref(), node.sides[1].as_ref());
            node.status = node.quick_status;
            self.mark_dirty(id);
        }
        for id in children {
            if !used.contains(&id) && previous.contains_key(&id) {
                self.remove_side(id, side);
            }
        }
        new_children.retain(|&id| self.nodes[id].alive);
        new_children.sort_by(|&a, &b| self.nodes[a].name.cmp(&self.nodes[b].name));
        table
            .values_mut()
            .for_each(|ids| ids.retain(|id| self.nodes[*id].alive));
        self.nodes[parent].children = new_children;
        self.nodes[parent].child_table = table;
        self.nodes[parent].loaded[side] = true;
        self.mark_dirty(parent);
        self.summarize();
        Ok(())
    }
    fn remove_side(&mut self, id: NodeId, side: usize) {
        let mut pending = vec![id];
        while let Some(id) = pending.pop() {
            let node = &mut self.nodes[id];
            pending.extend(node.children.iter().copied());
            node.sides[side] = None;
            node.loaded[side] = false;
            node.revision += 1;
            node.content_status = None;
            node.alive = node.sides.iter().any(Option::is_some);
            node.quick_status = self
                .criteria
                .quick(node.sides[0].as_ref(), node.sides[1].as_ref());
            node.status = node.quick_status;
            self.mark_dirty(id);
        }
    }
    fn mark_dirty(&mut self, id: NodeId) {
        let mut current = Some(id);
        while let Some(id) = current {
            self.dirty.insert(id);
            current = self.nodes[id].parent;
        }
        self.generation += 1;
    }
    fn summarize(&mut self) {
        while let Some(id) = self.dirty.pop_last() {
            let node = &self.nodes[id];
            let mut summary = FolderSummary::status(if node.is_dir {
                node.quick_status
            } else {
                node.status
            });
            if node.is_dir {
                summary.pending = node.sides.iter().enumerate().any(|(side, e)| {
                    e.as_ref().is_some_and(|e| e.kind == EntryKind::Dir) && !node.loaded[side]
                });
                if id == 0 {
                    summary.pending = !node.loaded[0] || !node.loaded[1];
                }
                for &child in &node.children {
                    if self.nodes[child].alive {
                        summary.merge(self.nodes[child].summary);
                    }
                }
            }
            let node = &mut self.nodes[id];
            node.summary = summary;
            if node.is_dir && !node.is_orphan() {
                node.status = if summary.error {
                    Error
                } else if summary.red || summary.purple {
                    Different
                } else if summary.pending {
                    Unknown
                } else {
                    Same
                };
            }
        }
    }
    pub fn evaluate_quick(&mut self, criteria: &Criteria) {
        self.criteria = criteria.clone();
        for id in 1..self.nodes.len() {
            if !self.nodes[id].alive {
                continue;
            }
            let node = &mut self.nodes[id];
            node.revision += 1;
            node.content_status = None;
            node.quick_status = criteria.quick(node.sides[0].as_ref(), node.sides[1].as_ref());
            node.status = node.quick_status;
            self.mark_dirty(id);
        }
        self.summarize();
    }
    pub fn apply_content(
        &mut self,
        id: NodeId,
        revision: u64,
        status: CompareStatus,
        versions_equal: Option<bool>,
    ) -> bool {
        let Some(node) = self
            .nodes
            .get_mut(id)
            .filter(|n| n.alive && n.revision == revision)
        else {
            return false;
        };
        node.content_status = Some(status);
        node.status = self
            .criteria
            .combine(node.quick_status, Some(status), versions_equal);
        self.mark_dirty(id);
        self.summarize();
        true
    }
    pub fn set_expanded(&mut self, paths: Option<&[String]>, expanded: bool) {
        let ids: Vec<_> = if let Some(paths) = paths {
            paths.iter().filter_map(|p| self.find(p)).collect()
        } else {
            (1..self.nodes.len()).collect()
        };
        for id in ids {
            if self.nodes[id].is_dir {
                self.nodes[id].expanded = expanded;
            }
        }
        self.generation += 1;
    }
    pub fn visible_rows(&self, filter: DisplayFilter) -> Vec<RowRef> {
        let mut cache = self.visible.borrow_mut();
        if cache.key == Some((self.generation, filter)) {
            return cache.rows.clone();
        }
        cache.rows.clear();
        let mut pending: Vec<_> = self.nodes[0]
            .children
            .iter()
            .rev()
            .map(|&node| RowRef { node, depth: 0 })
            .collect();
        while let Some(row) = pending.pop() {
            let node = &self.nodes[row.node];
            if !node.alive || node.excluded {
                continue;
            }
            if filter.shows(node.status) || node.is_dir {
                cache.rows.push(row);
            }
            if node.is_dir && node.expanded {
                pending.extend(node.children.iter().rev().map(|&node| RowRef {
                    node,
                    depth: row.depth + 1,
                }));
            }
        }
        cache.key = Some((self.generation, filter));
        cache.rows.clone()
    }
    pub fn visible(&self, filter: &DisplayFilter) -> Vec<&DirNode> {
        self.visible_rows(*filter)
            .iter()
            .map(|r| &self.nodes[r.node])
            .collect()
    }
    pub(crate) fn criteria_ref(&self) -> &Criteria {
        &self.criteria
    }

    pub fn side_relative(&self, id: NodeId, side: usize) -> String {
        let mut parts = Vec::new();
        let mut current = Some(id);
        while let Some(id) = current.filter(|&id| id != 0) {
            let node = &self.nodes[id];
            parts.push(
                node.sides
                    .get(side)
                    .and_then(Option::as_ref)
                    .map_or(node.name.as_str(), |e| e.name.as_str()),
            );
            current = node.parent;
        }
        parts.reverse();
        parts.join("/")
    }

    /// Apply caller-provided content decisions after metadata/name tests. All ancestor
    /// promotion is structural; excluded folders still block their descendants.
    pub fn apply_filters(
        &mut self,
        names: &crate::NameFilter,
        other: &crate::OtherFilters,
        clock: crate::filter::FilterClock,
        content_included: impl Fn(NodeId, usize) -> bool,
    ) {
        let mut blocked = vec![[false; 2]; self.nodes.len()];
        for id in 1..self.nodes.len() {
            if !self.nodes[id].alive {
                continue;
            }
            let parent = self.nodes[id].parent.unwrap_or(0);
            for (side, inherited) in blocked[parent].into_iter().enumerate() {
                let Some(entry) = self.nodes[id].sides[side].as_ref() else {
                    continue;
                };
                let relative = self.side_relative(id, side);
                let metadata = other.matches_metadata(entry, clock);
                blocked[id][side] = inherited || (entry.kind == EntryKind::Dir && !metadata);
                self.nodes[id].filtered[side] = inherited
                    || !metadata
                    || !names.matches(&relative, self.nodes[id].is_dir)
                    || (!self.nodes[id].is_dir && !content_included(id, side));
            }
        }
        // Actual included descendants promote their ancestors even if no positive
        // folder mask matched that ancestor. Never promote a protected subtree.
        for id in (1..self.nodes.len()).rev() {
            if !self.nodes[id].alive {
                continue;
            }
            if let Some(parent) = self.nodes[id].parent {
                for side in 0..2 {
                    if self.nodes[id].sides[side].is_some()
                        && !self.nodes[id].filtered[side]
                        && !self.nodes[id].excluded
                    {
                        self.nodes[parent].filtered[side] = false;
                    }
                }
            }
        }
        self.generation += 1;
    }

    pub fn count_status(&self, status: CompareStatus) -> usize {
        self.nodes()
            .filter(|(_, n)| !n.is_dir && n.status == status)
            .count()
    }
}
