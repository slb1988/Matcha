# 文本合并（Text Merge）

> 三方合并：left / right 两个版本相对共同祖先 center 的改动合并到输出文件，可无窗口自动合并，也可在界面里解决冲突。
> 状态：**已实现（引擎、命令行、界面，含行内差异、语法着色、查找替换、横向滚动条、输入窗格选择复制；合并语义未经参考样本确认）**（2026-11-19 更新）。
> 参考依据与样本采集阻塞见 [reference-parity/text-merge.md](../reference-parity/text-merge.md)。

## 功能简介

- 三个文本文件以 center 为轴对齐，逐行判定改动类型：未变 / 左改 / 右改 / 同改 / 异改。
- 能自动合并的自动合并；两侧改动重叠或相距很近的区段标为冲突，交给用户。
- 命令行 `/automerge` 无窗口执行，以退出码报告结果；`/reviewconflicts` 让它在有冲突时转入界面。
- 界面：上排三个只读输入窗格，从左到右为 BASE / LEFT / RIGHT，底部可编辑的输出窗格（OUTPUT，标题条不显示路径）。
  四个窗格共用一张行网格与同一滚动位置，右侧一条纵向滚动条；打开即定位到第一个冲突
  （没有冲突则第一个改动），`Ctrl+N` / `Ctrl+P` 逐个走改动区段（含冲突）。到头时光标不动，状态栏上方弹出
  **Difference not found.** 面板并响一声（取代原来状态栏的 "No more changes below/above"），下一次按键或鼠标按下关闭；
  遵循 Tools › Options › Next Difference（Wrap around 回到第一个/最后一个改动区段，没有改动仍弹面板；Show message panel 关闭时静默），
  规则见 [text-compare.md](text-compare.md)「找不到差异时的消息面板」。
- 各窗格标题条的路径是只读地址框（`chrome::pathbar::show_readonly`）：可选中、Ctrl+C 复制，
  右键菜单 Copy / Select All，选区内右击保留选区。
- **行内差异高亮**：左、右输入窗格的改动行与 base 同一对齐行逐字符比较，不同字符用所属侧强调色
  （左侧 `MERGE_LEFT` / 右侧 `MERGE_RIGHT`，未解决冲突区段 `MERGE_CONFLICT`）着墨；只在不重要元素
  （默认注释）上的差异用不重要差异色。
- **语法着色**：四个窗格按文件格式库的语法着色（格式按 base 的文件名解析），风格与文本比较一致；
  View › Syntax Highlighting 开关（默认开），关闭时只用普通文字色，改动 / 冲突底色不变。
- **查找替换**：复用文本比较的查找条（字面 / 正则、大小写、全词、仅在改动中），在活动窗格查找；
  替换只对输出窗格可用。
- **横向滚动条**：任一窗格可见内容超宽时，窗格区底部出现一条四窗格共用的横向滚动条。
- **输入窗格选择与复制**：单击定位、拖选、Shift+单击、`Ctrl+A`、`Ctrl+C`；右键菜单 Take Left / Center / Right、
  Copy、Select All。
- **编号书签** 0–9：挂在对齐行上并记住窗格，Take / 输出编辑不移动书签，见 [text-bookmarks.md](text-bookmarks.md)。
- **Word Wrap**（View 菜单）：四个窗格各按自身宽度折行，对齐行高取四者最大值，见 [text-word-wrap.md](text-word-wrap.md)。
- 左改动 / 右改动 / 冲突 / 手工编辑四种着色（`chrome/theme.rs` 的 `MERGE_*`）；未解决冲突行的
  行号栏左缘有红色竖条。状态栏显示未解决冲突区段数与光标所在行的类型。
- 面向版本控制工具（Perforce P4V）的合并器集成。

## 代码地图

