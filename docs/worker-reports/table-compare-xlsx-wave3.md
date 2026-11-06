# table-compare-xlsx — Wave 3 增量报告

日期：2026-11-06；以 `2ef42c2` 的部分收口为基线，按用户直接请求继续开发。未创建新派发，未复用失效的 Orca Task/Dispatch。

## 结果：继续推进，整体仍未完成

OpenSpec 从 **15/23 → 17/23**：本轮新增完成 **5.1、5.3**。**4.1–4.5、6.1 继续未勾选**，不以构建成功或局部回归代替整项验收。

修复了 Wave 2 报告中最优先的未提交草稿风险，并补齐剪贴板保真与多选删除。排序交互已经在本会话提出明确方案，等待用户确认，未自行替用户定案。

## 本轮改动与验证

| 改动 | 实际证据 |
|---|---|
| Modify 从独立窗口改为锚定单元格的内嵌编辑浮层 | RawInput F2、几何断言、Escape 取消；浮层位于实际单元格位置 |
| 未提交草稿进入 dirty/关闭保护 | Home 请求后 Cancel 保留草稿且不改模型；Exit 的 Discard 不写模型；Save changed files 先提交该侧草稿再保存 |
| 编辑安全 | 无变化 Commit 保留原值类型、公式、StyleId 且不建立 undo；失败保留草稿；进入字面值时保留数字格式并清除旧公式，不求值 |
| 异步结果与草稿隔离 | 编辑期间暂存比较结果，不改变编辑坐标；新 Commit 后旧结果被拒绝，再按新修订重比 |
| Ctrl+Enter 与焦点 | 在 TextEdit 处理前消费提交快捷键，避免意外写入换行；复制后目的侧键盘 Undo/Redo 可用 |
| TSV 剪贴板 | tab、CRLF、双引号、空字段采用限定符转义；真实 Copy/Paste 事件往返；空剪贴板不产生编辑 |
| 剪贴板焦点 | Find 输入框接收自己的粘贴；菜单粘贴显式恢复网格焦点；失效的粘贴请求不会抢走后续 Find 粘贴 |
| 普通文本 Paste | 保留目的单元格样式，不把粘贴误作格式清除 |
| 多选删除 | 可见投影映射到实际源行/源列；过滤选区不删除隐藏行，列错序和排序视图下仍删除正确源索引；Undo 恢复 |
| 修改键值 | RawInput F2 → 文本 → Ctrl+Enter，原孤儿重新配对；Undo 恢复原键和孤儿状态 |

新增 `bc-gui/src/table/edit_tests.rs` **13 项测试**。与既有 6 项表格 GUI 测试合计 19 项，整个 GUI crate 为 37 项；不是声称已有完整手动验收矩阵。

### 5.1 原生采集链路实际重跑

命令：

```text
powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/golden/gen_table_expected.ps1 -ReferenceExe <获准参考程序> -WorkerId direct-user-table-iteration -LockTimeoutSeconds 30
cargo test -p bc-engine-table --test golden_table -- --nocapture
```

- 30 例全部实际生成：27 CSV、3 个临时 xlsx（含多 sheet），不是复用旧结果后打印成功。
- 加固脚本：限制普通 Handling 属性名；生成前删除自有 target 旧报告，要求新报告非空；NUL JSON 封装在写入前验证解码字节与原生输出相等，保留 UTF-8 BOM（如有）。
- 持共享锁完成会话备份、临时注入、调用、自有进程退出、finally 恢复及原字节核对。此次备份目录为忽略的 `tools/golden/backup/table-20261106-104118-727/`，未提交私人配置。
- 脚本输出确认 `027-quoted-newline` 的无损 JSON 分支实际执行，不再沿用 Wave 2 对该分支“未在采集链路重跑”的限制。
- 30 份新 oracle 解码后与旧版比较，**除 Produced 时间行外字节完全一致**。这个审计只排除报告时间；正式黄金统计没有归一化任何状态。
- 生成的工作簿和参考图标继续只在忽略的 target，未提交二进制 fixture 或参考图标；本轮未再次进行二进制逆向，没有安装/修改工具配置。

```text
lossless JSON wrapper verified: 027-quoted-newline
generated 028-xlsx-values
generated 029-xlsx-booleans-errors
generated 030-xlsx-multiple
table sessions restored and byte-verified
table.golden cases=29/30 rows=81/83 (97.59%) cells=236/242 (97.52%) differences=1
```

唯一 Unaligned 例外及完整签名未扩大，也未通过增加无关相同行稀释差异。原生采集时只由脚本直接操作获准的普通会话 XML；未直接读取/复制授权或密钥文件，不写安装目录。

### 5.3 对照清单与自动覆盖检查

