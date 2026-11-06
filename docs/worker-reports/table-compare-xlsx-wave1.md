# table-compare-xlsx — Wave 1 实施报告

## 结果：succeeded（仅任务组 1–3）

`tasks.md` 的 **1.1–1.5、2.1–2.4、3.1–3.5 全部完成**，OpenSpec **14/23**；任务组 4–6 未开始。工作于 master；没有进入文件夹线 worktree，没有读取/修改 vault，也未提交或删除仍未跟踪的 `openspec/changes/folder-compare-sync/`。

| 最终验证 | 结果 |
|---|---|
| `cargo clippy --workspace --all-targets -- -D warnings` | 退出 0，零警告 |
| `cargo test --workspace` | **164 通过，0 失败，2 个压力测试默认忽略** |
| `cargo test --release -p bc-engine-table -- --ignored` | 两个压力测试**实际执行并通过**，0 忽略 |
| `cargo build --release -p bcompare` | 退出 0 |
| `openspec validate table-compare-xlsx --strict --json` | valid=true，0 issues |
| `git diff --check` | 通过 |

引擎增加 40 个常规测试及 2 个显式压力测试；格式层增加 2 个测试。原有文本/格式/GUI 回归没有改断言或削弱测试。

## 完成清单与证据

测试均在 `crates/bc-engine-table/tests/`，除特别注明外。

| 任务 | 实现/验证 |
|---|---|
| 1.1 | `model.rs` 列式 Sheet/Cell、共享列快照、类型/公式/样式去重/合并范围；`sources::model_is_columnar_and_snapshot_isolation_and_styles`。格式层 `bc-formats/tests/table_options.rs` 两测与原 `xml_roundtrip` 六测全部通过，25 格式语义、Node/STX、文本往返保留，新增缺省 XML 字段不写出 |
| 1.2 | `source.rs` RFC 引号/多行/转义、分隔符集合/连续/空白/检测、Unicode 字符固定宽度；`sources` 对应场景通过，1 GiB synthetic stream 262,144 条记录逐条消耗、只持单条记录缓冲，**4.69 s** |
| 1.3 | 自动/强制表头、显式或宿主注入区域、严格千分组、小数与日期、编码完整 UTF-8 流式验证、UTF-16/ANSI、Ctrl+Z；跨 8192-byte UTF-8 边界、EOF 后二进制垃圾、欧式数字、日期测试通过 |
| 1.4 | `workbook.rs` 使用 calamine cell/formula/style 与 merge API；生成 ZIP/XML 覆盖 3 sheets、懒加载坏 sheet、缓存值/公式/无缓存公式、日期/1904 epoch、布尔、错误、内置/自定义数字格式、合并范围；100 MB sheet 惰性/IO 取消压力 **0.26 s** |
| 1.5 | 保存保留未改字段原拼写、分隔符与逐行行尾，必要限定符、同目录暂存+rename；只读 workbook 与显式 CSV 导出、保存失败目标不损坏、复制数值在 Text 模式也保持拼写的测试通过 |
| 2.1 | `columns.rs/rules.rs` 比较列单侧/整列 Move、Tidy、默认 1:1、Handling/默认继承与 serde 往返；General/Text、大小写/空白、数值/日期容差在比较中生效 |
| 2.2 | `inference.rs` 完整唯一性 + D3 权重评分、相邻双列、0.8 阈值；95% id 重合、复合键、重复无键、自动标注、用户锁定/全部列/按列号预设测试通过 |
| 2.3 | 表头归一化优先、1000 行值集合/类型兜底；中间插入不飘移、98% 内容重命名、移动、删除及列数不等测试通过 |
| 2.4 | `sheets.rs` 名称配对、左右来源顺序、单侧 sheet、单侧重排与同文件不同 sheet；序列化往返通过 |
| 3.1 | `align.rs` 复用 core Quick/Smart/Patience/None、closeness、取消与锚点；重复 occurrence、稳定数值/复合键排序、容差跨桶、不同键拒配、手动锚点/清除、随机小表分区不变量全部通过 |
| 3.2 | `compare.rs` 三态 × 三层，数字格式按属性而非 StyleId 编号比较；6 bit/格、12 B/行索引/汇总；公式切档、仅格式、混合行、孤儿、sheet 色点及 Ignore Unimportant 原始状态保留测试通过 |
| 3.3 | `map.rs/jobs.rs` 派生行/导航/统计/过滤、已应用 EditItem、复制单元格/整行/空侧、插入后锚点移位、撤销/重做、新 revision 防 ABA、键改变重对齐、Job trait、后台初次结果安装、过期结果拒绝测试通过 |
| 3.4 | `summary.rs` 文本/CSV v1，按键、行状态、变化值/公式/格式和列结构；精确包含 `7 | price | 1.0 -> 1.2`，Ignore Unimportant、控制字符/CSV 引号验证通过 |
| 3.5 | `examples/table_bench.rs` 系统临时目录生成两份 100 万 × 10 列 CSV，读/推断/后台 Smart/比较/取消实测如下；退出后目录已确认清理 |