| 层 | 位置 | 内容 |
|---|---|---|
| 核心 | `crates/bc-core/src/align.rs` | `reconcile3` 三方调和、`Aligner::align3` |
| 引擎 | `crates/bc-engine-text` | `TextMap::merge`、`jobs::Align3Job`、`merge::{MergeState, auto_merge}`、`rules::TextMergeRules` |
| 命令行 | `crates/bc-script` | `cli::{MergeSpec, MergeSwitches}`、`CliArgs::{merge_spec, merge_switches, title}`；`runner::{AutoMergeRequest, AutoMergeReport, auto_merge_files, replace_file, auto_merge_exit_code}` |
| 应用 | `apps/bcompare/src/routing.rs`、`main.rs` | `Route::AutoMerge`、`merge_launch`、`review_args`；无窗口执行、等待与退出码 |
| GUI | `crates/bc-gui/src/merge/model.rs`、`view.rs` | 合并数据模型、输出编辑与撤销、`MergeModel::replace_all`；四窗格视图（`line_job` 语法与行内着墨、`ensure_inline` 行内缓存、`input_editors` 输入窗格选区、`find_action` 查找替换、共用横向滚动条）；`app.rs` 的 `View::TextMerge` |
| GUI 复用 | `crates/bc-gui/src/text/{render.rs,find_panel.rs,editor.rs}` | `render::layout`（`LineInk.important` 覆盖重要差异色）、查找条、光标与选区 |
| GUI 启动与交接 | `crates/bc-gui/src/lib.rs`、`remote.rs` | `merge::MergeLaunch`、`LaunchOptions.merge`、`RemoteOpen.merge`、`Outcome::Closed(i32)`；`run_with_remote` 返回启动标签页结果码 |
| 核心测试 | `crates/bc-core/tests/align3.rs` | 三方对齐不变量 |
| 引擎测试 | `crates/bc-engine-text/tests/{align3,merge,golden_merge}.rs` | 三方对齐、合并语义与语料 |
| 命令行测试 | `crates/bc-script/tests/auto_merge.rs` | 自动合并输出与退出码 |
| 交接测试 | `apps/bcompare/tests/hand_off.rs` | 合并转交与等待 |
| GUI 测试 | `crates/bc-gui/src/merge/{model_tests,view_tests}.rs` | 输出编辑、撤销、保存与界面命令 |
| 语料 | `crates/bc-engine-text/tests/golden/merge/` | 20 个三方用例；`tools/golden/gen_merge_cases.py` 重建输入 |

## 关键语义

- **调用**：`matcha <base> <theirs> <yours> [<output>]`（对应 P4V `%b %1 %2 %r`：theirs 进 RIGHT，yours 即本地文件进 LEFT），或
  `matcha /center=<file> [/mergeoutput=<path>] <left> <right>`；`/mergeoutput=` 优先于第 4 个位置参数。
  三个输入都是 xlsx/xlsm 工作簿时改走[表格合并](table-merge.md)，工作簿与非工作簿混合报错（100）；
  其余（含 CSV/TSV，除非带 `/tablemerge`）按行文本合并；文件夹、缺失、不可读或非文本输入报错。
- **行类型决定内容，冲突加在区段上。** 未变取 center，左改取 left，右改取 right，同改取 left，异改取 center。
  连续的非未变行是一个改动段；改动段里出现异改、或同时有两侧改动时是冲突；相隔不超过
  `conflict_gap`（默认 2）个未变行、分别带有相对侧改动的两段并成一个冲突。
- **不重要的差异默认也算改动**（一侧只改缩进的修改照样进入结果）。`/iu` 才把它们当作相同。
- **favor 只作用于被 `/iu` 忽略的不重要冲突**：取 favored 侧而不是 center。
- 冲突区段内相距很近但不重叠的改动已被采纳，只是需要确认；所以界面里有"接受当前内容"。
- 输出逐行保留来源行的行尾；编码与 BOM 按三方取舍；输出文件的既有内容从不读取；不建备份。
  写出先用同目录临时文件再替换，输出路径可以等于某个输入。
- 输入窗格恒只读；规则在打开时确定。输入窗格可放光标、选择、复制（`Ctrl+C` 保留原行尾），
  键入、粘贴、删除、回车与输入法提交一律丢弃，内容不变。
- **行内差异只在 left / right 输入窗格**：该对齐行属于改动或冲突、且本侧与 base 都有内容时才逐字符比较；
  整行新增 / 删除仍只按整行着色。base 窗格与输出窗格**不做**行内高亮（输出按来源底色区分）。
  比较规则与载入时一致（含 `/iu`）。行内着墨优先于语法颜色。
- **查找**作用于活动窗格内容，从光标起查，**不回绕**：到首尾时状态栏提示 `No more matches`，选区不变；
  命中后选中匹配，其对齐行不在视野内时把四个窗格滚到该行。「仅在改动中」（Differences Only）只查改动 / 冲突区段的行。
- **替换只在输出窗格**：活动窗格为输入窗格时 Replace… 禁用、查找条不显示替换行。Replace 替换当前匹配后查下一个；
  Replace All 替换输出窗格全部匹配，作为一个撤销步；被替换的行标为手工，波及的未解决冲突区段视为已解决。
- 横向滚动条在所有可见内容放得下时隐藏，横向偏移归零；拖动、点击轨道、在其上滚轮都同步横移四个窗格。
- **`/automerge` 在发起进程内无窗口执行，从不转交给已运行的实例，且必须有输出目标。**
  成功或有冲突时向 stdout 写一行结果（如 `Success` 或
  `Conflicts detected, merge output not written: 1 conflict section(s)`）；`/silent` 或没有标准输出时
  不写结果，也不为结果弹框。错误写 stderr，没有控制台时用消息框。
