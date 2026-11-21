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
- **导航到头的消息面板**：文件夹比较的 Next / Previous Difference（`Ctrl+N` / `Ctrl+P`、`F7` / `Shift+F7`、菜单）与同步视图的
  Next / Previous Conflict 没有目标时，焦点不动，状态栏正上方（日志面板之下）弹出 **Difference not found.** 面板并响一声，取代原来状态栏的
  "No further difference/conflict" / "No further conflict"；下一次按键或鼠标按下关闭。冲突在后台线程查找，找不到时回传
  `Notice::NotFound`，UI 线程据偏好弹面板。遵循 Tools › Options › Next Difference（Wrap around 回到第一个/最后一个差异条目或冲突，
  `Command::FocusConflict` 带 `wrap`；Show message panel 关闭时静默），偏好由 `app.rs` 每帧经 `DirCompareView::set_next_diff` 同步，
  也传给内嵌文本窗口。规则见 [text-compare.md](text-compare.md)「找不到差异时的消息面板」。
- 界面：文件夹树右键菜单按参考排列（Open Folder / Open、Open/Close Subfolders、Set as Base、Open in New View、Open With、
  Compare To…、Compare Contents、Copy/Move to Right|Left、Copy/Move to Folder、Delete、Rename、Attributes、Touch、Exclude、
  New Folder、Copy Filename、Ignored、Refresh Selection、Synchronize、Explorer），复用 Actions 菜单命令，Synchronize 子项
  与 Open With ▸ Folder Sync 暂为禁用；右键先选中所点一侧。Session Settings 按参考帮助分六页（Specs、Comparison、Handling、
  Name Filters、Other Filters、Misc），编辑草稿，OK / Apply 只重发变化的部分（过滤、规则重扫、路径变更重载）。
  文件操作与范围确认对话框以文字呈现标题与选项（Copy to Folder、Both sides、Keep relative folder structure 等）。

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
- 系统拖放：把文件夹拖到窗口左/右半边即替换该侧基准文件夹（比较与同步视图相同；拖入文件只提示）。
  规则见 [gui-shell.md](gui-shell.md)「系统文件拖放落点」。

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
