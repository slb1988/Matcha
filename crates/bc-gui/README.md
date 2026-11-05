# bc-gui

eframe/egui shell. Scaffold deliverable: Home placeholder page, dual-pane text view
prototype and a scroll benchmark that validated the framework on 110 MB inputs.
Folder Compare / Folder Sync now have worker-backed virtual views (Wave C).

## Responsibilities

- `app::MatchaApp` – top bar, status bar (update ms / frame CPU ms / rows drawn), view
  switching, benchmark driver.
- `home::HomeView` – placeholder with two path fields and an Open button.
- `text_view::LoadedText` – mmap + `u32` line-start index; `TextCompareView` – two
  `ScrollArea::show_rows` panes with locked scroll offsets, line-number gutter,
  per-line render cap.
- `bench::BenchState` – unchanged text benchmark.
- `dir_view` – single worker owns DirTree; latest-only immutable row snapshots, side-aware
  selection, column sorting/details, filters, navigation, comparison, operations and refresh.
- `sync_view` – preset/custom methods, preview actions, per-item overrides and conflicts;
  successful runs maintain a per-open-view baseline, never advance unresolved conflicts.
- `dialogs/*`, `filters_bar`, `legend`, `log_panel` – shared path/operation/confirmation
  controls, presets, colors and virtualized operation log.
- `bench_dir` – real native-window benchmark and framebuffer evidence (`bench.dir.*`).

Run: `cargo run --release -p bcompare -- <left> <right> [--bench[=frames]]`.
Two folders automatically select Folder Compare; Home also accepts two dropped folders.
`cargo run --release -p bcompare -- --bench-dir <left-folder> <right-folder>` measures
600 frames after warm-up and closes the window. `tools/golden/bench_dir.ps1` builds,
generates two 100,000-file temporary trees, verifies p99 <4 ms and captures the framebuffer.

Folder UI tests: `cargo test -p bc-gui --lib`.
Presets: `Matcha/folder-filters.xml` under APPDATA (or MATCHA_SETTINGS_DIR override).
File operations execute only after explicit scope review plus class-specific confirmations.
The text child view reuses the existing read-only prototype; no text/bench implementation
was changed. F12 captures this viewport to a diagnostic BMP in the system temp directory,
without desktop capture or other windows. See
[ADR 104](../../docs/decisions/104-folder-ui-and-virtual-pairs.md) and the Wave C report.

The current environment cannot deliver physical desktop input. Native framebuffer visual
checks and real egui/worker tests are the approved equivalent; physical input remains a
separately marked owner checklist, not a claimed manual pass.

## Dependency boundary

The **only** crate allowed to depend on a GUI toolkit (`eframe`, default features off:
`glow`, `default_fonts`, `wayland`, `x11`) plus `memmap2`. Depends on `bc-core`,
`bc-formats`, `bc-vfs`, `bc-jobs`, `bc-engine-text`, `bc-engine-dir`, `bc-session` so the
real views can be wired without new edges. Engine crates never depend on this crate.
One audited `unsafe` block (memory-mapping a read-only file).

## Reference class map (from `vmt_report.txt`)

| this crate | reference classes |
|---|---|
| `MatchaApp` | `TBcApplication`, `TBcView`, `TEmptyView`, `TView` |
| `HomeView` | `THomeView`, `THomeState`, `THomeActions`, `THomeCommands` |
| `TextCompareView` (prototype) | `TTextView`, `TTextCompareView`, `TTextViewState`, `TTextCompareState`, `TFileView`, `TFileViewState` |
| text widget (next task) | `TUiTextEditor`, `TTextEditor`, `TTextDetailsEditor`, `TUiTextBody`, `TUiTextGutter`, `TUiTextRuler`, `TUiTextNav`, `TUiTextSel`, `TUiTextChild`, `TUiTextBaseChild` |
| folder tree (next task) | `TUiMapTreeView`, `TUiCustomMapTreeView`, `TUiMapTreeViewHeader`, `TUiSimpleTreeView`, `TDirTreeView`, `TDirBaseTreeView`, `TDirSyncTreeView` |
| other views (later) | `TTextMergeView`, `TTextEditView`, `TTextPatchView`, `TTableView`, `TTableCompareView`, `THexView`, `THexCompareView`, `TPixView`, `TPixCompareView`, `TDirView`, `TDirCompareView`, `TDirMergeView`, `TDirSyncView`, `TUiHexEditor`, `TUiTableEditor` |

## Open questions

- Custom editable text widget design (caret, selection, IME) – the main GUI risk noted in
  the framework decision.
- Theme: the reference ships a dark VCL style (`BCDarkTheme.vsf`); egui's visuals are
  the starting point, colours come from `bc_session::prefs::DiffColors`.
- Native file dialogs without extra dependencies (the prototype uses text fields).
