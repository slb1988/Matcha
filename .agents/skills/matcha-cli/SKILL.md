---
name: matcha-cli
version: 1.2.0
description: 本项目 Tools/matcha/ 内置的 Matcha 文件对比/合并工具速查，项目内做文件 diff 优先用它：三件套分工、/report 无窗口 diff（文本 patch、xlsx/CSV 单元格级表格报告）、/qc 退出码（xlsx 按单元格判定）、/automerge 三方合并与 P4V 合并工具配置、@script 脚本模式、资源管理器右键比较。对比任意文件、审查 RawData Excel 的 P4 变动、判断两文件是否相同、合并 xlsx/文本冲突、CI 集成、问 matcha 用法时使用。
---

# Matcha 命令行速查

项目内对比文件优先用随 P4 同步的 `Tools/matcha/`。cmd/PowerShell 将该目录加入 PATH 后，下面的 `matcha` 会命中 `matcha.com`；Git Bash 在仓库根目录写全 `Tools/matcha/matcha.com`，开关用 `-` 前缀。
启动器说明见 `Tools/matcha/README.txt`，当前版本的完整开关表以 `matcha /?`（cmd）为准。

## 选择启动器

| 入口 | 何时使用 |
|---|---|
| `matcha.com` | 命令行、CI、脚本：等待比较结束，输出报告并返回退出码。 |
| `matcha.exe` | GUI 主程序；P4V 的合并工具用它，等待合并结束。 |
| `matcha-comp.exe` | 右键菜单、VCS difftool：把比较交给 GUI 后立即返回。 |

## 比较：按所需结果选命令

| 目标 | 命令（cmd） | 看什么 |
|---|---|---|
| 交互比较 | `matcha left.txt right.txt` | 打开时定位首个差异，关闭标签页后返回。 |
| 文本 patch | `matcha /report=patch-unified a.txt b.txt` | 统一差异写 stdout；加 `/reportfile=out.patch` 存盘。 |
| xlsx/xlsm 或 CSV/TSV 逐格比较 | `matcha /report=summary /reportoptions=title-row:1 old.xlsx new.xlsx` | `result:` 给结论，再按 sheet、行、列看旧值 → 新值；表头在第 1 行时用 `title-row:1`。 |
| 只判断是否相同 | `matcha /qc old.xlsx new.xlsx` | 按规则比较，xlsx 按单元格判定：2 为相同，13 为不同。按字节比较用 `/qc=binary`。 |

**`/report` 返回 0 只表示报告生成成功。** CI 需要按差异分支时，读取 `/qc` 的[退出码](references/cli-reference.md#退出码)。
CSV/TSV 用 `summary` 看单元格，`patch*` 看文本；工作簿的 `patch*` 自动转为表格报告。见[报告示例、键列与 unified diff 解读](references/compare-reports.md)。

审查 `Main/RawData`：用 `p4 print -q -o` 导出前后两个 revision，再用 `/qc` 判定、`/report=summary` 定位改动。
见[可复制的 P4 配方](references/compare-reports.md#rawdata-的-p4-历史对比)和[历史实测报告](references/test-rawdata-xlsx-20261112.md)。

## 三方合并

位置参数顺序为 `base theirs yours [output]`。无窗口自动合并需给出输出文件：

```bat
matcha /automerge base.xlsx theirs.xlsx yours.xlsx result.xlsx
```

三个工作簿按键逐格合并，CSV/TSV 加 `/tablemerge` 才按表格合并。`/automerge` 无冲突时写出结果（0），有冲突时保留既有输出（101）；要交由人工处理可加 `/reviewconflicts`。
P4V 使用 `matcha.exe`，参数 `%b %1 %2 %r`。保存会写入本地文件（yours）并同步 `%r`。详见[合并规则、键列与 P4V 配置](references/merge.md)。

## 脚本与桌面集成

`matcha @script.txt`（cmd）按顺序无界面执行脚本命令。位置参数、通用开关、脚本报告语法和资源管理器右键操作见 [CLI 速查](references/cli-reference.md)。

## 按症状排查

先核对启动器和当前 shell 的开关前缀，再按症状查对应章节：

| 症状 | 章节 |
|---|---|
| Git Bash 把 `/report` 当作输入文件 | [命令被当作路径](references/pitfalls.md#命令被当作路径) |
| 有差异却返回 0、stdout 为空、布局不支持或 hunk 偏长 | [报告输出](references/pitfalls.md#报告输出) |
| xlsx 单行过长、`same` 数与物理行数不同 | [表格行与计数](references/pitfalls.md#表格行与计数) |
| P4V 提前返回、表格合并加 `/force` 报错 | [合并异常](references/pitfalls.md#合并异常) |
