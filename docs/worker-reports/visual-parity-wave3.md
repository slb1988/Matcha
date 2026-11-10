# visual-parity Wave 3（任务 4.1：文件夹比较主视图核心外观）

## 结论

- 文件夹比较视图按参考换成 chrome 布局：菜单栏（Session/Actions/Edit/Search/View/Tools/Help，条目按 `inventories.folder` 转录）、大图标工具栏（Home、Sessions▾、All、Diffs▾、Same、Files▾、Minor、Rules、Copy、Expand、Collapse、Select、Files、Refresh、Swap、Stop）、过滤栏（`Filters:` + 组合框 + Filters/Peek，45 px）、每侧路径栏（可编辑组合框 + 小图标按钮，26 px）、双树（列头 24 px、行高 20、列宽 Name 437 / Size 78 / Modified 150、树线、文件夹/文件小图标、名称颜色语义、中缝）、日志区 106 px、状态栏 25 px。
- 三场景（folder-basic / folder-expanded / folder-diffs-filter）`compare.ps1` 命名区域全部 ≤ 2 px（headers/trees/splitter/statusbar 精确 0；menubar/toolbar/log/filterbar/pathbar 边缘 ±1–2 为 Wave 2b 同款外缘检测噪声）。`tools/visual/manual-verify-folder.py` 手工逐点核验 22 项边界 worst |dev| = 2 px。
- 颜色样本：folder-basic 2/2 精确通过（folderSelection #A8CDF1、folderStripe #F8FAF9）；folder-expanded 5 项全部因 egui 灰度 AA vs GDI ClearType 的字形覆盖率差异未命中采样点——采样点落在文字笔划上，按 HANDOFF 第 8 节记已知差异（最近邻证据见下）。
- `snap-matcha.ps1 -All -Verify` 八场景两次哈希一致（文本三场景哈希与 Wave 2b 记录逐字相同，未回退）；`cargo clippy --workspace --all-targets -- -D warnings` 零警告；`cargo test --workspace` 81 个目标 429 passed / 0 failed；触及文件 rustfmt 无差异。
- 引擎 crate、bc-core、bc-session 零修改；同步视图保持旧布局（`sync_mode` 分支，连 Modified 的 UTC 文本与 "—" 占位都未动）；无新增依赖。
- 提交：`9d438e6`（布局与 chrome 迁移）+ `e127a6d`（树线肘形、同步隔离、测试）+ 本报告提交。

## 并排图（入库）

- [folder-basic](assets/visual-wave3-folder-basic-side.png)
- [folder-expanded](assets/visual-wave3-folder-expanded-side.png)
- [folder-diffs-filter](assets/visual-wave3-folder-diffs-filter-side.png)

## 命名区域偏差（compare.ps1 自动扫描，四边 左/上/右/下）

|区域|folder-basic|folder-expanded|folder-diffs-filter|
|---|---|---|---|
|menubar|−2,0,−2,0|−2,0,−2,0|−2,0,−2,0|
|toolbar|−2,0,0,0|−2,0,0,0|−2,0,0,0|
|filterbar|1,0,2,0|1,0,2,0|1,0,2,0|
|pathbar.left|1,0,2,0|1,0,2,0|1,0,2,0|
|pathbar.right|2,0,2,0|2,0,2,0|2,0,2,0|
|headers.left|0,0,0,0|0,0,0,0|0,0,0,0|
|headers.right|0,0,0,0|0,0,0,0|0,0,0,0|
|tree.left|0,0,0,0|0,0,0,0|0,0,0,0|
|tree.right|0,0,0,0|0,0,0,0|0,0,0,0|
|splitter|0,0,0,0|0,0,0,0|0,0,0,0|
|log|−2,0,2,0|−2,0,2,0|−2,0,2,0|
|statusbar|0,0,0,0|0,0,0,0|0,0,0,0|

`menubar/toolbar` 的 −2 外缘与 Wave 2b 文本视图相同（扫描对客户区外缘的检测噪声）。手工核验（manual-verify-folder.py）覆盖：菜单栏底 y27、工具栏底线 y70、过滤组合框 x43/y81、路径框 x1/593/702、列头顶线 y141–142、列头外缘 x664/1365（#A0A0A0）、树接缝 x664/702（#C0C0C0，只到内容末行）、树底 y830/y831、日志底 y935、状态栏顶 y936、滚动条槽 #F0F0F0——三场景 worst |dev| = 2 px。

## 颜色样本

folder-basic：2/2 通过（容差 2）。

folder-expanded 5 项均未通过，全部是文字/小方块笔划覆盖率差（egui 灰度 AA vs GDI ClearType 全强度笔划核心），非取色错误。最近邻证据（实测图中距采样点 ≤6 px 内存在语义正确的像素）：

