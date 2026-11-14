# 文本合并：参考帮助逐项对照

## 范围与证据

做法同 [text-compare.md](text-compare.md)：只读解包本机参考帮助（`BCompare.chm`）的 HTML 到临时目录，逐项核对
`commandstextmerge.html`、`sessiontextmerge.html`、`sessiontextmergespecs.html`、`sessiontextmergeimportance.html`、
`sessiontextmergealignment.html`、`viewtextmerge.html`、`using_text_merge.html`、`3-way_merge_concepts.html`，以及
`command_line_reference.html` 中与合并有关的开关和退出码。没有读取授权/密钥文件，没有写安装目录。

状态含义：**已实现**＝有真实入口/引擎行为及回归；**进行中**＝在 change `text-merge` 范围内，尚未落地；
**有意未采纳**＝与本项目本期设计冲突或被等价入口替代；**后置**＝有价值但属于后续能力，不宣称当前可用。
命令名称仅为内部对照标签。

对应变更：`openspec/changes/text-merge/`；功能文档：[docs/features/text-merge.md](../features/text-merge.md)。

## 设计依据（design R1–R11）

| # | 出处 | 参考帮助原意 | 本项目采纳 |
|---|---|---|---|
| R1 | viewtextmerge | 合并会话含两到三个不可编辑的源窗格与一个可编辑的输出窗格 | 三个输入窗格恒只读 |
| R2 | using_text_merge | 输出初始与 center 相同，可自动采纳的非冲突改动除外；改动按来源窗格着色；直接编辑输出使该区段变黄；再次 Take 可恢复为某一输入 | 默认输出、来源着色、手工色、Take 覆盖手工内容 |
| R3 | using_text_merge | 左右两侧的不同改动在同一行或相距两行以内时，这些区段算冲突；两侧做相同改动不算冲突 | 冲突按区段判定，`conflict_gap` 默认 2，同改不计 |
| R4 | sessiontextmergealignment | "A merge conflict is"：`Only different change lines`，或 `Left change and right change separated by <N> lines` | `TextMergeRules::conflict_gap` |
| R5 | command_line_reference | `/automerge` 无冲突时不经交互自动合并；`/reviewconflicts` 在 `/automerge` 发现冲突时打开合并视图；`/force` 在 `/automerge` 时把冲突以 CVS 风格标记写入输出 | 同 |
| R6 | command_line_reference | `/iu` 在 `/automerge` 时打开 Ignore Unimportant Differences；`/favorleft|right`：被忽略的不重要冲突从 favored 侧取而不是取 center，并抑制该侧非冲突改动在输出中的着色 | 输出内容部分采纳；着色抑制后置 |
| R7 | command_line_reference | 3 个文件为 left、right、center，4 个再加 output；`/center=`、`/mergeoutput=` 显式指定；`/title1..4` 显示在各路径框 | 同 |
| R8 | viewtextmerge | 输出文件的既有内容被忽略，保存即丢失 | 不读取既有输出 |
| R9 | commandstextmerge | Take Left/Center/Right 作用于选中行或当前区段；Conflict 清除或设置冲突；Next/Previous Conflict Section | Take 作用于当前区段；"接受当前内容"对应清除冲突 |
| R10 | command_line_reference | 主程序单实例，第二个实例通知已有实例后立即退出；需要等待的集成使用启动器 | 合并调用隐式等待（design D10） |
| R11 | shot_textmerge.png | 相邻的左删除与右删除同属一个冲突区段，输出中两处删除都已采纳 | 冲突区段内仍逐行采纳非重叠改动 |

## 待参考样本确认（design P1–P8）

样本语料：`crates/bc-engine-text/tests/golden/merge/`，采集脚本：`tools/golden/gen_merge_expected.ps1`。

