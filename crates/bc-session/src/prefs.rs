//! Global preferences subset – BC `TBcPrefs` (`TPrefs`, `BCPreferences.xml`
//! `<BCPreferences><TBcPrefs>…`). Only values the engines need are modelled; everything
//! else is retained as raw nodes for write-through.

use bc_core::persist::Node;

/// Difference colours (FR-T1: important=red, unimportant=blue, insert=green; configurable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffColors {
    pub important: [u8; 3],
    pub unimportant: [u8; 3],
    pub inserted: [u8; 3],
    pub conflict: [u8; 3],
}

impl Default for DiffColors {
    fn default() -> Self {
        Self {
            important: [0xFF, 0x00, 0x00],
            unimportant: [0x00, 0x00, 0xFF],
            inserted: [0x00, 0x80, 0x00],
            conflict: [0xFF, 0x80, 0x00],
        }
    }
}

/// Folder palette is independent of text importance colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FolderColors {
    pub unknown: [u8; 3],
    pub same: [u8; 3],
    pub orphan: [u8; 3],
    pub different: [u8; 3],
    pub filtered: [u8; 3],
    pub unscanned: [u8; 3],
}
impl Default for FolderColors {
    fn default() -> Self {
        Self { unknown: [128,128,128], same: [0,0,0], orphan: [0x6A,0,0xD5],
            different: [255,0,0], filtered: [0,128,128], unscanned: [255,255,0] }
    }
}

/// Backup behaviour on save (`/nobackups` overrides).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BackupPrefs {
    pub enabled: bool,
    /// Suffix such as `.bak`; `None` = same folder with `~` prefix.
    pub extension: Option<String>,
}

/// Editor font (reference: Options › Colors, Fonts › Editor Font) used by the
/// text compare / text merge panes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorFont {
    /// Installed family name (English); unknown names fall back to Consolas.
    pub family: String,
    /// Size in points (96 dpi: px = pt × 4 / 3).
    pub size_pt: u8,
}

impl EditorFont {
    pub const DEFAULT_FAMILY: &'static str = "Consolas";
    pub const DEFAULT_SIZE_PT: u8 = 10;
    pub const SIZE_RANGE: std::ops::RangeInclusive<u8> = 6..=36;

    /// Size in logical pixels (points at 96 dpi).
    pub fn size_px(&self) -> f32 {
        f32::from(self.size_pt) * 4.0 / 3.0
    }
}

impl Default for EditorFont {
    fn default() -> Self {
        Self {
            family: Self::DEFAULT_FAMILY.to_owned(),
            size_pt: Self::DEFAULT_SIZE_PT,
        }
    }
}

/// Preferences used by the engines and the CLI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Prefs {
    pub colors: DiffColors,
    /// Text editor font (family + size).
    pub editor_font: EditorFont,
    pub backups: BackupPrefs,
    /// Default ANSI code page when detection is inconclusive.
    pub default_code_page: u16,
    /// Tab size fallback when the format does not specify one.
    pub default_tab_size: u32,
    /// Unmodelled subtree retained verbatim.
    pub unknown: Vec<Node>,
}

impl Prefs {
    /// Read `BCPreferences.xml` (missing file = defaults).
    pub fn read(xml: &str) -> Result<Prefs, crate::SessionError> {
        todo!("bc-session::prefs: parse TBcPrefs subset")
    }

    /// Write back, merging with retained unknown nodes.
    pub fn write(&self) -> Result<String, crate::SessionError> {
        todo!("bc-session::prefs: serialise")
    }
}
