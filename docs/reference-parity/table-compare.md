# Table Compare 公开行为对照

## 范围与证据口径

本清单对应 `table-compare-xlsx` 的 D9 与任务 5.3，来源是指定 CHM 的只读解包主题。**清单覆盖完整，不表示 GUI 功能全部完成**；4.1–4.5 和最终集成仍须按任务验收。本轮代码测试使用 egui RawInput/tessellation，不冒称锁定桌面上的物理鼠标或原生 IME 候选窗已验收。

- `commandstable.html` 含 **77 个 p_Command 段落**，其中包含菜单分组和拆开的子项，不等于 77 个独立可执行命令。
- 下表逐段保留原顺序与名称，包括两个重复的 `Open With >` 和隐藏标记 `*`，避免菜单组被去重后漏项。
- “已有实现”仅指代码路径存在；“部分/待专项验收/矩阵待补”明确不能作为任务完成证据。
- 已批准后置：会话持久化、父文件夹联动、外部程序、独立报告、全局字体；依据 [ADR 202](../decisions/202-table-view-and-golden.md) 及 design D9。
- 目前唯一正式黄金差异为 Unaligned 报告路径，见 [KNOWN_DIFFS](../../crates/bc-core/tests/golden/table/KNOWN_DIFFS.md)。不把该路径推断为参考 GUI 全部行为。
- 实施/回归入口：`bc-gui/src/table/{view,commands,grid,header,details,sheet_tabs,settings_panel,find,worker,cache,selection}.rs`，`tests.rs`、`edit_tests.rs`；算法与模型由 `bc-engine-table` 负责。

## 命令逐项：`commandstable.html`

### Session

| 编号 | 原帮助段落 | 状态 | 入口 / 差异 / 证据边界 |
|---|---|---|---|
| C01 | `Save Session` | 已批准后置 | session-and-cli 统一会话持久化；菜单明确禁用，不发明过渡文件格式。 |
| C02 | `Save Session As` | 已批准后置 | session-and-cli 统一会话持久化；菜单明确禁用，不发明过渡文件格式。 |
| C03 | `Session Settings` | 已有实现、矩阵待补 | settings_panel.rs：五个内存设置页；当前比较可应用，完整场景矩阵仍属 4.4。 |
| C04 | `Locked` | 未接线 | 会话锁定尚未实现；Specs 的 Disable editing 不等价于锁定会话。 |
| C05 | `Clear Session` | 未接线 | Home 有离开确认，但不是清空为新 Table Session；尚待统一会话命令评审。 |
| C06 | `Swap Sides` | 已有实现、待专项验收 | commands.rs::swap：交换路径、格式、模型、历史、映射和锚点后后台比较；多 sheet 缓存边界待验。 |
| C07 | `Reload Files` | 已有实现、待专项验收 | commands.rs::request/perform：脏数据确认后读取来源；失败及双侧编辑边界待 4.4。 |
| C08 | `Recompare Files` | 已有实现 | view.rs::recompare/worker.rs：快照 Job、旧 revision 拒绝；保留历史，不在 update 同步对齐。 |
| C09 | `Table Compare Report` | 按 D9 后置 | 按 D9 后置 hex-and-reports；当前结构化摘要不等于 HTML/打印报告。 |
| C10 | `Table Compare Info` | 已有实现、待专项验收 | 缓存当前及其他 sheet 三态/孤儿/列数；未扫描结果标 pending，不伪装为 Same。 |
| C11 | `Compare Files Using` | 部分 | 仅接线 Text Compare；需已有文件路径，其他会话类型未接线。 |
| C12 | `Compare Parent Folders` | 已批准后置 | 等待文件夹线合并，明确禁用；尚未产生文件夹子会话。 |

### File

