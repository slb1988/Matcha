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

/// Backup behaviour on save (`/nobackups` overrides).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BackupPrefs {
    pub enabled: bool,
    /// Suffix such as `.bak`; `None` = same folder with `~` prefix.
    pub extension: Option<String>,
}

/// Preferences used by the engines and the CLI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Prefs {
    pub colors: DiffColors,
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
