# bc-gui

eframe/egui shell. Scaffold deliverable: Home placeholder page, dual-pane text view
prototype and a scroll benchmark that validated the framework on 110 MB inputs
(results in `docs/ui-framework-decision.md`).

## Responsibilities

- `app::MatchaApp` – top bar, status bar (update ms / frame CPU ms / rows drawn), view
  switching, benchmark driver.
- `home::HomeView` – placeholder with two path fields and an Open button.
- `text_view::LoadedText` – mmap + `u32` line-start index; `TextCompareView` – two
  `ScrollArea::show_rows` panes with locked scroll offsets, line-number gutter,
  per-line render cap.
- `bench::BenchState` – scroll pattern, per-frame statistics, `bench.*` summary lines.

Run: `cargo run --release -p bcompare -- <left> <right> [--bench[=frames]]`.

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
