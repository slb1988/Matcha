//! View projections never change comparison statuses. Flatten is a virtual pairing.
use crate::{
    criteria::Criteria,
    filter::{pair_visibility, DisplayFilter, FolderDisplay},
    tree::{DirTree, NodeId},
    CompareStatus,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ViewOptions {
    pub display: DisplayFilter,
    pub folders: FolderDisplay,
    pub suppress_filters: bool,
    pub ignore_unimportant: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewRow {
    /// Each side references its source node, which can differ in Flatten mode.
    pub sides: [Option<NodeId>; 2],
    pub node: Option<NodeId>,
    pub depth: usize,
    pub status: CompareStatus,
    pub teal: [bool; 2],
}
impl DirTree {
    pub fn view_rows(&self, options: ViewOptions) -> Vec<ViewRow> {
        if let Some((generation, old_options, rows)) = self.view_cache.borrow().as_ref() {
            if *generation == self.generation && *old_options == options {
                return rows.clone();
            }
        }
        let rows = self.build_view_rows(options);
        *self.view_cache.borrow_mut() = Some((self.generation, options, rows.clone()));
        rows
    }

    fn build_view_rows(&self, options: ViewOptions) -> Vec<ViewRow> {
        if options.folders == FolderDisplay::Flatten {
            return self.flatten_rows(options);
        }
        let mut eligible = BTreeMap::new();
        let mut descendants = BTreeMap::new();
        let mut statuses = BTreeMap::new();
        for (id, node) in self.nodes().collect::<Vec<_>>().into_iter().rev() {
            let mut status = node.status;
            let has_visible_file = node
                .children
                .iter()
                .any(|child| descendants.get(child) == Some(&true));
            if options.ignore_unimportant {
                if status == CompareStatus::SimilarUnimportant {
                    status = CompareStatus::Same;
                }
                if node.is_dir
                    && !node.is_orphan()
                    && node.children.iter().all(|child| {
                        statuses
                            .get(child)
                            .map_or(true, |s| *s == CompareStatus::Same)
                    })
                    && !node.summary.pending
                {
                    status = CompareStatus::Same;
                }
            }
            statuses.insert(id, status);
            let included = std::array::from_fn(|s| !node.filtered[s] && !node.excluded);
            let present = std::array::from_fn(|s| node.sides[s].is_some());
            let visibility = pair_visibility(
                present,
                included,
                status,
                options.display,
                options.suppress_filters,
                options.ignore_unimportant,
            );
            let pending_hint=node.summary.pending&&options.display!=DisplayFilter::None&&(!node.is_orphan()||options.display.shows(status));
            let error_hint=node.sides.iter().flatten().any(|e|e.error.is_some())&&options.display.shows(status);
            let show = if node.is_dir {
                (options.suppress_filters
                    || present.iter().enumerate().any(|(s, p)| *p && included[s]))
                    && (options.suppress_filters
                        || options.folders.shows(
                            status,
                            has_visible_file || pending_hint || error_hint,
                            options.display,
                        ))
            } else {
                visibility.visible
            };
            descendants.insert(id, if node.is_dir { has_visible_file } else { show });
            eligible.insert(id, (show, visibility.teal));
        }
        let mut rows = Vec::new();
        let mut pending: Vec<_> = self
            .root()
            .children
            .iter()
            .rev()
            .map(|&id| (id, 0))
            .collect();
        while let Some((id, depth)) = pending.pop() {
            let Some(node) = self.node(id) else {
                continue;
            };
            let (show, teal) = eligible[&id];
            if !show {
                continue;
            }
            rows.push(ViewRow {
                node: Some(id),
                sides: std::array::from_fn(|s| node.sides[s].as_ref().map(|_| id)),
                depth,
                status: statuses[&id],
                teal,
            });
            if node.expanded {
                pending.extend(node.children.iter().rev().map(|&child| (child, depth + 1)));
            }
        }
        rows
    }
    fn flatten_rows(&self, options: ViewOptions) -> Vec<ViewRow> {
        let mut groups: BTreeMap<String, [Vec<NodeId>; 2]> = BTreeMap::new();
        for (id, node) in self.nodes() {
            for side in 0..2 {
                if let Some(entry) = &node.sides[side] {
                    if entry.kind == bc_vfs::EntryKind::Dir {
                        continue;
                    }
                    let mut name = if self.pairing().normalize_nfc {
                        // set_pairing validated platform support before this projection.
                        bc_vfs::LocalFs::normalize_nfc(&entry.name).expect("validated NFC support")
                    } else {
                        entry.name.clone()
                    };
                    if !self.pairing().case_sensitive {
                        name = name.to_lowercase();
                    }
                    groups.entry(name).or_default()[side].push(id);
                }
            }
        }
        let mut rows = Vec::new();
        for mut sides in groups.into_values() {
            for (side, ids) in sides.iter_mut().enumerate() {
                ids.sort_by_key(|&id| self.side_relative(id, side));
            }
            for index in 0..sides[0].len().max(sides[1].len()) {
                let ids = sides.each_ref().map(|ids| ids.get(index).copied());
                let entries = std::array::from_fn::<_, 2, _>(|s| {
                    ids[s]
                        .and_then(|id| self.node(id))
                        .and_then(|n| n.sides[s].as_ref())
                });
                let mut status = self.criteria().quick(entries[0], entries[1]);
                if ids[0].is_some() && ids[0] == ids[1] {
                    status = self.node(ids[0].unwrap()).unwrap().status;
                } else if let (Some(left), Some(right)) = (ids[0], ids[1]) {
                    if let Some(result) = self.virtual_result(left, right) {
                        status =
                            self.criteria()
                                .combine(status, result.content, result.versions_equal);
                    }
                }
                if options.ignore_unimportant && status == CompareStatus::SimilarUnimportant {
                    status = CompareStatus::Same;
                }
                let included = std::array::from_fn(|s| {
                    ids[s]
                        .and_then(|id| self.node(id))
                        .is_some_and(|n| !n.filtered[s] && !n.excluded)
                });
                let visibility = pair_visibility(
                    ids.map(|id| id.is_some()),
                    included,
                    status,
                    options.display,
                    options.suppress_filters,
                    options.ignore_unimportant,
                );
                if visibility.visible {
                    rows.push(ViewRow {
                        sides: ids,
                        node: None,
                        depth: 0,
                        status,
                        teal: visibility.teal,
                    });
                }
            }
        }
        rows
    }
    pub(crate) fn criteria(&self) -> &Criteria {
        self.criteria_ref()
    }
}
