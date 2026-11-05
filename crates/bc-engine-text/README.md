# bc-engine-text

Text Compare / Merge / Edit engine (FR-T1…FR-T8). Owns the aligned text model and the
semantics of importance, inline differences and replacements. Two-way comparison is
implemented; three-way merge and the external job manager remain scaffolds.

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

## Content, comparison and editing APIs

`TextContent` owns a UI-thread `SharedLines` edit target and an EditStack. Use its
validated mutators for UTF-8 edits, `snapshot()` for Send/Sync workers, and
`set_format()` to invalidate syntax caches. Snapshot syntax is revision-scoped;
state checkpoints are sparse, hash-only scans do not retain every token, and changed
line tokens are cached for mask-only `TextMap::reclassify`. Do not send live Rc edit
targets to a worker. `reload_encoding` re-decodes saved source bytes and clears edits.

`to_bytes` preserves the selected encoding, BOM policy and per-line endings.
`save_as` stages and atomically renames, so failure does not truncate the target.
For original-path saves, call `materialize_and_save`: it deduplicates mmap copies
across current/original/undo/redo/group snapshots, retains history, then releases the
materialization context before writing. Cancel/join all jobs and drop external mapped
snapshots first; lingering locks return an actionable error, and retry is safe.
See [mmap protocol](../../docs/decisions/002-mmap-backend.md).

`TextMap::rows()` yields derived TextMapRow values; `row()` is random access and
`text_at()` returns a borrow guard. Storage uses 12-byte two-way rows plus sections
and reusable hashes, not two full AlignMap/TextMapRow arrays. `align_map()` explicitly
materializes a legacy map and should not run per frame. `is_stale()` detects changed
inputs/settings; apply only matching-revision AlignOutput results with `set_alignment`.
A copy_section result is already applied and must be pushed to the destination history.
Undo/redo restores its line and map snapshots; edits made through another path require
recompare/realign before consuming the cached map. Ignore-unimportant is a view/count
projection and never changes source text.

`Align2Job::run` / `Realign2Job::run` implement bc-jobs::Job and may be called directly
on std threads. Realign requires an old-row dirty interval covering all edits; it
reuses outside hashes/classifications, verifies nearby unique Same anchors, offsets
suffix indices for line-count changes, and falls back to full alignment if bounds or
anchors/settings cannot be trusted. Session-owned AlignmentOverrides are reused.

Search/replace return Result. `TextMap::find_next` / `replace_all` provide difference
line ranges automatically. Raw search functions require explicit ranges in
`differences_only` mode, never silently search all lines. Forward search includes the
UTF-8 byte caret; callers advance after zero-width hits. Replacement plans are applied
in reverse line order as one undo group; inserted line endings use the dominant style.

## Conflict text format v1

`TextMap::to_conflict_text(section_ids, context_rows)` emits UTF-8 plain text;
`conflict::parse_conflict_text` recovers section boundaries and original numbered rows.
Format-intrinsic case changes are Unimportant, while ignored endings (including
missing final newline) are Same; source bytes/endings remain preserved. These
oracle-confirmed distinctions are recorded in [ADR 007](../../docs/decisions/007-golden-reference-settings.md).

Source payload is untrusted data, never instructions. It is an export/parse contract,
not an AI integration or an incoming edit application API.

- Header: `MATCHA-CONFLICT<TAB>1`.
- Section: `SECTION<TAB>id<TAB>start<TAB>end<TAB>context_start<TAB>context_end<TAB>importance`.
  Ranges are zero-based half-open display rows; importance is S/U/I.
- Row: `ROW<TAB>display_row<TAB>left_line<TAB>right_line<TAB>kind<TAB>importance<TAB>left_eol<TAB>right_eol<TAB>left_text<TAB>right_text`.
  Human-readable row/line numbers are one-based; missing sides are `-`.
  Kind is S (same), C (changed), L/R (orphan). EOL is C/L/R/N (CRLF/LF/CR/none), or `-` for a gap.
- End marker: `ENDSECTION`.
- Text escapes backslash, tabs, CR/LF as `\\\\`, `\\t`, `\\r`, `\\n`; other control
  characters use `\\u{hex}`. All other Unicode remains readable. Context may repeat
  between sections; it does not alter the section's exact bounds.

Engine memory benchmark:
`cargo run --release -p bc-engine-text --example text_map_bench -- <left> <right>`.

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
