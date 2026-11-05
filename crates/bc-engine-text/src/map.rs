//! Aligned text model – BC `TTextMap`: the row-oriented view over 2 or 3
//! [`crate::TextContent`]s plus per-row importance, backed by a `bc_core::align::AlignMap`.
//! Edits to the map (`TTextMapInsertItem`, `TTextMapDeleteItem`, `TTextMapMoveItem`,
//! `TTextMapAlignItem`, `TTextMapResetItem`…) are undoable via the content's `EditStack`.

use crate::content::TextContent;
use crate::importance::Importance;
use crate::rules::TextRules;
use bc_core::align::AlignMap;
use bc_core::edit::EditItem;
use bc_core::{DiffKind, Side};

/// One display row: aligned line indices and classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextMapRow {
    pub left: Option<usize>,
    pub right: Option<usize>,
    pub center: Option<usize>,
    pub kind: DiffKind,
    pub importance: Importance,
    /// Index of the difference *section* (contiguous block) this row belongs to.
    pub section: Option<usize>,
}

/// Two- or three-pane aligned text.
#[derive(Debug)]
pub struct TextMap {
    pub left: TextContent,
    pub right: TextContent,
    /// Present for Text Merge.
    pub center: Option<TextContent>,
    pub rules: TextRules,
    align: AlignMap,
    rows: Vec<TextMapRow>,
}

impl TextMap {
    /// Create an unaligned map (rows empty until an align job runs).
    pub fn new(left: TextContent, right: TextContent, center: Option<TextContent>, rules: TextRules) -> Self {
        Self { left, right, center, rules, align: AlignMap::new(), rows: Vec::new() }
    }

    pub fn rows(&self) -> &[TextMapRow] {
        &self.rows
    }

    pub fn align_map(&self) -> &AlignMap {
        &self.align
    }

    /// Replace alignment + classification (called by `jobs::Align2Job` / `Align3Job`).
    pub fn set_alignment(&mut self, align: AlignMap, rows: Vec<TextMapRow>) {
        self.align = align;
        self.rows = rows;
    }

    /// Text of the line shown in `side` at `row`, if any.
    pub fn text_at(&self, side: Side, row: usize) -> Option<&[u8]> {
        todo!("bc-engine-text::map: row → side line lookup")
    }

    /// Next row after `from` with importance ≥ `min` (next difference navigation, FR-T6).
    pub fn next_difference(&self, from: usize, min: Importance) -> Option<usize> {
        todo!("bc-engine-text::map: navigation")
    }

    pub fn prev_difference(&self, from: usize, min: Importance) -> Option<usize> {
        todo!("bc-engine-text::map: navigation")
    }

    /// Copy the section containing `row` from `from` side to the other side
    /// (BC "Copy Section to Right/Left") as an undoable edit.
    pub fn copy_section(&mut self, row: usize, from: Side) -> Box<dyn EditItem> {
        todo!("bc-engine-text::map: TTextMapMoveItem equivalent")
    }

    /// Count of (important, unimportant) difference sections.
    pub fn difference_counts(&self) -> (usize, usize) {
        todo!("bc-engine-text::map: statistics")
    }
}
