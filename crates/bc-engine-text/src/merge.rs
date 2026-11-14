//! Three-way merge – behaviour of BC `TTextMergeSession` / `TTextMergeRules` and the
//! `/automerge` `/favorleft|right` `/force` `/iu` switches (FR-T7).
//! Alignment is center-based (`TTextMapAlign3Job`), see `jobs::Align3Job`.
//!
//! Two layers, deliberately separate:
//!
//! * **Row type** decides content. Every aligned row is unchanged, changed on the left,
//!   changed on the right, changed identically on both sides, or changed differently.
//! * **Conflict sections** are a review flag on ranges of rows: a change run that holds
//!   both sides' changes, or two runs with opposite-side changes that are at most
//!   `conflict_gap` unchanged rows apart. Non-overlapping changes inside a conflict
//!   section are still taken; only rows changed differently keep the center.

use crate::importance::{CompiledComparison, Importance};
use crate::jobs::pair_importance;
use crate::rules::{Favor, TextMergeRules};
use crate::{TextContent, TextMap, TextMapRow};
use bc_core::{
    cancel::NeverCancel,
    encoding::{self, BomPolicy, Encoding},
    line::LineEnding,
    CoreError, Side,
};
use std::ops::Range;

/// Change type of one aligned row relative to the center.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeChoice {
    /// Both sides equal the center.
    Center,
    /// Only the left differs from the center.
    Left,
    /// Only the right differs from the center.
    Right,
    /// Both differ from the center and equal each other.
    Both,
    /// Both differ from the center and from each other.
    Conflict,
}

/// Per-row merge state (one entry per [`TextMap`] row).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MergeRow {
    pub choice: MergeChoice,
    /// Side this row was explicitly taken from (a `resolve`, or favor on an ignored
    /// unimportant conflict). `None` = the default for `choice`.
    pub resolved: Option<Side>,
    /// Largest importance among the row's differences (left/center, center/right,
    /// left/right), regardless of `ignore_unimportant`.
    pub importance: Importance,
}

/// Unresolved conflict section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub rows: Range<usize>,
    pub importance: Importance,
}

/// Outcome of an automatic merge (drives exit codes 0/14/101).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoMergeOutcome {
    pub conflicts: Vec<Conflict>,
    /// Encoded output (with conflict markers when `force_conflict_markers`); empty when
    /// withheld.
    pub output: Vec<u8>,
    /// `true` when output was not produced because conflicts remain (exit 101).
    pub withheld: bool,
    /// Non-fatal remarks: mixed line endings, disagreeing input encodings.
    pub warnings: Vec<String>,
}

/// Labels written after `<<<<<<<` and `>>>>>>>`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MergeLabels {
    pub left: String,
    pub right: String,
}

/// Where an output line comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Center,
    Left,
    Right,
    /// A conflict marker line.
    Marker,
}

/// One line of the merge output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputLine {
    pub text: Vec<u8>,
    pub ending: LineEnding,
    pub origin: Origin,
    /// Aligned row this line belongs to (the first row of the section for markers).
    pub row: usize,
}

#[derive(Debug, Clone)]
struct Section {
    rows: Range<usize>,
    importance: Importance,
    resolved: bool,
}

/// Merge model over a three-way [`TextMap`].
#[derive(Debug)]
pub struct MergeState {
    pub rows: Vec<MergeRow>,
    pub rules: TextMergeRules,
    /// Change runs: maximal ranges of rows that are not `Center`.
    runs: Vec<Range<usize>>,
    conflicts: Vec<Section>,
}

fn differs(importance: Importance, ignore_unimportant: bool) -> bool {
    match importance {
        Importance::Same => false,
        Importance::Unimportant => !ignore_unimportant,
        Importance::Important => true,
    }
}

/// `raw` = importance of left/center, center/right, left/right.
fn choice_of(raw: [Importance; 3], ignore_unimportant: bool) -> MergeChoice {
    match (
        differs(raw[0], ignore_unimportant),
        differs(raw[1], ignore_unimportant),
    ) {
        (false, false) => MergeChoice::Center,
        (true, false) => MergeChoice::Left,
        (false, true) => MergeChoice::Right,
        (true, true) if differs(raw[2], ignore_unimportant) => MergeChoice::Conflict,
        (true, true) => MergeChoice::Both,
    }
}

