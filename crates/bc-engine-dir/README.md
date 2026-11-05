# bc-engine-dir

Folder Compare / Merge / Sync engine (FR-D1…FR-D5): dual tree, criteria pipeline,
filters, sync planning, file operations and their background jobs. All bodies `todo!()`.

## Responsibilities

- `tree::DirTree` / `DirNode` / `CompareStatus` – paired tree with per-side `VfsEntry`s,
  expansion, exclusion, manual align overrides.
- `criteria::Criteria` – `attrib`, `version`, `timestamp[:n sec;IgnoreDST]`,
  `size|CRC|binary|rules-based`, `timezone`, `follow-symlinks`, `ignore-unimportant`.
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
compare). No GUI.

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
