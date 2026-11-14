# bcompare (binary: `matcha`)

Main GUI program. The crate directory keeps the spec's internal name; the executable is
`matcha.exe` (no reference-product branding in shipped artefacts).

## Subsystem and output

On Windows `matcha.exe` is a GUI-subsystem program: double-click, Explorer menu and
shortcuts open the window without a console. Command-line use goes through the console
launcher shipped beside it as `matcha.com` (see `apps/bcomp`). `src/console.rs` attaches to
the launcher's console when it is waiting (and for dev/perf runs), keeps redirected handles
as they are, and falls back to a message box for user-facing errors when there is no
console at all. Decision record: `docs/decisions/301-windows-subsystem-split.md`.

## Headless commands

`/qc`, `/report=…`, `/automerge` and `@script` run in the starting process before any GUI or
single-instance logic (`routing::Route::{QuickCompare, Report, AutoMerge, Script}`), so a running
window never receives them; a running instance refuses them if they are handed off anyway.
Reports go to standard output or a file, errors to standard error. When the process has no
console at all (Explorer, shortcut) errors are collected into one message box, and a report
for standard output fails before any command runs.
`/automerge` prints one result line unless `/silent` or no standard output is available;
results never fall back to a message box. With `/reviewconflicts`, conflict-free output is
written without a window; conflicts leave the output untouched and enter the GUI merge
path instead (`/reviewconflicts` overrides `/force`, but is disabled by `/silent`).

## Responsibilities

- Parse the full command line via `bc_script::cli::CliArgs` (switches + positional
  inference).
- Single instance (FR-G1): a second start hands its arguments to the running instance,
  unless `/solo`; waiting requests return when their tab closes.
- Merge routing: `Route::AutoMerge` selects headless execution; `routing::merge_launch`
  builds `bc_gui::merge::MergeLaunch` from normalized paths, titles and applicable rules.
  `routing::review_args` removes `/automerge` before a conflict-review GUI hand-off so the
  running instance does not reject it as a headless request.
- Merge invocations always wait in `matcha.exe` itself, with or without the console
  launcher: a hand-off sets `Request.wait` to the launcher wait marker **or** a merge
  invocation and receives the tab result through `remote::Outcome::Closed(i32)`.
- Start `bc_gui::run_with_remote` with the inferred session; its startup-tab result becomes
  the process exit code, including the first-instance and `/solo` paths. For a GUI merge,
  0 means the last save had no unresolved conflicts, 14 means the last save still had
  unresolved conflicts, 101 means never saved, and 100 means the merge view could not open.
  `/qc`, `/automerge` and `@script` run through `bc-script` and return their documented codes.

## GUI invocations

`matcha [<left> <right>] [--bench[=<frames>]]` opens a comparison or Home;
`--bench` runs the scroll benchmark and prints `bench.*` lines.
`matcha <left> <right> <center> [<output>]`, or
`matcha /center=<file> [/mergeoutput=<path>] <left> <right>`, opens Text Merge;
`/mergeoutput=` overrides the fourth positional path. Merge calls always use text merge
(CSV/TSV are line-based), not file-format view routing. `main.rs` documents the hand-off.

## Dependency boundary

`bc-gui`, `bc-script`, `bc-session`.

## Reference map

| this crate | reference |
|---|---|
| `matcha.exe` | main program executable (`TBcApplication`, single instance with argument forwarding) |
| single-instance hand-off | `bc-instance` request forwarding; waiting callers receive the tab's completion code |

## Known limitation

- A first-instance or `/solo` process exits only when the whole window closes, even if its
  startup merge tab has already closed; with only that tab, closing it closes the window.
  `matcha-comp.exe` returns immediately and must not be configured as a merge tool; use
  `matcha.exe` for P4V (`%1 %2 %b %r`, optionally prefixed by `/automerge /reviewconflicts`).
