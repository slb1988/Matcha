# folder-compare-sync — Wave B 实施报告

## 结果

**任务组 4–5 的 7 项全部完成**：4.1–4.4、5.1–5.3 均已勾选，
OpenSpec apply 为 **23/34**。任务组 6 及以后未开始，剩余 11 项。

本报告覆盖上次的阻塞报告；上次派发确实未实施，不追溯改写其 failed 生命周期。
本次重派已明确批准四项边界，现已按批准方案完成实现与验证。

最终全量验证：
- `cargo clippy --workspace --all-targets -- -D warnings`：退出码 0，零 warning。
- `cargo test --workspace`：**126 passed / 0 failed / 1 ignored**。
- `cargo build --release -p bcompare`：退出码 0。
- `git diff --check` 通过；当前实现提交均带指定 Co-Authored-By trailer。

唯一 ignored 项仍是 Wave A 的回收站手动测试；本轮回收站不支持/取消回退使用受控
假 provider 验证，未修改系统回收站配置、清空回收站或在真实数据上试验永久删除。
文件系统副作用测试全部在系统临时目录执行并清理；未动仓库 tmp、主工作树或 vault。

## 完成清单与证据

| 项 | 实现 | 实际验证 |
|---|---|---|
| 4.1 | OpPlan 纯快照规划，侧向 Selection、ScanRequired、名称/属性/显示过滤与 include-hidden 覆盖；Copy/Move/Other、Exchange、Copy/Move To 三种路径模式、DOS/正则 Rename、Attributes、Touch 两种形式、Exclude、Ignored、NewFolder、CopyFilename | [operations.rs](../../crates/bc-engine-dir/tests/operations.rs)：所有操作生成计划，未扫描文件夹拒绝、隐藏文件默认跳过与显式包含、相对/base/none 目标、掩码/正则改名、目标冲突；真实执行复制/移动/交换/Touch/属性/新建/会话效果；目录元数据在子项完成后恢复 |
| 4.2 | FileOpJob + ConfirmPolicy + 共享 OpLog；确认按类别缓存，目标较新/只读独立，回收站 Unsupported 必须另行确认；单项错误继续、逐块字节进度、取消保留结果与受影响路径；Exchange 双临时副本与失败恢复 | 假 Interaction 回放 YesToAll/NoToAll/单项 No；普通 YesToAll 不越过永久删除回退；无 Interaction 时跳过回退；删除失败继续处理其他文件；5,000 文件中途取消与单个大复制取消；[exchange.rs](../../crates/bc-vfs/src/exchange.rs) 注入第二次发布失败后两侧原内容恢复 |
| 4.3 | update_entry 只更新目标条目及脏祖先；refresh_locations / refresh_selection / refresh_loaded / full_refresh；AutoRefresh 消费创建、修改、删除、重命名与 Overflow | 100,000 行模型中复制 1 个真实临时文件后只读 2 份单侧元数据、0 次 list、1 行改变，未触碰邻行修订号；真实 Windows watcher 新增文件自动入树，Full/普通 Refresh 可发现新项；大小写重命名后保留目标名称拼写 |
| 4.4 | DirOp::parse_script 与 Selection::parse_script；选择掩码侧/状态/文件类型可省略、多个掩码 OR、empty.folders，文件夹要求后代同类 | [op_script.rs](../../crates/bc-engine-dir/tests/op_script.rs)：帮助全部操作语法/示例可解析并构建预期计划（显式同名外部目标碰撞报错），全部 select 示例及齐次目录/空目录、非法参数验证 |
| 5.1 | 五种预设 + CustomSyncRules、逐项快照、文件夹动作摘要、可选 SyncBaseline 双侧变化冲突、visible/create-empty；有内容待复制的父目录显式规划，即使 create-empty 关闭也保留其元数据 | [sync.rs](../../crates/bc-engine-dir/tests/sync.rs)：五预设 A/B/C 真值表、自定义映射、Update Right/Mirror Right、文件夹复制+删除并存、可见过滤、空目录选项、无 create-empty 的必要父目录时间戳 |
| 5.2 | 逐项/目录覆盖与 Exclude、冲突前后导航、快照直接转 OpPlan，execute_and_refresh 复用执行/日志/确认/取消；实际统计与刷新错误分开保留 | 覆盖计划删除项为 LeaveAlone 后文件仍在、Exclude 项不复制、镜像实际删除、执行后 A/B 为 Same、基线双侧变化冲突、无方向冲突、覆盖解决导航冲突、失败与取消统计且不改写基线 |
| 5.3 | SyncCommand::parse_script 与预设反向序列化；mirror:all、多模式、非法方向报错 | 要求的 `sync create-empty mirror:left->right` 及其他四种预设参数往返；非法 mirror:all 在解析与 build 两层拒绝，自定义规则无脚本拼写则显式报错 |

