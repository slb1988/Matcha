# 表格按需存储／内存优化：新 Session 交接

## 先读这段

**状态：真实 GUI ready 性能门已通过（2026-11-08，batch-20261108-102043，lazy ×0.857 vs dense）；稳定内存达标；实验入口仍默认关闭。切默认需用户明确决定，峰值项仍未达标，见下。**

- 工作目录：`D:\Github\Matcha`，分支：`master`。
- 本轮交付分两笔提交：`883deab`（测量工具修正：fingerprint CR 剥离 + 采样后端 CIM→Get-Process/性能计数器；A+C 切口证伪回退的记录）与紧随其后的三相位分块并行提交（align.rs/map.rs/perf.rs + 测量记录）。用 `git log --oneline -3` 确认。
- 迭代史：① A+C（demand.rs 空覆盖层短路 + TLS 行跨度暂存）经正式批 ×1.4552 未过门、受控探针证伪（A 中性、C 净负），已全回退，demand.rs 回 HEAD；② 引擎链三相位分块并行（row-text/verify-filter/compare，保序拼接，dense 同码受益）过门。两条教训：取值路径微优化对本行形（~70B/10列短行）无效；相位级并行有效。
- 基准数据中的 `revision=aae1494...`、`dirty=true` 是测量当时的事实，不能改成后来提交的 SHA。
- 当前用户已批准引擎侧共享代码的性能改动（迭代 2 已实施；迭代 3 = infer_keys/interpret 方向进行中）；切默认、push 仍需逐项授权。
- 阅读顺序：本文 → `tools/demand_proto.md` 末尾两个最新批次（A+C 证伪批 + 三相位并行过门批）→ 最新 GUI 基线与所需代码。
- 根 README 的 scaffold 描述是早期快照，不能据此判断当前表格功能尚未实现。

## 用户已确认的目标与执行边界

1. 比较指定两份百万行 CSV，内存目标 **300M 以下，争取更低**；127.2MB 的参考软件数据是对标线索，不是已经核实的同口径硬门槛。
2. 当前按 MiB 记录，Private Bytes、Private Working Set、总 Working Set 必须分列；峰值也必须报告，不能隐藏映射页、强制换页或裁剪工作集来“达标”。最终产品是否要求所有瞬时/总工作集也低于300M，尚未明确，不能自行放宽。
3. **真实 GUI ready 中位耗时退化不超过10%**；同机、同输入、同构建条件下配对比较。不能拿离线原型时间替代 GUI。
4. 完整比较及现有编辑、撤销/重做、结构操作、搜索、排序、复制、导出、保存语义不得缩水。不允许一次普通操作偷偷全量物化2000万 Cell 来掩盖兼容缺口。
5. 有可复跑、可追踪的统计，后续迭代追加历史。无样本特例，不为过摘要修改输入、规则或期望。
6. 继续实施使用 **真实 Orca 编排 + k3.1-pro**，先与协调 session 对齐切口/预算/验收，遇范围扩大再 ask；不要擅自改用其他模型或 Pi workflow/subagent。
7. 默认 Dense 路径保持，实验开关不自动启用；XLSX 不做专属解析优化。未请求独立代码评审，不默认开启评审流程。
8. 用户允许必要时参考 `https://github.com/pascalkuthe/imara-diff.git`；**本轮未读取、未引入**。若后续实际参考，记录版本/相关文件并核实许可证，不盲换差异算法。

## 最新权威 GUI 结果：ready 门已通过（batch-20261108-102043）

