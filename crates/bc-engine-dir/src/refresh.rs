//! Explicit refresh work: I/O stays outside view derivation and unrelated rows survive.
use crate::{
    op_exec::metadata_optional,
    ops::{location, Location, Selection},
    tree::NodeId,
    DirTree,
};
use bc_core::cancel::CancelCheck;
use bc_vfs::{ChangeEvent, EntryKind, VfsError, VfsManager, VfsPath, VfsResult, Watcher};
use std::collections::{BTreeSet, VecDeque};

#[derive(Debug, Clone, Default)]
pub struct RefreshStats {
    pub metadata_reads: usize,
    pub listings: usize,
    pub changed_nodes: BTreeSet<NodeId>,
}
fn check(cancel: &dyn CancelCheck) -> VfsResult<()> {
    if cancel.is_cancelled() {
        Err(VfsError::Cancelled)
    } else {
        Ok(())
    }
}
fn valid_relative(relative: &str) -> VfsResult<()> {
    if relative
        .replace('\\', "/")
        .split('/')
        .any(|part| part == ".." || part.contains(':'))
    {
        return Err(VfsError::Other("invalid refresh path".into()));
    }
    Ok(())
}
pub fn refresh_locations(
    tree: &mut DirTree,
    vfs: &VfsManager,
    locations: &[Location],
    cancel: &dyn CancelCheck,
) -> VfsResult<RefreshStats> {
    let mut stats = RefreshStats::default();
    let mut seen = BTreeSet::new();
    let mut locations: Vec<_> = locations.iter().filter(|p| p.side.is_some()).collect();
    locations.sort_by_key(|p| p.relative.matches('/').count());
    for place in locations {
        let side = place.side.unwrap();
        if !seen.insert((side, place.relative.clone())) {
            continue;
        }
        valid_relative(&place.relative)?;
        check(cancel)?;
        if place.relative.is_empty() {
            continue;
        }
        let parent = place.relative.rsplit_once('/').map_or("", |(p, _)| p);
        // Only missing ancestors need metadata reads. Existing ancestors are summarized
        // in memory; a one-file copy does not re-list its 100,000 siblings.
        let mut prefix = String::new();
        for part in parent.split('/').filter(|s| !s.is_empty()) {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            let absent = tree
                .find(&prefix)
                .and_then(|id| tree.node(id))
                .map_or(true, |n| n.sides[side].is_none());
            if absent {
                let base = tree.bases[side]
                    .as_ref()
                    .ok_or(VfsError::Unsupported("missing base"))?;
                let entry = metadata_optional(vfs, &base.join(&prefix))?;
                stats.metadata_reads += 1;
                if entry.is_none() {
                    break;
                }
                if let Some(id) = tree.update_entry(side, &prefix, entry)? {
                    stats.changed_nodes.insert(id);
                }
            }
        }
        if tree.find(parent).is_none() {
            continue;
        }
        let entry = metadata_optional(vfs, &place.path)?;
        stats.metadata_reads += 1;
        let scan = entry.as_ref().is_some_and(|e| e.kind == EntryKind::Dir)
            && tree
                .find(&place.relative)
                .and_then(|id| tree.node(id))
                .map_or(true, |n| !n.loaded[side]);
        if let Some(id) = tree.update_entry(side, &place.relative, entry)? {
            stats.changed_nodes.insert(id);
        }
        if scan {
            scan_from(tree, vfs, side, &place.relative, cancel, &mut stats)?;
        }
    }
    Ok(stats)
}
pub fn refresh_selection(
    tree: &mut DirTree,
    vfs: &VfsManager,
    selection: &Selection,
    cancel: &dyn CancelCheck,
) -> VfsResult<RefreshStats> {
    let locations = selection
        .items
        .iter()
        .map(|&(id, side)| location(tree, id, side).map_err(|e| VfsError::Other(e.to_string())))
        .collect::<VfsResult<Vec<_>>>()?;
    refresh_locations(tree, vfs, &locations, cancel)
}
fn scan_from(
    tree: &mut DirTree,
    vfs: &VfsManager,
    side: usize,
    relative: &str,
    cancel: &dyn CancelCheck,
    stats: &mut RefreshStats,
) -> VfsResult<()> {
    let base = tree.bases[side]
        .clone()
        .ok_or(VfsError::Unsupported("missing base"))?;
    let mut queue = VecDeque::from([relative.to_owned()]);
    let mut canonical = BTreeSet::new();
    while let Some(relative) = queue.pop_front() {
        check(cancel)?;
        let path = if relative.is_empty() {
            base.clone()
        } else {
            base.join(&relative)
        };
        if let Some(local) = path.to_local_path() {
            if !canonical.insert(std::fs::canonicalize(local)?) {
                continue;
            }
        }
        let entries = vfs.resolve(&path)?.list(&path)?;
        stats.listings += 1;
        for e in &entries {
            if e.kind == EntryKind::Dir && e.error.is_none() {
                queue.push_back(if relative.is_empty() {
                    e.name.clone()
                } else {
                    format!("{relative}/{}", e.name)
                });
            }
        }
        tree.apply_listing(side, &relative, entries)?;
        if let Some(id) = tree.find(&relative) {
            stats.changed_nodes.insert(id);
        }
    }
    Ok(())
}
pub fn full_refresh(
    tree: &mut DirTree,
    vfs: &VfsManager,
    cancel: &dyn CancelCheck,
) -> VfsResult<RefreshStats> {
    let mut stats = RefreshStats::default();
    for side in 0..2 {
        if tree.bases[side].is_some() {
            scan_from(tree, vfs, side, "", cancel, &mut stats)?;
        }
    }
    Ok(stats)
}
/// Ordinary Refresh re-enumerates loaded folders only; Full Refresh recursively scans
/// newly discovered/unexpanded folders too.
pub fn refresh_loaded(
    tree: &mut DirTree,
    vfs: &VfsManager,
    cancel: &dyn CancelCheck,
) -> VfsResult<RefreshStats> {
    let loaded: Vec<_> = tree
        .nodes()
        .filter(|(_, n)| n.is_dir)
        .flat_map(|(id, n)| (0..2).filter(move |&s| n.loaded[s]).map(move |s| (id, s)))
        .collect();
    let mut stats = RefreshStats::default();
    for (id, side) in loaded {
        check(cancel)?;
        if tree.node(id).is_none() {
            continue;
        }
        let place = location(tree, id, side).map_err(|e| VfsError::Other(e.to_string()))?;
        match vfs.resolve(&place.path)?.list(&place.path) {
            Ok(entries) => {
                tree.apply_listing(side, &place.relative, entries)?;
                stats.listings += 1;
                stats.changed_nodes.insert(id);
            }
            Err(VfsError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                tree.update_entry(side, &place.relative, None)?;
            }
            Err(e) => return Err(e),
        }
    }
    Ok(stats)
}
pub struct AutoRefresh {
    watchers: Vec<(usize, VfsPath, Box<dyn Watcher>)>,
}
impl AutoRefresh {
    pub fn new(tree: &DirTree, vfs: &VfsManager) -> VfsResult<Self> {
        let mut watchers = Vec::new();
        for side in 0..2 {
            if let Some(base) = &tree.bases[side] {
                watchers.push((side, base.clone(), vfs.resolve(base)?.watch(base)?));
            }
        }
        Ok(Self { watchers })
    }
    pub fn poll(
        &mut self,
        tree: &mut DirTree,
        vfs: &VfsManager,
        cancel: &dyn CancelCheck,
    ) -> VfsResult<RefreshStats> {
        let mut locations = Vec::new();
        let mut overflow = false;
        for (side, base, watcher) in &mut self.watchers {
            for event in watcher.poll() {
                let paths = match event {
                    ChangeEvent::Created(p)
                    | ChangeEvent::Modified(p)
                    | ChangeEvent::Deleted(p) => vec![p],
                    ChangeEvent::Renamed { from, to } => vec![from, to],
                    ChangeEvent::Overflow => {
                        overflow = true;
                        vec![]
                    }
                };
                for path in paths {
                    let base_path = base
                        .to_local_path()
                        .ok_or(VfsError::Unsupported("non-local watch"))?;
                    let full = path
                        .to_local_path()
                        .ok_or(VfsError::Unsupported("non-local watch"))?;
                    let relative = full
                        .strip_prefix(base_path)
                        .map_err(|e| VfsError::Other(e.to_string()))?
                        .to_string_lossy()
                        .replace('\\', "/");
                    locations.push(Location {
                        path,
                        side: Some(*side),
                        relative,
                    });
                }
            }
        }
        if overflow {
            full_refresh(tree, vfs, cancel)
        } else {
            refresh_locations(tree, vfs, &locations, cancel)
        }
    }
}
