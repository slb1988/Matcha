//! Token/format semantics, compiled equivalence rules, and importance propagation.
use crate::rules::ComparisonRules;
use bc_core::{line::LineEnding, pattern::Pattern, CoreError, Side};
use bc_formats::{grammar::Token, text_rules::ElementImportance, ElementKind, FileFormat};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[repr(u8)]
pub enum Importance {
    #[default]
    Same,
    Unimportant,
    Important,
}
impl Importance {
    pub fn with_ignore_unimportant(self, ignore: bool) -> Self {
        if ignore && self == Self::Unimportant {
            Self::Same
        } else {
            self
        }
    }
}

#[derive(Clone, Copy)]
pub struct LineView<'a> {
    pub text: &'a str,
    pub tokens: &'a [Token],
    pub format: Option<&'a FileFormat>,
    pub ending: LineEnding,
}
impl<'a> LineView<'a> {
    pub fn plain(text: &'a str) -> Self {
        Self {
            text,
            tokens: &[],
            format: None,
            ending: LineEnding::None,
        }
    }
}

#[derive(Debug, Clone)]
struct ReplacementRule {
    pattern: Pattern,
    to: String,
    whole: bool,
    bidirectional: bool,
}
#[derive(Debug, Clone)]
pub struct CompiledComparison {
    pub rules: ComparisonRules,
    ignored: Vec<(Pattern, Option<ElementKind>)>,
    replacements: Vec<ReplacementRule>,
}
pub struct PreparedLine {
    pub exact: String,
    pub significant: String,
    /// Original-byte ranges removed by element/whitespace/text masks.
    pub ignored: Vec<Range<usize>>,
}

