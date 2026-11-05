//! # bc-session
//!
//! Session, session-folder and workspace model plus read/write compatibility with the
//! reference tool's `BCSessions.xml` / `BCPreferences.xml` (spec §5, FR-X1).
//!
//! | module | BC classes |
//! |---|---|
//! | [`session`] | `TSession` → `TFileSession`/`TLineSession`/`TTextSession`/`TTextCompareSession`/`TTextMergeSession`/`TTextEditSession`/`TTextPatchSession`, `TTableSession`/`TTableCompareSession`, `THexSession`/`THexCompareSession`, `TTagSession`/`TPixCompareSession`/`TMediaCompareSession`/`TVerCompareSession`, `TRegSession`/`TRegCompareSession`, `TDirBaseSession`/`TDirSession`/`TDirCompareSession`/`TDirMergeSession`/`TDirSyncSession`, `TExternalSession` |
//! | [`state`] | `TSessionState` → `TFileSessionState` → `TTextSessionState`/`TTextCompareSessionState`/…, `TDirSessionState`/`TDirCompareSessionState`/… |
//! | [`folder`] | `TSessionFolder`, `TSessionManager`, `TSessionClassList` |
//! | [`workspace`] | `TWorkspaceItem`, `TWorkspaceList` |
//! | [`xml`] | `BCSessions.xml` reader/writer on top of `bc_core::persist` |
//! | [`prefs`] | `TBcPrefs` (`TPrefs`) subset used by the engines |
#![allow(unused_variables)] // scaffold: bodies are `todo!()`; remove once implemented
#![forbid(unsafe_code)]

pub mod folder;
pub mod prefs;
pub mod session;
pub mod state;
pub mod workspace;
pub mod xml;

pub use folder::{SessionFolder, SessionManager};
pub use session::{Session, SessionKind, SessionRules, SessionSpecs};
pub use state::SessionState;
pub use workspace::{Workspace, WorkspaceItem};

/// Session layer error.
#[derive(Debug)]
pub enum SessionError {
    Persist(bc_core::persist::PersistError),
    /// Element/class name not recognised (kept for write-through where possible).
    UnknownClass(String),
    NotFound(String),
    Io(std::io::Error),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionError::Persist(e) => write!(f, "{e}"),
            SessionError::UnknownClass(c) => write!(f, "unknown session class {c}"),
            SessionError::NotFound(n) => write!(f, "session not found: {n}"),
            SessionError::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<bc_core::persist::PersistError> for SessionError {
    fn from(e: bc_core::persist::PersistError) -> Self {
        SessionError::Persist(e)
    }
}

impl From<std::io::Error> for SessionError {
    fn from(e: std::io::Error) -> Self {
        SessionError::Io(e)
    }
}
