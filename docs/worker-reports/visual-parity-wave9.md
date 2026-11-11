# visual-parity Wave 9: folder sync view re-skin

## Summary

- The folder sync view (`sync_view.rs`) no longer uses the legacy row-based layout. It now uses the classic chrome over the shared folder renderers:
  - **Menu bar:** Session/Actions/Edit/Search/View/Tools/Help, transcribed from `inventories.sync.menus`. Existing actions are wired and unimplemented items are disabled; Help uses product-neutral wording.
  - **Toolbar:** large icons per `inventories.sync.toolbar`: Home, Sessions▾, Minor (toggle), Expand, Collapse, Select, Refresh, Stop (enabled only while busy), Peek (toggle).
  - **Paths area (40 px):** two editable folder boxes, each with a "..." browse button, and a swap button between them.
  - **Operations area (191 px):** the operation dropdown (default **Update Both**, matching the reference), a rules button, a description line, and the summary lines "Copy N file(s) and M folder(s) to left/right, B bytes" and "Prompt for N file(s)". These come from the **existing sync plan** (`SyncSummary`, computed from the `SyncPlan` items in `publish`). Also: the Filters box with a filter button and Accept/Cancel, and the Sync Now button, wired to the **existing** `Command::SyncRun` (enabled once the plan is ready).
  - **Trees, log and status bar:** the two trees are **drawn by the shared `DirCompareView::draw_grid`** (no copied tree code), with the central column widened to 117 px for the arrow column. Each row shows its plan mark there: left→right arrows in the first sub-column, right→left arrows in the third, "?" in both for an unresolved conflict, and a red × for deletes. The log (106 px) and the status bar (per-side file count / bytes + free space) also reuse the shared renderers.
- Opening state matches the reference: fully expanded, differences only (identical files and folders are not listed), dates in the local format, log open, filter box `*.*`.
- `compare.ps1` folder-sync named regions: **11/11 ≤ 2 px**. tree.left and tree.right are exact (0). arrows, headers.left and headers.right are within 0–1 px. menubar, toolbar, paths, operations, log and statusbar are within ±2 px (outer-edge detection noise). The `folder-sync` scene has no color samples in tokens, so I sampled 11 structural colors from the reference image myself; **all 11 match exactly** (table below).
- `snap-matcha.ps1 -All -Verify`: **17/17 verified=True**, identical across two runs. The 16 existing scenes are **byte-identical** to Wave 8, including the three folder compare scenes folder-basic `BCFE37E8`, folder-expanded `86977D7E` and folder-diffs-filter `060B1F8C`.
- clippy exit 0. `cargo test --workspace`: 81 targets, **451 passed / 0 failed / 5 ignored**. That is 3 new sync view tests; all existing folder tests pass, with one sync-mode test adapted (see below). Touched files show no rustfmt diff.
- Engine crates are untouched, there are no new dependencies, `tokens.json` and `compare.ps1` are unchanged, and the reference tool was not launched.

## Commits

| Commit | Content |
|---|---|
| `1484038` | feat(gui): sync view chrome re-skin (`sync_view.rs` rewrite; `dir_view.rs`: sync marks/plan summary, 117 px arrow column, sync status fields, shared renderers exposed as `pub(crate)`, legacy sync layout code removed; `app.rs` wiring) |
| `35851fb` | feat(visual): `folder-sync` snapshot scene + `snap-matcha.ps1` scene list |
| `6603214` | fix(gui): needless `mut` in a test (I committed before clippy finished; corrected in a separate commit) |
| `f20cdf7` | feat(gui): append the legacy sync view entries to the new menus (so no entry is lost) |
| this report's commit | report, side-by-side image, evidence |

## Side-by-side image

- [folder-sync](assets/visual-wave9-folder-sync-side.png) (left: reference, right: Matcha)

## Named-region deviations (compare.ps1; edges left/top/right/bottom)

| Region | folder-sync |
|---|---|
| menubar | −2,0,−2,0 |
| toolbar | −2,0,2,0 |
| paths | 1,0,2,0 |
| operations | −2,0,2,1 |
| headers.left | 0,1,0,1 |
| headers.right | 0,1,0,1 |
| tree.left | 0,0,0,0 |
| tree.right | 0,0,0,0 |
| arrows | 0,1,0,0 |
| log | −2,0,2,0 |
| statusbar | −2,0,0,0 |

Raw results: `evidence/visual-parity-wave9/folder-sync-result.json`. The arrow glyph positions also match by measurement: the yellow-pixel bounding box of the left→right arrow is x639–648 in both images; the right→left arrow is x717–726 in the reference and x718–727 in Matcha.

## Color samples (self-sampled from the reference; tokens have none for this scene)