**采集状态（2026-11-14）：未能采集。** 本机参考程序（5.0.6）在，但三种写法都打开 `left.txt <--> right.txt - Text Compare` 窗口而不是合并：`left right center out`、`left right center`、`/fv="Text Merge" left right center out`（带不带 `/automerge` 都一样），输出文件不产生。**原因未确认**：帮助里合并各页带 Pro 标记，推断是本机许可为不含三方合并的版本；按仓库约定没有读许可文件核对。
下表全部保持暂定，Matcha 的合并行为目前只有帮助文字与截图作依据，没有逐字节样本。参考工具能打开 Text Merge
之后运行 `gen_merge_expected.ps1`（20 个用例 × 4 种开关组合）即可补齐。

| # | 暂定语义 | 结论 | 样本 |
|---|---|---|---|
| P1 | `conflict_gap` 默认 2，按两段之间未变行数 ≤ N 计 | 未能采集（见上） | — |
| P2 | 近距离冲突的输出保留两侧已采纳的改动；重叠行取 center | 未能采集（见上） | — |
| P3 | 标记为 CVS 两段式（无 base 段），标签取标题或文件名 | 未能采集（见上） | — |
| P4 | `/iu` 下不重要的单侧改动取 center；favor 不带 `/iu` 不改变输出 | 未能采集（见上） | — |
| P5 | 行尾逐行保留；编码按三方取舍 | 未能采集（见上） | — |
| P6 | 101 时不触碰已存在的输出文件 | 未能采集（见上） | — |
| P7 | GUI：保存且无冲突 0、带冲突保存 14、未保存 101 | 待人工观察（任务 7.2） | — |
| P8 | 合并输出不建备份 | 未能采集（见上） | — |

## command_line_reference.html — 合并相关

| 条目 | 状态 | 对应实现或理由 |
|---|---|---|
| 3 files | 已实现 | `Invocation::Merge3` → 合并视图（left、right、center） |
| 4 files | 已实现 | `Invocation::Merge4` → 合并视图 + 输出目标 |
| `/automerge` | 已实现 | 发起进程内无窗口执行，`bc_script::runner::auto_merge_files` |
| `/center=` | 已实现 | 两个文件 + `/center=` 归一为三方调用 |
| `/favorleft` `/favorright` | 已实现 | 带 `/automerge` 或 `/reviewconflicts` 且有 `/iu` 时，被忽略的不重要冲突取 favored 侧；着色抑制后置 |
| `/force` | 已实现 | `/automerge` 有冲突时带标记写出，退出码 14 |
| `/fv="Text Merge"` | 后置 | 合并调用本身已固定为文本合并；两个文件强制开合并视图不在本期 |
| `/iu` | 已实现 | 带 `/automerge` 或 `/reviewconflicts` 时不重要差异按相同处理 |
| `/mergeoutput=` | 已实现 | 输出目标，优先于第 4 个位置参数 |
| `/nobackups` | 有意未采纳 | 合并输出本就不建备份；开关接受但无效果 |
| `/reviewconflicts` | 已实现 | `/automerge` 有冲突时转入合并视图 |
| `/ro` `/ro1` `/ro2` | 有意未采纳 | 输入窗格恒只读；开关接受但无效果 |
| `/savetarget=` | 后置 | 接受但无效果 |
| `/silent` | 已实现 | 不写结果行；使 `/reviewconflicts` 失效 |
| `/solo` | 已实现 | 合并在独立窗口打开 |
| `/title1..4` | 已实现 | 各窗格标题；前两个兼作冲突标记标签 |
| `/vcs1..4` | 后置 | 接受但无效果 |
| 退出码 0 / 14 / 100 / 101 | 已实现 | `bc_script::ExitCode`；`/automerge` 与 GUI 合并两条路径 |

## commandstextmerge.html — Session

