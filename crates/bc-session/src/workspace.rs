//! Workspaces – BC `TWorkspaceItem` / `TWorkspaceList` (`TWORKSPACEMANAGERDIALOG`):
//! a named set of open sessions (tabs) restored together. Three-level organisation:
//! workspace > session > tab.

use crate::session::Session;

/// One open tab inside a workspace.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceItem {
    /// Saved session name, or an inline (unsaved) session.
    pub session: WorkspaceSession,
    pub active: bool,
}

/// Reference or inline session.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkspaceSession {
    Named(String),
    Inline(Box<Session>),
}

/// Named workspace.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Workspace {
    pub name: Option<String>,
    pub items: Vec<WorkspaceItem>,
    /// Main window geometry (`BCState.xml` keeps the running copy).
    pub window: Option<WindowPlacement>,
}

/// Window geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowPlacement {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub maximized: bool,
}

impl Workspace {
    /// Resolve the tab that should be active (first when none flagged).
    pub fn active_index(&self) -> Option<usize> {
        self.items.iter().position(|i| i.active).or(if self.items.is_empty() { None } else { Some(0) })
    }
}
