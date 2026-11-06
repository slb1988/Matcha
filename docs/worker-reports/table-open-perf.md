# 表格比较（xlsx）拉起耗时：诊断与优化

任务：用户反馈表格比较拉起特别慢。测量先行定位，再迭代优化。本报告为最终交付；
原始日志与脚本全部落盘于 `target/table-open-perf/`（不入库）。

## 结论

同机同输入、交替 A/B、各 5 轮取中位（release，`MATCHA_TABLE_PERF=1` 计时）：

| 输入 | 基线（首帧可交互） | 优化后 | 提升 |
|---|---|---|---|
| 5万×10 xlsx 对（3.2MB×2） | 1610 ms | **699 ms** | 2.3× |
| 100万×10 xlsx 对（64MB×2） | 27097 ms | **7234 ms** | 3.7× |

稳态帧时间无回归（wave7 同 CSV 输入，交替 3 轮：基线 p99 1.03–1.25ms vs 优化后
1.01–1.14ms，统计等效）。结果正确性：`cargo test --workspace` 373 passed / 0 failed
（含新增真实 xlsx 并行打开回归 1 项），`cargo clippy --workspace --all-targets` 0 警告；
优化版 5 轮的比较结果指纹（rows/same/important/left_only/right_only 计数）逐轮一致。

## 阶段1：测量与定位

计时手段（已入库，`814390c`）：`MATCHA_TABLE_PERF=1` 时在 stderr 输出全链路
mark/span（进程启动→字体→窗口→worker 各段→install→首个可交互帧）；
`MATCHA_TABLE_PERF_EXIT=1` 在首帧就绪后自动关窗，便于脚本化。分析脚本
`target/table-open-perf/summarize.py`，驱动 `run_rounds.sh` / `ab.sh`。

基线 1M×10 拆分（5 轮中位，干净机器状态）：

| 段 | 耗时 | 占比 |
|---|---|---|
| 进程+窗口+字体 | ~0.45 s | 1.6% |
| **xlsx 解析（两侧串行）** | **~17.9 s** | **64%** |
| align 作业（prepare-rules/key-align/compare） | ~9.0 s | 33% |
| cache-build + install + 首帧 | <0.1 s | ~0% |

根因（代码位置 + 证据）：

1. **同一 sheet XML 被解压+解析 3 遍**：`Workbook::load_sheet`
   （`crates/bc-engine-table/src/workbook.rs`）先 `next_cell_with_style_id` 全量扫
   （values+styles），再 `next_formula` 全量扫（formulas），再
   `merge_cells_by_sheet_name` 全量扫（merges）。calamine 0.38 的 `next_*` 系列共享
   同一 XML 流且互不兼容（不存在 value+style+formula 一次返回的 API），mergeCells
   又在 sheetData 之后必须整扫。无公式无合并单元格的输入也要付出全部代价。
   证据：values-pass 4.3s + formulas 2.8s + merges 1.9s（每侧），后两遍纯浪费在
   重复解压（本测试输入无公式/无合并）。
2. **两侧文件串行加载**：`LoadJob::run` 顺序 load 左、右（worker.rs），
   两个 64MB zip 各自单线程 inflate。
3. **键推断逐候选全表哈希**（`inference.rs`）：10 个候选列 × 双侧全行哈希 +
   SipHash HashMap 插入 + 每行两次 Vec 分配 → prepare-rules 4.3s。
4. **key-align 的 closeness 文本**（`align.rs::row_text`）：每单元格
   `Cell::display()` 分配 String，10M 次分配。
5. **逐单元格比较**（`compare.rs`）：formula 比较每格分配 String；文本单元格每次
   比较都跑 `regional::interpret`（尝试解析数字/日期）。

## 阶段2：优化迭代（每轮同轮交替 A/B）

| 迭代 | 改动 | 1M×10 首帧中位 |
|---|---|---|
| iter1 | `Workbook::open` 套 4MB BufReader（syscall 降噪） | 28.3→28.0s 噪声内，保留 |
| iter2 | **values/formulas/merges 三路并行**（`load_sheet_parallel`，同文件 3 句柄）+ **LoadJob 双侧并行**（失败侧取消兄弟侧；memory/staged 路径保持原串行） | →**14.7s** |
| iter3 | inference：自写 multiply-mix u64 哈希器（xxh64 键已均匀）+ map/scratch 复用 + 免分配行哈希；row_text 走 `Cell::display_into` 复用缓冲 | →**10.9s** |
| iter4 | compare：formula 比较改 &str 免分配；raw 文本相等且区域设置相同时跳过 interpret（Text 强制类型列除外，保持 display-aware 语义）；keys_equal 同法 | →**8.6s** |
| iter5 | StringPool 换 word-wise 哈希器。**首版少了终末混合导致回退 20%**（8 字节串最后一字符只影响 hash 高 8 位而 hashbrown 用低位索引 → "item-NNNN" 全碰撞）；加 mul+xor 终末器后正常 | →**8.4s** |
| iter6 | infer_keys 候选按序分块并行（≤4 线程，结果按原序拼接，best() 平局裁决不变）；KeyAlign 的 hashes/row_text 左右并行 | →**7.3s** |

提交（master，未 push）：`814390c` 计时仪表 → `369b4a1` 并行加载 → `ef969fc`
哈希/推断 → `b9ab844` compare → `9248b3a` 字符串池 → `f22867f` 并行 align →
`24ad8a8` 指纹 → 新增回归测试。

## 验证

- `cargo test --workspace`：373 passed / 0 failed（含新增
  `open_two_real_xlsx_files_compares_via_parallel_load`：真实 xlsx 走并行路径，
  校验统计与单元格内容；既有 golden/单测断言未删改）。
- `cargo clippy --workspace --all-targets`：0 警告。
- 稳态帧基线（--bench-table 600 帧，wave7 同 69MB CSV 对）：交替 3 轮，
  update p99 基线 1.03–1.25ms vs 优化后 1.01–1.14ms，无回归。
- 结果指纹跨 5 轮一致：`large-1m rows=999980 important=990020 left_only=4980
  right_only=4980`（本测试数据为 ~100% 行有差异的最坏情况）。
- 环境提示：本机有持续背景负载且会热降频（同 exe 同输入观察到 28s 与 50s 两档），
  所有结论均基于同轮交替 A/B 中位数，不受漂移影响。

## 剩余已知边界（未做）

- values 流为单 zip deflate 流，无法按行切分；当前每侧墙钟 = max(values 4.3s,
  formulas 2.8s, merges 1.9s)。再降需要自写单遍解析（高风险，不建议本次做）。
- 大表 Find/Replace、Save/Export 前台同步路径（wave7 已记录）未动。
- 文本比较拉起未测未动。
- 进程/窗口初始化 ~0.45s（eframe/wgpu）未动。

## 产物

- 日志：`target/table-open-perf/logs/`（baseline/F-base/F-opt 各轮全量 span）
- 输入与生成器：`target/table-open-perf/inputs/`（SHA256 在 gen.log）、`gen_xlsx.py`
- exe 快照与哈希：`target/table-open-perf/baseline-exe/`
- 分析脚本：`summarize.py`、`run_rounds.sh`、`ab.sh`
