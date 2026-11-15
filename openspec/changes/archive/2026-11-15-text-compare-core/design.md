# Design

## Context

动机见 `proposal.md - Why`。当前状态：

- 14 crate 骨架已就位（见根 `README.md`），依赖方向 apps → bc-gui/bc-script → bc-session → engines → bc-formats/bc-vfs/bc-jobs → bc-core；所有算法为 `todo!()`。
- 已实现且可依赖：`bc_core::persist::parse_xml`（Delphi XML 方言读取）、`bc_formats::builtin_formats()`（25 种格式嵌入）、`bc-gui` 原型（mmap + 行索引 + `show_rows` 虚拟滚动，两侧各 110 MB 60 fps，见 `docs/ui-framework-decision.md`）。
- 参考材料：vault `luckey/03_projects/bcompare-clone/*.md`（规格与分析笔记）、`luckey/09_generated/bc5-analysis/`（`help-html/` 帮助全文、`vmt_report.txt` 类名、`rt_strings.txt` 文案）。本仓库不改动这些文件。
- 约束：引擎 crate 不依赖 GUI；无异步运行时；UTF-8 源文件；路径用 `std::path`；clippy 零警告；不实现任何授权逻辑；不在产物中使用参考产品名称。

## Goals / Non-Goals

**Goals:**

- 文本比较从"可打开"到"可日常使用"：对齐、分级、着色、编辑、保存、查找、导航全部可用，行为以参考工具为准。
- 为后续 change 留下稳定接口：`AlignMap`/`TextMap` 可被表格引擎与 3 路合并复用；`TextMap` 可无损导出为文本冲突包（AI 合并输入）。
- 黄金语料体系跑起来，使后续算法调整有回归保护。

**Non-Goals:**

- 不做 3 路合并、不做任何 agent 接入（只预留数据导出接口）。
- 不做会话持久化、CLI 解析、报告输出、文件夹视图。
- 不追求与参考工具字节级一致的 XML 写出（本 change 只读导入格式文件）。
- 不在本 change 内实现 `TBcSmartAlign` 的精确评分复刻；采用黑盒逼近 + 语料回归，评分细节由反汇编结论在后续迭代修正。

## Decisions

### D1. 行哈希 + Myers 作为 diff 基线，Smart 对齐作为后处理层

- 选择：先用标准化后的行哈希（xxHash64）跑 Myers O(ND)（带公共前后缀裁剪、超大输入时分块），得到 LCS 骨架；Smart 对齐在每个差异区块内做二次配对（相似度打分 + 权重锚点 + 偏移容忍），Quick 对齐直接采用 LCS 骨架。
- 理由：与参考工具"`TBcQuickAlign` 是 `TBcSmartAlign` 的退化形式"的类继承关系一致；两档共享一套 diff 核心，语料可以分别校验。
- 备选：直接实现 patience diff 作为 Smart（参考工具有 `TBcPatienceDiffAlign` 子类）。保留为策略枚举值，不作默认。
- 相似度：先用字符级 LCS 比率（长行截断）+ 词级 Jaccard 的加权，阈值可配（`SmartAlignOptions`），以语料调参。

### D2. 重要性在分词层计算一次，向上传播

- 选择：每行分词结果（元素区间）缓存于内容侧；行对比较时按元素区间做"掩码后比较"，得到行三态；区块三态由行三态传播。开关变化（忽略空白、元素重要性）只需重算掩码，不需重新分词。
- 理由：100 MB 文件下重新分词是 O(文件)，而掩码重算只涉及差异行。
- 备选：把重要性折进行哈希（一个开关一套哈希）。会使 Smart 对齐与着色耦合，放弃；但"忽略空白/大小写"确实需要进入对齐用哈希，故 `LineHasher` 的标准化标志与重要性开关保持一一映射。

### D3. 分词器：按格式编译一次，支持跨行状态

- 选择：`Tokenizer` 把语法项编译为有序匹配器（Basic/Delimited 的正则用 `regex` crate，List 用按首字符分桶的词表，Columns 直接切片）；逐行扫描，Delimited 的未闭合状态作为行状态传递给下一行；视图只对可见行及其前导状态链惰性分词，后台任务负责全量分词以支撑统计。
- 理由：参考工具的 `TTextTokenizer` 同样是逐行 + 行状态模型；惰性分词保证大文件打开速度。
- 备选：tree-sitter。表达力过剩且与 25 种内置格式的语法项模型不对应，放弃。

### D4. 正则方言：`regex` crate + 兼容垫片

- 选择：`bc_core::pattern` 用 `regex` 实现；对参考工具语法中 `regex` 不支持的记号（如 `\<` `\>`）在编译前做等价改写；不支持的回溯特性报错并提示。
- 理由：内置 25 种格式的全部正则已核对可被 `regex` 接受（build 阶段可加断言）。
- 备选：`fancy-regex`（支持回溯）。性能不可控，仅在用户格式确需时再考虑。

### D5. 编码：`encoding_rs` 负责 ANSI，BOM/UTF 自己处理

