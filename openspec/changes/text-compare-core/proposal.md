# Proposal

## Why

Matcha 的目标是一款以「对比」为本职、行为上一比一复刻 BC5 核心体验、再在表格（xlsx）与 AI 辅助合并上超越它的现代化比较工具。目前仓库只有 14 crate 的骨架（全部算法为 `todo!()`），而文本比较是所有后续能力（文件夹的规则比较、表格对齐、3 路合并、AI 冲突包）的地基：没有可用的行 diff、对齐、重要性分级与自绘文本控件，其余路线图无法开工。本 change 把文本比较这一条腿从骨架做成可日常使用的产品功能。

## What Changes

- 实现 2 路文本比较引擎：行 diff（Myers 基线 + 前后缀裁剪 + 大文件降级）、Quick / Smart 两档对齐、`AlignMap` 导航（上一/下一差异、手动 Align With）。
- 实现重要性管线：按内置/用户文件格式分词 → 元素级重要性 → 行级 → 差异区块传播；支持「忽略不重要差异」、不重要文本规则、替换规则（FR-T4）、空白/大小写/行尾选项。
- 实现行内字符级差异高亮（FR-T2）。
- 实现编码与行尾往返：UTF-8/16（含 BOM）与常见 ANSI 代码页的检测、解码、保存时保持原 BOM/行尾风格；每行独立行尾（FR-T5）。
- 实现自绘双窗格文本控件（取代原型）：差异着色（重要=红/不重要=蓝/插入删除=绿、可配）、显示过滤（All / Differences / Same / Context）、缩略图导航条、行号栏、行内高亮、选择/光标/基本编辑、撤销重做、两侧独立保存、Swap Sides / Reload / Recompare。
- 实现文内查找/替换与差异区块复制到另一侧（Copy Section）。
- 建立黄金语料回归：用参考工具的 `text-report layout:xml` 产出作为期望值，覆盖对齐、重要性与行内高亮。
- 不在本 change 范围：3 路合并与 AI 辅助（后续 `text-merge-and-ai`）、文件夹/表格/Hex 视图、会话持久化与 CLI（后续 `session-and-cli`）、报告输出、VFS 远程协议、图片/媒体/注册表/版本比较（长期后置）、授权与商标相关的一切（红线）。

### 模块取舍（本项目整体口径，供后续 change 复用）

| BC5 模块 | 取舍 |
|---|---|
| Text Compare / Text Merge / Folder Compare / Folder Sync / Table Compare / Hex Compare | 行为 1:1 复刻（表格再超越） |
| 文件格式库、CLI 开关与退出码、双启动器、查找 | 1:1 |
| 会话/工作区、偏好、报告、脚本引擎 | 精简：取数据模型，自有持久化格式，只读导入 BC 配置；脚本保留核心子集 |
| Text Edit / Text Patch / Home | 精简：复用编辑器与报告，不单独立项 |
| Folder Merge、快照 .bcss、设置包 .bcpkg、Picture / Media / Registry / Version | 后置 |
| VFS 远程协议、SCC、外壳扩展、剪贴板比较、更新检查、主题包 | 本期丢弃 |
| 授权 / 试用 / 注册 | 红线，永不实现 |

### 路线图（每项一个 change）

`text-compare-core`（本 change）→ `folder-compare-sync` → `session-and-cli` → `table-compare-xlsx` ‖ `text-merge-and-ai` → `hex-and-reports`。

## Capabilities

### New Capabilities

- `text-diff-alignment`: 行级 diff、Quick/Smart 对齐、对齐映射与差异导航、手动对齐覆盖的行为规范。
- `text-importance`: 基于文件格式语法的分词、元素/行/区块级重要性分级、不重要文本与替换规则、忽略不重要差异的语义。
- `text-inline-diff`: 已配对行内部的字符级差异标注规则。
- `text-encoding`: 文本编码/BOM/行尾的检测、解码与保存往返行为。
- `text-compare-view`: 双窗格文本比较视图的用户可感知行为：着色、显示过滤、导航条、编辑与保存、查找替换、区块复制、Swap/Reload/Recompare。
- `file-format-library`: 内置格式库与用户格式导入、按掩码解析格式、格式对比较行为的影响。

### Modified Capabilities

（无：项目尚无既有 spec。）

## Impact

- 代码：`crates/bc-core`（line/diff/align/edit/encoding/hash/pattern 由 `todo!()` 变为实现）、`crates/bc-formats`（grammar tokenizer、registry 解析）、`crates/bc-engine-text`（content/map/importance/inline/search/jobs）、`crates/bc-gui`（原型 `text_view` 升级为正式控件）、`crates/bc-core/tests/golden/text/*` 与 `align/*` 填充语料。
- 依赖：新增 `regex`（`bc-core::pattern` 后端）、`xxhash-rust`（行哈希）、`encoding_rs`（ANSI 代码页）；均为纯 Rust、无 GUI、无异步运行时。`bc-gui` 继续仅依赖 eframe/egui。
- 架构约束不变：引擎 crate 不依赖 GUI；依赖方向单向无环；所有路径使用 `std::path`。
- 为后续 change 预留的接口：`TextMap` 的行/区块/重要性 API 要能直接导出为 AI 冲突包（`ConflictPacket`）的文本表示；`AlignMap` 要能被表格引擎以行键复用。
- 验收口径：黄金语料对齐一致率、100 MB 文件打开与滚动指标不低于骨架阶段原型（打开 < 200 ms/100 MB，update p99 < 4 ms）。
