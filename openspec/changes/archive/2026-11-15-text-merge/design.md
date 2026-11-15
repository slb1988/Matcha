# Design

## Context

动机见 proposal.md。以下是动手前对现状的核对（均已对照代码），后面每个决定都以此为前提。

**已就位**

- `bc-core::align`：`AlignRow` / `AlignMap` 已含 `center` 列与 `center_to_row` 反查；`Aligner::align3` 的三个实现是 `todo!`。
- `bc-engine-text`：`TextMap.center: Option<TextContent>`、`TextMapRow.center` 字段存在；`merge.rs`（`MergeState`、`auto_merge`）与 `jobs.rs` 的 `Align3Job` / `Realign3Job` 全是 `todo!`，没有任何调用方，**签名可以按本设计调整**。
- `bc-script`：`Invocation::Merge3/Merge4` 与 `/automerge` `/center=` `/favorleft|right` `/force` `/iu` `/mergeoutput=` `/reviewconflicts` `/title1..4` 都能解析；`ExitCode` 0/14/100/101 已入表；`runner::auto_merge_exit_code` 已实现（依赖 `AutoMergeOutcome.withheld`）。

**缺口（原计划未覆盖或判断有误）**

- 行表只有两列：`CompactRow`（`#[repr(C)]`，left/right 两个 `u32`）、`AlignedData` 只有左右哈希和两侧 revision；`AlignedData::row` 恒返回 `center: None`，`TextMap::text_at(Side::Center)` 返回 `None`。
- `Align3Job` 没有 `overrides` 字段。双方比较的真实对齐管线是 `jobs::align_range`（权重、closeness、skew、never-align、Unaligned），不是裸 `Aligner::align2`。
- 等待：只有环境变量 `MATCHA_LAUNCHER_WAIT`（`matcha.com` 设置）会让转交方阻塞。直接运行 `matcha.exe` 时，若已有实例则转交后立即以 0 退出；`matcha-comp.exe` 启动主程序后立即返回。
- 退出码：`remote::Outcome` 只有 `Closed` / `Failed`，不带结果码；`run_gui` 成功即 `ExitCode::SUCCESS`。首个实例与 `/solo` 两条路径没有任何通道把标签页结果变成进程退出码。
- GUI 复用面：`text/editor.rs`（面向 `TextContent`）、`text/projection.rs`、`text/gutter.rs::paint` 是独立组件；`text/render.rs` 是 `impl TextCompareView` 的方法（`pane(&mut self, ui, s, …)`），**不能直接复用**。
- 命名会话尚未实现：`Invocation::Session` 仍是 Unsupported，`Session::new` 是 `todo!`，文本比较的 Save Session 在 parity 表里是"后置"。
- `routing.rs` 现有单测 `help_and_unsupported_invocations` 断言 `a b c` 为 Unsupported，要随本变更改写。

## 参考依据

来自参考工具帮助（BC 5 `BCompare.chm`，取得方式同 `docs/reference-parity/text-compare.md`：只读解包到临时目录）。任务 1.1 把逐项对照落到 `docs/reference-parity/text-merge.md`，这里只列直接决定设计的条目。

| # | 出处 | 参考行为 | 决定 |
|---|---|---|---|
| R1 | viewtextmerge | 合并会话 = 两到三个**不可编辑**的源窗格 + 一个可编辑输出窗格 | D8 |
| R2 | using_text_merge | 输出初始等于 center，能自动采纳的非冲突改动除外；改动按来源窗格着色；直接编辑输出使该区段变黄；再次 Take 可恢复为某一输入 | D5、D9 |
| R3 | using_text_merge | 左右两侧的不同改动落在同一行、或彼此相距两行以内，这些区段算冲突；两侧做了相同改动不算冲突 | D4 |
| R4 | sessiontextmergealignment | "A merge conflict is"：`Only different change lines`，或 `Left change and right change separated by <N> lines` | D4 |
| R5 | command_line_reference | `/automerge`：无冲突时不经交互自动合并；`/reviewconflicts`：`/automerge` 发现冲突时打开合并视图；`/force`：`/automerge` 时把冲突以 CVS 风格标记写入输出 | D6 |
| R6 | command_line_reference | `/iu`：`/automerge` 时打开 Ignore Unimportant Differences；`/favorleft|right`：被忽略的不重要冲突改从 favored 侧取、而不是取 center（另有输出着色抑制的显示效果） | D5 |
| R7 | command_line_reference | 3 个文件 = left、right、center；4 个再加 output；`/center=`、`/mergeoutput=` 显式指定；`/title1..4` 显示在各路径框 | D6 |
| R8 | viewtextmerge | 输出文件的既有内容被忽略，保存即覆盖 | D7 |
| R9 | commandstextmerge | Take Left/Center/Right 作用于选中行或当前区段；Conflict 命令可清除/设置冲突；Next/Previous Conflict Section | D9 |
| R10 | command_line_reference | 主程序单实例：第二个实例通知已有实例后立即退出；需要等待的集成走启动器 | D10 |
| R11 | shot_textmerge.png | 相邻的左删除与右删除被标成同一个冲突区段，但输出里两处删除都已采纳 | D4、D5 |

