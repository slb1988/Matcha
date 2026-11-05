//! Folder Sync planning – BC `TDirSyncTree` / `TDirSyncSession` and the `sync` script
//! command (`sync [visible] [create-empty] (update|mirror):(left->right|right->left|all)`).

use crate::tree::DirTree;
use bc_core::Side;

/// Sync direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyncDirection {
    LeftToRight,
    RightToLeft,
    /// Bidirectional (`all`), only valid with [`SyncMode::Update`].
    Both,
}

/// Sync mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyncMode {
    /// Copy newer and orphan files in the direction; leave the other side's extras.
    Update(SyncDirection),
    /// Make target identical to source (deletes destination orphans).
    Mirror(SyncDirection),
}

impl Default for SyncMode {
    fn default() -> Self {
        SyncMode::Update(SyncDirection::LeftToRight)
    }
}

/// One planned action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncAction {
    Copy {
        relative: String,
        from: Side,
        to: Side,
    },
    Delete {
        relative: String,
        side: Side,
    },
    CreateFolder {
        relative: String,
        side: Side,
    },
    /// Shown but not acted on (conflict: both newer).
    Skip {
        relative: String,
        reason: String,
    },
}

/// Preview of what a sync will do (shown before execution, FR-D4).
#[derive(Debug, Clone, Default)]
pub struct SyncPlan {
    pub actions: Vec<SyncAction>,
}

impl SyncPlan {
    /// Build the plan from the current tree statuses.
    pub fn build(
        tree: &DirTree,
        mode: SyncMode,
        create_empty: bool,
        visible_only: bool,
    ) -> SyncPlan {
        todo!("bc-engine-dir::sync: plan actions from statuses")
    }

    /// Counts of (copies, deletes, creates, skips).
    pub fn summary(&self) -> (usize, usize, usize, usize) {
        todo!("bc-engine-dir::sync: summary")
    }
}