| 编号 | 原帮助段落 | 状态 | 入口 / 差异 / 证据边界 |
|---|---|---|---|
| C13 | `Open File` | 已有实现、待专项验收 | 本地路径框、Home 和拖放；格式覆盖和已有脏数据交互待 4.4。 |
| C14 | `Open File on File System *` | 能力合并 | 本地能力与 Open File 合并；路径框代替原生文件选择器，无单独隐藏别名按钮。 |
| C15 | `Open File Using Profile *` | 范围外 | 远端/VFS profile 选择与写入不在当前本地来源适配中，未连接。 |
| C16 | `Open Clipboard` | 已有实现、待专项验收 | 请求 Paste 载荷加载指定侧并设 clipboard_dirty；跨侧缓存与保存完整回归待 4.4。 |
| C17 | `Open With` | 按 D9 后置 | 按 D9 留 session-and-cli/外部工具处理；未调用关联程序或系统文件管理器。 |
| C18 | `Open With >` | 菜单分组 | 原帮助的菜单前缀段落；重复保留两个编号，不作为新增可执行命令。 |
| C19 | `Associated Application` | 按 D9 后置 | 按 D9 留 session-and-cli/外部工具处理；未调用关联程序或系统文件管理器。 |
| C20 | `Open With >` | 菜单分组 | 原帮助的菜单前缀段落；重复保留两个编号，不作为新增可执行命令。 |
| C21 | `Text Edit` | 未接线 | 独立 Text Edit 尚未连接；不能把双文件 Text Compare 当成此项。 |
| C22 | `Save File` | 已实现、部分回归 | 独立目的侧保存点、暂存+rename；先提交该侧草稿，xlsx 原格式禁用并提示 CSV 出口。 |
| C23 | `Save File As` | 已实现、部分回归 | 本地路径框选择目标，另一侧仍保持 dirty；备份/失败的完整矩阵待 4.4。 |
| C24 | `Save File on File System As *` | 能力合并 | 与本地 Save File As 共用能力，没有重复隐藏按钮。 |
| C25 | `Save File Using Profile As *` | 范围外 | 远端/VFS profile 选择与写入不在当前本地来源适配中，未连接。 |
| C26 | `Explorer` | 按 D9 后置 | 按 D9 留 session-and-cli/外部工具处理；未调用关联程序或系统文件管理器。 |

### Edit

| 编号 | 原帮助段落 | 状态 | 入口 / 差异 / 证据边界 |
|---|---|---|---|
| C27 | `Undo` | 已实现、部分回归 | 目的侧 EditStack + 后台重算；TextEdit 草稿使用本地历史，已提交编辑生成新 revision 防 ABA。 |
| C28 | `Redo` | 已实现、部分回归 | 目的侧 EditStack + 后台重算；TextEdit 草稿使用本地历史，已提交编辑生成新 revision 防 ABA。 |
| C29 | `Align With` | 已有实现、待专项验收 | 先选源行再点对侧，锚点作为 EditItem；清除入口存在，完整输入回归仍待 4.3。 |
| C30 | `Copy to Right / Copy to Left *` | 能力合并 | 方向别名并入 Copy to Other Side，按活动侧选择目的侧。 |
| C31 | `Copy to Other Side` | 部分 | 单格复制/撤销已有回归；多选批量复制的事务性、排序视图和空隙处理仍需 4.3。 |
| C32 | `Copy Row to Right / Copy Row to Left *` | 能力合并 | 方向别名并入 Copy Row to Other Side。 |
| C33 | `Copy Row to Other Side` | 部分 | 整行复制有引擎测试；GUI 多选/排序视图场景仍待验。 |
| C34 | `Cut / Copy / Paste / Delete` | 安全路径已补、完整矩阵待补 | edit_tests.rs 验证 TSV 引号/tab/CRLF/空字段往返、Find 焦点隔离与菜单粘贴；完整 IME/批量矩阵待 4.3。 |
| C35 | `Delete Row` | 多选已补 | 可见投影映射到源行集合，一次重建删除；过滤选区与撤销已有回归。 |
| C36 | `Delete Column` | 多选已补 | 按比较列映射到源列，降序删除并修正映射；错序源列的多选删除/撤销已有回归。 |
| C37 | `Insert Row Before` | 已有实现、部分回归 | 以当前行确定前/后插入位置，返回一个 EditItem；空表、边界、完整 redo 矩阵待 4.3。 |
| C38 | `Insert Row After` | 已有实现、部分回归 | 以当前行确定前/后插入位置，返回一个 EditItem；空表、边界、完整 redo 矩阵待 4.3。 |
| C39 | `Insert Column Before` | 已有实现、部分回归 | 新增列并调整该侧映射，另一侧内容不改；完整交互矩阵待 4.3。 |
| C40 | `Insert Column After` | 已有实现、部分回归 | 新增列并调整该侧映射，另一侧内容不改；完整交互矩阵待 4.3。 |
| C41 | `Modify` | 已改为单元格内编辑 | 编辑浮层锚定单元格；Ctrl+Enter 提交、Escape 取消。草稿纳入 Home/Exit/保存确认，失败保留草稿，等待中的对齐结果不移动编辑坐标。 |
| C42 | `Sort` | 待确认并补齐 | 现有实现仍只用当前列；拟增加稳定多键排序，跨行选区是否限定排序范围待用户确认。 |
| C43 | `Select All` | 部分 | 生成边界选区后按可见投影执行数据命令；空表/陈旧缓存边界仍需 4.3 验证。 |