## 百万行最终实测

命令：`cargo run --release -p bc-engine-table --example table_bench`。Windows，本机 Rust 1.99.1，release；**preserve_source=true**，未关闭词法保存来美化内存结果。

两份 CSV 各 **69,258,868 B**，各 1,000,000 数据行 × 10 列（表头另计）。只有每 100 行一个 amount 值变化；最终核对 **990,000 Same / 10,000 Important / 0 orphan**，自动键第 1 列得分 1.0。每阶段计时不包含后续进程内存查询；RSS/PeakWorkingSet64 通过系统进程计数读取。

| 阶段 | 时间 ms | 阶段结束 RSS B | 进程累计峰值 RSS B |
|---|---:|---:|---:|
| 生成临时输入 | 659.575 | 4,669,440 | 4,669,440 |
| 读取左侧 | 3,093.567 | 843,739,136 | 843,739,136 |
| 读取右侧 | 3,116.002 | 1,682,251,776 | 1,682,251,776 |
| 键推断 | 2,871.882 | 1,682,268,160 | 1,753,575,424 |
| 后台 Smart 对齐 | 2,153.301 | 1,772,138,496 | **2,014,801,920** |
| 单元格比较及计数核对 | 1,281.932 | 1,703,636,992 | 2,014,801,920 |
| 发出取消 → worker join | **0.749** | 1,703,645,184 | 2,014,801,920 |

取消测试在后台执行真实百万行 TableAlignJob，启动 10 ms 后取消，得到 `CoreError::Cancelled`，不发布部分结果；是整个键准备/对齐/比较流水线的取消验证，不宣称特定 core 内部步骤的独立延迟。界面滚动/p99 属于 4.x，本波没有 GUI 性能宣称。

### 内存构成与后续优化基线

峰值约 **2.015 GB / 1.876 GiB**。这不是只有位图的体积，亦不是单个文件的内存；以下为结构尺寸解释，非堆 profiler 的精确归因：

- 两侧 2,000 万 Cell，每格 **48 B**，逻辑单元格载荷约 **960 MB**（尚不含 Vec capacity）。
- `preserve_source=true` 额外保留逐记录文本/字段 spans/行尾/Arc 与分配器开销；用于不改写未编辑字段，读取两侧后进程 RSS 实测约 **1.682 GB**。
- 键推断/对齐期间有两侧键哈希与 occurrence 索引、排序排列、core AlignMap 行/反向索引、Smart 的有界行特征输入；这些临时结构造成峰值上升，并在比较后释放。
- 最终比较行索引/汇总 **12,000,000 B**，状态位图 **7,500,000 B**，不是每格三个 enum 或每行 Vec。
- 后续可优化 lexical 保存的 spans 容量/布局、读取时更紧凑的源存储、Smart 文本准备与临时分配复用。当前数字作为基线完整保留，未通过丢弃词法信息压低报告值。

最终基准原始关键输出：

```text
phase=read_left elapsed_ms=3093.567 rss_bytes=843739136 cumulative_peak_rss_bytes=843739136
phase=read_right elapsed_ms=3116.002 rss_bytes=1682251776 cumulative_peak_rss_bytes=1682251776
phase=infer_keys elapsed_ms=2871.882 rss_bytes=1682268160 cumulative_peak_rss_bytes=1753575424
key_selection=Automatic { columns: [0], score: 1.0 }
phase=align_background_smart elapsed_ms=2153.301 rss_bytes=1772138496 cumulative_peak_rss_bytes=2014801920
phase=compare_cells elapsed_ms=1281.932 rss_bytes=1703636992 cumulative_peak_rss_bytes=2014801920
same_rows=990000 important_rows=10000 orphan_rows=0
cell_bytes=48 compact_rows_bytes=12000000 packed_states_bytes=7500000
phase=cancel_to_join elapsed_ms=0.749 rss_bytes=1703645184 cumulative_peak_rss_bytes=2014801920
cancelled=true temp_cleanup=on_drop
```

