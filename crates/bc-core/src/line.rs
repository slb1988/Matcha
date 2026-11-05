//! Line model – BC `TSsLineList` / `TSsLineParser` / `TSsLineWriter`.
//!
//! A [`LineList`] owns (or borrows via mmap in a later milestone) a byte buffer and an
//! index of line start offsets. Lines are addressed by zero-based index; the text of a
//! line excludes its ending, which is tracked separately so that mixed endings survive a
//! round trip.

use std::ops::Range;

/// Line terminator of a single line. BC keeps per-line endings (`TSsLineListModifyEndingItem`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LineEnding {
    /// `\r\n`
    #[default]
    CrLf,
    /// `\n`
    Lf,
    /// `\r`
    Cr,
    /// Last line without terminator.
    None,
}

impl LineEnding {
    /// Byte representation written by [`LineWriter`].
    pub fn as_bytes(self) -> &'static [u8] {
        match self {
            LineEnding::CrLf => b"\r\n",
            LineEnding::Lf => b"\n",
            LineEnding::Cr => b"\r",
            LineEnding::None => b"",
        }
    }
}

/// Dominant line ending style of a file (`TSsLineListModifyEndingStyleItem`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EndingStyle {
    #[default]
    Windows,
    Unix,
    Mac,
    Mixed,
}

/// One parsed line: byte range of the text (without terminator) plus its ending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// Byte range into the owning [`LineList`] buffer.
    pub span: Range<usize>,
    pub ending: LineEnding,
}

/// Ordered list of lines over a byte buffer – BC `TSsLineList`.
///
/// Editing operations are recorded as [`crate::edit::EditItem`]s (`TSsLineListInsertItem`,
/// `TSsLineListDeleteItem`, `TSsLineListModifyEndingItem`) so they can be undone.
#[derive(Debug, Default)]
pub struct LineList {
    buffer: Vec<u8>,
    lines: Vec<Line>,
    style: EndingStyle,
}

impl LineList {
    /// Empty list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse a byte buffer into lines using [`LineParser`] defaults.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        todo!("bc-core::line: split buffer into lines, detect ending style")
    }

    /// Number of lines.
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// `true` when there are no lines.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Raw bytes of line `index` (without terminator).
    pub fn line_bytes(&self, index: usize) -> Option<&[u8]> {
        self.lines.get(index).map(|l| &self.buffer[l.span.clone()])
    }

    /// Ending of line `index`.
    pub fn ending(&self, index: usize) -> Option<LineEnding> {
        self.lines.get(index).map(|l| l.ending)
    }

    /// Dominant ending style.
    pub fn ending_style(&self) -> EndingStyle {
        self.style
    }

    /// Insert `text` as a new line before `index` (`TSsLineListInsertItem`).
    pub fn insert(&mut self, index: usize, text: &[u8], ending: LineEnding) {
        todo!("bc-core::line: insert line")
    }

    /// Remove line `index` (`TSsLineListDeleteItem`).
    pub fn delete(&mut self, index: usize) {
        todo!("bc-core::line: delete line")
    }

    /// Replace the text of line `index`.
    pub fn replace(&mut self, index: usize, text: &[u8]) {
        todo!("bc-core::line: replace line text")
    }

    /// Change the ending of a single line (`TSsLineListModifyEndingItem`).
    pub fn set_ending(&mut self, index: usize, ending: LineEnding) {
        todo!("bc-core::line: modify ending")
    }

    /// Serialise back to bytes with [`LineWriter`].
    pub fn to_bytes(&self) -> Vec<u8> {
        todo!("bc-core::line: write lines")
    }
}

/// Options controlling how a buffer is split into lines – BC `TSsLineParser`.
#[derive(Debug, Clone, Default)]
pub struct LineParser {
    /// Treat a lone `\r` as a terminator (classic Mac files).
    pub accept_cr: bool,
    /// Stop after this many bytes (0 = unlimited) – used by quick previews.
    pub limit: usize,
}

impl LineParser {
    /// Compute line spans and endings for `bytes`.
    pub fn parse(&self, bytes: &[u8]) -> (Vec<Line>, EndingStyle) {
        todo!("bc-core::line: line parser")
    }
}

/// Writes a [`LineList`] back out, optionally normalising endings – BC `TSsLineWriter`.
#[derive(Debug, Clone, Default)]
pub struct LineWriter {
    /// When set, every line is written with this ending instead of its own.
    pub force_ending: Option<LineEnding>,
}

impl LineWriter {
    /// Serialise `lines` into `out`.
    pub fn write(&self, lines: &LineList, out: &mut dyn std::io::Write) -> std::io::Result<()> {
        todo!("bc-core::line: line writer")
    }
}