### Search

| 编号 | 原帮助段落 | 状态 | 入口 / 差异 / 证据边界 |
|---|---|---|---|
| C44 | `Next Difference Cell` | 已有实现、部分回归 | commands.rs::navigate：行优先、列次序，Ignore Unimportant 生效，跳转滚入视野。 |
| C45 | `Previous Difference Cell` | 已有实现、矩阵待补 | 反向、不循环；完整首尾/过滤/隐藏列组合待 4.2。 |
| C46 | `Next Difference Row` | 已有实现、矩阵待补 | 基于差异行缓存，不逐帧扫描表；反向及首尾组合待 4.2。 |
| C47 | `Previous Difference Row` | 已有实现、矩阵待补 | 基于差异行缓存，不逐帧扫描表；反向及首尾组合待 4.2。 |
| C48 | `Next Difference Files` | 按 D9 后置 | 按 D9 后置文件夹父/子会话；Copy Row 不是文件级复制并打开下一项。 |
| C49 | `Previous Difference Files` | 按 D9 后置 | 按 D9 后置文件夹父/子会话；Copy Row 不是文件级复制并打开下一项。 |
| C50 | `Copy File to Right and Open Next Difference` | 按 D9 后置 | 按 D9 后置文件夹父/子会话；Copy Row 不是文件级复制并打开下一项。 |
| C51 | `Copy File to Left and Open Next Difference` | 按 D9 后置 | 按 D9 后置文件夹父/子会话；Copy Row 不是文件级复制并打开下一项。 |
| C52 | `Next Edit` | 已有实现、矩阵待补 | 按编辑行标记定位；完整输入矩阵和首尾提示验证仍待 4.2。 |
| C53 | `Previous Edit` | 已有实现、矩阵待补 | 按编辑行标记定位；完整输入矩阵和首尾提示验证仍待 4.2。 |
| C54 | `Find` | 已有实现、部分回归 | 字面/正则、大小写/全词、指定侧，命中定位到单元格；已有控制器回归。 |
| C55 | `Replace` | 已有实现、矩阵待补 | 当前命中/全部替换使用 Pattern 捕获；零宽与方向组合仍待 4.4。 |
| C56 | `Find Next` | 已有实现、部分回归 | 正向/反向、不循环；当前实现扫描命中，不宣称后台搜索已经实现。 |
| C57 | `Find Previous` | 已有实现、矩阵待补 | 正向/反向、不循环；当前实现扫描命中，不宣称后台搜索已经实现。 |
| C58 | `Go To` | 已有实现、矩阵待补 | 比较行/比较列的 1-based 定位并钳制边界；与过滤、隐藏列的组合仍待验。 |

### View

