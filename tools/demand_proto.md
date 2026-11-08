# demand_proto：按需存储原型与 GUI 接入验证（GUI 阶段已实施：实验入口 `MATCHA_TABLE_CSV_LAZY=1` 默认关闭；本轮验收未通过，见文末）

新 session 先读：[表格内存优化交接](../docs/worker-reports/table-memory-handoff.md)（目标、当前验收状态、源码入口、复跑与证据边界）。本页保留实验/GUI阶段的详细统计。

目的：验证"单一原文映射 + 逻辑记录索引 + 按需解析 + 有界视口缓存"在 300 MiB 设计预算下保持完整比较正确性。

## 复跑命令

```sh
cargo build --release -p bc-engine-table --example demand_proto
powershell -NoProfile -ExecutionPolicy Bypass -File tools/demand_proto_measure.ps1   # 默认3轮×两模式串行
```

- 原型二进制：`target/release/examples/demand_proto.exe`（构建产物，不入库）
- 直接单次运行：`demand_proto.exe --mode=reference|demand --left=<csv> --right=<csv> [--hold=12] [--cache-cap-mib=8]`
- 原始逐轮样本/日志：`target/memory-audit/prototype/`（可清理）；历史小基线（可追加对比）：`tools/demand_proto_baseline.json`

## 输入与对照基线（固定）

- 输入：`target/table-wave7/audit/inputs/table-left.csv` / `table-right.csv`（各 69,258,868 B，1M 数据行 ×10 列；SHA256 见 baseline json）。本修正轮对实际输入做了只读 SHA256 核对：left=1c7f090e…886d7、right=53c033e4…b479c81，与 b6 批次记录值一致。测量脚本自下一批次起每批开始前实测并写入两文件哈希（历史期望常量仅作校验，不冒充实测值），故每批均在两输入刚被完整预读后的条件下运行。
- 冻结参考：`tools/demand_proto_reference_frozen.json`（byte-identical 复制自 target 的原始冻结，SHA256 058b748d…；**仅覆盖行级摘要**，为 trait 加法前的未改引擎 reference；不冒充逐格冻结）。
- 正确性对照（本轮新增，非回溯伪造）：三轮 × 两模式恒定一致且与 GUI 指纹一致
  - 行级摘要流 rows_digest=`092d09a4649bbfc5`（保留旧口径）；
  - 逐格三层分类摘要 cell_digest=`612baef10de64f25`（经既有公开接口 `ComparedData::cell_diff`，10M 格 value/formula/format 三层）；
  - 访问层值摘要 value_digest=`41bbcbeeca0830a5`（两路径 TableData 逐格 typed display 一致性核对）；
  - 比较范围列映射+键 columns_digest=`5513a3fb0a7609e5`。
  - 统计 rows=1,000,000 same=990,000 unimportant=0 important=10,000 left_only=0 right_only=0。

## 优化历程（compare-mark 口径批次）

| 批次 | variant | demand 核心 | reference 核心 | 比值 | demand 峰值PB(各轮) | 备注 |
|---|---|---|---|---|---|---|
| b2 | 初版（整行 Records 重解析） | 28219.3 ms | 7259.4 ms | 3.89× | 449.9/449.9/449.9 |  |
| b4 | +免引号拆分借用原文 | 11375.9 ms | 7555.3 ms | 1.51× | 450.1/450.1/292.3 |  |
| b5 | +compare 阶段原文直供（差异格仍走引擎类型化归一） | 7623.2 ms | 7342.8 ms | **1.04×** | 450.1/450.1/292.3 | 达 ≤+10% 目标 |
| b6 | +align 的 row_text 缓冲在 strategy-align 后即释放（drop(text) 一行，行为不变） | 7548.4 ms | 7313.6 ms | **1.03×** | 293.2/292.4/292.3 | 观测 PB 峰值降至 300 内（OS WS 峰值仍 430.5>300）；推断串行化实验(P1)已被单次数据否定（峰值不变且更慢）并回退，实验接口已清理 |

- 语义不变性：四摘要全部批次恒定（092d09a4649bbfc5 / 612baef10de64f25 / 41bbcbeeca0830a5 / 5513a3fb0a7609e5）；引擎 69 测试全绿，含 split-vs-Records 差分边界、引号端到端、数值文本差异判同（"0.25" vs "0.250"）回归。
- 峰值：b6 观测 PB 峰值三轮 293.2/292.4/292.3 MiB（原始字节 307458048/306614272/306466816），中位 292.4、最大 293.2 MiB，位于 infer_keys 阶段；为采样观测最大值，非瞬时真峰；reference 同批观测 PB 峰值中位 1760.1 MiB（1760.14453125）；row_text 缓冲提前释放生效。**OS 历史 WS 峰值中位 430.5 MiB 仍高于 300**——低于 300 的只有稳定私有提交、稳定 WS 与采样 PB 峰值，不得概括为“所有内存峰值已进 300”。各结构量级为估算标记，不将不同阶段估算相加充当实测归因。
- 峰值结论按同批 raw_dir 复算（batch-20261107-121848）；仅带 raw_dir 的批次可按目录追溯，早期无 raw_dir 批次的原始样本存在被覆盖风险（本轮起脚本按批次隔离），不以最新 raw 替旧批次补证。

