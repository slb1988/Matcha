//! File operations on the selection – backend of BC's `TOp*` dialogs
//! (`TOPEXCLUDEDIALOG`, `TOPDOSATTRIBDIALOG`, `TOPUNIXATTRIBDIALOG`, …) and of the
//! `copy`/`copyto`/`move`/`moveto`/`delete`/`rename`/`touch`/`attrib` script commands (FR-D3).

use bc_core::Side;
use bc_vfs::FileAttrs;
use std::time::SystemTime;

/// `copyto`/`moveto` path option: `path:(relative|base|none)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PathMode {
    #[default]
    None,
    Relative,
    Base,
}

/// Which side(s) an operation targets: `left|right|all`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Target {
    Left,
    Right,
    #[default]
    All,
}

/// Rename specification – `rename [regexpr <old mask>] <new mask>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenameSpec {
    /// DOS-style mask rename (`*.bak`).
    Dos { new_mask: String },
    /// Regex rename with capture substitution (`(...)(...)\.txt` → `$2$1.txt`).
    Regex { old_mask: String, new_mask: String },
}

/// `touch` specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TouchSpec {
    /// Copy timestamps across (`touch left->right`).
    Copy { from: Side, to: Side },
    /// Set to now or an explicit time (`touch left:now`).
    Set { target: Target, when: Option<SystemTime> },
}

/// A file operation over the current selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirOp {
    Copy { from: Side, to: Side },
    CopyTo { target: Target, path_mode: PathMode, dest: String },
    Move { from: Side, to: Side },
    MoveTo { target: Target, path_mode: PathMode, dest: String },
    Delete { target: Target, recycle_bin: bool },
    Rename(RenameSpec),
    Touch(TouchSpec),
    /// `attrib +sh -a`: `set` bits to add, `clear` bits to remove.
    Attrib { set: FileAttrs, clear: FileAttrs },
    /// Exclude selection from the comparison (adds name filter entries).
    Exclude,
}

/// Outcome log entry (shown in the log pane and script log).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpLogEntry {
    pub relative: String,
    pub op: String,
    pub ok: bool,
    pub message: String,
}
