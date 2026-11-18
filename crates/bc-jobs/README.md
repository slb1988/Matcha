# bc-jobs

Background job framework: the `Job` trait, cooperative cancellation, progress reporting and
`Worker<T>`, which runs one job on its own `std` thread owned by the code that started it.
All I/O and comparison work in the application runs as jobs so the UI thread only renders
and edits (spec §7). There is no shared pool or scheduler – see
[ADR 008](../../docs/decisions/008-background-jobs-per-view-thread.md).

## Responsibilities

- `Job` trait (`name`, declarative `priority`, `run(self: Box<Self>, &JobContext)`).
- `JobContext` with `CancelToken`, `ProgressSink`, optional `Interaction`
  (confirmation prompts; `None` under `/silent`) and `Confirmation` (prompt / yes-to-all /
  no-to-all – the `option confirm:` values).
- `Worker<T>` – `spawn(Box<dyn Job>)`, `poll()` for the typed output (wrong type or a panic
  → `JobError::Failed`), `cancel()`, `progress()` (latest message), and **cancel + join on
  drop** so a job never outlives its owner (views rely on this before saving over or
  unloading a memory-mapped file). Used by the text, hex and table views.

## Dependency boundary

Only `bc-core` (the `CancelCheck` bridge). **No async runtime** (hard constraint). Core does
not depend on jobs. `CancelToken` implements `bc_core::cancel::CancelCheck`; aligners/diff
accept this interface and return `CoreError::Cancelled` without publishing partial maps.

Cancellation integration: `cargo test -p bc-jobs --test alignment_cancel`.
Background benchmark: `cargo run --release -p bc-jobs --example align_bench -- <left> <right>`.

## Reference class map (from `vmt_report.txt`)

| this crate | reference classes |
|---|---|
| `Job` | `TJob`, `TThreadJob`, `TSimpleJob` |
| `JobContext`, `Interaction` | `TJobContext`, `TDirInteractiveContext`, `TJobWaitForBlocker` |
| `Worker` | `TThreadJob` on a dedicated thread |
| not modelled (ADR 008) | `TJobManager`, `TJobThreadPool`, `TJobPriorityQueue`, `TJobResourcePool`, `TJobDependencyArray` |
| UI (not here) | `TJobsCancelDialog` |
| concrete jobs (in engines) | `TVfs*Job`, `TTextMapAlign2Job`, `TTextMapAlign3Job`, `TTableMapAlignJob`, `TDirTreeContentCompareJob`, `TQuickCompareFilesJob`, `TReportJob`, `TOpMoveBothJob` |

## Open questions

- A scheduling layer (priorities, resource limits) once remote file systems or bulk
  reports need it – new ADR; `Worker`'s join-on-drop guarantee must be kept.
