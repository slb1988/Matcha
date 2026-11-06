use crate::{
    rules::{ColumnHandling, ColumnType, Layers},
    CellValue, Sheet,
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
pub(crate) fn normalized_value<'a>(
    sheet: &'a Sheet,
    row: usize,
    column: Option<usize>,
    handling: &ColumnHandling,
) -> Cow<'a, CellValue> {
    let cell = column.and_then(|c| sheet.cell(row, c));
    if handling.column_type == ColumnType::Text {
        let text = column
            .and_then(|c| sheet.display_at(row, c))
            .unwrap_or_default();
        return Cow::Owned(CellValue::Text(handling.normalize_text(&text).into()));
    }
    let value = cell.map_or(&CellValue::Empty, |c| &c.value);
    if let CellValue::Text(text) = value {
        let parsed = crate::regional::interpret(text, &sheet.regional);
        if !matches!(parsed, CellValue::Text(_)) {
            return Cow::Owned(parsed);
        }
        if handling.ignore_case || handling.ignore_whitespace {
            return Cow::Owned(CellValue::Text(handling.normalize_text(text).into()));
        }
    }
    Cow::Borrowed(value)
}
pub(crate) fn equal_values(left: &CellValue, right: &CellValue, handling: &ColumnHandling) -> bool {
    match (left, right) {
        (CellValue::Number(l), CellValue::Number(r)) => {
            l == r || (l - r).abs() <= handling.numeric_tolerance
        }
        (CellValue::Date(l), CellValue::Date(r)) => {
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
    pub fn compare(
        &self,
        left: &Sheet,
        right: &Sheet,
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
        let lc = columns.0.and_then(|c| left.cell(l, c));
        let rc = columns.1.and_then(|c| right.cell(r, c));
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
        let value = if lc.is_some() != rc.is_some() {
            changed
        } else if equal_values(
            &normalized_value(left, l, columns.0, &exact_handling),
            &normalized_value(right, r, columns.1, &exact_handling),
            &exact_handling,
        ) {
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
        let formula = |c: Option<&crate::Cell>| {
            c.and_then(|c| c.formula.as_deref())
                .unwrap_or("")
                .trim()
                .trim_start_matches('=')
                .to_string()
        };
        let formula = if formula(lc) == formula(rc) {
            Importance::Same
        } else {
            changed
        };
        let ls = left
            .style(lc.and_then(|c| c.style))
            .and_then(|s| s.number_format.as_deref());
        let rs = right
            .style(rc.and_then(|c| c.style))
            .and_then(|s| s.number_format.as_deref());
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
