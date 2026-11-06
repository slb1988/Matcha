# text-compare-core — Wave 3b 收尾报告

## 结果：succeeded

任务 **7.1、7.3 均完成并勾选**，OpenSpec 进度 **30/40**。正式 30 个用例的 **60 份原始 expected XML** 和 **30 份同设置规则模板** 已入库；两个黄金入口不再走 oracle 缺失分支。

| 指标 | 结果 |
|---|---|
| Myers O(ND) / 内部 Quick | **30/30，100%** |
| Standard + closeness / 内部 Smart | **29/30，96.67%** |
| 正式 KNOWN_DIFFS | **1 条**：012-unrelated-replacement |
| 严格 Clippy | 零警告，退出码 0 |
| cargo test --workspace | **103 个测试通过，0 失败**，正式黄金实际运行 |
| cargo build --release -p bcompare | 通过 |

本报告取代 Wave 3 报告中“黄金未完成”的状态；Wave 3 的历史诊断与失败报告仍保留，未伪改历史结论。本轮未开始 8.x，也未提交/删除另一条线未跟踪的 folder-compare-sync change。

## 决策落地

### Myers 展示行

[ADR 006](../decisions/006-myers-display-packing.md) 记录实现与证据。MyersDiff SES 完全不改；Quick 默认在展示展开层把相邻 Delete/Insert 区段按位置配为 Changed，多余行仍为 orphan，不跨 Equal 或手动锚点。

新增 `align2_unpaired` / `align2_unpaired_cancellable`。Smart 使用 unpaired 骨架，engine 的 NeverAlignDiffs=True 也走此入口。正式 Myers 期望使用参考默认 **NeverAlignDiffs=False**，没有通过改变黄金选项规避默认打包行为。

[core 测试](../../crates/bc-core/tests/align.rs) 覆盖打包、尾部孤立行、原始入口与顺序不变量；[engine 测试](../../crates/bc-engine-text/tests/map_jobs.rs) 验证开关路由。原先的 never-align 诊断测试改用对应 unpaired API，原期望不改。

### 重要性与 EOF

正式第一轮发现的两个三态差异已由协调者明确批准修正，相关规格由协调者编辑：

- 格式内在不区分大小写导致的原始文本差异为 **Unimportant**，不是 Same；全局“大小写重要”仍不能将其升级为 Important。行内区间同步为不重要。
- 忽略行尾时，包括最终换行缺失在内的行尾差异为 **Same**；开启比较则为 Important，原始行尾与保存往返不变。

既有两个断言按获准契约更新，另外保留开启行尾比较和行内小写差异的覆盖。EOF 澄清还暴露了复制到未终止末行时默认 CRLF 的边界问题：复制操作现在将来源分隔符修复与区块替换合为同一 undo item；原字节级复制/撤销测试不放宽，已通过。

### 同设置模板与长行

[ADR 007](../decisions/007-golden-reference-settings.md) 记录 TTextRules 普通属性名、类型/偏移与黑盒验证。模板显式设置 CaseSig、LeadWSSig、EmbeddedWSSig、TrailWSSig、CompareEndings、NonWSSig、OrphansAlwaysSig，以及全部 10 类元素的 ElementSigs。

ElementSigs 实际结构是 `_N/ElementStr + ValueBool`，不是直接名称子节点或布尔数组。无空白的注释删除探针验证 True → leftorphan，False → leftsimilar。

参考 TTextFormat.CharPerLine 默认 4096，会人工拆分超长行。生成器临时添加仅匹配 `.matcha-golden-long` 的普通文本格式并设为 **65536**，使 12011 字符的用例保持一条物理行。输入文本没有缩短或修改，参考报告没有人工合并或改写。

## 正式语料与唯一差异

目录：[text/2way](../../crates/bc-core/tests/golden/text/2way)。每例包含原 UTF-8 输入模板、case.properties、expected.myers.xml、expected.standard.xml；30 个用例均按原输入保留，只有长行用例增加参考专用扩展名元数据。Git 对比确认本轮没有修改任何 left.txt/right.txt。

[KNOWN_DIFFS](../../crates/bc-core/tests/golden/KNOWN_DIFFS.md) 保存唯一例外的两份输入与双方完整输出：

- 参考 Standard：三行完全不相似替换按位置打包，整份输出 5 行。
- 项目 Standard：按“不相似不强配”要求保留三行删除、三行插入，整份输出 8 行。
- Myers 已按参考打包，本例 Myers 完全相等。

回归不只是按用例名放行：项目完整行序列、参考完整行序列、重要性以及正式例外列表都被校验，白名单过期或任一方产生其他变化都会失败。SmartAlignOptions 的默认阈值/权重没有调松。

正式逐行回归输出：

```text
golden.myers=30/30 golden.standard=29/30 known_diffs=1
```

core 额外检查 26 个格式无关的归一化对齐用例；engine 检查全部 30 个用例的对齐和原始三态，包含格式相关案例。

## 共享参考工具与恢复

