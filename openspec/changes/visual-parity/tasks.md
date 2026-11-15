# Tasks

说明：2026-11-10 用户要求串行执行、控制范围（核心：共享 chrome + 文本比较 + 文件夹比较主视图；标签类、Home、表格/同步换肤、对话框、右键菜单后置）。全程约束见 design D8；调用参考工具前持 `%TEMP%\matcha-reference-tool.lock`；同一 worktree 只允许一个提交者；不读取授权/密钥文件、不写参考工具安装目录、不提取或描摹其图标资源；每个任务组至少一个 Conventional Commits 提交。

## 1. 参考采集与度量（分支 `sunlaibing/visual-ref`）

- [x] 1.1 编写固定语料 `tools/visual/corpus/`（文本对：基础差异/长文件/CJK 与制表符/行尾差异；文件夹对：含孤儿、较新、相同、子目录；CSV 对；同 mtime 固定为 2026-01-02 03:04:05 的设置脚本）；验证脚本可重复生成且 git 中无大文件
- [x] 1.2 编写 `tools/visual/capture-reference.ps1`：持锁、启动参考工具、固定外框、裁客户区、等待比较完成、场景操作（键盘/菜单/展开）、弹出菜单用屏幕拷贝、必要的设置文件按备份-写-恢复纪律；验证 ≥ 20 个场景（Home、文本 ×6、各顶层菜单展开 ×7、文件夹比较 ×3、文件夹同步、表格比较 ×2、会话设置/选项/查找对话框）截图入库 `docs/visual-parity/reference/`
- [x] 1.3 度量并写 `docs/visual-parity/tokens.json` 与说明 `tokens.md`：颜色、几何、字体（含参考 Options 中的默认编辑器字体/字号）、每视图菜单树/工具栏/状态栏/右键菜单清单、场景命名区域矩形；验证 JSON 可被 `tools/visual/compare.ps1` 读取
- [x] 1.4 编写 `tools/visual/compare.ps1`（只用 PowerShell + System.Drawing）：并排图、差异热图、命名区域边界检测与偏差表、颜色 token 抽样核对，输出 `target/visual/compare/report.md`；验证用参考图对自身比较偏差为 0

> 任务组 1 按 2026-11-10 协调者收窄范围验收：核心文本/文件夹度量优先；其余精调 deferred；Find 按实际改为 text-find-bar；停止参考工具后仅离线收尾。详见 tokens.md。

## 2. 外观基础（master）

- [x] 2.1 `chrome/fonts.rs`：系统 Segoe UI / 编辑器字体 / CJK 回退加载、DPI 与快照模式 `pixels_per_point=1`；验证单测：字体缺失回退不 panic
- [x] 2.2 `chrome/theme.rs`：token 常量（初值取种子截图，组 3 按 tokens.json 校准）与全局 Visuals/Style；验证单测：token 与 tokens.json 一致（tokens.json 未合入前跳过）
- [x] 2.3 `chrome/{menubar,toolbar,pathbar,statusbar,splitter,scrollbar}.rs` 通用组件（数据驱动：菜单树、工具栏条目、状态字段）；验证 egui 输入注入单测：菜单打开/助记符/快捷键显示、切换按钮态、分体按钮下拉
- [x] 2.4 `chrome/icons.rs` 原创图标集（覆盖四个视图工具栏全部条目与路径栏/状态栏小图标）+ `icon-catalog` 场景；验证快照
- [x] 2.5 快照基础设施：`--snapshot=<scene>`、`--window-size=WxH`、场景注册表、等待后台完成、禁用闪烁、`tools/visual/snap-matcha.ps1`；验证同场景连续两次快照字节一致

## 3. 文本比较外观（master，Wave 2；核心区域优先）

