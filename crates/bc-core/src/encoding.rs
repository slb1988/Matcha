//! Text encoding detection and conversion (FR-T5: UTF-8/16 + common ANSI, BOM handling).

/// Encodings the text engine can read and write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Encoding {
    #[default]
    Utf8,
    Utf16Le,
    Utf16Be,
    /// Windows/ANSI code page (e.g. 1252, 936).
    CodePage(u16),
    /// Detected as binary – shown in Hex compare instead.
    Binary,
}

/// Byte order mark handling for save.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BomPolicy {
    /// Keep whatever the file had.
    #[default]
    Preserve,
    Always,
    Never,
}

/// Result of sniffing a buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    pub encoding: Encoding,
    pub has_bom: bool,
    /// Confidence 0..=100 (BC prompts via `TFILECODEPAGEERRORDIALOG` when low).
    pub confidence: u8,
}

/// Detect encoding from the first bytes (BOM, UTF-8 validity, NUL density, mime.types sniff).
pub fn detect(bytes: &[u8], default_code_page: u16) -> Detection {
    todo!("bc-core::encoding: detection")
}

/// Decode `bytes` in `encoding` to UTF-8, reporting undecodable positions.
pub fn decode(bytes: &[u8], encoding: Encoding) -> Result<String, crate::CoreError> {
    todo!("bc-core::encoding: decode")
}

/// Encode UTF-8 `text` to `encoding`, optionally emitting a BOM.
pub fn encode(text: &str, encoding: Encoding, bom: BomPolicy) -> Result<Vec<u8>, crate::CoreError> {
    todo!("bc-core::encoding: encode")
}