| 编号 | 原帮助段落 | 状态 | 入口 / 差异 / 证据边界 |
|---|---|---|---|
| C59 | `Show All` | 已有实现 | Projection 全行视图，不改来源编号或内容。 |
| C60 | `Show Differences` | 已有实现、部分回归 | 仅差异投影；Ignore Unimportant 生效，过滤后的删除不触及隐藏行。 |
| C61 | `Show Same` | 已有实现、矩阵待补 | 差异区间的补集；完整切换组合待 4.2。 |
| C62 | `Show None *` | 已有实现、矩阵待补 | 投影为空、模型/统计保留；隐藏状态下的数据命令边界待专项验收。 |
| C63 | `Ignore Unimportant Differences` | 已有实现、部分回归 | 仅将 U 投影为 S，保留原三态；影响导航、过滤、计数和 sheet 色点。 |
| C64 | `Hide Same Columns` | 已有实现、矩阵待补 | 按列汇总隐藏无差异列；隐藏计数提示与组合场景仍待 4.1。 |
| C65 | `Resize Columns to Fit` | 已有实现、矩阵待补 | 初始有界采样；显式命令由 worker 扫描全部已加载内容，长字段输入回归待补。 |
| C66 | `Visible Whitespace` | 已有实现、矩阵待补 | 空格/tab/换行可视符号仅用于绘制，不写入文件。 |
| C67 | `Row Numbers` | 已有实现、矩阵待补 | 显示源行编号和空隙；开关的完整输入回归待 4.1。 |
| C68 | `Display Font *` | 按 D9 后置 | D9 后置全局偏好；系统 CJK fallback 不是这些命令的实现。 |
| C69 | `Increase Display Font Size *` | 按 D9 后置 | D9 后置全局偏好；系统 CJK fallback 不是这些命令的实现。 |
| C70 | `Decrease Display Font Size *` | 按 D9 后置 | D9 后置全局偏好；系统 CJK fallback 不是这些命令的实现。 |
| C71 | `Reset Display Font Size *` | 按 D9 后置 | D9 后置全局偏好；系统 CJK fallback 不是这些命令的实现。 |
| C72 | `Side-by-Side Layout` | 已有实现、部分回归 | 共享 x/y 偏移的左右/上下网格；已有二维虚拟化、几何与 tessellation 回归。 |
| C73 | `Over-Under Layout` | 已有实现、部分回归 | 共享 x/y 偏移的左右/上下网格；已有二维虚拟化、几何与 tessellation 回归。 |
| C74 | `Thumbnail` | 已有实现、矩阵待补 | 固定 512 桶和可视范围，点击定位入口已有；过滤组合的输入矩阵待 4.1。 |
| C75 | `Text Details` | 已有实现、部分回归 | 两侧完整逻辑值、公式和数字格式；不计算公式。 |
| C76 | `File Info` | 已有实现、矩阵待补 | 路径/字节/只读/修改提示已有；编码诊断、缓存 sheet 和保存目标展示待核验。 |
| C77 | `Toolbar` | 待补齐 | 命令栏常驻，完整隐藏/恢复入口尚未实现，保留为 4.1 缺口。 |

## 会话设置主题

### `sessiontable.html`

五页入口 Specs / Format / Columns / Alignment / Sheets 已有内存草稿、Apply 和验证路径。并非持久化会话；完整 Apply/取消、跨页、脏编辑与重载失败矩阵仍在 4.4。

### `sessiontablespecs.html`

| 选项 | 当前实现 / 边界 |
|---|---|
| Left / Right 文件 | 两侧路径框；改变路径会重新读取并触发脏数据确认。 |
| Disable editing | 每侧保护标志；修改与保存检查，readonly 测试已有。 |
| Description | 当前设置窗口内存文字，不写会话文件；需会话持久化时后续统一接入。 |

### `sessiontableformats.html`

| 选项 | 当前实现 / 边界 |
|---|---|
| Detected / 文件格式掩码 | Home 目前只按 csv/tsv/xlsx/xlsm 后缀；任意用户格式掩码的路由/选择尚未完整连接，不声称已完成 4.1。 |
| 左/右格式选择 | 现有 table 内置预设和逐侧 Type/Regional 选项；更改需重新读取。 |
| Encoding overrides / None | 自动检测、UTF-8/16、1252/936 等 UI 项；引擎支持其明示代码页集合，失败不静默吞掉。 |
| 覆盖优先级 | ReadOptions 承载当前会话解析设置，完整格式默认/会话覆盖切换场景仍待 4.4。 |

