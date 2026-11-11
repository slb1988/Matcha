# text-patch-reports 任务 6.1：真实控制台与无控制台验收

2026-11-11，Windows 11 26100，`cargo xtask dist` 产出的 `target/dist/matcha-0.1.0-windows-x86_64/`（`matcha.exe` GUI 子系统 + `matcha.com` 控制台启动器 + `matcha-comp.exe`）。三个脚本都在**新建的控制台窗口**里运行（不是管道），末尾转储该窗口的控制台缓冲区，证明报告确实显示在控制台中。

| 文件 | 场景 | 结果 |
|---|---|---|
| [`console-check.ps1`](console-check.ps1) → [`powershell.log`](powershell.log) | PowerShell 窗口 | `matcha` 解析到 `matcha.com`；`/report=patch-unified` 与 `@s.txt a.txt b.txt`（`%1 %2`）的统一格式 patch 出现在控制台缓冲区，退出码 0；管道 `\| Select-String '^\+'` 得到 `+2`、`+four`；语法错误 106（带行号）、比较文件缺失 107、脚本缺失 105、`/report=side-by-side` 100 |
| [`console-check.cmd`](console-check.cmd) → [`cmd.log`](cmd.log) | cmd 窗口 | `where matcha` 先列 `.com`；normal 格式 patch 出现在控制台缓冲区；`> redirected.patch`、`/reportfile=direct.patch` 与脚本 `output-to:s.patch` 三者 `fc /b` 逐字节相同；管道到 `findstr /b "+"` 正常；缺失文件 107 且错误在标准错误 |
| [`no-console-check.ps1`](no-console-check.ps1) → [`no-console.log`](no-console.log) | 无控制台（经 GUI 子系统的 `matcha-comp.exe` 以 ShellExecute 启动，等同 Explorer/快捷方式） | 写到标准输出的 `/report=patch` 弹出标题为 “Matcha” 的消息框，关闭后退出码 100；`/reportfile=r.patch` 无窗口、自行退出、退出码 0、文件内容正确 |

全量回归（任务 6.2）：[`regression.log`](regression.log)——`cargo clippy --workspace --all-targets` 零警告，`cargo test --workspace` 599 通过 0 失败（含 255 份参考 patch 样本逐字节比对与经 `matcha.com` 的端到端测试），`openspec validate text-patch-reports --strict` 通过。

注意（no-console-check 的方法）：直接 `Start-Process matcha.exe` 时父进程是自动化环境里的 PowerShell，它可能带隐藏控制台，`AttachConsole` 会成功，报告写进看不见的控制台——这不代表 Explorer 双击。脚本因此经 `matcha-comp.exe` 启动，并用一个只记录 std 句柄的探针确认过该链路下 `matcha.exe` 的 std 句柄为 NULL、`AttachConsole` 失败、没有控制台窗口。

复现：

```powershell
cargo xtask dist
$e = "$PWD\docs\worker-reports\evidence\text-patch-reports"; $d = "$PWD\target\dist\matcha-0.1.0-windows-x86_64"
Start-Process -Wait powershell -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',"$e\console-check.ps1",'-Dist',$d,'-Log',"$e\powershell.log"
Start-Process -Wait cmd.exe -ArgumentList '/c',"$e\console-check.cmd",$d,"$e\cmd.log"
powershell -NoProfile -ExecutionPolicy Bypass -File "$e\no-console-check.ps1" -Dist $d -Log "$e\no-console.log"
```
