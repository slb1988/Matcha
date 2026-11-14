//! Row re-pointing of A1 references after a merge inserted or removed rows — what Excel
//! does to formulas when rows are inserted or deleted.
//!
//! A [`RowMap`] maps the rows of one input sheet to rows of the merged sheet. References
//! in formulas, defined names, chart series and range attributes (`ref`, `sqref`) are
//! rewritten through it: a single cell whose row is gone becomes `#REF!`; a range keeps
//! its surviving rows (start moves down, end moves up). Rows past the input's last row
//! move with the end of the sheet. Columns never change (columns merge by position).
//! String literals, function names, whole-column ranges, external (`[1]Sheet!A1`) and 3-D
//! (`Sheet1:Sheet3!A1`) references are left alone.

/// Input row (0-based) → merged row; `None` = the row is not in the output.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RowMap {
    pub rows: Vec<Option<usize>>,
    /// Rows in the merged sheet.
    pub output_len: usize,
}

impl RowMap {
    pub fn is_identity(&self) -> bool {
        self.output_len == self.rows.len()
            && self.rows.iter().enumerate().all(|(i, r)| *r == Some(i))
    }

    /// Merged row of input row `row`.
    pub fn row(&self, row: usize) -> Option<usize> {
        match self.rows.get(row) {
            Some(mapped) => *mapped,
            None => Some(row - self.rows.len() + self.output_len),
        }
    }

    /// Merged rows of the inclusive input range; `None` when no row survives.
    pub fn range(&self, first: usize, last: usize) -> Option<(usize, usize)> {
        let start = (first..=last).find_map(|r| self.row(r))?;
        let end = (first..=last).rev().find_map(|r| self.row(r))?;
        Some((start, end))
    }
}

const MAX_ROW: usize = 1_048_576;
const MAX_COLUMN: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ref<'a> {
    /// `$A$5`: column text with its `$`, row `$`, 0-based row.
    Cell {
        column: &'a str,
        row_abs: bool,
        row: usize,
    },
    /// `$5` (only as half of a row range).
    Row { abs: bool, row: usize },
}

fn parse_ref(token: &str) -> Option<Ref<'_>> {
    let bytes = token.as_bytes();
    let mut i = usize::from(bytes.first() == Some(&b'$'));
    let letters = bytes[i..]
        .iter()
        .take_while(|b| b.is_ascii_alphabetic())
        .count();
    if letters == 0 {
        let digits = &token[i..];
        let row: usize = digits.parse().ok().filter(|r| (1..=MAX_ROW).contains(r))?;
        if !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        return Some(Ref::Row {
            abs: i == 1,
            row: row - 1,
        });
    }
    if letters > 3 {
        return None;
    }
    let column_end = i + letters;
    let column_value = token[i..column_end].bytes().fold(0usize, |n, b| {
        n * 26 + usize::from(b.to_ascii_uppercase() - b'A' + 1)
    });
    if column_value > MAX_COLUMN {
        return None;
    }
    i = column_end;
    let row_abs = bytes.get(i) == Some(&b'$');
    i += usize::from(row_abs);
    let digits = &token[i..];
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let row: usize = digits.parse().ok().filter(|r| (1..=MAX_ROW).contains(r))?;
    Some(Ref::Cell {
        column: &token[..column_end],
        row_abs,
        row: row - 1,
    })
}

fn write_ref(r: Ref<'_>, row: usize, out: &mut String) {
    match r {
        Ref::Cell {
            column, row_abs, ..
        } => {
            out.push_str(column);
            if row_abs {
                out.push('$');
            }
            out.push_str(&(row + 1).to_string());
        }
        Ref::Row { abs, .. } => {
            if abs {
                out.push('$');
            }
            out.push_str(&(row + 1).to_string());
        }
    }
}

fn ref_row(r: Ref<'_>) -> usize {
    match r {
        Ref::Cell { row, .. } | Ref::Row { row, .. } => row,
    }
}

