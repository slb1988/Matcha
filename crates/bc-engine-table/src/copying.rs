//! Plan comparison-coordinate copies before applying one reversible edit.
use crate::{
    columns::{ColumnChange, Side},
    invalid,
    store::SheetStore,
    Result, TableMap,
};
use bc_core::{align::AlignmentOverrides, edit::EditItem};
use std::cmp::Reverse;

fn copy_value(
    source: &SheetStore,
    src: (usize, usize),
    target: &mut SheetStore,
    dst: (usize, usize),
) -> Result<()> {
    let mut cell = source
        .cell_snapshot(src.0, src.1)
        .ok_or_else(|| invalid("source cell out of range"))?;
    cell.style = source
        .style(cell.style)
        .cloned()
        .map(|s| target.intern_style(s));
    let display = source
        .display_at(src.0, src.1)
        .ok_or_else(|| invalid("source cell out of range"))?;
    target.set_cell(dst.0, dst.1, cell)?;
    target.insert_display_override(dst.0, dst.1, display.into());
    Ok(())
}

impl TableMap {
    /// Copy all selected cells atomically. Gaps or invalid coordinates reject the
    /// entire request, including with deferred edits. One item owns one snapshot
    /// pair, rather than copying whole changed columns once per selected cell.
    pub fn copy_cells(&self, cells: &[(usize, usize)], from: Side) -> Result<Box<dyn EditItem>> {
        if self.is_stale() {
            return Err(invalid("recompare before copying through table map"));
        }
        if cells.is_empty() {
            return Err(invalid("no cells selected"));
        }
        let source = match from {
            Side::Left => self.left.borrow().clone(),
            Side::Right => self.right.borrow().clone(),
        };
        let plan = {
            let rules = self.rules();
            cells
                .iter()
                .map(|&(row, column)| {
                    let pair = self
                        .row(row)
                        .ok_or_else(|| invalid("display row out of range"))?;
                    let mapping = rules
                        .columns
                        .0
                        .get(column)
                        .ok_or_else(|| invalid("comparison column out of range"))?;
                    let (src, dst) = match from {
                        Side::Left => (pair.left, pair.right),
                        Side::Right => (pair.right, pair.left),
                    };
                    let src = src.ok_or_else(|| invalid("source cell is a gap"))?;
                    let dst =
                        dst.ok_or_else(|| invalid("destination row is a gap; copy the row first"))?;
                    let sc = mapping
                        .index(from)
                        .ok_or_else(|| invalid("source column is a gap"))?;
                    let dc = mapping
                        .index(from.other())
                        .ok_or_else(|| invalid("destination column is a gap"))?;
                    Ok(((src, sc), (dst, dc)))
                })
                .collect::<Result<Vec<_>>>()?
        };
        self.edit_sheet(
            from.other(),
            if cells.len() == 1 {
                "Copy cell"
            } else {
                "Copy cells"
            },
            move |target, _, _| {
                for (src, dst) in plan {
                    copy_value(&source, src, target, dst)?;
                }
                Ok(())
            },
        )
    }

    /// Copy comparison rows using original destination indices. Insertions are
    /// applied in descending *source-file* order, not display order (which may
    /// be sorted). Existing rows at an insertion point are written first.
    pub fn copy_rows(&self, rows: &[usize], from: Side) -> Result<Box<dyn EditItem>> {
        if self.is_stale() {
            return Err(invalid("recompare before copying through table map"));
        }
        if rows.is_empty() {
            return Err(invalid("no rows selected"));
        }
        let (source, destination_len) = match from {
            Side::Left => (self.left.borrow().clone(), self.right.borrow().row_count()),
            Side::Right => (self.right.borrow().clone(), self.left.borrow().row_count()),
        };
        let destination = from.other();
        let mut selected = rows.to_vec();
        selected.sort_unstable();
        selected.dedup();
        let mut plan = Vec::with_capacity(selected.len());
        // Reuse the next destination for a contiguous gap run; do not rescan
        // the same suffix once per orphan when copying a large selection.
        let mut gap_anchor = None;
        for display in selected {
            let pair = self
                .row(display)
                .ok_or_else(|| invalid("display row out of range"))?;
            let (src, dst) = match from {
                Side::Left => (pair.left, pair.right),
                Side::Right => (pair.right, pair.left),
            };
            let src = src.ok_or_else(|| invalid("source row is a gap"))?;
            let at = if let Some(dst) = dst {
                dst
            } else {
                let anchor = match gap_anchor {
                    Some((end, at)) if display < end => (end, at),
                    _ => ((display + 1)..self.row_count())
                        .find_map(|r| {
                            let row = self.row(r)?;
                            match destination {
                                Side::Left => row.left,
                                Side::Right => row.right,
                            }
                            .map(|at| (r, at))
                        })
                        .unwrap_or((self.row_count(), destination_len)),
                };
                gap_anchor = Some(anchor);
                anchor.1
            };
            plan.push((display, src, at, dst.is_none()));
        }
        plan.sort_by_key(|&(display, _, at, insert)| Reverse((at, !insert, display)));
        self.edit_sheet(
            destination,
            if plan.len() == 1 {
                "Copy row"
            } else {
                "Copy rows"
            },
            move |target, rules, anchors| {
                let mut columns = Vec::new();
                for column in &mut rules.columns.0 {
                    if let Some(sc) = column.index(from) {
                        let dc = if let Some(dc) = column.index(destination) {
                            dc
                        } else {
                            let dc = target.column_count();
                            target.ensure_size(target.row_count(), dc + 1);
                            match destination {
                                Side::Left => column.left = Some(dc),
                                Side::Right => column.right = Some(dc),
                            }
                            column.changes.retain(|c| {
                                !matches!(c, ColumnChange::Inserted | ColumnChange::Deleted)
                            });
                            dc
                        };
                        columns.push((sc, dc));
                    }
                }
                for (_, src, at, insert) in plan {
                    if insert {
                        target.insert_row(at, Vec::new())?;
                        let mut shifted = AlignmentOverrides::default();
                        for &(l, r) in anchors.pairs() {
                            shifted.insert(
                                if destination == Side::Left && l >= at {
                                    l + 1
                                } else {
                                    l
                                },
                                if destination == Side::Right && r >= at {
                                    r + 1
                                } else {
                                    r
                                },
                            )?;
                        }
                        *anchors = shifted;
                    }
                    for &(sc, dc) in &columns {
                        copy_value(&source, (src, sc), target, (at, dc))?;
                    }
                }
                Ok(())
            },
        )
    }
}
