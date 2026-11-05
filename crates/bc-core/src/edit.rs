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
    fn try_merge(&mut self, next: &dyn EditItem) -> bool {
        false
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
        todo!("bc-core::edit: undo group in reverse order")
    }

    fn redo(&mut self) {
        todo!("bc-core::edit: redo group in order")
    }
}

/// Undo/redo stack with grouping and save-point tracking – `TSsEditStack`.
#[derive(Debug, Default)]
pub struct EditStack {
    undo: Vec<Box<dyn EditItem>>,
    redo: Vec<Box<dyn EditItem>>,
    open_groups: Vec<EditGroup>,
    /// Depth of the undo stack when the document was last saved.
    saved_depth: Option<usize>,
    /// Maximum retained items (0 = unlimited).
    pub limit: usize,
}

impl EditStack {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an already-applied edit. Clears the redo stack.
    pub fn push(&mut self, item: Box<dyn EditItem>) {
        todo!("bc-core::edit: push (coalesce, group, truncate redo)")
    }

    /// Start a group; subsequent pushes go into it until [`Self::end_group`].
    pub fn begin_group(&mut self, description: impl Into<String>) {
        todo!("bc-core::edit: begin group")
    }

    /// Close the innermost group.
    pub fn end_group(&mut self) {
        todo!("bc-core::edit: end group")
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Undo one step; returns its description.
    pub fn undo(&mut self) -> Option<String> {
        todo!("bc-core::edit: undo")
    }

    /// Redo one step; returns its description.
    pub fn redo(&mut self) -> Option<String> {
        todo!("bc-core::edit: redo")
    }

    /// Mark the current state as saved (`is_dirty` becomes false).
    pub fn mark_saved(&mut self) {
        self.saved_depth = Some(self.undo.len());
    }

    /// `true` when the document differs from the last saved state.
    pub fn is_dirty(&self) -> bool {
        self.saved_depth != Some(self.undo.len())
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.open_groups.clear();
        self.saved_depth = None;
    }
}
