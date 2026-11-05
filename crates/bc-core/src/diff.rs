//! Sequence diff. Spec §6: Myers O(ND) baseline with common prefix/suffix trimming and
//! hashed-block degradation for huge inputs. The aligner ([`crate::align`]) sits on top.

use crate::hash::ContentHash;

/// One edit-script operation over index ranges of the two inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffOp {
    /// `a[a_start..a_start+len]` equals `b[b_start..b_start+len]`.
    Equal {
        a_start: usize,
        b_start: usize,
        len: usize,
    },
    /// Items only in `a`.
    Delete {
        a_start: usize,
        len: usize,
        b_pos: usize,
    },
    /// Items only in `b`.
    Insert {
        b_start: usize,
        len: usize,
        a_pos: usize,
    },
}

/// Diff algorithm abstraction so Quick/Smart alignment can plug different cores.
pub trait Differ {
    /// Diff two hashed sequences.
    fn diff(&self, a: &[ContentHash], b: &[ContentHash]) -> Vec<DiffOp>;
}

/// Myers O(ND) with prefix/suffix trimming (spec §6 baseline).
#[derive(Debug, Clone, Default)]
pub struct MyersDiff {
    /// Above this many items fall back to block-hash diffing (0 = never).
    pub block_fallback_threshold: usize,
}

impl Differ for MyersDiff {
    #[allow(unused_variables)] // remaining scaffold entry point
    fn diff(&self, a: &[ContentHash], b: &[ContentHash]) -> Vec<DiffOp> {
        todo!("bc-core::diff: Myers O(ND)")
    }
}

/// Patience diff used by BC's `TBcPatienceDiffAlign` (a `TBcSmartAlign` subclass).
#[derive(Debug, Clone, Default)]
pub struct PatienceDiff;

impl Differ for PatienceDiff {
    #[allow(unused_variables)] // remaining scaffold entry point
    fn diff(&self, a: &[ContentHash], b: &[ContentHash]) -> Vec<DiffOp> {
        todo!("bc-core::diff: patience diff")
    }
}
