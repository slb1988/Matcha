# 命令行与脚本（CLI / Script / 退出码）

> 三件式命令行、无窗口快速比较 / 自动合并与 `@script` 脚本引擎。状态：**已实现**
> （无窗口子集：文本 patch 报告、`/qc`、`/automerge`、脚本核心命令）。对应 FR-S1、FR-S2。

## 功能简介

三个可执行文件解决 Windows 子系统链接期固定的问题（ADR 301）：

| 文件 | 子系统 | 用途 |
|---|---|---|
| `matcha.exe` | GUI | 双击/资源管理器菜单：开窗，无控制台 |
| `matcha.com` | console | cmd/PowerShell 中 `matcha …`：等待、可重定向、回传退出码 |
| `matcha-comp.exe` | GUI | VCS 比较集成：立即交接返回，不能用于合并 |

合并调用：`matcha <base> <theirs> <yours> [<output>]`（即 P4V `%b %1 %2 %r`；theirs 进 RIGHT，yours 进 LEFT，界面保存写 LEFT），或
`matcha /center=<file> [/mergeoutput=<path>] <left> <right>`；`/mergeoutput=` 优先于第 4 个位置参数。
三个输入都是 xlsx/xlsm 工作簿时走[表格合并](table-merge.md)（键对齐逐格合并），CSV/TSV 默认仍按行文本合并，
加 `/tablemerge` 才按表格合并（`/keys=1,3` 指定键列）；工作簿与非工作簿混合报错 100。
**合并调用总是等待**：`matcha.exe` 与 `matcha.com` 都等待合并标签页关闭并回传结果码，包括转交已有实例；
自己成为首个实例或 `/solo` 时要等整个窗口关闭（仅合并一页时关页即关窗）。无窗口合并则等待执行完成。

无窗口能力（无窗口执行阶段都在启动进程内运行，不转交给已打开的窗口）：

- `/qc a b`：快速比较，退出码 2 same / 12 similar / 13 different；`/qc=binary|crc|size`
  → 1 / 11；`/silent` 抑制单行结果输出。
- `/automerge base theirs yours output`：需要输出目标；无冲突写出并返回 0，有冲突不写出为 101，
  带 `/force` 时带冲突标记写出为 14；参数、输入或写出错误为 100。写同目录临时文件再替换，不建备份，
  不读取既有输出；输出可等于输入，101 时既有输出保持原样。结果为 stdout 一行，`/silent` 或无标准输出时
  不写也不为结果弹框；错误写 stderr（无控制台时为消息框）。加 `/reviewconflicts` 时，无冲突仍不开窗；
  有冲突先不写输出，转入界面并由界面决定退出码。`/reviewconflicts` 优先于 `/force`，但 `/silent` 使它失效。
  表格合并没有冲突标记形式：0 / 101 / 100，`/force` 报错 100；结果行带实际键列（`(keys: 1 (inferred))`）。
- `/report=`：见 [reports.md](reports.md)。
- `@script.txt a b`：脚本模式，`%1`…`%9` 参数替换、`%date%`、`#` 注释、`&` 续行、
  大小写不敏感；支持 `text-report`、`table-report`（`summary` 布局）、`log`、`option`、`beep`，
  其余命令在执行前整体拒绝。

## 代码地图

| 层 | 位置 | 内容 |
|---|---|---|
| 引擎 | `crates/bc-script` | `ExitCode`、`Command`（29 变体）、脚本解析、CLI 开关表、runner |
| 应用 | `apps/bcompare/src` | `main.rs` / `console.rs` / `routing.rs`（GUI vs 控制台路由） |
| 应用 | `apps/bcomp/src` | `matcha-comp` / `matcha-compc` 启动器 |
| 打包 | `tools/xtask` | `cargo xtask dist` 按平台布局产出三件套 |

## 关键语义

- **退出码表是兼容契约**：0/1/2/11/12/13/14/100/101…；`104` 仅为枚举保留，永不产生
  （红线：无任何授权/试用逻辑）。
- `/report`、`output-to:-`、`/reportfile` 是 Matcha 自有扩展，参考工具没有。
- 脚本解析期即做参数替换；逻辑行保留首行行号。
- 脚本引擎与 GUI 共享内部命令 API，保证交互与脚本行为一致。

## 使用与验证

```bat
matcha /qc a.txt b.txt
matcha /report=patch-unified a.txt b.txt
matcha /report=summary a.xlsx b.xlsx
matcha @diff.txt a.txt b.txt
matcha base.txt theirs.txt yours.txt result.txt
matcha /automerge base.txt theirs.txt yours.txt result.txt
matcha /automerge /reviewconflicts base.txt theirs.txt yours.txt result.txt
matcha /automerge /keys=1 base.xlsx theirs.xlsx yours.xlsx result.xlsx
matcha /tablemerge /automerge base.csv theirs.csv yours.csv result.csv
cargo test -p bc-script
```

速查手册：`.agents/skills/matcha-cli/SKILL.md`。

## 深入阅读

- 文本合并、退出码与 P4V 配置：[text-merge.md](text-merge.md)
- 表格合并（xlsx/CSV）：[table-merge.md](table-merge.md)
- `crates/bc-script/README.md`、`apps/bcomp/README.md`
- ADR：`docs/decisions/301-windows-subsystem-split.md`
- 变更过程：`openspec/changes/text-patch-reports/`