- [x] 3.1 菜单栏（Session/File/Edit/Search/View/Tools/Help 全树，接既有动作；无对应实现的项按参考显示为禁用）与大图标工具栏（条目、顺序、切换态、分体按钮）；现有按钮行功能全部迁移，迁移清单入报告
- [x] 3.2 路径栏、信息头（时间/大小/格式▾/编码▾/行尾）、缩略图栏、窗格（行号槽、箭头槽、差异底色与差异字符色、缺行斜纹、EOF 后网纹、当前行、选区、编辑器字体）
- [x] 3.3 每侧窗格状态、行详情（两行、可见行尾符）、底部状态栏字段、窗口标题 `<左> <--> <右> - Text Compare - Matcha`
- [x] 3.4 按 tokens.json 校准组 2 初值；文本场景对照：区域偏差 ±2 px、颜色 token 一致、并排图评审；`--bench` 不退化

## 4. 文件夹比较/同步外观（master，Wave 3；仅 4.1 本期，4.2/4.3 中同步部分后置）

- [x] 4.1 文件夹比较菜单/工具栏/过滤栏/路径栏/双树（列头、行高、展开符、文件夹与文件图标、名称颜色语义、中缝比较符号）/状态栏
- [ ] 4.2 文件夹同步视图同上；操作确认对话框外观
- [ ] 4.3 文件夹场景对照达标并合回 master（只在 `dir_view.rs`/`sync_view.rs`/`filters_bar.rs` 与 `app.rs` 最小接线范围冲突）

## 5. Home 与表格比较外观（后置：2026-11-10 用户收窄范围，抓大放小）

- [ ] 5.1 Home：左侧会话树（分组/最近/自动保存）、右侧新会话类型网格、工具栏与菜单
- [ ] 5.2 表格比较：菜单/工具栏/路径栏/信息头/网格（表头、行号、状态色、差异单元格色、键列标记）/sheet 标签/行详情/状态栏
- [ ] 5.3 Home 与表格场景对照达标；`--bench-table` 不退化

## 6. 对话框与菜单（后置；注：参考工具 Find/Replace 实为停靠查找条，非对话框，见 tokens 场景 text-find-bar）

- [ ] 6.1 会话设置（文本/文件夹/表格各页签）、Options、Find/Replace、File Formats、Go To 的窗口尺寸、页签、控件布局与按钮行（OK/Cancel/Help）
- [ ] 6.2 右键菜单（文本窗格、文件夹树、表格网格）条目与顺序；弹出菜单阴影/边框/高亮色
- [ ] 6.3 对话框与菜单场景对照达标

## 7. 收尾

- [x] 7.1 全场景对照报告 `docs/reference-parity/visual.md`（每场景并排图链接、区域偏差表、已知差异及理由）；clippy/test/基准 A/B 回归；Workflow follow-up 记录

## Workflow follow-up

- 2026-11-11 核心版收口：1A（参考采集）、1B（chrome）、2/2b（文本视图）、3（文件夹视图 4.1）完成；7.1 核心版报告 `docs/reference-parity/visual.md`（协调者复核 clippy 零警告、429 tests passed）。执行者：Codex（1A/1B/2，gpt-6-astra high）→ pi k3.1-pro（2b/3）。
- 后置未做：4.2/4.3 同步视图、任务组 5（Home、表格）、任务组 6（对话框、右键菜单）、标签类、缩略图。change 不归档，等后置范围由用户决定。


- **2026-11-15 外观方向决定（用户）**：采用现代风设计系统（ADR 402：`chrome::widgets`/`window`/下拉菜单，抹茶绿强调色，
  `theme::Palette`），**不做经典 Windows 1:1 复刻**。任务组 6（对话框/右键菜单）若重启，目标是对照参考工具的
  信息结构（页签、控件、条目与顺序、按钮行），视觉样式以设计系统为准，不复刻 Win32 方角标签页、像素箭头、阴影等经典外观；
  任务组 4.2/4.3、5 同理。change 仍不归档，剩余范围待用户决定。
