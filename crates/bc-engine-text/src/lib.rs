//! # bc-engine-text
//!
//! Text comparison and merge engine (FR-T1…FR-T8). Sits on bc-core (lines, diff,
//! alignment, undo), bc-formats (grammar, importance rules) and bc-jobs (background
//! alignment). Knows nothing about the GUI.
//!
//! | module | BC classes |
//! |---|---|
//! | [`content`] | `TLineContent`, `TLineConLineList`, `TFileContent` |
//! | [`rules`] | `TTextRules`, `TTextMergeRules`, `TLineRules`, `TTextWeightList` |
//! | [`map`] | `TTextMap`, `TTextMapAlignLines`, `TTextMap*Item` (undoable edits) |
//! | [`importance`] | importance propagation (`TTextSessionImportanceFrame` semantics) |
//! | [`inline`] | character-level differences within a changed line pair |
//! | [`merge`] | 3-way merge state, conflicts, favor rules (`TTextMergeSession` behaviour) |
//! | [`jobs`] | `TTextMapJob`, `TTextMapAlign2Job`, `TTextMapRealign2Job`, `TTextMapAlign3Job`, `TTextMapRealign3Job` |
//! | [`search`] | find / find-in-files (`TTEXTFINDINFILESDIALOG` backend) |
#![forbid(unsafe_code)]

pub mod conflict;
pub mod content;
pub mod importance;
pub mod inline;
pub mod jobs;
pub mod map;
pub mod merge;
pub mod rules;
pub mod search;

pub use content::TextContent;
pub use importance::Importance;
pub use map::{TextMap, TextMapRow};
pub use rules::{TextMergeRules, TextRules};