- **`/reviewconflicts` 配合 `/automerge`**：无冲突时直接写出并返回 0；有冲突时先不写输出，转入合并视图，
  退出码由界面决定。它优先于 `/force`；带 `/silent` 时不生效。
- `/iu`、`/favorleft`、`/favorright` 在带 `/automerge` 或 `/reviewconflicts` 时生效；后者也能把规则带入界面会话。
  `/force` 只在 `/automerge` 时有意义；`/favorleft` 与 `/favorright` 同时给出是命令行错误。
- `/title1..4` 分别是 left / right / center / 输出标题；前两个兼作冲突标记标签，未给时用文件名。
  `/ro`、`/ro1`、`/ro2`、`/nobackups`、`/savetarget=`、`/vcs1..4` 在合并调用里接受但无效果；
  `/solo` 在独立窗口打开合并。
- **仅 `/automerge /force` 写冲突标记**：`<<<<<<< <left 标签>`、left 各行、`=======`、right 各行、
  `>>>>>>> <right 标签>`。界面保存始终按输出窗格当前内容写出，不加标记。
- 输出编辑复用文本比较编辑器（输入法、选区、剪切/复制/粘贴、`Ctrl+A`）。编辑或新增的行标为手工；
  被手工编辑的冲突区段视为已解决，再次 Take 会覆盖手工内容。新增行在输出中多占显示行，输入对应位置为空隙。
  Take、接受当前内容与编辑共用一个撤销栈。
- 输出窗格文本区悬停/拖选时显示 I 形插入光标；选区底色与文本比较相同（不透明选区蓝 `Palette::text_selection`）。
- **保存默认写到 LEFT 的文件**（本地真实路径）；给了输出文件（第 4 个参数 / `/mergeoutput=`，即 P4V `%r`）时，
  每次保存后再把同一内容复制过去。Save Output As 可另选路径。会话自打开起就算未保存，关闭标签页或退出时询问
  Don't Save / Cancel / Save。仍有未解决冲突时，保存先确认 Save Anyway。
- **合并调用总是等待**：由 `matcha.exe` 或 `matcha.com` 启动、转交已有实例、自己成为首个实例或 `/solo`
  都不会提前返回。转交请求的 wait = 启动器等待标记或这是合并调用，等待合并标签页关闭；首个实例与 `/solo`
  要等整个窗口关闭（见下方 P4V 已知限制）。无窗口 `/automerge` 等本次执行完成。
- 冲突间距、标记格式、`/iu` 与 favor、输出编码与行尾目前只有参考帮助依据，**未经参考样本确认**；
  本机参考程序不提供三方合并，样本采集被阻塞，不能把本地回归当作参考对拍。

### `/automerge` 退出码（未转入界面时）

| 码 | 结果 |
|---|---|
| 0 | 无冲突，已写出 |
| 14 | 有冲突且带 `/force`，已带冲突标记写出 |
| 101 | 有冲突，未写出；已存在的输出文件保持原样 |
| 100 | 参数、输入或写出错误（包括缺输出目标） |

### 界面合并退出码

| 码 | 标签页结果 |
|---|---|
| 0 | 最后一次保存时没有未解决冲突 |
| 14 | 最后一次保存时仍有未解决冲突 |
| 101 | 从未保存 |
| 100 | 合并视图打不开（输入缺失、不是文本等） |

## 使用与验证

```bat
matcha base.txt theirs.txt yours.txt result.txt                        :: 界面合并（保存写 yours.txt 并同步 result.txt）
matcha /automerge base.txt theirs.txt yours.txt result.txt             :: 无窗口：0 / 14 / 101 / 100
matcha /automerge /reviewconflicts base.txt theirs.txt yours.txt out   :: 无冲突不开窗，有冲突进界面
matcha /center=base.txt /mergeoutput=result.txt theirs.txt yours.txt  :: 显式祖先与输出目标
cargo test -p bc-core -p bc-engine-text -p bc-script
cargo test -p bcompare --test hand_off
cargo test -p bc-gui --lib merge
```

### P4V 配置

- Application：`matcha.exe`；Arguments：`%b %1 %2 %r`。
  base / theirs / yours / result 分别映射为 BASE / RIGHT / LEFT / output；界面保存写 LEFT（本地文件）并同步 `%r`。
- 想“能自动合并就不弹窗”，Arguments 用 `/automerge /reviewconflicts %b %1 %2 %r`。
- 合并调用隐式等待，已有实例也会等待合并标签页关闭并返回结果码；不需要依赖启动器等待标记。
  `matcha.com` 也能等，但从 GUI 程序启动会出现控制台；`matcha-comp.exe` 立即返回，**不能用于合并**。
- **已知限制**：自己成为首个实例或使用 `/solo` 时，进程要到整个窗口关闭才退出；若另外打开了标签页，
  只关闭合并页还不会返回。只开合并一页时，关页即关窗。

