# 决策：只读 mmap 的安全边界与降级

- 状态：本轮已实现；协调者在实施问答中批准。
- 范围：`bc-core::line::LineList::open_mapped`，设计 D6。

## 决定

Windows 以 `OpenOptionsExt::share_mode(FILE_SHARE_READ)` 打开，禁止共享写入和删除；非空文件通过 `memmap2` 只读映射，句柄与映射一起保存在 `Arc` 后端。最后一个引用（包括撤销历史和快照）释放前始终保留句柄，避免常规文件 API 改写或截断映射内容。crate 改为 `deny(unsafe_code)`，唯一豁免是带 SAFETY 注释的 `Mmap::map` 调用。

遇到 Windows 共享冲突（错误 32）时降级为 owned-read；文件为空时也使用 owned 后端。非 Windows 统一 owned-read，不假装具备跨平台锁保护。其他 I/O 错误如权限拒绝或不存在仍正常返回错误。`LineList::backend()` 返回 `Owned` / `Mapped` / `Segmented`，上层状态区可据此显示实际后端（不把存储状态混入字符编码 Detection）。

首次编辑将原块拆为共享前后片段和一个拥有新字节的修改块。行索引按块保存范围，原 mmap 与索引不复制；撤销快照同样共享原块。只在显式 `to_bytes` / 保存时才串行写出内容。

## 保存与外部修改限制

映射存活期间正常外部写入、删除和截断会被 Windows 拒绝。原路径保存之前必须释放所有映射引用（含历史快照），或先另存到其他路径；保存流程由后续引擎/GUI 任务处理。Wave 3 已新增显式 materialize_and_save：MaterializeContext 按共享缓冲身份去重，当前内容、原始内容及 EditStack 的 undo/redo/group 快照都换为 owned，保留历史；转换 context 释放旧映射后再以临时文件原子写回。外部 worker 持有的快照不能强行失效，GUI 保存前必须 cancel/join 相关 job 并释放这些快照；若仍有锁则原文件不被截断，释放后重试或安全另存。Wave 4 已移除原型 GUI 的 mmap：正式视图保存前 cancel/join worker，随后使用同一个 MaterializeContext 调用两侧 TextContent::materialize_owned，再释放 context 并保存目标侧。必须覆盖两侧是因为 Copy Section 的 LineList slice 可能在另一侧当前内容/撤销历史中继续引用来源 mmap；仅物化保存侧不足以解除该锁。另一侧的 dirty/save point 不改变。

## 验证

- `cargo test -p bc-core --test mmap`：原字节与行尾、分段编辑、快照独立性、句柄锁生命周期、现有写句柄触发 owned 降级、空文件和不存在路径。
- `cargo run --release -p bc-core --example line_bench -- <input-file>`：打开与索引耗时，编辑一行且保留撤销快照前后的 Windows 进程私有内存差；只读输入。
- 测量数据见 [Wave 1 报告](../worker-reports/text-compare-core-wave1.md)。
