# 文本合并（Text Merge）

> 三方合并：left / right 两个版本相对共同祖先 center 的改动合并到输出文件，可无窗口自动合并，也可在界面里解决冲突。
> 状态：**已实现（引擎、命令行、界面；合并语义未经参考样本确认）**（2026-11-14 更新）。
> 参考依据与样本采集阻塞见 [reference-parity/text-merge.md](../reference-parity/text-merge.md)。

## 功能简介

- 三个文本文件以 center 为轴对齐，逐行判定改动类型：未变 / 左改 / 右改 / 同改 / 异改。
- 能自动合并的自动合并；两侧改动重叠或相距很近的区段标为冲突，交给用户。
- 命令行 `/automerge` 无窗口执行，以退出码报告结果；`/reviewconflicts` 让它在有冲突时转入界面。
- 界面：上排三个只读输入窗格（LEFT / BASE / RIGHT），底部可编辑的输出窗格（OUTPUT）。
  四个窗格共用一张行网格与同一滚动位置，右侧一条纵向滚动条；打开即定位到第一个冲突。
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
| GUI | `crates/bc-gui/src/merge/model.rs`、`view.rs` | 合并数据模型、输出编辑与撤销、四窗格视图；`app.rs` 的 `View::TextMerge` |
| GUI 启动与交接 | `crates/bc-gui/src/lib.rs`、`remote.rs` | `merge::MergeLaunch`、`LaunchOptions.merge`、`RemoteOpen.merge`、`Outcome::Closed(i32)`；`run_with_remote` 返回启动标签页结果码 |
| 核心测试 | `crates/bc-core/tests/align3.rs` | 三方对齐不变量 |
| 引擎测试 | `crates/bc-engine-text/tests/{align3,merge,golden_merge}.rs` | 三方对齐、合并语义与语料 |
| 命令行测试 | `crates/bc-script/tests/auto_merge.rs` | 自动合并输出与退出码 |
| 交接测试 | `apps/bcompare/tests/hand_off.rs` | 合并转交与等待 |
| GUI 测试 | `crates/bc-gui/src/merge/{model_tests,view_tests}.rs` | 输出编辑、撤销、保存与界面命令 |
| 语料 | `crates/bc-engine-text/tests/golden/merge/` | 20 个三方用例；`tools/golden/gen_merge_cases.py` 重建输入 |

## 关键语义

- **调用固定为文本合并**：`matcha <left> <right> <center> [<output>]`，或
  `matcha /center=<file> [/mergeoutput=<path>] <left> <right>`；`/mergeoutput=` 优先于第 4 个位置参数。
  不按文件格式路由，CSV/TSV 也按行合并；文件夹、缺失、不可读或非文本输入报错。
- **行类型决定内容，冲突加在区段上。** 未变取 center，左改取 left，右改取 right，同改取 left，异改取 center。
  连续的非未变行是一个改动段；改动段里出现异改、或同时有两侧改动时是冲突；相隔不超过
  `conflict_gap`（默认 2）个未变行、分别带有相对侧改动的两段并成一个冲突。
- **不重要的差异默认也算改动**（一侧只改缩进的修改照样进入结果）。`/iu` 才把它们当作相同。
- **favor 只作用于被 `/iu` 忽略的不重要冲突**：取 favored 侧而不是 center。
- 冲突区段内相距很近但不重叠的改动已被采纳，只是需要确认；所以界面里有"接受当前内容"。
- 输出逐行保留来源行的行尾；编码与 BOM 按三方取舍；输出文件的既有内容从不读取；不建备份。
  写出先用同目录临时文件再替换，输出路径可以等于某个输入。
- 输入窗格恒只读；规则在打开时确定。
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
- 有输出目标的会话自打开起就算未保存，关闭标签页或退出时询问 Don't Save / Cancel / Save；
  没有输出目标时保存会询问路径。仍有未解决冲突时，保存先确认 Save Anyway。
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
matcha theirs.txt yours.txt base.txt result.txt                        :: 界面合并
matcha /automerge theirs.txt yours.txt base.txt result.txt             :: 无窗口：0 / 14 / 101 / 100
matcha /automerge /reviewconflicts theirs.txt yours.txt base.txt out   :: 无冲突不开窗，有冲突进界面
matcha /center=base.txt /mergeoutput=result.txt theirs.txt yours.txt  :: 显式祖先与输出目标
cargo test -p bc-core -p bc-engine-text -p bc-script
cargo test -p bcompare --test hand_off
cargo test -p bc-gui --lib merge
```

### P4V 配置

- Application：`matcha.exe`；Arguments：`%1 %2 %b %r`。
  theirs / yours / base / result 分别映射为 left / right / center / output。
- 想“能自动合并就不弹窗”，Arguments 用 `/automerge /reviewconflicts %1 %2 %b %r`。
- 合并调用隐式等待，已有实例也会等待合并标签页关闭并返回结果码；不需要依赖启动器等待标记。
  `matcha.com` 也能等，但从 GUI 程序启动会出现控制台；`matcha-comp.exe` 立即返回，**不能用于合并**。
- **已知限制**：自己成为首个实例或使用 `/solo` 时，进程要到整个窗口关闭才退出；若另外打开了标签页，
  只关闭合并页还不会返回。只开合并一页时，关页即关窗。

### 界面命令与快捷键

菜单为 Session / File / Edit / Search；工具栏为 Home、Save、Left、Center、Right、Accept、Next Conflict、Prev Conflict。

| 命令 | 快捷键 | 行为 |
|---|---|---|
| Save Output | `Ctrl+S` | 保存输出 |
| Save Output As... | `F12` | 另存输出 |
| Undo | `Ctrl+Z` | 撤销 Take、接受或编辑 |
| Redo | `Ctrl+Y` | 重做 |
| Take Left | `Ctrl+L` | 当前区段取左侧 |
| Take Center | `Ctrl+B` | 当前区段取共同祖先 |
| Take Right | `Ctrl+R` | 当前区段取右侧 |
| Accept Current Content | `Ctrl+K` | 不改输出，清除当前区段冲突标记，然后跳到下一个冲突 |
| Next Conflict Section | `Ctrl+N` | 下一个未解决冲突区段 |
| Previous Conflict Section | `Ctrl+P` | 上一个未解决冲突区段 |
| New Tab | `Ctrl+T` | 新建标签页 |
| Close Tab | `Ctrl+W` | 关闭当前标签页 |
| Exit | `Ctrl+Q` | 退出 |

Take 与接受作用于光标所在区段：冲突区段优先，否则为改动段，不是按选中行操作。
验收时可依次 Take、手工增行、接受、撤销/重做，核对四窗格行网格、冲突数、来源色与保存结果；
分别在无冲突保存、带冲突保存、从未保存后关闭，核对退出码 0 / 14 / 101。

## 尚未实现

- 输入窗格编辑，以及输入窗格的选择与复制；会话内改规则。
- 命名会话与 Home 合并入口；显示过滤、语法着色、行内差异高亮。
- 横向滚动条（可用 Shift+滚轮横向滚动）。
- Take Left Then Right、按行 Take、右键菜单、查找替换。

## 深入阅读

- 变更过程与设计决定：`openspec/changes/text-merge/`（design.md 的 D1–D12）
- 参考对照与未确认的语义：[reference-parity/text-merge.md](../reference-parity/text-merge.md)
- `crates/bc-engine-text/README.md`、`crates/bc-script/README.md`
- 命令行总览：[cli-and-scripting.md](cli-and-scripting.md)