## Goals / Non-Goals

**Goals:**

- center-based 三方对齐，直接复用双方对齐管线，每一对（left↔center、center↔right）的配对结果与单独做双方比较相同。
- 合并语义只有一份实现：`classify` / `auto_merge` 是不依赖 GUI 的纯函数，CLI 与 GUI 共用。
- `/automerge` 在发起进程内无窗口执行；P4V 以 `matcha.exe %1 %2 %b %r`（GUI）或 `matcha.exe /automerge /reviewconflicts %1 %2 %b %r`（能自动合并就不开窗）直接可用，且**不论是否已有实例在运行**，调用方都等到合并结束并拿到正确退出码。
- GUI 合并视图：三个只读输入窗格 + 可编辑输出窗格，冲突导航、Take、保存。

**Non-Goals（一期不做，写明以免实现时摇摆）:**

- 表格 key 列三方合并、文件夹三方合并（`TDirMergeSession`）、patch / stdin 调用。
- 输入窗格编辑及其增量重对齐（参考工具的源窗格本就不可编辑，R1）。
- 合并视图内修改规则：会话设置对话框、Ignore Unimportant / Favor 开关、手工对齐（Align With / Isolate）。规则在打开时确定。
- 命名会话保存/重开、Home 内的合并入口、recent 列表记录合并——依赖尚不存在的会话持久化；一期入口是命令行与单实例转交。
- Take Left Then Right / Take Right Then Left、按行 Take、显示过滤（Show Conflicts 等）、隐藏 center 窗格、独立输出窗口、favor 的着色抑制效果。
- `/savetarget=`、`/vcs1..4`、`/nobackups`、`/ro*` 在合并调用中的行为（接受但不产生效果，见 D6）。
- 修改 `matcha-comp.exe` 的等待语义（已决定另起变更，见 Open Questions Q1）。

## Decisions

### D1: 三方对齐 = 两次双方对齐 + 以 center 为轴调和

`Align3Job::execute` 用与 `Align2Job` 相同的管线（`align_range`：哈希、权重、closeness、skew、never-align、Unaligned）分别求 `A = align(left, center)` 与 `B = align(center, right)`，再调和为三列行表。参数顺序按窗格的左右次序（center 在 A 中居右、在 B 中居左），这样每一对的配对与用户单独打开对应双方比较看到的一致。

调和规则（策略无关，`bc-core` 中只实现一次，三个策略的 `align3` 都委托它）：

- 每个 center 行 `c` 产生一行 `(A 中与 c 配对的 left?, c, B 中与 c 配对的 right?)`。
- 未与 center 配对的 left / right 行（孤儿）归入"桶"：桶号 = 该行在各自双方行序中、位于它之前的 center 行数。同一桶内左右孤儿按位置并排成 `(l, -, r)`，多出的单独成行；桶内行排在同桶号的 center 行之前。

不变量（写成属性测试）：

- I1：每侧的行号各出现恰好一次且递增（即通过 `AlignMap::try_from_rows`）。
- I2：配对保持——三方行表里的 left↔center 配对集合等于 A 的配对集合，center↔right 等于 B 的。
- I3：`left == center` 时，结果等于 B 的行表且每个 center 行都带同号 left；`right == center` 同理。

备选：独立 diff3 算法。放弃：与 `AlignMap` 行表模型不兼容，重要性与权重语义要重做一遍。