来源：`tools/demand_gui_baseline.json` 末批，原始目录 `target/memory-audit/gui/batch-20261108-154405/`；历经三相位并行（迭代2）+ interpret 快拒/融合扫描（迭代3）+ 分配消除/计分并行（迭代4）+ 加载跨度索引快速扫描（迭代5）后 3 轮中位：ready dense 4420.9 vs lazy 2394.8 ms（**×0.5417，≤1.10 回归门保持**）；稳定 lazy PB 161.4 / PWS 100.4 / WS 266.6 MiB 均 <300；OS WS 峰值 523.5 MiB >300 仍为未达标项；加载观测 PB 峰值受采样器首样本盲窗限制（口径注记见 demand_proto.md）。6 轮 fingerprint 全等冻结统计。exe `a0bfbbd3ff6f…`。迭代历程与相位明细见 demand_proto.md 末尾各批。已知新问题（用户反馈，优先級高于继续性能迭代）：GUI 移动/滚动 CSV 内容卡顿。

## 旧批结果：A+C 批未过门（batch-20261108-004948，已回退）与原始接入批（batch-20261107-161551）

A+C 批：ready ×1.4552 未过门；受控探针证伪该切口（A 中性、C 全相位净负），代码已全回退。细节见 demand_proto.md。以下为最初接入批的原始记录：

来源：`tools/demand_gui_baseline.json`，原始目录：
`target/memory-audit/gui/batch-20261107-161551/`。

每种模式三轮运行，分别取数值中位数：

| 指标 | Dense 默认路径 | Lazy 实验路径 |
|---|---:|---:|
| app:first-ready-frame | 4879.8 ms | 7093.1 ms（**+45.36%**） |
| 稳定 Private Bytes | 1793.4 MiB | **164.8 MiB** |
| 稳定 Private Working Set | 1667.9 MiB | **102.5 MiB** |
| 稳定总 Working Set | 1701.9 MiB | **268.6 MiB** |
| ready 前采样 PB 峰值 | 1843.0 MiB | **418.4 MiB** |
| OS 历史总 WS 峰值 | 1958.8 MiB | **523.7 MiB** |

- 稳定三项内存均低于300MiB；ready 与两个峰值仍有未达目标项。因此 GUI 主任务按 `failed` 结算，**不能切默认**。
- 本批10% ready上限约5367.8ms；下一批仍以同条件对照为准，不跨环境硬套这个毫秒数。
- 旧“4965.8 / 7112.1ms、+43.2%”是错误聚合：PowerShell `[int](n/2)` 对奇数索引取整，且 history 曾把 ready ms 乘1000仍标ms。已在 JSON `corrections` 中保留原值并纠正，raw 未改，不是新测量。
- GUI 验证时 exe SHA256：`c1ad58263d7cee4a145abf78be45169381a37c270b57246916810b89f40814b2`。重新构建后须记录新哈希，不能沿用旧二进制证据。
- 离线 b6 的7.5484s vs7.3136s（+3.21%）是**含加载的离线管线累计**，不是 GUI，也不是纯比较链；该历史结果有效，但不能证明 GUI 达标。
- 当前相位证据指向多遍取值/重复解析：Lazy 加载更快，但 prepare/infer、row-text/align、compare 链更慢。相位详情在工具说明；这不是“只有改共享引擎才可能达标”的下界证明。

## 已实现什么／优先看哪些文件

| 位置 | 作用 |
|---|---|
| `crates/bc-engine-table/src/demand.rs` | 私有快照、只读 mmap、逻辑记录跨度索引、借用快路径、8MiB视口缓存、身份键控稀疏编辑、行列映射、COW版本 |
| `crates/bc-engine-table/src/store.rs` | `SheetStore::{Dense,Lazy}` 存储桥 |
| `crates/bc-engine-table/src/data.rs` | `TableData/CellData/CellInfo` 只读访问接口 |
| `crates/bc-engine-table/src/jobs.rs`、`map.rs` | 后台比较、版本快照和结果模型；旧语义保留 |
| `crates/bc-engine-table/src/align.rs`、`inference.rs`、`compare.rs` | 阶段瓶颈入口；已提前释放不再需要的行文本缓冲 |
| `crates/bc-engine-table/src/source.rs`、`editing.rs`、`copying.rs`、`summary.rs` | 解析、编辑、复制、流式保存/导出适配 |
| `crates/bc-core/src/mmap.rs` | mmap 边界；不要把只读映射误作原文件不可变 |
| `crates/bc-gui/src/table/worker.rs` | `MATCHA_TABLE_CSV_LAZY=1` 受控加载入口 |
| 同目录 `commands.rs/cache.rs/view.rs/details.rs` | GUI命令/视图适配 |
| `crates/bc-gui/src/table/lazy_tests.rs` | Lazy 专用视图链测试 |
| `crates/bc-engine-table/tests/demand_access.rs` | 访问、格式边界、Store 对等与回写测试 |
| `crates/bc-engine-table/examples/demand_proto.rs` | 原型/访问正确性参考，现已使用引擎存储 |

