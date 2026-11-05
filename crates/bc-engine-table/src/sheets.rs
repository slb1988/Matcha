use crate::{
    columns::{normalized_name, Side},
    invalid, Result,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SheetPair {
    pub left: Option<usize>,
    pub right: Option<usize>,
}
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SheetPairs(pub Vec<SheetPair>);
impl SheetPairs {
    pub fn by_name(left: &[String], right: &[String], left_is_source: bool) -> Self {
        if !left_is_source {
            return Self(
                Self::by_name(right, left, true)
                    .0
                    .into_iter()
                    .map(|p| SheetPair {
                        left: p.right,
                        right: p.left,
                    })
                    .collect(),
            );
        }
        let mut used = HashSet::new();
        let mut pairs = Vec::new();
        for (l, name) in left.iter().enumerate() {
            let name = normalized_name(name);
            let r = right
                .iter()
                .enumerate()
                .find(|(r, n)| !used.contains(r) && normalized_name(n) == name)
                .map(|(r, _)| r);
            if let Some(r) = r {
                used.insert(r);
            }
            pairs.push(SheetPair {
                left: Some(l),
                right: r,
            });
        }
        for r in 0..right.len() {
            if !used.contains(&r) {
                pairs.push(SheetPair {
                    left: None,
                    right: Some(r),
                });
            }
        }
        Self(pairs)
    }
    pub fn move_to(&mut self, from: usize, to: usize, side: Side) -> Result<()> {
        if from >= self.0.len() || to >= self.0.len() {
            return Err(invalid("sheet mapping index out of range"));
        }
        let get = |p: &SheetPair| match side {
            Side::Left => p.left,
            Side::Right => p.right,
        };
        let set = |p: &mut SheetPair, v| match side {
            Side::Left => p.left = v,
            Side::Right => p.right = v,
        };
        let value = get(&self.0[from]);
        if from < to {
            for i in from..to {
                let next = get(&self.0[i + 1]);
                set(&mut self.0[i], next);
            }
        } else {
            for i in (to + 1..=from).rev() {
                let prev = get(&self.0[i - 1]);
                set(&mut self.0[i], prev);
            }
        }
        set(&mut self.0[to], value);
        Ok(())
    }
}
