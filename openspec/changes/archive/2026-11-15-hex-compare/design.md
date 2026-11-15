# Design

## Context

现状：`bc-core::align::ByteAlign` 是 `todo!()`；`bc-engine-bin::hex` 只有 `HexContent`/`HexRules`/`HexMap` 数据模型骨架；`bc-session::SessionKind::HexCompare`（`THexCompareSession`）已可命名但无实现；GUI 只有 text/table/folder/sync 四类视图。可复用资产：`bc-core` 的 `AlignMap`/`CancelCheck`/`mmap`/`hash`、`bc-gui` 的 `chrome/` 外观层（菜单栏、工具栏、路径栏、状态栏、缩略图栏，visual-parity 成果）、text/table 视图的后台 job + revision 拒绝模式。

参考行为真源：`docs/reference-parity/hex-compare.md`（运行时截图 + CHM 逐条对照）。关键证据结论：**渲染单元是"对齐图打包成的单元格行"**，不是两侧各自按文件偏移分行——这决定了派生行模型必须在引擎侧一次算好。

## Goals / Non-Goals

**Goals:**
- 三档字节对齐（Complete/Fast/None）落在 `bc-core::align::ByteAlign`，正确性不变量与参考语料统计逐字节一致。
- `bc-gui/src/hex/` 双栏视图覆盖 `hex-compare-view` spec 的全部命令行为（编辑、导航、过滤、查找、设置三页、Info 统计）。
- Home/CLI/会话互切入口接线；`hex-report` 黄金语料流程跑通。

**Non-Goals:**
- GUI 报告输出、会话持久化、父文件夹联动、外部程序、全局字体、VFS profile、格式 Conversion 外部程序（均按 proposal 后置）。
- Version/Picture/Media/Registry compare。
- REA 静态符号级逆向（类名映射沿用骨架注释；行为以运行时证据为准）。

## Decisions

### D1：对齐算法 = 锚点分块 + 区间细化，三档策略共享同一输出契约

字节序列是 256 符号的高重复序列，经典 Myers O(ND) 在 N=文件大小时不可行。采用二进制 diff 的标准工程做法：

- **None**：offset i 对 offset i；长度差尾部为 orphan。
- **Fast**：固定块长（默认 64 字节）滚动哈希找相同锚块，锚块间直接标记为 orphan（不做字节级细化）。面向超大文件，允许粗糙分段。
- **Complete**（默认）：同样的锚块骨架，锚块间区间做字节级编辑距离对齐（带长度上限；超限区间降级为整段 orphan，保证有界）。参考语料必须精确复现 8/16/32 的三段分类与计数。

备选：引入现成二进制 diff 依赖（如 bsdiff 族）——否决，proposal 锁定零新增依赖，且其输出是补丁不是对齐图；纯 Myers——否决，复杂度不可控。

`ByteAlign::align(left, right, strategy) -> AlignMap` 复用现有 `AlignMap`/`AlignRow`（行号=字节索引），不新增公共类型。

### D2：渲染行模型在引擎侧派生（cell packing）

`bc-engine-bin::hex` 从 `AlignMap` 派生 `Vec<HexRow>`：每行 N 个单元格（默认 16，来自 HexRules），单元格 = 配对字节 / 单侧孤儿 / 对侧占位；行标签 = 该侧本行首个真实字节偏移（全占位则为空）。GUI 只画可见行，不做任何对齐计算。奇数长度插入导致的行相位偏移（证据见参考文档 `24-odd-insert-all.png`）由该模型自然产生，不需要特例。

备选：GUI 按文件偏移各自分行、动态插入整行空隙——否决，无法表达行中空隙（5 字节插入场景），与参考行为不符。

### D3：加载分层：小文件整载可编辑，大文件 mmap 只读（初版）

- < 阈值（默认 2 MB，对齐参考工具）：整读入 `Vec<u8>`，不加文件锁，可编辑。
- ≥ 阈值：`bc-core::mmap` 只读映射 + 共享读锁；初版**禁用编辑与保存**（mmap 原位编辑的撤销/重对齐复杂度高，列为已知限制）。对齐在映射上按块取字节，不整拷贝。

### D4：编辑 = 字节缓冲操作 + 全量重对齐（worker + revision）

编辑命令直接改本侧字节缓冲（Insert/Overwrite 两模式），Undo 栈存字节级逆操作；任何编辑提交后投递重对齐 job（携带 revision，过期结果丢弃）——复用 text/table 的 worker 模式，不做增量对齐。Copy to Other Side = 按选中区段规划对侧覆盖写 + 一次 Undo。

### D5：GUI 模块与接线

新增 `crates/bc-gui/src/hex/`：`view.rs`（控制器/输入路由）、`render.rs`（网格/地址列/ASCII 列/斜纹占位/缩略图）、`worker.rs`（对齐 job）、`settings_panel.rs`（Specs/Format/Comparison 三页）、`find.rs`（内联查找栏）、`menu.rs`（Session/Edit/Search/View 菜单）。`app.rs` 加 `View::HexCompare`，`home.rs` 加会话卡与 `hex_paths` 路由判定（内置 hex 格式掩码初版 `*.bin`，Detected 机制沿用 table 的掩码路由模式）。`/fv="Hex Compare"` 经 `SessionKind::force_view_name` 既有解析接入。Compare Files Using 在 text/table 视图增加 Hex Compare 互切（原位替换视图，沿用 table→text 的既有模式）。

### D6：会话规则模型

`HexRules` 扩展为：`alignment: None|Fast|Complete`（默认 Complete）、`bytes_per_line`（默认 16，Fit to window 开启时由视图按宽度计算）、`bytes_per_group`（默认 8）、`context_count`（默认 16）、`lock_threshold`（默认 2 MB）。骨架中的 `skew_tolerance` 归入 Complete 的区间上限参数，不作为独立 UI 项。

### D7：黄金语料与互斥锁

`crates/bc-core/tests/golden/hex/`：语料生成器（固定种子的伪随机缓冲 + 程序化改动/插入/删除，与参考采集所用语料同构）+ 参考工具 `hex-report layout:side-by-side options:display-all,line-numbers` 生成的期望文本；调用参考工具前持 `%TEMP%\matcha-reference-tool.lock`。正式差异记 `KNOWN_DIFFS.md`。

## Risks / Trade-offs

- Complete 对齐在病态输入（如整块重复区）上可能退化 → 区间长度上限 + 降级 orphan 段；Fast 兜底大文件。
- 大文件只读限制与参考工具（带锁但仍可编辑）存在差距 → 初版接受，spec 未承诺大文件可编辑；后续按反馈评估。
- 行相位偏移使"地址列连续递增且 16 对齐"的直觉不再成立 → 渲染与测试都以 cell packing 模型为准，黄金截图（Matcha 快照）对照参考截图。
- REA 静态分析未纳入 → Fast/Complete 的精确算法边界以后续静态核对或更多黑盒实验校准；spec 只约束正确性不变量与公开语料期望，不受影响。

## Open Questions

- Little/Big Endian Values 子菜单的完整数值类型清单（字节/字/双字/浮点等）未逐项截图，实现时按参考菜单再核对——仅影响该子菜单项集合，不影响渲染与对齐契约。