保留的优化：无引号且语义等价时借用原文字段；相等原文复用既有守卫，原文不等仍类型化判断（`0.25` 与 `0.250` 的判同必须保留）；对齐后提前释放临时行文本。未采用 allocator 替换，推断串行化试验因更慢且不降峰值已回退，默认并行策略未改。

## 正确性与验证边界

本输入/规则的四摘要必须保持：

- rows：`092d09a4649bbfc5`
- cells：`612baef10de64f25`
- values：`41bbcbeeca0830a5`
- columns：`5513a3fb0a7609e5`
- 统计：rows=1000000、same=990000、important=10000；unimportant/left_only/right_only=0。

`tools/demand_proto_reference_frozen.json` 是**未改引擎的行级冻结**，不包含后来才加入的逐格冻结；不能伪造其历史覆盖范围。

已执行的证据：
- 原生 OS 窗口：Dense/Lazy各3轮打开与就绪、600帧滚动/远跳；GUI fingerprint一致。
- Headless真实视图链：Lazy值编辑、撤销/重做、结构插删/重排、保存复读、复制/导出对等。它调用真实视图/命令/工作线程，但**不是窗口内人工点击验证**。
- 实现阶段全仓80个测试套件通过；测量工具收尾38项纯函数/语法/门禁检查通过。没有做独立代码评审。
- 未验证或覆盖不足：原生窗口人工编辑/保存、XLSX与Lazy混合、百万行结构操作/排序/粘贴时延、fit-all全量宽度扫描、find对话框、会话持久化、regional/格式面板。
- 非UTF-8/定宽输入显式回退Dense并记录日志；不要把这些路径当已完成低内存适配。

## 复跑入口（PowerShell）

脚本当前面向本机 `D:\Github\Matcha`。不要在当前 handoff/commit 阶段再开开发任务；下次按改动范围选择测试，保留输出，不为换一种 grep 统计反复跑全仓。

```powershell
Set-Location D:\Github\Matcha
cargo build --release --bin matcha

$left  = "target\table-wave7\audit\inputs\table-left.csv"
$right = "target\table-wave7\audit\inputs\table-right.csv"

# 默认路径；先关闭自己这次打开的窗口，再测另一条路径
Remove-Item Env:MATCHA_TABLE_CSV_LAZY,Env:MATCHA_TABLE_PERF_EXIT -ErrorAction SilentlyContinue
& .\target\release\matcha.exe $left $right

# 实验路径（仅当前PowerShell及其子进程；用完移除变量恢复默认）
$env:MATCHA_TABLE_CSV_LAZY = "1"
& .\target\release\matcha.exe $left $right
Remove-Item Env:MATCHA_TABLE_CSV_LAZY -ErrorAction SilentlyContinue

# 工具自检；不启动大数据GUI测量
powershell -NoProfile -ExecutionPolicy Bypass -File tools/demand_measure_tests.ps1

# 修改后正式配对测量：默认3轮×两路径，稳定窗12s，追加GUI独立历史
powershell -NoProfile -ExecutionPolicy Bypass -File tools/demand_gui_measure.ps1

# 按变更范围选跑
cargo test --release -p bc-engine-table --test demand_access
cargo test --release -p bc-gui lazy_tests
```

