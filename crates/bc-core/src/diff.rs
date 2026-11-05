//! Sequence diff: linear-space Myers O(ND), common-edge trimming, bounded block
//! fallback for huge inputs, and patience anchors. Operations partition both inputs.

use crate::{
    cancel::{CancelCheck, NeverCancel},
    hash::ContentHash,
    CoreError,
};
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffOp {
    Equal {
        a_start: usize,
        b_start: usize,
        len: usize,
    },
    Delete {
        a_start: usize,
        len: usize,
        b_pos: usize,
    },
    Insert {
        b_start: usize,
        len: usize,
        a_pos: usize,
    },
}

pub trait Differ {
    fn diff(&self, a: &[ContentHash], b: &[ContentHash]) -> Vec<DiffOp>;
}

/// Above 16K remaining items, content-defined block anchors bound worst-case work.
/// Set the threshold to zero for an exact LCS at any size (potentially quadratic time).
#[derive(Debug, Clone)]
pub struct MyersDiff {
    pub block_fallback_threshold: usize,
}

impl Default for MyersDiff {
    fn default() -> Self {
        Self {
            block_fallback_threshold: 16_384,
        }
    }
}

impl Differ for MyersDiff {
    fn diff(&self, a: &[ContentHash], b: &[ContentHash]) -> Vec<DiffOp> {
        self.diff_cancellable(a, b, &NeverCancel)
            .expect("never cancelled")
    }
}

impl MyersDiff {
    pub fn diff_cancellable(
        &self,
        a: &[ContentHash],
        b: &[ContentHash],
        cancel: &dyn CancelCheck,
    ) -> Result<Vec<DiffOp>, CoreError> {
        diff_ranges(a, b, self.block_fallback_threshold, false, cancel)
    }
}

#[derive(Debug, Clone, Default)]
pub struct PatienceDiff;

impl Differ for PatienceDiff {
    fn diff(&self, a: &[ContentHash], b: &[ContentHash]) -> Vec<DiffOp> {
        diff_ranges(
            a,
            b,
            MyersDiff::default().block_fallback_threshold,
            true,
            &NeverCancel,
        )
        .expect("never cancelled")
    }
}

#[derive(Debug)]
enum Work {
    Range(Range<usize>, Range<usize>, bool),
    Equal(usize, usize, usize),
}

fn diff_ranges(
    a: &[ContentHash],
    b: &[ContentHash],
    threshold: usize,
    patience: bool,
    cancel: &dyn CancelCheck,
) -> Result<Vec<DiffOp>, CoreError> {
    let mut out = Vec::new();
    let mut work = vec![Work::Range(0..a.len(), 0..b.len(), patience)];
    while let Some(job) = work.pop() {
        cancel.check()?;
        let (mut ar, mut br, patience) = match job {
            Work::Equal(a_start, b_start, len) => {
                emit(
                    &mut out,
                    DiffOp::Equal {
                        a_start,
                        b_start,
                        len,
                    },
                );
                continue;
            }
            Work::Range(ar, br, patience) => (ar, br, patience),
        };
        let prefix = common_edge(a[ar.clone()].iter().zip(&b[br.clone()]), cancel)?;
        emit(
            &mut out,
            DiffOp::Equal {
                a_start: ar.start,
                b_start: br.start,
                len: prefix,
            },
        );
        ar.start += prefix;
        br.start += prefix;
        let suffix = common_edge(
            a[ar.clone()].iter().rev().zip(b[br.clone()].iter().rev()),
            cancel,
        )?;
        ar.end -= suffix;
        br.end -= suffix;
        if suffix != 0 {
            work.push(Work::Equal(ar.end, br.end, suffix));
        }
        if patience && !ar.is_empty() && !br.is_empty() {
            let anchors = unique_anchors(&a[ar.clone()], &b[br.clone()], cancel)?;
            if !anchors.is_empty() {
                let mut jobs = Vec::with_capacity(anchors.len() * 2 + 1);
                let (mut ap, mut bp) = (ar.start, br.start);
                for (ai, bi) in anchors {
                    let (ai, bi) = (ar.start + ai, br.start + bi);
                    jobs.push(Work::Range(ap..ai, bp..bi, true));
                    jobs.push(Work::Equal(ai, bi, 1));
                    ap = ai + 1;
                    bp = bi + 1;
                }
                jobs.push(Work::Range(ap..ar.end, bp..br.end, true));
                work.extend(jobs.into_iter().rev());
                continue;
            }
            // No unique anchors in this subrange: use bounded Myers, not recursion.
        }
        if ar.is_empty() || br.is_empty() {
            replace(&mut out, ar, br);
        } else if ar.len() == 1 || br.len() == 1 {
            single(&mut out, a, b, ar, br);
        } else if threshold != 0 && ar.len().max(br.len()) > threshold {
            let jobs = block_jobs(a, b, ar, br, threshold, cancel)?;
            work.extend(jobs.into_iter().rev());
        } else if ar.len() + br.len() > 512 && disjoint(&a[ar.clone()], &b[br.clone()]) {
            replace(&mut out, ar, br);
        } else {
            let (x, y) = bisect(&a[ar.clone()], &b[br.clone()], cancel)?;
            assert!(
                (x, y) != (0, 0) && (x, y) != (ar.len(), br.len()),
                "Myers split must make progress"
            );
            let am = ar.start + x;
            let bm = br.start + y;
            work.push(Work::Range(am..ar.end, bm..br.end, false));
            work.push(Work::Range(ar.start..am, br.start..bm, false));
        }
    }
    cancel.check()?;
    Ok(out)
}

