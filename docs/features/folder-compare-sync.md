# 文件夹比较与同步（Folder Compare / Sync）

> 两个目录树的配对比较、过滤、后台内容比较与五向同步。状态：**已实现**
> （openspec `folder-compare-sync` 34/34；本地文件系统，远程 VFS 为后续里程碑）。
> 对应 FR-D1…D5。

## 功能简介

- 目录树配对（`DirTree`）：稳定 arena NodeId、每文件夹子表、手动对齐覆盖（virtual pairs）。
- 比较条件流水线（criteria）：时间戳/大小/属性/CRC/二进制/规则内容比较，后台执行。
- 过滤器：名称/属性/时间/大小，预设持久化在 `Matcha/folder-filters.xml`。
- 文件操作：复制/移动/删除/重命名，回收站策略（ADR 102），显式范围+分类双重确认（ADR 103）。
- 同步：五种预设 + 自定义、预览动作、逐项覆盖与冲突导航；成功后维护同步基线。

## 代码地图

| 层 | 位置 | 内容 |
|---|---|---|
| 引擎 | `crates/bc-engine-dir` | `tree`（DirTree/CompareStatus）、`criteria`、`filters`、`sync`（计划）、`ops`（文件操作）、`jobs`（后台比较） |
| GUI | `crates/bc-gui/src/dir_view.rs` | 单 worker 独占 DirTree，GUI 只读不可变行快照；列排序/过滤/导航/文件操作/局部刷新 |
| GUI | `crates/bc-gui/src/sync_view.rs` | 同步预览与执行、冲突导航、基线维护 |
| GUI | `crates/bc-gui/src/filters_bar.rs`、`dialogs/` | 过滤预设与确认对话框 |

打开文件对时 worker 只校验路径，文本比较视图在 UI 线程构建并以浮动窗口显示。

## 关键语义

- 配对、孤儿项、排除规则见 ADR 101；UI 与 virtual pairs 见 ADR 104。
- 同步不推进未解决冲突；基线语义见 ADR 103。
- 引擎被 `bc-engine-text` 反向用于"规则内容比较"（criteria 的一种）。

## 使用与验证

```powershell
cargo run --release -p bcompare -- <left-folder> <right-folder>   # 两个目录参数自动进入
cargo run --release -p bcompare -- --bench-dir <L> <R>            # 原生窗口基准
powershell tools/golden/bench_dir.ps1                             # 10 万文件树 p99<4ms 门槛
```

## 深入阅读

- `crates/bc-engine-dir/README.md`
- ADR：`docs/decisions/101…105`（配对/过滤、回收策略、操作确认与基线、UI、golden 证据）
- 行为对齐：`docs/reference-parity/folder-compare.md`、`folder-sync.md`
- 实施报告：`docs/worker-reports/folder-compare-sync-waveA/B/C.md`、`folder-compare-sync-integration.md`