| 命令 | 状态 | 对应实现或理由 |
|---|---|---|
| Save Session | 后置 | 会话持久化不在本变更 |
| Save Session As | 后置 | 同上 |
| Session Settings | 后置 | 一期规则在打开时确定，会话内不可改 |
| Locked | 后置 | 与会话持久化一起 |
| Clear Session | 有意未采纳 | 与文本比较一致：以 Home 返回，不另建空会话命令 |
| Swap Sides | 后置 | 一期不提供 |
| Reload Files | 后置 | 一期不提供 |
| Recompare Files | 后置 | 输入恒只读且规则固定，一期无触发场景 |
| Text Merge Info | 已实现 | 状态栏：未解决冲突数 |
| Compare to Output > Left Side / Center Side / Right Side | 后置 | 一期不提供 |
| Merge Parent Folders | 后置 | 文件夹合并不在本变更 |

## commandstextmerge.html — File

| 命令 | 状态 | 对应实现或理由 |
|---|---|---|
| Open File | 后置 | 一期入口为命令行与单实例转交 |
| Open File on File System * | 后置 | 同上 |
| Open File Using Profile * | 后置 | VFS profile 不在范围 |
| Open Clipboard | 后置 | |
| Open With | 后置 | |
| Open With > Associated Application | 后置 | |
| Open With > Text Edit | 后置 | |
| Open With > Text Patch | 后置 | |
| Save File | 已实现 | 保存输出窗格到输出目标 |
| Save File As | 已实现 | 无输出目标时保存即另存 |
| Save File on File System As * | 有意未采纳 | 与 Save File As 合并 |
| Save File Using Profile As * | 后置 | VFS profile 不在范围 |
| Explorer | 后置 | |

## commandstextmerge.html — Edit

| 命令 | 状态 | 对应实现或理由 |
|---|---|---|
| Undo | 已实现 | 输出缓冲的撤销栈，含 Take 与手工编辑 |
| Redo | 已实现 | 同上 |
| Align With | 后置 | 手工对齐不在一期 |
| Isolate | 后置 | 同上 |
| Conflict | 已实现（仅清除方向） | 只提供清除方向："接受当前内容"；手工设置冲突后置 |
| Take Left / Take Center / Take Right | 已实现 | 作用于光标所在区段；按选中行 Take 后置 |
| Take Left Then Right / Take Right Then Left | 后置 | 可用手工编辑达成 |
| Take Left Line / Take Center Line / Take Right Line | 后置 | |
| Increase Indent | 后置 | 合并视图尚未接入此命令 |
| Decrease Indent | 后置 | 同上 |
| Cut / Copy / Paste / Delete | 已实现 | 输出窗格复用文本编辑器；输入窗格的选择与复制后置 |
| Delete Line * | 后置 | |
| Delete to Start of Line * | 后置 | |
| Delete to End of Line * | 后置 | |
| Delete Word * | 后置 | |
| Delete to Start of Word * | 后置 | |
| Delete to End of Word * | 后置 | |
| Insert Line Before * | 后置 | |
| Insert Line After * | 后置 | |
| Select All | 已实现 | 复用文本编辑器 |
| Select Section | 后置 | |
| Compare Selection to Clipboard | 后置 | |

## commandstextmerge.html — Search

| 命令 | 状态 | 对应实现或理由 |
|---|---|---|
| Clear Conflict Section, Next | 已实现 | "接受当前内容"后自动定位下一冲突 |
| Next Conflict Section | 已实现（改名 Next Change Section） | 逐个走改动区段（含冲突，不再只跳未解决冲突） |
| Previous Conflict Section | 已实现（改名 Previous Change Section） | 同上 |
| Next Difference | 已实现 | 与 Next Change Section 同一命令（`Ctrl+N`） |
| Previous Difference | 已实现 | 与 Previous Change Section 同一命令（`Ctrl+P`） |
| Next Difference Section | 已实现 | 改动区段即“删改增”连续段，导航按区段跳动 |
| Previous Difference Section | 已实现 | 同上 |
| Next Difference Files | 后置 | 子会话不在范围 |
| Previous Difference Files | 后置 | 同上 |
| Take Left File and Open Next Difference | 后置 | 同上 |
| Take Right File and Open Next Difference | 后置 | 同上 |
| Next Left Taken / Next Right Taken | 后置 | |
| Previous Left Taken / Previous Right Taken | 后置 | |
| Next Edit | 后置 | |
| Previous Edit | 后置 | |
| Find | 后置 | |
| Replace | 后置 | |
| Find Next | 后置 | |
| Find Previous | 后置 | |
| Go To | 后置 | |
| Toggle Bookmark | 后置 | |
| Go To Bookmark | 后置 | |
| Clear Bookmarks | 后置 | |

