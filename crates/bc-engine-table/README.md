# bc-engine-table

Table Compare engine (FR-C1, M2). CSV via the format's delimiter/qualifier first; Excel
through the `convertMSExcel` converter later. All bodies `todo!()`.

## Responsibilities

- `TableModel`, `TableRow`, `Cell` – parsed table per side (`parse_delimited`).
- `TableRules`, `ColumnHandling` – header row, key columns, numeric tolerance,
  unimportant/ignored columns, sorting, sheet selection.
- `KeyAlign` – row pairing by key columns producing a `bc_core::align::AlignMap`.
- `TableMap` – aligned pair with per-cell `DiffKind`.
- `jobs::TableAlignJob`, `jobs::DetectColumnWidthsJob`.

## Dependency boundary

`bc-core`, `bc-formats`, `bc-jobs`. No GUI, no spreadsheet library yet (xlsx reading is a
later decision; the reference embeds FlexCel).

## Reference class map (from `vmt_report.txt`)

| this crate | reference classes |
|---|---|
| `TableModel` | `TTableContent` (`TLineContent`), `TTableDefinition`, `TTableColWidthsDetector` |
| `TableRules`, `ColumnHandling` | `TTableRules` (`TLineRules`), `TTableColHandlingDialog`, `TTableDefaultColHandlingDialog`, `TTableColResizeDialog` |
| `KeyAlign`, `TableMap` | `TTableMap`, `TTableMapAlignLines` (`TBcAlignLines`), `TTableMapSortList` |
| map edits (future) | `TTableMapEditItem`, `TTableMapEditSideItem`, `TTableMapAlignItem`, `TTableMapEditAllItem`, `TTableMapDeleteItem`, `TTableMapInsertItem`, `TTableMapDeleteColItem`, `TTableMapInsertColItem`, `TTableMapModifyItem`, `TTableMapModifyColItem` |
| `jobs::*` | `TTableMapJob`, `TTableMapAlignBaseJob`, `TTableMapAlignJob`, `TTableMapRealignJob`, `TTableMapSortJob`, `TTableJob`, `TTableColDetectSizeJob`, `TTableColDetectSizeAllJob`, `TTableColDetectSizeDetailsJob`, `TTableColDetectSizeRangeJob` |
| sessions (bc-session) | `TTableSession`, `TTableCompareSession`, `TTableSessionState`, `TTableCompareSessionState` |
| settings UI (not here) | `TTableSessionSheetsFrame`, `TTableSessionColumnsFrame`, `TTableSessionRowsFrame`, `TTableSessionFormatsFrame`, `TTableSessionSpecsFrame`, `TTableFormatTypeFrame`, `TTableFormatRegionalFrame` |
| view (bc-gui) | `TTableView`, `TTableCompareView`, `TUiTableEditor`, `TUiListTable` |

## Open questions

- Regional settings (`TTableFormatRegionalFrame`: decimal/thousands separators) are not
  modelled yet.
- Excel support path: converter to CSV (as the reference's `convertMSExcel` suggests) vs.
  native xlsx reader.
