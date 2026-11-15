# Proposal

## Why

二进制文件比较是用户点名要"一比一再超越"的能力之一（proposal 路线图，`bc-engine-bin` 规划中的 P2 项）。参考工具的 Hex Compare 以"字节对齐 + 单元格行打包"模型支持改字节、插入、删除三类差异的同屏对照（证据见 `docs/reference-parity/hex-compare.md`）。目前 `bc-core::align::ByteAlign` 与 `bc-engine-bin::hex` 全是 `todo!()` 骨架，`bc-session` 已能命名 `THexCompareSession`/`HexCompare` 但 GUI 无视图、Home 无入口。本 change 把 hex compare 做到与参考工具公开行为对齐。

## What Changes

- 字节对齐引擎：`bc_core::align::ByteAlign` 落地，支持参考工具的三档对齐策略 **Complete**（默认，全量字节对齐）/ **Fast**（大文件快速启发式）/ **None**（逐字节位置对齐）；输出 `AlignMap`（行=字节），统计 same / left orphan / right orphan / difference 四分类计数。
- 加载与保存：整文件载入（默认 < 2 MB 阈值内不加文件锁，允许外部编辑，阈值可配），大文件经 `bc-core::mmap` 流式访问；单侧保存 / 另存（暂存+改名落盘）、单侧 Disable editing、Open Clipboard。
- 双栏 hex 视图（`bc-gui/src/hex/`）：地址列 + 8+8 分组 hex 单元格 + ASCII 预览列 + 缩略图条 + File Info 头；**单元格级行打包**（孤儿字节占格、对侧斜纹占位格可位于行中、行标签=该侧本行首个真实字节偏移）；红底差异/孤儿着色；Side-by-Side/Over-Under；Show All/Differences/Same 过滤（Diffs 带 Context count 上下文，默认 16）；Layout 对话框（Bytes per group/Groups per item/Fit to window/Context count）。
- 交互：Next/Previous Difference Bytes（Ctrl+N/Ctrl+P）、Go To（十进制/十六进制地址、选侧、跨侧同步光标）、内联查找栏（hex/文本模式、Prev/Next）、Replace、字节编辑（Insert/Overwrite、Cut/Copy/Paste/Delete、Select All、Undo/Redo、Copy to Other Side Ctrl+R）、Swap Sides、Reload/Recompare、Hex Compare Info（四分类统计）、Session Settings 三页（Specs/Format/Comparison）。
- 入口与接线：Home 新增 Hex Compare 会话卡与路由（内置 hex 格式掩码，初版 `*.bin` 等，其余未知扩展仍走 Text）；`View::HexCompare` 标签页接入应用外壳；`matcha /fv="Hex Compare"` 强制会话类型；文本/表格会话的 Compare Files Using 增加 Hex Compare 互切。
- 黄金语料：用参考工具脚本 `hex-report`（`layout:side-by-side/summary`、`options:display-all/display-mismatches,line-numbers`）对构造语料生成期望，校验对齐与统计一致（调用参考工具前持 `%TEMP%\matcha-reference-tool.lock`）。
- 已批准后置（与 text/table 既有约定一致）：会话持久化（Save Session/Locked/Clear Session）、Hex Compare Report 的 GUI 报告输出、父文件夹联动与子会话命令、外部程序 Open With、全局字体、VFS profile、外部转换程序（格式 Conversion 页）、Version/Picture/Media/Registry compare。

## Capabilities

### New Capabilities

- `hex-byte-alignment`：字节级对齐与差异分类——三档对齐策略、对齐图语义（same/orphan/difference、顺序保持）、四分类统计、编辑后重对齐。
- `hex-compare-view`：Hex Compare 会话的用户可感知行为——加载/保存与文件锁语义、单元格行打包渲染、导航/过滤/查找/编辑命令、会话设置三页、Home/CLI/会话互切入口。

### Modified Capabilities

（无。`openspec list --specs` 当前无已归档能力；与既有 change 的共享约束——`AlignMap` 公共语义、DiffColors、显示过滤语义、互斥锁流程——仅复用不修改。）

## Impact

- 代码：`crates/bc-core`（`align.rs` 的 `ByteAlign` 实现）、`crates/bc-engine-bin`（`hex` 模块落地）、`crates/bc-gui`（新增 `hex/` 模块、`View::HexCompare`、`home.rs` 入口与路由、文本/表格视图的 Compare Files Using 互切）、`crates/bc-session`（HexRules 接线）、`apps/bcompare`（`/fv=` 与路由）。
- 依赖：不新增第三方依赖；复用 `bc-core` 的 `mmap`/`cancel`/`hash` 与 `bc-gui` 的 `chrome/` 外观层（visual-parity 的成果）。
- 黄金语料：`crates/bc-core/tests/golden/hex/*`（新增），由参考工具 `hex-report` 生成，KNOWN_DIFFS 记录正式差异。
- 并行约束：在 master 上开发；红线沿用——不提取参考工具图标/位图资源，参考截图仅作内部工程证据（私有仓库）。
