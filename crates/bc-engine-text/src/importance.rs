//! Importance classification – spec §6 "重要性传播":
//! grammar tokens → per-line importance flags → difference blocks → whole-block
//! downgrade when every differing token is unimportant.

use bc_formats::grammar::Token;
use bc_formats::text_rules::ElementImportance;

/// Importance level of a difference. Colour mapping (FR-T1): important = red,
/// unimportant = blue, insert/delete = green/..., same = none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum Importance {
    /// No difference.
    #[default]
    Same,
    /// Differs only in unimportant text (comments, whitespace per rules, …).
    Unimportant,
    /// Differs in important text.
    Important,
}

impl Importance {
    /// Collapse unimportant to same when the view ignores unimportant differences.
    pub fn with_ignore_unimportant(self, ignore: bool) -> Importance {
        if ignore && self == Importance::Unimportant {
            Importance::Same
        } else {
            self
        }
    }
}

/// Classify the difference between two token streams of a paired line.
pub fn classify_pair(
    left: &str,
    left_tokens: &[Token],
    right: &str,
    right_tokens: &[Token],
    rules: &ElementImportance,
) -> Importance {
    todo!("bc-engine-text::importance: token-level comparison with unimportant masking")
}

/// Propagate line importance over a contiguous difference block: the block is
/// unimportant only if *every* line pair in it is unimportant or same.
pub fn propagate_block(lines: &[Importance]) -> Importance {
    todo!("bc-engine-text::importance: block propagation")
}
