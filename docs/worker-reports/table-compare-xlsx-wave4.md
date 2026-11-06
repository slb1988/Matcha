# table-compare-xlsx — Wave 4 批量复制与三层投影修复

日期：2026-11-06。按用户“继续迭代修复”接续，起点为干净的 master `f4ab5a5`，不是 Wave 2 的旧工作区。两份协调者 spec 已由该提交入库，文件夹线已通过 `43f8dab` 合入；本轮没有重新修改这些 spec、改写既有提交或进入其他 worktree。

## 结果：18/23，整体仍为部分交付

从 **17/23 → 18/23**，本轮完成 **4.5**：三档真实点击往返、两侧公式详情、数字格式与 Ignore Unimportant 的回归齐备。数字格式是 Wave 1/design D2 已明确的格式层范围；字体/填充仍后置，不冒称实现。

**4.1、4.2、4.3、4.4、6.1 仍未完成，未归档。** 排序的跨行选区范围仍按 Wave 3 的待确认项保留，未把“继续修复”解释为用户已经选择某个排序语义。

## 修复与先失败后通过的证据

### 批量复制

新增 `copy_tests.rs` 的 4 项回归在修复前**全部失败**，修复后全部通过：

| 原问题 | 修复与验证 |
|---|---|
| 复制遍历原始矩形，触及过滤掉的行和隐藏列 | GUI 使用已有 `Selection` 的可见投影；RawInput 点击 Copy 按钮后，隐藏行/列的公式不被覆盖，目标值正确，一次 Undo/Redo 恢复整批 |
| 批次遇到空隙后报错，但此前成功的单元格/行已写入 | 先规划并验证全部源/目的坐标，再生成一个 EditItem；失败不改变内容、修订号、历史或活动侧 |
| Show None 下仍按光标复制并建立历史 | 空可见选区直接返回；复制单元格和整行均不产生编辑 |
| 排序视图插入空隙后，继续使用旧的目的源索引，覆盖别的行 | 依据原始目的文件索引倒序应用；同插入点先更新原有行，再按比较顺序插入空隙。回归核对源键序列，防止丢行/重复键，并检查一次撤销/重做 |

引擎新增 `TableMap::copy_cells` / `copy_rows`，单格/单行入口委托给批量实现。每批只持一组前后快照，不再按每格创建快照和历史项；保留单元格类型、公式、样式属性及词法拼写。缺少目的行的单元格复制仍要求先 Copy Row，源侧空隙仍拒绝，未擅自新增“复制空隙即删除对侧”的语义。

新增 `batch_copy.rs` **4 项**验证：

- 错序列映射、右→左复制、数值类型、公式、跨 StyleId 样式与 `1.00` 拼写；
- 后续坐标非法时整批不变；deferred 模式也拒绝陈旧比较坐标，过期 job 不能覆盖新内容；
- 空侧补列、重复选中行去重及完整 undo/redo；
- 连续孤儿插入同一位置，保持顺序并正确移动手动锚点。

这里不宣称所有批量操作已具备线性性能：结构插入仍使用现有逐行 mutator，百万级连续插入的性能需要专项测量。因此不据此勾选 4.3。

### 显示层与 sheet 汇总

修复前，切换显示层会调用同时取消当前比较和其他 sheet 扫描的 `cancel`，后续标签可能一直 pending。现在将当前比较取消与整体取消分开：**仅投影/Resize to Fit 保留全层原始汇总扫描**；加载、重比等仍按原生命周期取消。

`projection_tests.rs` 新增 **3 项**：

1. 真实 RawInput 点击 Values / + Formulas / + Number formats，重复往返、切换 Ignore Unimportant；检查三态计数、两侧公式实际 text shape、格式原始 U 状态保留，且不创建编辑历史。
2. 真实点击缓存 sheet 标签，检查层档位、Ignore、Differences 过滤及状态一致。
3. 受控后台汇总 Job 暂停后切换显示层：修复前失败，修复后任务保留并发布标签结果，不再永远 pending。

第 2 项使用内存中的真实比较 Pair，第 3 项控制线程时序；它们**不是原生多工作簿加载、手动配对、设置和保存的完整端到端矩阵**，4.4 继续保留。原生 IME/物理鼠标验收也未冒充完成。

## 验证

Windows，Rust 1.99.1；没有宣称 MSRV 1.88 工具链验证。所有命令在本主工作区运行，没有新派发或复用旧 Orca Task/Dispatch。