fn common_edge<'a>(
    pairs: impl Iterator<Item = (&'a ContentHash, &'a ContentHash)>,
    cancel: &dyn CancelCheck,
) -> Result<usize, CoreError> {
    let mut count = 0;
    for (a, b) in pairs {
        if count % 4096 == 0 {
            cancel.check()?;
        }
        if a != b {
            break;
        }
        count += 1;
    }
    Ok(count)
}

fn replace(out: &mut Vec<DiffOp>, a: Range<usize>, b: Range<usize>) {
    emit(
        out,
        DiffOp::Delete {
            a_start: a.start,
            len: a.len(),
            b_pos: b.start,
        },
    );
    emit(
        out,
        DiffOp::Insert {
            b_start: b.start,
            len: b.len(),
            a_pos: a.end,
        },
    );
}

fn single(
    out: &mut Vec<DiffOp>,
    a: &[ContentHash],
    b: &[ContentHash],
    ar: Range<usize>,
    br: Range<usize>,
) {
    if ar.len() == 1 {
        if let Some(j) = b[br.clone()].iter().position(|v| *v == a[ar.start]) {
            let j = br.start + j;
            emit(
                out,
                DiffOp::Insert {
                    b_start: br.start,
                    len: j - br.start,
                    a_pos: ar.start,
                },
            );
            emit(
                out,
                DiffOp::Equal {
                    a_start: ar.start,
                    b_start: j,
                    len: 1,
                },
            );
            emit(
                out,
                DiffOp::Insert {
                    b_start: j + 1,
                    len: br.end - j - 1,
                    a_pos: ar.end,
                },
            );
        } else {
            replace(out, ar, br);
        }
    } else if let Some(i) = a[ar.clone()].iter().position(|v| *v == b[br.start]) {
        let i = ar.start + i;
        emit(
            out,
            DiffOp::Delete {
                a_start: ar.start,
                len: i - ar.start,
                b_pos: br.start,
            },
        );
        emit(
            out,
            DiffOp::Equal {
                a_start: i,
                b_start: br.start,
                len: 1,
            },
        );
        emit(
            out,
            DiffOp::Delete {
                a_start: i + 1,
                len: ar.end - i - 1,
                b_pos: br.end,
            },
        );
    } else {
        replace(out, ar, br);
    }
}