## 新增依赖与 MSRV

唯一新**直接**第三方依赖：workspace `calamine = { version = "0.38", default-features = false }`，仅 bc-engine-table 引用。沿用已有 workspace 的 `encoding_rs`（增量解码）、`serde`（规则模型）与 dev `serde_json`；无新 GUI、运行时、日期或 ZIP dev 依赖。

calamine 带来 11 个新增锁包：`calamine 0.38.0`、`atoi_simd 0.18.1`、`byteorder 1.5.0`、`codepage 0.1.3`、`debug_unsafe 0.1.4`、`fast-float2 0.2.4`、`nix 0.31.3`、`quick-xml 0.42.0`、`typed-path 0.13.0`、`zip 9.4.0`、`zopfli 0.8.4`。现有 quick-xml 0.41 依赖不变，0.42 是 calamine 的传递版本。

调查 0.30.1 后发现 0.38 新公开数字样式 API；协调者明确要求优先新版并批准 workspace MSRV **1.80 → 1.88**，根 README 只修改此构建要求说明。本机 1.99.1 全量验证；没有宣称用 1.88 工具链重新跑过。

## 接口调整

- 原骨架 TableModel 的行式字段替换为 `sheets: Vec<Sheet>`；Sheet 列式存储、可复制快照、私有 Cell 列与受校验 mutators。无仓库内旧使用者需要迁移。
- `TableModel::parse_delimited` 现在返回 Result；通用入口为 `source::read_text/read_path(ReadOptions)`，返回 Sheet 与表头/编码/混合类型诊断。输入流不整体 read_to_end；只有最长记录缓冲与输出模型，编码 sniff 上限 8 KiB/检测候选前缀 64 KiB。
- `Workbook::from_reader/open` 接收 Arc<dyn CancelCheck>；`load_sheet` 只加载当前 sheet，保留共享字符串元数据但不加载其他 sheet 值。xlsx/xlsm 允许内存编辑，保存原格式报只读；显式 `export_csv` 是独立出口。
- `TableRules` 拥有映射、默认/逐列处理、KeySelection、策略与三层档位；默认打开推断键，用户 Manual/AllColumns/ByColumnNumber 不被建议覆盖。日期容差内部毫秒，days/hours 明确构造；system locale 由宿主传入，默认 invariant。
- `KeyAlignment::map` 的 core 索引指向排序排列；使用 left_order/right_order 回源行，不能把排序 map 当源索引。
- `TableMap::row/rows` 派生访问，`cell_diff` 保留原三态；规则通过 rules/rules_mut 访问。Layers 与 Ignore Unimportant 仅投影，不使缓存失效；其他语义规则/内容/锚点变化触发 stale。
- `TableAlignJob/TableRealignJob` 接收 Send+Sync Sheet 快照，不发送 Rc/RefCell 编辑目标。初次 job 可在后台解析默认列/键；`TableMap::from_output` 无重复重算安装初次结果。后续 `set_alignment` 验证双侧 revisions、比较规则与锚点。Realign 当前是安全的全量回退。
- `copy_cell/copy_row/modify_cell` 返回已应用 EditItem，调用方推入目的侧 EditStack；还原同时更新全局唯一 revision，拒绝 ABA 旧结果。同步方便入口会重对齐；GUI 大表应调用直接编辑 + 后台 RealignJob 路径，不在 update 中调用同步重算。
- 摘要是转义的 UTF-8/CSV 输出协议，源载荷不是指令；不提供导入编辑/AI 执行语义。

## 协调者批准的范围例外与 spec/design 建议

决定均记入 [ADR 201](../decisions/201-table-source-and-alignment.md)。

