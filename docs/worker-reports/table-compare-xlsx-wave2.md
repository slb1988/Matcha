# table-compare-xlsx — Wave 2 阶段收口报告

日期：2026-11-06（本机 UTC+8）。工作区：`D:\Github\Matcha`，分支 `master`。

## 结论：部分交付，Wave 2 未完成

以 Wave 1 `ed55e66` 为基线，接续已有语义修正 `8dbafe8` 及未提交实现。本轮停止功能扩展，核查归属、验证、分组提交；除本波文件格式整理及黄金报告的无损文本封装外，没有继续补齐 GUI 功能。

**OpenSpec 为 15/23，仍有 8 项未完成，未归档。** 本轮只将证据完整的 **5.2** 勾选。**4.1–4.5、5.1、5.3、6.1 均保留未完成**；可构建、测试通过不等于整波功能验收通过。

优先风险：**未 Commit 的 Modify 草稿未进入关闭保护**；多选结构操作和剪贴板保真尚有缺口。当前 GUI 不应被描述为编辑安全性已完整验收的最终交付。

## 已提交成果及证据边界

| 范围 | 已交付内容 | 本轮证据与限制 |
|---|---|---|
| 引擎编辑支撑 | `editing.rs` 的行列结构操作、粘贴；`TableMap::edit_sheet`、deferred edits、修订号检查；源列映射、编辑行标记、列/整表摘要及有界预览 | `deferred_editing.rs` 2 测通过：旧 job 拒绝、后台安装、undo/redo 与投影保留、结构编辑及保存往返；不等同于 GUI 全操作验收 |
| 表格视图与接线 | `bc-gui/src/table/` 双网格、行列可见区绘制、共享滚动、导航/过滤、缩略条、详情、sheet 标签；Home/App/`--bench-table` 接线 | 6 个表格 GUI 测试通过；百万行原生程序基准、应用自身 framebuffer 已实际检查。Home 当前按 csv/tsv/xlsx/xlsm 扩展名路由，不声称已接入任意自定义格式掩码 |
| 编辑及设置入口 | Modify 窗口、复制、撤销、结构命令、查找替换、设置页、保存/导出等已有入口 | 仅按下面缺口清单交付；入口存在不代表完整交互回归已完成 |
| 黄金语料 | 30 个合成用例，参考报告与独立 Rules；HTML 解析/统计、运行时 xlsx 构造器、生成脚本、`KNOWN_DIFFS.md` | 29/30 用例完全一致；行 81/83、格 236/242；唯一 Unaligned 差异完整签名锁定，其他差异会失败。5.2 完成 |
| 文本线保护 | 文本实现未修改；原命令与黄金继续执行 | workspace 中原文本测试通过；Myers 30/30、Standard 29/30，既有唯一差异未增加；性能结论见下文 |

GUI 6 测的实际覆盖：

1. 二维虚拟化、tessellation、跳转和上下布局；
2. 差异导航、Copy Cell 后后台重算、undo、Ignore Unimportant 与过滤；
3. RawInput 鼠标聚焦、F2、文本输入和 Commit，以及查找/全部替换/undo；
4. 单行/单列插入撤销、保存字段及保存点；
5. 值相同公式不同的投影、详情中公式及禁止编辑命令；
6. Home 扩展名路由、**已 Commit** 的脏编辑在 Home 确认框中取消。

这些测试混合控制器调用和 RawInput，不是 6 套完整端到端场景。尤其第 6 项不能证明未提交草稿的关闭保护。

## 未完成项与后续验收

