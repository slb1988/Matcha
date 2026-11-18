//! Session tree – BC `TSessionFolder` (nested folders in the Home view) and
//! `TSessionManager` (owns the tree, loads/saves `BCSessions.xml`).

use crate::session::Session;
use crate::SessionError;
use bc_core::persist::Node;
use std::path::{Path, PathBuf};

/// File name of the session store inside the settings directory.
pub const SESSIONS_FILE: &str = "BCSessions.xml";

/// One entry of a folder, in document order.
#[derive(Debug, Clone, PartialEq)]
pub enum FolderItem {
    Folder(SessionFolder),
    Session(Box<Session>),
    /// Element of an unknown class (e.g. `TExternalSession`), kept verbatim.
    Raw(Node),
}

/// Folder of sessions and sub-folders (`<TSessionFolder Value="name"><Items>…`).
/// The document root (`<BCSessions>`) is the unnamed top folder.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SessionFolder {
    pub name: Option<String>,
    pub items: Vec<FolderItem>,
}

impl SessionFolder {
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            items: Vec::new(),
        }
    }

    /// Direct sub-folders.
    pub fn folders(&self) -> impl Iterator<Item = &SessionFolder> {
        self.items.iter().filter_map(|i| match i {
            FolderItem::Folder(f) => Some(f),
            _ => None,
        })
    }

    /// Direct sessions.
    pub fn sessions(&self) -> impl Iterator<Item = &Session> {
        self.items.iter().filter_map(|i| match i {
            FolderItem::Session(s) => Some(s.as_ref()),
            _ => None,
        })
    }

    /// Find a saved session by display name (depth first) – `load "My Session"`.
    pub fn find(&self, name: &str) -> Option<&Session> {
        self.items.iter().find_map(|item| match item {
            FolderItem::Session(s) if s.name.as_deref() == Some(name) => Some(s.as_ref()),
            FolderItem::Folder(f) => f.find(name),
            _ => None,
        })
    }

    /// Flatten all sessions (depth first, document order).
    pub fn all_sessions(&self) -> Vec<&Session> {
        let mut out = Vec::new();
        self.collect(&mut out);
        out
    }

    fn collect<'a>(&'a self, out: &mut Vec<&'a Session>) {
        for item in &self.items {
            match item {
                FolderItem::Session(s) => out.push(s),
                FolderItem::Folder(f) => f.collect(out),
                FolderItem::Raw(_) => {}
            }
        }
    }

    /// Sub-folder at `path` (by name), created on demand.
    fn folder_mut(&mut self, path: &[String]) -> &mut SessionFolder {
        let Some((first, rest)) = path.split_first() else {
            return self;
        };
        let index = self
            .items
            .iter()
            .position(|i| matches!(i, FolderItem::Folder(f) if f.name.as_deref() == Some(first)))
            .unwrap_or_else(|| {
                self.items
                    .push(FolderItem::Folder(SessionFolder::named(first.clone())));
                self.items.len() - 1
            });
        match &mut self.items[index] {
            FolderItem::Folder(f) => f.folder_mut(rest),
            _ => unreachable!("index points at a folder"),
        }
    }

    /// Remove the first session named `name` anywhere in the tree.
    pub fn remove(&mut self, name: &str) -> Option<Session> {
        if let Some(index) = self
            .items
            .iter()
            .position(|i| matches!(i, FolderItem::Session(s) if s.name.as_deref() == Some(name)))
        {
            return match self.items.remove(index) {
                FolderItem::Session(s) => Some(*s),
                _ => unreachable!("index points at a session"),
            };
        }
        self.items.iter_mut().find_map(|item| match item {
            FolderItem::Folder(f) => f.remove(name),
            _ => None,
        })
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
    /// Settings directory of this project (Windows: `%APPDATA%\Matcha`; elsewhere
    /// `$XDG_CONFIG_HOME/Matcha`), shared with the format library and preferences.
    pub fn default_settings_dir() -> Result<PathBuf, SessionError> {
        let formats = bc_formats::registry::user_formats_dir()
            .map_err(|e| SessionError::NotFound(e.to_string()))?;
        formats
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| SessionError::NotFound("settings directory".into()))
    }

    /// `<settings dir>\BCSessions.xml`.
    pub fn default_path() -> Result<PathBuf, SessionError> {
        Ok(Self::default_settings_dir()?.join(SESSIONS_FILE))
    }

    /// Load `BCSessions.xml` from `path` (missing file = empty tree).
    pub fn load(path: &Path) -> Result<Self, SessionError> {
        let root = match std::fs::read_to_string(path) {
            Ok(text) => crate::xml::read_sessions(&text)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => SessionFolder::default(),
            Err(e) => return Err(e.into()),
        };
        Ok(Self {
            root,
            path: Some(path.to_path_buf()),
            dirty: false,
        })
    }

    /// Save back to the path it was loaded from (atomic replace).
    pub fn save(&mut self) -> Result<(), SessionError> {
        let path = self
            .path
            .clone()
            .ok_or_else(|| SessionError::NotFound("session file path".into()))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temp = path.with_file_name(format!(".{SESSIONS_FILE}.{}.tmp", std::process::id()));
        std::fs::write(&temp, crate::xml::write_sessions(&self.root))?;
        if let Err(e) = std::fs::rename(&temp, &path) {
            let _ = std::fs::remove_file(&temp);
            return Err(e.into());
        }
        self.dirty = false;
        Ok(())
    }

    /// Insert or replace a named session. A session of the same name is replaced
    /// in place when it lives in the target folder; elsewhere it is removed, so
    /// names stay unique across the tree.
    pub fn upsert(&mut self, folder_path: &[String], session: Session) {
        self.dirty = true;
        let name = session.name.clone();
        let folder = self.root.folder_mut(folder_path);
        if let Some(name) = name.as_deref() {
            if let Some(slot) = folder
                .items
                .iter_mut()
                .find(|i| matches!(i, FolderItem::Session(s) if s.name.as_deref() == Some(name)))
            {
                *slot = FolderItem::Session(Box::new(session));
                return;
            }
            self.root.remove(name);
        }
        self.root
            .folder_mut(folder_path)
            .items
            .push(FolderItem::Session(Box::new(session)));
    }

    /// Delete a named session; `true` when it existed.
    pub fn remove(&mut self, name: &str) -> bool {
        let removed = self.root.remove(name).is_some();
        self.dirty |= removed;
        removed
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
}