新增 [table-compare.md](../reference-parity/table-compare.md)，按 D9 逐条列出实际实现、部分验收、未接线与已批准后置项。

```text
python -B tools/golden/check_table_parity.py --help-dir <已只读解包的帮助目录>
table.parity.command_paragraphs=77 missing=0 topics=15
```

77 是原帮助的 p_Command 段落数，保留重复菜单前缀，并非 77 个功能全完成。ISO-8859-1 声明按原帮助正确解码。另有 Python 3 项单测，覆盖重复段落保留、遗漏/改名/编号错误以及缺主题时拒绝。

## 门禁

| 验证 | 结果 |
|---|---|
| 严格 workspace Clippy | 退出 0、零警告 |
| `cargo test --workspace` | **188 passed / 0 failed / 2 ignored** |
| `cargo build --release -p bcompare` | 通过 |
| 表格黄金 | 3 个 Rust 测试通过；30 语料行/格 **97.59% / 97.52%** |
| 对照校验器 | 实际帮助 77/77 段、15/15 主题；Python 3 测通过 |
| OpenSpec strict validate | valid=true，0 issues |
| 本轮文件 rustfmt / git diff --check | 通过 |

本轮没有重跑两项显式压力测试、百万行 GPU 滚动基准或文本同轮 A/B 基准；历史数字仍以 Wave 2 报告为准，不把历史 p99 写成本轮新测量。因此 6.1 保持未完成。

## 接口与剩余事项

- `Sheet::remove_row_indices` 新增非连续源行集合删除入口，单次重建列，避免把过滤后的显示区间误当源区间。
- GUI `Selection` 统一处理可见投影与列映射；`CellEditor` 保留原值、原 Cell 和源坐标，失败不会先 take 掉草稿。
- GUI 增加暂存 worker 输出及焦点恢复状态；仍以原修订号协议安装结果，不将对齐移回 update。
- 4.3 尚缺：多列排序（等待确认）、批量复制的事务性/性能与空隙场景、快速连续结构操作以及完整 IME/所有面板焦点矩阵。已补的多选删除不能替代这些验收。
- 待用户确认的排序方案：按所选比较列顺序做稳定多键排序；跨行选区只排序所选行，未跨行选择则排序当前侧全表。本轮未改既有 Sort 行为。
- 4.1/4.2/4.4/4.5 仍按对照清单及 Wave 2 报告补矩阵，特别是自定义格式 mask、工具栏隐藏/恢复、导航组合、多 sheet 缓存与设置、三档实际点击/数字格式。
- 原生 IME 候选窗与物理鼠标仍待可交互桌面人工验收；没有尝试解锁。
- 没有新增第三方依赖，也没有修改 app.rs/home.rs 接线，减少与文件夹线的合并冲突。

## 提交与保留状态

| 范围 | commit |
|---|---|
| 表格草稿、剪贴板、投影删除及 13 项回归 | `c937dfc` |
| 原生采集核验、30 份 oracle 时间更新、77 段对照、任务 5.1/5.3 | `d9ded8a` |

本报告另作 docs 提交；均带 `Co-Authored-By: pi <noreply@pi.dev>`。未 push、未归档、未派发 worker。

两份协调者 spec 修改继续留在工作区，字节与 Wave 2 记录完全一致，未混入提交：

- table-cell-comparison/spec.md：`a71177b2058a66dc94682fc6d6f82055c44460109897b1db25fb25ccce4b52af`
- table-columns-and-sheets/spec.md：`c7c06663d7b0525e029af5110c7461fb02b60f252bad5f11c6bda5d18b067b08`

未跟踪 folder-compare-sync change 保持原样；没有进入其 worktree、没有访问 vault、没有改动 tmp 输入。

## 最后 10 行（不足则全部）

### Clippy

```text
    Checking bc-gui v0.1.0 (D:\Github\Matcha\crates\bc-gui)
    Checking bcompare v0.1.0 (D:\Github\Matcha\apps\bcompare)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.93s
```

### Workspace tests

```text

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests bcomp

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### Release build

```text
   Compiling bc-engine-table v0.1.0 (D:\Github\Matcha\crates\bc-engine-table)
   Compiling bc-session v0.1.0 (D:\Github\Matcha\crates\bc-session)
   Compiling bc-report v0.1.0 (D:\Github\Matcha\crates\bc-report)
   Compiling bc-gui v0.1.0 (D:\Github\Matcha\crates\bc-gui)
   Compiling bc-script v0.1.0 (D:\Github\Matcha\crates\bc-script)
   Compiling bcompare v0.1.0 (D:\Github\Matcha\apps\bcompare)
    Finished `release` profile [optimized + debuginfo] target(s) in 8.16s
```