/// Change runs and conflict ranges for a sequence of row types.
fn sections(choices: &[MergeChoice], gap: usize) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    struct Group {
        rows: Range<usize>,
        left: bool,
        right: bool,
    }
    let mut runs = Vec::new();
    let mut conflicts = Vec::new();
    let mut group: Option<Group> = None;
    let close = |group: Option<Group>, conflicts: &mut Vec<Range<usize>>| {
        if let Some(group) = group.filter(|g| g.left && g.right) {
            conflicts.push(group.rows);
        }
    };
    let mut i = 0;
    while i < choices.len() {
        if choices[i] == MergeChoice::Center {
            i += 1;
            continue;
        }
        let start = i;
        let (mut left, mut right) = (false, false);
        while i < choices.len() && choices[i] != MergeChoice::Center {
            left |= matches!(choices[i], MergeChoice::Left | MergeChoice::Conflict);
            right |= matches!(choices[i], MergeChoice::Right | MergeChoice::Conflict);
            i += 1;
        }
        runs.push(start..i);
        group = match group.take() {
            Some(mut open)
                if start - open.rows.end <= gap
                    && ((open.left && right) || (open.right && left)) =>
            {
                open.rows.end = i;
                open.left |= left;
                open.right |= right;
                Some(open)
            }
            previous => {
                close(previous, &mut conflicts);
                Some(Group {
                    rows: start..i,
                    left,
                    right,
                })
            }
        };
    }
    close(group, &mut conflicts);
    (runs, conflicts)
}

fn content(map: &TextMap, side: Side) -> Option<&TextContent> {
    match side {
        Side::Left => Some(&map.left),
        Side::Right => Some(&map.right),
        Side::Center => map.center.as_ref(),
        Side::Output => None,
    }
}

fn line_of(map: &TextMap, side: Side, row: &TextMapRow) -> Option<(Vec<u8>, LineEnding)> {
    let index = match side {
        Side::Left => row.left,
        Side::Right => row.right,
        Side::Center => row.center,
        Side::Output => None,
    }?;
    let lines = content(map, side)?.lines.borrow();
    Some((lines.line_bytes(index)?.to_vec(), lines.ending(index)?))
}

/// Ending for marker lines and for lines that lost theirs by moving off the end of a
/// file: the first terminated line of the center, then left, then right, else LF.
fn fallback_ending(map: &TextMap) -> LineEnding {
    [Side::Center, Side::Left, Side::Right]
        .into_iter()
        .filter_map(|side| content(map, side))
        .find_map(|content| {
            content
                .lines
                .borrow()
                .iter()
                .map(|(_, ending)| ending)
                .find(|ending| *ending != LineEnding::None)
        })
        .unwrap_or(LineEnding::Lf)
}

fn encoding_of(content: &TextContent) -> (Encoding, bool) {
    (
        content.encoding,
        content.detection.as_ref().is_some_and(|d| d.has_bom),
    )
}

/// Encoding and BOM of the output: a three-way pick on the file attribute itself.
/// The `bool` flags the case where both sides moved away from the center differently
/// (the center's encoding is kept).
pub fn output_encoding(map: &TextMap) -> ((Encoding, bool), bool) {
    let (left, right) = (encoding_of(&map.left), encoding_of(&map.right));
    let center = map.center.as_ref().map_or(left, encoding_of);
    match (left == center, right == center) {
        (true, true) => (center, false),
        (false, true) => (left, false),
        (true, false) => (right, false),
        (false, false) if left == right => (left, false),
        (false, false) => (center, true),
    }
}