### `sessiontablecolumns.html`

| 选项 | 当前实现 / 边界 |
|---|---|
| 比较列、左/右列号 | ColumnMapping，允许不同源列或 gap；界面 0 表示空映射，展示列号 1-based。 |
| 样本数据 / Handling 摘要 | 当前列表主要显示列号和结构标记，尚非完整原生样本/摘要布局。 |
| Move Up / Move Down 单侧 | 左或右的源列顺序移动；接口/往返有引擎测试，完整 GUI 矩阵待补。 |
| 移动整条比较列 | 同时移动映射和 Handling。 |
| Tidy | 删除双侧都为空的映射。 |
| Key 与优先级 | 比较列次序决定复合键优先级；无显式键使用全部列。 |
| Edit Default | 默认 Handling 草稿；Use Default 列继承，键标记仍逐列。 |
| 超出项 | 自动键建议带“自动”标注并可手动锁定；结构检测/Accept 与按列号兼容预设已有，交互矩阵待补。 |

### `dlgtablecolhandling.html`

| 选项 | 当前实现 / 边界 |
|---|---|
| Key / Use Default | 分别控制键与默认处理继承，不把会话默认 Key 传播给每列。 |
| General / Text | General 解释数值/日期/文本；Text 保留原字段拼写。 |
| Unimportant | 原始值不同则 U，Ignore Unimportant 时才在投影视为 S。 |
| Character case | 仅规范化后相等的原始差异为 U，不是 S；本机黄金已校准。 |
| Whitespace | 同上，前后与词间数量变化按规则处理。 |
| Numeric tolerance | 容差内差异为 U；原本相同的值仍 S。 |
| Date tolerance | 内部毫秒，GUI 小时输入（24 = 一天）；引擎有天/小时构造。 |
| 格式重要性 | 数字格式默认 U、引擎可配重要性；GUI 配置及三档完整回归仍待 4.4/4.5。 |

### `sessiontablealignment.html`

| 选项 | 当前实现 / 边界 |
|---|---|
| Sort rows before alignment | 按 Handling 的键稳定排序后交给 core；不同于 Edit > Sort 的数据重排。 |
| Standard | core Smart；有界 closeness，匹配后仍拒绝不等键（手动锚点例外）。 |
| Myers O(ND) | core Quick 的键序列骨架，不重写 diff 算法。 |
| Patience | core Patience，保留取消和手动锚点。 |
| Unaligned | 项目规格为 n↔n；原生报告 024 的例外完整锁定在 KNOWN_DIFFS。 |
| Use closeness matching | 可关闭，策略选择仍为 Standard；GUI 名称与文本线一致。 |
| Left file is source | 列/Sheet 名称配对顺序；底层已支持，完整界面重排验收待 4.4。 |
| 中间结果 | 原子发布最终快照；已有内容可在重比期间显示，不宣称复刻原生分段增量对齐输出。 |

### `sessiontablesheets.html`

按归一化名称配对，单侧 Sheet 保留；Move Up/Down 重排一侧以手动改变配对。实现有引擎测试，GUI 跨标签缓存、脏编辑保留和手动配对仍需 4.4 的完整回归。

## 文件格式主题

### `formattable.html`

General / Conversion / Type / Regional 为参考分类。当前 TableOptions 承载 Type/Regional；GUI 内存选项并不等于已完成原生格式管理器的每个面板。

### `formattablegeneral.html`

- Mask、Description：bc-formats 模型/注册表存在；表格 GUI 对任意自定义 mask 的应用未完整接线。
- 内置预设保持只读来源语义；不能把修改会话选项说成改写了内置资源。

### `formattableconversion.html`

