//! Compact two-way comparison rows. Public rows are derived values, not a second table.
use crate::{content::TextContent, importance::Importance, rules::TextRules};
use bc_core::{
    align::{AlignMap, AlignRow, AlignmentOverrides},
    edit::EditItem,
    hash::ContentHash,
    line::{LineEdit, MaterializeContext},
    CoreError, DiffKind, Side,
};
use std::{
    cell::{Ref, RefCell},
    ops::Range,
    rc::Rc,
    sync::Arc,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextMapRow {
    pub left: Option<usize>,
    pub right: Option<usize>,
    pub center: Option<usize>,
    pub kind: DiffKind,
    pub importance: Importance,
    pub section: Option<usize>,
}
const GAP: u32 = u32::MAX;
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub(crate) struct CompactRow {
    pub left: u32,
    pub right: u32,
    pub kind: DiffKind,
    pub importance: Importance,
}
impl CompactRow {
    pub(crate) fn new(
        left: Option<usize>,
        right: Option<usize>,
        kind: DiffKind,
        importance: Importance,
    ) -> Result<Self, CoreError> {
        let encode = |line: Option<usize>| match line {
            Some(i) if i < (u32::MAX as usize) => Ok(i as u32),
            Some(_) => Err(CoreError::Invalid(
                "line index exceeds compact-map limit".into(),
            )),
            None => Ok(GAP),
        };
        Ok(Self {
            left: encode(left)?,
            right: encode(right)?,
            kind,
            importance,
        })
    }
    pub(crate) fn index(&self, side: Side) -> Option<usize> {
        let i = match side {
            Side::Left => self.left,
            Side::Right => self.right,
            _ => GAP,
        };
        (i != GAP).then_some(i as usize)
    }
    pub(crate) fn shifted(mut self, left: isize, right: isize) -> Self {
        if self.left != GAP {
            self.left = (self.left as isize + left) as u32;
        }
        if self.right != GAP {
            self.right = (self.right as isize + right) as u32;
        }
        self
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub rows: Range<usize>,
    pub importance: Importance,
}
#[derive(Debug, Clone, Default)]
pub(crate) struct AlignedData {
    pub rows: Vec<CompactRow>,
    pub sections: Vec<Section>,
    pub left_hashes: Arc<Vec<ContentHash>>,
    pub right_hashes: Arc<Vec<ContentHash>>,
    pub revisions: ((u64, u64), (u64, u64)),
    pub rules: TextRules,
    pub overrides: AlignmentOverrides,
}
impl AlignedData {
    pub(crate) fn rebuild_sections(&mut self) {
        self.sections.clear();
        let mut start = None;
        let mut importance = Importance::Same;
        for (i, row) in self.rows.iter().enumerate() {
            if row.importance == Importance::Same {
                if let Some(start) = start.take() {
                    self.sections.push(Section {
                        rows: start..i,
                        importance,
                    });
                }
                importance = Importance::Same;
            } else {
                start.get_or_insert(i);
                importance = importance.max(row.importance);
            }
        }
        if let Some(start) = start {
            self.sections.push(Section {
                rows: start..self.rows.len(),
                importance,
            });
        }
    }
    fn row(&self, index: usize) -> Option<TextMapRow> {
        let row = self.rows.get(index)?;
        let section = self.sections.partition_point(|s| s.rows.end <= index);
        let section = self
            .sections
            .get(section)
            .filter(|s| s.rows.contains(&index))
            .map(|_| section);
        Some(TextMapRow {
            left: row.index(Side::Left),
            right: row.index(Side::Right),
            center: None,
            kind: row.kind,
            importance: row.importance,
            section,
        })
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DifferenceCounts {
    pub important: usize,
    pub unimportant: usize,
    pub left_only: usize,
    pub right_only: usize,
}

#[derive(Debug)]
pub struct TextMap {
    pub left: TextContent,
    pub right: TextContent,
    pub center: Option<TextContent>,
    pub rules: TextRules,
    pub overrides: AlignmentOverrides,
    pub(crate) data: Rc<RefCell<Arc<AlignedData>>>,
}
impl TextMap {
    pub fn new(
        left: TextContent,
        right: TextContent,
        center: Option<TextContent>,
        rules: TextRules,
    ) -> Self {
        Self {
            left,
            right,
            center,
            rules,
            overrides: AlignmentOverrides::default(),
            data: Rc::new(RefCell::new(Arc::new(AlignedData::default()))),
        }
    }
    pub fn compare(
        left: TextContent,
        right: TextContent,
        rules: TextRules,
    ) -> Result<Self, CoreError> {
        let mut map = Self::new(left, right, None, rules);
        map.recompare()?;
        Ok(map)
    }
    pub fn recompare(&mut self) -> Result<(), CoreError> {
        let output =
            crate::jobs::Align2Job::from_map(self).execute(&bc_core::cancel::NeverCancel)?;
        self.set_alignment(output)
    }
    pub fn set_alignment(&mut self, output: crate::jobs::AlignOutput) -> Result<(), CoreError> {
        if output.data.revisions != (self.left.revision(), self.right.revision())
            || !output.rules.same_comparison(&self.rules)
            || output.overrides != self.overrides
        {
            return Err(CoreError::Invalid("stale alignment result".into()));
        }
        *self.data.borrow_mut() = output.data;
        Ok(())
    }
    pub fn is_stale(&self) -> bool {
        let data = self.data.borrow();
        data.revisions != (self.left.revision(), self.right.revision())
            || !data.rules.same_comparison(&self.rules)
            || data.overrides != self.overrides
    }
    /// Recompute importance without alignment/tokenization when only masks changed.
    pub fn reclassify(&mut self, rules: crate::rules::ComparisonRules) -> Result<(), CoreError> {
        let old = self.rules.comparison.clone();
        self.rules.comparison = rules;
        let a = &old.importance;
        let b = &self.rules.comparison.importance;
        if old.replacements != self.rules.comparison.replacements
            || a.case_unimportant != b.case_unimportant
            || a.leading_whitespace_unimportant != b.leading_whitespace_unimportant
            || a.embedded_whitespace_unimportant != b.embedded_whitespace_unimportant
            || a.trailing_whitespace_unimportant != b.trailing_whitespace_unimportant
        {
            return self.recompare();
        }
        let comparison = crate::importance::CompiledComparison::new(&self.rules.comparison)?;
        let left = self.left.snapshot();
        let right = self.right.snapshot();
        let mut data = (**self.data.borrow()).clone();
        if data.revisions != (left.revision(), right.revision()) {
            return self.recompare();
        }
        for row in &mut data.rows {
            *row = crate::jobs::classify_row(
                &left,
                &right,
                row.index(Side::Left),
                row.index(Side::Right),
                &comparison,
                &bc_core::cancel::NeverCancel,
            )?;
        }
        data.rules = self.rules.clone();
        data.rebuild_sections();
        *self.data.borrow_mut() = Arc::new(data);
        Ok(())
    }
    pub fn row_count(&self) -> usize {
        self.data.borrow().rows.len()
    }
    pub fn row(&self, index: usize) -> Option<TextMapRow> {
        self.data.borrow().row(index)
    }
    pub fn rows(&self) -> Rows<'_> {
        Rows {
            data: self.data.borrow(),
            index: 0,
        }
    }
    pub fn sections(&self) -> Vec<Section> {
        self.data.borrow().sections.clone()
    }
    /// Explicit materialization for legacy consumers; do not call on the rendering hot path.
    pub fn align_map(&self) -> AlignMap {
        AlignMap::from_rows(
            self.rows()
                .map(|r| AlignRow {
                    left: r.left,
                    right: r.right,
                    center: r.center,
                    kind: r.kind,
                })
                .collect(),
        )
    }
    pub fn text_at(&self, side: Side, row: usize) -> Option<Ref<'_, [u8]>> {
        let row = self.row(row)?;
        let (content, index) = match side {
            Side::Left => (&self.left, row.left?),
            Side::Right => (&self.right, row.right?),
            _ => return None,
        };
        Ref::filter_map(content.lines.borrow(), |lines| lines.line_bytes(index)).ok()
    }
    pub fn next_difference(&self, from: usize, min: Importance) -> Option<usize> {
        let data = self.data.borrow();
        let row = data.row(from)?;
        let min = if self.rules.ignore_unimportant {
            min.max(Importance::Important)
        } else {
            min
        };
        let after = row.section.map_or(from, |i| data.sections[i].rows.end - 1);
        data.sections
            .iter()
            .find(|s| s.rows.start > after && s.importance >= min)
            .map(|s| s.rows.start)
    }
    pub fn prev_difference(&self, from: usize, min: Importance) -> Option<usize> {
        let data = self.data.borrow();
        let row = data.row(from)?;
        let min = if self.rules.ignore_unimportant {
            min.max(Importance::Important)
        } else {
            min
        };
        let before = row.section.map_or(from, |i| data.sections[i].rows.start);
        data.sections
            .iter()
            .rev()
            .find(|s| s.rows.start < before && s.importance >= min)
            .map(|s| s.rows.start)
    }
    pub fn statistics(&self) -> DifferenceCounts {
        let data = self.data.borrow();
        let mut counts = DifferenceCounts::default();
        for section in &data.sections {
            match section
                .importance
                .with_ignore_unimportant(self.rules.ignore_unimportant)
            {
                Importance::Important => counts.important += 1,
                Importance::Unimportant => counts.unimportant += 1,
                _ => {}
            }
        }
        for row in &data.rows {
            if row
                .importance
                .with_ignore_unimportant(self.rules.ignore_unimportant)
                != Importance::Same
            {
                match row.kind {
                    DiffKind::LeftOnly => counts.left_only += 1,
                    DiffKind::RightOnly => counts.right_only += 1,
                    _ => {}
                }
            }
        }
        counts
    }
    pub fn difference_counts(&self) -> (usize, usize) {
        let c = self.statistics();
        (c.important, c.unimportant)
    }
    pub fn difference_line_ranges(&self, side: Side) -> Vec<Range<usize>> {
        let data = self.data.borrow();
        let mut ranges: Vec<Range<usize>> = Vec::new();
        for row in &data.rows {
            if row
                .importance
                .with_ignore_unimportant(self.rules.ignore_unimportant)
                == Importance::Same
            {
                continue;
            }
            if let Some(i) = row.index(side) {
                if let Some(last) = ranges.last_mut().filter(|last| last.end == i) {
                    last.end += 1;
                } else {
                    ranges.push(i..i + 1);
                }
            }
        }
        ranges
    }
    /// Already-applied edit: caller records it on the destination content's EditStack.
    pub fn copy_section(&mut self, row: usize, from: Side) -> Result<Box<dyn EditItem>, CoreError> {
        let destination = match from {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
            _ => return Err(CoreError::Invalid("two-way copy side required".into())),
        };
        let before = self.data.borrow().clone();
        let section = before
            .row(row)
            .and_then(|r| r.section)
            .ok_or_else(|| CoreError::Invalid("row is not a difference section".into()))?;
        let range = before.sections[section].rows.clone();
        let indexes = |side| {
            before.rows[range.clone()]
                .iter()
                .filter_map(move |r| r.index(side))
                .collect::<Vec<_>>()
        };
        let sources = indexes(from);
        let targets = indexes(destination);
        let start = before.rows[..range.start]
            .iter()
            .rev()
            .find_map(|r| r.index(destination))
            .map_or(0, |i| i + 1);
        let (source, target) = if from == Side::Left {
            (&self.left, &self.right)
        } else {
            (&self.right, &self.left)
        };
        target.ensure_writable()?;
        let replacement = if let (Some(first), Some(last)) = (sources.first(), sources.last()) {
            source.lines.borrow().slice(*first..*last + 1)
        } else {
            bc_core::line::LineList::new()
        };
        let mut line =
            LineEdit::replace_range(&target.lines, start..start + targets.len(), &replacement);
        if let Err(error) = self.recompare() {
            line.undo();
            return Err(error);
        }
        let after = self.data.borrow().clone();
        Ok(Box::new(MapEdit {
            line,
            before,
            after,
            state: self.data.clone(),
        }))
    }
    pub fn find_next(
        &self,
        side: Side,
        from: (usize, usize),
        options: &crate::search::FindOptions,
    ) -> Result<Option<crate::search::FindHit>, CoreError> {
        let content = match side {
            Side::Left => &self.left,
            Side::Right => &self.right,
            _ => return Err(CoreError::Invalid("two-way search side required".into())),
        };
        let ranges = self.difference_line_ranges(side);
        crate::search::find_next(content, side, from, options, Some(&ranges))
    }
    pub fn replace_all(
        &mut self,
        side: Side,
        options: &crate::search::FindOptions,
        replacement: &str,
    ) -> Result<usize, CoreError> {
        let ranges = self.difference_line_ranges(side);
        let content = match side {
            Side::Left => &mut self.left,
            Side::Right => &mut self.right,
            _ => {
                return Err(CoreError::Invalid(
                    "two-way replacement side required".into(),
                ))
            }
        };
        let count = crate::search::replace_all(content, options, replacement, Some(&ranges))?;
        if count > 0 {
            self.recompare()?;
        }
        Ok(count)
    }
    pub fn storage_bytes(&self) -> usize {
        let d = self.data.borrow();
        d.rows.capacity() * std::mem::size_of::<CompactRow>()
            + d.sections.capacity() * std::mem::size_of::<Section>()
            + (d.left_hashes.capacity() + d.right_hashes.capacity())
                * std::mem::size_of::<ContentHash>()
    }
}
pub struct Rows<'a> {
    data: Ref<'a, Arc<AlignedData>>,
    index: usize,
}
impl Iterator for Rows<'_> {
    type Item = TextMapRow;
    fn next(&mut self) -> Option<Self::Item> {
        let row = self.data.row(self.index)?;
        self.index += 1;
        Some(row)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.data.rows.len() - self.index;
        (n, Some(n))
    }
}
impl ExactSizeIterator for Rows<'_> {}
#[derive(Debug)]
struct MapEdit {
    line: LineEdit,
    before: Arc<AlignedData>,
    after: Arc<AlignedData>,
    state: Rc<RefCell<Arc<AlignedData>>>,
}
impl EditItem for MapEdit {
    fn description(&self) -> String {
        "Copy section".into()
    }
    fn undo(&mut self) {
        self.line.undo();
        *self.state.borrow_mut() = self.before.clone();
    }
    fn redo(&mut self) {
        self.line.redo();
        *self.state.borrow_mut() = self.after.clone();
    }
    fn materialize(&mut self, context: &mut MaterializeContext) {
        self.line.materialize(context);
    }
}