impl MergeState {
    /// Classify every row of a three-way alignment and find the conflict sections.
    pub fn classify(map: &TextMap, rules: &TextMergeRules) -> Result<Self, CoreError> {
        let center = map
            .center
            .as_ref()
            .ok_or_else(|| CoreError::Invalid("a merge needs a center file".into()))?;
        let comparison = CompiledComparison::new(&map.rules.comparison)?;
        let (l, c, r) = (map.left.snapshot(), center.snapshot(), map.right.snapshot());
        let cancel = NeverCancel;
        let mut raw = Vec::with_capacity(map.row_count());
        for row in map.rows() {
            let lc = pair_importance(&l, &c, row.left, row.center, &comparison, &cancel)?;
            let cr = pair_importance(&c, &r, row.center, row.right, &comparison, &cancel)?;
            let lr = if lc == Importance::Same && cr == Importance::Same {
                Importance::Same
            } else {
                pair_importance(&l, &r, row.left, row.right, &comparison, &cancel)?
            };
            raw.push([lc, cr, lr]);
        }
        let ignore = rules.ignore_unimportant;
        let gap = rules.conflict_gap as usize;
        let importance_of = |rows: &Range<usize>| {
            raw[rows.clone()]
                .iter()
                .flatten()
                .copied()
                .max()
                .unwrap_or(Importance::Same)
        };
        let choices: Vec<MergeChoice> = raw.iter().map(|r| choice_of(*r, ignore)).collect();
        let (runs, conflict_rows) = sections(&choices, gap);
        let mut rows: Vec<MergeRow> = choices
            .iter()
            .zip(&raw)
            .map(|(&choice, r)| MergeRow {
                choice,
                resolved: None,
                importance: r[0].max(r[1]).max(r[2]),
            })
            .collect();
        // Ignored unimportant conflicts: what would be a conflict made of unimportant
        // differences only. They read as unchanged (center) unless a side is favored.
        let favored = match rules.favor {
            Favor::None => None,
            Favor::Left => Some(Side::Left),
            Favor::Right => Some(Side::Right),
        };
        if let (true, Some(side)) = (ignore, favored) {
            let unfiltered: Vec<MergeChoice> = raw.iter().map(|r| choice_of(*r, false)).collect();
            for range in sections(&unfiltered, gap).1 {
                if importance_of(&range) == Importance::Unimportant {
                    for row in &mut rows[range] {
                        row.resolved = Some(side);
                    }
                }
            }
        }
        let conflicts = conflict_rows
            .into_iter()
            .map(|rows| Section {
                importance: importance_of(&rows),
                rows,
                resolved: false,
            })
            .collect();
        Ok(Self {
            rows,
            rules: rules.clone(),
            runs,
            conflicts,
        })
    }

    /// Unresolved conflict sections, in row order.
    pub fn conflicts(&self) -> Vec<Conflict> {
        self.conflicts
            .iter()
            .filter(|c| !c.resolved)
            .map(|c| Conflict {
                rows: c.rows.clone(),
                importance: c.importance,
            })
            .collect()
    }

