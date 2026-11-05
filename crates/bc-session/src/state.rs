//! View state saved with a session – BC `TSessionState` hierarchy (`<State>` element):
//! `TFileSessionState` → `TTextSessionState` → `TTextCompareSessionState` /
//! `TTextMergeSessionState`; `TTableSessionState`; `THexSessionState`; `TTagSessionState`;
//! `TRegSessionState`; `TDirSessionState` → `TDirCompareSessionState` /
//! `TDirMergeSessionState` / `TDirSyncSessionState`.

use bc_engine_dir::DisplayFilter;

/// Text view state (`TTextCompareSessionState`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextViewState {
    /// `<ShowContext Value="True"/>` – show context lines around differences.
    pub show_context: bool,
    /// Display filter: all / differences / same / context.
    pub display: TextDisplayFilter,
    pub ignore_unimportant: bool,
    pub word_wrap: bool,
    pub show_line_numbers: bool,
    pub show_whitespace: bool,
    /// Pane layout: side-by-side (`true`) or over/under.
    pub side_by_side: bool,
}

impl Default for TextViewState {
    fn default() -> Self {
        Self {
            show_context: true,
            display: TextDisplayFilter::All,
            ignore_unimportant: false,
            word_wrap: false,
            show_line_numbers: false,
            show_whitespace: false,
            side_by_side: true,
        }
    }
}

/// Text toolbar display filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextDisplayFilter {
    #[default]
    All,
    Differences,
    Same,
    /// Differences with context lines.
    Context,
}

/// Folder view state (`TDirCompareSessionState`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DirViewState {
    pub display: DisplayFilter,
    /// `/expandall` – expand all subfolders on load.
    pub expand_all: bool,
    pub show_excluded: bool,
    /// Visible columns (`column-size`, `column-timestamp`, …).
    pub columns: Vec<String>,
}

/// Generic key/value state for session kinds without a dedicated struct yet
/// (Hex/Picture/Media/Registry/Version/Table).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GenericState {
    pub values: Vec<(String, String)>,
}

/// Per-kind state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    Text(TextViewState),
    Dir(DirViewState),
    Other(GenericState),
}

impl Default for SessionState {
    fn default() -> Self {
        SessionState::Other(GenericState::default())
    }
}
