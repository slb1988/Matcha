//! Line-oriented Unicode-safe search and grouped replacements; no implicit wrap.
use crate::{importance::whole_word, TextContent};
use bc_core::{
    line::{LineEdit, LineEnding, LineList},
    pattern::Pattern,
    CoreError, Side,
};
use std::ops::Range;
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FindOptions {
    pub pattern: String,
    pub is_regexp: bool,
    pub case_sensitive: bool,
    pub whole_words: bool,
    pub backwards: bool,
    pub differences_only: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindHit {
    pub side: Side,
    pub line: usize,
    pub range: Range<usize>,
}
fn compile(options: &FindOptions) -> Result<Pattern, CoreError> {
    if options.is_regexp {
        Pattern::new(&options.pattern, options.case_sensitive)
    } else {
        Pattern::literal(&options.pattern, options.case_sensitive)
    }
}
fn ranges(
    count: usize,
    options: &FindOptions,
    allowed: Option<&[Range<usize>]>,
) -> Result<Vec<Range<usize>>, CoreError> {
    if !options.differences_only {
        return Ok(std::iter::once(0..count).collect());
    }
    let mut ranges = allowed
        .ok_or_else(|| {
            CoreError::Invalid("differences-only search requires TextMap line ranges".into())
        })?
        .to_vec();
    if ranges.iter().any(|r| r.start > r.end || r.end > count) {
        return Err(CoreError::Invalid("invalid search line range".into()));
    }
    ranges.sort_by_key(|r| r.start);
    let mut merged: Vec<Range<usize>> = Vec::new();
    for range in ranges {
        if let Some(last) = merged.last_mut().filter(|r| r.end >= range.start) {
            last.end = last.end.max(range.end);
        } else {
            merged.push(range);
        }
    }
    Ok(merged)
}
/// Forward search includes the caret; backward search ends at it. Columns are UTF-8
/// byte offsets. Pass a prior nonempty hit's end for forward Find Next.
pub fn find_next(
    content: &TextContent,
    side: Side,
    from: (usize, usize),
    options: &FindOptions,
    allowed: Option<&[Range<usize>]>,
) -> Result<Option<FindHit>, CoreError> {
    let pattern = compile(options)?;
    let lines = content.lines.borrow();
    let mut ranges = ranges(lines.len(), options, allowed)?;
    if options.backwards {
        ranges.reverse();
    }
    for range in ranges {
        let indices: Box<dyn Iterator<Item = usize>> = if options.backwards {
            Box::new((range.start..range.end.min(from.0.saturating_add(1))).rev())
        } else {
            Box::new(range.start.max(from.0)..range.end)
        };
        for line in indices {
            let text = std::str::from_utf8(lines.line_bytes(line).unwrap())
                .map_err(|e| CoreError::Invalid(e.to_string()))?;
            let col = if line == from.0 {
                from.1.min(text.len())
            } else if options.backwards {
                text.len()
            } else {
                0
            };
            if !text.is_char_boundary(col) {
                return Err(CoreError::Invalid("search column splits UTF-8".into()));
            }
            let mut cursor = if options.backwards { 0 } else { col };
            let mut found = None;
            while let Some(m) = pattern.find_at(text, cursor) {
                if options.backwards && m.span.end > col {
                    break;
                }
                if !options.whole_words || whole_word(text, m.span.clone()) {
                    let hit = FindHit {
                        side,
                        line,
                        range: m.span.clone(),
                    };
                    if !options.backwards {
                        return Ok(Some(hit));
                    }
                    found = Some(hit);
                }
                if m.span.end == text.len() {
                    break;
                }
                cursor = if m.span.end > m.span.start {
                    m.span.end
                } else {
                    m.span.end + text[m.span.end..].chars().next().unwrap().len_utf8()
                };
            }
            if found.is_some() {
                return Ok(found);
            }
        }
    }
    Ok(None)
}
/// Every replacement is planned before mutation, then applied in reverse line
/// order as one undo group. New internal line endings use the dominant style.
pub fn replace_all(
    content: &mut TextContent,
    options: &FindOptions,
    replacement: &str,
    allowed: Option<&[Range<usize>]>,
) -> Result<usize, CoreError> {
    content.ensure_writable()?;
    let pattern = compile(options)?;
    let snapshot = content.lines.borrow().clone();
    let ranges = ranges(snapshot.len(), options, allowed)?;
    let replacement = if options.is_regexp {
        replacement.to_string()
    } else {
        replacement.replace('$', "$$")
    };
    let mut edits = Vec::new();
    let mut count = 0;
    for range in ranges {
        for index in range {
            let text = std::str::from_utf8(snapshot.line_bytes(index).unwrap())
                .map_err(|e| CoreError::Invalid(e.to_string()))?;
            let mut hits = 0;
            let changed = pattern.replace_all_if(text, &replacement, |span| {
                if !options.whole_words || whole_word(text, span) {
                    hits += 1;
                    true
                } else {
                    false
                }
            });
            if hits == 0 {
                continue;
            }
            count += hits;
            let ending = snapshot.ending(index).unwrap();
            let terminated = changed.ends_with(['\r', '\n']);
            let mut bytes = changed.into_bytes();
            bytes.extend_from_slice(ending.as_bytes());
            let mut lines = LineList::from_bytes(bytes);
            let dominant = snapshot.dominant_ending();
            lines.set_all_endings(dominant);
            if !lines.is_empty() {
                lines.set_ending(
                    lines.len() - 1,
                    if ending != LineEnding::None {
                        ending
                    } else if terminated {
                        dominant
                    } else {
                        LineEnding::None
                    },
                );
            }
            edits.push((index, lines));
        }
    }
    if !edits.is_empty() {
        content.edits.begin_group("Replace all");
        for (index, replacement) in edits.into_iter().rev() {
            content.edits.push(Box::new(LineEdit::replace_range(
                &content.lines,
                index..index + 1,
                &replacement,
            )));
        }
        content.edits.end_group();
    }
    Ok(count)
}