- GUI历史：`tools/demand_gui_baseline.json`；离线历史：`tools/demand_proto_baseline.json`。不要混成一条趋势。
- 新脚本实算输入哈希、保留进程句柄/精确创建ticks、正常关闭/有界等待、不强杀；提前退出/未知退出码/稳定窗不足等失败时保留raw、不写有效基线。
- 子环境显式清除继承的 Lazy 与 PERF_EXIT，防止“Dense其实也Lazy”或自动退出污染结果。不要回退这些门禁。
- 旧GUI批次没有精确ticks/exit_code/正常关闭记录，不能补造或追认为满足新门禁。早期无 `raw_dir` 的raw可能被覆盖，不能拿新raw替旧批次补证。
- 不按进程名批量终止；不得关闭用户原有窗口，不清系统缓存/裁剪工作集。强杀/崩溃可能遗留 `%TEMP%\matcha-demand-<pid>-<n>.csv`，仅清理有确切归属证据的项。

## 输入与证据的保留

**先别 `cargo clean` 或 `git clean -fdx`：大输入及原始证据在 target，未提交到Git。**

两文件各69,258,868字节，100万数据行、10列：
- left SHA256：`1c7f090e6863bc81d4ce3c2802017ced9589363dafaca50be6b259e0410886d7`
- right SHA256：`53c033e4a8feb2c54eec8636884c690c4c2f1a3635fc461637a846166b479c81`

缺失时可用现有生成器：
`cargo run --release -p bc-engine-table --example table_gui_inputs -- 1000000`。
它在OS临时目录生成并输出 `left=...`、`right=...`、`cleanup=...`；按 `tools/table_wave7_ab_bench.ps1` 的输入准备逻辑复制到上述audit路径，**先核对大小/哈希，不覆盖未确认文件**。不要为了只恢复输入启动整个旧A/B基准。

详细证据和复跑说明集中在 `tools/demand_proto.md`，勿再复制整份历史报告。

## 下一 Session 的第一步

1. `git status --short`、`git log --oneline -3`，确认交付与新改动；不 reset/clean。
2. 读本文和 `tools/demand_proto.md` 末尾最新批次，核实输入仍在，再按相位定位最窄代码。不要从所有模块重新调查。
3. ready 门已过；剩余优化目标（用户批准中）：prepare-rules（infer_keys 全列扫描，lazy 链 delta 主体）。再往下候选：interpret 快拒；切默认是独立决定，需用户明确授权且须先处理峰值与未验证项清单。
4. 任何改动同时守住正确性（四摘要/fingerprint 恒定）、稳定内存 <300MiB 与 ready≤同轮 Dense×1.10；峰值继续单独跟踪，不允许为压峰值引入语义变化。
5. 参考软件研究只取得部分运行行为证据，未还原完整存储/索引布局；不要复述“必然mmap/绝非懒加载/每格3.4字节”等已撤销推断。

## Orca／协调记录（按需读取，不是当前执行授权）

- 本次编排任务均已结算；GUI主任务性能验收 `failed`，工具修正任务成功。没有需要新session继续等待的本任务。
- 最近Run：`run_6c3bfcd20045`（迭代 1 A+C 证伪回退 + 迭代 2 三相位并行过门）；有效实施 Dispatch：`ctx_339beba2ce85`（A+C）、`ctx_cc4efe216a84`（三相位并行，`task_2d9fc2e40d94`）；另有三个 dispatch_input 失败的重复 Task/Dispatch 已标 failed 并注明 duplicate。保留终端：`term_0e8e9892-7821-437e-b19e-6e597294eced`（pi/k3.1-pro，迭代期 retain）。
- 更早一轮（GUI 接入）：Run `run_68d3238fb8c0`；Task `task_e335ed96e8db`，Dispatch `ctx_27120ff79b01`；其终端均不可假设存活。
- usage 工具会把注入失败的 dispatch 与重试 session 重复计费；校正版账单：`C:/Users/admin/.orca-coordinator/bills/run_6c3bfcd20045.corrected.json`（合计 $51.50），原始账单与 bills.jsonl 趋势未修，不用原始重复总额。该工具问题不是当前性能任务的前置工作。
