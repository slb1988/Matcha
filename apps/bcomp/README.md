# bcomp (binaries: `matcha-comp`, `matcha-compc`)

Dual launcher (FR-S2). One crate, one shared `launch` function, two executables:

| binary | reference equivalent | behaviour |
|---|---|---|
| `matcha-comp` | GUI launcher (`.exe`) | starts the main program (or hands off to the running instance) and returns immediately – for VCS integrations |
| `matcha-compc` | console launcher (`.com`) | requires a console, waits for the comparison/script to finish and returns its exit code – for CI and batch files |

The crate directory keeps the spec's internal name; the executables do not use the
reference product's name.

## Scaffold behaviour

`locate_main_executable` and `launch` are `todo!()`; running either binary panics.
Exit codes `102` (cannot wait) and `103` (cannot find main) are reserved for the real
implementation (`bc_script::ExitCode`).

## Dependency boundary

`bc-script` only (for `ExitCode` and, later, usage text). No GUI dependency so that
`matcha-compc` works on a headless build agent.

## Open questions

- Wait mechanism: the reference passes a handle/event to the main instance; options here
  are an inherited event handle, a named pipe, or waiting on a child process when a new
  instance was started. Must also work when the comparison is forwarded to an already
  running instance.
- Windows cannot produce a `.com`-style dual binary from one crate; two differently named
  executables are the chosen equivalent.
