# folder-compare-sync — 任务组 8 集成报告（8.1 全量回归 / 8.2 合并 master）

日期：2026-11-06。分支 `sunlaibing/folder-compare-sync`，worktree 独立。
任务组 8 两项完成后，OpenSpec apply 为 **34/34**；本报告不包含归档（等待评审）。

## 结论

| 门禁 | 合并前（89c3408） | 合并后（781f004） |
|---|---|---|
| `cargo clippy --workspace --all-targets -- -D warnings` | 退出 0，零 warning | 退出 0，零 warning |
| `cargo test --workspace` | 143 passed / 0 failed / 1 ignored | **246 passed / 0 failed / 3 ignored** |
| 10 万文件 native 基准 update p99 | 0.740 ms（p50 0.405 / p95 0.667 / max 0.947） | **0.776 ms**（p50 0.462 / p95 0.633 / max 0.930） |
| 目录黄金文件状态 | 32/33 = 96.97%（1 条已批准 Unsupported） | **33/33 = 100%，已知差异 0** |
| 目录黄金文件夹结构 | 4/4 | 4/4 |

合并后测试数包含 master 带来的文本/表格测试；3 个 ignored 为本线回收站人工测试 1 个、
master 既有 2 个，均非本轮新增。两次基准都是 600 个计时样本、每帧最多绘制 32 行、
`bench.dir.pass=true`；fixture 生成时间（72 s / 107 s）不计入 UI 帧。

## 8.2 合并

`git merge master`（不 rebase，保留已发布历史），合并提交 `696fb68`。
master 自分叉点 `4481062` 新增 30 个提交（正式文本视图、表格比较、MSRV 1.88）。

冲突与处理：

| 文件 | 处理 |
|---|---|
| `Cargo.toml` | 同时保留 `windows-sys`（本线）与 `calamine`（master）；MSRV 取 1.88 |
| `apps/bcompare/src/main.rs` | `--bench-dir` 先于 `--bench-table` / `--bench` 前缀识别；保留 `--screenshot=` |
| `bc-gui/src/lib.rs` | master 的 `text`/`table` 模块 + 本线文件夹模块；旧 `text_view` 原型随 master 删除；`LaunchOptions` 合并 `bench_table`/`bench_dir`/`screenshot` |
| `bc-gui/src/app.rs` | 以 master 版本为底，加入 FolderCompare/FolderSync 视图、`--bench-dir` 驱动；两个目录参数先于表格/文本路由；接通 master 预留的 `ViewAction::CompareParentFolders` |
| `bc-gui/src/home.rs` | `routed()` 两侧均为目录时进入文件夹比较（拖放与 Compare 按钮共用）；保留文件夹卡片，最近列表沿用 master |
| README / `ui-framework-decision.md` / `tools/golden/README.md` | 两边内容并存，文本章节在前、文件夹章节在后 |

接线要点：

- **正式文本视图**：master 的 `TextContent` 含 `Rc`/`RefCell`，不能在 worker 构造。
  dir_view 的 worker 只校验两侧是文件，UI 线程构建 `TextCompareView`，仍以浮动窗口显示。
  关闭窗口、顶栏 Home、关闭主窗口都先走文本视图自身的保存确认（含子文件夹视图中的文本窗口）；
  文本窗口打开时暂停文件夹的 Enter/Delete/方向键快捷键，避免编辑时误触发文件夹删除对话框。
- **Compare Parent Folders**：文本视图中该命令在两侧均为磁盘文件且无未保存编辑时启用；
  顶层文本视图切换为文件夹比较，文件夹内的文本窗口打开独立子文件夹视图。
- **MSRV 1.88 lint**：本线代码的 `map_or(true, …)`、`% n == 0`、`chunks_exact` 按 clippy
  建议机械改为 `is_none_or`/`is_multiple_of`/`as_chunks`（与 master 的 8132e52 同类）。
- **测试临时目录冲突**：master 文本 GUI 测试与本线文件夹 GUI 测试都使用
  `matcha-gui-test-{pid}-{n}` 且计数器各自独立，并行时互删目录导致 4 个文件夹测试失败；
  本线改用 `matcha-folder-gui-test-` 前缀后连续 3 次 35/35 通过。

## 2.5 规则比较接通（提交 `781f004`）

