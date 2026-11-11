# visual-parity Wave 8: table compare main view re-skin (task group 5)

## Summary

- The table compare view now uses the shared classic chrome. That covers the menu bar (Session/File/Edit/Search/View/Tools/Help, transcribed from `inventories.table.menus`) and the large-icon toolbar (Home, Sessions▾, All/Diffs/Same, Minor, Rules, Format▾, Copy, Next Diff/Prev Diff, Swap, Reload). Each side has a 26 px path bar and a 27 px file info row (modified time, bytes, format▾, encoding▾, line endings, delimiter). The grid has a 28 px column header (▼ marks the cursor column, red markers mark difference columns, columns are lettered A/B/C), a 34 px row-number gutter (▶ marks the cursor row, red markers mark difference rows), 18 px rows, `#A0A0A0` grid lines and `#FFE3E3` difference cells. The inactive pane uses `#FCFCFC` / `#B0B0B0` / `#FCE6E6`, all measured from the reference. Below the grid are a 19 px pane status per side, an 81 px Text Details strip (current row of both sides as a two-row mini grid), the 52 px thumbnail column, and a 25 px status bar with a size grip. The old egui top strip, the three button rows and the info text lines are removed. The app no longer draws its own top bar for the table view.
- Window title: `<left file name> <--> <right file name> - Table Compare - Matcha`.
- `compare.ps1` named regions, both scenes: **13/14 pass**. grid.left/right, headers.left/right, paneStatus.left/right and statusbar are exact (0). menubar, toolbar, pathbar, info.left and details are within ±2 px. The automatic FAIL on info.right is flat-band detection noise; manual pixel measurement is below. Color samples: **2/2** exact (tableDiff `#FFE3E3`, tableGrid `#A0A0A0`).
- `snap-matcha.ps1 -All -Verify`: **16/16 verified=True**. The 14 existing scenes are **byte-identical** to the hashes Wave 7 recorded. To get there I fixed a snapshot focus flake that already existed in the baseline (details below).
- `cargo clippy --workspace --all-targets -- -D warnings`: exit 0. `cargo test --workspace`: 81 targets, **448 passed / 0 failed / 5 ignored**. The 124 existing table GUI tests all pass, plus 1 new test. Touched files show no rustfmt diff.
- Performance: the table bench p99 went up about 55% (three rounds). The cause is the reference density: drawn cells per frame went from 406 to 680. Per-cell cost is flat. The coordinator ruled this accepted and recorded as a known difference (option A). Text and folder benches: no real regression.
- Engine crates are untouched, there are no new dependencies, `tokens.json` and `compare.ps1` are unchanged, and the sync view is untouched.

## Commits

| Commit | Content |
|---|---|
| `ee04d77` | feat(gui): table view chrome re-skin (`table/*`, `app.rs` wiring, test drivers moved to the new entry points) |
| `6096a42` | feat(visual): `table-basic` / `table-cursor-on-diff` snapshot scenes, `snap-matcha.ps1` scene list |
| `fd131b2` | perf(gui): grid lines batched per row/column; per-frame menu/label refresh no longer clones rules or paths (pixel output unchanged) |
| `68974d0` | fix(visual): snapshot input pins viewport focus (`focused=false`), removing the caret focus flake |
| this report's commit | report, side-by-side images, evidence |

## Side-by-side images (left: reference, right: Matcha)

- [table-basic](assets/visual-wave8-table-basic-side.png)
- [table-cursor-on-diff](assets/visual-wave8-table-cursor-on-diff-side.png)
- [CJK / long-content self-check](assets/visual-wave8-table-cjk-check.png) (no reference image; see the calibration section)

## Named-region deviations (compare.ps1 automatic scan; edges left/top/right/bottom)

| Region | table-basic | table-cursor-on-diff |
|---|---|---|
| menubar | −2,0,−2,0 | −2,0,−2,0 |
| toolbar | −2,0,0,0 | −2,0,0,0 |
| pathbar.left | 0,0,−2,0 | 0,0,−2,0 |
| pathbar.right | 0,0,2,0 | 0,0,2,0 |
| info.left | 2,0,1,0 | 2,0,1,0 |
| info.right | 8,0,−1,0 **auto FAIL (noise)** | same |
| headers.left / right | 0,0,0,0 | 0,0,0,0 |
| grid.left / right | 0,0,0,0 | 0,0,0,0 |
| paneStatus.left / right | 0,0,0,0 | 0,0,0,0 |
| details | 0,0,2,0 | 0,0,2,0 |
| statusbar | 0,0,0,0 | 0,0,0,0 |

