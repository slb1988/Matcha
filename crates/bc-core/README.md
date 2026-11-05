# bc-core

Foundation layer: line model, diff and alignment traits, undo stack, hashing, encoding,
regex wrapper and the Delphi-style XML persistence tree. Equivalent of the reference
tool's `Ss*` infrastructure units.

## Responsibilities

- `line` – `LineList` over a byte buffer with per-line endings (`LineParser`, `LineWriter`).
- `diff` – `Differ` trait with `MyersDiff` (spec §6 baseline) and `PatienceDiff` placeholders.
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
No GUI or async runtime. Everything above (`bc-formats`, engines, session) builds on it;
`bc-formats`' build script uses `persist::parse_xml` at build time.

Golden-fixture layout for later algorithm work: `tests/golden/README.md`.
Lightweight baseline: `cargo run --release -p bc-core --example core_baseline`.
Integration tests: `cargo test -p bc-core`.

## Reference class map (from `vmt_report.txt`)

| this crate | reference classes |
|---|---|
| `line::LineList`, `LineParser`, `LineWriter` | `TSsLineList`, `TSsLineParser`, `TSsLineWriter`, `TLineConLineList` |
| `line` edit items (future) | `TSsLineListEditItem`, `TSsLineListInsertItem`, `TSsLineListDeleteItem`, `TSsLineListModifyEndingItem`, `TSsLineListModifyEndingStyleItem` |
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
