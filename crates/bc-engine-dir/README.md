# bc-engine-dir

Folder Compare / Merge / Sync engine (FR-D1…FR-D5): dual tree, criteria pipeline,
filters, sync planning, file operations and their background jobs. Wave A implements
local tree/criteria/content comparison; operations and sync remain scaffolded.

## Responsibilities

- `tree::DirTree` / `DirNode` / `CompareStatus` – paired tree with per-side `VfsEntry`s,
  expansion, exclusion, manual align overrides. Stable arena NodeIds and per-folder
  child tables replace recursive owned nodes; readers use node/find/root/nodes.
  Visible rows are generation-cached; dirty ancestor summaries keep red and purple
  independently and expose pending (hollow) / unscanned (yellow) states.
- `criteria::Criteria` – `attrib`, `version`, `timestamp[:n sec;IgnoreDST]`,
  `size|CRC|binary|rules-based`, `timezone`, `follow-symlinks`, `ignore-unimportant`.
  Independent quick size/name-case flags and content skip/override flags round-trip
  through explicit additional switches (no-size, filename-case, skip-quick-same,
  no-content-override). Script omission disables timestamp tests.
- `filter` – `NameFilter`/`Mask` (`-` exclude, trailing `\` folder), `OtherFilters`
  (cutoff, attrib, size, protected), `DisplayFilter` (12 values = report `display-*`).
- `rules::DirRules` / `DirMergeRules` / `DirSyncRules`, `HandlingOptions`, `DirHandling`.
- `sync::SyncPlan` – `update|mirror` × direction → `SyncAction`s.
- `ops::DirOp` – copy/copyto/move/moveto/delete/rename/touch/attrib/exclude with the
  script option types (`PathMode`, `Target`, `RenameSpec`, `TouchSpec`).
- `jobs` – `LoadDirJob`, `ContentCompareJob` (also `/qc` and `compare`),
  `ContentSearchJob`, `FileOpJob`.

## Dependency boundary

`bc-core`, `bc-formats`, `bc-jobs`, `bc-vfs`, `bc-engine-text` (rules-based content
compare), bc-engine-bin (safe PE resource reader). No GUI or async runtime.
No bc-core or bc-jobs changes were needed. Content jobs use CancelCheck and Crc32.
RulesBased deliberately returns Unsupported until the text branch is merged.

ContentResult carries NodeId + revision, content status, bytes_read and version result;
apply rejects stale work after listing changes, criteria changes or realignment.
Version jobs read PE resources, never execute a DLL. Binary comparison reads 64 KiB
blocks, handles short reads, and stops at the first differing block. Job::run returns
typed DirectoryListing / ContentResult payloads without mutating a tree on a worker.

PairingOptions controls case sensitivity, extension stripping and NFC.
AlignmentRule uses corresponding wildcard captures (or regex captures), optionally
scoped to a relative directory; Align With inserts an exact rule. Rule changes replay
stored listings and retire old NodeIds, retaining overrides across rescans.
NFC uses bc-vfs's Windows NormalizeString boundary; non-Windows returns Unsupported.

Validation: `cargo test -p bc-engine-dir`, `cargo test -p bc-engine-bin ver::`.
Benchmark: `cargo run --release -p bc-engine-dir --example list_bench`.
It generates two 100,000-file trees in the system temporary directory, checks top-level
visibility <1 s and cleans them up. Sparse 1 GiB early-exit/cancellation tests are in
tests/content.rs (Windows fsutil sets the sparse flag on disposable files only).

## Reference class map (from `vmt_report.txt`)

| this crate | reference classes |
|---|---|
| `DirTree`, `DirNode` | `TDirTree` (`TSsMapTree`), `TDirSyncTree`, `TDirAlignWithItem`, `TDirAlignWithList` |
| `Criteria`, `DirRules` | `TDirRules`, `TDirMergeRules`, `TDirSyncRules`, `TSessionRules` |
| `NameFilter`, `OtherFilters` | `TDirFilters`, `TDirOtherFilterItem`, `TDirOtherFilterList`, `TDirFileFormatList` |
| `jobs::LoadDirJob` | `TVfsLoadDirJob`, `TVfsEnumChildrenJob` |
| `jobs::ContentCompareJob` | `TQuickCompareJob`, `TQuickCompareFilesJob`, `TDirTreeContentCompareJob` |
| `jobs::ContentSearchJob` | `TDirTreeContentSearchJob` |
| `jobs::FileOpJob`, `ops::DirOp` | `TOpMoveBothJob`, `TOP*` dialogs (`TOPEXCLUDEDIALOG`, `TOPDOSATTRIBDIALOG`, `TOPUNIXATTRIBDIALOG`, …) |
| sessions (bc-session) | `TDirBaseSession`, `TDirSession`, `TDirCompareSession`, `TDirMergeSession`, `TDirSyncSession`, `TDirSessionState`, `TDirCompareSessionState`, `TDirMergeSessionState`, `TDirSyncSessionState`, `TDirChildSessionList` |
| settings UI (not here) | `TDirSessionComparisonFrame`, `TDirSessionHandlingFrame`, `TDirSessionNameFilterFrame`, `TDirSessionOtherFiltersFrame`, `TDirSessionMiscFrame`, `TDirSessionSpecsFrame`, `TDirSyncSessionOpsFrame`, `TDIRALIGNWITHITEMDIALOG`, `TDIRNAMEFILTERPRESETDIALOG`, `TSNAPSHOTDIALOG` |
| views (bc-gui) | `TDirView`, `TDirCompareView`, `TDirMergeView`, `TDirSyncView`, `TDirTreeView`, `TDirSyncTreeView`, `TDirBaseTreeView` |

## Open questions

- Exact `CompareStatus` set versus the reference's colouring (e.g. "newer on both
  sides" vs. conflict in sync) – confirm with `rt_strings.txt` status strings.
- Timestamp comparison granularity across file systems (FAT 2 s vs NTFS 100 ns); the
  default tolerance here is 2 s, to be verified against the reference's default.
- Snapshot (`.bcss`) format – required before `snapshot` can be implemented.
