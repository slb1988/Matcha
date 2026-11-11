# bc-script

Scripting engine, command-line parsing and the exit-code table (FR-S1, FR-S2). Shares the
internal command API with the GUI so scripted and interactive behaviour match. Parsing
and execution bodies are `todo!()`; the tables are complete.

## Responsibilities

- `exit_code::ExitCode` – 15 codes exactly as documented:
  `0 1 2 11 12 13 14 100 101 102 103 104 105 106 107` with descriptions and reverse lookup.
  `104` (trial expired) is present for table fidelity only and is never produced.
- `command::CommandKind` / `Command` – every script command with typed arguments
  (`criteria`, `filter`, `select` masks, `copyto path:` modes, `sync update|mirror`,
  `snapshot` flags, report arguments via `bc_report::ReportRequest`).
- `parser` – script file syntax (comments, `&` continuation, quoting, `%n` substitution).
- `cli::Switch` – all switches: `/? /h /help /automerge /center= /closescript /edit
  /expandall /favorleft /favorright /filters= /force /fv= /iu /mergeoutput= /nobackups
  /qc[=] /reviewconflicts /ro /ro1 /ro2 (/readonly /lro /rro) /savetarget= /silent /solo
  /sync /title1..4= (/lefttitle…) /vcs1..4=`; `cli::Invocation` – positional inference.
- `runner::ScriptRunner` – sequential interpreter; `quick_compare` (`/qc`) and
  `auto_merge_exit_code` (`/automerge` → 0/14/101, implemented).

## Dependency boundary

Everything below the GUI: `bc-core`, `bc-formats`, `bc-vfs`, `bc-jobs`, the four engines,
`bc-session`, `bc-report`. **Never `bc-gui`** – the console launcher path must work
without a window.

## Exit codes (verbatim from the command-line reference)

| code | meaning | variant |
|---|---|---|
| 0 | Success | `Success` |
| 1 | Binary same | `BinarySame` |
| 2 | Rules-based same | `RulesBasedSame` |
| 11 | Binary differences | `BinaryDifferent` |
| 12 | Similar | `Similar` |
| 13 | Rules-based differences | `RulesBasedDifferent` |
| 14 | Conflicts detected | `ConflictsDetected` |
| 100 | Unknown error | `UnknownError` |
| 101 | Conflicts detected, merge output not written | `ConflictsMergeNotWritten` |
| 102 | Launcher unable to wait for main program | `LauncherCannotWait` |
| 103 | Launcher cannot find main program | `LauncherCannotFindMain` |
| 104 | Trial period expired (never produced here) | `TrialExpired` |
| 105 | Error loading script | `ScriptLoadError` |
| 106 | Script syntax error | `ScriptSyntaxError` |
| 107 | Script failed to load file or folder | `ScriptLoadFileOrFolderFailed` |

## Reference class map (from `vmt_report.txt` / unit names)

| this crate | reference |
|---|---|
| `cli` | `SsCmdLine` unit, `TBcApplication` argument handling |
| `exit_code` | `wBcExitCode` (binary profile) |
| `runner` | `TDIRSCRIPTDIALOG`, `TScriptDialogOpeningEventArgs`, `TCommands`, `TBcCommands`, `TSessionCommands`, `TViewCommands`, `TCommandsManager`, `TCommandsClassList` |
| `runner::quick_compare` | `TQuickCompareFilesJob` |

The script engine has almost no dedicated RTTI classes in the binary (it is procedural
Delphi code), so behaviour is taken from the help topics `scripting_reference.html`
and `command_line_reference.html`.

## Open questions

- Notes said 28 commands; the reference list contains 29 (`CommandKind::ALL`). Decide
  whether the note should be corrected.
- `touch` with an explicit timestamp and `filter cutoff:` dates depend on the OS regional
  format per the reference; a lenient parser plus ISO-8601 is the planned minimal choice.
- `/qc` (implemented, `runner::quick_compare`): default type is "rules-based" per the
  reference and yields 2/12/13; `/qc=binary|crc|size` yields 1/11. Choices made here:
  line-ending-only differences are "rules-based same" (2); binary input in rules mode
  reports 11; the result description is printed as one stdout line unless `/silent`
  (the reference wording for this is not documented); folders and missing files are 100.
  12 ("Similar") needs rules that mark something unimportant – the defaults mark nothing.
