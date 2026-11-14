//! Session rules for text views – BC `TLineRules` → `TTextRules` → `TTextMergeRules`.
//! These are the values behind the Session Settings frames
//! (`TTextSessionImportanceFrame`, `TTextSessionAlignmentFrame`,
//! `TTextSessionReplacementsFrame`, `TTextMergeSessionAlignmentFrame`).

use bc_core::align::AlignStrategy;
use bc_formats::text_rules::{ElementImportance, Replacement, UnimportantText};
use bc_formats::WeightItem;

/// Which differences are shown / compared.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComparisonRules {
    pub importance: ElementImportance,
    pub unimportant_text: Vec<UnimportantText>,
    pub replacements: Vec<Replacement>,
    /// Compare only the first N characters of each line (0 = all).
    pub compare_line_prefix: u32,
}

/// Alignment settings – `TTextSessionAlignmentFrame`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlignmentRules {
    pub strategy: AlignStrategy,
    /// Standard's public "Use closeness matching" option.
    pub use_closeness: bool,
    /// Never align differences (show as pure insert/delete).
    pub never_align_differences: bool,
    /// Skew tolerance in lines for Smart alignment.
    pub skew_tolerance: u32,
    /// Per-session weight overrides merged with the format's weights.
    pub weights: Vec<WeightItem>,
}

impl Default for AlignmentRules {
    fn default() -> Self {
        Self {
            strategy: AlignStrategy::Smart,
            use_closeness: true,
            never_align_differences: false,
            skew_tolerance: 2000,
            weights: Vec::new(),
        }
    }
}

/// Text Compare / Text Edit rules – `TTextRules`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TextRules {
    pub comparison: ComparisonRules,
    pub alignment: AlignmentRules,
    /// Session-level format override (name), else resolved by mask.
    pub format_override: Option<String>,
    /// View/navigation projection only; original classifications stay intact.
    pub ignore_unimportant: bool,
}

impl TextRules {
    /// View-only ignore projection does not invalidate comparison work or caches.
    pub fn same_comparison(&self, other: &Self) -> bool {
        self.comparison == other.comparison
            && self.alignment == other.alignment
            && self.format_override == other.format_override
    }
}

/// Which side wins ignored unimportant conflicts – CLI `/favorleft` `/favorright`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Favor {
    #[default]
    None,
    Left,
    Right,
}

/// Text Merge rules – `TTextMergeRules` (adds merge-specific options).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextMergeRules {
    pub text: TextRules,
    /// Side taken for ignored unimportant conflicts (only with `ignore_unimportant`).
    pub favor: Favor,
    /// `/iu`: unimportant differences count as no difference.
    pub ignore_unimportant: bool,
    /// `/force`: write conflicts with CVS-style markers instead of failing.
    pub force_conflict_markers: bool,
    /// "A merge conflict is: left change and right change separated by N lines":
    /// opposite-side changes at most this many unchanged rows apart form one conflict.
    pub conflict_gap: u32,
}
impl Default for TextMergeRules {
    fn default() -> Self {
        Self {
            text: TextRules::default(),
            favor: Favor::None,
            ignore_unimportant: false,
            force_conflict_markers: false,
            conflict_gap: 2,
        }
    }
}
