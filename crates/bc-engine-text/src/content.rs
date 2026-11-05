//! Loaded text of one pane – BC `TLineContent` (`TFileContent` subclass) wrapping a
//! `TLineConLineList`.

use bc_core::edit::EditStack;
use bc_core::encoding::{Detection, Encoding};
use bc_core::line::LineList;
use bc_formats::FileFormat;
use bc_vfs::VfsPath;

/// One side's text, its encoding and its edit history.
#[derive(Debug)]
pub struct TextContent {
    pub source: Option<VfsPath>,
    pub lines: LineList,
    pub encoding: Encoding,
    pub detection: Option<Detection>,
    /// Resolved file format (grammar) – `None` = plain text.
    pub format: Option<FileFormat>,
    pub edits: EditStack,
    pub read_only: bool,
}

impl TextContent {
    /// Empty content.
    pub fn empty() -> Self {
        Self {
            source: None,
            lines: LineList::new(),
            encoding: Encoding::Utf8,
            detection: None,
            format: None,
            edits: EditStack::new(),
            read_only: false,
        }
    }

    /// Load from bytes (encoding detection, optional converter, line split).
    pub fn from_bytes(bytes: Vec<u8>, format: Option<FileFormat>) -> Result<Self, bc_core::CoreError> {
        todo!("bc-engine-text::content: detect encoding, convert, split lines")
    }

    /// Serialise for saving (`/savetarget`, backups handled by the session layer).
    pub fn to_bytes(&self) -> Result<Vec<u8>, bc_core::CoreError> {
        todo!("bc-engine-text::content: encode with original BOM/ending policy")
    }

    pub fn is_dirty(&self) -> bool {
        self.edits.is_dirty()
    }
}
