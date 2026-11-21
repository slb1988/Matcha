# Matcha

Matcha 是用 Rust 编写的桌面文件比较与合并工具，优先支持 Windows。你可以并排查看两版代码改了哪些字符、两份 Excel 工作簿改了哪些单元格，或在同步两个文件夹之前核对要复制和删除的文件。只需要差异报告时，不用打开窗口。

## 第一次比较（Windows x64）

1. 打开 [最新正式版](https://github.com/slb1988/Matcha/releases/latest)，在 **Assets** 中下载 `matcha-<版本>-windows-x86_64.zip`，而不是 *Source code*。解压整个 ZIP，双击 `matcha.exe`；使用发行包不需要安装 Rust。
2. 准备两个文本文件。比如在 `old.txt` 写一行 `port=8080`，在 `new.txt` 写一行 `port=8081`。在资源管理器里选中两个文件，一起拖到 Matcha 的 **Home** 页。如果文件在不同目录，先点 **Text Compare**，再分别拖到左右窗格。
3. 比较完成后，视图会定位到第一个差异，标出行内不同的字符。这个例子里只改了端口号的最后一位。查看长文件时，按 `Ctrl+N` / `Ctrl+P` 跳到下一个 / 上一个差异区段。只查看不会改动文件。

## 按任务选择视图

| 任务 | 在 Matcha 里怎么做 |
| --- | --- |
| 核对或修改文本 | [文本比较](docs/features/text-compare.md)可以就地编辑，或用行号旁的箭头把一段改动复制到另一侧。用[数字书签](docs/features/text-bookmarks.md)可在几处修改之间往返。 |
| 找出表格里改了哪个值 | 将两份 CSV、TSV 或 Excel 工作簿（`.xlsx` / `.xlsm`）拖到 Home。[表格比较](docs/features/table-compare.md)按键列对齐行，再逐格显示差异；多工作表可切换查看。旧版 `.xls` 不支持原生表格比较。 |
| 看两个目录相差哪些文件 | 将两个目录一起拖到 Home，打开[文件夹比较](docs/features/folder-compare-sync.md)。要同步时用 **Folder Sync**，先预览每项动作，再执行。 |
| 查看二进制差异 | 用[十六进制比较](docs/features/hex-compare.md)并排看字节、按偏移定位，区分内容变化与单侧插入。 |
| 合并两人基于同一版本的修改 | [文本三方合并](docs/features/text-merge.md)能逐段解决冲突；[表格三方合并](docs/features/table-merge.md)能把双方对同一行不同单元格的修改合在一起。工作簿自动按表格合并；CSV/TSV 默认按文本行合并，加 `/tablemerge` 才按单元格合并。 |

经常核对同一对路径？可以[保存为命名会话](docs/features/saved-sessions.md)，下次从 Home 的 **Saved** 列表直接重开。

## 在命令行里比较

解压出的三个程序应放在同一目录，按用途选择：

- `matcha.exe`：双击打开桌面界面，也是 P4V 应配置的合并工具。
- `matcha.com`：在终端或脚本中调用，能输出报告、等待完成并返回退出码。
- `matcha-comp.exe`：供版本控制工具发起比较，交给界面后立即返回，不适合作为合并工具。

在含 `matcha.com` 的目录打开 **PowerShell**。以下命令分别运行，把示例文件名换成实际路径（含空格时加双引号）：

```powershell
# 快速判断两份文本是否相同
.\matcha.com /qc old.txt new.txt
$LASTEXITCODE   # 上面的端口号示例返回 13

# 看具体改了哪几行（unified diff）
.\matcha.com /report=patch-unified old.txt new.txt

# 列出工作簿内有差异的工作表、行和单元格
.\matcha.com /report=summary old.xlsx new.xlsx

# 无窗口三方合并：共同祖先、对方版本、本地版本、输出文件
.\matcha.com /automerge base.txt theirs.txt yours.txt merged.txt
```

- **判断是否相同**用 `/qc`：对文本或表格按规则比较时，`2` 表示相同，`12` 表示只有不重要差异，`13` 表示有重要差异。`/report` 返回 `0` 只表示报告生成成功，不表示文件相同。
- **自动合并**无冲突时写入输出文件并返回 `0`；上例遇到冲突时返回 `101`，不改动已有输出。
- 查看开关用 `.\matcha.com /?`。更多示例见[命令行与脚本](docs/features/cli-and-scripting.md)、[报告格式](docs/features/reports.md)和 [P4V 合并配置](docs/features/text-merge.md#p4v-配置)。

## 从源码运行

在 Windows 上准备好 Git、[Rust 稳定版](https://rustup.rs/)（MSVC 工具链），以及 Visual Studio Build Tools 的「使用 C++ 的桌面开发」工作负载和 Windows SDK。在 PowerShell 中运行：

```powershell
git clone https://github.com/slb1988/Matcha.git
cd Matcha
powershell -ExecutionPolicy Bypass -File .\tools\dev-build.ps1
.\target\debug\matcha.exe
```

- **测试与打包**：在仓库根目录运行 `cargo test --workspace`；`cargo xtask dist` 将发行文件组装到 `target/dist/`，不生成 ZIP。
- **找代码**：主程序入口在 [apps/bcompare](apps/bcompare/)，界面在 [bc-gui](crates/bc-gui/)。[功能文档索引](docs/features/README.md)按任务列出实现位置和验证方法；设计取舍见[决策记录](docs/decisions/)，近期变化见[更新日志](CHANGELOGS.md)。

## 许可证

本项目尚未指定开源许可证；保留所有权利。
