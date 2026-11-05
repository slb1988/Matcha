//! Versioned plain-text conflict packets. Source payload is data, never instructions.
use crate::{importance::Importance, TextMap};
use bc_core::{line::LineEnding, CoreError, DiffKind};
use std::{fmt::Write, ops::Range};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictRow {
    pub row: usize,
    pub left: Option<usize>,
    pub right: Option<usize>,
    pub kind: DiffKind,
    pub importance: Importance,
    pub left_ending: Option<LineEnding>,
    pub right_ending: Option<LineEnding>,
    pub left_text: String,
    pub right_text: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictSection {
    pub id: usize,
    pub rows: Range<usize>,
    pub context: Range<usize>,
    pub importance: Importance,
    pub lines: Vec<ConflictRow>,
}
fn importance(i: Importance) -> &'static str {
    match i {
        Importance::Same => "S",
        Importance::Unimportant => "U",
        Importance::Important => "I",
    }
}
fn kind(k: DiffKind) -> &'static str {
    match k {
        DiffKind::Same => "S",
        DiffKind::Changed => "C",
        DiffKind::LeftOnly => "L",
        DiffKind::RightOnly => "R",
    }
}
fn ending(e: Option<LineEnding>) -> &'static str {
    match e {
        Some(LineEnding::CrLf) => "C",
        Some(LineEnding::Lf) => "L",
        Some(LineEnding::Cr) => "R",
        Some(LineEnding::None) => "N",
        None => "-",
    }
}
fn number(n: Option<usize>) -> String {
    n.map_or_else(|| "-".into(), |n| (n + 1).to_string())
}
fn escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => {
                write!(out, "\\u{{{:x}}}", c as u32).unwrap();
            }
            _ => out.push(c),
        }
    }
    out
}
fn bad() -> CoreError {
    CoreError::Invalid("malformed Matcha conflict packet".into())
}
fn unescape(text: &str) -> Result<String, CoreError> {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next().ok_or_else(bad)? {
            '\\' => out.push('\\'),
            't' => out.push('\t'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            'u' => {
                if chars.next() != Some('{') {
                    return Err(bad());
                }
                let mut hex = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(c) if c.is_ascii_hexdigit() && hex.len() < 6 => hex.push(c),
                        _ => return Err(bad()),
                    }
                }
                out.push(
                    char::from_u32(u32::from_str_radix(&hex, 16).map_err(|_| bad())?)
                        .ok_or_else(bad)?,
                );
            }
            _ => return Err(bad()),
        }
    }
    Ok(out)
}
impl TextMap {
    /// Selected difference sections plus N surrounding display rows, with original
    /// line numbers and classifications (not the ignore-unimportant projection).
    pub fn to_conflict_text(
        &self,
        sections: &[usize],
        context: usize,
    ) -> Result<String, CoreError> {
        if self.is_stale() {
            return Err(CoreError::Invalid(
                "recompare before exporting stale text".into(),
            ));
        }
        let all = self.sections();
        let mut out = String::from("MATCHA-CONFLICT\t1\n");
        let mut seen = std::collections::HashSet::new();
        let left = self.left.lines.borrow();
        let right = self.right.lines.borrow();
        for &id in sections {
            if !seen.insert(id) {
                return Err(bad());
            }
            let section = all.get(id).ok_or_else(bad)?;
            let range = section.rows.start.saturating_sub(context)
                ..section
                    .rows
                    .end
                    .saturating_add(context)
                    .min(self.row_count());
            writeln!(
                out,
                "SECTION\t{id}\t{}\t{}\t{}\t{}\t{}",
                section.rows.start,
                section.rows.end,
                range.start,
                range.end,
                importance(section.importance)
            )
            .unwrap();
            for index in range {
                let row = self.row(index).unwrap();
                let lt = row
                    .left
                    .and_then(|i| left.line_bytes(i))
                    .unwrap_or_default();
                let rt = row
                    .right
                    .and_then(|i| right.line_bytes(i))
                    .unwrap_or_default();
                writeln!(
                    out,
                    "ROW\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    index + 1,
                    number(row.left),
                    number(row.right),
                    kind(row.kind),
                    importance(row.importance),
                    ending(row.left.and_then(|i| left.ending(i))),
                    ending(row.right.and_then(|i| right.ending(i))),
                    escape(std::str::from_utf8(lt).map_err(|_| bad())?),
                    escape(std::str::from_utf8(rt).map_err(|_| bad())?)
                )
                .unwrap();
            }
            out.push_str("ENDSECTION\n");
        }
        Ok(out)
    }
}
pub fn parse_conflict_text(text: &str) -> Result<Vec<ConflictSection>, CoreError> {
    let mut lines = text.lines();
    if lines.next() != Some("MATCHA-CONFLICT\t1") {
        return Err(bad());
    }
    let parse = |s: &str| s.parse::<usize>().map_err(|_| bad());
    let optional = |s: &str| -> Result<Option<usize>, CoreError> {
        if s == "-" {
            Ok(None)
        } else {
            Ok(Some(parse(s)?.checked_sub(1).ok_or_else(bad)?))
        }
    };
    let imp = |s: &str| match s {
        "S" => Ok(Importance::Same),
        "U" => Ok(Importance::Unimportant),
        "I" => Ok(Importance::Important),
        _ => Err(bad()),
    };
    let eol = |s: &str| match s {
        "-" => Ok(None),
        "C" => Ok(Some(LineEnding::CrLf)),
        "L" => Ok(Some(LineEnding::Lf)),
        "R" => Ok(Some(LineEnding::Cr)),
        "N" => Ok(Some(LineEnding::None)),
        _ => Err(bad()),
    };
    let mut result = Vec::new();
    let mut seen = std::collections::HashSet::new();
    while let Some(header) = lines.next() {
        let h: Vec<_> = header.split('\t').collect();
        if h.len() != 7 || h[0] != "SECTION" {
            return Err(bad());
        }
        let mut section = ConflictSection {
            id: parse(h[1])?,
            rows: parse(h[2])?..parse(h[3])?,
            context: parse(h[4])?..parse(h[5])?,
            importance: imp(h[6])?,
            lines: Vec::new(),
        };
        if !seen.insert(section.id)
            || section.rows.is_empty()
            || section.context.start > section.rows.start
            || section.context.end < section.rows.end
        {
            return Err(bad());
        }
        loop {
            let line = lines.next().ok_or_else(bad)?;
            if line == "ENDSECTION" {
                break;
            }
            let f: Vec<_> = line.split('\t').collect();
            if f.len() != 10 || f[0] != "ROW" {
                return Err(bad());
            }
            let row = parse(f[1])?.checked_sub(1).ok_or_else(bad)?;
            if row != section.context.start + section.lines.len() || row >= section.context.end {
                return Err(bad());
            }
            let parsed = ConflictRow {
                row,
                left: optional(f[2])?,
                right: optional(f[3])?,
                kind: match f[4] {
                    "S" => DiffKind::Same,
                    "C" => DiffKind::Changed,
                    "L" => DiffKind::LeftOnly,
                    "R" => DiffKind::RightOnly,
                    _ => return Err(bad()),
                },
                importance: imp(f[5])?,
                left_ending: eol(f[6])?,
                right_ending: eol(f[7])?,
                left_text: unescape(f[8])?,
                right_text: unescape(f[9])?,
            };
            if parsed.left.is_some() != parsed.left_ending.is_some()
                || parsed.right.is_some() != parsed.right_ending.is_some()
            {
                return Err(bad());
            }
            section.lines.push(parsed);
        }
        if section.lines.len() != section.context.len() {
            return Err(bad());
        }
        result.push(section);
    }
    Ok(result)
}