生成命令：

```powershell
./tools/golden/gen_expected.ps1 -ReferenceExe '<参考程序路径>' -WorkerId '<worker handle>'
```

新增互斥要求后，最终 60 份期望在持锁的完整生成轮中重新产出：

- `reference_lock.ps1` 独占 CreateNew 创建 `%TEMP%/matcha-reference-tool.lock`，记录 worker id、PID、UTC 时间，争用最多等待 600 秒，不擅自抢占或删除别人的锁。
- 持锁检查没有其他参考进程；只备份/临时写入指定会话 XML 与需要的格式 XML。
- finally 在自有进程退出后恢复原字节；原本不存在的格式文件删除；在持锁状态验证两份设置恢复，然后释放租约。
- `test_reference_lock.ps1` 验证独占、元数据、争用超时和 finally 清理；测试使用独立路径，不调用参考工具。
- 最终生成日志确认设置恢复及字节校验，备份目录为本机 `tools/golden/backup/20261106-043124-363`。Git 只跟踪 backup/.gitignore，没有提交私人配置。
- 不读取/复制任何授权或密钥文件，不写安装目录，不访问 vault。定向只读检查严格限定获准的对齐/普通重要性/行长配置元数据。

恢复与重跑细节见 [tools/golden/README](../../tools/golden/README.md)。Rust 测试只读入库 XML，不需要参考程序或上述锁。

## 后续事项（不阻塞本次验收）

- 创建临时用户格式时，带 STX 的新名称未生效，字面名称则生效。本波生成器已使用字面名称；这提示 bc-formats 的旧“新建名称 STX”推断需单独核实。已通知协调者并记录在 [ADR 001](../decisions/001-stx-write-rule.md)，本轮没有越权修改 bc-formats。
- 协调者后续修改的 text-encoding/text-importance spec 仍未提交；worker 没有编辑或纳入本次实现提交。第一步指定的两个 spec 已按要求单独提交。
- 未跟踪的 `openspec/changes/folder-compare-sync/` 保持原样。剩余任务组 8–9 不属于本轮。

## 提交

| 范围 | commit |
|---|---|
| 指定策略名称/词边界 spec 澄清 | `b7561ed7677b557c3b4ffec0ab388067da339a1b` |
| Quick/三态兼容、正式生成器/模板/60 期望、完整回归与决策 | `0e4110fa36e45860f79d1ebae565fba5de98681f` |

均含 `Co-Authored-By: pi <noreply@pi.dev>`；本报告另作 docs 提交。没有新增第三方依赖。

## 验证输出最后 10 行（不足 10 行列全部）

### cargo clippy --workspace --all-targets -- -D warnings

```text
    Checking bc-engine-text v0.1.0 (D:\Github\Matcha\crates\bc-engine-text)
    Checking bc-engine-dir v0.1.0 (D:\Github\Matcha\crates\bc-engine-dir)
    Checking bc-session v0.1.0 (D:\Github\Matcha\crates\bc-session)
    Checking bc-report v0.1.0 (D:\Github\Matcha\crates\bc-report)
    Checking bc-gui v0.1.0 (D:\Github\Matcha\crates\bc-gui)
    Checking bc-script v0.1.0 (D:\Github\Matcha\crates\bc-script)
    Checking bcomp v0.1.0 (D:\Github\Matcha\apps\bcomp)
    Checking bcompare v0.1.0 (D:\Github\Matcha\apps\bcompare)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.52s
```

### cargo test --workspace

```text
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests bcomp

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```

### cargo build --release -p bcompare

```text
   Compiling bc-engine-text v0.1.0 (D:\Github\Matcha\crates\bc-engine-text)
   Compiling bc-engine-bin v0.1.0 (D:\Github\Matcha\crates\bc-engine-bin)
   Compiling bc-engine-table v0.1.0 (D:\Github\Matcha\crates\bc-engine-table)
   Compiling bc-engine-dir v0.1.0 (D:\Github\Matcha\crates\bc-engine-dir)
   Compiling bc-session v0.1.0 (D:\Github\Matcha\crates\bc-session)
   Compiling bc-report v0.1.0 (D:\Github\Matcha\crates\bc-report)
   Compiling bc-gui v0.1.0 (D:\Github\Matcha\crates\bc-gui)
   Compiling bc-script v0.1.0 (D:\Github\Matcha\crates\bc-script)
   Compiling bcompare v0.1.0 (D:\Github\Matcha\apps\bcompare)
    Finished `release` profile [optimized + debuginfo] target(s) in 6.05s
```

### 持锁生成轮

```text
generated 022-final-newline
generated 023-mixed-endings
generated 024-utf8-bom
generated 025-utf16le-versus-utf8
generated 026-utf16be-versus-utf8
generated 027-utf16-endianness
generated 028-unicode-change
generated 029-long-line
generated 030-multiline-comment
session/temporary-format settings restored and byte-verified; backup: D:\Github\Matcha\tools\golden\backup\20261106-043124-363
```
