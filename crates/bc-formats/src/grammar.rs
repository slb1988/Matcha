//! Compiled, ordered, line-state tokenizer. Unclassified gaps remain unimportant-
//! policy-neutral; tokens contain only grammar-classified UTF-8 byte ranges.
use crate::model::{ElementKind, FileFormat, GrammarMatch};
use bc_core::{pattern::Pattern, CoreError};
use std::{borrow::Cow, collections::HashMap, ops::Range};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub span: Range<usize>,
    pub element: ElementKind,
    pub grammar_index: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LineState {
    pub open_item: Option<usize>,
}

#[derive(Debug)]
enum Compiled {
    Basic(Pattern),
    Delimited {
        start: Pattern,
        stop: Option<Pattern>,
        stop_eol: bool,
        escape: Option<char>,
    },
    List {
        patterns: Vec<Pattern>,
        boundaries: Vec<(bool, bool)>,
        buckets: HashMap<char, Vec<usize>>,
        fallback: Vec<usize>,
        case_sensitive: bool,
    },
    Columns {
        start: usize,
        stop: usize,
    },
}

#[derive(Debug)]
pub struct Tokenizer<'f> {
    format: Cow<'f, FileFormat>,
    compiled: Vec<Compiled>,
    weights: Vec<(Pattern, bc_core::align::LineWeight)>,
}

fn compile(text: &str, regex: bool, case: bool) -> Result<Pattern, CoreError> {
    if regex {
        Pattern::new(text, case)
    } else {
        Pattern::literal(text, case)
    }
}

fn word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

impl<'f> Tokenizer<'f> {
    pub fn new(format: &'f FileFormat) -> Result<Self, CoreError> {
        let mut compiled = Vec::with_capacity(format.grammar.len());
        for (index, item) in format.grammar.iter().enumerate() {
            let build = || -> Result<Compiled, CoreError> {
                Ok(match &item.matcher {
                    GrammarMatch::Basic { text, is_regexp } => {
                        Compiled::Basic(compile(text, *is_regexp, item.case_sensitive)?)
                    }
                    GrammarMatch::Delimited {
                        start,
                        stop,
                        is_regexp,
                        stop_eol,
                        escape,
                    } => Compiled::Delimited {
                        start: compile(start, *is_regexp, item.case_sensitive)?,
                        stop: stop
                            .as_ref()
                            .map(|s| compile(s, *is_regexp, item.case_sensitive))
                            .transpose()?,
                        stop_eol: *stop_eol,
                        escape: *escape,
                    },
                    GrammarMatch::List { items, is_regexp } => {
                        let mut patterns = Vec::new();
                        let mut boundaries = Vec::new();
                        let mut buckets: HashMap<char, Vec<usize>> = HashMap::new();
                        let mut fallback = Vec::new();
                        for (i, text) in items.iter().enumerate() {
                            patterns.push(compile(text, *is_regexp, item.case_sensitive)?);
                            boundaries.push((
                                !*is_regexp && text.chars().next().is_some_and(word),
                                !*is_regexp && text.chars().next_back().is_some_and(word),
                            ));
                            if !*is_regexp && text.starts_with(|c: char| c.is_ascii()) {
                                let c = text.chars().next().unwrap();
                                let key = if item.case_sensitive {
                                    c
                                } else {
                                    c.to_ascii_lowercase()
                                };
                                buckets.entry(key).or_default().push(i);
                            } else {
                                fallback.push(i);
                            }
                        }
                        Compiled::List {
                            patterns,
                            boundaries,
                            buckets,
                            fallback,
                            case_sensitive: item.case_sensitive,
                        }
                    }
                    GrammarMatch::Columns {
                        start_col,
                        stop_col,
                    } => {
                        if *start_col == 0 || *stop_col < *start_col {
                            return Err(CoreError::Invalid("invalid column range".into()));
                        }
                        Compiled::Columns {
                            start: *start_col as usize - 1,
                            stop: *stop_col as usize,
                        }
                    }
                })
            };
            compiled.push(build().map_err(|e| {
                CoreError::Invalid(format!("format {} grammar {index}: {e}", format.name))
            })?);
        }
        let weights = format
            .weights
            .iter()
            .map(|weight| {
                compile(&weight.find_text, weight.is_regexp, true)
                    .map(|pattern| (pattern, bc_core::align::LineWeight(weight.priority.0)))
                    .map_err(|e| CoreError::Invalid(format!("format {} weight: {e}", format.name)))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self {
            format: Cow::Borrowed(format),
            compiled,
            weights,
        })
    }

    pub fn format(&self) -> &FileFormat {
        &self.format
    }

    pub fn into_owned(self) -> Tokenizer<'static> {
        Tokenizer {
            format: Cow::Owned(self.format.into_owned()),
            compiled: self.compiled,
            weights: self.weights,
        }
    }

    /// Highest matching format priority, ready for AlignInput::weights. Weight
    /// regexes use their own inline case flags; grammar case settings stay separate.
    pub fn line_weight(&self, line: &str) -> bc_core::align::LineWeight {
        self.weights
            .iter()
            .filter(|(pattern, _)| pattern.find_at(line, 0).is_some())
            .map(|(_, weight)| *weight)
            .max()
            .unwrap_or_default()
    }