- 口径：核心时间=已有 compare 标记；digest、viewport 各单列；wall_to_hold 为含验证/hold 前的墙上总时长，不作核心时间。旧批次（baseline json batches[0]）为 hold:begin 旧口径，已标注保留。
- demand 稳定态包含**保持存活的有界缓存**（live_cache_bytes=8,388,505 ≈ 上限 8 MiB，采样后才释放）；51.6 MiB 为缓存+索引+比较产物同存的实测稳定值。
- demand WS 185.7 MiB 含映射文件触碰页；**映射页实际驻留未单独测量（unknown）**，mapped_virtual_bytes=138,517,736 仅为虚拟映射尺寸，不能以静态文件大小冒充实测驻留。
- 采样：WMI 按 PID 目标 300 ms；观测峰值为采样最大值非瞬时真峰；os_working_set_peak 为 OS 历史计数器，单独命名。
- 退出码：三轮两模式全部 exit 0（Process 类读取）；缺 summary/cells/compare/hold:end 或摘要跨轮不一致时测量脚本非零退出且不写基线。
- 耗时口径：离线原型仅与同模式同批次 reference 比（b6：7548.4 vs 7313.6 ms，+3.21%）；不得与 GUI 5.53 s 直接比，10% 验收属下一阶段。

## 视口有界缓存验证（demand 内建驱动）

112,640 次访问（顺序扫描+4096 远跳+2048 回访），字节预算 8 MiB：max_cache_bytes=8,388,608 恰达上限不越界，淘汰 87,795 次后远跳回访正常重建，无随访问总量增长；缓存填充后保活至稳定采样窗口结束（live_cache_bytes=8,388,505），采样后正常释放。

## 当前稳定内存与耗时（最新批次 b6，3 轮串行中位，MiB）

| 模式 | 稳定 PrivateBytes | 稳定 WS | 稳定 PrivateWS | OS WS 峰值(中位) | 核心(compare标记) |
|---|---|---|---|---|---|
| reference | 1687.6 | 1624.6 | 1620.6 | 1883.2 | 7313.6 ms |
| demand | 51.6 | 185.7 | 49.7 | 430.5（>300，未达标项） | 7548.4 ms |

- 实现 variant：mmap 只读映射 + 逻辑记录跨度索引（Records 语义）+ 免引号行直接拆分借用原文切片（含引号/转义行回退 Records）+ compare 阶段原文直供（引擎既有 raw-text 守卫处理相等格，差异格由 normalized_value 类型化，不绕过 formula/format 层）+ align 的 row_text 缓冲提前释放 + 8 MiB 字节封顶 LRU 视口缓存（保活采样）。
- imara-diff：项目未使用，本轮热点为 CSV 解析/访问而非 diff 算法，未读取未采用。

## 已证/未证边界

- 已证：同一引擎 infer/align/compare 代码经 `TableData` 只读 trait 在按需存储上产出行级+逐格分类+访问层值三类摘要全等（含引号/转义/单侧行，见 tests/demand_access.rs 最小边界测试）。
- 已证内存：稳定态（含存活 8 MiB 缓存）51.6 MiB 远低于 300 MiB 设计预算。
- 未证/限制：原型只读；编辑/撤销/排序/导出/原格式回写的正式接入未验证；映射页实际驻留 unknown；OS 历史 WS 峰值中位 430.5 MiB 仍超 300（稳定 PB/WS 与采样 PB 峰值在 300 内，采样最大值非瞬时真峰）；核心耗时 b6 为 reference 的 1.03×（7548.4 vs 7313.6 ms，+3.21%）。
- 引擎改动面（最小加法，公开签名全部保留为薄包装）：`data.rs` 新增 trait/CellData/CellInfo + Sheet 实现；`infer_keys_data` / `KeyAlign::align_data` / `compare_alignment_data` / `align_columns_data` / `detect_header_data` / `CellComparator::compare_data` 通用入口；`align.rs` 一行 drop(text) 提前释放 row_text 缓冲。GUI/默认加载路径零改动；引擎 69 测试 + GUI 141 测试全绿。
- Cargo.toml 变化：bc-engine-table [dev-dependencies] 加 memmap2（仅示例用，bc-core 已有同款先例）。

## 耗时与峰值根因（b6 实测后）

原“多次全列扫描 × 每行重解析”主因已由原型内快路径解决（免引号拆分借用 + compare 原文直供），核心耗时从 28.2s 降至 7.5s（1.03× reference，+3.21%）。观测 PB 峰值经一行 row_text 缓冲提前释放从中位 ~450 降至 292.4 MiB（最大 293.2）；OS 历史 WS 峰值中位 430.5 MiB 仍高于 300。此前候选清单（融合管线/字段偏移索引/分块/受限类型缓存）均无需采用；imara-diff 未读取未采用（项目未使用，热点在 CSV 访问不在 diff）。

## 下一步需审批

是否接 GUI 验证阶段（含编辑/回写/排序/导出接入与 10% GUI 口径验收）；300 MiB 最终统计口径确认。

## GUI 接入验证阶段（真实窗口，已实施；实验入口默认关闭）