1. Smart closeness 后仍拒绝不同键配对，只有 Unaligned 和手动锚点例外；遵守 row spec，而不是将“相似”视作相同键。
2. 正容差不具传递性；相邻桶找候选并逐一复核，按出现顺序消耗。极低选择性的多列容差最坏可能较慢；重复键计数是**精确规范化重复数**，不是非传递容差的“等价类”计数。
3. 最新 calamine 实际可提供数字格式属性，已读取，不止“日期/普通数字”类别；仍仅比较数字格式，**字体/填充未实现**，不声称通过 spec 中“仅加粗”的原例。该例按 design D2 的明确后置处理。
4. D2 概述列出 xls/ods，但 proposal/source spec 的明确验收为 xlsx/xlsm；协调者确认本波仅后者。xls/ods 的惰性/合并范围适配未实现、未验证，已在 README 明示，建议后续修正 D2 文字范围。
5. 新增 TableOptions 的 XML 属性名称是项目扩展契约；25 个内置的兼容往返已严格验证，但尚未用参考工具原生保存 Type/Regional 设置来证明这些新增属性名的互操作。建议 5.x 黄金/格式互操作阶段核实，不把自己的 roundtrip 等同于原生写兼容。
6. 日期容差帮助未指明单位，内部毫秒并暴露天/小时；不访问平台全局区域，宿主注入 system regional。无悬而未决的方向性阻塞。
7. MSRV 提升激活共享代码的 Clippy 建议，协调者批准**唯一跨原 ownership 的例外**：独立 `8132e52` 机械 lint 提交，无语义修改、无测试修改、无 allow：
   - `bc-core/src/encoding.rs`：chunks_exact(2) → as_chunks::<2>().0.iter()。
   - `bc-core/src/persist.rs`：两处 repeat().take() → repeat_n()。
   - `bc-formats/src/grammar.rs`、`registry.rs`：map_or(true, …) → is_none_or(…)。
   - `bc-gui/src/bench.rs`：余数零判断 → is_multiple_of(4)。
   - `bc-gui/src/home.rs`：chunks_exact(2) → as_chunks::<2>().0.iter()。
   - 文件夹分支合并时需留意这些共享文件的 rebase；未进入其 worktree。bc-engine-text、bc-session **零修改**。
8. 参考帮助仅从指定 CHM 只读解包到系统临时目录，阅读 D9 列出的 Type/Regional/Columns/Alignment/Sheets/Handling/命令主题。没有运行参考程序、写安装目录、访问授权/密钥文件或私人配置，未借此启动 5.x 黄金工作。

## 提交

全部提交均含 `Co-Authored-By: pi <noreply@pi.dev>`。

| 范围 | commit |
|---|---|
| 第一步：仅 table-compare-xlsx change | `1581467ac6c6a07f42ab5d5a04c9966db6e75060` |
| 任务组 1 | `6d4cac049c6417848332a94e48e997a019063b52` |
| 获批 MSRV 机械 lint 例外 | `8132e525dd768a38d11619f6b6366ff1c5eaab56` |
| 任务组 2 | `1a811ffea838c396591ad76a4b95dba718dcb024` |
| 任务组 3 | `1f64c75fe81beda81e162ce38556c297e27339d2` |

本报告另外作 docs 提交。下一波剩余 4.x GUI、5.x 黄金/逐项公开行为对照、6.x 最终集成；没有提前勾选或实现这些任务。

## 验证最后 10 行（不足 10 行列全部）

### cargo clippy --workspace --all-targets -- -D warnings

最终复跑复用编译缓存，整个输出只有一行；其前完整重检同样零警告。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.51s
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
   Compiling image v0.25.10
   Compiling calamine v0.38.0
   Compiling eframe v0.37.0
   Compiling bc-engine-table v0.1.0 (D:\Github\Matcha\crates\bc-engine-table)
   Compiling bc-session v0.1.0 (D:\Github\Matcha\crates\bc-session)
   Compiling bc-report v0.1.0 (D:\Github\Matcha\crates\bc-report)
   Compiling bc-gui v0.1.0 (D:\Github\Matcha\crates\bc-gui)
   Compiling bc-script v0.1.0 (D:\Github\Matcha\crates\bc-script)
   Compiling bcompare v0.1.0 (D:\Github\Matcha\apps\bcompare)
    Finished `release` profile [optimized + debuginfo] target(s) in 13.28s
```
