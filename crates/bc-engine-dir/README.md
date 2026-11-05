# bc-engine-dir

Folder Compare / Merge / Sync engine (FR-D1…FR-D5): dual tree, criteria pipeline,
filters, sync planning, file operations and their background jobs. Wave A implements
local tree/criteria/content comparison. Wave B adds operation planning, execution and refresh;
sync is described below.

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
  (cutoff, attrib, size, protected, streamed UTF-8 content), DisplayFilter (13 values).
  NameFilter::parse returns Result, compiles once and reuses core split_mask_list.
  Character sets, no-extension masks, rooted/relative/any-depth paths and four-group
  evaluation are implemented; exclusions always win.
- `rules::DirRules` / `DirMergeRules` / `DirSyncRules`, `HandlingOptions`, `DirHandling`.
- `sync::SyncPlan` – five presets and CustomSyncRules → snapshot SyncItems, overrides,
  folder action summaries, conflict navigation and OpPlan execution.
- `ops::DirOp` / `OpPlan` – copy/copyto/move/moveto/delete/rename/touch/attrib/exclude with the
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

## Filtering and view contracts

- FilterClock explicitly supplies local today-midnight and UTC offset; no implicit UTC
  midnight or locale guessing. OtherFilters defaults to excluding hidden+system items.
- Other content filters exclude files containing/not containing literal UTF-8 text,
  stream with bounded overlap, poll CancelCheck and error on invalid UTF-8.
- DirTree::apply_filters accepts completed content decisions; it preserves comparison
  statuses, records per-side visibility and promotes included files' ancestors.
- view_rows(ViewOptions) is generation-cached and produces source-aware rows for all
  four folder modes, 13 display modes, suppression/teal and Ignore Unimportant.
  Flatten pairs same-name duplicates by sorted relative path, not contents.
- FilterSettings::apply_script performs transactional partial updates. FilterPresets
  provides validated XML model serialization through bc-core, no new dependencies.
- Decisions and remaining reference-parity questions:
  [pairing/filter ADR](../../docs/decisions/101-folder-pairing-and-filters.md).

## File operations and refresh

OpPlan::build is pure and requires fully scanned selected folders (ScanRequired otherwise).
Selection is side-aware. Plans expand folders into filtered per-file items, validate collisions
and preserve unknown/new/hidden children on directory cleanup. PlanOptions::from_handling
combines session defaults with an explicit include-hidden confirmation override.
Creation time is optional; NTFS permission copying returns Unsupported before any I/O.
Directory metadata is a separate post-order PreserveDir step after child transfers, so
creating/moving children cannot accidentally replace the intended directory timestamp.

FileOpJob consumes a plan, ConfirmPolicy and a shared OpLog. Confirmations are class-scoped;
the permanent-delete fallback always requires its own Interaction answer, even under a
general YesToAll policy. Sources/known targets are revalidated before execution.
Its typed OpOutcome preserves completed/skipped/failed counts, cancellation, bytes,
affected paths, clipboard text and session-only effects, including on cancellation.
Exchange stages both file copies; rollback failure retains recovery backups and reports paths.

refresh_locations/refresh_selection read only affected entries and missing ancestors;
existing ancestor summaries update in memory. refresh_loaded enumerates loaded folders,
full_refresh recursively enumerates all folders, and AutoRefresh turns watch events into
local refreshes (overflow requests a full scan). UI code must reapply current filters after
refresh and apply OpOutcome's session effects explicitly.
Script operation/selection parsing lives in op_script, not bc-script; no runner/GUI wiring
is included here. Script selection ignores expansion but honors file/display filtering.

Tests: `cargo test -p bc-engine-dir --test operations --test op_script`.
[Confirmation and baseline ADR](../../docs/decisions/103-op-confirmation-and-sync-baseline.md).

## Sync previews

SyncPlan::build returns Result (mirror:all is rejected); build_with accepts SyncOptions
and an optional SyncBaseline. Update Both treats two metadata changes since the baseline
or an undirected mismatch as conflict/LeaveAlone. Baseline CRC differences count only
when both snapshots actually contain a CRC; losing a cache is not a file change.
Baseline capture/persistence is caller-owned and is never advanced after a partial run.

Rows keep default and override choices separately; directory overrides/exclusions apply
to descendants transactionally. FolderActions summarizes copy/delete/conflict flags.
next_conflict/previous_conflict do not wrap; an explicit override resolves the navigation
conflict while retaining the original conflict flag for audit.

to_op_plan uses the preview's captured sources/targets, never rebuilds decisions from
new statuses. execute_and_refresh reuses confirmations/log/cancellation, reports actual
copy/delete/create/skip/failure counts and preserves refresh errors alongside outcomes.
Visible-only means display/file filtering, not current expansion or viewport clipping.
Empty folders are created only when requested; nonempty parents needed by selected file
copies are explicit creation/metadata steps even when create-empty is off. Deletion cleans directories only after
their planned children, preserving filtered/new/unexecuted contents.
SyncCommand parses the script presets and serializes them; custom rules have no script
spelling and return an error rather than inventing a command.

Tests: `cargo test -p bc-engine-dir --test sync`.

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
