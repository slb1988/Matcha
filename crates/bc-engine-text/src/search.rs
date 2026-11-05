//! Find / replace / find-in-files (FR-T6) – backend of BC's find dialogs and
//! `TTEXTFINDINFILESDIALOG`.

use crate::TextContent;
use bc_core::Side;

/// Search options.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FindOptions {
    pub pattern: String,
    pub is_regexp: bool,
    pub case_sensitive: bool,
    pub whole_words: bool,
    pub backwards: bool,
    /// Restrict to lines that are differences.
    pub differences_only: bool,
}

/// A hit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindHit {
    pub side: Side,
    pub line: usize,
    pub range: std::ops::Range<usize>,
}

/// Find next occurrence starting after `(line, col)` on `side`.
pub fn find_next(content: &TextContent, side: Side, from: (usize, usize), options: &FindOptions) -> Option<FindHit> {
    todo!("bc-engine-text::search: find")
}

/// Replace all occurrences; returns count. Recorded as a single undo group.
pub fn replace_all(content: &mut TextContent, options: &FindOptions, replacement: &str) -> usize {
    todo!("bc-engine-text::search: replace all")
}
