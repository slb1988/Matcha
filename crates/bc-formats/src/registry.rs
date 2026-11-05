//! Format registry – BC `TFileFormatManager` / `TFileFormatList`.
//!
//! Holds built-in plus user formats, resolves a path to a format by mask (first match
//! in user order wins, as in BC's "File Formats" dialog ordering) and supports import /
//! export of user definitions.

use crate::model::{FileFormat, FormatKind};
use std::path::Path;

/// Ordered collection of formats.
#[derive(Debug, Clone, Default)]
pub struct FormatRegistry {
    formats: Vec<FileFormat>,
}

impl FormatRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registry pre-populated with the built-in library.
    pub fn with_builtins() -> Self {
        Self { formats: crate::builtin_formats().to_vec() }
    }

    pub fn formats(&self) -> &[FileFormat] {
        &self.formats
    }

    /// Append or replace (same `(kind, name)`) a format.
    pub fn upsert(&mut self, format: FileFormat) {
        todo!("bc-formats::registry: upsert")
    }

    /// Find by `(kind, name)`.
    pub fn get(&self, kind: FormatKind, name: &str) -> Option<&FileFormat> {
        self.formats.iter().find(|f| f.kind == kind && f.name == name)
    }

    /// Resolve the format for `path` restricted to `kind` (or any kind when `None`),
    /// by matching file masks in registry order. Falls back to `None` (= plain text).
    pub fn resolve(&self, path: &Path, kind: Option<FormatKind>) -> Option<&FileFormat> {
        todo!("bc-formats::registry: mask matching")
    }

    /// Load user formats from a `BCFileFormats.xml` document and merge them in.
    pub fn import_xml(&mut self, xml: &str) -> Result<usize, crate::bcff::FormatError> {
        todo!("bc-formats::registry: import user formats")
    }

    /// Export the non-built-in formats as `BCFileFormats` XML.
    pub fn export_xml(&self) -> Result<String, crate::bcff::FormatError> {
        todo!("bc-formats::registry: export user formats")
    }
}
