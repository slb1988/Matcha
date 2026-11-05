# bc-jobs

Background job framework: cooperative cancellation, progress, dependencies, priority
bands, worker threads and capacity-limited resources. All I/O and comparison work in
the application runs as jobs so the UI thread only renders and edits (spec §7).

## Responsibilities

- `Job` trait (`name`, `priority`, `resources`, `run(self: Box<Self>, &JobContext)`).
- `JobContext` with `CancelToken` (implemented), `ProgressSink`, optional `Interaction`
  (confirmation prompts; `None` under `/silent`) and `Confirmation` (prompt / yes-to-all /
  no-to-all – the `option confirm:` values).
- `JobManager` / `JobHandle` – submit, dependencies, cancel, wait, inline execution for the
  sequential script runner.
- `pool::ThreadPool` – `std::thread` workers over per-priority crossbeam channels.
- `resource::ResourcePool` / `ResourceLease` – serialise access to connections/disks.

Everything except `CancelToken`, `ResourcePool::declare` and id allocation is `todo!()`.

## Dependency boundary

`crossbeam-channel` and downward-only `bc-core` (the `CancelCheck` bridge).
**No async runtime** (hard constraint). Core does not depend on jobs.
`CancelToken` implements `bc_core::cancel::CancelCheck`; aligners/diff accept this
interface and return `CoreError::Cancelled` without publishing partial maps.

Cancellation integration: `cargo test -p bc-jobs --test alignment_cancel`.
Background benchmark: `cargo run --release -p bc-jobs --example align_bench -- <left> <right>`.
The benchmark uses scoped std threads, not JobManager (which remains a scaffold).

## Reference class map (from `vmt_report.txt`)

| this crate | reference classes |
|---|---|
| `Job` | `TJob`, `TThreadJob`, `TSimpleJob`, `TResourceJob` |
| `JobContext`, `Interaction` | `TJobContext`, `TDirInteractiveContext`, `TJobWaitForBlocker` |
| `JobManager`, `JobHandle` | `TJobManager` (`TAsyncThreadSynchronizer`), `TJobArray`, `TJobDependencyArray` |
| `pool::ThreadPool` | `TJobThreadPool`, `TJobWorkerThread`, `TJobPriorityQueue` (`TSsPriorityQueue`) |
| `resource::ResourcePool` | `TJobResourcePool`, `TJobResource`, `TJobResourceThread` |
| UI (not here) | `TJobsCancelDialog` |
| concrete jobs (in engines) | `TVfs*Job`, `TTextMapAlign2Job`, `TTextMapAlign3Job`, `TTableMapAlignJob`, `TDirTreeContentCompareJob`, `TQuickCompareFilesJob`, `TReportJob`, `TOpMoveBothJob` |

## Open questions

- Completion marshalling to the UI thread: eframe can be woken with
  `Context::request_repaint`; decide whether `JobManager` owns a waker callback or the GUI
  polls `JobHandle::completion()` channels each frame.
- Whether `Priority::Interactive` reserves a dedicated worker (config flag exists).