|token|采样点|期望|实测（采样点）|最近邻（≤6px）|
|---|---|---|---|---|
|folderDifferent|(48,193)|#FF0000|#F8FAF9（条纹底，字形缝隙）|(255,1,1) @ (54,194)，Δ1|
|folderNewer|(728,433)|#FF0000|#A8CDF1（选中底）|(252,6,8) @ (734,434)，Δ8|
|folderOlder|(27,433)|#808080|#A8CDF1（选中底）|(129,130,132) @ (32,434)，Δ4|
|folderOrphan|(63,253)|#6A00D5|#FFFFFF（白底，字形缝隙）|(107,1,214) @ (65,256)，Δ1|
|folderSame|(62,373)|#000000|#FFFFFF（白底，字形缝隙）|(5,5,5) @ (66,376)，Δ5|

## 快照确定性

`tools/visual/snap-matcha.ps1 -All -Verify`（先钉住语料 mtime 再构建，每场景跑两次比对 SHA256）：

```text
icon-catalog        A65C53F7… verified=True（目录新增 6 枚原创图标，哈希变化属预期）
chrome-demo         532DC16A… verified=True
text-basic          04E9AA13… verified=True（与 Wave 2b 记录一致）
folder-basic        BCFE37E8… verified=True
text-diffs-filter   722A05AF… verified=True（同上）
text-cursor-on-diff FFD0B5DD… verified=True（同上）
folder-expanded     86977D7E… verified=True
folder-diffs-filter 060B1F8C… verified=True
```

快照预置与参考操作等价：folder-basic = 折叠 + 首行双侧选中；folder-expanded = 先按 Ctrl+A 语义只选顶层行、再 Expand All（与参考 `Ctrl+A; Edit > Expand All` 同序，故子行不被选中）；folder-diffs-filter = 工具栏 Diffs（DisplayFilter::Mismatches）+ 首行选中。

## 旧入口 → 新入口（迁移表）

|旧入口|新入口|
|---|---|
|Session 菜单按钮（Browse base folders / Swap sides / Up left/right/both / Compare parent folders / Rules / Sync base folders）|Session 菜单：Browse for Folder▾、Swap Sides、Up One Level▾、Compare Parent Folders、Session Settings...（Rules 按钮同）、Sync Base Folders|
|Actions 菜单按钮（打开/展开/设基/对齐/快速比较/内容比较/复制移动删除重命名属性 Touch/Exclude/Ignore/CopyFilename/刷新所选 等全部条目）|Actions 菜单同名项；Compare Contents 由叶子改为子菜单（CRC / Binary / Rules-based）；Ignored 改为切换（全忽略则清除）|
|Selection 菜单按钮（Select all / all files / newer / orphans / invert）|Edit 菜单：Select All (Ctrl+A)、Select All Files (Shift+Ctrl+A)、Select Newer▾、Select Orphans▾、Invert Selection；工具栏 Select / Files|
|View 菜单按钮（Toolbar/Log/Legend/Advanced/各列勾选）|View 菜单同名项 + Columns▾（Ext/Status/Revision/Owner/Group 未实现禁用；Size/CRC/Modified/Attributes/Version 启用）|
|Rules / Find / Find next / Find previous 按钮|工具栏 Rules；Search 菜单 Find Filename (Ctrl+F) / Find Next (F3) / Find Previous (Shift+F3)|
|Back / Forward 按钮|Session 菜单 Back/Forward（Alt+Left/Right）+ 路径栏回退钮与右侧带回退列表的前进/回退钮|
|Expand all / Collapse all 按钮|工具栏 Expand / Collapse；Edit 菜单同名项|
|Refresh / Full refresh 按钮|工具栏 Refresh；Edit 菜单 Refresh (F5) / Full Refresh (Ctrl+F5)；Actions 菜单 Refresh Selection (Shift+F5)|
|Previous / Next difference 按钮|Search 菜单 Next/Previous Difference (Ctrl+N/P)；旧 F7 导航保留|
|Filters 文本行（名称过滤 + Apply + Presets + Add preset + Show all/Files only 下拉 + Suppress/Ignore unimportant + Other filters）|过滤栏组合框（名称过滤，预设在内嵌下拉）+ Filters 按钮（应用）+ Peek 钮（=Suppress filters 切换）；显示过滤/文件夹模式/不重要差异在 View 菜单；Add preset 与 Other filters 字段迁入 Session Settings（Rules）窗口|
|路径文本行（只读）|每侧可编辑路径组合框（回车重载，内嵌历史下拉）|
|统计行（"N files, M folders; K visible rows; J selected"）与消息行|状态栏：无选择时显示参考的 "Shows ... items." 图例；纯文件选择时显示每侧 "N file(s), B bytes" + 剩余空间（一次性 PowerShell 查询，装载时取）；忙碌时显示进度文本（Stop 钮可用）。总数统计无参考对应字段，不再显示|
|日志区（130 px 底面板 + 头部行）|参考位置/高度的 106 px 日志区：齿轮=收起、文档=保存日志（弹出路径窗）；内容沿用现有（装载行 + OpLog 记录，无时间戳以保证快照确定）|
|app 顶条 Home 按钮|工具栏 Home（经文本子窗口保存确认）；app 不再为文件夹比较画顶条|
|中缝比较符号列（"."、"="、"!=" 文本）|参考中缝为空白：比较视图不再绘制（同步视图保留）|