- 选择：UTF-8/16 检测与 BOM 由 `bc_core::encoding` 实现；ANSI 代码页解码/编码交给 `encoding_rs`（Windows 代码页 → WHATWG 标签映射表内置）。内容在内存中以 UTF-8 字节保存，行尾逐行记录。
- 理由：零 unsafe、纯 Rust、覆盖常见代码页；不引入 ICU。
- 备选：Win32 `MultiByteToWideChar`。平台绑定，放弃。

### D6. 大文件内存模型：mmap 只读 + 写时拷贝的行表

- 选择：未编辑时 `LineList` 的缓冲区可以是 mmap 的只读视图；首次编辑把文件切换为"分段缓冲"（rope 风格的块数组），行表按块维护偏移。
- 理由：满足"100 MB 可开可滚"与"禁止整文件多次复制"；编辑是少数场景。
- 备选：一开始就用 rope。实现成本高且纯比较场景多付代价，放弃。

### D7. 自绘文本控件而非 egui `TextEdit`

- 选择：在 `bc-gui` 实现专用控件：按 `TextMap` 行渲染字形行，自己管理光标/选区/IME 提交、键盘编辑映射到 `EditStack` 项；只绘制可见行；导航条独立控件。
- 理由：`TextEdit` 不能虚拟化，也无法表达对齐空隙与双栏联动；参考工具同样是自绘（`TUiTextEditor`）。
- 风险见下。

### D8. 重新对齐策略：编辑后窗口化重算

- 选择：编辑产生脏行范围 → 防抖 300–500 ms → 对脏范围前后扩展到最近的"相同"锚点行后局部重跑对齐（`Realign2Job`）；锚点找不到时退化为全量。
- 理由：满足规格"输入后 0.5 秒内刷新"且大文件不全量重算。

### D9. 黄金语料生成与格式

- 选择：语料目录结构见 `crates/bc-core/tests/golden/README.md`；期望文件由参考工具通过脚本 `text-report layout:xml` 生成（人工操作一次，产物入库），测试侧解析该 XML 取对齐行序列与三态。
- 理由：同时验证 Quick/Smart 两档与重要性，不依赖反汇编进度。
- 语料来源限制：只用合成文本或宽松许可文本。

### D10. 参考工具行为对照清单（流程决策，适用于本项目所有 change）

- 选择：每个 change 在写 specs 前必须遍历参考工具对应的帮助主题（`help-html/commands*.html`、`session*.html`、`view*.html`、`format*.html`）与功能清单，把其中设计合理的命令、选项、对话框逐项映射到需求；未采纳的项在 design 的"未采纳项"小节写明理由。本 change 的扫描范围：`commandstext.html`、`sessiontext*.html`、`viewtext.html`、`format*.html`、`grammars.html`、`sample_regular_expressions.html`。
- 理由：用户要求"不限于点名的功能，参考工具做得好的都要移过来"；对照清单把这一要求变成可检查的产物。

### 未采纳项（本 change 扫描结果）

| 参考工具项 | 处理 |
|---|---|
| Open With / Explorer 右键 | 后置到 `session-and-cli`（需要外部程序注册表） |
| Compare Files Using（切换视图类型） | 本 change 只有文本视图，接口预留，`hex-and-reports` 后可用 |
| Text Compare Report | 后置到 `hex-and-reports` |
| Lock Session | 随会话持久化一起做 |
| Line Mode / 高亮当前行的次要显示选项 | 列入 tasks 的"次要显示选项"一项，若排期不足可延后，不影响规格 |

## Risks / Trade-offs

- [自绘编辑器的 IME 与可访问性] → 先实现英文/ASCII 编辑与 egui 的 IME 提交事件，CJK 组合输入在 Windows 上做专项验证；编辑之外的所有功能不依赖 IME。
- [Smart 对齐与参考工具不完全一致] → 以语料一致率为指标（目标 ≥ 95% 用例逐行一致），差异用例单独归档，等反汇编结论再收敛；UI 提供 Quick 作为可预测的退路。
- [100 MB 文件全量分词/统计耗时] → 分词与统计在后台 job 中进行，视图先按惰性分词显示；进度显示在状态区。
- [`regex` 不支持用户格式中的回溯语法] → 导入时逐条编译并给出具体错误；文档说明支持的方言。
- [编辑后局部重对齐产生与全量不同的结果] → 提供 Recompare 做全量重算；局部重算结果与全量结果的差异限制在脏范围内。
- [新增依赖体积与编译时间] → 仅 `regex`、`xxhash-rust`、`encoding_rs`，均无传递的 GUI/异步依赖。

## Migration Plan

- 无用户数据迁移。原型 `bc-gui::text_view` 由正式控件替换，`--bench` 开关与 `bench.*` 输出格式保持不变以便对比性能。
- 回滚：功能按 crate 分层落地，任一阶段可用 git 回退到上一个可编译提交；语料测试默认参与 `cargo test`。

## Open Questions

- Smart 对齐相似度阈值与权重的最终数值（由语料调参，不影响规格）。
- 上下文模式默认行数是否取参考工具的默认值（待从 `rt_strings.txt`/偏好样例确认，当前取 3）。
- `encoding_rs` 未覆盖的少数代码页（如部分 EBCDIC）是否需要支持（当前不支持并提示）。
