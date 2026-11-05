//! Regular-expression wrapper – BC `TSsRegExpr` / `TREParser`.
//!
//! BC grammars, unimportant-text rules, replacements, name filters and `rename regexpr`
//! all share one regex dialect (Perl-compatible, see help topic `sample_regular_expressions`).
//! This wrapper isolates the rest of the workspace from the engine choice (likely the
//! `regex` crate with a compatibility shim for `\<` / `\>` style tokens).

/// Compiled pattern.
#[derive(Debug, Clone)]
pub struct Pattern {
    source: String,
    case_sensitive: bool,
}

/// A match: byte span plus capture spans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub span: std::ops::Range<usize>,
    pub captures: Vec<Option<std::ops::Range<usize>>>,
}

impl Pattern {
    /// Compile a BC-dialect regular expression.
    #[allow(unused_variables)] // remaining scaffold entry point
    pub fn new(source: &str, case_sensitive: bool) -> Result<Self, crate::CoreError> {
        todo!("bc-core::pattern: compile")
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn case_sensitive(&self) -> bool {
        self.case_sensitive
    }

    /// First match at or after `start`.
    #[allow(unused_variables)] // remaining scaffold entry point
    pub fn find_at(&self, haystack: &str, start: usize) -> Option<Match> {
        todo!("bc-core::pattern: find")
    }

    /// Match anchored at exactly `start` (tokenizer use).
    #[allow(unused_variables)] // remaining scaffold entry point
    pub fn match_at(&self, haystack: &str, start: usize) -> Option<Match> {
        todo!("bc-core::pattern: anchored match")
    }

    /// Replace all matches with `replacement` (supports `$1` style groups).
    #[allow(unused_variables)] // remaining scaffold entry point
    pub fn replace_all(&self, haystack: &str, replacement: &str) -> String {
        todo!("bc-core::pattern: replace")
    }
}

/// Convert a DOS-style file mask (`*.pas;-*.bak;My Folder\`) into a pattern.
/// Mask semantics follow help topic `file_masks` (see bc-engine-dir `NameFilter`).
#[allow(unused_variables)] // remaining scaffold entry point
pub fn glob_to_pattern(mask: &str, case_sensitive: bool) -> Result<Pattern, crate::CoreError> {
    todo!("bc-core::pattern: glob translation")
}
