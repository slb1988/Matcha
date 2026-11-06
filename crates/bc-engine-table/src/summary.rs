//! Outbound UTF-8 data protocol, not instructions or an incoming edit API.
use crate::{
    columns::ColumnChange, compare::Importance, invalid, map::TableMap, rules::Layers, Result,
};
use bc_core::DiffKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummaryFormat {
    Text,
    Csv,
}
fn escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '|' => out.push_str("\\|"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
            c => out.push(c),
        }
    }
    out
}
fn csv(text: &str) -> String {
    if text.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.into()
    }
}
fn status(kind: DiffKind, importance: Importance) -> &'static str {
    match kind {
        DiffKind::LeftOnly => "LeftOnly",
        DiffKind::RightOnly => "RightOnly",
        _ => match importance {
            Importance::Same => "Same",
            Importance::Unimportant => "Unimportant",
            Importance::Important => "Changed",
        },
    }
}
impl TableMap {
    pub fn export_summary(&self, format: SummaryFormat) -> Result<String> {
        if self.is_stale() {
            return Err(invalid("cannot export stale table comparison"));
        }
        let rules = self.rules();
        let left = self.left.borrow();
        let right = self.right.borrow();
        let mut out: String = match format {
            SummaryFormat::Text => "MATCHA-TABLE-DIFF\t1\n".into(),
            SummaryFormat::Csv => "record,key,status,column,layer,old,new\r\n".into(),
        };
        for (index, column) in rules.columns.0.iter().enumerate() {
            if rules.ignore_unimportant && rules.handling(index).unimportant {
                continue;
            }
            for change in &column.changes {
                let change = match change {
                    ColumnChange::Inserted => "Inserted",
                    ColumnChange::Deleted => "Deleted",
                    ColumnChange::Renamed => "Renamed",
                    ColumnChange::Moved => "Moved",
                };
                let old = column.left.map(|c| left.column_name(c)).unwrap_or_default();
                let new = column
                    .right
                    .map(|c| right.column_name(c))
                    .unwrap_or_default();
                match format {
                    SummaryFormat::Text => out.push_str(&format!(
                        "COLUMN\t{}\t{change}\t{}\t{}\t{}\t{}\n",
                        index + 1,
                        column.left.map_or("-".into(), |c| (c + 1).to_string()),
                        column.right.map_or("-".into(), |c| (c + 1).to_string()),
                        escape(&old),
                        escape(&new)
                    )),
                    SummaryFormat::Csv => out.push_str(&format!(
                        "column,,{change},{},structure,{},{}\r\n",
                        index + 1,
                        csv(&old),
                        csv(&new)
                    )),
                }
            }
        }
        let keys = rules.key_columns();
        for (row_index, row) in self.rows().enumerate() {
            if row.importance == Importance::Same {
                continue;
            }
            let key = keys
                .iter()
                .map(|&c| {
                    let col = &rules.columns.0[c];
                    row.left
                        .and_then(|r| col.left.and_then(|c| left.display_at(r, c)))
                        .or_else(|| {
                            row.right
                                .and_then(|r| col.right.and_then(|c| right.display_at(r, c)))
                        })
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
                .join(" / ");
            let state = status(row.kind, row.importance);
            if format == SummaryFormat::Text {
                out.push_str(&format!("ROW\t{}\t{state}\n", escape(&key)));
            }
            let mut emitted = false;
            for (c, column) in rules.columns.0.iter().enumerate() {
                let diff = self.cell_diff(row_index, c).unwrap();
                let name = column
                    .left
                    .map(|c| left.column_name(c))
                    .or_else(|| column.right.map(|c| right.column_name(c)))
                    .unwrap_or_else(|| (c + 1).to_string());
                let lc = row
                    .left
                    .and_then(|r| column.left.and_then(|c| left.cell(r, c)));
                let rc = row
                    .right
                    .and_then(|r| column.right.and_then(|c| right.cell(r, c)));
                for (layer, importance) in [
                    ("value", diff.value),
                    ("formula", diff.formula),
                    ("format", diff.format),
                ] {
                    if importance.projected(rules.ignore_unimportant) == Importance::Same
                        || (layer == "formula" && rules.layers == Layers::Values)
                        || (layer == "format" && rules.layers != Layers::All)
                    {
                        continue;
                    }
                    let (old, new) = match layer {
                        "formula" => (
                            lc.and_then(|c| c.formula.as_deref())
                                .unwrap_or("")
                                .to_string(),
                            rc.and_then(|c| c.formula.as_deref())
                                .unwrap_or("")
                                .to_string(),
                        ),
                        "format" => (
                            left.style(lc.and_then(|c| c.style))
                                .and_then(|s| s.number_format.as_deref())
                                .unwrap_or("")
                                .to_string(),
                            right
                                .style(rc.and_then(|c| c.style))
                                .and_then(|s| s.number_format.as_deref())
                                .unwrap_or("")
                                .to_string(),
                        ),
                        _ => (
                            row.left
                                .and_then(|r| column.left.and_then(|c| left.display_at(r, c)))
                                .unwrap_or_default(),
                            row.right
                                .and_then(|r| column.right.and_then(|c| right.display_at(r, c)))
                                .unwrap_or_default(),
                        ),
                    };
                    match format {
                        SummaryFormat::Text => {
                            let label = if layer == "value" {
                                escape(&name)
                            } else {
                                format!("{} ({layer})", escape(&name))
                            };
                            out.push_str(&format!(
                                "{} | {label} | {} -> {}\n",
                                escape(&key),
                                escape(&old),
                                escape(&new)
                            ));
                        }
                        SummaryFormat::Csv => out.push_str(&format!(
                            "cell,{},{},{},{},{},{}\r\n",
                            csv(&key),
                            state,
                            csv(&name),
                            layer,
                            csv(&old),
                            csv(&new)
                        )),
                    }
                    emitted = true;
                }
            }
            if !emitted && format == SummaryFormat::Csv {
                out.push_str(&format!("row,{},{state},,,,\r\n", csv(&key)));
            }
        }
        Ok(out)
    }
}