接入切口（经协调者批准，选 B：结构编辑完整支持）：引擎新增 `demand.rs`（DemandSheet：私有快照副本 + 只读 mmap + 记录跨度索引 + 免引号借用拆分 + 8 MiB 字节封顶 LRU 视口缓存 + 身份键控稀疏覆盖层/行列顺序映射 + COW 版本快照）与 `store.rs`（`SheetStore::{Dense,Lazy}` 薄桥）；`TableMap`/`TableAlignJob` 换持 SheetStore，后台比较走 `_data` 泛型入口的 Arc 版本快照（编辑 COW 不影响在跑 job；stale 结果沿用 revision 丢弃机制）；编辑/撤销/重做经既有 EditItem 快照机制作用于覆盖层（不入 LRU）；保存/导出流式写（干净行原文直写、脏行按记录跨度拼接、插入行序列化、列结构变化时整行重序列化，与 dense `write_delimited` 字节相等）；快照副本使原地另存不与映射冲突，Drop 先 unmap 再删快照。GUI 侧：`MATCHA_TABLE_CSV_LAZY=1` 仅切 `worker::load` 的 CSV 分支（xlsx/默认路径零改动；非 UTF-8/定宽显式回退 Dense 并打日志）。bc-core 新增安全 `mmap::map_readonly` 包装（引擎 crate forbid unsafe）。

复测命令：

```sh
cargo build --release --bin matcha
powershell -NoProfile -ExecutionPolicy Bypass -File tools/demand_gui_measure.ps1   # 默认3轮×两路径串行
```

- 旧路径：`matcha.exe <left> <right>`；按需路径：同命令加环境变量 `MATCHA_TABLE_CSV_LAZY=1`；两路径均 `MATCHA_TABLE_PERF=1` 取 `app:first-ready-frame` 为 ready 口径（不用 PERF_EXIT 冒充稳定采样；稳定窗 12s 后按 PID+路径+启动时间+命令行核实仅关自己实例）。
- 原始逐轮样本/日志：`target/memory-audit/gui/batch-*`；历史追加：`tools/demand_gui_baseline.json`。

工具修正（本轮，2026-11-07 后半）：`tools/demand_gui_measure.ps1` 与 `tools/demand_proto_measure.ps1` 的 `Get-Med` 原为 `[int](n/2)` 索引（3 元素取到最大而非中位且截断小数），已修为数值排序+floor 索引/偶数均值、保留小数；GUI history 的 ready 汇总原为毫秒×1000，已修为直接以 ms 保存。GUI 脚本关闭协议重写为安全关门：启动后即取独立 Get-Process 句柄对象并留存在身份记录中（固定 PID、精确 StartTime ticks、exe 路径、命令行子串），关闭前重核身份，仅 Refresh+CloseMainWindow+有界等待（默认不强杀），记录真实 exit_code 与 shutdown_method；提前退出/缺 ready/缺指纹/稳定窗未走完/样本不足/身份或关闭失败均整批失败且不再启动下一实例、保留 raw 不追加基线。纯函数与门禁测试：`tools/demand_measure_tests.ps1`（语法、空/奇/偶/小数/真实三元组、窗口选择、静态门禁）；小数据 smoke 已验证篡改 ticks 拒绝+正常关闭+真实退出码 0。

旧批次局限（不追认）：batch-20261107-161551 早于安全关闭协议——身份为 PID+路径+StartTime<60s 粗判+Stop-Process -Force 强杀，无 exit_code/shutdown_method/精确 ticks 记录；其 raw/聚合（除上文 corrections 注明项外）保持原值不改，不声称它满足新安全门槛。正常关闭后按需路径快照可经 Drop 自删，减少强杀造成的 %TEMP% 快照泄漏。

### 真实 GUI 三轮结果（batch-20261107-161551，同机同构建串行，MiB）

| 路径 | ready 中位 | 稳定PB | 稳定WS | 稳定PWS | 加载观测PB峰值 | 观测PB峰值 | OS WS峰值 | 映射虚拟 |
|---|---|---|---|---|---|---|---|---|
| dense | 4879.8 ms | 1793.4 | 1701.9 | 1667.9 | 1843.0 | 2097.5 | 1958.8 | 0 |
| lazy | 7093.1 ms（**+45.4%，超 ≤+10% 门**） | **164.8** | **268.6** | **102.5** | 418.4（>300，未达标项） | 418.4（>300） | 523.7（>300） | 138.5MB（虚拟≠驻留） |

- 稳定私有提交/私有工作集（首要门槛）：lazy 164.8/102.5 MiB **<300 达成**（dense 的 ~10.9× 缩减）；总 WS 268.6 <300；加载/OS 峰值如实记录为 >300 未达标项，不概括全部内存达标。
- 正确性门禁：6 轮 fingerprint 全等且等于冻结统计（rows=1,000,000 same=990,000 unimportant=0 important=10,000 left_only=0 right_only=0）；exe c1ad58263d…、rev aae1494 dirty、输入 SHA256 实算同前。
- 远处行/首屏真实窗口证据：`--bench-table` 600 帧平滑+远跳（36.4fps，帧 p50 0.65ms），fingerprint 全等，滚动会话稳定 PB 169.6 MiB / 会话观测 PB 峰值 346.9 MiB / OS WS 峰值 524.2 MiB，缓存有界（raw: target/memory-audit/gui/lazy-bench.json）。

### ready 退化相位实测（3 轮中位）与三个耗时口径

lazy 加载 1.59s/侧（快照复制+索引；dense 3.36s/侧物化）→ 净省 ~1.77s；但比较链 +3.97s：prepare-rules +0.88s（infer_keys 全列扫描×10 候选）、key-align +2.17s（row-text +1.61s、verify-filter +0.46s、hashes +0.10s）、compare +0.98s。当前实现的阶段瓶颈：引擎链对两侧数据做 ~5 次全量遍次，按需存储每次重解析原文，dense 每遍近零成本重读物化格。**这是当前实现的相位瓶颈，不是下界证明**；下一轮候选（未测、不排除）：紧凑字段偏移定位、块访问、受限类型缓存、链相位并行/合并（触及共享引擎代码，需另行批准）。