### D2: 三方行表 = 既有两列行 + 平行的 center 列

`CompactRow` 不变（它在双方比较的热路径上，加一列会让每个双方比较多占三分之一行表内存）。`AlignedData` 增加：平行的 `center: Vec<u32>`（双方比较时为空）、`center_hashes`、center 的 revision。`AlignedData::row` 在 center 列非空时填 `TextMapRow.center`；`TextMap::text_at(Side::Center, …)` 可用；`is_stale` / `set_alignment` 把 center revision 计入。

三方行的 `importance` = 两侧各自相对 center 的重要性取大者，所以既有的 `sections()` 与 `next_difference` 直接给出"改动段"。`kind` 在三方行上只区分 `Same` / `Changed`；行的合并类型以 `MergeRow` 为准（D3）。

约束：`size_of::<CompactRow>()` 不变（加断言测试）；双方比较的黄金测试与滚动基准不回归。

### D3: 行类型（逐行，相对 center）

对每个对齐行，用既有分类器分别比较 left↔center 与 right↔center（一侧是空隙、另一侧有行也算不同），再比较 left↔right：

| 行类型 | 条件 | `MergeChoice` |
|---|---|---|
| 未变 | 两侧都与 center 相同 | `Center` |
| 左改 | 仅 left 不同 | `Left` |
| 右改 | 仅 right 不同 | `Right` |
| 同改 | 两侧都不同，且 left 与 right 相同 | `Both` |
| 异改 | 两侧都不同，且 left 与 right 也不同 | `Conflict` |

"不同"的判定：默认任何差异都算（重要与不重要一视同仁，不重要的单侧改动照样被采纳——否则一侧只改缩进的修改会在合并结果里被悄悄还原）。开启 `ignore_unimportant`（`/iu`）时，重要性为 Unimportant 的差异按相同处理。

### D4: 冲突区段（按区段，不按行）

行类型决定内容，冲突是加在**区段**上的待审标记。

- 改动段 = 连续的非未变行。
- 一个改动段含异改行，或同时含左侧改动（左改/异改）与右侧改动（右改/异改），它本身就是冲突区段。
- 相邻两个改动段之间的未变行数 ≤ `conflict_gap`，且一段带左侧改动、另一段带右侧改动时，两段连同中间的未变行并成一个冲突区段；并入后继续向后按同一规则扩展。
- 同改行不算任何一侧的改动。

`TextMergeRules` 新增 `conflict_gap: u32`（默认 2，对应 R3/R4），删除没有参考依据的 `conflict_on_center_change`。`Conflict.importance`：区段内全部差异都不重要时为 Unimportant，否则 Important。

为什么按区段：两侧在同一位置插入不等长内容时，对齐行是 `(X1,-,Y1) (X2,-,-) (X3,-,-)`。只按行判定会把 X2、X3 当成"仅左改"自动采纳，解决冲突时取右侧还会留下半截左侧内容。区段化之后整块一起判定、一起解决。

### D5: 输出的默认内容

逐行：未变取 center，左改取 left，右改取 right，同改取 left，异改取 center。冲突区段内同样适用——相距很近但不重叠的两侧改动会各自被采纳，只是区段带着待审标记（R11）；真正重叠的行保持 center，等待决定（R2）。

`/iu` 与 favor：

- 开启 `ignore_unimportant` 后，不重要的差异不构成改动，相应行取 center。
- 仅由不重要差异构成的冲突区段（被忽略的不重要冲突）不计入冲突；favor 为 Left/Right 时该区段改取 favored 侧，否则取 center（R6）。
- favor 在未开启 `ignore_unimportant` 时不改变输出内容。

### D6: 命令行语义

**参数归一**：`CliArgs` 增加 `merge_spec() -> Option<MergeSpec { left, right, center, output: Option<_> }>`，把 `Merge3`、`Merge4`、`Files + /center=` 归一；`/mergeoutput=` 与第 4 个位置参数同时出现时以 `/mergeoutput=` 为准。路由、headless、GUI 三处只认 `MergeSpec`。

**合并调用不按文件格式路由**：一律文本合并（CSV/TSV 按行）。文件格式取 center 路径的文件名、规则取默认文本规则（与 `/qc`、无界面报告相同的默认格式库）。任一输入是文件夹、不存在、不可读或被判为二进制（含 xlsx）→ 错误。