/// Find the overlap of forward/reverse furthest-reaching paths. Frontier arrays
/// are released before processing subranges; no O(D²) trace or recursive call stack.
fn bisect(
    a: &[ContentHash],
    b: &[ContentHash],
    cancel: &dyn CancelCheck,
) -> Result<(usize, usize), CoreError> {
    let n = a.len() as isize;
    let m = b.len() as isize;
    let max_d = (n + m + 1) / 2;
    let offset = max_d + 1;
    let size = (2 * max_d + 3) as usize;
    let mut forward = vec![-1isize; size];
    let mut reverse = vec![-1isize; size];
    forward[(offset + 1) as usize] = 0;
    reverse[(offset + 1) as usize] = 0;
    let delta = n - m;
    let odd = delta % 2 != 0;
    for d in 0..=max_d {
        cancel.check()?;
        for k in (-d..=d).step_by(2) {
            let i = (offset + k) as usize;
            let mut x = if k == -d || (k != d && forward[i - 1] < forward[i + 1]) {
                forward[i + 1]
            } else {
                forward[i - 1] + 1
            };
            let mut y = x - k;
            while x < n && y < m && x >= 0 && y >= 0 && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }
            forward[i] = x;
            let other = offset + delta - k;
            if odd
                && (0..size as isize).contains(&other)
                && reverse[other as usize] >= 0
                && x <= n
                && y <= m
                && x >= n - reverse[other as usize]
            {
                return Ok((x as usize, y as usize));
            }
        }
        for k in (-d..=d).step_by(2) {
            let i = (offset + k) as usize;
            let mut x = if k == -d || (k != d && reverse[i - 1] < reverse[i + 1]) {
                reverse[i + 1]
            } else {
                reverse[i - 1] + 1
            };
            let mut y = x - k;
            while x < n
                && y < m
                && x >= 0
                && y >= 0
                && a[(n - x - 1) as usize] == b[(m - y - 1) as usize]
            {
                x += 1;
                y += 1;
            }
            reverse[i] = x;
            let other = offset + delta - k;
            if !odd
                && (0..size as isize).contains(&other)
                && forward[other as usize] >= 0
                && x <= n
                && y <= m
                && forward[other as usize] >= n - x
            {
                let ax = forward[other as usize];
                return Ok((ax as usize, (ax - (delta - k)) as usize));
            }
        }
    }
    unreachable!("forward and reverse Myers paths must intersect")
}

fn disjoint(a: &[ContentHash], b: &[ContentHash]) -> bool {
    let (small, large) = if a.len() < b.len() { (a, b) } else { (b, a) };
    let values: HashSet<_> = small.iter().collect();
    !large.iter().any(|v| values.contains(v))
}

/// Unique-in-both pairs, followed by an increasing subsequence on right indices.
/// Iterating the left input (not HashMap order) makes tie-breaking deterministic.
fn unique_anchors(
    a: &[ContentHash],
    b: &[ContentHash],
    cancel: &dyn CancelCheck,
) -> Result<Vec<(usize, usize)>, CoreError> {
    fn positions(
        values: &[ContentHash],
        cancel: &dyn CancelCheck,
    ) -> Result<HashMap<ContentHash, Option<usize>>, CoreError> {
        let mut map = HashMap::with_capacity(values.len());
        for (i, &value) in values.iter().enumerate() {
            if i % 4096 == 0 {
                cancel.check()?;
            }
            map.entry(value)
                .and_modify(|pos| *pos = None)
                .or_insert(Some(i));
        }
        Ok(map)
    }
    let left = positions(a, cancel)?;
    let right = positions(b, cancel)?;
    let mut pairs = Vec::new();
    for (i, value) in a.iter().enumerate() {
        if i % 4096 == 0 {
            cancel.check()?;
        }
        if left[value] == Some(i) {
            if let Some(j) = right.get(value).copied().flatten() {
                pairs.push((i, j));
            }
        }
    }
    let mut tails: Vec<usize> = Vec::new();
    let mut previous = vec![None; pairs.len()];
    for (i, &(_, j)) in pairs.iter().enumerate() {
        if i % 4096 == 0 {
            cancel.check()?;
        }
        let p = tails.partition_point(|&t| pairs[t].1 < j);
        if p > 0 {
            previous[i] = Some(tails[p - 1]);
        }
        if p == tails.len() {
            tails.push(i);
        } else {
            tails[p] = i;
        }
    }
    let mut result = Vec::with_capacity(tails.len());
    let mut cursor = tails.last().copied();
    while let Some(i) = cursor {
        result.push(pairs[i]);
        cursor = previous[i];
    }
    result.reverse();
    Ok(result)
}

