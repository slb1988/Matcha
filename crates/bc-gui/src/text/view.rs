//! 双窗格控制器：后台比较、命令、保存协议和弹窗；绘制只访问可见行。
#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
use super::{
    editor::{Caret, Editor},
    find_panel::{FindAction, FindPanel},
    nav_bar::NavBar,
    projection::{DisplayFilter, DisplayRow, Projection},
    settings_panel::SettingsPanel,
    status_bar,
    worker::Worker,
};
use bc_core::{
    align::AlignmentOverrides,
    encoding::{BomPolicy, Encoding},
    line::{LineEdit, MaterializeContext},
    CoreError, Side,
};
use bc_engine_text::{
    importance::{CompiledComparison, Importance},
    inline::InlineDiff,
    jobs::{Align2Job, Realign2Job},
    map::{DifferenceCounts, Section, TextMapRow},
    rules::TextRules,
    TextContent, TextMap,
};
use bc_formats::{registry::FormatRegistry, FormatKind};
use bc_session::prefs::Prefs;
use eframe::egui;
use std::{
    collections::HashMap,
    ops::Range,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Default, Debug, Clone, Copy)]
pub struct PaneStats {
    pub rows_drawn: usize,
}
#[derive(Debug, Clone)]
pub enum ViewAction {
    Home,
    Exit,
    CompareParentFolders { left: PathBuf, right: PathBuf },
}
#[derive(Clone)]
enum Pending {
    Home,
    Exit,
    Reload(usize),
    Open(usize, PathBuf),
    Clipboard(usize, String),
    Encoding(usize, Encoding),
}
#[derive(Clone, Copy)]
enum FileMode {
    Open,
    SaveAs,
    Target,
}
struct FileDialog {
    side: usize,
    mode: FileMode,
    path: String,
}
#[derive(Default)]
pub struct FileInfo {
    pub path: Option<PathBuf>,
    pub target: Option<PathBuf>,
    pub byte_len: usize,
    pub load_time: Duration,
    pub metadata_dirty: bool,
}
pub struct TextCompareView {
    pub map: TextMap,
    pub files: [FileInfo; 2],
    pub editors: [Editor; 2],
    pub last_stats: PaneStats,
    pub(crate) active: usize,
    pub(crate) current_row: usize,
    pub(crate) scroll: egui::Vec2,
    pub(crate) stacked: bool,
    pub(crate) projection: Projection,
    pub(crate) inline: HashMap<usize, InlineDiff>,
    pub(crate) comparison: CompiledComparison,
    pub(crate) dragging: Option<usize>,
    pub(crate) pane_rects: [egui::Rect; 2],
    pub(crate) visible: Range<usize>,
    pub(crate) align_with: Option<(usize, usize)>,
    pub(crate) show_whitespace: bool,
    pub(crate) show_syntax: bool,
    pub(crate) show_numbers: bool,
    pub(crate) font_size: f32,
    pub(crate) content_width: f32,
    pending_scroll: Option<usize>,
    filter: DisplayFilter,
    context: usize,
    expanded: Vec<Range<usize>>,
    nav: NavBar,
    pub(crate) sections: Vec<Section>,
    counts: DifferenceCounts,
    registry: FormatRegistry,
    worker: Option<Worker>,
    dirty: Option<Range<usize>>,
    edited_at: Option<Instant>,
    pub message: String,
    find: FindPanel,
    settings: SettingsPanel,
    formats: super::format_panel::FormatPanel,
    pending: Option<Pending>,
    file_dialog: Option<FileDialog>,
    clipboard_side: Option<usize>,
    action: Option<ViewAction>,
    goto_line: usize,
    goto_column: usize,
    recent: Vec<PathBuf>,
}
impl TextCompareView {
    pub fn open(left: &Path, right: &Path) -> Result<Self, CoreError> {
        let registry = match bc_formats::registry::user_formats_dir() {
            Ok(path) => FormatRegistry::load_user_dir(&path)
                .map_err(|e| CoreError::Invalid(format!("user formats: {e}")))?,
            Err(_) => FormatRegistry::with_builtins(),
        };
        let (l, lf) = load(left, &registry)?;
        let (r, rf) = load(right, &registry)?;
        let mut view = Self::from_contents(l, r, registry);
        view.files = [lf, rf];
        view.recent = vec![left.to_path_buf(), right.to_path_buf()];
        Ok(view)
    }
    pub fn from_contents(left: TextContent, right: TextContent, registry: FormatRegistry) -> Self {
        let rules = TextRules::default();
        let mut view = Self {
            comparison: CompiledComparison::new(&rules.comparison).expect("default rules"),
            map: TextMap::new(left, right, None, rules),
            files: Default::default(),
            editors: Default::default(),
            last_stats: Default::default(),
            active: 0,
            current_row: 0,
            scroll: egui::Vec2::ZERO,
            stacked: false,
            projection: Default::default(),
            inline: HashMap::new(),
            dragging: None,
            pane_rects: [egui::Rect::NOTHING; 2],
            visible: 0..0,
            align_with: None,
            show_whitespace: false,
            show_syntax: true,
            show_numbers: true,
            font_size: 13.0,
            content_width: 0.0,
            pending_scroll: None,
            filter: DisplayFilter::All,
            context: 3,
            expanded: Vec::new(),
            nav: Default::default(),
            sections: Vec::new(),
            counts: Default::default(),
            registry,
            worker: None,
            dirty: None,
            edited_at: None,
            message: String::new(),
            find: Default::default(),
            settings: Default::default(),
            formats: Default::default(),
            pending: None,
            file_dialog: None,
            clipboard_side: None,
            action: None,
            goto_line: 1,
            goto_column: 1,
            recent: Vec::new(),
        };
        view.rebuild_projection();
        view.recompare();
        view
    }
    pub fn content(&self, side: usize) -> &TextContent {
        if side == 0 {
            &self.map.left
        } else {
            &self.map.right
        }
    }
    pub fn content_mut(&mut self, side: usize) -> &mut TextContent {
        if side == 0 {
            &mut self.map.left
        } else {
            &mut self.map.right
        }
    }
    pub fn is_ready(&self) -> bool {
        self.worker.is_none() && !self.map.is_stale() && self.edited_at.is_none()
    }
    pub fn is_dirty(&self) -> bool {
        (0..2).any(|s| self.side_dirty(s))
    }
    fn side_dirty(&self, s: usize) -> bool {
        self.content(s).is_dirty() || self.files[s].metadata_dirty
    }
    pub fn row_count(&self) -> usize {
        if self.map.is_stale() {
            self.map
                .left
                .lines
                .borrow()
                .len()
                .max(self.map.right.lines.borrow().len())
                + 1
        } else {
            self.map.row_count() + 1
        }
    }
    pub fn top_row(&self, row_height: f32) -> usize {
        (self.scroll.y / row_height).floor().max(0.0) as usize
    }
    pub fn scroll_to_row(&mut self, row: usize) {
        self.pending_scroll = Some(row);
    }
    pub fn request_home(&mut self) {
        self.request(Pending::Home);
    }
    pub fn request_exit(&mut self) {
        self.request(Pending::Exit);
    }
    pub fn paths(&self) -> [Option<PathBuf>; 2] {
        [self.files[0].path.clone(), self.files[1].path.clone()]
    }
    pub fn index_bytes(&self) -> usize {
        (self.map.left.lines.borrow().len() + self.map.right.lines.borrow().len())
            * std::mem::size_of::<bc_core::line::Line>()
    }
    pub fn poll(&mut self) {
        if let Some(result) = self.worker.as_mut().and_then(Worker::poll) {
            self.worker = None;
            match result {
                Ok(output) => {
                    if self.map.set_alignment(output).is_ok() {
                        self.edited_at = None;
                        self.dirty = None;
                        self.after_alignment();
                        self.message = "Comparison complete".into();
                    }
                }
                Err(bc_jobs::JobError::Cancelled) => {}
                Err(e) => {
                    self.message = e.to_string();
                    self.edited_at = None;
                }
            }
        }
        if self.worker.is_none()
            && self
                .edited_at
                .is_some_and(|t| t.elapsed() >= Duration::from_millis(350))
        {
            let job = Realign2Job::from_map(
                &self.map,
                self.dirty.clone().unwrap_or(0..self.map.row_count()),
            );
            self.worker = Some(Worker::spawn(Box::new(job)));
        }
    }
    pub fn recompare(&mut self) {
        self.cancel_join();
        self.edited_at = None;
        self.dirty = None;
        self.inline.clear();
        self.worker = Some(Worker::spawn(Box::new(Align2Job::from_map(&self.map))));
        self.message = "Comparing…".into();
    }
    fn cancel_join(&mut self) {
        drop(self.worker.take());
    }
    fn after_alignment(&mut self) {
        self.inline.clear();
        self.expanded.clear();
        self.comparison =
            CompiledComparison::new(&self.map.rules.comparison).expect("validated session rules");
        self.sections = self.map.sections();
        self.counts = self.map.statistics();
        self.nav.rebuild(&self.map);
        self.rebuild_projection();
        self.current_row = self.row_for_line(self.active, self.editors[self.active].caret.line);
    }
    pub(crate) fn edited(&mut self, old_line_count: usize) {
        if let Some(worker) = &self.worker {
            worker.cancel();
        }
        let new_count = self.content(self.active).lines.borrow().len();
        let range = if old_line_count != new_count || self.dirty.is_some() {
            0..self.map.row_count()
        } else {
            self.current_row.saturating_sub(1)..(self.current_row + 2).min(self.map.row_count())
        };
        self.dirty = Some(range);
        self.edited_at = Some(Instant::now());
        self.inline.clear();
        self.find.last_hit = None;
        self.rebuild_projection();
    }
    fn rebuild_projection(&mut self) {
        let stale = self.map.is_stale();
        let ranges: Vec<_> = self
            .sections
            .iter()
            .filter(|s| {
                s.importance
                    .with_ignore_unimportant(self.map.rules.ignore_unimportant)
                    != Importance::Same
            })
            .map(|s| s.rows.clone())
            .collect();
        self.projection = Projection::build(
            self.row_count(),
            &ranges,
            if stale {
                DisplayFilter::All
            } else {
                self.filter
            },
            self.context,
            &self.expanded,
        );
    }
    pub(crate) fn row(&self, row: usize) -> TextMapRow {
        if !self.map.is_stale() {
            if let Some(r) = self.map.row(row) {
                return r;
            }
        }
        let left = self.map.left.lines.borrow().len();
        let right = self.map.right.lines.borrow().len();
        // A virtual EOF row allows editing empty files and typing after a final newline.
        let eof = !self.map.is_stale() && row >= self.map.row_count();
        TextMapRow {
            left: if eof {
                Some(left)
            } else {
                (row <= left).then_some(row)
            },
            right: if eof {
                Some(right)
            } else {
                (row <= right).then_some(row)
            },
            ..Default::default()
        }
    }
    pub(crate) fn row_for_line(&self, side: usize, line: usize) -> usize {
        if self.map.is_stale() {
            return line;
        }
        let mut low = 0;
        let mut high = self.map.row_count();
        while low < high {
            let mid = (low + high) / 2;
            let candidate =
                (mid..high).find_map(|r| index(self.map.row(r).unwrap(), side).map(|i| (r, i)));
            match candidate {
                Some((r, i)) if i < line => low = r + 1,
                Some((r, i)) if i == line => return r,
                _ => high = mid,
            }
        }
        low.min(self.map.row_count())
    }
    fn navigate(&mut self, next: bool, edge: bool) {
        if self.map.is_stale() {
            self.message = "Wait for comparison before navigating differences".into();
            return;
        }
        let target = if edge {
            let mut iter = self.sections.iter().filter(|s| {
                s.importance
                    .with_ignore_unimportant(self.map.rules.ignore_unimportant)
                    != Importance::Same
            });
            if next { iter.next() } else { iter.next_back() }.map(|s| s.rows.start)
        } else if next {
            self.map
                .next_difference(self.current_row, Importance::Unimportant)
        } else {
            self.map
                .prev_difference(self.current_row, Importance::Unimportant)
        };
        if let Some(row) = target {
            self.goto_row(row);
        } else {
            self.message = "No more differences".into();
        }
    }
    pub(crate) fn goto_row(&mut self, row: usize) {
        self.current_row = row;
        for s in 0..2 {
            if let Some(line) = index(self.row(row), s) {
                self.editors[s].place(Caret { line, byte: 0 }, false);
            }
        }
        if self.filter == DisplayFilter::Same {
            self.filter = DisplayFilter::All;
            self.rebuild_projection();
        }
        self.scroll_to_row(self.projection.display_for(row));
    }
    fn copy_to(&mut self, destination: usize) -> Result<(), CoreError> {
        if self.map.is_stale() {
            return Err(CoreError::Invalid(
                "wait for comparison before copying aligned sections".into(),
            ));
        }
        self.cancel_join();
        let source = 1 - destination;
        self.content(destination).ensure_writable()?;
        if let Some((a, b)) = self.editors[source].selection() {
            let finish = (b.line + usize::from(b.byte > 0 || a.line == b.line))
                .min(self.content(source).lines.borrow().len());
            let first = self.row_for_line(source, a.line);
            let end = self.row_for_line(source, finish);
            let target_start = (0..first)
                .rev()
                .find_map(|r| index(self.row(r), destination))
                .map_or(0, |i| i + 1);
            let target_end = (first..end)
                .filter_map(|r| index(self.row(r), destination))
                .last()
                .map_or(target_start, |i| i + 1);
            let replacement = self.content(source).lines.borrow().slice(a.line..finish);
            let target = self.content_mut(destination);
            target.edits.push(Box::new(LineEdit::replace_range(
                &target.lines,
                target_start..target_end,
                &replacement,
            )));
            self.recompare();
        } else {
            let edit = self.map.copy_section(self.current_row, side(source))?;
            self.content_mut(destination).edits.push(edit);
            self.after_alignment();
        }
        Ok(())
    }
    pub fn swap(&mut self) {
        self.cancel_join();
        std::mem::swap(&mut self.map.left, &mut self.map.right);
        self.files.swap(0, 1);
        self.editors.swap(0, 1);
        self.active = 1 - self.active;
        let mut anchors = AlignmentOverrides::default();
        for &(l, r) in self.map.overrides.pairs() {
            let _ = anchors.insert(r, l);
        }
        self.map.overrides = anchors;
        self.find.last_hit = None;
        self.align_with = None;
        self.recompare();
        self.rebuild_projection();
    }
    pub fn save_to(
        &mut self,
        s: usize,
        path: &Path,
        prefs: &Prefs,
        keep_source: bool,
    ) -> Result<(), CoreError> {
        self.content(s).ensure_writable()?;
        // Drop/join the worker before materialization; it owns the only external snapshots.
        self.cancel_join();
        if prefs.backups.enabled && path.exists() {
            let backup = backup_path(path, prefs.backups.extension.as_deref())?;
            std::fs::copy(path, &backup)?;
        }
        {
            let mut context = MaterializeContext::default();
            // A copied section can share the OTHER pane's original mmap, too.
            self.map.left.materialize_owned(&mut context);
            self.map.right.materialize_owned(&mut context);
        }
        let original_source = self.content(s).source.clone();
        self.content_mut(s).materialize_and_save(path)?;
        if keep_source {
            self.content_mut(s).source = original_source;
        } else {
            self.files[s].path = Some(path.to_path_buf());
            self.files[s].target = None;
        }
        self.files[s].metadata_dirty = false;
        self.message = format!("Saved {}", path.display());
        if self.map.is_stale() {
            self.recompare();
        }
        Ok(())
    }
    fn save(&mut self, s: usize, prefs: &Prefs) -> Result<bool, CoreError> {
        let target = self.files[s].target.clone();
        if let Some(path) = target.as_ref().or(self.files[s].path.as_ref()).cloned() {
            self.save_to(s, &path, prefs, target.is_some())?;
            Ok(true)
        } else {
            self.file_dialog = Some(FileDialog {
                side: s,
                mode: FileMode::SaveAs,
                path: String::new(),
            });
            Ok(false)
        }
    }
    fn request(&mut self, pending: Pending) {
        if self
            .affected(&pending)
            .into_iter()
            .any(|s| self.side_dirty(s))
        {
            self.pending = Some(pending);
        } else {
            self.perform(pending);
        }
    }
    fn affected(&self, p: &Pending) -> Vec<usize> {
        match p {
            Pending::Home | Pending::Exit => vec![0, 1],
            Pending::Reload(s)
            | Pending::Open(s, _)
            | Pending::Clipboard(s, _)
            | Pending::Encoding(s, _) => vec![*s],
        }
    }
    fn perform(&mut self, p: Pending) {
        let result = match p {
            Pending::Home => {
                self.action = Some(ViewAction::Home);
                Ok(())
            }
            Pending::Exit => {
                self.action = Some(ViewAction::Exit);
                Ok(())
            }
            Pending::Reload(s) => self.files[s].path.clone().map_or_else(
                || {
                    Err(CoreError::Invalid(
                        "clipboard content has no source file".into(),
                    ))
                },
                |p| self.open_side(s, &p),
            ),
            Pending::Open(s, path) => self.open_side(s, &path),
            Pending::Clipboard(s, text) => {
                self.cancel_join();
                match TextContent::from_bytes(text.into_bytes(), None) {
                    Ok(mut c) => {
                        c.edits.clear();
                        *self.content_mut(s) = c;
                        self.files[s] = FileInfo::default();
                        self.editors[s] = Editor::default();
                        self.recompare();
                        self.rebuild_projection();
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            }
            Pending::Encoding(s, encoding) => {
                self.cancel_join();
                let result = self.content_mut(s).reload_encoding(encoding);
                if result.is_ok() {
                    self.editors[s] = Editor::default();
                    self.recompare();
                    self.rebuild_projection();
                }
                result
            }
        };
        if let Err(e) = result {
            self.message = e.to_string();
        }
    }
    fn open_side(&mut self, s: usize, path: &Path) -> Result<(), CoreError> {
        // Load transactionally before dropping the old editable content.
        let (content, file) = load(path, &self.registry)?;
        self.cancel_join();
        *self.content_mut(s) = content;
        self.files[s] = file;
        self.editors[s] = Editor::default();
        self.map.overrides.clear();
        self.align_with = None;
        self.find.last_hit = None;
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_path_buf());
        self.recent.truncate(12);
        self.recompare();
        self.rebuild_projection();
        Ok(())
    }
    fn find_action(&mut self, action: FindAction) -> Result<(), CoreError> {
        if self.map.is_stale() {
            return Err(CoreError::Invalid(
                "wait for comparison before searching".into(),
            ));
        }
        let s = self.active;
        if matches!(action, FindAction::All) {
            let ranges = self.map.difference_line_ranges(side(s));
            let options = self.find.options.clone();
            let replacement = self.find.replacement.clone();
            let count = bc_engine_text::search::replace_all(
                self.content_mut(s),
                &options,
                &replacement,
                Some(&ranges),
            )?;
            self.message = format!("Replaced {count} occurrences");
            if count > 0 {
                self.recompare();
                self.rebuild_projection();
            }
            self.find.last_hit = None;
            return Ok(());
        }
        if matches!(action, FindAction::Replace) {
            let hit = self
                .find
                .last_hit
                .clone()
                .ok_or_else(|| CoreError::Invalid("Find a match before replacing".into()))?;
            if hit.side != side(s) {
                return Err(CoreError::Invalid("match belongs to the other pane".into()));
            }
            let text = self
                .content(s)
                .lines
                .borrow()
                .line_bytes(hit.line)
                .unwrap()
                .to_vec();
            let text = std::str::from_utf8(&text).unwrap();
            let replacement = if self.find.options.is_regexp {
                let p = bc_core::pattern::Pattern::new(
                    &self.find.options.pattern,
                    self.find.options.case_sensitive,
                )?;
                let changed =
                    p.replace_all_if(text, &self.find.replacement, |range| range == hit.range);
                changed[hit.range.start..changed.len() - (text.len() - hit.range.end)].to_string()
            } else {
                self.find.replacement.clone()
            };
            let old = self.content(s).lines.borrow().len();
            self.editors[s].caret = Caret {
                line: hit.line,
                byte: hit.range.end,
            };
            self.editors[s].anchor = Some(Caret {
                line: hit.line,
                byte: hit.range.start,
            });
            let content = if s == 0 {
                &mut self.map.left
            } else {
                &mut self.map.right
            };
            self.editors[s].insert(content, &replacement)?;
            self.edited(old);
            return Ok(());
        }
        let backwards = matches!(action, FindAction::Previous) || self.find.options.backwards;
        let mut options = self.find.options.clone();
        options.backwards = backwards;
        let mut from = (self.editors[s].caret.line, self.editors[s].caret.byte);
        if let Some(hit) = self.find.last_hit.as_ref().filter(|h| h.side == side(s)) {
            from = (
                hit.line,
                if backwards {
                    hit.range.start
                } else {
                    hit.range.end
                },
            );
            if hit.range.is_empty() {
                let mut editor = self.editors[s].clone();
                editor.place(
                    Caret {
                        line: from.0,
                        byte: from.1,
                    },
                    false,
                );
                editor.move_horizontal(self.content(s), !backwards, false, false);
                if editor.caret
                    == (Caret {
                        line: from.0,
                        byte: from.1,
                    })
                {
                    self.message = "No more matches".into();
                    return Ok(());
                }
                from = (editor.caret.line, editor.caret.byte);
            }
        }
        if let Some(hit) = self.map.find_next(side(s), from, &options)? {
            self.current_row = self.row_for_line(s, hit.line);
            self.editors[s].anchor = Some(Caret {
                line: hit.line,
                byte: hit.range.start,
            });
            self.editors[s].caret = Caret {
                line: hit.line,
                byte: hit.range.end,
            };
            if matches!(
                self.projection
                    .row(self.projection.display_for(self.current_row)),
                Some(DisplayRow::Fold(_))
            ) || self.filter == DisplayFilter::Same
            {
                self.filter = DisplayFilter::All;
                self.rebuild_projection();
            }
            self.scroll_to_row(self.projection.display_for(self.current_row));
            self.find.last_hit = Some(hit);
        } else {
            self.message = "No more matches".into();
        }
        Ok(())
    }
    pub fn ui(&mut self, ui: &mut egui::Ui, prefs: &mut Prefs) -> Option<ViewAction> {
        self.poll();
        let ctx = ui.ctx().clone();
        if self.worker.is_some() || self.edited_at.is_some() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        self.keyboard(&ctx, prefs);
        let blocked = self.pending.is_some() || self.file_dialog.is_some();
        if blocked {
            ui.disable();
        }
        self.toolbar(ui, prefs);
        let progress = self
            .worker
            .as_ref()
            .map(Worker::progress)
            .unwrap_or_else(|| self.message.clone());
        status_bar::ui(
            ui,
            &self.map.left,
            &self.map.right,
            &self.editors,
            self.counts,
            &progress,
        );
        if let Some(action) = self.find.ui(ui) {
            if let Err(e) = self.find_action(action) {
                self.message = e.to_string();
            }
        }
        let before = self.map.rules.clone();
        let old_context = self.context;
        if !blocked
            && self
                .settings
                .ui(&ctx, &mut self.map.rules, &mut self.context, prefs)
        {
            if !before.same_comparison(&self.map.rules) {
                self.recompare();
            } else {
                self.after_alignment();
            }
        }
        if old_context != self.context {
            self.expanded.clear();
            self.rebuild_projection();
        }
        if !blocked {
            self.formats.ui(&ctx, &mut self.registry);
        }
        self.dialogs(&ctx, prefs);
        self.last_stats.rows_drawn = 0;
        let row_height = self.font_size + 5.0;
        if let Some(row) = self.pending_scroll.take() {
            self.scroll.y = row.min(self.projection.len().saturating_sub(1)) as f32 * row_height;
        }
        ui.horizontal_top(|ui| {
            let nav_height = ui.available_height();
            let mut target = None;
            ui.allocate_ui(egui::vec2(22.0, nav_height), |ui| {
                target = self
                    .nav
                    .ui(ui, &prefs.colors, self.visible.clone(), self.current_row);
            });
            if let Some(row) = target {
                self.goto_row(row);
            }
            let available = ui.available_size();
            ui.allocate_ui_with_layout(available, egui::Layout::top_down(egui::Align::Min), |ui| {
                if self.stacked {
                    let height = (available.y - 8.0) / 2.0;
                    for s in 0..2 {
                        ui.allocate_ui(egui::vec2(available.x, height), |ui| {
                            self.pane(ui, s, height, prefs);
                        });
                    }
                } else {
                    ui.columns(2, |cols| {
                        for (s, col) in cols.iter_mut().enumerate() {
                            self.pane(col, s, available.y, prefs);
                        }
                    });
                }
            });
        });
        if !ctx.input(|i| i.pointer.primary_down()) {
            self.dragging = None;
        }
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if let Some(path) = dropped
            .first()
            .filter(|_| !blocked)
            .map(|f| f.path().to_path_buf())
        {
            let pos = ctx.input(|i| i.pointer.hover_pos());
            let s = pos
                .and_then(|p| self.pane_rects.iter().position(|r| r.contains(p)))
                .unwrap_or(self.active);
            self.request(Pending::Open(s, path));
        }
        self.action.take()
    }
    fn toolbar(&mut self, ui: &mut egui::Ui, prefs: &Prefs) {
        egui::Panel::top("text_commands").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (name, next, edge) in [
                    ("First difference", true, true),
                    ("Previous", false, false),
                    ("Next", true, false),
                    ("Last", false, true),
                ] {
                    if ui.button(name).clicked() {
                        self.navigate(next, edge);
                    }
                }
                if ui.button("Copy to Left").clicked() {
                    if let Err(e) = self.copy_to(0) {
                        self.message = e.to_string();
                    }
                }
                if ui.button("Copy to Right").clicked() {
                    if let Err(e) = self.copy_to(1) {
                        self.message = e.to_string();
                    }
                }
                if ui.button("Swap Sides").clicked() {
                    self.swap();
                }
                if ui.button("Recompare").clicked() {
                    self.recompare();
                }
                if ui.button("Find / Replace").clicked() {
                    self.find.open = !self.find.open;
                }
                if ui.button("Settings").clicked() {
                    self.settings.draft = self.map.rules.clone();
                    self.settings.open = !self.settings.open;
                }
                if ui.button("File formats").clicked() {
                    self.formats.open = !self.formats.open;
                }
                if ui.button("Save active").clicked() {
                    if let Err(e) = self.save(self.active, prefs) {
                        self.message = e.to_string();
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                let old = self.filter;
                for f in [
                    DisplayFilter::All,
                    DisplayFilter::Differences,
                    DisplayFilter::Same,
                    DisplayFilter::Context,
                ] {
                    ui.selectable_value(&mut self.filter, f, format!("{f:?}"));
                }
                if old != self.filter {
                    self.expanded.clear();
                    self.rebuild_projection();
                    self.scroll.y = 0.0;
                }
                if ui
                    .checkbox(&mut self.map.rules.ignore_unimportant, "Ignore unimportant")
                    .changed()
                {
                    self.settings.draft.ignore_unimportant = self.map.rules.ignore_unimportant;
                    self.after_alignment();
                }
                ui.checkbox(&mut self.stacked, "Over-Under");
                ui.menu_button("View / alignment", |ui| {
                    ui.checkbox(&mut self.show_numbers, "Line numbers");
                    ui.checkbox(&mut self.show_whitespace, "Visible whitespace");
                    ui.checkbox(&mut self.show_syntax, "Syntax highlighting");
                    ui.add(egui::Slider::new(&mut self.font_size, 10.0..=24.0).text("Font size"));
                    if ui.button("Align With: pick other side line").clicked() {
                        self.align_with = Some((self.active, self.editors[self.active].caret.line));
                        ui.close();
                    }
                    if ui.button("Clear manual alignment").clicked() {
                        self.map.overrides.clear();
                        self.recompare();
                        ui.close();
                    }
                    ui.add_enabled(
                        false,
                        egui::Button::new("Compare Parent Folders (not available)"),
                    );
                });
                ui.label("Go to");
                ui.add(egui::DragValue::new(&mut self.goto_line).range(1..=usize::MAX));
                ui.label(":");
                ui.add(egui::DragValue::new(&mut self.goto_column).range(1..=usize::MAX));
                if ui.button("Go").clicked() {
                    let content = self.content(self.active);
                    let lines = content.lines.borrow();
                    let line = (self.goto_line - 1).min(lines.len().saturating_sub(1));
                    let text =
                        std::str::from_utf8(lines.line_bytes(line).unwrap_or_default()).unwrap();
                    let byte = super::editor::byte_at_char(text, self.goto_column - 1);
                    drop(lines);
                    self.current_row = self.row_for_line(self.active, line);
                    self.editors[self.active].place(Caret { line, byte }, false);
                    self.filter = DisplayFilter::All;
                    self.rebuild_projection();
                    self.scroll_to_row(self.current_row);
                }
            });
        });
    }
    pub(crate) fn header(&mut self, ui: &mut egui::Ui, s: usize, prefs: &Prefs) {
        let label = self.files[s].path.as_ref().map_or_else(
            || "Clipboard / untitled".into(),
            |p| p.display().to_string(),
        );
        ui.horizontal(|ui| {
            if ui
                .selectable_label(self.active == s, if s == 0 { "Left" } else { "Right" })
                .clicked()
            {
                self.active = s;
            }
            ui.label(
                egui::RichText::new(format!(
                    "{}{}",
                    label,
                    if self.side_dirty(s) { " *" } else { "" }
                ))
                .small(),
            );
            ui.menu_button("File", |ui| {
                for (label, mode) in [
                    ("Open…", FileMode::Open),
                    ("Save as…", FileMode::SaveAs),
                    ("Save target…", FileMode::Target),
                ] {
                    if ui.button(label).clicked() {
                        self.file_dialog = Some(FileDialog {
                            side: s,
                            mode,
                            path: self.files[s]
                                .path
                                .as_ref()
                                .map(|p| p.display().to_string())
                                .unwrap_or_default(),
                        });
                        ui.close();
                    }
                }
                if ui.button("Save").clicked() {
                    if let Err(e) = self.save(s, prefs) {
                        self.message = e.to_string();
                    }
                    ui.close();
                }
                if ui.button("Reload").clicked() {
                    self.request(Pending::Reload(s));
                    ui.close();
                }
                if ui.button("Open Clipboard").clicked() {
                    self.clipboard_side = Some(s);
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                    ui.close();
                }
                ui.checkbox(&mut self.content_mut(s).read_only, "Read-only");
                ui.separator();
                for (label, encoding) in [
                    ("UTF-8", Encoding::Utf8),
                    ("UTF-16 LE", Encoding::Utf16Le),
                    ("UTF-16 BE", Encoding::Utf16Be),
                    ("GBK (936)", Encoding::CodePage(936)),
                    ("Windows-1252", Encoding::CodePage(1252)),
                ] {
                    if ui
                        .selectable_label(self.content(s).encoding == encoding, label)
                        .clicked()
                    {
                        self.request(Pending::Encoding(s, encoding));
                        ui.close();
                    }
                }
                ui.label("Save BOM policy");
                let old = self.content(s).bom_policy;
                for (name, policy) in [
                    ("Preserve", BomPolicy::Preserve),
                    ("Always", BomPolicy::Always),
                    ("Never", BomPolicy::Never),
                ] {
                    ui.selectable_value(&mut self.content_mut(s).bom_policy, policy, name);
                }
                if old != self.content(s).bom_policy {
                    self.files[s].metadata_dirty = true;
                }
                ui.menu_button("Convert line endings", |ui| {
                    for e in [
                        bc_core::line::LineEnding::Lf,
                        bc_core::line::LineEnding::CrLf,
                        bc_core::line::LineEnding::Cr,
                    ] {
                        if ui.button(format!("{e:?}")).clicked() {
                            match self.content_mut(s).convert_endings(e) {
                                Ok(()) => self.recompare(),
                                Err(e) => self.message = e.to_string(),
                            }
                            ui.close();
                        }
                    }
                });
            });
        });
        ui.horizontal(|ui| {
            let selected = self
                .content(s)
                .format()
                .map_or("Plain text", |f| f.name.as_str())
                .to_string();
            let mut choice = None;
            egui::ComboBox::from_id_salt(("format", s))
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(false, "Detected").clicked() {
                        choice = Some(
                            self.files[s]
                                .path
                                .as_ref()
                                .and_then(|p| self.registry.resolve(p, Some(FormatKind::Text)))
                                .cloned(),
                        );
                    }
                    if ui.selectable_label(false, "Plain text").clicked() {
                        choice = Some(None);
                    }
                    for f in self
                        .registry
                        .formats()
                        .iter()
                        .filter(|f| f.kind == FormatKind::Text)
                    {
                        if ui.selectable_label(false, &f.name).clicked() {
                            choice = Some(Some(f.clone()));
                        }
                    }
                });
            if let Some(format) = choice {
                match self.content_mut(s).set_format(format) {
                    Ok(()) => self.recompare(),
                    Err(e) => self.message = e.to_string(),
                }
            }
            if let Some(target) = &self.files[s].target {
                ui.label(format!("→ {}", target.display()));
            }
        });
    }
    fn keyboard(&mut self, ctx: &egui::Context, prefs: &Prefs) {
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            if let egui::Event::Paste(text) = &event {
                if let Some(s) = self.clipboard_side.take() {
                    self.request(Pending::Clipboard(s, text.clone()));
                    continue;
                }
            }
            if self.pending.is_some() || self.file_dialog.is_some() {
                continue;
            }
            if let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = &event
            {
                if modifiers.command && *key == egui::Key::F {
                    self.find.open = true;
                    continue;
                }
                if modifiers.command && *key == egui::Key::S {
                    if let Err(e) = self.save(self.active, prefs) {
                        self.message = e.to_string();
                    }
                    continue;
                }
                if *key == egui::Key::F3 {
                    if let Err(e) = self.find_action(if modifiers.shift {
                        FindAction::Previous
                    } else {
                        FindAction::Next
                    }) {
                        self.message = e.to_string();
                    }
                    continue;
                }
                if *key == egui::Key::F7 {
                    self.navigate(!modifiers.shift, false);
                    continue;
                }
            }
            let s = self.active;
            if !ctx.memory(|m| m.has_focus(editor_id(s))) {
                continue;
            }
            let old = self.content(s).lines.borrow().len();
            let before = self.editors[s].caret;
            let content = if s == 0 {
                &mut self.map.left
            } else {
                &mut self.map.right
            };
            match self.editors[s].event(content, &event, ctx) {
                Ok(true) => self.edited(old),
                Ok(false) => {}
                Err(e) => self.message = e.to_string(),
            }
            if before != self.editors[s].caret {
                self.current_row = self.row_for_line(s, self.editors[s].caret.line);
                if let Some(line) = index(self.row(self.current_row), 1 - s) {
                    self.editors[1 - s].place(Caret { line, byte: 0 }, false);
                }
                let row = self.projection.display_for(self.current_row);
                let top = self.top_row(self.font_size + 5.0);
                if row < top || row > top + 30 {
                    self.scroll_to_row(row.saturating_sub(8));
                }
            }
        }
    }
    fn dialogs(&mut self, ctx: &egui::Context, prefs: &Prefs) {
        if let Some(mut dialog) = self.file_dialog.take() {
            let mut accept = false;
            let mut cancel = false;
            egui::Window::new("File path")
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label("Local path (native file picker is deferred)");
                    ui.text_edit_singleline(&mut dialog.path);
                    for p in &self.recent {
                        if ui
                            .selectable_label(false, p.display().to_string())
                            .clicked()
                        {
                            dialog.path = p.display().to_string();
                        }
                    }
                    ui.horizontal(|ui| {
                        accept = ui.button("OK").clicked();
                        cancel = ui.button("Cancel").clicked();
                    });
                });
            if accept && !dialog.path.trim().is_empty() {
                let path = PathBuf::from(dialog.path.trim());
                match dialog.mode {
                    FileMode::Open => self.request(Pending::Open(dialog.side, path)),
                    FileMode::SaveAs | FileMode::Target => {
                        let target = matches!(dialog.mode, FileMode::Target);
                        match self.save_to(dialog.side, &path, prefs, target) {
                            Ok(()) => {
                                if target {
                                    self.files[dialog.side].target = Some(path);
                                }
                            }
                            Err(e) => {
                                self.message = e.to_string();
                                self.file_dialog = Some(dialog);
                            }
                        }
                    }
                }
            } else if !cancel {
                self.file_dialog = Some(dialog);
            }
        }
        if let Some(pending) = self.pending.clone() {
            let mut decision = 0;
            egui::Window::new("Unsaved changes")
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label("Save changes before continuing?");
                    for s in self.affected(&pending) {
                        if self.side_dirty(s) {
                            ui.label(if s == 0 {
                                "Left is modified"
                            } else {
                                "Right is modified"
                            });
                        }
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            decision = 1;
                        }
                        if ui.button("Discard").clicked() {
                            decision = 2;
                        }
                        if ui.button("Cancel").clicked() {
                            decision = 3;
                        }
                    });
                });
            if decision == 1 {
                let mut saved = true;
                for s in self.affected(&pending) {
                    if self.side_dirty(s) {
                        match self.save(s, prefs) {
                            Ok(true) => {}
                            Ok(false) => {
                                saved = false;
                                break;
                            }
                            Err(e) => {
                                self.message = e.to_string();
                                saved = false;
                                break;
                            }
                        }
                    }
                }
                if saved {
                    self.pending = None;
                    self.perform(pending);
                }
            } else if decision == 2 {
                self.pending = None;
                self.perform(pending);
            } else if decision == 3 {
                self.pending = None;
            }
        }
    }
    pub(crate) fn expand(&mut self, range: Range<usize>) {
        self.expanded.push(range);
        self.rebuild_projection();
    }
}
pub(crate) fn side(s: usize) -> Side {
    if s == 0 {
        Side::Left
    } else {
        Side::Right
    }
}
pub(crate) fn index(row: TextMapRow, s: usize) -> Option<usize> {
    if s == 0 {
        row.left
    } else {
        row.right
    }
}
pub(crate) fn editor_id(s: usize) -> egui::Id {
    egui::Id::unique(("text_editor", s))
}
fn load(path: &Path, registry: &FormatRegistry) -> Result<(TextContent, FileInfo), CoreError> {
    let start = Instant::now();
    let metadata = std::fs::metadata(path)?;
    let mut content = TextContent::open(
        path,
        registry.resolve(path, Some(FormatKind::Text)).cloned(),
        1252,
    )?;
    content.read_only = metadata.permissions().readonly();
    Ok((
        content,
        FileInfo {
            path: Some(path.to_path_buf()),
            byte_len: metadata.len() as usize,
            load_time: start.elapsed(),
            ..Default::default()
        },
    ))
}
fn backup_path(path: &Path, suffix: Option<&str>) -> Result<PathBuf, CoreError> {
    let name = path
        .file_name()
        .ok_or_else(|| CoreError::Invalid("save path has no file name".into()))?;
    let mut backup = std::ffi::OsString::new();
    if let Some(suffix) = suffix {
        if suffix.is_empty() || suffix.contains(['/', '\\']) {
            return Err(CoreError::Invalid(
                "backup suffix must be nonempty and contain no separators".into(),
            ));
        }
        backup.push(name);
        backup.push(suffix);
    } else {
        backup.push("~");
        backup.push(name);
    }
    Ok(path.with_file_name(backup))
}