**`/automerge`**：`routing.rs` 新增 `Route::AutoMerge { spec, … }`，判定位于 `/qc` 之后、`unsupported` 之前；不满足三方调用或没有输出目标 → `Route::Unsupported`（退出码 100）。常驻实例收到带 `/automerge` 的转交请求时拒绝（`unsupported()` 增加 "cannot be handed off" 分支，与 `/qc`、`/report` 一致）。执行体放在 `bc_script::runner::auto_merge_files`（仿 `quick_compare_files`），`main.rs` 只负责输出与退出码。

| 结果 | 输出文件 | 退出码 |
|---|---|---|
| 无冲突区段 | 写出 | 0 |
| 有冲突，带 `/reviewconflicts` 且无 `/silent` | 不写，转入 GUI 合并（走与普通合并调用相同的单实例/`/solo` 路径） | 由 GUI 决定（D11） |
| 有冲突，带 `/force` | 写出，冲突区段带标记 | 14 |
| 有冲突，其他 | 不写，已存在的文件保持原样 | 101 |
| 参数/加载/写出错误 | 不写 | 100 |

`/reviewconflicts` 优先于 `/force`；`/silent` 使 `/reviewconflicts` 失效。成功或有冲突时向 stdout 写一行结果（退出码描述，有冲突时附冲突区段数），`/silent` 时不写；进程没有标准输出时（P4V 直接启动 `matcha.exe`）也不写，更不能像错误那样退化成消息框——否则每次自动合并成功都会弹一个框拦住调用方。错误写 stderr（无控制台时为消息框，与 `/qc` 一致）。

**其他开关**：`/solo` 照常；`/title1..3` 用作输入窗格标题与冲突标记标签，`/title4` 用作输出窗格标题；`/iu`、`/favorleft|right`、`/force` 只在 `/automerge` 时生效（含由 `/reviewconflicts` 转入的 GUI 会话，使界面看到的冲突与自动合并判定一致）；`/ro*`、`/nobackups`、`/savetarget=`、`/vcs1..4` 接受但不产生效果。

理由：P4V 等调用方靠进程退出与结果文件判定；转交给常驻实例会让"是否写出"脱离调用方控制。

### D7: 输出字节

- **行尾**：输出的每一行保留其来源行的行尾字节，不做归一化。三份输入行尾风格不一致时结果可能混合行尾，`/automerge` 在结果行追加一句警告（不改变退出码）。默认规则不比较行尾（`line_endings_unimportant = true` 时分类器对"文字相同、行尾不同"返回 Same），所以纯行尾差异的行算未变、取 center——一侧只做了整体行尾转换时，这个转换不会进入合并结果。
- **编码与 BOM**：对"编码+BOM"这一属性做同样的三方取舍——两侧都与 center 相同取 center；只有一侧不同取那一侧；两侧都不同且彼此不同取 center 并警告。所选编码无法无损表示输出文本时报错（headless 退出码 100；GUI 为保存失败）。
- **冲突标记**（`/force`）：每个未解决冲突区段渲染为

  ```
  <<<<<<< <left 标签>
  <区段内 left 各行>
  =======
  <区段内 right 各行>
  >>>>>>> <right 标签>
  ```

  标签 = `/title1`、`/title2`，未给出时取路径的文件名。标记行的行尾 = center 中第一个带行尾的行的行尾（center 没有则依次取 left、right，都没有则 LF）。
- **末行无行尾**：只有输出的最后一行可以没有行尾。来源行没有行尾（它是某个输入的末行）而在输出里又不是最后一行时，补上与标记行相同的行尾——否则它会和下一行粘在一起（例如 center 末行无换行、right 在其后追加了一行）。冲突标记块内某侧最后一行同理。
- **写出**：先完整读入三份输入再写；写同目录临时文件后重命名覆盖。输出路径允许与某个输入相同（写出前必须释放该输入的内存映射，否则 Windows 上替换失败）。输出文件的既有内容从不读取（R8）。合并输出不创建备份文件。

### D8: 范围取舍