### 界面命令与快捷键

菜单为 Session / File / Edit / View / Search；工具栏为 Home、Save、Left、Center、Right、Accept、Next Change、Prev Change。

| 命令 | 快捷键 | 行为 |
|---|---|---|
| Save Output | `Ctrl+S` | 保存输出 |
| Save Output As... | `F12` | 另存输出 |
| Undo | `Ctrl+Z` | 撤销 Take、接受或编辑 |
| Redo | `Ctrl+Y` | 重做 |
| Take Left | `Ctrl+L` | 选中行或当前区段取左侧 |
| Take Center | `Ctrl+B` | 选中行或当前区段取共同祖先 |
| Take Right | `Ctrl+R` | 选中行或当前区段取右侧 |
| Take Left Then Right | — | 选中行或当前区段：先左侧各行，再右侧各行 |
| Take Right Then Left | — | 选中行或当前区段：先右侧各行，再左侧各行 |
| Take Left Line | `Shift+Ctrl+L` | 只换光标所在的一行为左侧 |
| Take Center Line | `Shift+Ctrl+B` | 只换光标所在的一行为共同祖先 |
| Take Right Line | `Shift+Ctrl+R` | 只换光标所在的一行为右侧 |
| Accept Current Content | `Ctrl+K` | 不改输出，清除当前区段冲突标记，然后跳到下一个冲突 |
| Syntax Highlighting | — | View 菜单勾选项，开关四窗格语法着色（默认开） |
| Find... | `Ctrl+F` | 打开查找条，在活动窗格查找 |
| Find Next | `F3` | 下一个匹配（不回绕） |
| Find Previous | `Shift+F3` | 上一个匹配（不回绕） |
| Replace... | `Ctrl+H` | 打开带替换行的查找条（仅输出窗格活动时可用） |
| Next Change Section | `Ctrl+N` | 下一个改动区段（含冲突） |
| Previous Change Section | `Ctrl+P` | 上一个改动区段（含冲突） |
| New Tab | `Ctrl+T` | 新建标签页 |
| Close Tab | `Ctrl+W` | 关闭当前标签页 |
| Exit | `Ctrl+Q` | 退出 |

Take 系列作用于输出窗格的选中行（选区止于某行行首时不含该行）；没有选区时作用于光标所在区段
（冲突区段优先，否则为改动段），与参考 "Takes selected lines or current section" 一致。接受只看光标所在区段。
Take Left Then Right / Right Then Left 的后一侧各行排在前一侧之后，显示在区段末行之下（输入窗格对应位置为空隙）。
按行 Take 只换一行；参考按行记冲突、本项目按区段记，所以多行冲突区段按行 Take 后仍为未解决（继续 Take 或接受），
单行冲突区段随之解决。

输出窗格右键菜单：Take Left / Center / Right、Take Left Then Right / Right Then Left、Take Left / Center / Right Line、
Accept Current Content、Cut / Copy / Paste、Select All（区段 Take 与 Accept 在光标不在改动上且无选区时禁用；
Cut / Copy 需要选区）。条目取自参考 Edit 菜单（无参考截图，样式走设计系统）。
输入窗格右键菜单：Take Left / Center / Right（作用于所点行所在区段，不在改动上时禁用）、Copy（无选区时禁用）、
Select All；右键先把光标行与活动窗格移到所点处。
验收时可依次 Take、手工增行、接受、撤销/重做，核对四窗格行网格、冲突数、来源色与保存结果；
分别在无冲突保存、带冲突保存、从未保存后关闭，核对退出码 0 / 14 / 101。

## 尚未实现

- 输入窗格编辑；从输入窗格选区 Take 选中行；会话内改规则。
- 合并的命名会话与 Home 合并入口（比较视图的命名会话见 [saved-sessions.md](saved-sessions.md)）；显示过滤（Show Conflicts 等）；自动折行。
- base 与输出窗格的行内差异高亮（输出按来源底色区分；base 与哪一侧比对有歧义）。
- 超长行只排版、比较每行前 4096 字节（不做横向窗口化）。

## 深入阅读

- 变更过程与设计决定：`openspec/changes/text-merge/`（design.md 的 D1–D12）；审阅增强见 `openspec/changes/archive/2026-11-19-text-merge-essentials/`；主规格 `openspec/specs/text-merge-view/spec.md`
- 参考对照与未确认的语义：[reference-parity/text-merge.md](../reference-parity/text-merge.md)
- `crates/bc-engine-text/README.md`、`crates/bc-script/README.md`
- 命令行总览：[cli-and-scripting.md](cli-and-scripting.md)
- 编号书签：[text-bookmarks.md](text-bookmarks.md)
- 自动折行：[text-word-wrap.md](text-word-wrap.md)
- xlsx/CSV 的键对齐逐格合并：[table-merge.md](table-merge.md)
