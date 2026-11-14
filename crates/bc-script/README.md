# bc-script

Scripting engine, command-line parsing and the exit-code table (FR-S1, FR-S2). Shares the
internal command API with the GUI so scripted and interactive behaviour match.

## Script support in this build (change `text-patch-reports`)

`@script` runs headless in the starting process. Syntax: one command per line,
case-insensitive keywords and options, `#` comments outside double quotes, trailing `&`
continuation (the logical line keeps its first line number), double quotes group and are
removed. Substitution happens when the script is parsed: `%1`…`%9` are the arguments after
the script name that do not start with `/` (missing → empty), `%date%` (`yyyy-mm-dd`),
`%time%` (regional format), `%fn_time%` (`hh-mm-ss`), `%NAME%` an environment variable
(case-sensitive; left as written when undefined). Script files: BOM → UTF-8/UTF-16,
otherwise UTF-8, or the system ANSI code page when not valid UTF-8.

| command | status |
|---|---|
| `text-report` | `layout:patch` with `patch-normal|patch-context|patch-unified`, `ignore-unimportant`, `title:`; two file names as `<comparison>`; `output-to:<file>` or `output-to:-` (standard output, **Matcha extension**) |
| `log`, `option`, `beep` | as in the reference (`option confirm:` is accepted; nothing asks yet) |
| the other 25 (incl. `file-report`, `load`, `select`, `folder-report`) | recognised, rejected as not supported |

The whole script is parsed before anything runs. Rejections:

| problem | exit |
|---|---|
| script file unreadable | 105 |
| unknown command, invalid argument, missing `layout:`/`output-to:` | 106 |
| reference command/layout/target this build does not run (other layouts, printer, clipboard, `html-*`, session or selection as comparison) | 100 |

A syntax error anywhere wins over an unsupported command. Each rejection names the line.

## Responsibilities

- `exit_code::ExitCode` – 15 codes exactly as documented:
  `0 1 2 11 12 13 14 100 101 102 103 104 105 106 107` with descriptions and reverse lookup.
  `104` (trial expired) is present for table fidelity only and is never produced.
- `command::CommandKind` / `Command` – every script command with typed arguments
  (`criteria`, `filter`, `select` masks, `copyto path:` modes, `sync update|mirror`,
  `snapshot` flags, report arguments via `bc_report::ReportRequest`).
- `parser` – script file syntax (comments, `&` continuation, quoting, substitution) and the
  argument grammar of the supported commands; `ScriptError` carries the line and the kind.
- `platform` – local time, regional time format, ANSI code page, beep (the only module
  allowed to use `unsafe`, via `windows-sys` on Windows).
- `cli::Switch` – all switches: `/? /h /help /automerge /center= /closescript /edit
  /expandall /favorleft /favorright /filters= /force /fv= /iu /mergeoutput= /nobackups
  /qc[=] /reviewconflicts /ro /ro1 /ro2 (/readonly /lro /rro) /savetarget= /silent /solo
  /sync /tablemerge /keys= /title1..4= (/lefttitle…) /vcs1..4=`; `cli::Invocation` –
  positional inference.
- `cli::{MergeSpec, MergeSwitches}` – normalized merge paths and switches;
  `CliArgs::merge_spec` accepts three/four positional paths or two paths plus `/center=`
  (`/mergeoutput=` overrides the fourth path), `CliArgs::merge_switches` collects the merge
  options, and `CliArgs::title` resolves the left/right/center/output pane titles.
- `runner::ScriptRunner` – sequential interpreter; `quick_compare` implements `/qc`.
- `runner::{AutoMergeRequest, AutoMergeReport, auto_merge_files}` – headless text merge;
  `AutoMergeRequest::from_cli` builds the request, `replace_file` writes through a temporary
  file, and `auto_merge_exit_code` maps the engine outcome to 0/14/101.
- `table_merge` – table merges: `merge_kind` (all three inputs xlsx/xlsm by extension or ZIP
  header → table; none → text, or table with `/tablemerge`; mixed → error), `load` (reads the
  three sides like `table_report`; a text table's header mode is decided by the center and
  reused for left/right), `write_output` (xlsx via `bc_engine_table::xlsx_write`, text tables
  written back record-preserving, both through `replace_file`) and `auto_merge_tables`
  (0 written / 101 withheld; the report carries the key columns used).

`/automerge` requires an output target and returns 0 when conflict-free output was written,
14 when conflicts were written with `/force`, 101 when conflicts prevented writing, or 100
for argument, input or write errors (including folders, missing/unreadable/non-text input).
A 101 result leaves an existing output file unchanged.
Writes use a temporary file in the target directory before replacement, allow the output
path to equal an input, never read the previous output content and make no backup.

## Dependency boundary

Everything below the GUI: `bc-core`, `bc-formats`, `bc-vfs`, `bc-jobs`, the four engines,
`bc-session`, `bc-report`; `windows-sys` on Windows (`platform` only). **Never `bc-gui`** – the console launcher path must work
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