pub fn whole_word(text: &str, span: Range<usize>) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    !text[..span.start].chars().next_back().is_some_and(word)
        && !text[span.end..].chars().next().is_some_and(word)
}
fn pattern(source: &str, regex: bool, case: bool) -> Result<Pattern, CoreError> {
    if regex {
        Pattern::new(source, case)
    } else {
        Pattern::literal(source, case)
    }
}
impl CompiledComparison {
    pub fn new(rules: &ComparisonRules) -> Result<Self, CoreError> {
        let ignored = rules
            .unimportant_text
            .iter()
            .map(|r| pattern(&r.pattern, r.is_regexp, r.case_sensitive).map(|p| (p, r.only_in)))
            .collect::<Result<_, _>>()?;
        let replacements = rules
            .replacements
            .iter()
            .map(|r| {
                pattern(&r.from, r.is_regexp, r.case_sensitive).map(|pattern| ReplacementRule {
                    pattern,
                    to: if r.is_regexp {
                        r.to.clone()
                    } else {
                        r.to.replace('$', "$$")
                    },
                    whole: r.whole_words,
                    bidirectional: r.bidirectional,
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            rules: rules.clone(),
            ignored,
            replacements,
        })
    }
    fn replace(&self, mut text: String, side: Side) -> String {
        for rule in &self.replacements {
            if side == Side::Left || rule.bidirectional {
                text = rule.pattern.replace_all_if(&text, &rule.to, |span| {
                    !rule.whole || whole_word(&text, span)
                });
            }
        }
        text
    }
    pub fn prepare(&self, view: LineView<'_>, side: Side) -> PreparedLine {
        let mut ignored = Vec::new();
        for (pattern, only_in) in &self.ignored {
            let mut cursor = 0;
            while let Some(m) = pattern.find_at(view.text, cursor) {
                if only_in.is_none()
                    || view.tokens.iter().any(|t| {
                        Some(t.element) == *only_in
                            && t.span.start <= m.span.start
                            && t.span.end >= m.span.end
                    })
                {
                    ignored.push(m.span.clone());
                }
                if m.span.end == view.text.len() {
                    break;
                }
                cursor = if m.span.end > m.span.start {
                    m.span.end
                } else {
                    m.span.end + view.text[m.span.end..].chars().next().unwrap().len_utf8()
                };
            }
        }
        ignored.sort_by_key(|r| r.start);
        let mut spans: Vec<Range<usize>> = Vec::new();
        for span in ignored {
            if let Some(last) = spans.last_mut().filter(|last| last.end >= span.start) {
                last.end = last.end.max(span.end);
            } else {
                spans.push(span);
            }
        }
        let mut ignored: Vec<Range<usize>> = Vec::new();
        let mut span_index = 0;
        let leading = view.text.len() - view.text.trim_start_matches(char::is_whitespace).len();
        let trailing = view.text.trim_end_matches(char::is_whitespace).len();
        let mut exact = String::new();
        let mut significant = String::new();
        let flags = &self.rules.importance;
        let mut token_index = 0;
        for (number, (i, c)) in view.text.char_indices().enumerate() {
            while token_index < view.tokens.len() && view.tokens[token_index].span.end <= i {
                token_index += 1;
            }
            while span_index < spans.len() && spans[span_index].end <= i {
                span_index += 1;
            }
            let token = view.tokens.get(token_index).filter(|t| t.span.contains(&i));
            let element_insensitive = token.is_some_and(|t| {
                view.format
                    .is_some_and(|f| !f.is_case_significant(t.element))
            });
            if element_insensitive {
                exact.extend(c.to_lowercase());
            } else {
                exact.push(c);
            }
            let masked = token.is_some_and(|t| flags.unimportant.contains(&t.element))
                || (c.is_whitespace()
                    && ((i < leading && flags.leading_whitespace_unimportant)
                        || (i >= trailing && flags.trailing_whitespace_unimportant)
                        || (i >= leading
                            && i < trailing
                            && flags.embedded_whitespace_unimportant)))
                || spans.get(span_index).is_some_and(|span| span.contains(&i))
                || (self.rules.compare_line_prefix != 0
                    && number >= self.rules.compare_line_prefix as usize);
            if masked {
                if let Some(last) = ignored.last_mut().filter(|last| last.end == i) {
                    last.end += c.len_utf8();
                } else {
                    ignored.push(i..i + c.len_utf8());
                }
            } else if element_insensitive {
                significant.extend(c.to_lowercase());
            } else {
                significant.push(c);
            }
        }
        let significant = self.replace(significant, side);
        PreparedLine {
            exact,
            significant: if flags.case_unimportant {
                significant.to_lowercase()
            } else {
                significant
            },
            ignored,
        }
    }
    pub fn fragment_case_importance(
        &self,
        left: LineView<'_>,
        ls: Range<usize>,
        right: LineView<'_>,
        rs: Range<usize>,
    ) -> Option<Importance> {
        let canonical = |view: LineView<'_>, span: Range<usize>| {
            let mut text = String::new();
            for (offset, c) in view.text[span.clone()].char_indices() {
                let index = span.start + offset;
                let insensitive = view
                    .tokens
                    .iter()
                    .find(|t| t.span.contains(&index))
                    .is_some_and(|t| {
                        view.format
                            .is_some_and(|f| !f.is_case_significant(t.element))
                    });
                if insensitive {
                    text.extend(c.to_lowercase());
                } else {
                    text.push(c);
                }
            }
            text
        };
        let l = canonical(left, ls);
        let r = canonical(right, rs);
        if l == r {
            Some(Importance::Same)
        } else if self.rules.importance.case_unimportant && l.to_lowercase() == r.to_lowercase() {
            Some(Importance::Unimportant)
        } else {
            None
        }
    }
    pub fn needs_semantic_hash(&self, format: Option<&FileFormat>) -> bool {
        !self.replacements.is_empty()
            || format.is_some_and(|f| {
                f.case_significance
                    .iter()
                    .any(|(_, significant)| !significant)
            })
    }
    /// Replacements are semantic preprocessing; element masks do not enter alignment hashes.
    pub fn alignment_text(&self, view: LineView<'_>, side: Side) -> String {
        self.replace(self.prepare(view, side).exact, side)
    }
    pub fn classify(&self, left: LineView<'_>, right: LineView<'_>) -> Importance {
        if left.text == right.text {
            return if left.ending == right.ending
                || (self.rules.importance.line_endings_unimportant
                    && left.ending != LineEnding::None
                    && right.ending != LineEnding::None)
            {
                Importance::Same
            } else if self.rules.importance.line_endings_unimportant {
                Importance::Unimportant
            } else {
                Importance::Important
            };
        }
        let l = self.prepare(left, Side::Left);
        let r = self.prepare(right, Side::Right);
        if l.exact == r.exact {
            if left.ending == right.ending
                || (self.rules.importance.line_endings_unimportant
                    && left.ending != LineEnding::None
                    && right.ending != LineEnding::None)
            {
                Importance::Same
            } else if self.rules.importance.line_endings_unimportant {
                Importance::Unimportant
            } else {
                Importance::Important
            }
        } else if l.significant == r.significant
            && (self.rules.importance.line_endings_unimportant || left.ending == right.ending)
        {
            Importance::Unimportant
        } else {
            Importance::Important
        }
    }
    pub fn classify_orphan(&self, line: LineView<'_>, side: Side) -> Importance {
        if self.prepare(line, side).significant.is_empty() {
            Importance::Unimportant
        } else {
            Importance::Important
        }
    }
    pub fn span_ignored(&self, line: LineView<'_>, side: Side, span: Range<usize>) -> bool {
        let prepared = self.prepare(line, side);
        line.text
            .char_indices()
            .filter(|(i, _)| span.contains(i))
            .all(|(i, _)| prepared.ignored.iter().any(|r| r.contains(&i)))
    }
}
pub fn classify_pair(
    left: &str,
    left_tokens: &[Token],
    right: &str,
    right_tokens: &[Token],
    rules: &ElementImportance,
) -> Importance {
    let rules = ComparisonRules {
        importance: rules.clone(),
        ..Default::default()
    };
    CompiledComparison::new(&rules)
        .expect("no regex rules")
        .classify(
            LineView {
                text: left,
                tokens: left_tokens,
                ..LineView::plain(left)
            },
            LineView {
                text: right,
                tokens: right_tokens,
                ..LineView::plain(right)
            },
        )
}
pub fn propagate_block(lines: &[Importance]) -> Importance {
    lines.iter().copied().max().unwrap_or_default()
}