| Sample | Point | Reference | Matcha |
|---|---|---|---|
| Selected row | (300,333) | #A8CDF1 | #A8CDF1 |
| Stripe row | (300,363) | #F8FAF9 | #F8FAF9 |
| Row background | (300,353) | #F8FAF9 | #F8FAF9 |
| Arrow column separator (rows) | (663,380) | #C0C0C0 | #C0C0C0 |
| Arrow column separator (header) | (663,312) | #A0A0A0 | #A0A0A0 |
| Left tree / arrow column seam | (625,380) | #C0C0C0 | #C0C0C0 |
| Header background | (300,312) | #FFFFFF | #FFFFFF |
| Log text box | (700,900) | #E3E7ED | #E3E7ED |
| Operations panel | (100,250) | #F0F0F0 | #F0F0F0 |
| Scrollbar track | (1375,600) | #F0F0F0 | #F0F0F0 |
| Status bar | (500,950) | #F0F0F0 | #F0F0F0 |

Row text colors (red = different/newer, purple = orphan, gray = older) reuse the Wave 3 folder palette (`legend::side_color`). Samples on glyph strokes are not counted (HANDOFF §8).

## Old entry → new entry (migration table)

| Old sync view entry | New entry |
|---|---|
| "Folder Sync" heading | Window title `<left> <--> <right> - Folder Sync - Matcha` |
| Method dropdown (Update left/right/both, Mirror to left/right, Custom) | Operations dropdown (Update Left/Right/Both, Mirror to Left/Right, Custom Sync); default changed from Update right to **Update Both** (the reference default) |
| Custom: five rule dropdowns | Shown in the operations area when Custom Sync is selected |
| "Create empty folders" / "Visible only" checkboxes | Session › Create Empty Folders / Visible Items Only (appended checkable items) |
| Sync Now button | Operations area Sync Now (large button) + Session › Sync Now |
| Previous / Next conflict buttons | Search › Next Conflict (Ctrl+N) / Previous Conflict (Ctrl+P); F7/Shift+F7 kept |
| "Override selected item" menu (six choices) | Actions › Leave Alone / Copy Right to Left / Copy Left to Right / Delete Left / Delete Right / Exclude (**now applies to every selected item**; before, only the first one) |
| Preview statistics line | Operations area summary lines + Session › Folder Sync Info (Ctrl+I, full statistics) |
| Legacy shared button row: Session (browse base folders, swap, back/forward, up level, compare base/parent folders, sync base folders, rules) | Paths area "..." buttons, swap button; Session › Swap Sides / Compare Base Folders / Compare Parent Folders / Session Settings... (also the operations-area rules button) + appended Back / Forward / Up One Level |
| Legacy Actions menu (open pair, compare to, align with, quick compare, content compare, copy/move/exchange/delete/rename/attributes/touch/new folder/exclude/ignored/copy filename, open/close/subfolders, set base folder / on other side / both sides, open in new view, refresh selection) | Actions menu items with the same names plus an appended section (Open, Open Folder, Compare To..., Align With..., Quick Compare▸, Compare Contents▸, Copy/Move to Side..., Copy/Move to Folder..., Exchange..., Delete..., Rename..., Attributes..., Touch..., New Folder..., Ignored, Set as Base Folder, Set as Base on Other Side); Enter still opens the file pair |
| Legacy Selection menu (select all / files / newer / orphans / invert) | Edit › Select All / Invert Selection + appended Select All Files, Select Newer▸, Select Orphans▸; toolbar Select |
| Legacy View menu (toolbar, log, legend, columns, filter options) | View › Ignore Unimportant Differences (toolbar Minor) / Suppress Filters (toolbar Peek) / Columns▸ / Legend / Log / Toolbar + appended Show All / Show Differences |
| Legacy Find / Next / Previous, Next / Previous difference, Expand / Collapse all, Refresh / Full refresh | Search › Find Filename... (Ctrl+F) / Find Next/Previous Filename (F3/Shift+F3) + appended Next/Previous Difference; Edit › Expand All / Collapse All (toolbar Expand/Collapse) / Refresh (F5, toolbar Refresh) / Full Refresh (Ctrl+F5) |
| Legacy filter row (name filter + apply) | Filters box (Enter or Accept applies, Cancel reverts the edit) + filter button (Session Settings filter page); presets and other filters are in Session Settings |
| Path text line | Editable folder boxes (Enter loads) |
| Statistics line / progress bar + cancel task | Status bar (progress while busy) + toolbar Stop |
| Log panel | 106 px log area (gear closes it, document icon saves the log); View › Log |
| Home (app top bar) | Toolbar Home, Session › Close Tab; the app no longer draws its top bar for the sync view |

Unmigrated entries: none. Reference items with no Matcha implementation are shown disabled: New Session/Tab/Window, Workspace, Open/Save Session, Locked, Clear Session, Open With, Explorer, Columns Ext/Status/Revision/Owner/Group, Tools, Help.

## Sync logic

