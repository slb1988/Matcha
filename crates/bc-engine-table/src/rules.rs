use crate::{invalid, Result};
use bc_core::align::AlignStrategy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ColumnType {
    #[default]
    General,
    Text,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DateTolerance(pub i64);
impl DateTolerance {
    pub fn days(days: u32) -> Self {
        Self(i64::from(days) * crate::regional::DAY_MS)
    }
    pub fn hours(hours: u32) -> Self {
        Self(i64::from(hours) * 3_600_000)
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColumnHandling {
    pub is_key: bool,
    pub use_default: bool,
    pub column_type: ColumnType,
    pub unimportant: bool,
    pub ignore_case: bool,
    pub ignore_whitespace: bool,
    pub numeric_tolerance: f64,
    pub date_tolerance: DateTolerance,
}
impl Default for ColumnHandling {
    fn default() -> Self {
        Self {
            is_key: false,
            use_default: false,
            column_type: ColumnType::General,
            unimportant: false,
            ignore_case: false,
            ignore_whitespace: false,
            numeric_tolerance: 0.0,
            date_tolerance: DateTolerance::default(),
        }
    }
}
impl ColumnHandling {
    pub fn resolved(&self, defaults: &Self) -> Self {
        if !self.use_default {
            return self.clone();
        }
        Self {
            is_key: self.is_key,
            use_default: false,
            ..defaults.clone()
        }
    }
    pub fn validate(&self) -> Result<()> {
        if !self.numeric_tolerance.is_finite()
            || self.numeric_tolerance < 0.0
            || self.date_tolerance.0 < 0
        {
            return Err(invalid("tolerances must be finite and nonnegative"));
        }
        Ok(())
    }
    pub fn normalize_text(&self, value: &str) -> String {
        let text = if self.ignore_whitespace {
            value.split_whitespace().collect::<Vec<_>>().join(" ")
        } else {
            value.into()
        };
        if self.ignore_case {
            text.to_lowercase()
        } else {
            text
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ColumnMode {
    #[default]
    ByNumber,
    Automatic,
}
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum KeySelection {
    /// Suggestions are not locked. Once the user edits, call set_manual_keys.
    Automatic {
        columns: Vec<usize>,
        score: f64,
    },
    Manual(Vec<usize>),
    #[default]
    AllColumns,
    /// Compatibility preset: positional column mapping, all columns as keys.
    ByColumnNumber,
}
impl KeySelection {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Automatic { .. } => "自动",
            Self::Manual(_) => "手动",
            Self::AllColumns => "全部列",
            Self::ByColumnNumber => "按列号",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Layers {
    #[default]
    Values,
    ValuesAndFormulas,
    All,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TableRules {
    pub columns: crate::columns::ColumnMapping,
    pub default_handling: ColumnHandling,
    pub column_mode: ColumnMode,
    pub keys: KeySelection,
    pub sort_by_key: bool,
    #[serde(with = "strategy_serde")]
    pub strategy: AlignStrategy,
    pub use_closeness: bool,
    pub layers: Layers,
    pub format_important: bool,
    /// Projection only; raw comparison states are unchanged.
    pub ignore_unimportant: bool,
}
impl Default for TableRules {
    fn default() -> Self {
        Self {
            columns: Default::default(),
            default_handling: Default::default(),
            column_mode: Default::default(),
            keys: KeySelection::Automatic {
                columns: Vec::new(),
                score: 0.0,
            },
            sort_by_key: false,
            strategy: AlignStrategy::Smart,
            use_closeness: true,
            layers: Layers::Values,
            format_important: false,
            ignore_unimportant: false,
        }
    }
}
impl TableRules {
    pub fn handling(&self, column: usize) -> ColumnHandling {
        self.columns
            .0
            .get(column)
            .map(|c| c.handling.resolved(&self.default_handling))
            .unwrap_or_else(|| self.default_handling.clone())
    }
    pub fn key_columns(&self) -> Vec<usize> {
        let explicit: Vec<_> = self
            .columns
            .0
            .iter()
            .enumerate()
            .filter(|(_, c)| c.handling.is_key)
            .map(|(i, _)| i)
            .collect();
        if !explicit.is_empty() {
            return explicit;
        }
        match &self.keys {
            KeySelection::Automatic { columns, .. } | KeySelection::Manual(columns)
                if !columns.is_empty() =>
            {
                columns.clone()
            }
            _ => (0..self.columns.0.len()).collect(),
        }
    }
    pub fn set_manual_keys(&mut self, columns: Vec<usize>) -> Result<()> {
        if columns.iter().any(|&i| i >= self.columns.0.len()) {
            return Err(invalid("key column out of range"));
        }
        for (i, column) in self.columns.0.iter_mut().enumerate() {
            column.handling.is_key = columns.contains(&i);
        }
        self.keys = KeySelection::Manual(columns);
        Ok(())
    }
    pub fn validate(&self, left_columns: usize, right_columns: usize) -> Result<()> {
        self.default_handling.validate()?;
        self.columns.validate(left_columns, right_columns)?;
        for c in &self.columns.0 {
            c.handling.validate()?;
        }
        if self
            .key_columns()
            .iter()
            .any(|&i| i >= self.columns.0.len())
        {
            return Err(invalid("key column out of range"));
        }
        Ok(())
    }
}
mod strategy_serde {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        strategy: &AlignStrategy,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(match strategy {
            AlignStrategy::Quick => "Quick",
            AlignStrategy::Smart => "Smart",
            AlignStrategy::Patience => "Patience",
            AlignStrategy::None => "None",
        })
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<AlignStrategy, D::Error> {
        match String::deserialize(d)?.as_str() {
            "Quick" => Ok(AlignStrategy::Quick),
            "Smart" => Ok(AlignStrategy::Smart),
            "Patience" => Ok(AlignStrategy::Patience),
            "None" => Ok(AlignStrategy::None),
            v => Err(serde::de::Error::custom(format!(
                "unknown alignment strategy {v}"
            ))),
        }
    }
}