本轮新增 **25 个 Rust test**（VFS 1、operations 12、op_script 2、sync 10），
加上 Wave A 基线 101，合计 126。测试中同一函数覆盖多个输入，不把输入个数冒计为 test 个数。

## 取消与进度实测

环境：Windows x86_64 MSVC，Rust/Cargo 1.99.1。
测试启动真实 std 工作线程，调用 `FileOpJob::run`，不是预先取消或假装执行。

```powershell
cargo test --release -p bc-engine-dir --test operations five_thousand_file_job -- --nocapture
```

系统临时目录真实生成 **5,000 个文件**，每文件 4 B；规划 5,000 个复制项。
ProgressSink 在已完成数到达 2,500 时记录请求时间并调用 CancelToken::cancel。

| 指标 | 实测 |
|---|---:|
| 完成 / 计划 | **2,500 / 5,000** |
| 目标实际文件数 | **2,500** |
| 字节进度合计 | **10,000 B** |
| 执行到返回总耗时（不含 fixture 生成/清理） | **13.943364 s** |
| 取消请求到 join/结果返回 | **3.972 ms** |
| 最后进度 total | **5,000** |

日志有明确 Cancelled 事件，OpOutcome.cancelled=true，已完成项保留，
未开始的 2,500 项没有生成目标。debug 多次复测取消约 8–9 ms，同样 <1 s。
完整本地日志：`target/folder-waveB-cancel-release.log`。

另一个 4 MiB 复制测试在首个 64 KiB 字节回调取消：
completed=0、bytes_written=65,536，目标目录为空（最终文件与临时文件都不存在）。
进度 message 包含当前文件路径、当前项/总项和累计字节；目录操作计划还包含
EnsureDir、PreserveDir、空目录清理步骤，因此非本 fixture 的 total 是计划步骤数，
不能一概解读为文件数。

## 局部刷新证据与边界

`localized_refresh_touches_one_row_in_a_hundred_thousand_node_model` 使用
**100,000 行的确定性树模型 + 1 个真实临时文件**，避免每次 workspace 单测
都生成 20 万个磁盘文件。复制后统计：

- VFS metadata 调用：2（同一行左右两侧）。
- VFS list 调用：0。
- changed_nodes：1。
- 无关邻行 revision：保持不变。
- 祖先只作内存汇总，不重新列整棵树。

这不是“本轮又生成了 10 万物理文件”的说法；Wave A 已完成双侧真实 10 万文件
基准，本轮验证的是局部 I/O/修订号不变量。若创建了之前不存在的父目录，
刷新会额外读取缺失祖先；目录重命名/新目录会扫描其相关子树，不能套用单文件常数。

AutoRefresh 只提供 watcher→刷新桥接；GUI 定时调度与菜单接线仍属任务组 6，
没有声称本轮完成 GUI 自动刷新界面。

## 安全语义与已批准决定

[ADR 103](../decisions/103-op-confirmation-and-sync-baseline.md) 已记录：

1. 未扫描的参与目录返回 ScanRequired，调用方先用 LoadDirJob 扫完整。
2. 普通 Yes/No-to-All 按类别缓存；Prompt 无 Interaction 默认拒绝需要确认的动作。
3. 回收站 Unsupported 的永久删除回退永远独立确认；
   即使普通策略为 YesToAll，无 Interaction 也不得回退永久删除。