struct Block {
    range: Range<usize>,
    hash: ContentHash,
}

/// Content-defined boundaries recover alignment after shifted insertions. A hard
/// maximum also bounds repetitive/adversarial data that has no natural boundaries.
fn blocks(
    values: &[ContentHash],
    range: Range<usize>,
    cancel: &dyn CancelCheck,
) -> Result<Vec<Block>, CoreError> {
    let mut result = Vec::new();
    let mut start = range.start;
    let mut hasher = xxhash_rust::xxh64::Xxh64::new(0);
    for i in range.clone() {
        if i % 4096 == 0 {
            cancel.check()?;
        }
        hasher.update(&values[i].0.to_le_bytes());
        let len = i + 1 - start;
        if (len >= 32 && values[i].0 & 63 == 0) || len == 128 || i + 1 == range.end {
            hasher.update(&(len as u64).to_le_bytes());
            result.push(Block {
                range: start..i + 1,
                hash: ContentHash(hasher.digest()),
            });
            hasher = xxhash_rust::xxh64::Xxh64::new(0);
            start = i + 1;
        }
    }
    Ok(result)
}

fn block_jobs(
    a: &[ContentHash],
    b: &[ContentHash],
    ar: Range<usize>,
    br: Range<usize>,
    threshold: usize,
    cancel: &dyn CancelCheck,
) -> Result<Vec<Work>, CoreError> {
    let ab = blocks(a, ar.clone(), cancel)?;
    let bb = blocks(b, br.clone(), cancel)?;
    let ah: Vec<_> = ab.iter().map(|block| block.hash).collect();
    let bh: Vec<_> = bb.iter().map(|block| block.hash).collect();
    let anchors = unique_anchors(&ah, &bh, cancel)?;
    let mut result = Vec::new();
    let (mut ap, mut bp) = (ar.start, br.start);
    for (ai, bi) in anchors {
        cancel.check()?;
        let (left, right) = (&ab[ai].range, &bb[bi].range);
        // Hash collisions must never manufacture equal line ranges.
        if a[left.clone()] != b[right.clone()] {
            continue;
        }
        windows(
            &mut result,
            ap..left.start,
            bp..right.start,
            threshold.min(1024),
        );
        result.push(Work::Equal(left.start, right.start, left.len()));
        ap = left.end;
        bp = right.end;
    }
    windows(&mut result, ap..ar.end, bp..br.end, threshold.min(1024));
    Ok(result)
}

fn windows(jobs: &mut Vec<Work>, mut a: Range<usize>, mut b: Range<usize>, limit: usize) {
    while !a.is_empty() || !b.is_empty() {
        let ae = a.start + a.len().min(limit);
        let be = b.start + b.len().min(limit);
        jobs.push(Work::Range(a.start..ae, b.start..be, false));
        a.start = ae;
        b.start = be;
    }
}

fn emit(out: &mut Vec<DiffOp>, op: DiffOp) {
    let len = match op {
        DiffOp::Equal { len, .. } | DiffOp::Delete { len, .. } | DiffOp::Insert { len, .. } => len,
    };
    if len == 0 {
        return;
    }
    match (out.last_mut(), &op) {
        (
            Some(DiffOp::Equal {
                a_start: pa,
                b_start: pb,
                len: pl,
            }),
            DiffOp::Equal {
                a_start, b_start, ..
            },
        ) if *pa + *pl == *a_start && *pb + *pl == *b_start => *pl += len,
        (
            Some(DiffOp::Delete {
                a_start: pa,
                b_pos: pb,
                len: pl,
            }),
            DiffOp::Delete { a_start, b_pos, .. },
        ) if *pa + *pl == *a_start && pb == b_pos => *pl += len,
        (
            Some(DiffOp::Insert {
                b_start: pb,
                a_pos: pa,
                len: pl,
            }),
            DiffOp::Insert { b_start, a_pos, .. },
        ) if *pb + *pl == *b_start && pa == a_pos => *pl += len,
        _ => out.push(op),
    }
}