/// Rewrite one reference or range (`A5`, `A5:C9`, `5:9`) through `map`.
fn shift_area(first: &str, second: Option<&str>, map: &RowMap, out: &mut String) -> bool {
    let Some(a) = parse_ref(first) else {
        return false;
    };
    match second {
        None => {
            if matches!(a, Ref::Row { .. }) {
                return false;
            }
            match map.row(ref_row(a)) {
                Some(row) => write_ref(a, row, out),
                None => out.push_str("#REF!"),
            }
            true
        }
        Some(second) => {
            let Some(b) = parse_ref(second) else {
                return false;
            };
            if matches!(a, Ref::Row { .. }) != matches!(b, Ref::Row { .. }) {
                return false;
            }
            let (lo, hi) = (ref_row(a).min(ref_row(b)), ref_row(a).max(ref_row(b)));
            match map.range(lo, hi) {
                Some((start, end)) => {
                    let (sa, sb) = if ref_row(a) <= ref_row(b) {
                        (start, end)
                    } else {
                        (end, start)
                    };
                    write_ref(a, sa, out);
                    out.push(':');
                    write_ref(b, sb, out);
                }
                None => out.push_str("#REF!"),
            }
            true
        }
    }
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '.' | '$' | '\\' | '?')
}

/// Rewrite the references of a formula. `own` maps unqualified references; `sheet`
/// returns the map of a sheet named in a qualified reference (`None` = leave it).
pub fn shift_formula<'m>(
    formula: &str,
    own: Option<&'m RowMap>,
    sheet: &dyn Fn(&str) -> Option<&'m RowMap>,
) -> String {
    let chars: Vec<(usize, char)> = formula.char_indices().collect();
    let at = |i: usize| chars.get(i).map(|c| c.1);
    let pos = |i: usize| chars.get(i).map_or(formula.len(), |c| c.0);
    let mut out = String::with_capacity(formula.len() + 8);
    let mut i = 0;
    // Area after a sheet prefix or bare; returns the index after it.
    let area = |i: usize, map: Option<&RowMap>, out: &mut String| -> usize {
        let token_end = |mut j: usize| {
            while at(j).is_some_and(|c| c.is_ascii_alphanumeric() || c == '$') {
                j += 1;
            }
            j
        };
        let first_end = token_end(i);
        let first = &formula[pos(i)..pos(first_end)];
        let (second, end) = if at(first_end) == Some(':') {
            let second_end = token_end(first_end + 1);
            (
                Some(&formula[pos(first_end + 1)..pos(second_end)]),
                second_end,
            )
        } else {
            (None, first_end)
        };
        let shifted = match map {
            Some(map) if !at(end).is_some_and(|c| is_name_char(c) || c == '(') => {
                let mut text = String::new();
                shift_area(first, second, map, &mut text).then_some(text)
            }
            _ => None,
        };
        match shifted {
            Some(text) => out.push_str(&text),
            None => out.push_str(&formula[pos(i)..pos(end)]),
        }
        end
    };
    let mut external = false;
    while i < chars.len() {
        let c = chars[i].1;
        match c {
            '"' => {
                let start = i;
                i += 1;
                while i < chars.len() {
                    if chars[i].1 == '"' {
                        if at(i + 1) == Some('"') {
                            i += 2;
                            continue;
                        }
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                out.push_str(&formula[pos(start)..pos(i)]);
            }
            '[' => {
                let start = i;
                let mut depth = 0;
                while i < chars.len() {
                    match chars[i].1 {
                        '[' => depth += 1,
                        ']' => {
                            depth -= 1;
                            if depth == 0 {
                                i += 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                out.push_str(&formula[pos(start)..pos(i)]);
                external = true;
                continue;
            }
            '\'' => {
                let start = i;
                i += 1;
                let mut name = String::new();
                while i < chars.len() {
                    if chars[i].1 == '\'' {
                        if at(i + 1) == Some('\'') {
                            name.push('\'');
                            i += 2;
                            continue;
                        }
                        i += 1;
                        break;
                    }
                    name.push(chars[i].1);
                    i += 1;
                }
                out.push_str(&formula[pos(start)..pos(i)]);
                if at(i) == Some('!') {
                    out.push('!');
                    let map = if external || name.contains(':') || name.starts_with('[') {
                        None
                    } else {
                        sheet(&name)
                    };
                    i = area(i + 1, map, &mut out);
                }
            }
            c if is_name_char(c) => {
                let start = i;
                while at(i).is_some_and(is_name_char) {
                    i += 1;
                }
                if at(i) == Some('!') {
                    let name = &formula[pos(start)..pos(i)];
                    out.push_str(name);
                    out.push('!');
                    let map = if external { None } else { sheet(name) };
                    i = area(i + 1, map, &mut out);
                } else if at(i) == Some(':')
                    && at(i + 1).is_some_and(|c| c.is_ascii_alphanumeric() || c == '$')
                {
                    // Possibly a range; let `area` re-read it from the start.
                    i = area(start, if external { None } else { own }, &mut out);
                } else if at(i) == Some('(') || chars[start].1 == '.' {
                    out.push_str(&formula[pos(start)..pos(i)]);
                } else {
                    let token = &formula[pos(start)..pos(i)];
                    let mut text = String::new();
                    match own.filter(|_| !external) {
                        Some(map) if shift_area(token, None, map, &mut text) => out.push_str(&text),
                        _ => out.push_str(token),
                    }
                }
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
        external = false;
    }
    out
}

/// Rewrite a space-separated range list attribute (`sqref`, `ref`). Ranges whose rows
/// are all gone are dropped; `None` when none survives.
pub fn shift_sqref(sqref: &str, map: &RowMap) -> Option<String> {
    let mut parts = Vec::new();
    for range in sqref.split_whitespace() {
        let (first, second) = match range.split_once(':') {
            Some((a, b)) => (a, Some(b)),
            None => (range, None),
        };
        let mut text = String::new();
        if shift_area(first, second, map, &mut text) {
            if text != "#REF!" {
                parts.push(text);
            }
        } else {
            parts.push(range.to_string());
        }
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// `A1` name of a 0-based cell.
pub fn cell_name(row: usize, column: usize) -> String {
    let mut letters = Vec::new();
    let mut n = column + 1;
    while n > 0 {
        let rem = (n - 1) % 26;
        letters.push(b'A' + rem as u8);
        n = (n - 1) / 26;
    }
    letters.reverse();
    format!("{}{}", String::from_utf8(letters).unwrap(), row + 1)
}

/// 0-based (row, column) of an `A1` name (no `$`).
pub fn parse_cell_name(name: &str) -> Option<(usize, usize)> {
    match parse_ref(name)? {
        Ref::Cell { column, row, .. } => {
            let column = column.trim_start_matches('$').bytes().fold(0usize, |n, b| {
                n * 26 + usize::from(b.to_ascii_uppercase() - b'A' + 1)
            });
            Some((row, column - 1))
        }
        Ref::Row { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Row 2 (index 1) deleted, two rows inserted before input row 4 (index 3).
    fn map() -> RowMap {
        RowMap {
            rows: vec![Some(0), None, Some(1), Some(4), Some(5)],
            output_len: 6,
        }
    }

    fn shift(f: &str) -> String {
        let m = map();
        let other = RowMap {
            rows: vec![Some(1), Some(2)],
            output_len: 3,
        };
        shift_formula(f, Some(&m), &|name| {
            (name == "Other" || name == "My Sheet").then_some(&other)
        })
    }

    #[test]
    fn cells_and_ranges_follow_their_rows() {
        assert_eq!(shift("=A1+B3*$C$4"), "=A1+B2*$C$5");
        assert_eq!(shift("=A2"), "=#REF!");
        assert_eq!(shift("=SUM(A1:A5)"), "=SUM(A1:A6)");
        assert_eq!(shift("=SUM(A2:A3)"), "=SUM(A2:A2)");
        assert_eq!(shift("=SUM(B2:B2)"), "=SUM(#REF!)");
        assert_eq!(shift("=SUM(4:5)"), "=SUM(5:6)");
        assert_eq!(shift("=A10"), "=A11");
        assert_eq!(shift("=SUM(A:A)+LOG10(A4)"), "=SUM(A:A)+LOG10(A5)");
    }

    #[test]
    fn literals_names_and_other_sheets() {
        assert_eq!(shift("=\"A3\"&A3"), "=\"A3\"&A2");
        assert_eq!(
            shift("=Other!A1+'My Sheet'!B2:B2"),
            "=Other!A2+'My Sheet'!B3:B3"
        );
        assert_eq!(shift("=Unknown!A3+A3"), "=Unknown!A3+A2");
        assert_eq!(shift("=[1]Other!A1"), "=[1]Other!A1");
        assert_eq!(shift("=Table1[Col]+A3"), "=Table1[Col]+A2");
        assert_eq!(shift("=IF(A3>1,TRUE,1.5E3)"), "=IF(A2>1,TRUE,1.5E3)");
    }

    #[test]
    fn range_lists() {
        assert_eq!(shift_sqref("A1:B3 D2", &map()).as_deref(), Some("A1:B2"));
        assert_eq!(shift_sqref("D2", &map()), None);
        assert_eq!(cell_name(4, 27), "AB5");
        assert_eq!(parse_cell_name("AB5"), Some((4, 27)));
    }
}
