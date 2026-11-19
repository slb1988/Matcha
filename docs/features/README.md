# docs/features — 功能文档索引

面向 AI 与新手贡献者的**功能级**入门文档：每个文件对应一个用户可感知的功能，回答
"这个功能是什么、做到什么程度、代码在哪里、怎么验证、去哪里深入"。

## 与其他文档的分工

| 位置 | 回答的问题 | 粒度 |
|---|---|---|
| `docs/features/`（本目录） | 功能全貌：状态、代码地图、用法、入口 | 功能（跨 crate） |
| `crates/*/README.md` | 单个 crate 的 API、内部结构、约束 | 实现细节 |
| `docs/decisions/` | 为什么这么做（ADR，编号即决策序列） | 单个决策 |
| `docs/reference-parity/` | 与参考工具（BC 5.0.6）的行为对齐证据 | 行为语义 |
| `openspec/changes/` | 某次变更的提案 / 任务清单 /  delta 规格 | 变更过程 |
| `docs/worker-reports/` | 历史工作波次的实施报告（归档性质，不更新） | 历史记录 |

**看功能从这里进，看实现进 crate README，看原因进 decisions。**

## 功能清单与状态

| 文档 | 功能 | 状态 | 主要 crate |
|---|---|---|---|
| [text-compare.md](text-compare.md) | 文本比较 / 编辑（FR-T1…T8） | 双向比较已实现；三方合并见 [text-merge.md](text-merge.md) | `bc-engine-text` |
| [text-merge.md](text-merge.md) | 文本三方合并（FR-T7）：自动合并、冲突解决、P4V 集成 | 已实现（引擎 + 命令行 + 界面；合并语义未经参考样本确认） | `bc-engine-text`、`bc-script`、`bc-gui` |
| [folder-compare-sync.md](folder-compare-sync.md) | 文件夹比较与同步（FR-D1…D5） | 已实现（wave A/B/C 完成） | `bc-engine-dir` |
| [table-compare.md](table-compare.md) | 表格比较 CSV/XLSX（FR-C1） | 已实现；三方合并见 [table-merge.md](table-merge.md) | `bc-engine-table` |
| [table-merge.md](table-merge.md) | 表格三方合并 xlsx/xlsm/CSV：键对齐逐格合并、自动合并、冲突解决 | 已实现（引擎 + 命令行 + 界面；BC 无对应功能，语义以 design 为准） | `bc-engine-table`、`bc-script`、`bc-gui` |
| [hex-compare.md](hex-compare.md) | 十六进制比较（hex-compare change 已落地） | 21/23（3.2 待参考机生成期望；7.2 Mac 侧 4 个无关既有夹具失败） | `bc-engine-bin` |
| [cli-and-scripting.md](cli-and-scripting.md) | 命令行三件套、脚本引擎、退出码（FR-S1/S2） | 无窗口子集已实现 | `bc-script` + `apps/` |
| [reports.md](reports.md) | 报告输出（patch 四种布局等，FR-R1） | patch 布局已实现 | `bc-report` |
| [sessions-and-formats.md](sessions-and-formats.md) | 会话 / 工作区、文件格式库（FR-X1） | 数据模型与格式库已实现 | `bc-session`、`bc-formats` |
| [saved-sessions.md](saved-sessions.md) | 命名会话：Save Session / Save Session As、Home「Saved」、`matcha "<会话名>"` | 已实现（单元测试 + app 流程测试） | `bc-session`、`bc-gui` |
| [disk-change-reload.md](disk-change-reload.md) | 文件在外部被修改后弹框询问重载 / 自动重载（Options › Tweaks） | 已实现（单元 + 集成 + app 流程测试；真实窗口焦点切换待手动确认） | `bc-gui`、`bc-session`、`bc-engine-text` |
| [resizable-splitters.md](resizable-splitters.md) | 窗格分隔线可拖动（左右 / 上下），位置写盘、重启保持 | 已实现（单元测试 + 快照对比） | `bc-gui` |
| [gui-shell.md](gui-shell.md) | GUI 外壳与各视图架构（FR-G1） | 已实现（视图群落地；visual-parity 已归档：结构对齐参考、样式走设计系统） | `bc-gui` |

> 状态列随开发推进更新；打开任一文档前先看它顶部的"状态"小节是否仍与代码一致。

## 维护约定（强制）

1. **新功能落地前**：在 `docs/features/` 新建或更新对应文档，先写"功能简介 / 验收方式"，
   再实现。文档文件名用 kebab-case，与 openspec change 名尽量一致。
2. **功能行为发生变化时**（语义、开关、退出码、UI 交互、文件格式任何一项）：
   同一次提交内同步更新对应 feature 文档，尤其是"状态"和"关键语义"两节。
   文档与代码冲突视为缺陷。
3. **状态词只用三档**：`未开始` / `进行中（说明缺什么）` / `已实现（说明验证方式）`；
   脚手架占位统一写 `脚手架`。
4. **不复制实现细节**：crate 内部 API、算法说明留在 crate README；feature 文档只放
   代码地图（指针）和功能级语义，细节处用链接跳转，避免双写腐烂。
5. **每个文档固定结构**（模板见下），新增功能文档必须套用：

```markdown
# <功能名>

> 一句话定位。状态：<未开始|进行中|已实现>（YYYY-MM-DD 更新）

## 功能简介
## 代码地图      ← crate/模块/GUI 入口/测试入口
## 关键语义      ← 做这个功能前必须知道的规则
## 使用与验证    ← 命令、测试、基准
## 深入阅读      ← 链到 crate README / ADR / reference-parity / openspec
```
