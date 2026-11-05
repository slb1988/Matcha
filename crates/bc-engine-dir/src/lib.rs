//! # bc-engine-dir
//!
//! Folder Compare / Folder Merge / Folder Sync engine (FR-D1…FR-D5).
//!
//! | module | BC classes |
//! |---|---|
//! | [`tree`] | `TDirTree` (`TSsMapTree`), `TDirSyncTree`, `TDirAlignWithItem/List` |
//! | [`criteria`] | comparison criteria of `TDirRules` (`criteria` script command) |
//! | [`filter`] | `TDirFilters`, `TDirOtherFilterItem/List`, name filter presets, display filter |
//! | [`rules`] | `TDirRules`, `TDirMergeRules`, `TDirSyncRules`, handling options |
//! | [`sync`] | sync plan / preview (`TDirSyncSession` behaviour, `sync` command) |
//! | [`ops`] | file operations (`TOp*` dialogs backend, `TOpMoveBothJob`) |
//! | [`jobs`] | `TVfsLoadDirJob` driver, `TDirTreeContentCompareJob`, `TQuickCompareFilesJob`, `TDirTreeContentSearchJob` |
#![forbid(unsafe_code)]

pub mod content;
pub mod criteria;
pub mod filter;
pub mod filter_script;
pub mod jobs;
pub mod ops;
pub mod op_exec;
pub mod op_script;
pub mod refresh;
pub mod rules;
pub mod sync;
pub mod tree;
pub mod view;

pub use criteria::{ContentCompare, Criteria};
pub use filter::{DisplayFilter, NameFilter, OtherFilters};
pub use rules::DirRules;
pub use sync::{SyncMode, SyncPlan};
pub use tree::{CompareStatus, DirNode, DirTree};
