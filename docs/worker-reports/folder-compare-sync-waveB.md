# folder-compare-sync — Wave B 阻塞交付报告

## 结果

**本次派发未完成，outcome = failed。** 仅完成两项前置文档工作；
任务组 4（4.1–4.4）和 5（5.1–5.3）均未开始，复选框保持未勾选。
OpenSpec 完成数仍为 16/34，不能把基线测试通过当作 Wave B 实现完成。

未修改任何 Rust 代码、Cargo 依赖、bc-core、bc-jobs、GUI、文本引擎或会话实现；
未进入主工作树/vault，未读取授权文件，未进行文件操作实现或新增副作用测试。

## 已完成项与提交

| 工作 | 证据 / commit |
|---|---|
| 提交协调者的 windows-sys 五特性说明 | `37ec2231c849322a281be9ac2ba31622b0c2b533` — docs(openspec): record five windows-sys features |
| 文件夹 ADR 重编号为 101/102，更新标题及全部当前引用 | `b0ff5a240f7e2096f81dcc60eaace0f6ce72df7b` — docs: renumber folder-track ADRs to 1xx |

已更新 bc-vfs/README.md、bc-engine-dir/README.md 和 Wave A 报告中的链接。
限定相关文件搜索旧文件名/旧 ADR 标签无命中，`git diff --check` 通过。
下一份文件夹 ADR 应从 103 开始。两个提交都含指定 Co-Authored-By trailer。
任务组 4/5 没有实现提交；本报告另作 docs 提交。

## 阻塞与待确认决策

已读取 Wave A 接口清单、操作/同步 specs、design D5/D6、现有 ops/sync/jobs/rules
及本轮所需的参考帮助页。实施前按要求通过 Orca ask 提交安全语义问题：

- question message ID：`msg_2f8667ac5193`
- escalation message ID：`msg_35602c56dfd8`
- task：`task_8cf0c3d69493`
- dispatch：`ctx_a1149b0cd04a`

原 ask 加五次对同一 ID 的 resume，共 **6 次各 600,000 ms 等待**，约 60 分钟，
仍未收到协调者答复。没有创建重复问题，也没有自行批准下述行为：

1. **未扫描文件夹**：建议纯 OpPlan 快照规划返回 ScanRequired，调用方先用 LoadDirJob
   扫完参与侧，再规划，以免递归复制/删除绕过过滤。
2. **确认缓存与回收站回退**：建议普通 Yes/No-to-All 按类别缓存，覆盖较新/只读单列；
   无显式策略的 silent 默认拒绝；永久删除回退始终独立确认、不复用普通 YesToAll，
   silent 且无 Interaction 时跳过。
3. **Update Both 冲突基线**：建议可选接收上次成功同步元数据快照判定双侧变化；
   无基线时 Different/无法判方向的项作为冲突。
4. **Exchange 文件夹**：建议过滤后逐文件规划，孤儿移到对侧；文件对先写临时副本，
   再发布并在失败时回滚，不承诺整棵目录树事务。

这些是**待确认建议，不是已采纳 ADR 或已实现行为**。
协调者可在新的派发中给出决策后继续任务组 4–5；本次未以猜测推进破坏性操作语义。

## 剩余工作与接口调整

- 4.1–4.4：OpPlan 全操作规划、FileOpJob/OpLog/确认/进度/取消、局部与自动刷新、
  完整操作与 select 脚本 API 全部待实现。
- 5.1–5.3：五种同步方法与自定义映射、冲突/覆盖/统计、OpPlan 执行桥接、
  sync 参数解析全部待实现。
- **取消与进度实测：本轮无新增实现，因此无 Wave B 实测数字。**
  Wave A 测试仍存在并在基线回归中通过，但不算 4.2 的取消/进度验收。
- **接口调整：无。bc-core / bc-jobs 改动均为零。**
- 未开始任务组 6 及以后。

## 基线回归（不是 Wave B 功能验收）

文档提交后实际执行，全部退出码 0：
- clippy：零 warning。
- workspace tests：**101 passed / 0 failed / 1 ignored**。
- release bcompare 构建通过。

### cargo clippy --workspace --all-targets -- -D warnings

日志 `target/folder-waveB-blocked-clippy.log`；不足 10 行则全文：

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.34s
```

### cargo test --workspace

日志 `target/folder-waveB-blocked-test.log`，最后 10 行：

```text

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests bcomp

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### cargo build --release -p bcompare

日志 `target/folder-waveB-blocked-release.log`；不足 10 行则全文：

```text
    Finished `release` profile [optimized + debuginfo] target(s) in 0.21s
```