三个耗时口径明确分开，不可混用：
- **GUI ready**（本轮验收口径）：`app:first-ready-frame`，dense 4879.8 vs lazy 7093.1 ms = +45.4%（门 ≤+10%）→ **本轮 GUI 验收未通过**。（聚合修正：Get-Med 取整缺陷曾把 dense 中位取成最大值 4965.8，且 GUI history 误将 ready 毫秒×1000；memory 各中位经主 session 按原 ready 窗口重算与上表一致；修正痕见 tools/demand_gui_baseline.json corrections。）
- **离线管线累计**（原型 b6 口径：加载至 compare 完成的累计墙上时间，含加载主导）：b6 demand 7548.4 vs reference 7313.6 ms = 1.03×——该口径原有定义如此，有效保留，但不代替 GUI 验收。
- **纯引擎链**（isolate 后 prepare+align+compare）：dense ≈1.24-1.39s vs lazy ≈5.21s（同机同输入，相位见上表）。

### 验收结论（本轮）

- 内存首要门槛（稳定私有提交/私有工作集）：lazy 164.8/102.5 MiB <300 **达成**（dense 的 ~10.9× 缩减）；总 WS 268.6 <300。
- 未解决项（如实列出，不表示已获放宽同意）：加载观测 PB 峰值 418.4 MiB >300；OS WS 峰值 523.7 MiB >300。
- ready +45.4% 明确违反 ≤+10% 门槛 → **本轮验收未通过**；实验入口保留默认关闭，不切默认、不回滚、不 commit。

### 功能链证据（来源分列：真实 OS 窗口 vs headless 视图链测试）

真实 OS 窗口（matcha.exe 实际进程，MATCHA_TABLE_CSV_LAZY=1）：
- 6 轮（dense×3 + lazy×3，同机同构建串行）+ 1 次滚动会话（--bench-table 600 帧平滑+远跳，36.4fps、帧 p50 0.65ms）：fingerprint 全等且等于冻结统计（rows=1,000,000 same=990,000 unimportant=0 important=10,000）；首屏就绪帧与远处行跳转渲染均实际发生（每帧绘制 ~60 行），缓存有界。raw: target/memory-audit/gui/batch-20261107-161551 + lazy-bench.json。
- 真实窗口未覆盖：窗口内人工编辑/保存操作（无桌面自动化）；该部分证据来自下述视图链测试。

headless 真实视图链（egui Context 驱动真实 TableCompareView/命令/工作线程，非存储桩）：
- `bc-gui/src/table/lazy_tests.rs`（3 测试，Lazy stores）：值编辑→撤销/重做→另存→字节核对→dense/lazy 复读一致；结构链（插行/插列→撤销×2→重做×2→排序落盘→撤销→另存→复读核对）；复制格→统计变化→撤销→导出字节核对。
- 引擎 `tests/demand_access.rs::store_chain_matches_dense_and_save_bytes_equal`：TableAlignJob over SheetStore 的行流+逐格三层+逐格显示全等，编辑后 recompare 统计相等，保存字节 dense=lazy。
- 全仓 80 测试套件全绿（引擎 72+3 新、GUI 141+3 新；dense 默认路径零回归）。Dense 测试通过不充当 Lazy 验证；lazy 覆盖如上，未验证项见下。
- 离线示例已改接引擎 DemandSheet：真实输入上四摘要仍 092d09a4649bbfc5 / 612baef10de64f25 / 41bbcbeeca0830a5 / 5513a3fb0a7609e5（两模式）。

### 未验证/限制（如实）

- **本轮验收未通过**：ready +45.4% 超 ≤+10% 门；加载观测 PB 峰值 418.4 与 OS WS 峰值 523.7 MiB >300 为未解决项（未获放宽同意）。
- 未验证：真实窗口内人工编辑/保存（无桌面自动化，视图链测试代替）；XLSX 与 lazy 混合比较（XLSX 走原路径，SheetStore 混合链未实测）；1M 规模下结构编辑/排序/粘贴的交互耗时（仅夹具级功能验证）；fit-all 列宽全量扫描（有界但慢）；find 对话框 lazy 端到端用例；会话持久化对 lazy 侧的序列化；regional/格式面板与 lazy 交互；快照副本磁盘占用（每侧 69MB 临时文件，Drop 删除）。
- 实验入口默认关闭；未切默认、未 commit。下一轮是否做相位优化（紧凑字段定位/块访问/类型缓存/链并行）由主 session 与用户确认范围。
- 运维注意：强杀进程（Stop-Process -Force / 崩溃）不会跑 Drop，会在 %TEMP% 泄漏 `matcha-demand-<pid>-<n>.csv` 快照（每侧一份输入拷贝）；正常退出/正常关闭窗口会自删。本轮测量后的 8 个泄漏快照已按自有 PID 核实清理。

启动与复测命令：

```sh
cargo build --release --bin matcha
# 旧路径真实窗口：
D:\Github\Matcha\target\release\matcha.exe target\table-wave7\audit\inputs\table-left.csv target\table-wave7\audit\inputs\table-right.csv
# 按需路径真实窗口：同上命令 + 环境变量 MATCHA_TABLE_CSV_LAZY=1
# 测量（3轮×两路径串行，raw独立目录+历史追加）：
powershell -NoProfile -ExecutionPolicy Bypass -File tools/demand_gui_measure.ps1
```