| 任务 | 状态及具体缺口 |
|---|---|
| 4.1 | 核心网格/基准已验证；Hide Same Columns、Resize Columns、可见空白、File Info、双轴交互等未形成完整场景矩阵；仅有程序 framebuffer，不宣称物理鼠标与原生 IME 已验收 |
| 4.2 | 导航和过滤实现已交付；Previous/首尾、行/编辑导航、Go To、四种过滤与 Ignore Unimportant 的组合缺少完整输入回归 |
| **4.3** | **多选结构操作未齐**：`commands.rs::structure` 从当前 cursor 取一个 row/col；删除行/列只处理当前项，排序只使用当前列，未按选中列组合排序。`clear_selected` 的矩形清空不能代替多选删除行列。**剪贴板转义缺失**：`clipboard_text` 直接以 tab/newline 拼接，含 tab/newline/引号的字段不能保证 Copy→Paste 往返。编辑器、查找框、设置框和网格的剪贴板焦点路由待专项回归。**未 Commit 草稿的关闭保护缺失**：`view.rs::is_dirty` 只看 clipboard/history，不看 `editor.text`；`request_home`/窗口关闭可能直接离开。已有取消测试先 commit，未覆盖此风险。Modify 当前是独立窗口，不是严格就地编辑。还需实际更改键并重配对、Align With/清除、多选操作及 redo 场景验收 |
| **4.4** | 多 sheet 名称/手动配对、标签切换、缓存编辑/统计、Settings 的 Specs/Format/Columns/Alignment/Sheets 完整交互待验；Swap/Reload/Recompare/Open File/Open Clipboard、双侧 Save/Save As、备份与失败路径、xlsx 禁止原格式保存及 CSV 导出、摘要导出入口尚无完整交互回归。已有内存设置和命令不可据此全勾。Save Session、Compare Parent Folders 等按 ADR 202 明确禁用/后置 |
| 4.5 | 三档控件和两侧公式详情已有实现；现测主要验证 Values→ValuesAndFormulas 的控制器投影和公式 shape，尚未覆盖完整三档实际点击往返与数字格式场景。字体/填充仍沿用 Wave 1 的后置边界，不冒称已实现 |
| 5.1 | 生成脚本、27 个 CSV 比较和 3 个生成工作簿比较的既有报告已入库；脚本语法、JSON 往返已验。**本轮没有重新启动参考工具完成锁→备份→生成→恢复的端到端采集**，尤其新增文本封装分支未在参考采集链路重跑，因此保留未完成 |
| **5.3** | `docs/reference-parity/table-compare.md` 仍不存在；D9/`commandstable.html` 逐命令、逐选项覆盖清单未完成。本轮未用一个占位清单冒充完成 |
| **6.1** | 构建、workspace 测试、黄金、压力测试及两条 GUI 基准本轮均已运行，报告已写；但完整交互验收矩阵未闭合，文本线亦无同轮基线 A/B 数据证明严格“不退化”，因此保守保留未完成，不能声称全波全量验收成功 |

优先接手步骤：先补 4.3 的草稿关闭保护和剪贴板/多选行为及回归，再完成 4.4 场景矩阵；补 5.3 逐命令清单，在完整验证后重新评定各任务。此处没有改变原任务的验收范围。

## 本轮验证

Windows、Rust/Cargo 1.99.1、egui/eframe 0.37、release 使用 glow。没有宣称使用 MSRV 1.88 工具链复测。

| 命令/检查 | 实际结果 |
|---|---|
| `cargo clippy --workspace --all-targets -- -D warnings` | 最终退出 0、零 warning |
| `cargo test --workspace` | 最终 **175 passed / 0 failed / 2 ignored**；新增 NUL 文本封装回归后复跑，不沿用最初的 174 计数 |
| `cargo build --release --workspace` | 退出 0，包含 bcompare/Matcha；是增量构建，不冒称 clean build |
| `cargo test -p bc-gui table::tests -- --nocapture` | 6 passed；workspace GUI 合计 24 测 |
| `cargo test -p bc-engine-table --test golden_table -- --nocapture` | 最终 3 passed；30 个 corpus case 不是 30 个 Rust test |
| `cargo test --release -p bc-engine-table -- --ignored` | 两个默认忽略压力测试均实际执行：1 GiB 流式记录 **5.34 s**、100 MB workbook 惰性/取消 **0.31 s**，0 失败 |
| `cargo test -p bc-core --test golden_align -- --nocapture` | 4 passed |
| `cargo test -p bc-engine-text --test golden_importance -- --nocapture` | 3 passed；Myers 30/30、Standard 29/30、known_diffs=1 |
| `openspec validate table-compare-xlsx --strict --json` | valid=true、0 issues |
| 本波 28 个 Rust 文件的 `rustfmt --check --edition 2021 --config skip_children=true` | 通过 |
| `cargo fmt --all -- --check`（额外检查） | **未通过**：尚有 21 个本轮未改文件的既有格式差异，涉及 bcomp、engine-dir、jobs、session 等；已核对与本波修改集合无交集，未借收口改无关模块 |
| `git diff --check` / `git diff 8dbafe8..HEAD --check` | 通过；黄金局部属性保留原生 CRLF，仍检查真正尾随空白 |
| Python AST、PowerShell Parser、PowerShell NUL JSON 往返 | 通过；没有运行参考程序或 RTTI 工具 |
| 提交内容审计 | 原生报告/输入字节核对；无 NUL 源文件、无 Git 二进制 diff、无参考图标/私人配置/二进制 fixture |