No new sync logic. The operation dropdown and custom rules still send `Command::SyncMode`, overrides send `Command::SyncOverride`, and Sync Now sends `Command::SyncRun`. The summary lines and arrow marks are read from the `SyncPlan` the worker already builds (`effective()` / `unresolved_conflict()`). Two UI defaults now follow the reference: the default operation is Update Both (in both the worker's `Model::new` and `SyncView`, so the plan is built only once), and the view opens expanded and shows differences only.

## Shared tree component and folder compare non-regression

The tree, log and status bar for both views come from the same `DirCompareView` methods. `draw_grid` picks the central column width by mode (compare 37 px / sync 117 px), and only the sync branch draws the three arrow sub-columns and the marks. The folder compare output is unchanged; its three snapshot scenes are byte-identical. The legacy sync-only code was removed: `toolbar` / `action_menu` / `columns_menu` / `draw_table`, about 600 lines. The shared Ctrl+N/P / F5 / Ctrl+F / F3 shortcuts now also apply in the sync view, with Ctrl+N/P navigating conflicts there; F2 / Insert stay compare-only.

Test change: `crc_columns_quick_compare_and_sync_preview_stay_in_background` runs a quick compare on an identical file in sync mode. Since the sync view now shows differences only by default, the test switches to Show All first; the behavior it covers is unchanged. New tests:
- `summary_lines_follow_the_reference_wording`
- `menu_inventory_matches_tokens_and_hides_the_reference_name`
- `sync_view_plans_update_both_with_arrows_summary_and_layout` (real temp folders: Update Both plan, summary, arrow marks, identical file hidden, layout text, Sync Now enabled, title)

## Known differences

- **Row order:** in the reference, `newer.txt` (right) comes before `older.txt` (left). In Matcha, orphans go to the end of their group. This is the same existing folder-compare sort as Wave 3 (comparison behavior, unchanged).
- **Log:** the reference shows a "Username: …" line and timestamps. Matcha logs "Load comparison: …" and "Ready" without timestamps, so snapshots stay deterministic. This matches Wave 3's folder compare treatment.
- **Free space:** the status bar fields are wired to Wave 4's parallel host query (the same `snapshot.free`). The snapshot scene uses relative corpus paths, which have no drive letter, so the free-space fields are empty there. Absolute paths in normal use show "N GB free on X:\". I deliberately did not make drive detection resolve absolute paths in the snapshot: free space changes run to run and would break hash determinism.
- **Paths:** the path text is a relative local corpus path (the reference shows an absolute path on another machine), so this is a content difference, not a geometry one.
- **Icons:** toolbar/button icons, the arrow glyphs and the Sync Now icon are original drawings (red line: no reference resources extracted), so their shapes differ from the reference bitmaps.
- The hand-drawn arrows in the reference screenshot are capture annotations and were not reproduced (per the brief).
- The menu bar has no "New version available..." link (same as earlier waves).
- The Session Settings dialog for sync and the tab bars are not done (out of scope for this wave). The rules/filter buttons open the existing Session Settings (rules) window.

## Performance A/B (A = `716d123` worktree build, B = this wave; 1 round each)

Inputs are the same as Wave 8 (`target/table-wave7/audit/inputs/`, `$TEMP/matcha-wave3-fixture`). Logs: `evidence/visual-parity-wave9/bench-*.log`.

| Benchmark | A p99 (ms) | B p99 (ms) | Δ |
|---|---|---|---|
| `--bench-dir` | 0.620 | 0.599 | −3.4% |
| `--bench` text | 1.481 | 1.428 | −3.6% |
| `--bench-table` | 1.948 | 1.844 | −5.3% |

All are within 10%, so no extra rounds were needed. Sync view first frame: the old view drew every row through egui widgets in `draw_table`. It now draws only the visible rows through `draw_grid`, the same virtualized path `--bench-dir` measures. The plan is built once by the default Update Both, instead of being rebuilt after the UI switched modes. The B benchmark was built before `f20cdf7`, which only adds menu entries and does not touch any drawing path.

## Gate output

`cargo clippy --workspace --all-targets -- -D warnings` (`evidence/visual-parity-wave9/clippy.log`):

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.37s
exit=0
```

`cargo test --workspace`: exit 0, 81 test targets, **451 passed / 0 failed / 5 ignored** (`evidence/visual-parity-wave9/test-summary.log`).

`rustfmt --check` shows no diff on touched files: `dir_view.rs`, `sync_view.rs`, `app.rs`, `snapshot.rs`.

Snapshots (`evidence/visual-parity-wave9/snap-verify.log`): 17/17 `verified=True`; the 16 existing scenes match Wave 8; new: `folder-sync 86539D1B`.

## Compliance

The reference tool was not launched and only offline reference images were used. ObsidianVault was not read. `tokens.json` and `compare.ps1` are unchanged. No reference icon resources were extracted. The reference tool's name does not appear in the product. There are no new dependencies and the engine crates are untouched.
