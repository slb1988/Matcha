# Matcha

Matcha 是用 Rust 编写的桌面文件比较与合并工具，支持 **Windows** 和 **macOS**。你可以并排查看两版代码改了哪些字符、两份 Excel 工作簿改了哪些单元格，或在同步两个文件夹之前核对要复制和删除的文件。只需要差异报告时，不用打开窗口。

## 安装与第一次比较

- **Windows x64**：打开[最新正式版](https://github.com/slb1988/Matcha/releases/latest)，在 **Assets** 中下载 `matcha-<版本>-windows-x86_64.zip`，而不是 *Source code*。解压整个 ZIP，双击 `matcha.exe`；使用发行包不需要安装 Rust。
- **macOS**：按下方[从源码编译](#从源码编译)的步骤生成 `Matcha.app`。可以直接在 Finder 中双击，也可以先复制到「应用程序」再打开。

1. 准备两个文本文件：在 `old.txt` 写一行 `port=8080`，在 `new.txt` 写一行 `port=8081`。
2. 从资源管理器或 Finder 选中这两个文件，一起拖到 Matcha 的 **Home** 页。如果文件在不同目录，先点 **Text Compare**，再分别拖到左右窗格。
3. 视图会定位到第一个差异，标出端口号中变化的最后一位。查看长文件时，用 `Ctrl+N` / `Ctrl+P`（macOS：`⌘N` / `⌘P`）跳到下一个 / 上一个差异区段。只查看不会改动文件。

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

| 用途 | Windows 发行目录 | macOS 发行目录 |
| --- | --- | --- |
| 桌面界面 / P4V 交互合并 | `matcha.exe` | 双击 `Matcha.app`；P4V 指向 `Matcha.app/Contents/MacOS/matcha` |
| 终端或脚本：输出报告，等待并返回退出码 | `matcha.com` | `bin/matcha` |
| 版本控制工具发起比较：交给界面后立即返回，不用于合并 | `matcha-comp.exe` | `bin/matcha-comp` |

以下示例都从发行目录运行。把文件名换成实际路径，含空格时加双引号；不要把输入文件放进构建产物目录，重新打包会清空对应的发行目录。

**Windows（PowerShell）**：保留解压后的三个程序，用 `.\matcha.com` 调用：

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

**macOS（终端）**：在包含 `Matcha.app` 和 `bin/` 的发行目录中运行：

```sh
./bin/matcha -qc old.txt new.txt
echo $?
./bin/matcha -report=patch-unified old.txt new.txt
./bin/matcha -report=summary old.xlsx new.xlsx
./bin/matcha -automerge base.txt theirs.txt yours.txt merged.txt
```

若要在任意目录直接输入 `matcha`，可在发行目录运行 `export PATH="$PWD/bin:$PATH"`（当前终端有效）。`bin/` 里是相对符号链接，需与 `Matcha.app` 保持同级；若只保留了安装到「应用程序」的 App，可用 `/Applications/Matcha.app/Contents/MacOS/matcha-compc` 代替上例的 `./bin/matcha`。

- **判断是否相同**用 `/qc`：对文本或表格按规则比较时，`2` 表示相同，`12` 表示只有不重要差异，`13` 表示有重要差异。`/report` 返回 `0` 只表示报告生成成功，不表示文件相同。
- **自动合并**无冲突时写入输出文件并返回 `0`；上例遇到冲突时返回 `101`，不改动已有输出。
- 查看开关：Windows 用 `.\matcha.com /?`，macOS 用 `./bin/matcha --help`。更多示例见[命令行与脚本](docs/features/cli-and-scripting.md)、[报告格式](docs/features/reports.md)和 [P4V 合并配置](docs/features/text-merge.md#p4v-配置)。

## 从源码编译

两端都需要 Git 和 [Rust 稳定版](https://rustup.rs/)（含 Cargo）。

- **Windows**：使用 MSVC 工具链，安装 Visual Studio Build Tools 的「使用 C++ 的桌面开发」工作负载和 Windows SDK。
- **macOS**：安装 Xcode Command Line Tools（含 Git 和系统链接器）。尚未安装时，在终端运行 `xcode-select --install`，等安装完成后再继续。

获取源码，后续构建命令都在仓库根目录运行：

```sh
git clone https://github.com/slb1988/Matcha.git
cd Matcha
```

两端都可以只编译调试版主程序，也可以用 `cargo xtask dist` 编译 Release 版主程序和命令行工具。后者组装发行目录，不生成 ZIP。

### Windows（PowerShell）

编译并启动调试版：

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\dev-build.ps1
.\target\debug\matcha.exe
```

需要包含命令行工具的发行版时，运行 `cargo xtask dist`。在 Windows x64 上，产物位于 `target/dist/matcha-<版本>-windows-x86_64/`，可直接运行其中的 `matcha.exe` 或 `matcha.com`。

### macOS（终端）

编译并启动调试版（不需要安装到「应用程序」）：

```sh
cargo build -p bcompare
./target/debug/matcha
```

生成发行版，并把输出目录的路径存到 `dist`（Apple Silicon 为 `target/dist/matcha-<版本>-macos-aarch64/`）：

```sh
dist="$(cargo xtask dist)"
```

可直接运行 `open "$dist/Matcha.app"`。若要安装到「应用程序」，先退出正在运行的 Matcha，再在同一终端执行：

```sh
ditto "$dist/Matcha.app" /Applications/Matcha.app
open /Applications/Matcha.app
```

## 测试与开发文档

- **运行测试**：在仓库根目录运行 `cargo test --workspace`。
- **找代码**：主程序入口在 [apps/bcompare](apps/bcompare/)，界面在 [bc-gui](crates/bc-gui/)。[功能文档索引](docs/features/README.md)按任务列出实现位置和验证方法；设计取舍见[决策记录](docs/decisions/)，近期变化见[更新日志](CHANGELOGS.md)。

## 许可证

本项目尚未指定开源许可证；保留所有权利。
