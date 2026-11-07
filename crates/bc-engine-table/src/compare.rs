use crate::{
    data::{CellData, CellInfo, TableData},
    rules::{ColumnHandling, ColumnType, Layers},
    Sheet,
};
use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(u8)]
pub enum Importance {
    #[default]
    Same = 0,
    Unimportant = 1,
    Important = 2,
}
impl Importance {
    pub fn projected(self, ignore: bool) -> Self {
        if ignore && self == Self::Unimportant {
            Self::Same
        } else {
            self
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CellDiff {
    pub value: Importance,
    pub formula: Importance,
    pub format: Importance,
}
impl CellDiff {
    pub fn importance(self, layers: Layers, ignore: bool) -> Importance {
        let raw = match layers {
            Layers::Values => self.value,
            Layers::ValuesAndFormulas => self.value.max(self.formula),
            Layers::All => self.value.max(self.formula).max(self.format),
        };
        raw.projected(ignore)
    }
    pub(crate) fn packed(self) -> u8 {
        self.value as u8 | ((self.formula as u8) << 2) | ((self.format as u8) << 4)
    }
    pub(crate) fn unpack(value: u8) -> Self {
        fn state(v: u8) -> Importance {
            match v & 3 {
                0 => Importance::Same,
                1 => Importance::Unimportant,
                _ => Importance::Important,
            }
        }
        Self {
            value: state(value),
            formula: state(value >> 2),
            format: state(value >> 4),
        }
    }
    pub(crate) fn merge(self, other: Self) -> Self {
        Self {
            value: self.value.max(other.value),
            formula: self.formula.max(other.formula),
            format: self.format.max(other.format),
        }
    }
}
/// Both cells hold identical raw text: with equal regional settings and a
/// non-Text-typed column their normalized comparison values are identical,
/// so callers may skip normalization entirely.
pub(crate) fn raw_text_equal(a: Option<&CellInfo>, b: Option<&CellInfo>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => match (&a.value, &b.value) {
            (CellData::Text(t), CellData::Text(u)) => t == u,
            _ => false,
        },
        _ => false,
    }
}
pub(crate) fn normalized_value<'a, S: TableData + ?Sized>(
    sheet: &'a S,
    row: usize,
    column: Option<usize>,
    handling: &ColumnHandling,
) -> CellData<'a> {
    let info = column.and_then(|c| sheet.cell_info(row, c));
    if handling.column_type == ColumnType::Text {
        let text = column
            .map(|c| sheet.display_text(row, c))
            .unwrap_or_default();
        return CellData::Text(Cow::Owned(handling.normalize_text(&text)));
    }
    let Some(info) = info else {
        return CellData::Empty;
    };
    let value = info.value;
    if let CellData::Text(text) = &value {
        let parsed = crate::regional::interpret(text, sheet.regional());
        if !matches!(parsed, crate::CellValue::Text(_)) {
            return CellData::from_owned_value(parsed);
        }
        if handling.ignore_case || handling.ignore_whitespace {
            return CellData::Text(Cow::Owned(handling.normalize_text(text)));
        }
    }
    value
}
pub(crate) fn equal_values(left: &CellData, right: &CellData, handling: &ColumnHandling) -> bool {
    match (left, right) {
        (CellData::Number(l), CellData::Number(r)) => {
            l == r || (l - r).abs() <= handling.numeric_tolerance
        }
        (CellData::Date(l), CellData::Date(r)) => {
            (i128::from(*l) - i128::from(*r)).abs() <= i128::from(handling.date_tolerance.0)
        }
        _ => left == right,
    }
}
pub struct CellComparator<'a> {
    pub handling: &'a ColumnHandling,
    pub format_important: bool,
}
impl CellComparator<'_> {
    /// Compare on materialized sheets (original entry; behavior unchanged).
    pub fn compare(
        &self,
        left: &Sheet,
        right: &Sheet,
        rows: (Option<usize>, Option<usize>),
        columns: (Option<usize>, Option<usize>),
    ) -> CellDiff {
        self.compare_data(left, right, rows, columns)
    }
    /// Generic read-only entry over `TableData` (prototype/demand storage support).
    pub fn compare_data<S: TableData + ?Sized>(
        &self,
        left: &S,
        right: &S,
        rows: (Option<usize>, Option<usize>),
        columns: (Option<usize>, Option<usize>),
    ) -> CellDiff {
        if rows.0.is_none() || rows.1.is_none() {
            return CellDiff {
                value: Importance::Important,
                ..Default::default()
            };
        }
        let (l, r) = (rows.0.unwrap(), rows.1.unwrap());
        let lc = columns.0.and_then(|c| left.cell_info(l, c));
        let rc = columns.1.and_then(|c| right.cell_info(r, c));
        let changed = if self.handling.unimportant {
            Importance::Unimportant
        } else {
            Importance::Important
        };
        let exact_handling = ColumnHandling {
            ignore_case: false,
            ignore_whitespace: false,
            numeric_tolerance: 0.0,
            date_tolerance: Default::default(),
            ..self.handling.clone()
        };
        // Identical raw text with identical regional settings normalizes to
        // identical values; skip the parse/interpret work for that case. Only
        // text cells benefit, and only when the column is not forced to
        // Text-typed comparison (which reads override-aware display text).
        let raw_text_same = self.handling.column_type != ColumnType::Text
            && left.regional() == right.regional()
            && raw_text_equal(lc.as_ref(), rc.as_ref());
        let value = if lc.is_some() != rc.is_some() {
            changed
        } else if raw_text_same
            || equal_values(
                &normalized_value(left, l, columns.0, &exact_handling),
                &normalized_value(right, r, columns.1, &exact_handling),
                &exact_handling,
            )
        {
            Importance::Same
        } else if equal_values(
            &normalized_value(left, l, columns.0, self.handling),
            &normalized_value(right, r, columns.1, self.handling),
            self.handling,
        ) {
            Importance::Unimportant
        } else {
            changed
        };
        // Trim only boundary whitespace and the leading '='; formula string
        // literals and significant internal spaces must never be rewritten.
        // Compared as &str: no per-cell String allocation.
        let formula = if formula_text(lc.as_ref()) == formula_text(rc.as_ref()) {
            Importance::Same
        } else {
            changed
        };
        let ls = lc.as_ref().and_then(|i| i.number_format);
        let rs = rc.as_ref().and_then(|i| i.number_format);
        let format = if ls == rs {
            Importance::Same
        } else if self.format_important && !self.handling.unimportant {
            Importance::Important
        } else {
            Importance::Unimportant
        };
        CellDiff {
            value,
            formula,
            format,
        }
    }
}