| 验证 | 本轮结果 |
|---|---|
| `cargo clippy --workspace --all-targets -- -D warnings` | 退出 0、零 warning |
| `cargo test --workspace` | **270 passed / 0 failed / 3 ignored**，含已合入的文件夹线测试 |
| `cargo build --release --workspace` | 退出 0 |
| `cargo test -p bc-gui table::tests -- --nocapture` | 表格 GUI **26 passed**；GUI crate 合计 55 测 |
| `cargo test -p bc-engine-table --test batch_copy` | 4 passed |
| `cargo test -p bc-engine-table --test golden_table -- --nocapture` | 3 passed；30 例仍为行 **81/83（97.59%）**、格 **236/242（97.52%）**，唯一 `024-unaligned` 例外未扩大 |
| `cargo test --release -p bc-engine-table -- --ignored` | 两项实际通过：1 GiB 流式 **4.67 s**、100 MB 惰性/取消 **0.26 s** |
| `python -B -m unittest discover -s tools/golden -p test_table_parity.py` | 3 passed；仅校验器单测，本轮未重新解包/扫描参考帮助 |
| `openspec validate table-compare-xlsx --strict --json` | valid=true，0 issues |
| 本轮 9 个 Rust 文件定向 rustfmt、`git diff --check` | 通过；未全仓格式化 |

workspace 默认忽略的第三项为 `recycle_bin_manual`（Windows 手动回收站测试），本轮没有运行。不能把两项表格压力测试通过写成三个忽略项全跑过。

新增测试总计 **11 项**（4 引擎 + 4 GUI 复制 + 3 GUI 投影）。完整日志在忽略的 `target/table-wave4/`，包括 `copy-before.log`、`copy-after.log`、`projection-before.log`、`gui.log`、`batch-engine.log`、`clippy.log`、`test.log`、`release.log`、`golden.log`、`stress.log`。失败日志是修复前证据，不是最终门禁失败。

### 实际原生 GUI 基准

先以 `cargo run --release -p bc-engine-table --example table_gui_inputs` 生成系统临时目录中的百万行输入，然后运行 release Matcha；文本沿用未修改的 `tmp/left.txt`、`tmp/right.txt`。两组运行前后 SHA-256 一致；本轮临时 CSV 已清理。

| 输入与命令 | 帧数 | update mean / p99 / max ms | frame CPU p99 ms |
|---|---:|---:|---:|
| 两侧各 1,000,000 × 10、69,258,868 B；`--bench-table` | 600（另预热 30） | **0.709 / 1.190 / 1.427** | **4.004** |
| 两侧各 110,000,043 B；`--bench=600` | 600（另预热 30） | **0.759 / 1.416 / 1.755** | **2.188** |

均低于 update p99 4 ms / CPU p99 8 ms。表格平均 62 行/帧、末帧 420 格。wall time 包含加载/比较，index_mb 不是峰值内存，本轮没有新的峰值内存结论。

文本本轮 p99 1.416 ms 高于 Wave 2 记录的 1.282 ms；没有同轮基线 A/B 和多轮噪声评估，**不宣称严格性能不退化**，也不将这次波动直接归因为代码。日志 `bench-table.log`、`bench-text.log`、`bench-runs.json` 保留精确输入与输出。6.1 仍待完整交互矩阵和性能对照收敛。

## 剩余事项

- **4.3**：多列排序范围仍待用户确认。既有候选为“所选比较列顺序的稳定多键排序；跨行选区只排序选中行，未跨行时排序当前侧全表”。本轮没有改 Sort。大规模结构复制性能、快速连续结构命令、Align With/清除及完整 IME/焦点矩阵仍待验。
- **4.4**：Swap 与历史、原生多 sheet 配对/设置/重载/保存/失败路径等完整矩阵未完成。本次只验证显示层与缓存投影的一部分，不能整体勾选。
- **4.1/4.2**：自定义格式掩码、工具栏隐藏/恢复、导航及显示选项组合继续按对照清单推进。
- **6.1**：本轮已跑构建、自动回归、黄金、压力及两条原生基准，但仍保留完整验收及同轮性能对照缺口。

## 提交、接口及合并注意

| 范围 | commit |
|---|---|
| 本轮起点（协调者 spec 已提交） | `f4ab5a5e549727465dc30184ff9c2cd21d7e95ca` |
| 批量复制、扫描取消修复及 11 项回归 | `3436c936576fe4ee89be7bfebabe65a75c2561c1` |

本报告、命令对照状态和 tasks 4.5 另作 docs 提交；提交号用 `git log -1 --format=%H -- docs/worker-reports/table-compare-xlsx-wave4.md` 查询。未 push，未归档。

- 所有 `copy_*` 要求最新比较结果，即使开启 deferred edits；直接调用方遇到陈旧映射需先重比。GUI 原本已有 ready 检查，现在一次提交整批，而不是多次调用单格入口。
- 批量 Copy Row 的倒序依据是**目的源文件索引**，不是比较显示行；不能在合并时改回逐显示行倒序调用单行复制。
- 投影切换只保留返回全部原始层统计的扫描，不能把 `cancel_comparison` 替代到所有加载/设置变更路径，否则可能安装不同来源的旧结果。
- 无新增第三方依赖，Cargo.lock 未改；App/Home、文件夹代码、两份协调者 spec 均未修改。没有进入文件夹 worktree、访问 vault、调用参考程序或提交图标/私人配置/二进制 fixture。原来未跟踪的 folder change 已在本轮开始前由既有合并纳入版本控制，本轮没有另行处理它。
