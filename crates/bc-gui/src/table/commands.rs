use super::{
    find::Action,
    grid::{self, Cursor, GridAction},
    view::{CellEditor, FileMode, PathDialog, Pending, Staged, TableAction, TableCompareView},
    worker::{LoadJob, Loaded, Worker},
};
use bc_core::{cancel::NeverCancel, edit::EditStack, CoreError};
use bc_engine_table::{
    columns::{ComparisonColumn, Side},
    source::{read_text, ReadOptions},
    summary::SummaryFormat,
    Cell, Sheet,
};
use bc_session::prefs::Prefs;
use eframe::egui;
use std::{io::Cursor as IoCursor, path::Path};
pub(super) fn side(n: usize) -> Side {
    if n == 0 {
        Side::Left
    } else {
        Side::Right
    }
}
fn invalid(message: &str) -> CoreError {
    CoreError::Invalid(message.into())
}
impl TableCompareView {
    pub(super) fn source_row(&self, side: usize, row: usize) -> Result<usize, CoreError> {
        let pair = self
            .pair
            .as_ref()
            .ok_or_else(|| invalid("Table is loading"))?;
        let row = grid::mapped(&pair.map, row);
        (if side == 0 { row.left } else { row.right })
            .ok_or_else(|| invalid("Row is a gap; copy the row first"))
    }
    fn source_position(&self, s: usize, cursor: Cursor) -> Result<(usize, usize), CoreError> {
        let p = self
            .pair
            .as_ref()
            .ok_or_else(|| invalid("Table is loading"))?;
        let column = p
            .map
            .rules()
            .columns
            .0
            .get(cursor.column)
            .and_then(|c| c.index(side(s)))
            .ok_or_else(|| invalid("Column is a gap"))?;
        Ok((self.source_row(s, cursor.row)?, column))
    }
    /// A source sheet is pending while a staged load runs or the active pair's
    /// replacement side sheet has not been published; writes into it (and into
    /// its paired sheet, whose revision the in-flight install checks) are
    /// refused. A merely stale comparison (pending_side None) stays editable.
    pub(super) fn source_pending(&self) -> bool {
        self.staged.is_some() || self.pair.as_ref().is_some_and(|p| p.pending_side.is_some())
    }
    pub(super) fn begin_edit(&mut self, s: usize, cursor: Cursor, replacement: Option<String>) {
        if self.files[s].readonly {
            self.message = "Editing disabled".into();
            return;
        }
        if self.source_pending() {
            self.message = "Wait for the load to finish before editing".into();
            return;
        }
        if self.editor.is_some() {
            return;
        }
        match self.source_position(s, cursor) {
            Ok((row, col)) => {
                let editor = {
                    let p = self.pair.as_ref().unwrap();
                    let sheet = if s == 0 {
                        p.map.left.borrow()
                    } else {
                        p.map.right.borrow()
                    };
                    let original = sheet.display_at(row, col).unwrap_or_default();
                    CellEditor {
                        side: s,
                        source: Cursor { row, column: col },
                        display: cursor,
                        text: replacement.unwrap_or_else(|| original.clone()),
                        original,
                        original_cell: sheet.cell_snapshot(row, col).expect("cell in range"),
                        focus: true,
                    }
                };
                self.grid.active = s;
                self.grid.cursor = cursor;
                if !self.grid.columns.contains(&cursor.column) {
                    self.hide_same = false;
                    self.refresh_grid();
                }
                if !self.grid.visible.contains(&cursor.row) {
                    self.goto_cell(cursor);
                }
                self.editor = Some(editor);
            }
            Err(e) => self.message = e.to_string(),
        }
    }
    pub(super) fn commit_edit(&mut self) -> Result<(), CoreError> {
        let Some(editor) = self.editor.as_ref() else {
            return Ok(());
        };
        let side_index = editor.side;
        if !editor.changed() {
            self.editor = None;
            self.focus_grid = Some(side_index);
            self.key_focus = true;
            return Ok(());
        }
        if self.files[side_index].readonly {
            return Err(invalid("Editing disabled; the draft has been retained"));
        }
        let pair = self
            .pair
            .as_mut()
            .ok_or_else(|| invalid("Table is loading; the draft has been retained"))?;
        {
            let sheet = if side_index == 0 {
                pair.map.left.borrow()
            } else {
                pair.map.right.borrow()
            };
            if sheet
                .cell_snapshot(editor.source.row, editor.source.column)
                .as_ref()
                != Some(&editor.original_cell)
                || sheet
                    .display_at(editor.source.row, editor.source.column)
                    .as_deref()
                    != Some(&editor.original)
            {
                return Err(invalid(
                    "The cell changed while editing; the draft has been retained",
                ));
            }
        }
        let mut cell = editor.original_cell.clone();
        cell.value = bc_engine_table::CellValue::Text(editor.text.clone().into());
        cell.formula = None; // Entering a literal replaces a formula, never evaluates one.
        let edit = pair.map.modify_cell(
            side(side_index),
            editor.source.row,
            editor.source.column,
            cell,
        )?;
        pair.history[side_index].push(edit);
        self.editor = None;
        self.grid.active = side_index;
        self.focus_grid = Some(side_index);
        self.key_focus = true;
        self.edited();
        Ok(())
    }
    pub(super) fn grid_action(&mut self, action: GridAction) {
        let (s, cursor, extend, edit) = match action {
            GridAction::Select(s, c, e) => (s, c, e, false),
            GridAction::Edit(s, c) => (s, c, false, true),
            GridAction::Context(s, c) => {
                // Right click inside the selection keeps it (the menu acts on
                // it); elsewhere it selects the clicked cell first.
                let a = self.grid.anchor.unwrap_or(self.grid.cursor);
                let b = self.grid.cursor;
                let inside = self.grid.active == s
                    && (a.row.min(b.row)..=a.row.max(b.row)).contains(&c.row)
                    && (a.column.min(b.column)..=a.column.max(b.column)).contains(&c.column);
                if inside {
                    self.grid.active = s;
                    self.focus_grid = Some(s);
                    return;
                }
                (s, c, false, false)
            }
        };
        if self.align_with.is_some() && !self.is_ready() {
            self.align_with = None;
            self.message = "Wait for comparison before aligning rows".into();
            return;
        }
        if let Some((from, row)) = self.align_with.take() {
            if from != s {
                if let Ok(to) = self.source_row(s, cursor.row) {
                    let p = self.pair.as_mut().unwrap();
                    let edit = p.map.edit_sheet(side(s), "Align With", |_, _, anchors| {
                        anchors.insert(
                            if from == 0 { row } else { to },
                            if from == 0 { to } else { row },
                        )
                    });
                    match edit {
                        Ok(edit) => {
                            p.history[s].push(edit);
                            self.grid.active = s;
                            self.grid.cursor = cursor;
                            self.grid.anchor = None;
                            self.focus_grid = Some(s);
                            self.edited();
                        }
                        Err(e) => self.message = e.to_string(),
                    }
                }
            } else {
                self.message = "Pick a row on the other side".into();
                self.align_with = Some((from, row));
            }
            return;
        }
        if extend {
            self.grid.anchor.get_or_insert(self.grid.cursor);
        } else {
            self.grid.anchor = None;
        }
        self.grid.active = s;
        self.grid.cursor = cursor;
        // A user-placed cursor/selection paints in the selection color (a
        // freshly loaded comparison shows only the focus frame).
        self.grid.selection_visible = true;
        self.key_focus = true;
        if edit {
            self.begin_edit(s, cursor, None);
        }
    }
    pub(super) fn navigate(&mut self, forward: bool, kind: u8) {
        if !self.is_ready() {
            self.message = "Wait for comparison before navigating differences".into();
            return;
        }
        let p = self.pair.as_ref().unwrap();
        if p.map.row_count() == 0 || (kind == 0 && p.map.column_count() == 0) {
            self.message = "No more differences / edits".into();
            return;
        }
        let current = Cursor {
            row: self
                .grid
                .cursor
                .row
                .min(p.map.row_count().saturating_sub(1)),
            column: self
                .grid
                .cursor
                .column
                .min(p.map.column_count().saturating_sub(1)),
        };
        let rows = if kind == 2 {
            &p.cache.edited_rows
        } else {
            &p.cache.diff_rows
        };
        let next_row = |row: usize| {
            if forward {
                rows.get(rows.partition_point(|&r| r <= row)).copied()
            } else {
                rows.partition_point(|&r| r < row)
                    .checked_sub(1)
                    .and_then(|i| rows.get(i))
                    .copied()
            }
        };
        let target = if kind == 0 {
            let cols = p.map.column_count();
            let mut row = current.row;
            let mut column = if forward {
                current.column.saturating_add(1)
            } else {
                current.column
            };
            let mut found = None;
            loop {
                let range: Box<dyn Iterator<Item = usize>> = if forward {
                    Box::new(column..cols)
                } else {
                    Box::new((0..column.min(cols)).rev())
                };
                if let Some(c) = range.into_iter().find(|&c| {
                    p.map
                        .cell_importance(row, c)
                        .is_some_and(|i| i != bc_engine_table::compare::Importance::Same)
                }) {
                    found = Some(Cursor { row, column: c });
                    break;
                }
                let Some(next) = next_row(row) else {
                    break;
                };
                row = next;
                column = if forward { 0 } else { cols };
            }
            found
        } else {
            next_row(current.row).map(|row| Cursor {
                row,
                column: if self.grid.columns.contains(&current.column) {
                    current.column
                } else {
                    self.grid.columns.first().copied().unwrap_or(current.column)
                },
            })
        };
        if let Some(target) = target {
            self.goto_cell(target);
            self.focus_grid = Some(self.grid.active);
        } else {
            self.message = "No more differences / edits".into();
        }
    }
    pub(super) fn undo(&mut self, redo: bool) {
        self.cancel();
        if let Some(p) = &mut self.pair {
            if redo {
                p.history[self.grid.active].redo();
            } else {
                p.history[self.grid.active].undo();
            }
            self.edited();
        }
    }
    pub(super) fn copy(&mut self, whole_row: bool) {
        let from = self.grid.active;
        let to = 1 - from;
        if self.files[to].readonly {
            self.message = "Destination editing disabled".into();
            return;
        }
        if !self.is_ready() {
            self.message = "Wait for comparison before copying".into();
            return;
        }
        let selected = self.selection();
        if selected.rows.is_empty() || (!whole_row && selected.columns.is_empty()) {
            self.message = "No visible rows / cells selected".into();
            return;
        }
        let p = self.pair.as_mut().unwrap();
        let result = if whole_row {
            p.map.copy_rows(&selected.rows, side(from))
        } else {
            let cells = selected
                .rows
                .iter()
                .flat_map(|&row| selected.columns.iter().map(move |&column| (row, column)))
                .collect::<Vec<_>>();
            p.map.copy_cells(&cells, side(from))
        };
        match result {
            Ok(edit) => {
                p.history[to].push(edit);
                self.grid.active = to;
                self.focus_grid = Some(to);
                self.edited();
            }
            Err(error) => self.message = error.to_string(),
        }
    }
    fn selected_cells(&self) -> Vec<(usize, usize)> {
        let selected = self.selection();
        selected
            .rows
            .into_iter()
            .flat_map(|row| {
                selected.columns.iter().filter_map(move |&column| {
                    self.source_position(self.grid.active, Cursor { row, column })
                        .ok()
                })
            })
            .collect()
    }
    pub(super) fn clipboard_text(&self) -> Option<String> {
        let pair = self.pair.as_ref()?;
        let selected = self.selection();
        if selected.rows.is_empty() || selected.columns.is_empty() {
            return None;
        }
        let s = self.grid.active;
        let sheet = if s == 0 {
            pair.map.left.borrow()
        } else {
            pair.map.right.borrow()
        };
        let quote = |text: String| {
            if text.is_empty() || text.contains(['\t', '\r', '\n', '"']) {
                format!("\"{}\"", text.replace('"', "\"\""))
            } else {
                text
            }
        };
        let mut records = Vec::new();
        for row in selected.rows {
            let mapped = grid::mapped(&pair.map, row);
            if (if s == 0 { mapped.left } else { mapped.right }).is_none() {
                continue;
            }
            records.push(
                selected
                    .columns
                    .iter()
                    .map(|&column| {
                        quote(
                            self.source_position(s, Cursor { row, column })
                                .ok()
                                .and_then(|(r, c)| sheet.display_at(r, c))
                                .unwrap_or_default(),
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\t"),
            );
        }
        if records.is_empty() {
            None
        } else {
            Some(records.join("\r\n"))
        }
    }
    pub(super) fn clear_selected(&mut self) {
        let s = self.grid.active;
        if self.files[s].readonly {
            self.message = "Editing disabled".into();
            return;
        }
        if self.source_pending() {
            self.message = "Wait for the load to finish before deleting".into();
            return;
        }
        let cells = self.selected_cells();
        if cells.is_empty() {
            return;
        }
        let Some(p) = &mut self.pair else {
            return;
        };
        match p.map.edit_sheet(side(s), "Delete cells", |sheet, _, _| {
            for (r, c) in cells {
                sheet.set_cell(r, c, Cell::default())?;
            }
            Ok(())
        }) {
            Ok(e) => p.history[s].push(e),
            Err(e) => self.message = e.to_string(),
        }
        self.edited();
    }
    fn paste(&mut self, text: String) -> Result<(), CoreError> {
        let s = self.grid.active;
        if self.files[s].readonly {
            return Err(invalid("Editing disabled"));
        }
        if self.source_pending() {
            return Err(invalid("Wait for the load to finish before pasting"));
        }
        let (r, c) = self.source_position(s, self.grid.cursor).or_else(|_| {
            let p = self
                .pair
                .as_ref()
                .ok_or_else(|| invalid("Table is loading"))?;
            Ok::<_, CoreError>((
                if s == 0 {
                    p.map.left.borrow().row_count()
                } else {
                    p.map.right.borrow().row_count()
                },
                0,
            ))
        })?;
        let options = ReadOptions {
            format: bc_formats::TableOptions {
                delimiters: vec!['\t'],
                header: bc_formats::model::HeaderMode::Data,
                ..Default::default()
            },
            ..Default::default()
        };
        let pasted = read_text(IoCursor::new(text), &options, &NeverCancel)?.sheet;
        if pasted.row_count() == 0 || pasted.column_count() == 0 {
            return Ok(());
        }
        let mut values: Vec<Vec<Cell>> = (0..pasted.row_count())
            .map(|r| {
                (0..pasted.column_count())
                    .map(|c| Cell::text(pasted.display_at(r, c).unwrap_or_default()))
                    .collect()
            })
            .collect();
        let p = self.pair.as_mut().unwrap();
        let e = p
            .map
            .edit_sheet(side(s), "Paste cells", |sheet, rules, anchors| {
                for (row, cells) in values.iter_mut().enumerate() {
                    for (col, cell) in cells.iter_mut().enumerate() {
                        cell.style = sheet
                            .cell_snapshot(r + row, c + col)
                            .and_then(|cell| cell.style);
                    }
                }
                sheet.paste_cells(r, c, values)?;
                let max = sheet.column_count();
                let mut represented = rules
                    .columns
                    .0
                    .iter()
                    .filter_map(|c| c.index(side(s)))
                    .collect::<Vec<_>>();
                for c in 0..max {
                    if !represented.contains(&c) {
                        rules.columns.0.push(ComparisonColumn::new(
                            (s == 0).then_some(c),
                            (s == 1).then_some(c),
                        ));
                        represented.push(c);
                    }
                }
                anchors.clear();
                Ok(())
            })?;
        p.history[s].push(e);
        self.edited();
        Ok(())
    }
    pub(super) fn structure(&mut self, operation: u8) -> Result<(), CoreError> {
        if matches!(operation, 6 | 7) {
            return self.sort_selection(operation == 6);
        }
        let s = self.grid.active;
        if self.files[s].readonly {
            return Err(invalid("Editing disabled"));
        }
        if !self.is_ready() || self.editor.is_some() {
            return Err(invalid(
                "Finish editing and wait for comparison before changing structure",
            ));
        }
        let cursor = self.grid.cursor;
        let mut selected_rows = self.selected_source_rows(s);
        let mut selected_cols = self.selected_source_columns(s);
        selected_rows.sort_unstable();
        selected_rows.dedup();
        selected_cols.sort_unstable();
        selected_cols.dedup();
        if (operation == 2 && selected_rows.is_empty())
            || (operation == 5 && selected_cols.is_empty())
        {
            return Ok(());
        }
        let (row, col) = {
            let p = self.pair.as_ref().unwrap();
            let sheet = if s == 0 {
                p.map.left.borrow()
            } else {
                p.map.right.borrow()
            };
            let rules = p.map.rules();
            let source_row = p
                .map
                .row(cursor.row)
                .and_then(|r| if s == 0 { r.left } else { r.right });
            if matches!(operation, 0 | 1)
                && p.map.row_count() > 0
                && super::cache::projected(&p.projection, p.projection.display_for(cursor.row))
                    != Some(cursor.row)
            {
                return Err(invalid("Select a visible row before inserting"));
            }
            if matches!(operation, 3 | 4)
                && !rules.columns.0.is_empty()
                && !self.grid.columns.contains(&cursor.column)
            {
                return Err(invalid("Select a visible column before inserting"));
            }
            let row = source_row
                .map(|r| r + usize::from(operation == 1))
                .unwrap_or_else(|| {
                    (cursor.row.saturating_add(1)..p.map.row_count())
                        .find_map(|r| {
                            p.map
                                .row(r)
                                .and_then(|r| if s == 0 { r.left } else { r.right })
                        })
                        .unwrap_or(sheet.row_count())
                });
            let col = rules
                .columns
                .0
                .get(cursor.column)
                .and_then(|c| c.index(side(s)))
                .map(|c| c + usize::from(operation == 4))
                .unwrap_or_else(|| {
                    rules
                        .columns
                        .0
                        .iter()
                        .skip(cursor.column.saturating_add(1))
                        .find_map(|c| c.index(side(s)))
                        .unwrap_or(sheet.column_count())
                });
            (row, col)
        };
        let p = self
            .pair
            .as_mut()
            .ok_or_else(|| invalid("Table is loading"))?;
        let e = p.map.edit_sheet(
            side(s),
            "Change table structure",
            move |sheet, rules, anchors| {
                let keys = rules.key_columns();
                for (i, c) in rules.columns.0.iter_mut().enumerate() {
                    c.handling.is_key = keys.contains(&i);
                }
                match operation {
                    0 => sheet.insert_row(row.min(sheet.row_count()), Vec::new())?,
                    1 => sheet.insert_row(row.min(sheet.row_count()), Vec::new())?,
                    2 => sheet.remove_row_indices(&selected_rows)?,
                    3 | 4 => {
                        let at = col.min(sheet.column_count());
                        sheet.insert_column(at, "New column".into())?;
                        for c in &mut rules.columns.0 {
                            let index = if s == 0 { &mut c.left } else { &mut c.right };
                            if let Some(i) = index {
                                if *i >= at {
                                    *i += 1;
                                }
                            }
                        }
                        rules.columns.0.insert(
                            (cursor.column + usize::from(operation == 4))
                                .min(rules.columns.0.len()),
                            ComparisonColumn::new((s == 0).then_some(at), (s == 1).then_some(at)),
                        );
                    }
                    5 => {
                        for &column in selected_cols.iter().rev() {
                            sheet.remove_column(column)?;
                        }
                        for c in &mut rules.columns.0 {
                            let index = if s == 0 { &mut c.left } else { &mut c.right };
                            *index = index.and_then(|i| {
                                if selected_cols.binary_search(&i).is_ok() {
                                    None
                                } else {
                                    Some(i - selected_cols.partition_point(|&c| c < i))
                                }
                            });
                        }
                        rules.columns.tidy();
                    }
                    _ => return Err(invalid("unknown table structure command")),
                }
                anchors.clear();
                let keys = rules
                    .columns
                    .0
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.handling.is_key)
                    .map(|(i, _)| i)
                    .collect();
                rules.set_manual_keys(keys)?;
                Ok(())
            },
        )?;
        p.history[s].push(e);
        self.focus_grid = Some(s);
        self.edited();
        Ok(())
    }
    pub(super) fn clear_alignment(&mut self) {
        if !self.is_ready() {
            self.message = "Wait for comparison before clearing alignment".into();
            return;
        }
        if let Some(p) = &mut self.pair {
            if p.map.overrides().pairs().is_empty() {
                return;
            }
            match p
                .map
                .edit_sheet(side(self.grid.active), "Clear alignment", |_, _, a| {
                    a.clear();
                    Ok(())
                }) {
                Ok(edit) => {
                    p.history[self.grid.active].push(edit);
                    self.focus_grid = Some(self.grid.active);
                    self.edited();
                }
                Err(error) => self.message = error.to_string(),
            }
        }
    }
    pub(super) fn select_all(&mut self) {
        if let Some(p) = &self.pair {
            let rows = if p.map.is_stale() {
                p.map
                    .left
                    .borrow()
                    .row_count()
                    .max(p.map.right.borrow().row_count())
            } else {
                p.cache.total
            };
            self.grid.anchor = Some(Cursor::default());
            self.grid.cursor = Cursor {
                row: rows.saturating_sub(1),
                column: p.map.rules().columns.0.len().saturating_sub(1),
            };
            self.focus_grid = Some(self.grid.active);
        }
    }
    pub(super) fn find_action(&mut self, action: Action) -> Result<(), CoreError> {
        if !self.is_ready() {
            return Err(invalid("Wait for comparison before finding"));
        }
        let pattern = self.find.pattern()?;
        if self.find.query.is_empty() {
            return Err(invalid("Enter a search pattern"));
        }
        let s = self.find.side;
        let p = self.pair.as_ref().unwrap();
        let mut hits = Vec::new();
        let sheet = if s == 0 {
            p.map.left.borrow()
        } else {
            p.map.right.borrow()
        };
        for row in 0..p.map.row_count() {
            let mapped = grid::mapped(&p.map, row);
            let source = if s == 0 { mapped.left } else { mapped.right };
            let Some(r) = source else {
                continue;
            };
            for (column, col) in p.map.rules().columns.0.iter().enumerate() {
                let Some(c) = col.index(side(s)) else {
                    continue;
                };
                let text = sheet.display_at(r, c).unwrap_or_default();
                let mut at = 0;
                while let Some(hit) = pattern.find_at(&text, at) {
                    let end = hit.span.end;
                    if self.find.accepts(&text, hit.span.clone()) {
                        hits.push((row, column, hit.span.clone(), r, c, text.clone()));
                    }
                    if end == text.len() {
                        break;
                    }
                    at = if end > at {
                        end
                    } else {
                        end + text[end..].chars().next().unwrap().len_utf8()
                    };
                }
            }
        }
        drop(sheet);
        match action {
            Action::Next | Action::Previous => {
                let backwards = matches!(action, Action::Previous);
                // Hit positions are side-local: switching sides restarts from the boundary.
                let previous = if self.find.last_side == s {
                    self.find
                        .last
                        .as_ref()
                        .map(|(r, c, span)| (*r, *c, span.start))
                } else {
                    None
                };
                let hit = if backwards {
                    hits.iter()
                        .rev()
                        .find(|h| previous.is_none_or(|p| (h.0, h.1, h.2.start) < p))
                } else {
                    hits.iter()
                        .find(|h| previous.is_none_or(|p| (h.0, h.1, h.2.start) > p))
                };
                if let Some(hit) = hit {
                    self.find.last = Some((hit.0, hit.1, hit.2.clone()));
                    self.find.last_side = s;
                    self.grid.active = s;
                    self.goto_cell(Cursor {
                        row: hit.0,
                        column: hit.1,
                    });
                    self.message = format!("Found at row {}, column {}", hit.0 + 1, hit.1 + 1);
                } else {
                    self.message = "No more matches".into();
                }
            }
            Action::Replace | Action::All => {
                if self.files[s].readonly {
                    return Err(invalid("Editing disabled"));
                }
                let mut replacements = std::collections::BTreeMap::new();
                for hit in hits {
                    if matches!(action, Action::Replace)
                        && (self.find.last_side != s
                            || self.find.last.as_ref() != Some(&(hit.0, hit.1, hit.2.clone())))
                    {
                        continue;
                    }
                    let span = hit.2.clone();
                    let text = pattern.replace_all_if(&hit.5, &self.find.replacement, |r| {
                        self.find.accepts(&hit.5, r.clone())
                            && (matches!(action, Action::All) || r == span)
                    });
                    // Replacing a value with itself is not an edit.
                    if text != hit.5 {
                        replacements.insert((hit.3, hit.4), text);
                    }
                }
                if replacements.is_empty() {
                    self.message = "Replaced 0 cell(s)".into();
                    return Ok(());
                }
                let p = self.pair.as_mut().unwrap();
                let count = replacements.len();
                let e = p
                    .map
                    .edit_sheet(side(s), "Replace table cells", |sheet, _, _| {
                        for ((r, c), text) in replacements {
                            sheet.set_cell(r, c, Cell::text(text))?;
                        }
                        Ok(())
                    })?;
                p.history[s].push(e);
                // Route the next Undo/Redo to the side the edits landed on.
                self.grid.active = s;
                self.focus_grid = Some(s);
                self.edited();
                self.message = format!("Replaced {count} cell(s)");
            }
        }
        Ok(())
    }
    pub(super) fn keyboard(&mut self, ctx: &egui::Context, prefs: &Prefs) {
        if self.pending.is_some() || self.dialog.is_some() {
            return;
        }
        if self.editor.is_some() {
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        // A click outside the grid panes hands the keyboard to whatever was
        // clicked (find box, toolbar, settings, thumbnail...).
        for event in &events {
            if let egui::Event::PointerButton {
                pos, pressed: true, ..
            } = event
            {
                if !self.grid.pane_rects.iter().any(|r| r.contains(*pos)) {
                    self.key_focus = false;
                }
            }
        }
        let focused = self.key_focus;
        if self.paste_requested && !focused {
            self.paste_requested = false;
        }
        for event in events {
            match event {
                egui::Event::Paste(text) if self.clipboard_side.is_some() => {
                    let s = self.clipboard_side.take().unwrap();
                    self.request(Pending::Clipboard(s, text));
                }
                egui::Event::Key {
                    key: egui::Key::F,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.command => {
                    self.find.open = true;
                    self.key_focus = false;
                }
                egui::Event::Key {
                    key: egui::Key::S,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.command => {
                    if let Err(e) = self.save(self.grid.active, prefs) {
                        self.message = e.to_string();
                    }
                }
                egui::Event::Paste(text) if self.paste_requested || focused => {
                    self.paste_requested = false;
                    if let Err(e) = self.paste(text) {
                        self.message = e.to_string();
                    }
                }
                _ if !focused => {}
                egui::Event::Copy => {
                    if let Some(text) = self.clipboard_text() {
                        ctx.copy_text(text);
                    }
                }
                egui::Event::Cut => {
                    if let Some(text) = self.clipboard_text() {
                        ctx.copy_text(text);
                        self.clear_selected();
                    }
                }
                egui::Event::Text(text) => {
                    self.begin_edit(self.grid.active, self.grid.cursor, Some(text))
                }
                egui::Event::Ime(egui::ImeEvent::Preedit { .. }) => {
                    self.begin_edit(self.grid.active, self.grid.cursor, Some(String::new()))
                }
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => match key {
                    egui::Key::Z if modifiers.command => self.undo(modifiers.shift),
                    egui::Key::Y if modifiers.command => self.undo(true),
                    egui::Key::A if modifiers.command => self.select_all(),
                    egui::Key::F2 | egui::Key::Enter => {
                        self.begin_edit(self.grid.active, self.grid.cursor, None)
                    }
                    egui::Key::Delete | egui::Key::Backspace => self.clear_selected(),
                    egui::Key::Escape => {
                        self.key_focus = false;
                    }
                    egui::Key::F3 => {
                        self.find.open = true;
                        self.key_focus = false;
                        let a = if modifiers.shift {
                            Action::Previous
                        } else {
                            Action::Next
                        };
                        if let Err(e) = self.find_action(a) {
                            self.message = e.to_string();
                        }
                    }
                    egui::Key::ArrowDown
                    | egui::Key::ArrowUp
                    | egui::Key::ArrowLeft
                    | egui::Key::ArrowRight
                    | egui::Key::PageDown
                    | egui::Key::PageUp
                    | egui::Key::Tab => {
                        let old = self.grid.cursor;
                        if modifiers.shift {
                            self.grid.anchor.get_or_insert(old);
                        } else {
                            self.grid.anchor = None;
                        }
                        let page = (self.grid.pane_rects[self.grid.active].height()
                            / super::grid::ROW_HEIGHT)
                            .max(1.0) as usize;
                        let mut c = old;
                        match key {
                            egui::Key::ArrowDown => c.row += 1,
                            egui::Key::ArrowUp => c.row = c.row.saturating_sub(1),
                            egui::Key::ArrowLeft => c.column = c.column.saturating_sub(1),
                            egui::Key::PageDown => c.row += page,
                            egui::Key::PageUp => c.row = c.row.saturating_sub(page),
                            _ => c.column += 1,
                        }
                        if let Some(p) = &self.pair {
                            c.row = c.row.min(p.cache.total.saturating_sub(1));
                            c.column = c.column.min(p.map.column_count().saturating_sub(1));
                        }
                        self.grid.cursor = c;
                        self.ensure_cursor_visible();
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
    /// Scroll the active pane so the grid cursor stays visible after keyboard
    /// navigation (immediate, no framework animation; horizontal follows the
    /// goto_cell left-alignment convention).
    pub(super) fn ensure_cursor_visible(&mut self) {
        let Some(p) = &self.pair else { return };
        let rect = self.grid.pane_rects[self.grid.active];
        if !rect.is_positive() {
            return;
        }
        let display = p.projection.display_for(self.grid.cursor.row);
        let row_top = display as f32 * super::grid::ROW_HEIGHT;
        let row_bottom = row_top + super::grid::ROW_HEIGHT;
        if row_top < self.grid.scroll.y {
            self.grid.scroll.y = row_top;
        } else if row_bottom > self.grid.scroll.y + rect.height() {
            self.grid.scroll.y = row_bottom - rect.height();
        }
        if let Some(d) = self
            .grid
            .columns
            .iter()
            .position(|&c| c == self.grid.cursor.column)
        {
            let left = self.grid.offsets[d];
            let right = self.grid.offsets[d + 1];
            let view = (rect.width() - super::grid::GUTTER).max(0.0);
            if left < self.grid.scroll.x || right > self.grid.scroll.x + view {
                self.grid.scroll.x = left;
            }
        }
    }
    pub fn save_to(
        &mut self,
        s: usize,
        path: &Path,
        prefs: &Prefs,
        export: bool,
    ) -> Result<(), CoreError> {
        // Never write a placeholder: while the target side is still loading
        // (staged load or an unpublished pending pair sheet), refuse and keep
        // the load running instead of cancelling it and saving empty content.
        if self.staged.is_some() {
            return Err(invalid("Wait for the load to finish before saving"));
        }
        if self.pair.as_ref().is_some_and(|p| p.pending_side.is_some()) {
            return Err(invalid(
                "A sheet is still loading; saving now could write placeholder content",
            ));
        }
        if self.editor.as_ref().is_some_and(|e| e.side == s) {
            self.commit_edit()?;
        }
        // A save cancels only a pending comparison worker, never the sheet
        // scan; whether the write succeeds or fails, a stale map re-arms its
        // background realignment so the view cannot wedge in stale state.
        self.cancel_comparison();
        let result = self.write_side(s, path, prefs, export);
        if self.pair.as_ref().is_some_and(|p| p.map.is_stale()) {
            self.edited_at = Some(std::time::Instant::now());
        }
        result
    }
    fn write_side(
        &mut self,
        s: usize,
        path: &Path,
        prefs: &Prefs,
        export: bool,
    ) -> Result<(), CoreError> {
        let p = self
            .pair
            .as_mut()
            .ok_or_else(|| invalid("Table is loading"))?;
        let sheet = if s == 0 {
            p.map.left.borrow()
        } else {
            p.map.right.borrow()
        };
        if self.files[s].readonly {
            return Err(invalid("Editing/saving disabled"));
        }
        if !export && sheet.source_is_workbook() {
            return Err(invalid("Workbook is read-only; export as CSV"));
        }
        if prefs.backups.enabled && path.exists() {
            let name = path
                .file_name()
                .ok_or_else(|| invalid("Missing filename"))?
                .to_string_lossy();
            let backup = path.with_file_name(if let Some(ext) = &prefs.backups.extension {
                format!("{name}{ext}")
            } else {
                format!("~{name}")
            });
            std::fs::copy(path, backup)?;
        }
        if export {
            let temp = path.with_file_name(format!(
                ".matcha-table-export-{}-{}.tmp",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ));
            let result = (|| {
                let mut f = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&temp)?;
                bc_engine_table::store::export_store(&sheet, &mut f)?;
                f.sync_all()?;
                drop(f);
                std::fs::rename(&temp, path)?;
                Ok::<_, CoreError>(())
            })();
            if result.is_err() {
                let _ = std::fs::remove_file(temp);
            }
            result?;
        } else {
            bc_engine_table::store::save_store(&sheet, path)?;
            p.history[s].mark_saved();
            self.files[s].clipboard_dirty = false;
            self.files[s].target = Some(path.into());
        }
        drop(sheet);
        self.message = format!("Saved {}", path.display());
        Ok(())
    }
    pub(super) fn save(&mut self, s: usize, prefs: &Prefs) -> Result<bool, CoreError> {
        if let Some(path) = self.files[s]
            .target
            .clone()
            .or_else(|| self.files[s].path.clone())
        {
            self.save_to(s, &path, prefs, false)?;
            Ok(true)
        } else {
            self.dialog = Some(PathDialog {
                mode: FileMode::Save(s),
                path: String::new(),
            });
            Ok(false)
        }
    }
    pub(super) fn request(&mut self, p: Pending) {
        let needs_confirm = match &p {
            // Single-side replacement preserves the untouched side's edits and
            // history (cached pairs are re-hung, not dropped); only edits on the
            // REPLACED side — active or cached — plus an orphan placeholder
            // side or an open draft can lose data and therefore need confirmation.
            Pending::Open(s, _) | Pending::Clipboard(s, _) => {
                let s = *s;
                // An orphan tab's placeholder side can still hold pasted edits.
                let orphan_other_dirty =
                    self.pairs.0.get(self.active_pair).is_some_and(|pair| {
                        (if s == 0 { pair.right } else { pair.left }).is_none()
                    }) && self
                        .pair
                        .as_ref()
                        .is_some_and(|p| p.history[1 - s].is_dirty());
                self.files[s].clipboard_dirty
                    || self.pair.as_ref().is_some_and(|p| p.history[s].is_dirty())
                    || self.cached.values().any(|p| p.history[s].is_dirty())
                    || orphan_other_dirty
                    || self.editor.as_ref().is_some_and(CellEditor::changed)
            }
            Pending::Settings(d) => {
                let destroys = d.options != self.options
                    || d.pairs != self.pairs
                    || d.paths
                        != self
                            .paths()
                            .map(|p| p.map_or(String::new(), |p| p.display().to_string()))
                    || d.left_source != self.left_source;
                // An open changed draft is dirty state too: a rules-only Apply
                // must not silently discard it.
                (destroys || self.editor.as_ref().is_some_and(CellEditor::changed))
                    && self.is_dirty()
            }
            _ => self.is_dirty(),
        };
        if needs_confirm {
            self.pending = Some(p);
        } else {
            self.perform(p);
        }
    }
    /// Commit a staged single-side Open File: swap in the freshly loaded sheet,
    /// clear the replaced side's own undo stack, and re-hang every in-memory
    /// pair of the untouched side onto the new pairing by sheet index so its
    /// edits and history survive. Pairs whose replacement sheet has not loaded
    /// yet get an empty placeholder plus a pending fetch on next visit.
    pub(super) fn commit_open_side(
        &mut self,
        s: usize,
        path: std::path::PathBuf,
        preserve: bool,
        options: ReadOptions,
        loaded: Loaded,
    ) {
        let other = 1 - s;
        let other_index_of = |pair: &bc_engine_table::sheets::SheetPair| {
            if other == 0 {
                pair.left
            } else {
                pair.right
            }
        };
        // Collect the running session's in-memory pairs keyed by the untouched
        // side's sheet index. The active orphan placeholder side carries nothing.
        let mut old: Vec<(usize, super::view::Pair)> = Vec::new();
        if let Some(p) = self.pair.take() {
            if let Some(oj) = self.pairs.0.get(self.active_pair).and_then(other_index_of) {
                old.push((oj, p));
            }
        }
        for (i, p) in std::mem::take(&mut self.cached) {
            if let Some(oj) = self.pairs.0.get(i).and_then(other_index_of) {
                old.push((oj, p));
            }
        }
        self.names = loaded.names.clone();
        self.pairs = loaded.pairs.clone();
        self.states = vec![None; self.pairs.0.len()];
        self.summaries = vec![None; self.pairs.0.len()];
        self.grid.scroll = egui::Vec2::ZERO;
        let side_index_of = |pair: &bc_engine_table::sheets::SheetPair| {
            if s == 0 {
                pair.left
            } else {
                pair.right
            }
        };
        for (oj, mut pair) in old {
            let Some(k) = self
                .pairs
                .0
                .iter()
                .position(|np| other_index_of(np) == Some(oj))
            else {
                continue;
            };
            pair.history[s] = EditStack::new();
            if preserve && k == loaded.active {
                // The worker compared the new side against this very in-memory
                // sheet; finish the surgical install below.
                pair.pending_side = None;
                self.pair = Some(pair);
            } else {
                // Park it: show an honest empty placeholder until its own
                // replacement sheet loads on the next visit.
                pair.map.replace_sheet(side(s), Sheet::default());
                pair.pending_side = side_index_of(&self.pairs.0[k]).map(|idx| (s, idx));
                self.cached.insert(k, pair);
            }
        }
        self.files[s].path = Some(path.clone());
        self.files[s].target = None;
        self.files[s].clipboard_dirty = false;
        self.files[s].readonly = std::fs::metadata(&path)
            .map(|m| m.permissions().readonly())
            .unwrap_or(false);
        self.files[s].byte_len = loaded.bytes[s];
        self.files[s].load_time = loaded.times[s];
        self.options[s] = options;
        self.active_pair = loaded.active.min(self.pairs.0.len().saturating_sub(1));
        self.default_rules.columns.0.clear();
        let super::worker::Prepared {
            left,
            right,
            output,
            cache,
        } = loaded.prepared;
        let result = if self.pair.is_some() {
            let p = self.pair.as_mut().unwrap();
            p.map
                .replace_store(side(s), if s == 0 { left } else { right });
            // The worker aligned against the preserved in-memory sheet; if the
            // active pair was re-hung from another sheet (edited elsewhere),
            // realign in the background instead of installing a mismatch.
            let other_revision = if other == 0 {
                p.map.left.borrow().revision()
            } else {
                p.map.right.borrow().revision()
            };
            let prepared_revision = if other == 0 {
                output.revisions.0
            } else {
                output.revisions.1
            };
            if other_revision == prepared_revision {
                self.install_aligned(output, cache)
            } else {
                drop(cache);
                self.recompare();
                Ok(())
            }
        } else {
            self.install(super::worker::Prepared {
                left,
                right,
                output,
                cache,
            })
        };
        if let Err(e) = result {
            self.message = e.to_string();
        }
    }
    pub(super) fn perform(&mut self, p: Pending) {
        self.editor = None; // request/confirmation has resolved any dirty draft first.
        match p {
            Pending::Close => {
                self.cancel();
                self.action = Some(TableAction::Close);
            }
            Pending::Exit => {
                self.cancel();
                self.action = Some(TableAction::Exit);
            }
            Pending::Reload => {
                self.reload();
            }
            Pending::Open(s, path) => {
                // Route the new source through the format registry: a different
                // file type gets its matching table options instead of
                // inheriting the previous file's parse settings.
                let new_options = match super::formats::load_registry()
                    .and_then(|r| super::formats::options(&path, &r))
                {
                    Ok(o) => o,
                    Err(e) => {
                        self.message = e.to_string();
                        return;
                    }
                };
                // Stage the load: the running session (including the other
                // side's edits) survives a failure; commit happens in poll().
                let other = 1 - s;
                let mut memory = [None, None];
                // Only a live two-sided pair has an in-memory other sheet worth
                // preserving; an orphan tab's placeholder side reloads fresh.
                let preserve = self.pair.is_some()
                    && self
                        .pairs
                        .0
                        .get(self.active_pair)
                        .and_then(|pair| if other == 0 { pair.left } else { pair.right })
                        .is_some();
                if preserve {
                    let p = self.pair.as_ref().unwrap();
                    let sheet = if other == 0 {
                        p.map.left.borrow().clone()
                    } else {
                        p.map.right.borrow().clone()
                    };
                    let index = self
                        .pairs
                        .0
                        .get(self.active_pair)
                        .and_then(|pair| if other == 0 { pair.left } else { pair.right })
                        .unwrap_or(0);
                    memory[other] = Some(super::worker::MemorySheet {
                        sheet,
                        names: self.names[other].clone(),
                        index,
                    });
                }
                self.cancel();
                let mut rules = self.default_rules.clone();
                rules.columns.0.clear();
                let mut paths = self.paths();
                paths[s] = Some(path.clone());
                let mut options = self.options.clone();
                options[s] = new_options.clone();
                self.staged = Some(Staged::Open(s, path, preserve, new_options));
                self.worker = Some(Worker::spawn(Box::new(LoadJob {
                    paths,
                    options,
                    rules,
                    pair: None,
                    left_source: self.left_source,
                    memory,
                    fixed_pairs: None,
                })));
                self.message = "Opening file in background\u{2026}".into();
            }
            Pending::Clipboard(s, text) => {
                let byte_len = text.len() as u64;
                // A workbook side carries no meaningful text delimiters; detect
                // the actual delimiter of the clipboard text instead of
                // squeezing a TSV paste into one comma column.
                let mut options = self.options[s].clone();
                let workbook_side = self.files[s].path.as_ref().is_some_and(|p| {
                    p.extension().is_some_and(|e| {
                        matches!(e.to_string_lossy().to_lowercase().as_str(), "xlsx" | "xlsm")
                    })
                });
                if workbook_side {
                    options.format.detect_delimiter = true;
                }
                super::worker::pin_header_mode(&mut options);
                let parsed = read_text(IoCursor::new(text), &options, &NeverCancel);
                match parsed {
                    Err(e) => self.message = e.to_string(),
                    Ok(parsed) if self.pair.is_some() => {
                        // Single-side replacement that keeps the untouched side's
                        // sheets: every in-memory pair (active or cached) is
                        // re-hung onto the new pairing by that side's sheet
                        // index, with edits and undo history intact.
                        self.cancel();
                        let other = 1 - s;
                        let sheet_name = parsed.sheet.name.clone();
                        let other_of = |pair: &bc_engine_table::sheets::SheetPair| {
                            if other == 0 {
                                pair.left
                            } else {
                                pair.right
                            }
                        };
                        let prev_other = self.pairs.0.get(self.active_pair).and_then(other_of);
                        let mut old: Vec<(usize, super::view::Pair)> = Vec::new();
                        if let Some(p) = self.pair.take() {
                            if let Some(oj) = prev_other {
                                old.push((oj, p));
                            }
                        }
                        for (i, p) in std::mem::take(&mut self.cached) {
                            if let Some(oj) = self.pairs.0.get(i).and_then(other_of) {
                                old.push((oj, p));
                            }
                        }
                        self.names[s] = vec![sheet_name];
                        let (pairs, active) = super::worker::pair_for_replacement(
                            &self.names[s].clone(),
                            &self.names[other].clone(),
                            s,
                            prev_other.unwrap_or(0),
                            self.left_source,
                        );
                        self.pairs = pairs;
                        self.states = vec![None; self.pairs.0.len()];
                        self.summaries = vec![None; self.pairs.0.len()];
                        self.grid.scroll = egui::Vec2::ZERO;
                        let mut active_pair = None;
                        for (oj, mut pair) in old {
                            let Some(k) =
                                self.pairs.0.iter().position(|np| other_of(np) == Some(oj))
                            else {
                                continue;
                            };
                            pair.history[s] = EditStack::new();
                            if k == active {
                                // replace_sheet keeps the other side's sheet
                                // object and history valid, drops anchors into
                                // replaced content and re-matches the mapping
                                // when the layout changed.
                                pair.pending_side = None;
                                active_pair = Some(pair);
                            } else {
                                // The clipboard has exactly one sheet: every
                                // other tab becomes a placeholder-side orphan.
                                pair.map.replace_sheet(side(s), Sheet::default());
                                pair.pending_side = None;
                                self.cached.insert(k, pair);
                            }
                        }
                        self.files[s].path = None;
                        self.files[s].target = None;
                        self.files[s].readonly = false;
                        self.files[s].clipboard_dirty = true;
                        self.files[s].byte_len = byte_len;
                        self.files[s].load_time = std::time::Duration::ZERO;
                        self.active_pair = active;
                        match active_pair {
                            Some(pair) => {
                                pair.map.replace_sheet(side(s), parsed.sheet);
                                self.pair = Some(pair);
                                self.recompare();
                            }
                            None => {
                                // The active target's other sheet was never in
                                // memory: load it in the background against the
                                // clipboard sheet held by the worker.
                                let mut memory = [None, None];
                                memory[s] = Some(super::worker::MemorySheet {
                                    sheet: bc_engine_table::SheetStore::Dense(parsed.sheet),
                                    names: self.names[s].clone(),
                                    index: 0,
                                });
                                let mut rules = self.default_rules.clone();
                                rules.columns.0.clear();
                                self.staged = Some(Staged::PairSide(other));
                                self.worker = Some(Worker::spawn(Box::new(LoadJob {
                                    paths: self.paths(),
                                    options: self.options.clone(),
                                    rules,
                                    pair: self.pairs.0.get(active).cloned(),
                                    left_source: self.left_source,
                                    memory,
                                    fixed_pairs: Some(self.pairs.clone()),
                                })));
                            }
                        }
                        self.message = "Clipboard loaded; use Save As to write it".into();
                    }
                    Ok(parsed) => {
                        let other = Sheet::default();
                        self.cancel();
                        let files = self.files.clone();
                        let options = self.options.clone();
                        let (l, r) = if s == 0 {
                            (parsed.sheet, other)
                        } else {
                            (other, parsed.sheet)
                        };
                        *self = Self::from_sheets(l, r, self.default_rules.clone());
                        self.files = files;
                        self.options = options;
                        self.files[s].path = None;
                        self.files[s].target = None;
                        self.files[s].clipboard_dirty = true;
                        self.files[s].byte_len = byte_len;
                        self.message = "Clipboard loaded; use Save As to write it".into();
                    }
                }
            }
            Pending::Text => {
                if let [Some(left), Some(right)] = self.paths() {
                    self.cancel();
                    self.action = Some(TableAction::Text { left, right });
                } else {
                    self.message = "Save clipboard sources before opening Text Compare".into();
                }
            }
            Pending::Hex => {
                if let [Some(left), Some(right)] = self.paths() {
                    self.cancel();
                    self.action = Some(TableAction::Hex { left, right });
                } else {
                    self.message = "Save clipboard sources before opening Hex Compare".into();
                }
            }
            Pending::Settings(draft) => {
                let reload = draft.options != self.options
                    || draft.paths
                        != self
                            .paths()
                            .map(|p| p.map_or(String::new(), |p| p.display().to_string()))
                    || draft.pairs != self.pairs
                    || draft.left_source != self.left_source;
                if reload {
                    // Stage the reload: paths/options/rules/pairs commit only
                    // after the worker succeeds; a failure keeps the session.
                    self.cancel();
                    let paths = draft
                        .paths
                        .iter()
                        .map(|p| (!p.is_empty()).then(|| std::path::PathBuf::from(p)))
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap_or([None, None]);
                    let active = self.active_pair.min(draft.pairs.0.len().saturating_sub(1));
                    // Manual sheet pairings index into the CURRENT sources; a
                    // changed path invalidates them, so new sources are re-paired
                    // by name and the active tab comes from the loaded result.
                    let paths_changed = draft.paths
                        != self
                            .paths()
                            .map(|p| p.map_or(String::new(), |p| p.display().to_string()));
                    let job = LoadJob {
                        paths,
                        options: draft.options.clone(),
                        rules: draft.rules.clone(),
                        pair: if paths_changed {
                            None
                        } else {
                            draft.pairs.0.get(active).cloned()
                        },
                        left_source: draft.left_source,
                        memory: [None, None],
                        fixed_pairs: if paths_changed {
                            None
                        } else {
                            Some(draft.pairs.clone())
                        },
                    };
                    self.staged = Some(Staged::Settings(draft));
                    self.worker = Some(Worker::spawn(Box::new(job)));
                    self.message = "Reloading with new session settings\u{2026}".into();
                } else if let Some(p) = &self.pair {
                    // Rules-only change: validate before touching anything so a
                    // rejected draft cannot pollute the map or the defaults.
                    let valid = if draft.rules.columns.0.is_empty() {
                        Ok(())
                    } else {
                        draft.rules.validate(
                            p.map.left.borrow().column_count(),
                            p.map.right.borrow().column_count(),
                        )
                    };
                    match valid {
                        Ok(()) => {
                            *p.map.rules_mut() = draft.rules.clone();
                            self.default_rules = draft.rules.clone();
                            self.default_rules.left_is_source = draft.left_source;
                            for s in 0..2 {
                                self.files[s].readonly = draft.readonly[s];
                            }
                            self.recompare();
                        }
                        Err(e) => self.message = e.to_string(),
                    }
                }
            }
        }
    }
    pub(super) fn swap(&mut self) {
        self.cancel();
        self.files.swap(0, 1);
        self.options.swap(0, 1);
        self.names.swap(0, 1);
        for pair in &mut self.pairs.0 {
            std::mem::swap(&mut pair.left, &mut pair.right);
        }
        for p in self.pair.iter_mut().chain(self.cached.values_mut()) {
            // Mirrors sheets/rules/anchors/state; captured edits restore across
            // the swap through the shared orientation parity.
            p.map.swap_sides();
            p.history.swap(0, 1);
            // A pending replacement sheet now belongs to the other side; the
            // sheet index stays valid because the name lists swapped in place.
            if let Some((side, _)) = &mut p.pending_side {
                *side = 1 - *side;
            }
        }
        self.default_rules.transpose();
        // Per-pair summaries mention left/right-only counts; mirror them so the
        // sheet dots and Table Compare Info stay correct without rescanning.
        for summary in self.summaries.iter_mut().flatten() {
            for s in summary.iter_mut() {
                std::mem::swap(&mut s.left_only, &mut s.right_only);
            }
        }
        // An uncommitted cell draft keeps its source coordinates into the same
        // physical sheet, which now lives on the other side.
        if let Some(e) = &mut self.editor {
            e.side = 1 - e.side;
        }
        // Find hits are side-local; the sides' contents just exchanged.
        self.find.last = None;
        self.grid.active = 1 - self.grid.active;
        if self.pair.as_ref().is_some_and(|p| p.pending_side.is_some()) {
            // The active pair is a placeholder: load the pending side for the
            // new orientation instead of comparing the placeholder.
            self.load_pending_pair();
        } else if self.pair.is_some() {
            // The transposed snapshot is the mirrored comparison, not stale;
            // only the derived cache (statistics, bins) needs a rebuild.
            self.project();
            self.message = "Swapped sides".into();
        } else {
            self.reload();
        }
    }
    pub(super) fn dialogs(&mut self, ctx: &egui::Context, prefs: &Prefs) {
        let mut commit = false;
        let mut cancel = false;
        // The draft stays intact while a destructive-action confirmation is open.
        if self.pending.is_none() && self.dialog.is_none() {
            if let (Some(editor), Some(rect)) = (&mut self.editor, self.grid.edit_rect) {
                egui::Area::new(egui::Id::unique("table_modify"))
                    .fixed_pos(rect.min)
                    .order(egui::Order::Foreground)
                    .show(ctx, |ui| {
                        let p = crate::chrome::theme::palette(ui.ctx());
                        egui::Frame::NONE
                            .fill(p.surface)
                            .stroke(egui::Stroke::new(1.0, p.line))
                            .corner_radius(crate::chrome::theme::RADIUS_LG)
                            .shadow(crate::chrome::theme::shadow_popup())
                            .inner_margin(8)
                            .show(ui, |ui| {
                                commit |= ui.input_mut(|i| {
                                    i.consume_key(egui::Modifiers::COMMAND, egui::Key::Enter)
                                });
                                let width = rect.width().max(260.0);
                                let response = ui.add(
                                    egui::TextEdit::multiline(&mut editor.text)
                                        .id_salt("table_cell_text")
                                        .desired_width(width)
                                        .desired_rows(1),
                                );
                                if editor.focus {
                                    response.request_focus();
                                    editor.focus = false;
                                }
                                ui.horizontal(|ui| {
                                    use crate::chrome::widgets;
                                    commit |= widgets::primary_button(ui, "Commit (Ctrl+Enter)")
                                        .clicked();
                                    cancel = widgets::button(ui, "Cancel edit").clicked();
                                });
                                cancel |= ui.input_mut(|i| {
                                    i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                                });
                            });
                    });
            }
        }
        if commit {
            if let Err(e) = self.commit_edit() {
                self.message = e.to_string();
            }
        } else if cancel {
            if let Some(editor) = self.editor.take() {
                self.focus_grid = Some(editor.side);
                self.key_focus = true;
            }
        }
        let execute = self.dialog.take().and_then(|dialog| {
            use crate::chrome::native_dialog::{side_title, Kind, Request};
            let request = match dialog.mode {
                FileMode::Open(s) => {
                    Request::new(Kind::OpenFile, side_title("Open File on File System", s))
                }
                FileMode::Save(s) => Request::new(Kind::SaveFile, side_title("Save File As", s)),
                FileMode::Export(s) => Request::new(Kind::SaveFile, side_title("Export as CSV", s))
                    .filter("CSV files (*.csv)", &["csv"]),
                FileMode::Summary(false) => Request::new(Kind::SaveFile, "Table Compare Report")
                    .filter("Text files (*.txt)", &["txt"]),
                FileMode::Summary(true) => Request::new(Kind::SaveFile, "Export Summary as CSV")
                    .filter("CSV files (*.csv)", &["csv"]),
            };
            let initial = match dialog.mode {
                FileMode::Open(s) | FileMode::Save(s) | FileMode::Export(s) => self.files[s]
                    .target
                    .clone()
                    .or_else(|| self.files[s].path.clone()),
                FileMode::Summary(_) => None,
            };
            let initial = if dialog.path.trim().is_empty() {
                initial
            } else {
                Some(std::path::PathBuf::from(dialog.path.trim()))
            };
            request
                .initial_opt(initial.as_deref())
                .show()
                .map(|path| (dialog.mode, path))
        });
        if let Some((mode, path)) = execute {
            let result = match mode {
                FileMode::Open(s) => {
                    self.request(Pending::Open(s, path));
                    Ok(())
                }
                FileMode::Save(s) => self.save_to(s, &path, prefs, false),
                FileMode::Export(s) => self.save_to(s, &path, prefs, true),
                FileMode::Summary(csv) => self
                    .pair
                    .as_ref()
                    .ok_or_else(|| invalid("Table is loading"))
                    .and_then(|p| {
                        p.map.export_summary(if csv {
                            SummaryFormat::Csv
                        } else {
                            SummaryFormat::Text
                        })
                    })
                    .and_then(|text| std::fs::write(path, text).map_err(Into::into)),
            };
            if let Err(e) = result {
                self.message = e.to_string();
            }
        }
        if self.show_statistics {
            let keys = self.keys_description();
            crate::chrome::window::Window::new("Table Compare Info")
                .open(&mut self.show_statistics)
                .show(ctx, |ui| {
                    for (i,summary) in self.summaries.iter().enumerate(){
                        if let Some(summary)=summary {
                            let mut s=summary[super::cache::layer_index(self.default_rules.layers)];
                            if self.default_rules.ignore_unimportant{s.same+=s.unimportant;s.unimportant=0;}
                            ui.label(format!("Sheet {}: {} same / {} unimportant / {} important / {} left-only / {} right-only / {} columns",i+1,s.same,s.unimportant,s.important,s.left_only,s.right_only,s.columns));
                        }else{ui.label(format!("Sheet {}: comparison pending",i+1));}
                    }
                    ui.label(format!("Keys: {keys}"));
                    if let Some(p) = &self.pair {
                        ui.label(format!(
                            "Current sheet: {:?}; duplicate keys {:?}",
                            p.cache.statistics,
                            p.map.duplicate_keys()
                        ));
                    }
                });
        }
        let mut decision = 0;
        if let Some(pending) = &self.pending {
            use crate::chrome::{
                controls::Role,
                modal::{self, Modal, Tone},
            };
            let title = match pending {
                Pending::Close => "Save table changes before closing?",
                Pending::Exit => "Save table changes before quitting?",
                Pending::Reload => "Save table changes before reloading?",
                Pending::Open(..) => "Save table changes before opening another file?",
                Pending::Clipboard(..) => "Save table changes before loading the clipboard?",
                _ => "Save table changes first?",
            };
            let items: Vec<modal::Item> = (0..2)
                .filter(|&s| {
                    self.files[s].clipboard_dirty
                        || self.pair.as_ref().is_some_and(|p| p.history[s].is_dirty())
                })
                .filter_map(|s| {
                    let badge = if s == 0 { "LEFT" } else { "RIGHT" };
                    self.files[s]
                        .path
                        .as_deref()
                        .map(|p| modal::file_item(badge, p))
                })
                .collect();
            decision = match Modal::new("table-unsaved", Tone::Warning, title)
                .message(
                    "This will replace the comparison and its edited sheets.                      Unsaved edits will be lost if you don't save them.",
                )
                .items(items)
                .action("Don't Save", Role::Destructive)
                .action("Cancel", Role::Secondary)
                .action("Save", Role::Primary)
                .cancel(1)
                .show(ctx)
            {
                Some(0) => 2,
                Some(1) => 3,
                Some(2) => 1,
                _ => 0,
            };
        }
        match decision {
            1 => {
                if let Err(e) = self.commit_edit() {
                    self.message = e.to_string();
                    return;
                }
                let mut saved = true;
                for s in 0..2 {
                    if self.files[s].clipboard_dirty
                        || self.pair.as_ref().is_some_and(|p| p.history[s].is_dirty())
                    {
                        match self.save(s, prefs) {
                            Ok(ok) => saved &= ok,
                            Err(e) => {
                                self.message = e.to_string();
                                saved = false;
                            }
                        }
                    }
                }
                // Cached sheet pairs are dropped only by full replacements
                // (Close/Exit/Reload/Settings/Text); single-side Open/Clipboard
                // re-hang and preserve them — but their REPLACED-side edits
                // would be discarded, and workbook sheets cannot be written
                // back, so never pretend a save succeeded for those.
                let drops_cached = matches!(
                    self.pending,
                    Some(Pending::Close)
                        | Some(Pending::Exit)
                        | Some(Pending::Reload)
                        | Some(Pending::Settings(_))
                        | Some(Pending::Text)
                );
                let replaced_side_cached_dirty = match &self.pending {
                    Some(Pending::Open(s, _)) | Some(Pending::Clipboard(s, _)) => {
                        self.cached.values().any(|p| p.history[*s].is_dirty())
                    }
                    _ => false,
                };
                if (drops_cached
                    && self
                        .cached
                        .values()
                        .any(|p| p.history.iter().any(bc_core::edit::EditStack::is_dirty)))
                    || replaced_side_cached_dirty
                {
                    self.message="Other edited workbook sheets cannot be written back; export each as CSV or cancel/discard".into();
                    saved = false;
                }
                if saved {
                    if let Some(p) = self.pending.take() {
                        self.perform(p);
                    }
                }
            }
            2 => {
                if let Some(p) = self.pending.take() {
                    self.perform(p);
                }
            }
            3 => {
                self.pending = None;
                if let Some(e) = &mut self.editor {
                    e.focus = true;
                }
            }
            _ => {}
        }
    }
}
