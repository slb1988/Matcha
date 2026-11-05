//! Undo/redo – BC `TSsEditStack` with `TSsEditItem` / `TSsEditParentItem` (grouping).

/// A reversible edit. Concrete items live next to the model they mutate
/// (e.g. `TSsLineListInsertItem` in [`crate::line`], `TTextMapEditItem` in bc-engine-text).
pub trait EditItem: std::fmt::Debug {
    /// Human readable description for the Undo menu.
    fn description(&self) -> String;
    /// Revert the change.
    fn undo(&mut self);
    /// Re-apply the change.
    fn redo(&mut self);
    /// Whether this item absorbed a directly following `next` (typing coalescing).
    /// Default: never.
    fn try_merge(&mut self, _next: &dyn EditItem) -> bool {
        false
    }
    /// Optional concrete type access for coalescing compatible edits.
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        None
    }
}

/// Group of items undone/redone as one step – `TSsEditParentItem`.
#[derive(Debug, Default)]
pub struct EditGroup {
    pub description: String,
    pub items: Vec<Box<dyn EditItem>>,
}

impl EditItem for EditGroup {
    fn description(&self) -> String {
        self.description.clone()
    }

    fn undo(&mut self) {
        for item in self.items.iter_mut().rev() {
            item.undo();
        }
    }

    fn redo(&mut self) {
        for item in &mut self.items {
            item.redo();
        }
    }
}

/// Undo/redo stack with grouping and save-point tracking – `TSsEditStack`.
#[derive(Debug)]
pub struct EditStack {
    undo: Vec<Box<dyn EditItem>>,
    redo: Vec<Box<dyn EditItem>>,
    open_groups: Vec<EditGroup>,
    /// Depth of the undo stack when the document was last saved.
    saved_depth: Option<usize>,
    /// Maximum retained items (0 = unlimited).
    pub limit: usize,
}

impl Default for EditStack {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            open_groups: Vec::new(),
            saved_depth: Some(0),
            limit: 0,
        }
    }
}

impl EditStack {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an already-applied edit. Clears the redo stack.
    pub fn push(&mut self, item: Box<dyn EditItem>) {
        // A new branch makes a saved state in the discarded future unreachable.
        if self
            .saved_depth
            .is_some_and(|depth| depth > self.undo.len())
        {
            self.saved_depth = None;
        }
        let may_merge = self.redo.is_empty() && self.saved_depth != Some(self.undo.len());
        self.redo.clear();
        if let Some(group) = self.open_groups.last_mut() {
            if !group
                .items
                .last_mut()
                .is_some_and(|last| last.try_merge(item.as_ref()))
            {
                group.items.push(item);
            }
        } else {
            if !may_merge
                || !self
                    .undo
                    .last_mut()
                    .is_some_and(|last| last.try_merge(item.as_ref()))
            {
                self.undo.push(item);
            }
            self.enforce_limit();
        }
    }

    /// Start a group; subsequent pushes go into it until [`Self::end_group`].
    pub fn begin_group(&mut self, description: impl Into<String>) {
        self.open_groups.push(EditGroup {
            description: description.into(),
            items: Vec::new(),
        });
    }

    /// Close the innermost group.
    pub fn end_group(&mut self) {
        let group = self
            .open_groups
            .pop()
            .expect("end_group without begin_group");
        if group.items.is_empty() {
            return;
        }
        if let Some(parent) = self.open_groups.last_mut() {
            parent.items.push(Box::new(group));
        } else {
            self.undo.push(Box::new(group));
            self.enforce_limit();
        }
    }

    pub fn can_undo(&self) -> bool {
        self.open_groups.is_empty() && !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        self.open_groups.is_empty() && !self.redo.is_empty()
    }

    /// Undo one step; returns its description.
    pub fn undo(&mut self) -> Option<String> {
        if !self.can_undo() {
            return None;
        }
        self.enforce_limit();
        let mut item = self.undo.pop()?;
        let description = item.description();
        item.undo();
        self.redo.push(item);
        Some(description)
    }

    /// Redo one step; returns its description.
    pub fn redo(&mut self) -> Option<String> {
        if !self.can_redo() {
            return None;
        }
        let mut item = self.redo.pop()?;
        let description = item.description();
        item.redo();
        self.undo.push(item);
        self.enforce_limit();
        Some(description)
    }

    /// Mark the current state as saved (`is_dirty` becomes false).
    pub fn mark_saved(&mut self) {
        assert!(
            self.open_groups.is_empty(),
            "close edit groups before saving"
        );
        self.saved_depth = Some(self.undo.len());
    }

    /// `true` when the document differs from the last saved state.
    pub fn is_dirty(&self) -> bool {
        self.saved_depth != Some(self.undo.len())
            || self.open_groups.iter().any(|g| !g.items.is_empty())
    }

    /// Set the history bound and immediately discard oldest undo entries.
    pub fn set_limit(&mut self, limit: usize) {
        self.limit = limit;
        self.enforce_limit();
    }

    fn enforce_limit(&mut self) {
        if self.limit != 0 && self.undo.len() + self.redo.len() > self.limit {
            let removed = (self.undo.len() + self.redo.len() - self.limit).min(self.undo.len());
            self.undo.drain(..removed);
            self.saved_depth = self
                .saved_depth
                .and_then(|depth| depth.checked_sub(removed));
        }
        // Redo entries are reversed: retain the nearest future states.
        if self.limit != 0 && self.redo.len() > self.limit {
            self.redo.drain(..self.redo.len() - self.limit);
            if self
                .saved_depth
                .is_some_and(|depth| depth > self.undo.len() + self.redo.len())
            {
                self.saved_depth = None;
            }
        }
    }

    /// Discard history, not document changes. Call mark_saved after a successful save.
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.open_groups.clear();
        self.saved_depth = None;
    }
}