- 三个输入窗格恒只读，所以合并会话里没有"输入被编辑"这件事。
- 规则在打开时确定，会话内不变，所以没有"改规则后保留已解决决定"这件事。
- `Realign3Job` 一期以全量重对齐实现（结果必须与 `Align3Job` 相同），只为去掉 `todo!`；增量窗口留给真正需要它的变更。

原计划在规格里要求增量重对齐，又在风险里说"一期可先全量"，两者矛盾；这里统一为后者，并把规格改成只要求等价性。

### D9: GUI 合并模型

新增 `bc-gui/src/merge/`，`View::TextMerge`。

- **输出缓冲**：独立的 `TextContent`（自带撤销栈），初始内容 = D5 的默认输出。每个输出行记录来源（Left / Right / Center / Manual）与所属对齐行。
- **行网格**：四个窗格共用对齐行网格滚动。某对齐行在输出里对应多行（手工插入）时，输入窗格在该处显示空隙行；对应零行时输出显示空隙行。
- **Take Left / Center / Right**：作用于光标所在区段（冲突区段或普通改动段），把该区段的输出行整体替换为所取侧的行（含覆盖手工行），并清除冲突标记。
- **接受当前内容**（清除冲突标记）：不改内容，只把区段标为已解决。近距离冲突（两侧改动都已采纳）只能靠它确认，所以是必需命令而不是可选项。
- **手工编辑**：编辑或新增的输出行标为 Manual；被手工编辑过的冲突区段视为已解决。Take 与编辑同在输出缓冲的撤销栈里。
- **导航**：上一/下一冲突区段只在未解决的冲突间跳转；状态栏显示未解决冲突数。
- **着色**：四个主题色位——左改动、右改动、冲突、手工（参考：teal / magenta / red / yellow），色值走 `chrome/theme`，不在视图里写死。

### D10: 等待与退出码的三条路径

| 启动路径 | 等待 | 退出码来源 |
|---|---|---|
| 已有实例 → 转交 | **合并调用一律等待**：`Request.wait = WAIT_ENV 或 merge_spec().is_some()` | `Completion` 回复；`Outcome` 扩展为携带结果码 |
| 本进程成为首个实例 | 进程活到窗口关闭 | `run_with_remote` 返回"启动标签页"关闭时记下的结果码，`run_gui` 以它退出 |
| `/solo` | 同上 | 同上 |

- 合并标签页的结果码见 D11；其他视图恒为 0，行为不变。
- 已知限制：后两条路径下，如果用户在同一窗口又开了别的标签页，调用方会一直等到整个窗口关闭。只开合并一页时，关页即关窗。文档里给 P4V 的推荐配置用 `matcha.exe`；`matcha.com` 也能等，但从 GUI 程序启动时会出现控制台窗口；`matcha-comp.exe` 不等待，**不能**用于合并。

理由：原计划写"经 `matcha.com` 等待"，但任务里给 P4V 的配置又是 `matcha.exe`——照那样配置，只要 Matcha 已经开着，P4V 会在用户动手之前就收到退出码 0。让合并调用隐式等待后，哪个可执行文件都不会出这个问题。

### D11: GUI 保存与结果码

- 有输出目标的合并会话从打开起就算"未保存"（磁盘上还没有合并结果），关闭时走既有的未保存确认。没有输出目标时，保存即另存为。
- 保存写出的是输出窗格的当前内容（所见即所得），不注入冲突标记——标记只属于 `/automerge /force`（R5）。
- 仍有未解决冲突时保存，先提示并由用户确认。

| 标签页关闭时的状态 | 结果码 |
|---|---|
| 最后一次保存时没有未解决冲突 | 0 |
| 最后一次保存时仍有未解决冲突 | 14 |
| 从未保存 | 101 |
| 打不开（输入缺失/二进制等） | 100 |

原计划的"确认后按 `/force` 语义带标记写出"改掉了：窗格里显示的是 center 内容，写到文件里的却是标记，保存后界面与磁盘不一致。

### D12: 渲染复用方式

`editor.rs`、`projection.rs`、`gutter.rs` 是独立组件；`render.rs` 的窗格绘制是 `impl TextCompareView` 的方法。原定先做一次不改行为的抽取、再让合并视图接上去，并留了退路：抽取代价超出预期时，合并视图自带一份精简绘制，不动文本比较。

