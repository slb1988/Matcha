//! Session tree – BC `TSessionFolder` (nested folders in the Home view) and
//! `TSessionManager` (owns the tree, loads/saves `BCSessions.xml`).

use crate::session::Session;
use crate::SessionError;
use std::path::{Path, PathBuf};

/// Folder of sessions and sub-folders (`<TSessionFolder><Items>…`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SessionFolder {
    pub name: Option<String>,
    pub folders: Vec<SessionFolder>,
    pub sessions: Vec<Session>,
}

impl SessionFolder {
    /// Find a saved session by display name (depth first) – `load "My Session"`.
    pub fn find(&self, name: &str) -> Option<&Session> {
        todo!("bc-session::folder: lookup by name")
    }

    /// Flatten all sessions.
    pub fn all_sessions(&self) -> Vec<&Session> {
        todo!("bc-session::folder: flatten")
    }
}

/// Owner of the persistent session tree – `TSessionManager` (`TBcSettingsManager`).
#[derive(Debug, Default)]
pub struct SessionManager {
    pub root: SessionFolder,
    path: Option<PathBuf>,
    dirty: bool,
}

impl SessionManager {
    /// Default settings directory (Windows: `%APPDATA%\<vendor>\<product>`), created on demand.
    pub fn default_settings_dir() -> Result<PathBuf, SessionError> {
        todo!("bc-session::folder: resolve settings directory via std::env (no hard-coded drive)")
    }

    /// Load `BCSessions.xml` from `path` (missing file = empty tree).
    pub fn load(path: &Path) -> Result<Self, SessionError> {
        todo!("bc-session::folder: read via xml::read_sessions")
    }

    /// Save back to the path it was loaded from.
    pub fn save(&mut self) -> Result<(), SessionError> {
        todo!("bc-session::folder: write via xml::write_sessions")
    }

    /// Insert or replace a named session.
    pub fn upsert(&mut self, folder_path: &[String], session: Session) {
        todo!("bc-session::folder: upsert")
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}