最后三项构建门禁 stdout 摘要：

```text
clippy:  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.55s
tests:   175 passed / 0 failed / 2 ignored（汇总所有 test result 行）
release: Finished `release` profile [optimized + debuginfo] target(s) in 0.18s
```

详细本地日志在忽略目录 `target/wave2-closeout/`：`clippy.log`、`test.log`、`release.log`、`gui-table.log`、`golden-table.log`、`stress.log`、`golden-text-*.log`、`bench-*.log`、`bench-runs.json`、`openspec.log`、`fmt-final.log`。不将本地构建日志当作需要入库的 fixture。

### 表格和文本 GUI 基准

表格输入由以下命令生成到系统临时目录：

```text
cargo run --release -p bc-engine-table --example table_gui_inputs
# 用输出的 left/right 路径：
target/release/matcha.exe <left.csv> <right.csv> --bench-table --screenshot=<target 下的本应用截图>
target/release/matcha.exe tmp/left.txt tmp/right.txt --bench=600
```

两侧表格各 **1,000,000 数据行 × 10 列、69,258,868 B**；文本复用已有 `tmp/left.txt`、`tmp/right.txt`，各 **110,000,043 B**，未修改。基准前后两组输入 SHA-256 相同；本轮新建的两个临时 CSV 及其空目录已清理，原有 tmp 输入保留。

| 600 个计量帧（另有 30 帧预热） | update mean / p99 / max ms | frame CPU mean / p99 / max ms | 加载左/右 ms |
|---|---:|---:|---:|
| 百万行表格 | **0.691 / 1.087 / 1.293** | 2.259 / 3.011 / 3.214 | 3658 / 3335 |
| 110 MB 文本 | **0.723 / 1.282 / 1.535** | 1.254 / 1.899 / 2.235 | 140 / 145 |

- 表格平均每帧绘制 62 行，最后一帧 420 格；应用 framebuffer 可见 990,000 Same / 10,000 Important / 0 orphan、1,000,000 总行，确认是实际表格视图。截图仅留 `target/wave2-closeout/table.bmp`，非参考工具图标，也未入库。
- 两者均低于 update p99 4 ms / frame CPU p99 8 ms 门槛。表格本轮 1.087 ms 是新测量，不能用此前 1.345 ms 替代本轮记录。
- 文本 Wave 4 历史最终值为 update p99 1.129 ms / CPU p99 1.688 ms；本轮分别约高 **13.6% / 12.5%**。没有同轮基线对照和多轮噪声评估，不能将“仍低于门槛”写成严格性能不退化，也不能据此认定代码导致了退化。
- wall time（表格 24.01 s / 文本 11.43 s）包含加载/比较等待；overall FPS 不是稳定绘制帧率。`index_mb`（19.5 / 101.7）不是进程峰值内存。本轮没有重新测 GUI 峰值内存。

### 黄金结果与入库包装

```text
table.golden cases=29/30 rows=81/83 (97.59%) cells=236/242 (97.52%) differences=1
```

唯一差异为 `024-unaligned`：参考 table-report 输出按键配对的 Same，本项目遵照 Unaligned 规格逐位置配对；两个数据行及六个单元格为 Important。原因尚未确认，不将报告链路观察等同于参考 GUI 行为；没有改状态、删除行或添加无关相同行稀释比例。

提交前发现 `027-quoted-newline/expected.html` 含两个原生 NUL，会被 Git 判为二进制。为遵守禁入二进制 fixture，改为 **`expected.html.json` 的可逆 JSON 字符串**，解析前恢复原文；生成器同步支持这一包装。解码与 3,952 字节原件逐字节一致，SHA-256 为 `d29a2c51f9661aaf68d1273038a7bcc748a5a6dacd105e5c71baf7b586640825`。原件保存在忽略的 `target/wave2-closeout/027-expected.native.html`；测试不依赖它。新增回归确认两个 NUL、原文片段及单元格状态均未改变。

语料局部 `.gitattributes` 阻止 CSV/HTML/JSON/XML 自动换行转换，避免 CRLF 被误当尾随空白或被 Git 改写；HTML 内部 `BcImages/*.png` 引用原样保留，但图片文件没有提交。详情见 [语料说明](../../crates/bc-core/tests/golden/table/README.md)。

## 分组提交

依赖顺序：已有语义修正 → 引擎 → GUI → 黄金 → 本报告/任务状态。均在 master 本地提交，未 push，未使用旧 Task/Dispatch，也未新派发 worker。

