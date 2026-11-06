use super::view::TableCompareView;
use bc_core::CoreError;
use bc_engine_table::columns::Side;

impl TableCompareView {
    pub(super) fn sort_selection(&mut self, descending: bool) -> Result<(), CoreError> {
        let active = self.grid.active;
        if self.files[active].readonly {
            return Err(CoreError::Invalid("Editing disabled".into()));
        }
        if !self.is_ready() || self.editor.is_some() {
            return Err(CoreError::Invalid(
                "Finish editing and wait for comparison before sorting".into(),
            ));
        }
        let selected = self.selection();
        if selected.rows.is_empty() || selected.columns.is_empty() {
            self.message = "No visible rows / columns selected".into();
            return Ok(());
        }
        let side = if active == 0 { Side::Left } else { Side::Right };
        // Decide scope from the original gesture, not the number of rows left
        // after filtering/gap removal. A reduced selection must never expand.
        let partial = self
            .grid
            .anchor
            .is_some_and(|a| a.row != self.grid.cursor.row);
        let order = {
            let pair = self.pair.as_ref().unwrap();
            let rules = pair.map.rules();
            let keys = selected
                .columns
                .iter()
                .filter_map(|&column| {
                    rules
                        .columns
                        .0
                        .get(column)?
                        .index(side)
                        .map(|source| (source, rules.handling(column)))
                })
                .collect::<Vec<_>>();
            let mut rows = selected
                .rows
                .iter()
                .filter_map(|&row| {
                    let pair = pair.map.row(row)?;
                    if active == 0 {
                        pair.left
                    } else {
                        pair.right
                    }
                })
                .collect::<Vec<_>>();
            if rows.is_empty() || keys.is_empty() {
                self.message = "Selected cells are gaps on the active side".into();
                return Ok(());
            }
            let sheet = if active == 0 {
                pair.map.left.borrow()
            } else {
                pair.map.right.borrow()
            };
            if !partial {
                rows = (0..sheet.row_count()).collect();
            }
            sheet.sorted_row_order(&rows, &keys, descending)?
        };
        self.focus_grid = Some(active);
        let Some(order) = order else {
            self.message = "Selected rows already in requested order".into();
            return Ok(());
        };
        let pair = self.pair.as_mut().unwrap();
        let edit = pair
            .map
            .edit_sheet(side, "Sort rows", |sheet, _, anchors| {
                sheet.reorder_rows(&order)?;
                // Match existing structural-edit policy; undo restores the anchors.
                anchors.clear();
                Ok(())
            })?;
        pair.history[active].push(edit);
        self.edited();
        Ok(())
    }
}