4. Update Both 基线用于双侧元数据变化冲突；没有基线时无方向差异判冲突。
5. Exchange 是逐文件事务而非整个目录树事务，先完成两份副本再发布；
   回滚自身失败时保留备份路径并返回 RECOVERY REQUIRED，不清理恢复材料。

规划默认跳过过滤内容；包含隐藏项的偏好与单次覆盖由
`PlanOptions::from_handling(..., include_hidden_override)` 明确合并。
目录删除是后序逐项操作，隐藏/新出现/被拒绝的子项会使父目录保留；
空目录回收同样有独立永久回退确认。没有先递归删除再追溯判断过滤。

源与已知目标的元数据在执行前复核；陈旧目标或批内目的碰撞会报错，不用盲覆盖。
这不是跨进程文件系统锁或全计划原子性保证；正常并发文件系统竞态仍需由后续 UI
提示错误/重建计划处理，不宣称多目录事务。

## 接口调整（供合并与后续 worker）

- **bc-core / bc-jobs：零修改。** 继续使用 CancelCheck、Pattern、Side、
  CancelToken、JobContext、ProgressSink、Interaction、JobResult。
- **依赖：零新增、零变更。** Cargo.toml/Cargo.lock 本轮均未改；
  windows-sys 五特性仍是既有上限，无 GUI/异步运行时依赖。
- **Vfs**：新增默认 Unsupported 的 `copy_file/move_file/exchange_files/copy_metadata/
  remove_empty_dir`；LocalFs 实现。前三个接受 CancelCheck 和 byte-delta 回调，
  不递归遍历目录，过滤后的遍历归 OpPlan。
- **LocalFs**：普通文件复制共享既有临时文件/原子发布路径，移动同卷重命名、跨卷降级；
  copy_metadata 用于后序目录时间/属性/可选创建时间；Exchange 拥有单独恢复边界。
- **操作模型**：新增 Selection、PlanOptions、PlanError（含 ScanRequired/Collision）、
  OpPlan、OpItem/OpAction、Location、ConfirmKind。DirOp 扩展完整操作集合。
  Rename 的 DOS/regex 与 Windows case-only 路径均有测试，批内冲突拒绝。
- **FileOpJob**：原 `op/items` 占位字段改为 `plan/confirmation/log`。
  JobResult 的 payload 是 **OpOutcome**；取消不丢弃部分结果，不仅返回 JobError::Cancelled。
  调用方必须检查 outcome.cancelled/failed，而非只看 JobResult 是否 Ok。
- **OpLog**：线程安全的共享逐项事件序列（Started/Completed/Skipped/Failed/Cancelled/Info），
  可快照与写入文本；可由 GUI/脚本消费同一对象。
- **会话效果**：Exclude/Ignored 作为带 NodeId/revision 的结果，在调用方
  `OpOutcome::apply_session` 时应用；CopyFilename 返回文本供后续剪贴板 UI 使用，
  本轮引擎不依赖 GUI 或直接写系统剪贴板。
- **DirTree**：新增 ignored 标记、update_entry、set_ignored/set_excluded；
  局部更新保留其他行修订号；大小写重命名以目的拼写刷新，修复派生相对名称。
  I/O 完成后调用方应重新应用当前过滤，不在 view_rows 内隐式读取内容。
- **刷新**：refresh_locations、refresh_selection、refresh_loaded（已有扫描目录）、
  full_refresh（递归）与 AutoRefresh；返回可审计 metadata/listing/changed_nodes 统计。
- **脚本 API**：op_script 在引擎内解析操作与 select；不修改 bc-script 的未实施 runner。
  脚本选择忽略展开状态，但遵守文件/显示过滤，避免 unattended 行为依赖 GUI 折叠。
- **同步 API**：SyncPlan::build 现在返回 Result；build_with 接受 SyncOptions/可选基线；
  CustomSyncRules、SyncChoice、SyncItem、FolderActions、SyncStatistics、SyncRunResult。
  预览快照直接转 OpPlan，不在执行前按新状态重造另一套动作。
  目录覆盖递归作用于后代并先整体验证，冲突导航不环绕。