| 范围 | commit |
|---|---|
| Wave 1 报告基线（已有） | `ed55e66` |
| Unimportant 语义修正（已有，未重写） | `8dbafe84e51e8057562e9dd4dee8fcc1f2dc3a14` |
| 后台/结构编辑与投影支撑 | `5a87f0feb1469c68fe226bd0ed4fa691c9570b68` |
| 表格 GUI、入口、基准与 ADR 202（明确部分交付） | `3af9bb11627bf5314118e042fad323914fc99118` |
| 30 例黄金、解析器、无损包装、生成工具及说明 | `72a487cd3a7ee608967035aeaf5de83ef13638d5` |

本报告与 `tasks.md` 另作 `docs: record partial table compare wave two closeout` 提交，不在报告内制造自引用 hash；其提交号可用 `git log -1 --format=%H -- docs/worker-reports/table-compare-xlsx-wave2.md` 查询。

## 归属、保留内容及合并注意事项

### 两份协调者 spec 原样留在工作区

根据本轮交接提供的归属信息，将下列未提交差异视为协调者修改；未提交 diff 本身不能独立证明作者。核查差异仅涉及大小写/空白/容差内差异为 Unimportant，与 `8dbafe8` 呼应，**没有擅自混入上述任何提交**：

- `openspec/changes/table-compare-xlsx/specs/table-cell-comparison/spec.md`
  - SHA-256：`a71177b2058a66dc94682fc6d6f82055c44460109897b1db25fb25ccce4b52af`
- `openspec/changes/table-compare-xlsx/specs/table-columns-and-sheets/spec.md`
  - SHA-256：`c7c06663d7b0525e029af5110c7461fb02b60f252bad5f11c6bda5d18b067b08`

开始及收口核对字节未变，等待协调者单独确认/提交。未恢复旧 spec，也没有借本轮任务状态更新覆盖其修改。

### 保留与隔离

- `openspec/changes/folder-compare-sync/` 仍未跟踪，未提交、未删除；仅对当前 master 下该 change 的文件做哈希保护核对。与两份 spec 共 12 个文件的哈希保持一致。
- 没有进入文件夹线 worktree，没有访问 `D:\Github\ObsidianVault`。
- 没有运行参考工具，未为采集/逆向读取其授权文件或私人会话/格式配置，未写参考安装目录。已有参考产物只读审查；RTTI 辅助脚本仅作为源代码入库，未执行。
- 没有提交参考工具图标、私人会话/格式备份、xlsx 二进制、原始 NUL HTML 或本轮截图。所有提交文件均显式选取，没有 `git add .` / `git add -A`。
- `bc-core` 仅增加黄金数据，未修改其源码；`bc-engine-text`、`bc-session`、`bc-jobs`、文件夹引擎源码本轮零修改。无新第三方依赖；Cargo.lock 只增加 GUI→table 内部依赖。

### 合并提示

1. 与文件夹线合并时重点处理 `bc-gui/src/app.rs`、`home.rs`：保留 `View::TableCompare(Box<...>)`、`HomeAction::OpenTableCompare`、现有文本分支/关闭保护/基准和文件夹分支，勿用整文件覆盖。`Compare Parent Folders` 仍禁用，不能据入口文字宣称已联动。
2. `LaunchOptions` 新增 `bench_table`；命令行必须先匹配 `--bench-table`，再匹配通用 `--bench` 前缀，保持原文本 `bench.*` 协议。
3. `TableMap` 同步便捷 API 默认不变；GUI 显式启用 deferred edits，以 350 ms 防抖后台 Realign 并拒绝过期修订。不要将重对齐移回每帧 UI；新的 `TableRules::left_is_source` 默认 true，检查不使用 `..Default::default()` 的结构体字面量。
4. 黄金提交依赖 `8dbafe8` 的三态语义，不应通过放宽断言恢复旧 Same 判定。保留唯一已知差异的完整签名及 95% 双门槛；JSON 包装不是可以随意改写 oracle 的授权。
5. 原生会话持久化/关联程序/文件夹子会话、xlsx 写回与字体填充沿用既有后置范围。完整命令归属仍应由 5.3 清单收敛，不以 ADR 202 代替逐条清单。
6. 两份 spec 和未跟踪 folder change 是**有意保留的脏工作区**，不是本轮漏提交代码。后续检查、cherry-pick 或清理工作区时必须保护它们；整个 Wave 2 未完成前不得归档或宣称全部成功。
