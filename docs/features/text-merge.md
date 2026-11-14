# 文本合并（Text Merge）

> 三方合并：left / right 两个版本相对共同祖先 center 的改动合并到输出文件，可无窗口自动合并，也可在界面里解决冲突。
> 状态：**进行中**（change `text-merge`：引擎、命令行、界面按任务组依次落地；合并语义**未经参考样本确认**，
> 见 [reference-parity/text-merge.md](../reference-parity/text-merge.md)）。（2026-11-14 更新）

## 功能简介

- 三个文本文件以 center 为轴对齐，逐行判定改动类型：未变 / 左改 / 右改 / 同改 / 异改。
- 能自动合并的自动合并；两侧改动重叠或相距很近的区段标为冲突，交给用户。
- 命令行 `/automerge` 无窗口执行，以退出码报告结果；`/reviewconflicts` 让它在有冲突时转入界面。
- 界面：上排三个只读输入窗格（左 / base / 右），底部可编辑的输出窗格。
- 面向版本控制工具（Perforce P4V）的合并器集成。

## 代码地图

| 层 | 位置 | 内容 |
|---|---|---|
| 核心 | `crates/bc-core/src/align.rs` | 三方调和、`Aligner::align3` |
| 引擎 | `crates/bc-engine-text` | `jobs::Align3Job`、`merge.rs`（`MergeState`、`auto_merge`）、`rules::TextMergeRules` |
| 命令行 | `crates/bc-script` | `cli::CliArgs::merge_spec`、`runner::auto_merge_files` |
| 应用 | `apps/bcompare/src` | `routing.rs`（`Route::AutoMerge`）、`main.rs`（无窗口执行、等待与退出码） |
| GUI | `crates/bc-gui/src/merge/` | 合并视图 |
| 语料 | `crates/bc-engine-text/tests/golden/merge/` | 20 个三方用例；`tools/golden/gen_merge_cases.py` 重建输入 |

## 关键语义

- **行类型决定内容，冲突加在区段上。** 未变取 center，左改取 left，右改取 right，同改取 left，异改取 center。
  连续的非未变行是一个改动段；改动段里出现异改、或同时有两侧改动时是冲突；相隔不超过
  `conflict_gap`（默认 2）个未变行、分别带有相对侧改动的两段并成一个冲突。
- **不重要的差异默认也算改动**（一侧只改缩进的修改照样进入结果）。`/iu` 才把它们当作相同。
- **favor 只作用于被 `/iu` 忽略的不重要冲突**：取 favored 侧而不是 center。
- 冲突区段内相距很近但不重叠的改动已被采纳，只是需要确认；所以界面里有"接受当前内容"。
- 输出逐行保留来源行的行尾；编码与 BOM 按三方取舍；输出文件的既有内容从不读取；不建备份。
- 输入窗格恒只读；规则在打开时确定。

## 使用与验证

```bat
matcha theirs.txt yours.txt base.txt result.txt                        :: 界面合并
matcha /automerge theirs.txt yours.txt base.txt result.txt             :: 无窗口：0 / 14 / 101 / 100
matcha /automerge /reviewconflicts theirs.txt yours.txt base.txt out   :: 无冲突不开窗，有冲突进界面
cargo test -p bc-core -p bc-engine-text -p bc-script
```

## 深入阅读

- 变更过程与设计决定：`openspec/changes/text-merge/`（design.md 的 D1–D12）
- 参考对照与未确认的语义：[reference-parity/text-merge.md](../reference-parity/text-merge.md)
- `crates/bc-engine-text/README.md`、`crates/bc-script/README.md`
- 命令行总览：[cli-and-scripting.md](cli-and-scripting.md)
