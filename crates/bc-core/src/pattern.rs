//! Shared regular-expression dialect and single-name mask primitives.
//! Backtracking constructs are rejected by the linear-time regex backend.

use regex::{Regex, RegexBuilder};

#[derive(Debug, Clone)]
pub struct Pattern {
    source: String,
    case_sensitive: bool,
    regex: Regex,
}

/// UTF-8 byte spans. `captures[0]` is the whole match; unmatched groups are None.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub span: std::ops::Range<usize>,
    pub captures: Vec<Option<std::ops::Range<usize>>>,
}

impl Pattern {
    /// Compile a pattern; directional word boundaries `\<` and `\>` are supported.
    pub fn new(source: &str, case_sensitive: bool) -> Result<Self, crate::CoreError> {
        let regex = RegexBuilder::new(&rewrite_dialect(source))
            .case_insensitive(!case_sensitive)
            .build()
            .map_err(|error| {
                crate::CoreError::Invalid(format!(
                    "pattern {source:?}: {error}; backreferences and look-around are not supported"
                ))
            })?;
        Ok(Self {
            source: source.into(),
            case_sensitive,
            regex,
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn case_sensitive(&self) -> bool {
        self.case_sensitive
    }

    /// Search at/after a UTF-8 boundary. Invalid/out-of-range offsets return None.
    /// Anchors and word boundaries retain the context before `start`.
    pub fn find_at(&self, haystack: &str, start: usize) -> Option<Match> {
        if !haystack.is_char_boundary(start) {
            return None;
        }
        let captures = self.regex.captures_at(haystack, start)?;
        Some(Match {
            span: captures.get(0)?.range(),
            captures: captures.iter().map(|m| m.map(|m| m.range())).collect(),
        })
    }

    pub fn match_at(&self, haystack: &str, start: usize) -> Option<Match> {
        self.find_at(haystack, start)
            .filter(|m| m.span.start == start)
    }

    /// Replace using regex `$1` / `${name}` capture expansion (and `$$` for `$`).
    pub fn replace_all(&self, haystack: &str, replacement: &str) -> String {
        self.regex.replace_all(haystack, replacement).into_owned()
    }
}

fn rewrite_dialect(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let mut chars = source.chars();
    let mut class_depth = 0usize;
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('<') if class_depth == 0 => result.push_str(r"\b{start}"),
                Some('>') if class_depth == 0 => result.push_str(r"\b{end}"),
                Some(c @ ('<' | '>')) => result.push(c),
                Some(next) => {
                    result.push('\\');
                    result.push(next);
                }
                None => result.push('\\'),
            }
        } else {
            if c == '[' {
                class_depth += 1;
            }
            if c == ']' {
                class_depth = class_depth.saturating_sub(1);
            }
            result.push(c);
        }
    }
    result
}

/// Parsed mask metadata, without deciding include/exclude precedence or recursion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskSpec {
    /// No leading exclusion marker or trailing directory separator.
    pub pattern: String,
    pub exclude: bool,
    pub folder: bool,
}

/// Split semicolon-separated masks, trimming surrounding whitespace/empty entries.
/// Combination evaluation belongs to bc-engine-dir's NameFilter.
pub fn split_mask_list(mask: &str) -> Vec<MaskSpec> {
    mask.split(';')
        .filter_map(|part| {
            let part = part.trim();
            let (exclude, part) = part
                .strip_prefix('-')
                .map_or((false, part), |p| (true, p.trim()));
            let folder = part.ends_with(['\\', '/']);
            let pattern = part.trim_end_matches(['\\', '/']);
            if pattern.is_empty() {
                return None;
            }
            Some(MaskSpec {
                pattern: pattern.into(),
                exclude,
                folder,
            })
        })
        .collect()
}

/// Compile one positive mask, anchored to a complete name. `*`/`?` never cross a
/// separator; a trailing slash/backslash requires a directory marker in the name.
/// Lists and exclusion markers must first be handled with [`split_mask_list`].
pub fn glob_to_pattern(mask: &str, case_sensitive: bool) -> Result<Pattern, crate::CoreError> {
    let mask = mask.trim();
    if mask.is_empty() || mask.contains(';') || mask.starts_with('-') {
        return Err(crate::CoreError::Invalid(
            "expected one positive mask; use split_mask_list and NameFilter for lists/exclusions"
                .into(),
        ));
    }
    let mut source = String::from(r"\A(?:");
    for c in mask.chars() {
        match c {
            '*' => source.push_str(r"[^\\/]*"),
            '?' => source.push_str(r"[^\\/]"),
            '\\' | '/' => source.push_str(r"[\\/]"),
            _ => source.push_str(&regex::escape(&c.to_string())),
        }
    }
    source.push_str(r")\z");
    Pattern::new(&source, case_sensitive)
}