- `ContentCompareJob` 在 RulesBased 下读取两侧全文，按各自文件名解析文本格式
  （内置 + 用户格式，进程内加载一次；用户格式无效时回退内置），以
  `Criteria::rules` 组装 `TextRules` 运行 `Align2Job::execute(cancel)`：
  字节相同 → Same；仅不重要差异 → SimilarUnimportant；有重要差异 → Different。
  文本引擎拒绝的二进制输入回退字节结果；取消贯穿读取与对齐。
- `Criteria.rules` 默认等于文本引擎默认规则（注释重要、行尾不重要）。新增
  `comments-unimportant` 扩展脚本开关（GUI 规则框可输入，参与 to_script 往返）。
- 规格 folder-criteria“规则比较只有不重要差异”场景：`.cpp` 仅注释不同，在
  `rules-based comments-unimportant` 下为“相似”，开启 ignore-unimportant 后视为相同（单测）。
  默认规则下同一对为 Different，与原生黄金 `diff` 一致；原生 XML 不区分重要/不重要，
  因此“相似”一侧没有原生 oracle，只有引擎单测。
- 文件夹视图的 Quick compare 与 Compare contents 菜单均提供 Rules-based；GUI 测试覆盖
  跨行 a.txt（Different）与 b.txt（Same）。
- 黄金：rules-comments 从 Unsupported 变为与原生一致；回归改为零容忍。
  语料与生成器字节未改（保留 provenance 哈希），`case.xml` 中的 `knownDiff` 仅为采集时注记。

## 原生程序验证（release，合并后）

| 视图 | 方式 | 结果 |
|---|---|---|
| 文件夹（10 万文件） | `tools/golden/bench_dir.ps1` | p99 0.776 ms，[截图](assets/folder-integration-bench.png) |
| 文件夹（普通两目录） | `gui_smoke.ps1 -Action Start -Name folder-integration-smoke` | 同名/孤儿紫/较新红/较旧灰/属性后缀正确，[截图](assets/folder-integration-smoke.png) |
| 文件夹 → 正式文本 | 向本应用窗口 PostMessage `Down ×2` + `Enter`，F12 截本 viewport | newer.txt 左右路径正确，**行级 + 词级差异高亮**，状态 `1 important`，[截图](assets/folder-integration-open-text.png) |
| 文本（110 MB × 2） | `matcha.exe <tmp/left.txt> <tmp/right.txt> --bench=600 --screenshot=…` | update p99 1.538 ms，frame CPU p99 2.479 ms，[截图](assets/folder-integration-text.png) |
| 表格 | `matcha.exe 002-key-value-change/{left,right}.csv --bench=60 --screenshot=…` | update p99 0.407 ms，正常退出 |

文本基准仍满足 update p99 <4 ms / CPU p99 <8 ms，但高于 master 报告的 1.129 ms；
本次紧接 10 万文件基准运行，未做多轮复测，不据此声称文本性能无变化或有回退。
`tmp/` 只读使用。窗口消息注入不是物理鼠标键盘输入，不冒称物理输入验收。

旧报告的人工反馈“`newer.txt` 打开后看不出差异”由正式文本视图解决（见上表第三行）。

## 偏差、限制与后续

- 文件夹规则比较只开放 `comments-unimportant` 一项重要性设置；完整文本重要性/对齐设置、
  逐会话格式覆盖随会话项目后置（对照清单已改为“部分 / 后置”）。
- 规则比较需读入全文；大文件按设计在后台进行且可取消，未另设大小上限。
- Home 最近列表沿用 master 的文本/表格记录，不记录文件夹对。
- FolderSync 视图下点击 Home 仍直接返回（无未保存数据，与 Wave C 相同）。
- `bench_dir.ps1` 固定写 `folder-waveC-bench.png`、`gui_smoke.ps1` 默认名为
  `folder-waveC-smoke`：本轮跑完后已还原已提交的 Wave C 截图，冒烟显式使用新名称；
  用户本地修改的 `folder-waveC-smoke.png` 与未跟踪的 `manual-01.png` 未改动、未提交。
- 原生物理输入与系统剪贴板/回收站极端交互仍按 Wave C 报告的人工清单由负责人复测。
- 评审通过后归档本 change（Workflow follow-up）。

## 提交

- `696fb68` merge: integrate master (formal text view, table compare) into folder-compare-sync
- `781f004` feat(bc-engine-dir): run rules-based content comparison through bc-engine-text
- 本报告与 tasks.md 勾选另起 docs 提交。