- **基线**：SyncBaseline::capture 只建模型，不自行持久化或自动推进；
  执行失败/取消不修改原基线。调用方在确认成功后决定保存；
  CRC 只有两边都可用时才参与比较，缓存消失本身不算内容变化。
- **HandlingOptions**：新增 include_hidden、copy_creation_time、copy_ntfs_permissions；
  最后一项按 design D10 明确 Unsupported，不虚假复制 ACL。

## 待决策与后续建议

- **当前无阻塞的待决策问题。** 上次四问已由本次重派全部批准，不再等待旧 dispatch 的问答。
- DOS 改名、路径模式、选择掩码已按帮助示例回归，但仍需任务组 7 的黄金对照覆盖边界输入；
  不冒称本轮完成参考工具所有细节的一致率验收。
- SyncBaseline 是元数据快照，不是完整内容历史；同大小/同时间戳且无 CRC 的隐藏修改
  不能凭元数据发现。需要更强保证时由上层内容比较与后续持久化策略补充。
- 确认 UI 的“包含隐藏项”复选框、剪贴板写入、Sync Now、日志面板、定时刷新调度均留给任务组 6；
  本轮已经提供对应的引擎模型/接口，不把 UI 占位当成已经实现。
- 取消/失败保留已完成文件；目录元数据后置步骤可能尚未执行，这是部分操作结果，
  不应对用户宣称整棵目录都已完成。
- 未实施 NTFS ACL/8.3 别名复制，遵循设计后置项；未在真实跨卷/UNC 共享做写入验收。
- 非 Windows 代码保留，但本机仅 Windows target；未冒称 Linux/macOS 运行验收。
- 任务组 6–8 与 RulesBased 合并接通仍在后续范围；本轮没有修改 GUI、文本引擎、会话或核心共享 crate。

## 提交记录

| 阶段 | commit |
|---|---|
| 前次已完成：五特性说明 | `37ec2231c849322a281be9ac2ba31622b0c2b533` |
| 前次已完成：ADR 101/102 重编号 | `b0ff5a240f7e2096f81dcc60eaace0f6ce72df7b` |
| 历史阻塞报告（本报告已覆盖其当前文件） | `cbda7bd` |
| 任务组 4 | `d099a5d42889371bc1f25b8cedeb15eaa0f53c03` |
| 任务组 5 | `9221667caa136492d7b956a53a91e3a318ace949` |
| 大小写改名刷新、进度与结果回归补充 | `c1b316049bd09aa39502ea42b221a7e241f0d25b` |
| 后序目录元数据保真补充 | `c00d192422d2f405bd9dc7a6c39188159921d653` |

均包含 `Co-Authored-By: pi <noreply@pi.dev>`；本完成报告另作 docs 提交。

## 最终验证输出（最后 10 行）

### cargo clippy --workspace --all-targets -- -D warnings

退出码 0，零 warning；完整本地日志 `target/folder-waveB-clippy-final.log`。

```text
    Checking bc-engine-text v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-engine-text)
    Checking bc-engine-bin v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-engine-bin)
    Checking bc-engine-dir v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-engine-dir)
    Checking bc-session v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-session)
    Checking bc-report v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-report)
    Checking bc-gui v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-gui)
    Checking bc-script v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-script)
    Checking bcomp v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\apps\bcomp)
    Checking bcompare v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\apps\bcompare)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.26s
```

### cargo test --workspace

退出码 0，126 passed / 0 failed / 1 ignored；末尾是 doctest 汇总，
完整本地日志 `target/folder-waveB-test-final.log`。

```text

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests bcomp

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### cargo build --release -p bcompare

退出码 0；完整本地日志 `target/folder-waveB-release-final.log`。

```text
   Compiling bc-vfs v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-vfs)
   Compiling bc-engine-text v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-engine-text)
   Compiling bc-engine-bin v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-engine-bin)
   Compiling bc-engine-dir v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-engine-dir)
   Compiling bc-session v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-session)
   Compiling bc-report v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-report)
   Compiling bc-gui v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-gui)
   Compiling bc-script v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-script)
   Compiling bcompare v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\apps\bcompare)
    Finished `release` profile [optimized + debuginfo] target(s) in 4.85s
```
