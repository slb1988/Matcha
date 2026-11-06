//! A selection is expressed in comparison coordinates and projected to visible
//! rows/columns before any data command. Gaps never invent source rows.
use super::{cache::projected, grid, view::TableCompareView};
#[derive(Default)]
pub(super) struct Selection {
    pub rows: Vec<usize>,
    pub columns: Vec<usize>,
}
impl TableCompareView {
    pub(super) fn selection(&self) -> Selection {
        let Some(pair) = &self.pair else {
            return Selection::default();
        };
        let a = self.grid.anchor.unwrap_or(self.grid.cursor);
        let b = self.grid.cursor;
        let columns = self
            .grid
            .columns
            .iter()
            .copied()
            .filter(|c| (a.column.min(b.column)..=a.column.max(b.column)).contains(c))
            .collect();
        if pair.projection.is_empty() {
            return Selection {
                rows: Vec::new(),
                columns,
            };
        }
        let (lo, hi) = (a.row.min(b.row), a.row.max(b.row));
        let start = pair.projection.display_for(lo);
        let end = pair
            .projection
            .display_for(hi)
            .saturating_add(1)
            .min(pair.projection.len());
        Selection {
            rows: (start..end)
                .filter_map(|d| projected(&pair.projection, d))
                .filter(|r| (lo..=hi).contains(r))
                .collect(),
            columns,
        }
    }
    pub(super) fn selected_source_rows(&self, side: usize) -> Vec<usize> {
        let Some(pair) = &self.pair else {
            return Vec::new();
        };
        self.selection()
            .rows
            .into_iter()
            .filter_map(|r| {
                let row = grid::mapped(&pair.map, r);
                if side == 0 {
                    row.left
                } else {
                    row.right
                }
            })
            .collect()
    }
    pub(super) fn selected_source_columns(&self, side: usize) -> Vec<usize> {
        let Some(pair) = &self.pair else {
            return Vec::new();
        };
        let rules = pair.map.rules();
        self.selection()
            .columns
            .into_iter()
            .filter_map(|c| {
                rules
                    .columns
                    .0
                    .get(c)
                    .and_then(|c| if side == 0 { c.left } else { c.right })
            })
            .collect()
    }
}
