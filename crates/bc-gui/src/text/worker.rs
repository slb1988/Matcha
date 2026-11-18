//! 每视图一个可取消的后台对齐 job（`bc_jobs::Worker`，ADR 008）；保存/卸载前 join，
//! 绝不泄漏 mmap 快照。
pub type Worker = bc_jobs::Worker<bc_engine_text::jobs::AlignOutput>;
