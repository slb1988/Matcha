//! Syntax tokenizer skeleton – BC `TTextParser` family:
//! `TTextTokenizer` (grammar driven), `TTextColParser` (column items), `TREParser`
//! (regex state machine). Produces [`Token`]s that the text engine uses for highlighting
//! and for deciding which differences are unimportant.

use crate::model::{ElementKind, FileFormat};
use std::ops::Range;

/// A classified span of a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// Byte range within the line.
    pub span: Range<usize>,
    pub element: ElementKind,
    /// Index of the grammar item that produced it.
    pub grammar_index: usize,
}

/// Tokenizer state carried across lines (open multi-line comment/string).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LineState {
    /// Delimited item still open at the end of the previous line, if any.
    pub open_item: Option<usize>,
}

/// Grammar-driven tokenizer – `TTextTokenizer`.
#[derive(Debug)]
pub struct Tokenizer<'f> {
    format: &'f FileFormat,
}

impl<'f> Tokenizer<'f> {
    /// Compile the format's grammar (regexes, keyword sets).
    pub fn new(format: &'f FileFormat) -> Result<Self, bc_core::CoreError> {
        todo!("bc-formats::grammar: compile grammar items")
    }

    pub fn format(&self) -> &'f FileFormat {
        self.format
    }

    /// Tokenize one line given the state left by the previous line.
    pub fn tokenize_line(&self, line: &str, state: &mut LineState) -> Vec<Token> {
        todo!("bc-formats::grammar: tokenize (priority order, longest match, delimited spans)")
    }
}