The −2 on menubar/toolbar is the same outer-edge detection noise seen in Wave 2b and Wave 3. **Manual check of info.right:** the info row is a flat `#F0F0F0` band in both images, with no edge at x 712–739. The first dark text pixel of the date is at x=734 in both the reference and Matcha (left side: x=64 in both), and the header top line at y=123 starts at x=722 in both. The 8 px "shift" is the scan latching onto dithering differences inside the flat band (Matcha's frame has isolated `#EFEFEF` dither pixels). It is not a geometry deviation. Raw results: `evidence/visual-parity-wave8/table-*-result.json`.

Grid metrics, measured against the reference: pane left edge x52 / x722; gutter grid line x85 (34 px); column lines x132/165/233 (column widths 47/33/68, matching `tableColumnWidths` [47,33,67] measured as start-to-start); header y123..150 (28 px); row lines y168/186/204/222 (18 px); ▼ is 11×6 px and ▶ is 6×11 px, both `#0078D7`; difference markers are 9×9 white-rimmed squares; the selected cell is `#A8CDF1`; the active pane's focus frame is dotted, the other pane's is solid `#303030`.

## Color check

| Token | Point | Expected | Actual |
|---|---|---|---|
| tableDiff | (130,215) | #FFE3E3 | #FFE3E3 ✓ |
| tableGrid | (132,175) | #A0A0A0 | #A0A0A0 ✓ |

## Snapshot determinism

`evidence/visual-parity-wave8/snap-verify.log` (two consecutive `-All -Verify` runs with identical output):

```text
icon-catalog A65C53F7  chrome-demo 532DC16A  text-basic 563D91C3  folder-basic BCFE37E8
text-diffs-filter 28FCE048  text-cursor-on-diff FD8E1082  folder-expanded 86977D7E
folder-diffs-filter 060B1F8C  text-long-scrolled 2AEED0BE  home 84C22224
text-menu-edit 28E75965  text-find-bar F3937678
dlg-text-session-settings 2A224A7D  dlg-options E9C43E26
table-basic 17F4BF89  table-cursor-on-diff 5E4C0EEF   (new)
```

**Focus flake (already present in the baseline):** whether the OS gives a newly opened snapshot window foreground focus varies from run to run. egui paints the TextEdit caret only when the viewport is focused, so the selected path box sometimes shows a caret and sometimes does not. I ran the baseline build (`14ca2f0` worktree) and this wave's build alternately, 4 times each, on text-basic. Both flipped between `563D91C3` and `3ACED20F`, and `-Verify` occasionally failed. `68974d0` pins `focused=false` in `raw_input_hook` for snapshots only. After that fix the existing scenes reproduce the Wave 7 recorded hashes exactly.

The two table scenes mirror the reference state. The reference shows the CSV header row as data with lettered columns and pairs rows positionally (gamma↔delta as one changed row). Matcha's defaults are Auto header detection plus inferred keys, which would treat row 1 as column names and split gamma and delta into orphans. `snapshot_options()` therefore applies `HeaderMode::Data`, no key columns and `AlignStrategy::None` **in snapshot mode only**. Product defaults are unchanged. This follows the same precedent as Wave 2, which applied `case_unimportant` only in the snapshot preset. Follow-up below.

## Old entry → new entry (migration table)

| Old entry | New entry |
|---|---|
| App top bar "Home" button | Toolbar Home; Session › Close Tab |
| Session menu button: Session Settings / Swap Sides / Reload Files / Recompare Files / Table Compare Info / Compare Files Using Text Compare | Session › Session Settings... (also toolbar Rules and the Format button body) / Swap Sides (toolbar Swap) / Reload Files (F5, toolbar Reload) / Recompare Files (Ctrl+F5) / Table Compare Info (Ctrl+I) / Compare Files Using › Text Compare |
| Export summary (Text) / (CSV) | Session › Table Compare Report... (Text) / Session › Export Summary as CSV... (appended item) |
| File › Left/Right › Open File… / Open Clipboard / Save / Save As… / Export as CSV… | File › Open File... (Ctrl+O) / Open Clipboard / Save File (Ctrl+S) / Save File As... (F12) / Export as CSV... (appended), all acting on the **active side**; per-side entries: path bar browse/history buttons (open), path bar save button, editable path box (Enter loads that side) |
| Edit: Undo / Redo / Modify cell (F2) | Edit › Undo (Ctrl+Z) / Redo (Ctrl+Y) / Modify... (F2) |
| Edit: Copy to Other Side / Copy Row to Other Side | Edit › Copy Cell to Right (Shift+Ctrl+R, toolbar Copy) / Copy to Right (Ctrl+R); labels switch to "Left" automatically when the right side is active |
| Edit: Cut / Copy / Paste / Delete / Insert Row Before/After / Delete Row / Insert Column Before/After / Delete Column / Select All | Edit menu items with the same names |
| Edit: Sort descending / Sort ascending | Edit › Sort Descending / Sort Ascending (appended; reference "Sort..." stays disabled) |
| Edit: Align With — click opposite row / Clear alignment overrides | Edit › Align With... (F7) / Clear Alignment Overrides (appended) |
| Search: six difference/edit navigation items, Find / Replace | Search › Next/Previous Difference Cell (Shift+Ctrl+N/P) / Next/Previous Difference Row (Ctrl+N/P, toolbar Next Diff / Prev Diff) / Next/Previous Edit / Find... (Ctrl+F) / Replace... (Ctrl+H) / Find Next (F3) / Find Previous (Shift+F3) |
| Toolbar ◀ Difference / Difference ▶ | Search › Previous/Next Difference Cell (toolbar Next/Prev Diff now navigate by row, matching the reference) |
| Toolbar "Copy cell → other" | Toolbar Copy |
| Toolbar Settings | Toolbar Rules |
| Toolbar Go To (row/column inputs + Go) | Search › Go To... (Ctrl+G) opens a Go To dialog (Row/Column + Go; Go closes it; values persist) |
| Filter row All / Differences / Same / None | Toolbar All / Diffs / Same (mutually exclusive toggles) + View › Show All/Differences/Same; None → View › Show None (appended) |
| Ignore Unimportant checkbox | Toolbar Minor (toggle) + View › Ignore Unimportant Differences |
| Values / + Formulas / + Number formats | View › Compare Values / Compare Values + Formulas / Compare Values + Formulas + Number Formats (appended), also in the toolbar Format▾ dropdown |
| Hide Same Columns / Over-Under checkboxes | View › Hide Same Columns / Over-Under Layout (Side-by-Side Layout switches back) |
| View menu Toolbar / Row Numbers / Visible Whitespace / Text Details / File Info / Thumbnail / Resize Columns to Fit | View menu items with the same names (Text Details is now on by default, as in the reference) |
| Status line (same/unimportant/important/orphan/column counts · keys) | Status bar: difference row count, current cell status, load time (inventory `different row count / current cell status / Load time`); full statistics and **key-column annotation** moved to Session › Table Compare Info (Ctrl+I) |
| Status line message (progress/errors) | Status bar: progress while busy; any non-routine message shows in the last field |
| File Info line (path · bytes · editable/read-only · modified) | Path bar + info row (bytes, format, encoding, line endings, delimiter; "Read-only" / "Modified" at the reference's later field position) |
| Details panel (full cell text / formula / number format) | 81 px details strip (both sides' current row); the current cell's cached formula / number format are shown after the strip when present |
| Old 20 px thumbnail | 52 px thumbnail column (miniature, click to jump) |
| Sheet tabs | Unchanged (re-skin deferred); shown only with more than one sheet, so single-sheet CSV shows none, matching the reference |

Unmigrated entries: none. Reference inventory items with no Matcha implementation are shown disabled: New Session/Tab/Window, Workspace, Save Session, Locked, Clear Session, Compare Files Using other than Text, Compare Parent Folders (it was disabled before too), Open With, Explorer, Tools, Help. Help uses product-neutral wording ("Project Website", "About Matcha").

## Calibration (CJK / long content)

I opened a CSV containing Chinese text and an overlong description in Matcha (`assets/visual-wave8-table-cjk-check.png`). Rows stay at 18 px, CJK glyphs fit vertically, and overlong text is clipped at the grid line without spilling into the next cell. Width fitting uses "7 px × character count + 12", which is exact for the reference's ASCII corpus. It undercounts double-width CJK characters, so CJK columns are clipped earlier. There is **no reference table screenshot with CJK content**, so following the "no evidence, no change" rule I left this alone and list it as a known difference.

## Known differences

- **Table bench density (coordinator accepted, option A):** `--bench-table` p99 baseline 1.621/1.420/1.417 ms → this wave 2.373/2.217/1.947 ms (three rounds, about +55%); mean 0.73 → 1.16 ms. Drawn cells per frame went from 406 to 680 (+67%), because rows are 18 px instead of 22 px and columns are fitted to the reference widths. Per-cell cost is about 1.8 µs before and after. A frame without the grids costs 0.27 ms. Absolute values stay far below the 16 ms frame budget and the 4 ms table gate. `fd131b2` already batches grid lines and removes per-frame clones.
- The menu bar has no "New version available..." update link (same as Waves 2b and 3).
- The path bar shows the local corpus path (the reference shows a path on another machine), so the path selection length differs. The path box right-hand icons use the shared chrome pathbar (original icons, not identical to the reference's three icons).
- The info row shows nothing after the delimiter; the reference's truncated "Colu…"/"Sy…" fields have no counterpart in Matcha. The format▾ and encoding▾ entries open Session Settings, which is where the table view already switches format and encoding.
- The reference draws a 2 px double line below row 1 (y167–168). Matcha draws 1 px (within the grid region, under 1 px).
- Header column names: when a sheet has a header row (Matcha's default Auto detection), the source column names are shown instead of letters. The key marker ◆ is shown only when the keys are a proper subset of the columns.
- The status bar shows the reference filter legend ("Shows same, left orphan, right orphan and difference rows.") until the user places a cursor. The legend wording for the Differences/Same/None filters has no reference screenshot and was written by me.
- Thumbnail and details are approximations: the miniature is one line per row; the details strip's horizontal scroll band is a static placeholder ("—").
- egui grayscale AA versus GDI ClearType glyph intensity differences are not counted (HANDOFF §8).
- CJK column-width fitting (see calibration above).

## Follow-ups (outside visual scope)

- Default table rules versus the reference: in the reference the CSV header row appears as data and rows pair positionally without inferred keys. Matcha's defaults are Auto header detection and inferred keys. Verify the reference defaults offline before changing behavior (same kind of item as HANDOFF §5).
- Table Session Settings dialog (to be connected to `chrome/dialog.rs`) and sheet-tab re-skin: deferred.

## Performance A/B (A = `14ca2f0` worktree build, B = this wave)

Inputs: `target/table-wave7/audit/inputs/` (table 69 MB × 2, text 110 MB × 2), `--bench-dir` uses `$TEMP/matcha-wave3-fixture`. Logs: `evidence/visual-parity-wave8/bench-*.log`.

| Benchmark | A p99 (ms) | B p99 (ms) | Δ | Note |
|---|---|---|---|---|
| `--bench-table` r1/r2/r3 | 1.621 / 1.420 / 1.417 | 2.373 / 2.217 / 1.947 | about +55% | density; cells 406→680; accepted |
| `--bench` text | 1.566 | 1.580 | +0.9% | |
| `--bench-dir` r1/r2/r3 | 0.613 / 0.753 / 0.640 | 0.678 / 0.681 / 0.625 | median 0.640→0.678 | round 1 was +10.6%, so I ran 3 rounds; the intervals overlap and this is noise (the folder view was not changed) |

The text and folder B rounds used the build before `fd131b2`/`68974d0`; neither commit touches those paths.

## Gate output

`cargo clippy --workspace --all-targets -- -D warnings` (`evidence/visual-parity-wave8/clippy.log`):

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s
exit=0
```

`cargo test --workspace`: exit 0, 81 test targets, **448 passed / 0 failed / 5 ignored** (`evidence/visual-parity-wave8/test-summary.log`). The 124 existing table tests pass; the new test is `reference_layout_geometry_title_and_menu_inventory` (table-basic region geometry, window title, menu inventory matching tokens). Test drivers moved to the new entries; covered behavior was kept. A shared `legacy_path` helper resolves the old button labels to their new menu/toolbar paths. The "status bar names key columns" assertion became "Table Compare Info names key columns". The details-panel assertion became the two-row details strip. The row-difference-dot assertion now counts gutter markers. The virtualization bound went from `cells_drawn < 600` to `< 1200`, because narrower columns show more cells; 100,000 cells are still virtualized.

`rustfmt --check` shows no diff on touched files: `table/{chrome_ui,menu,grid,header,details,view,commands,cache,mod,tests}.rs`, `table/{command,copy,edit,lazy,navigation,projection,search,session,sort,view}_tests.rs`, `app.rs`, `snapshot.rs`.

## Compliance

The reference tool was not launched and only offline reference images were used. ObsidianVault was not read. `tokens.json` and `compare.ps1` are unchanged. No reference icon resources were extracted; icons are the original `chrome/icons.rs`. The reference tool's name does not appear in the product. There are no new dependencies and the engine crates are untouched.
