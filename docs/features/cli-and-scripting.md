# 命令行与脚本（CLI / Script / 退出码）

> 三件式命令行、无窗口快速比较与 `@script` 脚本引擎。状态：**已实现**
> （无窗口子集：文本 patch 报告、`/qc`、脚本核心命令）。对应 FR-S1、FR-S2。

## 功能简介

三个可执行文件解决 Windows 子系统链接期固定的问题（ADR 301）：

| 文件 | 子系统 | 用途 |
|---|---|---|
| `matcha.exe` | GUI | 双击/资源管理器菜单：开窗，无控制台 |
| `matcha.com` | console | cmd/PowerShell 中 `matcha …`：等待、可重定向、回传退出码 |
| `matcha-comp.exe` | GUI | VCS 集成：立即交接返回 |

无窗口能力（都在启动进程内执行，绝不进入已打开的窗口）：

- `/qc a b`：快速比较，退出码 2 same / 12 similar / 13 different；`/qc=binary|crc|size`
  → 1 / 11；`/silent` 抑制单行结果输出。
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
cargo test -p bc-script
```

速查手册：`.agents/skills/matcha-cli/SKILL.md`。

## 深入阅读

- `crates/bc-script/README.md`、`apps/bcomp/README.md`
- ADR：`docs/decisions/301-windows-subsystem-split.md`
- 变更过程：`openspec/changes/text-patch-reports/`
