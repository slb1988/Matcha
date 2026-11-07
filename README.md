# Matcha

A Windows-first desktop comparison tool written in Rust: text, table, hex, picture and
folder compare/merge/sync, behaviourally aligned with a well-known commercial reference
(version 5.0.6) whose internals were recovered by static analysis. The goal of the first
three milestones is **1:1 behaviour** — same difference colouring semantics, same session
concepts, compatible session/format XML files, identical CLI exit codes and script
commands — followed by extensions.

Status: **scaffold** (2026-11-05). Crate boundaries, traits, data models, the built-in
format library, the exit-code/command/switch tables and an egui prototype are in place;
every algorithm is a `todo!()`.

Current table-memory work: [session handoff](docs/worker-reports/table-memory-handoff.md)
and [measurement details](tools/demand_proto.md). The demand-backed GUI path is experimental,
default-off, and has **not passed the GUI ready-time gate**; the scaffold description above is historical.

## Project goals and non-goals

- Goal (M1–M3): Text Compare/Merge/Edit, Folder Compare/Sync, Table Compare (CSV), script
  engine with the full command set and exit-code table, session files readable by and
  from the reference tool.
- Non-goals inside M1–M3: remote VFS protocols, SCC integration, shell extension,
  printing, licensing.
- Later (P2–P4): Hex/Picture/Version/Registry/Media compare, snapshots, settings packages,
  VFS providers.

The authoritative specification lives in the project vault
(`luckey/03_projects/bcompare-clone/bc5-clone-spec.md`, "framework and feature
requirements"); milestone and FR numbering in crate docs refer to it. Milestones:

| milestone | content |
|---|---|
| M1 | P0: FR-T1…T6 text compare, FR-D1…D3 folder compare, FR-G1 single-instance GUI, FR-X1 session XML |
| M2 | P1: FR-T7 3-way merge, FR-T8 alignment settings, FR-D4/D5 sync + background compare, FR-S1/S2 script engine + launchers, FR-R1 reports, FR-C1 CSV table compare |
| M3+ | P2–P4 per the feature inventory |

## Red lines

- **No licensing, trial, registration or key handling of any kind.** The exit code `104`
  exists in the enumeration only because the reference table lists it; nothing produces it.
- **No use of the reference product's name, abbreviations or icons in anything shipped.**
  Executables are `matcha`, `matcha-comp`, `matcha-compc`. The `bc-` crate prefix and
  the class names quoted in doc comments are internal engineering references to the
  analysis notes, not product branding. The crate directories `apps/bcompare` and
  `apps/bcomp` keep the spec's internal names; their binaries do not.
- The analysis inputs in the vault are read-only; this repository only carries a copy of
  the built-in format library resource (`crates/bc-formats/data/bcfileformats.xml`).

## Workspace layout

```
Cargo.toml                 workspace (resolver 2, edition 2021)
crates/
  bc-core/                 line model, diff/align traits, undo stack, hashing, encoding, Delphi-XML persistence
  bc-formats/              format registry; build.rs converts bcfileformats.xml → embedded JSON
  bc-vfs/                  Vfs trait + LocalFs skeleton + VfsManager
  bc-jobs/                 Job/JobManager/ThreadPool/ResourcePool (std::thread + crossbeam)
  bc-engine-text/          TextMap, rules, importance, inline diff, merge, align jobs
  bc-engine-dir/           DirTree, criteria, filters, sync plan, file ops, jobs
  bc-engine-table/         TableModel, KeyAlign, TableRules
  bc-engine-bin/           hex / picture / version / registry / media models
  bc-session/              Session/SessionKind/Workspace + BCSessions.xml layer
  bc-report/               report request model + renderers
  bc-script/               ExitCode, Command (29), script parser, CLI switches, runner
  bc-gui/                  eframe shell: Home placeholder + dual-pane prototype + benchmark
apps/
  bcompare/                bin `matcha` (main GUI program)
  bcomp/                   bins `matcha-comp` (return immediately) and `matcha-compc` (wait for exit code)
docs/
  ui-framework-decision.md egui prototype measurements and framework verdict
```

Dependency direction is strictly downward and acyclic:

```
apps → bc-gui / bc-script → bc-report → bc-session → bc-engine-{text,dir,table,bin}
     → bc-formats / bc-vfs / bc-jobs → bc-core
```

`bc-engine-dir` additionally uses `bc-engine-text` (rules-based content compare).
No engine crate depends on `bc-gui`; `bc-gui` is the only crate with a GUI dependency.
`bc-jobs` uses only `std::thread` and `crossbeam-channel` (no async runtime).

## Build, test, run

Requirements: stable Rust ≥ 1.88 (calamine 0.38 requires 1.88; developed with 1.99), Windows 10/11 (the engine crates
are platform-neutral; only `bc-gui` needs a display).

```powershell
cargo check --workspace --all-targets     # all 14 crates
cargo clippy --workspace --all-targets    # warning-free at scaffold time
cargo test --workspace                    # one real test: bc-formats built-in library
cargo run --release -p bcompare -- <left> <right>             # dual-pane prototype
cargo run --release -p bcompare -- <left> <right> --bench=600 # scroll benchmark
```

`matcha-comp` / `matcha-compc` build but their launcher logic is `todo!()` and panics
when run (scaffold).

## Discrepancies found while scaffolding

Recorded here per the hand-over rule "notes conflict → log it, choose the minimal
implementation"; the spec wins over notes, and the extracted data wins over both.

| topic | notes say | data/reference says | what the scaffold does |
|---|---|---|---|
| built-in formats | 22 (19 text + 2 table + 1 picture) | the resource has **25** top-level entries: + `TMediaFormat MEDIA`, `TVerFormat EXE`, `TXMLFormat XML` | embeds all 25; unit test asserts the documented 22 by name/kind and the 25 total |
| script commands | "28 commands" | the reference's alphabetical list has **29** entries (`ATTRIB … VERSION-REPORT`) | `CommandKind` has 29 variants (+ `data-report` alias of `table-report`) |
| grammar match types | spec: `Delimited, Columns, List, RegEx` | classes: `TBasicTextGrammarItem` (Text + IsRegExp), `TDelimited…`, `TList…`, `TColumn…` | `GrammarMatch::{Basic, Delimited, List, Columns}`; "RegEx" = `Basic{is_regexp:true}`; `is_regex()` helper |
| format names | implied unique | `XML` exists twice (`TXMLFormat` and `TTextFormat`) | registry keys by `(kind, name)` |
| STX marker | "strings carry `&#2;`" | `ElementStr`/names carry it, `Text`/`Mask`/`FindText` do not | reader records `Node::stx`; the writer rule is an open question (see bc-formats README) |

## Open questions / follow-ups

See the "遗留问题" list in the hand-over summary and each crate README's *Open questions*
section. The biggest items: Smart-alignment scoring (spec §8), STX write rule, single
instance hand-off protocol, launcher wait mechanism, Windows recycle-bin/attribute APIs
without extra dependencies.

## Licence

Not yet chosen (project owner's decision). Until then: all rights reserved.
