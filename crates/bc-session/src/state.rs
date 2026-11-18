//! View state saved with a session – BC `TSessionState` hierarchy (`<State>` element):
//! `TFileSessionState` → `TTextSessionState` → `TTextCompareSessionState` /
//! `TTextMergeSessionState`; `TTableSessionState`; `THexSessionState`; `TTagSessionState`;
//! `TRegSessionState`; `TDirSessionState` → `TDirCompareSessionState` /
//! `TDirMergeSessionState` / `TDirSyncSessionState`.
//!
//! Which keys each class writes is only partly known (observed: `ShowContext`,
//! `DirHandling`, `SortSide`, `DisplayFilter`), so the state is kept as the ordered
//! list of `<Key Value="…"/>` children with typed accessors on top.

use bc_core::persist::Node;

/// Text sessions: show context lines around differences (`True`/`False`).
pub const SHOW_CONTEXT: &str = "ShowContext";
/// Folder sessions: `dirCompareFilesOnly`, …
pub const DIR_HANDLING: &str = "DirHandling";

/// `<State>` children in document order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionState {
    pub values: Vec<Node>,
}

impl SessionState {
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// `Value` of key `name`.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.values.iter().find(|n| n.name == name).and_then(Node::value_str)
    }

    /// Delphi boolean (`True`/`False`, case-insensitive); anything else is `None`.
    pub fn get_bool(&self, name: &str) -> Option<bool> {
        match self.get(name)? {
            v if v.eq_ignore_ascii_case("true") => Some(true),
            v if v.eq_ignore_ascii_case("false") => Some(false),
            _ => None,
        }
    }

    /// Replace the value of `name` in place, or append it.
    pub fn set(&mut self, name: &str, value: impl Into<String>) {
        let value = Some(value.into());
        match self.values.iter_mut().find(|n| n.name == name) {
            Some(node) => node.value = value,
            None => self.values.push(Node {
                value,
                ..Node::new(name)
            }),
        }
    }

    pub fn set_bool(&mut self, name: &str, value: bool) {
        self.set(name, if value { "True" } else { "False" });
    }

    pub fn remove(&mut self, name: &str) {
        self.values.retain(|n| n.name != name);
    }
}