| 项 | 处理 |
|---|---|
| Conversion method / 原生文本 | CSV/固定宽度由本引擎读取，xlsx/xlsm 由 calamine 原生读取。 |
| External program、Loading / Saving 命令行 | design D2/D9 明确不执行；不从格式配置启动任意外部程序。 |
| `%s %t %o` | 仅参考语法说明，无变量展开/执行实现。 |
| 转换程序返回 0 且输出非空 | 未执行转换器，因此不宣称已实现这项调用协议。 |
| 文件名 Unicode / ANSI | 无外部转换器参数编码路径；本地 std::path 与文本编码独立处理。 |
| Disable editing / Saving | GUI 每侧保护；xlsx 可内存修改但不能原格式保存，提供明确 CSV 导出。 |
| Encoding / 自动检测 | 增量解码与显式覆盖，原字节未整文件额外复制；GUI 可选项如前表。 |
| Ctrl+Z EOF | 已在解析器及格式选项提供，源测试覆盖 EOF 后垃圾。 |

### `formattabletype.html`

| 选项 | 当前实现 / 验证边界 |
|---|---|
| Delimited | 引号、双引号转义、字段内换行由 Records 处理。 |
| Detect | 有界前缀检测分隔符；读诊断可返回检测结果，完整 GUI 提示待验。 |
| Delimiters | 逗号/tab/分号/空格/pipe/自定义集合。 |
| Text qualifier | 双引号、单引号或 None；不能表示的保存值报错，不静默损坏。 |
| Treat consecutive delimiters as one | 解析器与格式设置均有，空白分隔测试通过。 |
| Treat surrounding whitespace as part of delimiter | 按设置修剪外侧，不重写未编辑的原字段。 |
| Fixed / Column widths | 分号分隔的正字符宽度，不是字节偏移；固定宽度写回不在本 change，导出 CSV。 |
| First line contains | Auto/Data/Names；工作簿和文本装载路径使用同一显式选择。 |

### `formattableregional.html`

Use system / Decimal separator / Thousands separator / Date order / Date separator 均有模型与内存设置。Windows GUI 由加载 worker 只读解析系统区域并缓存，不修改系统/进程 locale；无法取得时使用文档化 invariant 回退。显式设置优先，严格解析失败保留文本。完整原生 GUI 交互和所有区域组合仍待 4.4，不把单元测试推定为全平台 locale 覆盖。

## 总览与多 Sheet

### `viewtable.html`

- 双网格按比较列/比较行组织，不以源文件列号直接假设一一对应；仅键相等配对，排序/手动锚点按明确规则。
- 红/蓝/无色表示 I/U/S，行首摘要不受水平可见列限制；数据状态不在绘制帧重新计算。
- 行列二维可见区绘制已存在，历史原生基准与边界见 Wave 2 报告；本清单不是本轮新性能测量。

### `table_working_with_multiple.html`

- Sheet 标签用于切换当前比较；后台可逐 Sheet 计算色点/统计，未计算使用 pending 状态。
- 同文件两侧可以通过 Sheets 重排比较不同 Sheet；底层配对有测试，完整 GUI 情况仍待 4.4。
- HTML tables 按 design 非目标，不接入转换器伪装实现。
- 当前层档位/Ignore Unimportant 应影响标签与导航；扫描取消后的恢复及缓存编辑路径仍需专项验收。

## 额外协议与校准

- 值 / 值+公式 / 值+公式+数字格式：新功能，不是参考原生转换器的等价能力。字体/填充沿用 D2 后置，公式不求值。
- 摘要 `MATCHA-TABLE-DIFF` 文本/CSV：只输出当前投影的变化，载荷是不可信源数据，不是执行指令；不代替 Table Compare Report。
- 原生黄金采用 `load <default>` 后把表格会话名作为 `table-report` 参数；直接 `load` 表格会话会报错。规则证据与可逆 NUL 包装见 [黄金说明](../../tools/golden/README.md)。
- 当前三态校准基于真实原生报告，不将 U/S 统一后再统计；原始状态与唯一例外签名保留。

## 覆盖验证

```text
python tools/golden/check_table_parity.py --help-dir <只读解包的帮助目录>
```

校验器逐段比较名称/顺序/编号，检查 D9 的 15 个主题均已列出；重复菜单前缀不去重。任何缺段、改名或缺主题都失败。**该验证只证明文档覆盖，不证明所有条目已经实现或通过交互验收。**
