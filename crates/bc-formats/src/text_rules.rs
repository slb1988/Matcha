//! Per-session text rules that reference grammar elements – BC `TTextInsigItem` /
//! `TTextInsigList` (unimportant text) and `TTextReplacementItem` / `TTextReplacementList`
//! (FR-T4 replacements: regex pairs treated as equivalent during comparison).

use crate::model::ElementKind;

/// "Unimportant text" rule: text matched by `pattern` (or belonging to an element
/// kind marked unimportant) does not count as an important difference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnimportantText {
    pub pattern: String,
    pub is_regexp: bool,
    pub case_sensitive: bool,
    /// Restrict the rule to text already classified as this element (BC "Only in …").
    pub only_in: Option<ElementKind>,
}

/// Replacement rule – differences that match `from` ↔ `to` are not important.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Replacement {
    pub from: String,
    pub to: String,
    pub is_regexp: bool,
    pub case_sensitive: bool,
    /// Match whole words only.
    pub whole_words: bool,
    /// Apply in both directions (left→right and right→left).
    pub bidirectional: bool,
}

/// Importance assignment per element kind for a session (`TTextSessionImportanceFrame`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementImportance {
    /// Element kinds whose differences are *unimportant*; everything else is important.
    pub unimportant: Vec<ElementKind>,
    /// Leading whitespace unimportant.
    pub leading_whitespace_unimportant: bool,
    /// Embedded whitespace unimportant.
    pub embedded_whitespace_unimportant: bool,
    /// Trailing whitespace unimportant.
    pub trailing_whitespace_unimportant: bool,
    /// Character case unimportant (session level override of per-element CaseSig).
    pub case_unimportant: bool,
    /// Line ending style differences unimportant.
    pub line_endings_unimportant: bool,
}

impl Default for ElementImportance {
    fn default() -> Self {
        Self {
            unimportant: Vec::new(),
            leading_whitespace_unimportant: false,
            embedded_whitespace_unimportant: false,
            trailing_whitespace_unimportant: false,
            case_unimportant: false,
            line_endings_unimportant: true,
        }
    }
}