**实施结果：走了退路。** `pane()` 有 535 行，与文本比较的投影、行内差异缓存、查找、手工对齐、右键菜单状态交织在一起，抽取等于重写双方比较的热路径，风险与本变更的收益不相称。合并视图在 `merge/view.rs` 里自带绘制，复用 `Editor`（输出窗格的编辑、选区、输入法）以及 `render.rs` 的 `layout` 与 `hatch` 两个函数（改为 `pub(crate)`）。代价：合并窗格没有语法着色和行内差异高亮，纵向滚动是视图自己的一条滚动条，没有横向滚动条（Shift+滚轮可横向滚动）。

## 待参考样本确认

下列语义目前按帮助文字与截图暂定，任务 1.2–1.3 用参考工具跑样本后确认或修正（修正时同步改 spec）；P7 无法脚本化，在任务 7.2 人工观察。无法采集时按暂定语义实现，并在 feature 文档状态里写明"未经参考样本确认"。

| # | 暂定 | 要回答的问题 |
|---|---|---|
| P1 | `conflict_gap` 默认 2，按"两段之间未变行数 ≤ N"计 | 默认值与计数口径 |
| P2 | 近距离冲突的输出保留两侧已采纳的改动；重叠行取 center | `/force` 输出里这类区段长什么样 |
| P3 | 标记为 CVS 两段式（无 base 段），标签取标题或文件名 | 是否含 base 段、标签写法、行尾 |
| P4 | `/iu` 下不重要的单侧改动取 center；favor 不带 `/iu` 不改变输出 | `/iu`、favor 各组合的输出 |
| P5 | 行尾逐行保留；编码按三方取舍 | 输入编码/行尾不一致时的输出字节 |
| P6 | 101 时不触碰已存在的输出文件 | 101/14/0 各自是否写文件 |
| P7 | GUI：保存且无冲突 0、带冲突保存 14、未保存 101 | 启动器在这三种情况下的退出码 |
| P8 | 合并输出不建备份 | 参考工具是否为合并输出建备份 |

## Risks / Trade-offs

- **调和后的行序与参考工具不完全一致**（两侧在同一 center 区间都有大量插入时）→ 以 I1–I3 为硬约束，行序差异按 `KNOWN_DIFFS` 的做法登记，不为追行序破坏不变量。
- **默认 `conflict_gap = 2` 比 diff3 保守**，P4V 下 `/automerge` 的成功率会低于 `p4 resolve -am` → 这是参考默认；配合 `/reviewconflicts` 时代价只是多开一次窗口。
- **首个实例路径的等待要到窗口关闭**（D10）→ 文档写明；不在本变更里改启动器。
- **`render.rs` 抽取触及文本比较的热路径** → 以快照逐像素不变与滚动基准不回归把关，有退路（D12）。
- **输出路径等于某输入时的内存映射**（D7）→ 单独的集成测试覆盖。
- **P4V 的判定依据未经实测**（退出码还是结果文件是否改动）→ 两者都保证：未保存不动 `%r` 且返回非零；任务 7.2 实测后把结论写进 feature 文档。

## Migration Plan

纯新增能力，无数据迁移。`routing.rs` 中 `a b c` / `a b c d` 从 Unsupported（退出码 100）变为打开合并视图，属行为变化，写入 CHANGELOGS 与 `docs/features/cli-and-scripting.md`。

## Open Questions

- **Q1**：`matcha-comp.exe` 目前"转交后立即返回"，而参考帮助把 `BComp.exe` 描述为供版本控制系统调用的 GUI 程序，并且退出码表里有 102 "BComp.exe unable to wait until BCompare.exe finishes"——据此推断它是等待的（未实测）。本变更用 D10 的隐式等待绕开，不动它。**已决定（2026-11-14）**：另起一个变更把 `matcha-comp.exe` 改成等待并修订 ADR 301，不并入本变更；那个变更动手前先实测参考 `BComp.exe` 是否等待。
- **Q2**：Home 内的合并入口——**已决定（2026-11-14）**：一期不做，保持 Non-Goal。
- **Q3**：`/reviewconflicts`（任务 6.7）——**已决定（2026-11-14）**：保留。P4V 推荐配置为 `matcha.exe /automerge /reviewconflicts %1 %2 %b %r`（没有冲突不弹窗，有冲突才开合并视图）。
