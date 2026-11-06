use super::{
    find::Action,
    grid::{self, Cursor, GridAction},
    view::{CellEditor, FileMode, PathDialog, Pending, TableAction, TableCompareView},
};
use bc_core::{align::AlignmentOverrides, cancel::NeverCancel, CoreError};
use bc_engine_table::{
    columns::{ComparisonColumn, Side},
    source::{export_csv, read_text, save_as, ReadOptions},
    summary::SummaryFormat,
    Cell,
};
use bc_session::prefs::Prefs;
use eframe::egui;
use std::{io::Cursor as IoCursor, path::Path};
fn side(n: usize) -> Side {
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
    fn source_position(&self, s: usize, cursor: Cursor) -> Result<(usize, usize), CoreError> {
        let p = self
            .pair
            .as_ref()
            .ok_or_else(|| invalid("Table is loading"))?;
        let row = grid::mapped(&p.map, cursor.row);
        let column = p
            .map
            .rules()
            .columns
            .0
            .get(cursor.column)
            .and_then(|c| c.index(side(s)))
            .ok_or_else(|| invalid("Column is a gap"))?;
        Ok((
            (if s == 0 { row.left } else { row.right })
                .ok_or_else(|| invalid("Row is a gap; copy the row first"))?,
            column,
        ))
    }
    pub(super) fn begin_edit(&mut self, s: usize, cursor: Cursor, replacement: Option<String>) {
        if self.files[s].readonly {
            self.message = "Editing disabled".into();
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
                        original_cell: sheet.cell(row, col).unwrap().clone(),
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
            if sheet.cell(editor.source.row, editor.source.column) != Some(&editor.original_cell)
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
        self.edited();
        Ok(())
    }
    pub(super) fn grid_action(&mut self, action: GridAction) {
        let (s, cursor, extend, edit) = match action {
            GridAction::Select(s, c, e) => (s, c, e, false),
            GridAction::Edit(s, c) => (s, c, false, true),
        };
        if let Some((from, row)) = self.align_with.take() {
            if from != s {
                if let Ok((to, _)) = self.source_position(s, cursor) {
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
        let current = self.grid.cursor;
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
                    p.map.cell_importance(row, c)
                        != Some(bc_engine_table::compare::Importance::Same)
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
                column: current.column,
            })
        };
        if let Some(target) = target {
            self.goto_cell(target);
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
    fn clipboard_text(&self) -> Option<String> {
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
                        cell.style = sheet.cell(r + row, c + col).and_then(|cell| cell.style);
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
        let (row, col) = self.source_position(s, cursor).unwrap_or((0, 0));
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
                    1 => sheet.insert_row((row + 1).min(sheet.row_count()), Vec::new())?,
                    2 => sheet.remove_row_indices(&selected_rows)?,
                    3 | 4 => {
                        let at = (col + usize::from(operation == 4)).min(sheet.column_count());
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
                            cursor.column.min(rules.columns.0.len()),
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
        self.edited();
        Ok(())
    }
    pub(super) fn edit_menu(&mut self, ui: &mut egui::Ui) {
        ui.menu_button("Edit", |ui| {
            if ui.button("Undo").clicked() {
                self.undo(false);
                ui.close();
            }
            if ui.button("Redo").clicked() {
                self.undo(true);
                ui.close();
            }
            if ui.button("Modify cell (F2)").clicked() {
                self.begin_edit(self.grid.active, self.grid.cursor, None);
                ui.close();
            }
            if ui.button("Copy to Other Side").clicked() {
                self.copy(false);
                ui.close();
            }
            if ui.button("Copy Row to Other Side").clicked() {
                self.copy(true);
                ui.close();
            }
            if ui.button("Cut").clicked() {
                if let Some(text) = self.clipboard_text() {
                    ui.ctx().copy_text(text);
                    self.clear_selected();
                }
                ui.close();
            }
            if ui.button("Copy").clicked() {
                if let Some(text) = self.clipboard_text() {
                    ui.ctx().copy_text(text);
                }
                ui.close();
            }
            if ui.button("Paste").clicked() {
                self.paste_requested = true;
                self.focus_grid = Some(self.grid.active);
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                ui.close();
            }
            if ui.button("Delete").clicked() {
                self.clear_selected();
                ui.close();
            }
            for (name, op) in [
                ("Insert Row Before", 0),
                ("Insert Row After", 1),
                ("Delete Row", 2),
                ("Insert Column Before", 3),
                ("Insert Column After", 4),
                ("Delete Column", 5),
                ("Sort descending", 6),
                ("Sort ascending", 7),
            ] {
                let response = ui.button(name);
                let response = if matches!(op, 6 | 7) {
                    response.on_hover_text("Stable sort by selected visible columns. Multi-row selection: selected visible rows only; otherwise: whole active table.")
                } else {
                    response
                };
                if response.clicked() {
                    if let Err(e) = self.structure(op) {
                        self.message = e.to_string();
                    }
                    ui.close();
                }
            }
            if ui.button("Select All").clicked() {
                self.select_all();
                ui.close();
            }
            if ui.button("Align With — click opposite row").clicked() {
                if let Ok((r, _)) = self.source_position(self.grid.active, self.grid.cursor) {
                    self.align_with = Some((self.grid.active, r));
                }
                ui.close();
            }
            if ui.button("Clear alignment overrides").clicked() {
                if let Some(p) = &mut self.pair {
                    if let Ok(e) =
                        p.map
                            .edit_sheet(side(self.grid.active), "Clear alignment", |_, _, a| {
                                a.clear();
                                Ok(())
                            })
                    {
                        p.history[self.grid.active].push(e);
                        self.edited();
                    }
                }
                ui.close();
            }
        });
    }
    fn select_all(&mut self) {
        if let Some(p) = &self.pair {
            self.grid.anchor = Some(Cursor::default());
            self.grid.cursor = Cursor {
                row: p.cache.total.saturating_sub(1),
                column: p.map.column_count().saturating_sub(1),
            };
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
                let previous = self
                    .find
                    .last
                    .as_ref()
                    .map(|(r, c, span)| (*r, *c, span.start));
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
                        && self.find.last.as_ref() != Some(&(hit.0, hit.1, hit.2.clone()))
                    {
                        continue;
                    }
                    let span = hit.2.clone();
                    let text = pattern.replace_all_if(&hit.5, &self.find.replacement, |r| {
                        self.find.accepts(&hit.5, r.clone())
                            && (matches!(action, Action::All) || r == span)
                    });
                    replacements.insert((hit.3, hit.4), text);
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
        let focused =
            ctx.memory(|m| m.has_focus(egui::Id::unique(("table_grid_focus", self.grid.active))));
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
                } if modifiers.command => self.find.open = true,
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
                    egui::Key::F3 => {
                        self.find.open = true;
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
                    | egui::Key::Tab => {
                        let old = self.grid.cursor;
                        if modifiers.shift {
                            self.grid.anchor.get_or_insert(old);
                        } else {
                            self.grid.anchor = None;
                        }
                        let mut c = old;
                        match key {
                            egui::Key::ArrowDown => c.row += 1,
                            egui::Key::ArrowUp => c.row = c.row.saturating_sub(1),
                            egui::Key::ArrowLeft => c.column = c.column.saturating_sub(1),
                            _ => c.column += 1,
                        }
                        if let Some(p) = &self.pair {
                            c.row = c.row.min(p.cache.total.saturating_sub(1));
                            c.column = c.column.min(p.map.column_count().saturating_sub(1));
                        }
                        self.grid.cursor = c;
                    }
                    _ => {}
                },
                _ => {}
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
        if self.editor.as_ref().is_some_and(|e| e.side == s) {
            self.commit_edit()?;
        }
        self.cancel();
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
                export_csv(&sheet, &mut f)?;
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
            save_as(&sheet, path)?;
            p.history[s].mark_saved();
            self.files[s].clipboard_dirty = false;
            self.files[s].target = Some(path.into());
        }
        drop(sheet);
        if p.map.is_stale() {
            self.edited_at = Some(std::time::Instant::now());
        }
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
        let destroys = match &p {
            Pending::Settings(d) => {
                d.options != self.options
                    || d.pairs != self.pairs
                    || d.paths
                        != self
                            .paths()
                            .map(|p| p.map_or(String::new(), |p| p.display().to_string()))
                    || d.left_source != self.left_source
            }
            _ => true,
        };
        if (destroys || self.editor.as_ref().is_some_and(CellEditor::changed)) && self.is_dirty() {
            self.pending = Some(p);
        } else {
            self.perform(p);
        }
    }
    pub(super) fn perform(&mut self, p: Pending) {
        self.editor = None; // request/confirmation has resolved any dirty draft first.
        match p {
            Pending::Home => {
                self.cancel();
                self.action = Some(TableAction::Home);
            }
            Pending::Exit => {
                self.cancel();
                self.action = Some(TableAction::Exit);
            }
            Pending::Reload => {
                self.names = [Vec::new(), Vec::new()];
                self.reload();
            }
            Pending::Open(s, path) => {
                self.files[s].path = Some(path);
                self.files[s].target = None;
                self.names = [Vec::new(), Vec::new()];
                self.reload();
            }
            Pending::Clipboard(s, text) => {
                let parsed = read_text(IoCursor::new(text), &self.options[s], &NeverCancel);
                match parsed {
                    Ok(parsed) => {
                        let other = self
                            .pair
                            .as_ref()
                            .map(|p| {
                                if s == 0 {
                                    p.map.right.borrow().clone()
                                } else {
                                    p.map.left.borrow().clone()
                                }
                            })
                            .unwrap_or_default();
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
                        self.message = "Clipboard loaded; use Save As to write it".into();
                    }
                    Err(e) => self.message = e.to_string(),
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
            Pending::Settings(draft) => {
                let reload = draft.options != self.options
                    || draft.paths
                        != self
                            .paths()
                            .map(|p| p.map_or(String::new(), |p| p.display().to_string()))
                    || draft.pairs != self.pairs
                    || draft.left_source != self.left_source;
                if draft.paths
                    != self
                        .paths()
                        .map(|p| p.map_or(String::new(), |p| p.display().to_string()))
                {
                    self.names = [Vec::new(), Vec::new()];
                }
                self.default_rules = draft.rules.clone();
                self.default_rules.left_is_source = draft.left_source;
                self.options = draft.options;
                self.left_source = draft.left_source;
                for s in 0..2 {
                    self.files[s].readonly = draft.readonly[s];
                    self.files[s].path =
                        (!draft.paths[s].is_empty()).then(|| draft.paths[s].clone().into());
                }
                if reload {
                    self.cancel();
                    self.cached.clear();
                    self.pair = None;
                    self.pairs = draft.pairs;
                    self.states = vec![None; self.pairs.0.len()];
                    self.summaries = vec![None; self.pairs.0.len()];
                    self.active_pair = self.active_pair.min(self.pairs.0.len().saturating_sub(1));
                    self.worker = Some(super::worker::Worker::spawn(Box::new(
                        super::worker::LoadJob {
                            paths: self.paths(),
                            options: self.options.clone(),
                            rules: self.default_rules.clone(),
                            pair: self.pairs.0.get(self.active_pair).cloned(),
                            left_source: self.left_source,
                        },
                    )));
                } else if let Some(p) = &self.pair {
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
                            *p.map.rules_mut() = draft.rules;
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
            std::mem::swap(&mut p.map.left, &mut p.map.right);
            p.history.swap(0, 1);
            for c in &mut p.map.rules_mut().columns.0 {
                std::mem::swap(&mut c.left, &mut c.right);
            }
            let mut anchors = AlignmentOverrides::default();
            for &(l, r) in p.map.overrides().pairs() {
                let _ = anchors.insert(r, l);
            }
            p.map.set_overrides(anchors);
        }
        self.grid.active = 1 - self.grid.active;
        self.recompare();
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
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
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
                                commit |= ui.button("Commit (Ctrl+Enter)").clicked();
                                cancel = ui.button("Cancel edit").clicked();
                            });
                            cancel |= ui.input(|i| i.key_pressed(egui::Key::Escape));
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
            }
        }
        let mut execute = None;
        let mut cancel = false;
        if let Some(dialog) = &mut self.dialog {
            egui::Window::new("Table file path")
                .default_width(650.0)
                .show(ctx, |ui| {
                    ui.text_edit_singleline(&mut dialog.path);
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() {
                            execute = Some((dialog.mode, dialog.path.clone()));
                        }
                        cancel = ui.button("Cancel path").clicked();
                    });
                });
        }
        if cancel {
            self.dialog = None;
        }
        if let Some((mode, path)) = execute {
            self.dialog = None;
            let path = std::path::PathBuf::from(path);
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
            egui::Window::new("Table Compare Info")
                .open(&mut self.show_statistics)
                .show(ctx, |ui| {
                    for (i,summary) in self.summaries.iter().enumerate(){
                        if let Some(summary)=summary {
                            let mut s=summary[super::cache::layer_index(self.default_rules.layers)];
                            if self.default_rules.ignore_unimportant{s.same+=s.unimportant;s.unimportant=0;}
                            ui.label(format!("Sheet {}: {} same / {} unimportant / {} important / {} left-only / {} right-only / {} columns",i+1,s.same,s.unimportant,s.important,s.left_only,s.right_only,s.columns));
                        }else{ui.label(format!("Sheet {}: comparison pending",i+1));}
                    }
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
        if self.pending.is_some() {
            egui::Window::new("Unsaved table changes").collapsible(false).show(ctx,|ui|{
            ui.label("This operation may replace the comparison and its edited sheets. Save, discard, or cancel.");ui.horizontal(|ui|{if ui.button("Save changed files").clicked(){decision=1;}
                if ui.button("Discard").clicked(){decision=2;}
                if ui.button("Cancel").clicked(){decision=3;}});
        });
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
                if self
                    .cached
                    .values()
                    .any(|p| p.history.iter().any(bc_core::edit::EditStack::is_dirty))
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