    pub fn tokenize_line(&self, line: &str, state: &mut LineState) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut pos = 0;
        if let Some(index) = state.open_item.take() {
            if let Some(Compiled::Delimited {
                stop,
                stop_eol,
                escape,
                ..
            }) = self.compiled.get(index)
            {
                let (end, closed) = delimited_end(line, 0, stop.as_ref(), *escape);
                self.push(&mut tokens, index, 0..end);
                pos = end;
                if !closed && !stop_eol {
                    state.open_item = Some(index);
                }
            }
        }
        while pos < line.len() {
            let mut selected = None;
            for index in 0..self.compiled.len() {
                if let Some(end) = self.start_at(index, line, pos) {
                    if end > pos && selected.map_or(true, |(_, previous)| end > previous) {
                        selected = Some((index, end));
                    }
                }
            }
            let Some((index, mut end)) = selected else {
                pos += line[pos..].chars().next().unwrap().len_utf8();
                continue;
            };
            if let Compiled::Delimited {
                stop,
                stop_eol,
                escape,
                ..
            } = &self.compiled[index]
            {
                let (last, closed) = delimited_end(line, end, stop.as_ref(), *escape);
                end = last;
                if !closed && !stop_eol {
                    state.open_item = Some(index);
                }
            }
            self.push(&mut tokens, index, pos..end);
            pos = end;
        }
        // Fixed columns have priority at every covered position, including inside
        // a lower-priority long token that began before the column boundary.
        for (index, matcher) in self.compiled.iter().enumerate() {
            if let Compiled::Columns { start, stop } = matcher {
                let byte = |column| {
                    line.char_indices()
                        .nth(column)
                        .map_or(line.len(), |(i, _)| i)
                };
                let span = byte(*start)..byte(*stop);
                if !span.is_empty() {
                    tokens = overlay_column(
                        tokens,
                        Token {
                            span,
                            element: self.format.grammar[index].element,
                            grammar_index: index,
                        },
                    );
                }
            }
        }
        tokens
    }

    fn push(&self, tokens: &mut Vec<Token>, index: usize, span: Range<usize>) {
        if !span.is_empty() {
            tokens.push(Token {
                span,
                element: self.format.grammar[index].element,
                grammar_index: index,
            });
        }
    }

    fn start_at(&self, index: usize, line: &str, pos: usize) -> Option<usize> {
        match &self.compiled[index] {
            Compiled::Basic(pattern) | Compiled::Delimited { start: pattern, .. } => {
                pattern.match_at(line, pos).map(|m| m.span.end)
            }
            Compiled::Columns { .. } => None,
            Compiled::List {
                patterns,
                boundaries,
                buckets,
                fallback,
                case_sensitive,
            } => {
                let first = line[pos..].chars().next()?;
                let key = if *case_sensitive {
                    first
                } else {
                    first.to_ascii_lowercase()
                };
                let check = |i: usize| -> Option<usize> {
                    let found = patterns[i].match_at(line, pos)?;
                    let (before, after) = boundaries[i];
                    if before && line[..pos].chars().next_back().is_some_and(word) {
                        return None;
                    }
                    if after && line[found.span.end..].chars().next().is_some_and(word) {
                        return None;
                    }
                    (found.span.end > pos).then_some(found.span.end)
                };
                if first.is_ascii() {
                    buckets
                        .get(&key)
                        .into_iter()
                        .flatten()
                        .chain(fallback)
                        .filter_map(|&i| check(i))
                        .max()
                } else {
                    // Regex Unicode folding includes e.g. Kelvin sign/long-s: don't
                    // assume lowercase bucketing alone covers all non-ASCII folds.
                    (0..patterns.len()).filter_map(check).max()
                }
            }
        }
    }
}

fn delimited_end(
    line: &str,
    mut pos: usize,
    stop: Option<&Pattern>,
    escape: Option<char>,
) -> (usize, bool) {
    while pos <= line.len() {
        let matched = stop.and_then(|p| p.match_at(line, pos));
        let current = line[pos..].chars().next();
        if let Some(c) = current.filter(|&c| Some(c) == escape) {
            let width = c.len_utf8();
            let next = line[pos + width..].chars().next();
            // When escape equals the terminator (e.g. doubled quotes), only a
            // doubled terminator escapes; a single quote must still close.
            if let Some(ref found) = matched {
                if found.span.end == pos + width && next != escape {
                    return (found.span.end, true);
                }
            }
            pos += width + next.map_or(0, char::len_utf8);
            continue;
        }
        if let Some(found) = matched {
            return (found.span.end, true);
        }
        if let Some(c) = current {
            pos += c.len_utf8();
        } else {
            break;
        }
    }
    (line.len(), false)
}

fn overlay_column(tokens: Vec<Token>, column: Token) -> Vec<Token> {
    let mut result = Vec::with_capacity(tokens.len() + 2);
    let mut cursor = column.span.start;
    for token in tokens {
        if token.span.end <= column.span.start || token.span.start >= column.span.end {
            result.push(token);
        } else if token.grammar_index < column.grammar_index {
            if cursor < token.span.start {
                result.push(Token {
                    span: cursor..token.span.start,
                    ..column.clone()
                });
            }
            cursor = cursor.max(token.span.end.min(column.span.end));
            result.push(token);
        } else {
            if token.span.start < column.span.start {
                result.push(Token {
                    span: token.span.start..column.span.start,
                    ..token.clone()
                });
            }
            if token.span.end > column.span.end {
                result.push(Token {
                    span: column.span.end..token.span.end,
                    ..token
                });
            }
        }
    }
    if cursor < column.span.end {
        result.push(Token {
            span: cursor..column.span.end,
            ..column
        });
    }
    result.sort_by_key(|token| token.span.start);
    result
}