    /// Every conflict section with its resolved flag (for display and undo).
    pub fn conflict_sections(&self) -> impl Iterator<Item = (Range<usize>, bool)> + '_ {
        self.conflicts.iter().map(|c| (c.rows.clone(), c.resolved))
    }

    /// The section `row` belongs to: its conflict section if any (resolved or not),
    /// otherwise its change run; `None` on an unchanged row outside every conflict.
    pub fn section_at(&self, row: usize) -> Option<Range<usize>> {
        self.conflicts
            .iter()
            .map(|c| &c.rows)
            .chain(&self.runs)
            .find(|rows| rows.contains(&row))
            .cloned()
    }

    /// `true` when `row` lies in an unresolved conflict section.
    pub fn in_conflict(&self, row: usize) -> bool {
        self.conflicts
            .iter()
            .any(|c| !c.resolved && c.rows.contains(&row))
    }

    /// Side the output takes on `row`.
    pub fn source(&self, row: usize) -> Side {
        let row = &self.rows[row];
        row.resolved.unwrap_or(match row.choice {
            MergeChoice::Left | MergeChoice::Both => Side::Left,
            MergeChoice::Right => Side::Right,
            MergeChoice::Center | MergeChoice::Conflict => Side::Center,
        })
    }

    /// Take `rows` from `side` (left, right or center) and clear every conflict section
    /// they touch.
    pub fn take(&mut self, rows: Range<usize>, side: Side) {
        for row in &mut self.rows[rows.clone()] {
            row.resolved = Some(side);
        }
        self.set_resolved(&rows, true);
    }

    /// Resolve a conflict section by taking `side`.
    pub fn resolve(&mut self, conflict: &Conflict, side: Side) {
        self.take(conflict.rows.clone(), side);
    }

    /// Mark the conflict sections touching `rows` as reviewed without changing content.
    pub fn accept(&mut self, rows: &Range<usize>) {
        self.set_resolved(rows, true);
    }

    /// Set or clear the resolved flag of the conflict sections touching `rows`.
    pub fn set_resolved(&mut self, rows: &Range<usize>, resolved: bool) {
        for conflict in &mut self.conflicts {
            if conflict.rows.start < rows.end && rows.start < conflict.rows.end {
                conflict.resolved = resolved;
            }
        }
    }

    /// Lines `side` holds in `rows`, in order (gaps contribute nothing).
    pub fn side_lines(
        &self,
        map: &TextMap,
        rows: Range<usize>,
        side: Side,
    ) -> Vec<(Vec<u8>, LineEnding)> {
        rows.filter_map(|i| line_of(map, side, &map.row(i)?))
            .collect()
    }

    /// The output as lines. With `markers`, every unresolved conflict section is
    /// replaced by a marker block; without, it keeps its row-by-row content.
    pub fn output_lines(&self, map: &TextMap, markers: Option<&MergeLabels>) -> Vec<OutputLine> {
        let fallback = fallback_ending(map);
        let mut out = Vec::with_capacity(self.rows.len());
        let mut pending = self.conflicts.iter().filter(|c| !c.resolved).peekable();
        let mut i = 0;
        while i < self.rows.len() {
            while pending.peek().is_some_and(|c| c.rows.end <= i) {
                pending.next();
            }
            if let (Some(labels), Some(conflict)) =
                (markers, pending.peek().filter(|c| c.rows.start == i))
            {
                let marker = |text: String| OutputLine {
                    text: text.into_bytes(),
                    ending: fallback,
                    origin: Origin::Marker,
                    row: i,
                };
                let side = |side: Side, origin: Origin| {
                    conflict.rows.clone().filter_map(move |row| {
                        let (text, ending) = line_of(map, side, &map.row(row)?)?;
                        Some(OutputLine {
                            text,
                            ending,
                            origin,
                            row,
                        })
                    })
                };
                out.push(marker(format!("<<<<<<< {}", labels.left)));
                out.extend(side(Side::Left, Origin::Left));
                out.push(marker("=======".into()));
                out.extend(side(Side::Right, Origin::Right));
                out.push(marker(format!(">>>>>>> {}", labels.right)));
                i = conflict.rows.end;
                continue;
            }
            let source = self.source(i);
            if let Some((text, ending)) = map.row(i).and_then(|row| line_of(map, source, &row)) {
                out.push(OutputLine {
                    text,
                    ending,
                    origin: match source {
                        Side::Left => Origin::Left,
                        Side::Right => Origin::Right,
                        _ => Origin::Center,
                    },
                    row: i,
                });
            }
            i += 1;
        }
        // Only the last line of a file may be unterminated.
        let last = out.len().saturating_sub(1);
        for line in &mut out[..last] {
            if line.ending == LineEnding::None {
                line.ending = fallback;
            }
        }
        out
    }

    /// Encoded output bytes. Unresolved conflicts become marker blocks when
    /// `force_conflict_markers` is set; otherwise they keep their current content.
    pub fn render_output(&self, map: &TextMap, labels: &MergeLabels) -> Result<Vec<u8>, CoreError> {
        let markers = self.rules.force_conflict_markers.then_some(labels);
        let mut text = Vec::new();
        for line in self.output_lines(map, markers) {
            text.extend_from_slice(&line.text);
            text.extend_from_slice(line.ending.as_bytes());
        }
        let ((encoding, bom), _) = output_encoding(map);
        let text = std::str::from_utf8(&text).map_err(|e| CoreError::Invalid(e.to_string()))?;
        encoding::encode(text, encoding, BomPolicy::Preserve, bom)
    }

    fn warnings(&self, map: &TextMap, labels: &MergeLabels) -> Vec<String> {
        let mut warnings = Vec::new();
        if output_encoding(map).1 {
            warnings.push(
                "left and right use different encodings; the output keeps the center encoding"
                    .into(),
            );
        }
        let markers = self.rules.force_conflict_markers.then_some(labels);
        let mut endings = self
            .output_lines(map, markers)
            .into_iter()
            .map(|line| line.ending)
            .filter(|ending| *ending != LineEnding::None);
        if let Some(first) = endings.next() {
            if endings.any(|ending| ending != first) {
                warnings.push("the output has mixed line endings".into());
            }
        }
        warnings
    }
}

/// Non-interactive merge as performed by `/automerge`.
pub fn auto_merge(
    map: &TextMap,
    rules: &TextMergeRules,
    labels: &MergeLabels,
) -> Result<AutoMergeOutcome, CoreError> {
    let state = MergeState::classify(map, rules)?;
    let conflicts = state.conflicts();
    let withheld = !conflicts.is_empty() && !rules.force_conflict_markers;
    let (output, warnings) = if withheld {
        (Vec::new(), Vec::new())
    } else {
        (
            state.render_output(map, labels)?,
            state.warnings(map, labels),
        )
    };
    Ok(AutoMergeOutcome {
        conflicts,
        output,
        withheld,
        warnings,
    })
}
