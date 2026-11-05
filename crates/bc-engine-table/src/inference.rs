use crate::{
    columns::{row_hash, Side},
    rules::KeySelection,
    Result, Sheet, TableRules,
};
use bc_core::{cancel::CancelCheck, hash::ContentHash};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct KeyCandidate {
    pub columns: Vec<usize>,
    pub unique_left: f64,
    pub unique_right: f64,
    pub overlap: f64,
    pub stability: f64,
    pub position: f64,
    pub score: f64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct KeySuggestion {
    pub selection: KeySelection,
    pub candidates: Vec<KeyCandidate>,
}
fn values(
    sheet: &Sheet,
    columns: &[usize],
    rules: &TableRules,
    side: Side,
    cancel: &dyn CancelCheck,
) -> Result<HashMap<ContentHash, usize>> {
    let mapped: Vec<_> = columns
        .iter()
        .map(|&c| rules.columns.0[c].index(side).unwrap())
        .collect();
    let handling: Vec<_> = columns.iter().map(|&c| rules.handling(c)).collect();
    let mut set = HashMap::with_capacity(sheet.row_count());
    for row in 0..sheet.row_count() {
        if row.is_multiple_of(4096) {
            cancel.check()?;
        }
        set.entry(row_hash(sheet, row, &mapped, &handling))
            .or_insert(row);
    }
    Ok(set)
}
fn score(
    left: &Sheet,
    right: &Sheet,
    rules: &TableRules,
    columns: Vec<usize>,
    cancel: &dyn CancelCheck,
) -> Result<KeyCandidate> {
    let l = values(left, &columns, rules, Side::Left, cancel)?;
    let r = values(right, &columns, rules, Side::Right, cancel)?;
    let ul = l.len() as f64 / left.row_count().max(1) as f64;
    let ur = r.len() as f64 / right.row_count().max(1) as f64;
    let mut matches = 0;
    let mut displacement = 0.0;
    for (i, (hash, &lr)) in l.iter().enumerate() {
        if i.is_multiple_of(4096) {
            cancel.check()?;
        }
        if let Some(&rr) = r.get(hash) {
            matches += 1;
            displacement += (lr as f64 / left.row_count().max(1) as f64
                - rr as f64 / right.row_count().max(1) as f64)
                .abs();
        }
    }
    let overlap = matches as f64 / (l.len() + r.len() - matches).max(1) as f64;
    let stability = if matches == 0 {
        0.0
    } else {
        1.0 - displacement / matches as f64
    };
    let position = 1.0 - columns[0] as f64 / rules.columns.0.len().max(1) as f64;
    let score = 0.2 * ul + 0.2 * ur + 0.35 * overlap + 0.2 * stability + 0.05 * position;
    Ok(KeyCandidate {
        columns,
        unique_left: ul,
        unique_right: ur,
        overlap,
        stability,
        position,
        score,
    })
}
/// Full-column uniqueness, Jaccard, relative position stability, left-position
/// prior; adjacent pairs only when no unique single column passes 0.8.
pub fn infer_keys(
    left: &Sheet,
    right: &Sheet,
    rules: &TableRules,
    cancel: &dyn CancelCheck,
) -> Result<KeySuggestion> {
    rules.validate(left.column_count(), right.column_count())?;
    let usable: Vec<_> = rules
        .columns
        .0
        .iter()
        .enumerate()
        .filter(|(_, c)| c.left.is_some() && c.right.is_some())
        .map(|(i, _)| i)
        .collect();
    let mut candidates = Vec::new();
    for &c in &usable {
        candidates.push(score(left, right, rules, vec![c], cancel)?);
    }
    fn best(candidates: &[KeyCandidate]) -> Option<&KeyCandidate> {
        candidates
            .iter()
            .filter(|c| c.unique_left == 1.0 && c.unique_right == 1.0 && c.score >= 0.8)
            .max_by(|a, b| {
                a.score
                    .total_cmp(&b.score)
                    .then_with(|| b.columns.cmp(&a.columns))
            })
    }
    if best(&candidates).is_none() {
        for pair in usable.windows(2) {
            if pair[1] == pair[0] + 1 {
                candidates.push(score(left, right, rules, pair.to_vec(), cancel)?);
            }
        }
    }
    let selection =
        best(&candidates).map_or(KeySelection::AllColumns, |c| KeySelection::Automatic {
            columns: c.columns.clone(),
            score: c.score,
        });
    Ok(KeySuggestion {
        selection,
        candidates,
    })
}
impl KeySuggestion {
    /// A user edit locks the selection; background suggestions never overwrite it.
    pub fn apply(&self, rules: &mut TableRules) -> bool {
        if matches!(
            rules.keys,
            KeySelection::Manual(_) | KeySelection::ByColumnNumber
        ) {
            return false;
        }
        rules.keys = self.selection.clone();
        true
    }
}
