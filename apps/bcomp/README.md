# bcomp (binaries: `matcha-comp`, `matcha-compc`)

Dual launcher (FR-S2). One crate, one shared `launch` function, two executables:

| binary | shipped as (Windows) | subsystem | behaviour |
|---|---|---|---|
| `matcha-comp` | `matcha-comp.exe` | GUI | starts the main program (or hands off to the running instance) and returns immediately – for VCS integrations |
| `matcha-compc` | `matcha.com` | console | waits for the comparison/script to finish and returns its exit code – for the command line, CI and batch files |

`cargo xtask dist` ships the console launcher as `matcha.com` next to the GUI-subsystem
`matcha.exe`: typing `matcha …` in cmd/PowerShell resolves to the `.com` (`PATHEXT` lists
`.COM` before `.EXE`), double-clicking `matcha.exe` opens the GUI without a console. On
macOS the launchers live in `Matcha.app/Contents/MacOS/` and are reached through
`bin/` symlinks. See `docs/decisions/301-windows-subsystem-split.md`.

The crate directory keeps the spec's internal name; the executables do not use the
reference product's name.

## Behaviour

- Locate `matcha` next to the launcher (symlinks resolved), then on `PATH`, skipping the
  launcher itself; exit `103` when missing.
- Forward all arguments unchanged. The waiting launcher sets `bc_instance::WAIT_ENV`; the
  main program then blocks until the comparison closes and exits with its code, which the
  launcher propagates (`102` when the wait is lost).
- Merge invocations always wait in `matcha` itself, even without the launcher's wait marker;
  `matcha-comp` returns immediately and must not be configured as a merge tool.
- `/?` is answered by the waiting launcher itself, named as invoked (`matcha` when shipped
  as `matcha.com`).

## Dependency boundary

`bc-script` (exit codes, usage text) and `bc-instance` (wait marker). No GUI dependency so
that the console launcher works on a headless build agent.
