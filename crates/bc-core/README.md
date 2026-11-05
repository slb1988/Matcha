# bc-core

Foundation layer: line model, diff and alignment traits, undo stack, hashing, encoding,
regex wrapper and the Delphi-style XML persistence tree. Equivalent of the reference
tool's `Ss*` infrastructure units.

## Responsibilities

- `line` – per-line ending parser/writer, shared-piece `LineList`, reversible `LineEdit`,
  Windows locked mmap with owned fallback, copy-on-write edits and cheap snapshots.
- `diff` – `Differ`, linear-space `MyersDiff`, content-defined block fallback, `PatienceDiff`.
- `align` – `Aligner` trait, `AlignMap` (row ↔ side-line mapping used by every view),
  `QuickAlign`, `SmartAlign` (+ options), `ByteAlign`, `LineWeight`.
- `edit` – `EditItem`/`EditGroup`/`EditStack` with grouping and save-point tracking.
- `hash` – `ContentHash`, `LineHasher` (normalisation flags), streaming `Crc32`.
- `encoding` – detection, decode/encode, BOM policy.
- `pattern` – regex wrapper for the shared BC dialect + glob translation.
- `persist` – **implemented reader** for the `Value`-attribute / `_N`-list / `&#2;` XML
  dialect shared by all settings files; writer is `todo!()`.

## Dependency boundary

Depends on `quick-xml` (persist), `regex`, `xxhash-rust` (xxh64), and `encoding_rs`.
`memmap2` reuses the existing workspace dependency for the read-only line backend.
No GUI or async runtime. Everything above (`bc-formats`, engines, session) builds on it;
`bc-formats`' build script uses `persist::parse_xml` at build time.

Golden-fixture layout for later algorithm work: `tests/golden/README.md`.
Lightweight baseline: `cargo run --release -p bc-core --example core_baseline`.
Integration tests: `cargo test -p bc-core`.
100 MB open/edit benchmark: `cargo run --release -p bc-core --example line_bench -- <input-file>`.

## Line storage and encoding contracts

- Empty input has zero lines; a final terminator does not add a phantom empty line.
  `ending_style` reports Mixed, while `dominant_ending` selects the most frequent
  nonempty terminator (ties: CRLF, LF, CR). New lines can use `insert_default`.
- Line mutators reject embedded CR/LF and allow `None` only on the final line.
  `LineEdit` applies immediately to `SharedLines` and is then recorded in `EditStack`.
  Do not mutate a target out of band while retaining its undo history.
- `encode(text, encoding, policy, original_has_bom)` preserves the detection's BOM
  flag; `decode_with_errors` additionally reports malformed-sequence replacements.
  Unsupported code pages and unrepresentable output are explicit errors.
- Detection prioritises BOM, then a binary veto (>1% NUL or >10% non-printable ASCII
  controls in 8 KiB), then whole-input UTF-8 validation, then the default code page.
- [Mapped backend safety and save restrictions](../../docs/decisions/002-mmap-backend.md):
  Windows denies write/delete while mapped snapshots exist; sharing conflicts and
  non-Windows use owned storage. `backend()` exposes this choice to the caller.
- Hash normalisation uses Unicode whitespace and Unicode lowercase for valid UTF-8;
  invalid byte input keeps bytes distinct and uses ASCII-only normalisation.
- Edit history starts clean, never coalesces across a save boundary, and tracks
  unreachable saved states after branching/eviction. Close groups before saving.

## Diff contracts

Myers trims common edges before allocating frontiers. Below the fallback threshold it
returns an exact shortest edit script with O(ND) time / O(N+M) working memory and an
explicit work stack (no recursive stack or quadratic trace). Above the default 16,384
remaining items it uses 32–128-item content-defined block hashes, verifies anchored
block equality, and solves unanchored windows of at most 1,024 items. The fallback
preserves every input index in order but may sacrifice the globally shortest script;
set `block_fallback_threshold = 0` to require exact LCS regardless of time cost.
Patience uses unique-in-both anchors, a deterministic right-index LIS and repeated
subrange anchoring, falling back to bounded Myers only when anchors are absent.
`DiffOp` ranges always partition both inputs and adjacent compatible runs are merged.

## Pattern/mask boundary

`Pattern` uses linear-time `regex`, including Unicode case handling, captures and
replacement expansion. The dialect shim translates directional `\<` / `\>` into
word-start/word-end assertions; backreferences/look-around fail with diagnostic errors.
`find_at` offsets must be UTF-8 boundaries and preserve pre-offset anchor context.

`glob_to_pattern` accepts **one positive mask**, anchored to a complete name; `*` / `?`
do not cross path separators. Directory masks require an explicit trailing separator
(`obj\` / `obj/`), matching neither the file `obj` nor descendants. Semicolon lists
and leading exclusions are rejected by this primitive. `split_mask_list` only parses
`MaskSpec { pattern, exclude, folder }` and strips markers. Combining positive/negative
masks, negative-only defaults, and recursive path exclusion belong to
`bc-engine-dir::filter::NameFilter` in the later folder change (coordinator-confirmed
scope). The `*.c;*.h;-*.bak;obj\` regression table verifies parsing and each primitive,
not production list evaluation.

## Reference class map (from `vmt_report.txt`)

| this crate | reference classes |
|---|---|
| `line::LineList`, `LineParser`, `LineWriter` | `TSsLineList`, `TSsLineParser`, `TSsLineWriter`, `TLineConLineList` |
| `line::LineEdit` | `TSsLineListEditItem`, `TSsLineListInsertItem`, `TSsLineListDeleteItem`, `TSsLineListModifyEndingItem`, `TSsLineListModifyEndingStyleItem` |
| `align::Aligner`, `QuickAlign`, `SmartAlign` | `TBcAlign`, `TBcQuickAlign`, `TBcSmartAlign`, `TBcPatienceDiffAlign` |
| `align::ByteAlign` | `TBcByteAlign`, `TBcIntAlign`, `THexMapByteAlign` |
| `align::AlignMap` driver | `TBcAlignLines` (→ `TTextMapAlignLines`, `TTableMapAlignLines`) |
| `edit::EditStack`, `EditItem`, `EditGroup` | `TSsEditStack`, `TSsEditItem`, `TSsEditParentItem` |
| `pattern::Pattern` | `TSsRegExpr`, `TREParser`, `TREStateTable`, `TRETransitionTable`, `TREPartitionTable`, `TREPositionTable` |
| `persist::Node` | `TSsPersistent`, `TSsPersistentList`, `TSsPersistentObjList`, `TSsPersistentStrList`, `TSsPersistentParent`, `TSsPersistentParentEx` |
| `hash::Crc32` consumer | `TVfsGetCRC32Job`, `TIdHashCRC32` |
| thread/queue primitives used by bc-jobs | `TSsThread`, `TSsPriorityQueue`, `TAsyncThreadSynchronizer`, `TSsFileStream` |

## Open questions

- [STX write rule decision placeholder](../../docs/decisions/001-stx-write-rule.md).
- [Smart scoring decision placeholder](../../docs/decisions/003-smart-scoring.md).
