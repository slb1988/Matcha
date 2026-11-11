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

`/qc`, `/report=…` and `@script` run in the starting process before any GUI or
single-instance logic (`routing::Route::{QuickCompare, Report, Script}`), so a running
window never receives them; a running instance refuses them if they are handed off anyway.
Reports go to standard output or a file, errors to standard error. When the process has no
console at all (Explorer, shortcut) errors are collected into one message box, and a report
for standard output fails before any command runs.

## Responsibilities (target)

- Parse the full command line via `bc_script::cli::CliArgs` (switches + positional
  inference).
- Single instance (FR-G1): a second start hands its arguments to the running instance
  and exits, unless `/solo`.
- Start `bc_gui::run` with the inferred session; `/qc`, `/automerge` and `@script` paths
  run through `bc-script` and return the documented exit codes.

## Scaffold behaviour

`matcha [<left> <right>] [--bench[=<frames>]]` – opens the dual-pane prototype or the
Home placeholder; `--bench` runs the scroll benchmark and prints `bench.*` lines.
The real switch parser is `todo!()` in `bc-script`; `main.rs` documents the hand-over.

## Dependency boundary

`bc-gui`, `bc-script`, `bc-session`.

## Reference map

| this crate | reference |
|---|---|
| `matcha.exe` | main program executable (`TBcApplication`, single instance with argument forwarding) |
| single-instance hand-off | mutex + window message / named pipe in the reference; protocol to be designed |

## Open questions

- Single-instance transport on Windows (named pipe vs. `WM_COPYDATA`) and how the console
  launcher learns the final exit code (shared with `apps/bcomp`).
