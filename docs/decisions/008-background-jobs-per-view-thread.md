# ADR 008：后台任务每个 job 一个线程，由发起方持有；不建共享线程池

## 背景

`bc-jobs` 起初按参考工具的类层次搭了脚手架：`JobManager`（提交、依赖、优先级队列、等待、全部取消）、
`pool::ThreadPool`（按优先级的 crossbeam 通道 + 工作线程）、`resource::ResourcePool`（连接/磁盘租约）。
这些函数体一直是 `todo!()`，到 2026-11-18 没有任何调用方。

实际落地的后台工作：

- 文本 / 十六进制 / 表格视图各有一份几乎相同的 `worker.rs`（约 70 行）：为一个 `Job` 开一个
  `std::thread`，`downcast` 输出，记录最后一条进度消息，每帧 `poll`，drop 时取消并 join。
- 文件夹视图有自己的长驻 worker 线程（命令通道 + 结果通道），与上面的形态不同。
- 引擎内部的并行（表格对齐、字节对齐、推断）用 `std::thread::scope`，作用域内自管。

## 决定

1. 新增 `bc_jobs::Worker<T>`：一个 job 一个线程，发起方（视图）持有；`poll` 取结果、`cancel`、
   `progress`、drop 时取消并 **join**。三个视图的 `worker.rs` 改为它的类型别名，行为不变。
2. 删除未实现的 `JobManager` / `ThreadPool` / `ResourcePool`、`Job::resources`、`JobStatus`、
   `JobError::DependencyFailed` 与 `crossbeam-channel` 依赖。`Priority` 保留为声明性信息
   （引擎的 job 已在声明），当前不参与调度。
3. 文件夹视图的长驻 worker 与引擎内的作用域并行保持原样。

## 理由

- **join-on-drop 是硬约束**：视图保存覆盖文件或卸载前必须等对齐 job 结束，否则后台仍持有该文件的
  mmap 快照（Windows 上会阻止替换文件）。专属线程能在 drop 时同步 join；共享池里的任务没有这种所有权，
  要另造"等某个任务结束"的机制，复杂度与死锁风险都更高。
- **并发量很小**：同一时刻只有活动标签页与少数后台标签页在算，每个视图最多一个对齐 job；
  线程创建开销相对一次比较可以忽略，不存在需要池化的大量短任务。
- **没有需要依赖调度或资源租约的场景**：远程 VFS（FTP/SFTP 连接上限）尚未开始，届时再按真实需求设计，
  而不是先维护一套没人用、语义未经验证的接口。
- 消除三份复制代码，同时去掉 `todo!()` 脚手架带来的"看似已有调度器"的误导。

## 后果

- 新的后台 job：实现 `Job`，用 `bc_jobs::Worker::<输出类型>::spawn(Box::new(job))` 启动，在 UI 帧里 `poll`。
- 将来若出现大量并发任务或需要资源上限（远程文件系统、批量报告），另立 ADR 引入调度层；
  `Worker` 的 drop 语义需保留或给出等价保证。
