# bcompare (binary: `matcha`)

Main GUI program. The crate directory keeps the spec's internal name; the executable is
`matcha.exe` (no reference-product branding in shipped artefacts).

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