## commandstextmerge.html — View

| 命令 | 状态 | 对应实现或理由 |
|---|---|---|
| Show All | 已实现 | 一期唯一的显示方式 |
| Show Changes | 后置 | 显示过滤不在一期 |
| Show Conflicts | 后置 | 同上 |
| Show Left Changes | 后置 | 同上 |
| Show Right Changes | 后置 | 同上 |
| Show Mergeable | 后置 | 同上 |
| Show Unchanged | 后置 | 同上 |
| Show None * | 后置 | 同上 |
| Show Context | 后置 | 同上 |
| Ignore Unimportant Differences | 后置 | 一期由 `/automerge /iu` 或 `/reviewconflicts /iu` 带入，会话内不可切换 |
| Ignore Same Changes | 后置 | |
| Favor Left Changes | 后置 | 着色抑制效果不在一期 |
| Favor Right Changes | 后置 | 同上 |
| Ignored | 后置 | |
| Visible Whitespace | 后置 | |
| Line Numbers | 已实现 | 行号栏恒显示 |
| Syntax Highlighting | 后置 | 一期合并窗格不做语法着色 |
| Word Wrap | 后置 | |
| Display Font * | 后置 | |
| Increase Display Font Size * | 后置 | |
| Decrease Display Font Size * | 后置 | |
| Reset Display Font Size * | 后置 | |
| Side-by-Side Layout | 已实现 | 输入窗格左右并排，一期唯一布局 |
| Over-Under Layout | 后置 | |
| Detached Output Pane | 后置 | |
| Webpages | 有意未采纳 | 与文本比较一致，不提供网页面板 |
| Thumbnail | 后置 | |
| Center Pane | 后置 | 一期 center 窗格恒显示 |
| Text Details | 后置 | |
| Hex Details | 后置 | |
| Ruler | 后置 | |
| File Info | 后置 | |
| Toolbar | 已实现 | 合并工具栏恒显示 |

## sessiontextmerge*.html — 会话设置

| 主题 | 状态 | 对应实现或理由 |
|---|---|---|
| Specs：left / right / center、Merge to、Description | 已实现 | 路径来自命令行；输出目标即 Merge to 的 Other；Description 后置 |
| Format | 后置 | 一期按 center 文件名选格式 |
| Importance | 后置 | 一期使用默认文本规则；引擎层 `TextMergeRules.text` 可配置 |
| Alignment：算法、Skew、closeness、Never align differences | 已实现 | 引擎层支持全部设置；界面使用默认值 |
| Alignment："A merge conflict is" | 已实现 | `TextMergeRules::conflict_gap`；界面不可改 |

## viewtextmerge.html / using_text_merge.html / 3-way_merge_concepts.html

| 行为 | 状态 | 对应实现或理由 |
|---|---|---|
| 三个源窗格 + 输出窗格 | 已实现 | `bc-gui/src/merge/` |
| 源窗格不可编辑、输出可编辑 | 已实现 | |
| 输出随比较加载自动构造 | 已实现 | `MergeState` 默认输出 |
| 输出文件既有内容被忽略 | 已实现 | |
| 左改动 / 右改动 / 冲突 / 手工四种着色 | 已实现 | 色值走 `chrome/theme` |
| Take 可在工具栏、右键菜单与输出区段旁的按钮触发 | 已实现（工具栏、菜单与快捷键；右键菜单与区段旁按钮后置） | 当前区段 Take（Left / Center / Right） |
| 合并时显示过滤切换为合并状态过滤 | 后置 | |
| 隐藏 center 窗格、独立输出窗口、Word Wrap | 后置 | |
| 没有 center 的两方合并 | 后置 | 一期要求三个输入 |