未迁移的旧入口：无（全部行为可经新入口到达）。旧按钮行/旧过滤行在比较视图删除；同步视图继续用同一函数的旧布局。

## 数据/语义修正（均在 dir_view.rs 内）

- **Modified 列**：本地时区 `M/D/YYYY H:MM:SS AM/PM`（月/日/时不补零），用装载时已有的 `system_clock()` 偏移；同步视图保持 UTC。
- **尺寸/日期缺省**：比较视图空白（参考），同步视图保持 "—"。
- **图标**：文件夹图标按内容聚合着色（未扫描=黄、含差异=红、含孤儿=紫、全同=灰，四段垂直渐变 + 标签页，原创矢量）；文件图标为白底 8×8 + 5×6 语义色芯（参考的"未知类型"小方块）；树线为 #808080 逐像素点线的父→子肘形。
- **名称颜色**：文件夹名恒黑（颜色由图标承载），文件名与单元格用语义色（孤儿 #6A00D5、不同/较新 #FF0000、较旧 #808080、相同黑），选中行不反色（参考实测如此）；文件夹 Size/Modified 恒 #969696。
- **选中行**：整行 #A8CDF1 贯通双树与中缝；焦点行有跨全宽的黑色虚框。

## 已知差异（均不超核心区域）

- 菜单栏无参考的 "New version available..." 更新提示链接（通知类，与 Wave 2b 一致不做）。
- **行序**：既有比较排序把"对侧缺失"条目排在同组末尾（发布排序属比较行为，不在外观范围，按协调者 2026-11-10 指示不改）：参考为名称合并序（right-only 在 left-only 与 same 之间、newer.txt 在 older.txt 之前），本实现中 right-only/newer.txt 靠后。行几何/颜色语义不受影响。
- 路径栏文本为本地相对语料路径（参考为另一机器的绝对路径）——路径内容差异，非几何。
- egui 灰度 AA vs GDI ClearType 的字形强度差：落在文字笔划上的 5 个颜色样本未命中（证据见上表）。
- 文件夹/文件图标为原创近似（红线要求不描摹参考图标资源）；参考图标带 GDI 位图渐变，我们的为四段渐变。
- 树线点相位与参考差 1 px（x15/36 vs 参考实测一致，点在奇偶位相差 1）。
- 状态栏右下缩放把手已复刻（#BFBFBF 六点）；窗口边框/标题栏不在客户区快照范围。
- 文本视图 Snapshots 显式 `case_unimportant=true` 的跟进项不变（HANDOFF 第 5 节）。

## 性能 A/B（基线 `3d5a5dc` worktree 构建 vs 本波 HEAD；各 1 轮）

夹具：`bc-engine-dir` 例子 `dir_ui_fixture` 生成 100,000 文件/侧（`$TEMP/matcha-wave3-fixture`），`--bench-dir` 600 帧滚动协议；文本用 `target/table-wave7/audit/inputs/text-*.txt`（110 MB×2），`--bench` 600 帧。日志 `target/visual/wave3/bench/`。

|基准|A=3d5a5dc p99|B=本波 p99|Δ|
|---|---:|---:|---:|
|文件夹 `--bench-dir`|0.665 ms|0.561 ms|**−15.6%**（更快：自绘只画可见行）|
|文本 `--bench`|1.371 ms|1.402 ms|**+2.3%**（<10%，文本视图未受影响）|

## 最后十行

`cargo clippy --workspace --all-targets -- -D warnings`，exit 0：
```text
    Checking bc-gui v0.1.0 (D:\Github\Matcha\crates\bc-gui)
    Checking bcompare v0.1.0 (D:\Github\Matcha\apps\bcompare)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.64s
```
`cargo test --workspace`：81 个测试目标全绿，合计 **429 passed / 0 failed**（新增 4 项：chrome 菜单/工具栏动作与参考几何、菜单命令驱动既有动作、快照预置、本地时间格式）。

触及文件 `rustfmt --edition 2021 --check --config skip_children=true` 无差异：`crates/bc-gui/src/{dir_view,filters_bar,app,snapshot}.rs`、`crates/bc-gui/src/chrome/icons.rs`。

## 合规

未启动参考工具；未读取 ObsidianVault；compare.ps1 / tokens.json 未改；无新依赖；`tools/visual/snap-matcha.ps1` 仅在场景清单新增两项并在构建前钉住语料 mtime；`manual-verify-folder.py` 为新增的手工核验脚本（与 Wave 2b 的 manual-verify.py 平行）。