### A+C 取值热点削减批次（batch-20261108-004948，ready 门未过）

切口（只改 `crates/bc-engine-table/src/demand.rs`，经协调者批准）：A=空覆盖层短路（`overlay()` 访问器先判 `edits.is_empty()`；`display_text_at`/`preview` 对 `display_overrides` 判空；`cached_row_displays` 的 `keys().any` 前判空）；C=`SPAN_SCRATCH`（TLS 按表 id 键控 (record 序号, 免引号拆分的字段跨度 Vec<(u32,u32)>，相对 strip_ending 后 body；表 id 数≥4 清空，同 ROW_SCRATCH 约束），`raw_field_phys` 快路径同 record 连续列复用一次拆分，含限定符行仍回退 `parsed_field`）。未动共享引擎代码、未切默认开关、未 commit。

测量工具修正（本批前置，协调者逐项批准；门禁严格性不变）：①fingerprint 捕获串 `TrimEnd('\r')`——drain 用 `AppendLine` 重建 CRLF 行而正则 `.*` 吃掉 `\r`，冻结串精确比对在 Windows 上恒败（工具重写后首次跑完整批才发现）；②采样后端由 CIM `Win32_PerfRawData_PerfProc_Process`（本机实测 330–410ms/次、枚举全机 ~690 进程行，+300ms sleep ≈700ms/迭代，12s 窗仅 17 样本 < 20 门，两个中间批 batch-20261108-003632/004234 因此在 dense run1 中止，raw 保留未入历史）换为 Get-Process(`PrivateMemorySize64`/`WorkingSet64`/`PeakWorkingSet64`) + 按实例 `PerformanceCounter(Process\Working Set - Private)`；PID→实例名每轮启动时经 `ID Process` 计数器解析一次，映射不到/多义立即整批失败；三项指标定义、300ms 节拍、12s 窗、stable_n≥20 等阈值全部不变，raw 与聚合 JSON 均记录 `sampler_backend` 溯源。**已知限制**：实例解析在采样开始前（首样本延迟 ~1–1.5s），加载观测 PB 峰值的采样覆盖窗变窄（本批 lazy 加载观测峰值=稳定值，低于旧批 418.4 部分归因于此，不表示加载峰值真实下降）；OS WS 峰值为累计计数器口径不受影响。`tools/demand_proto_measure.ps1` 的同款 CIM 采样器本轮未动，存在同样的超节拍风险。

结果（exe `e235ad7cdf6db19f819ea3191094428d20b6dd1185c74591b93c73a9ed459a11`，3 轮中位，MiB）：

| 路径 | ready 中位 | 稳定PB | 稳定WS | 稳定PWS | 加载观测PB峰值 | 观测PB峰值 | OS WS峰值 |
|---|---|---|---|---|---|---|---|
| dense | 5859.9 ms | 1798.6 | 1712.6 | 1673.3 | 1801.8 | 1801.8 | 1969.9 |
| lazy | 8527.4 ms（**×1.4552，未过 ≤1.10 门**） | 166.9 | 277.0 | 105.5 | 166.9（采样盲窗，见上） | 166.9 | 532.0（>300 未达标项） |

- 正确性门禁：6 轮 fingerprint 全等冻结统计（rows=1,000,000 same=990,000 important=10,000，余为 0）；输入 SHA256 同前；exit=0、`ClosedByMainWindow` 正常关闭全部成立；历史已追加 `tools/demand_gui_baseline.json`。
- 相位证据（lazy 本批 vs 旧批，ms）：prepare-rules 1696–1756 vs 1170–1191；row-text 2287–2502 vs 1995–2013；verify-filter 691–703 vs 487–498；compare 1363–1392 vs 1227–1239；load-side 持平（~1505–1590）；同批 dense 各相位仅 +2–12%。比值与旧批 +45.4% 几乎相同 → **A+C 对 ready 无净改善**。初步判断（非结论）：~70 字节/10 列短行下每格 TLS HashMap 开销≈省下的部分重扫，列主序相位（infer_keys 逐列扫全行）使按记录暂存每格失效，反把 O(k) 部分扫描变每格强制全行拆分。环境无存压（空闲 ~28GB、pages/sec ~40）。
- 下一步未决：受控 A/B 隔离（-NoBaseline 旧新交错）确认 C 的净符号、回退 C 保留 A、或改走引擎侧相位融合/常驻字段索引，均需主 session 与用户批准；本切口代码现状保留未 commit。

探针归因与回退（同日晚间，协调者批准的受控隔离；8 个 `-Runs 1 -NoBaseline` 探针批，raw 在 `target/memory-audit/gui/probe-<态>-<轮>-*` 标注非正式，未写历史）：四态（S0=HEAD 旧、S1=仅 A、S2=仅 C、S3=A+C）交错 2 轮，lazy ready/相位（2 样本均值，ms）：S0 ready 7223（prepare 1228 / hashes 262 / row-text 2112 / verify 510 / compare 1269）；S1 7234（1251/271/2164/495/1211）；S2 8022（1652/384/2185/678/1361）；S3 8259（1696/364/2250/707/1390）；同探针 dense ready 负载指数 S0 5220 / S1 5032 / S2 5279 / S3 5625。**结论：A 净中性（全相位差在 ±60ms 噪声内），C 全相位一致净负（ready +799~+1036ms，列主序相位尤甚）**——「空覆盖层 60M 次哈希查找 + 40M 次行首重扫是热点」的本轮假设在本行形（~70B/10 列短行）下被实测证伪：列主序相位（infer_keys/verify 逐列扫全行）使按记录暂存每格失效，反把 O(k) 部分扫描变每格强制全行拆分+TLS HashMap 开销；行主序相位中短行部分重扫已低于 TLS 开销。据此 A、C 均已回退，`demand.rs` 回 HEAD（git diff 干净），正式批 batch-20261108-004948 的 A+C 测量作为已证伪切口的证据保留；上述两处测量工具修正（TrimEnd、采样后端）保留。引擎侧相位融合/常驻字段索引等方向需另行批准。

### 引擎链三相位分块并行批次（batch-20261108-102043，ready 门通过）

切口（经协调者批准，用户已批准动共享引擎代码；不改 demand.rs、不切默认、不 commit）：把三个可保序并行的链相位从 1-2 线程扩为分块并行，dense 同码受益，语义逐字节不变。① `align.rs::row_text`：按行区间分块（每侧 min(8, available_parallelism, 行数/4096) 个 scope 线程），逐格 display、256 字符截断、\r\n→空格逻辑抽为共享 per-row 函数，输出按块序拼接（先算总长一次分配，逐块 extend 后即释放，峰值不高于原单缓冲增长瞬态）；两侧改为先后串行各用满相位预算（原为两侧各 1 线程并发），staging 不叠加。② `map.rs::compare_alignment_data` 主循环：按对齐行区间分块并行算 `compare_data`，产出按位置索引的 packed 字节（每格 u8 + 每行 aggregate u8）与块级 column_states/summary 局部 merge；`PackedStates`/`rows` 装配在并行段之后单线程按块序完成，`column_states`/`summary` 为逐字段 max 语义与顺序无关；泛型 bound 加 `Sync`（`Sheet`/`DemandSheet`/`SheetStore` 均满足）。③ `align.rs` verify-filter 循环：按行区间分块并行过滤，`keys_equal`/`anchors` 只读共享，`filtered` 按块序拼接。三相位 cancel 检查均保持每 1024 行粒度；线程上界助手 `phase_threads` 加在 `perf.rs`（min(8, available_parallelism, 行数/4096)，<8192 行保持原单线程路径）；scope 线程，无新依赖，无常驻结构。

结果（exe `8ce6e4af58684116e332327b60abf56bd053658762f162502fc61d4d502d85ad`，rev 883deab dirty，3 轮中位，MiB）：

| 路径 | ready 中位 | 稳定PB | 稳定WS | 稳定PWS | 加载观测PB峰值 | 观测PB峰值 | OS WS峰值 |
|---|---|---|---|---|---|---|---|
| dense | 5451.1 ms | 1798.2 | 1706.0 | 1672.2 | 1797.9 | 1797.9 | 1963.6 |
| lazy | 4671.7 ms（**×0.8570，过 ≤1.10 门**） | 161.3 | 266.0 | 100.0 | 161.0（采样盲窗，同前批口径） | 161.0 | 523.8（>300 未达标项，累计计数器口径） |

- 正确性门禁：6 轮 fingerprint 全等冻结统计（rows=1,000,000 same=990,000 important=10,000，余为 0）；输入 SHA256 同前；exit=0、`ClosedByMainWindow` 全部成立；历史已追加 `tools/demand_gui_baseline.json`；raw 在 `target/memory-audit/gui/batch-20261108-102043/`。
- 相位（lazy 本批中位 vs 上批探针 S0，ms）：prepare-rules 1269.2 vs 1228（本轮未动）；hashes 315.0 vs 262；row-text 868.3 vs 2112（~2.4×）；verify-filter 106.2 vs 510（~4.8×）；compare 248.6 vs 1269（~5.1×）。同批 dense 相位（中位）：prepare 373.2 / hashes 185.9 / row-text 257.2 / verify 42.5 / compare 313.8 —— dense 同码受益且语义不变。第 3 轮两路径均有系统性偏慢（lazy verify/compare 322.2/712.5，dense row-text 912.0，ready 同向抬高），中位数不受影响。
- worker 口径（中位）：table-align-job lazy 2838.6 vs dense 1258.6 ms（链 delta 由 ~+4.0s 降至 ~+1.6s，主体剩余为 prepare-rules 的 infer_keys 全列扫描）；load-side 两側并行 lazy ~1595 vs dense ~3949 ms，加载净省 ~2.35s 覆盖剩余链 delta，故 ready 比值 <1。
- 引擎链四摘要离线复核（demand_proto 示例，1M 行真实输入，reference/demand 两模式）：rows 092d09a4649bbfc5 / cells 612baef10de64f25 / values 41bbcbeeca0830a5 / columns 5513a3fb0a7609e5 均与冻结值一致。
- 内存验收口径：稳定三项（PB 161.3 / WS 266.0 / PWS 100.0 MiB）<300 维持达成；加载观测 PB 峰值受采样器首样本 ~1-1.5s 盲窗限制等于稳定值（沿用前批注记，不表示加载峰值真实下降）；OS WS 峰值 523.8 MiB >300 仍为如实记录的未达标项。
- 测试：`cargo test --release -p bc-engine-table` 全绿；`cargo test --release -p bc-gui lazy`（3 项 lazy_tests）全绿；`tools/demand_measure_tests.ps1` 全 PASS。
- 未做（按本轮边界）：infer_keys 融合/剪枝（prepare-rules 1269ms 是剩余最大相位，下一轮候选）、regional interpret 快拒、demand.rs 改动、默认开关切换、imara-diff；未 commit。

### interpret 数字门快拒 + infer_keys 融合单列扫描批次（batch-20261108-105650，回归门保持）

两个独立切口（均为引擎共享代码，经批准；不改 demand.rs、不切默认、不 commit）：

- **切口 1：`regional.rs::interpret` 数字门快拒**。`parse_number` 成功要求 normalized 串含至少一个 ASCII 数字（其字符全部来自原文子串；"nan"/"inf" 结构性不可能且 f64 解析有 `is_finite` 过滤），`parse_date` 要求三个 i32 整数字段 ⇒ 不含数字的文本只可能是 Bool/Text。interpret 内以 `bytes().any(is_ascii_digit)` 门包住两次解析调用，数字文本调用顺序不变，Bool/Text 尾部两臂共享。等价性单测 `regional::tests::digit_gate_matches_ungated_interpretation`：以改动前链路为 oracle，对普通文本/含分隔符文本/true-false 各种大小写/空串/数字/日期/混合共 49 条用例 × 默认与欧洲区域（`,` 小数 `.` 千分 `/` 日期 DMY）逐一断言相等，并抽查快拒类落点。
- **切口 2：`inference.rs` 单列候选融合扫描**。原 score_wave 对 10 个单列候选各做一次全表 pass（每侧 10 pass × 1M 行，4 线程）；改为每侧一次融合 pass：按行分块（`phase_threads` 上界 8，行数/4096），块内每行对每个候选调同一 `row_hash_into`/`finish_row_hash`（逐字节同输入），写入按 (候选, 行) 索引的 `Vec<u64>` 数组（`chunks_mut` 安全切分不相交区间，无 unsafe）；随后**逐候选串行**从数组按行序重建 KeyMap（`or_insert` 首现、ScoreScratch 同容量策略、确定性 `U64KeyBuild`=`BuildHasherDefault<U64KeyHasher>` 固定乘混无随机 seed ⇒ map 布局/迭代序与 values_into 逐位一致，`score_maps` 提取的评分体逐字未改，float 累加顺序不变），每候选评分后数组即释放。pairs 波次保持原 score_wave 不动。candidates 顺序 = 列表顺序，`best()` tie-break 输入逐位不变。等价性证据：两模式四摘要冻结值全等（含 columns_digest 5513a3fb0a7609e5 = key 选择结果一致）。融合暂态 = 2 侧 × 10 候选 × 1M × 8B ≈ 160MB + 评分期一对复用 map ≈ 71MB ≈ 231MB，不高于原 4 线程 × 2 map ≈ 250-285MB 暂态。

结果（exe `96f58ec46066be6011b967359a6f65eb000163de61820dd4b2aee1c05249f944`，rev 896ef66 dirty，3 轮中位，MiB）：

| 路径 | ready 中位 | 稳定PB | 稳定WS | 稳定PWS | 加载观测PB峰值 | 观测PB峰值 | OS WS峰值 |
|---|---|---|---|---|---|---|---|
| dense | 4795.3 ms（上批 5451.1） | 1793.7 | 1701.3 | 1673.0 | 1793.7 | 1793.7 | 1964.9 |
| lazy | 4610.1 ms（上批 4671.7；**×0.9614，≤1.10 回归门保持**） | 161.1 | 266.7 | 100.5 | 161.1（采样盲窗，同前口径） | 161.1 | **523.8（与上批 523.8 持平，未变差**；>300 未达标项口径不变） |

- 正确性门禁：6 轮 fingerprint 全等冻结统计；输入 SHA256 同前；exit=0、ClosedByMainWindow 全部成立；raw 在 `target/memory-audit/gui/batch-20261108-105650/`。
- 相位（lazy 本批中位 vs 上批，ms）：**prepare-rules 815.4 vs 1269.2（−454ms，切口 2 主效）**；hashes 377.9 vs 315.0（三轮 412.8/377.9/279.7，噪声内）；row-text 829.9 vs 868.3；verify-filter 109.4 vs 106.2；compare 260.4 vs 248.6。dense 同批：**prepare 189.0 vs 373.2（−49%）**，hashes 195.9 / row-text 255.2 / verify 28.9 / compare 106.2（上批 compare 中位 313.8 为噪声抬高，本批三轮 102-109 一致）。
- worker 口径（中位）：table-align-job lazy 2472.8 vs dense 869.3 ms（上批 2838.6 vs 1258.6）；load-side 两側并行 lazy ~1678 vs dense ~3583 ms。
- 切口归因（未做 -NoBaseline 探针分离，非强制）：prepare-rules 的 −454/−184ms 与融合扫描直接对应；interpret 数字门在本行形（数字为主）各相位移动均在噪声内——其价值在文本主导输入跳过两次解析尝试，本输入不可见，等价性由单测与四摘要保证。
- 离线复核（demand_proto 两模式，1M 行真实输入）：rows 092d09a4649bbfc5 / cells 612baef10de64f25 / values 41bbcbeeca0830a5 / columns 5513a3fb0a7609e5 全等冻结值；示例内 infer_keys 阶段 demand 模式 ~1545ms → ~809ms。
- 测试：bc-engine-table 全绿（含新 regional 等价性单测）；bc-gui lazy 3 项全绿；demand_measure_tests 全 PASS。
- 未做（按边界）：demand.rs、候选裁剪/提前退出、默认开关、测量脚本、imara-diff；未 commit。剩余 lazy 链 delta（align-job +1.6s）主体转为各相位逐格取值成本（row-text +575ms / prepare +626ms / hashes +182ms vs dense），已过回归门，后续是否继续由主 session 决定。

### interpret 分配消除 + infer 计分并行批次（batch-20261108-113915，回归门保持）

两个切口（引擎共享代码，经批准；不改 demand.rs、不切默认、不 commit）：

- **切口 1：`regional.rs` 分配消除（oracle 等价）**。`parse_date` 的 `split().collect::<Vec>`（日期三段 + 时间字段两处）改为迭代器逐项取出——不含分隔符/段数不符时不分配直接 None，接受集与原逐一相同；`parse_number` 的 `String::with_capacity` 移到全部结构性校验（fraction 数字、thousands 分组长度与数字、integer 全数字）通过之后，校验改为无分配迭代器形式，所有失败路径均返回 None 故校验相对顺序不影响结果。针对本输入 5 个含数字文本列（`item0`/`note 0` 等）原每次 interpret 白付 parse_number 结构失败 + parse_date 两次 Vec 分配。等价性证据：`regional::tests` 新增两个测试——`rewritten_parsers_match_originals_exhaustively` 把改动前的 parse_number/parse_date 原样复制为 oracle，对 12 字符结构字母表全长度 ≤4 串（22,621 条）× 两种区域设置逐一断言 parse_number/parse_date/interpret 新旧全等；`allocation_free_parsers_keep_values` 手工值锚定（千分位/指数/符号/边界拒绝/日期+时间，date_ms 未动作为日期 oracle）；`digit_gate_matches_ungated_interpretation` 扩充用例后其 oracle 链路改调原版 parser，同时覆盖两个改动。
- **切口 2：`inference.rs::score_wave_fused` 计分并行**。原逐候选串行重建 KeyMap 改为候选间并行：scope 线程、预算 `min(phase_threads(行数), 候选数)`（1M 行 → 8 → chunk=2 实取 5 worker），每 worker 自带 ScoreScratch map 对，候选结果按块序拼接 = 列表顺序（`best()` 看到相同序列）；每候选内部仍按行序 or_insert 重建（布局/迭代序/float 累加序逐位不变，论证同迭代 3）。
- **观测性**：新增 env-gated span `key-infer:fused-pass` / `key-infer:scoring`（MATCHA_TABLE_PERF 关闭时零开销，与既有 span 同机制）。

结果（exe `9909cc4ed679ad1d9ccde8e6d17ed7abf55bffdeaa61d701eb6478f41cb399c6`，rev 4a0124e dirty，3 轮中位，MiB）：

| 路径 | ready 中位 | 稳定PB | 稳定WS | 稳定PWS | 加载观测PB峰值 | 观测PB峰值 | OS WS峰值 |
|---|---|---|---|---|---|---|---|
| dense | 4519.9 ms（上批 4795.3） | 1798.8 | 1706.1 | 1672.1 | 1798.8 | 1798.8 | 1962.9 |
| lazy | 3761.1 ms（上批 4610.1；**×0.8321，≤1.10 回归门保持**） | 162.0 | 266.5 | 100.3 | 162.0（采样盲窗，同前口径） | 162.0 | **524.0（对照上批 523.8，+0.2 未变差，无需降 4 线程**；>300 未达标项口径不变） |

- 正确性门禁：6 轮 fingerprint 全等冻结统计；输入 SHA256 同前；exit=0、ClosedByMainWindow 全部成立；raw 在 `target/memory-audit/gui/batch-20261108-113915/`。
- infer 两分段（本批中位，ms）：fused-pass lazy 535.2 vs dense 70.0；scoring lazy 55.7 vs dense 67.0（dense 侧 map 插入主导、计分不小于取值；lazy 侧计分并行从串行 ~250ms 量级降至 55.7）。
- 相位（lazy 本批中位 vs 上批，ms）：prepare-rules 595.1 vs 815.4（−220，= fused 533→535 持平 + 计分 ~250→56）；hashes 276.9 vs 377.9；row-text 693.3 vs 829.9（−137，分配消除在 5 个含数字文本列的收益）；verify-filter 87.3 vs 109.4；compare 264.1 vs 260.4（平）。dense：prepare 152.1 vs 189.0；hashes 187.2；row-text 232.9；verify 29.5；compare 104.9。
- worker 口径（中位）：table-align-job lazy ~1966 vs dense ~856 ms（上批 2472.8 vs 869.3）；load-side 两側并行与上批持平。
- 离线复核（demand_proto 两模式）：四摘要全等冻结值；示例内 infer_keys demand 模式 fused 533.2 + scoring 69.1。
- 测试：bc-engine-table 全绿（含 3 项 regional 等价/锚定测试）；bc-gui lazy 3 项全绿；demand_measure_tests 全 PASS。
- 内存结论：5 worker × 2 map（~36MB/个）+ 160MB 融合数组的计分暂态未抬高 OS WS 峰值（524.0 vs 523.8，噪声内）——峰值不由 infer 相位设定，维持 8 线程预算，未启用 4 线程回退。
- 未做（按边界）：候选裁剪/提前退出、infer 与 align hash 编码统一、hashes 相位两遍并行化（留作后续候选）、demand.rs、默认开关、测量脚本、imara-diff；未 commit。剩余 lazy-only 相位差：fused-pass +465ms / row-text +460ms / hashes +90ms / compare +159ms（对 dense），均为逐格取值成本，已过回归门。
