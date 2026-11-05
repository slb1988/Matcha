# bc-engine-text

Text Compare / Merge / Edit engine (FR-T1…FR-T8). Owns the aligned text model and the
semantics of importance, inline differences, replacements and 3-way merge. All bodies
are `todo!()`.

## Responsibilities

- `content::TextContent` – one pane: `LineList` + encoding + format + `EditStack`.
- `rules::TextRules` / `TextMergeRules` – comparison, alignment and merge settings
  (`Favor`, `/iu`, `/force`).
- `map::TextMap` / `TextMapRow` – rows over 2–3 contents with `DiffKind`, `Importance`
  and section index; navigation and section copy entry points.
- `importance` – `Importance {Same, Unimportant, Important}` and block propagation (spec §6).
- `inline` – character-level spans within a changed row (FR-T2).
- `merge` – `MergeState`, `Conflict`, `auto_merge` → outcome used for exit codes 0/14/101.
- `jobs` – `Align2Job`, `Realign2Job`, `Align3Job`, `Realign3Job` (`bc_jobs::Job` impls).
- `search` – find / replace-all (FR-T6).

Test scaffold: `cargo test -p bc-engine-text`; lightweight baseline:
`cargo run --release -p bc-engine-text --example text_baseline`.

## Dependency boundary

`bc-core`, `bc-formats`, `bc-jobs`, `bc-vfs`. No GUI. Consumers: `bc-engine-dir`
(rules-based compare), `bc-session`, `bc-report`, `bc-script`, `bc-gui`.

## Reference class map (from `vmt_report.txt`)

| this crate | reference classes |
|---|---|
| `TextContent` | `TLineContent`, `TFileContent`, `TLineConLineList` |
| `TextRules`, `TextMergeRules` | `TLineRules`, `TTextRules`, `TTextMergeRules`, `TTextWeightList` |
| `TextMap`, `TextMapRow` | `TTextMap`, `TTextMapAlignLines` |
| map edits (future `EditItem`s) | `TTextMapEditItem`, `TTextMapEditSideItem`, `TTextMapAlignItem`, `TTextMapEditAlignItem`, `TTextMapDeleteItem`, `TTextMapInsertItem`, `TTextMapMoveItem`, `TTextMapMoveEOFItem`, `TTextMapMoveEmptyEOFItem`, `TTextMapResetItem`, `TTextMapResetAllItem` |
| `jobs::*` | `TTextMapJob`, `TTextMapAlign2Job`, `TTextMapRealign2Job`, `TTextMapAlign3BaseJob`, `TTextMapAlign3Job`, `TTextMapRealign3Job` |
| session kinds (in bc-session) | `TTextSession`, `TTextCompareSession`, `TTextMergeSession`, `TTextEditSession`, `TTextPatchSession`, `TTextSessionState`, `TTextCompareSessionState`, `TTextMergeSessionState` |
| settings UI (not here) | `TTextSessionAlignmentFrame`, `TTextSessionImportanceFrame`, `TTextSessionReplacementsFrame`, `TTextMergeSessionAlignmentFrame`, `TTextSessionFormatsFrame`, `TTextSessionSpecsFrame` |
| editor widget (bc-gui) | `TTextEditor`, `TTextDetailsEditor`, `TUiTextEditor` |

## Open questions

- [Smart scoring decision placeholder](../../docs/decisions/003-smart-scoring.md).
- Inline "isolated difference" merging rules (spec §8).
- Exact conflict definition when only the center changes (`conflict_on_center_change`).
- Whether `TextMap` rows should be stored or derived on the fly from `AlignMap` for
  100 MB inputs (memory budget: one `TextMapRow` is 56 bytes; 2 M rows ≈ 112 MB, so a
  compact representation is likely needed).