/// Formula text for comparison: trimmed, without the leading '='.
fn formula_text<'a>(c: Option<&'a CellInfo<'a>>) -> &'a str {
    c.and_then(|i| i.formula)
        .unwrap_or("")
        .trim()
        .trim_start_matches('=')
}

/// Three independent 2-bit states per cell; no Vec<CellDiff> per row.
#[derive(Debug, Clone, Default)]
pub struct PackedStates {
    bytes: Vec<u8>,
    cells: usize,
}
impl PackedStates {
    pub fn new(cells: usize) -> Self {
        Self {
            bytes: vec![0; cells.saturating_mul(6).div_ceil(8)],
            cells,
        }
    }
    pub fn len_bytes(&self) -> usize {
        self.bytes.len()
    }
    pub fn get(&self, index: usize) -> Option<CellDiff> {
        if index >= self.cells {
            return None;
        }
        let mut packed = 0;
        for layer in 0..3 {
            let bit = (index * 3 + layer) * 2;
            packed |= ((self.bytes[bit / 8] >> (bit % 8)) & 3) << (layer * 2);
        }
        Some(CellDiff::unpack(packed))
    }
    pub(crate) fn set(&mut self, index: usize, diff: CellDiff) {
        let packed = diff.packed();
        for layer in 0..3 {
            let bit = (index * 3 + layer) * 2;
            let shift = bit % 8;
            self.bytes[bit / 8] =
                (self.bytes[bit / 8] & !(3 << shift)) | (((packed >> (layer * 2)) & 3) << shift);
        }
    }
}
