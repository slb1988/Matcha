# ADR 301：GUI/命令行双模式——子系统拆分与 `matcha.com`

状态：已实施（Windows）；macOS 布局已预留，未在真机验证。

## 问题

期望：双击 `matcha.exe` 直接出窗口、不弹控制台；在命令行里敲 `matcha …` 又能走命令模式——
被 shell 等待、文本打到终端或重定向到文件、以退出码结束。

Windows 可执行文件的子系统在链接期写死：

- GUI 子系统：双击不弹控制台；但 cmd/PowerShell 交互式不等待它，且从真实控制台启动时
  std 句柄为 NULL（实验证实：控制台句柄不会交给未附着的进程，只有重定向到文件/管道的
  句柄有效）。
- 控制台子系统：shell 等待、能打印；但双击必弹（或闪）控制台窗口。

单文件变通（GUI + `AttachConsole`、控制台 + `FreeConsole`、Win11 新 manifest
`consoleAllocationPolicy`）分别有“提示符先返回/退出码不可靠”“闪窗”“仅新系统”的缺陷。

## 决定

沿用参考实现的同名不同后缀做法：

| 发行文件 | 构建产物 | 子系统 | 用途 |
|---|---|---|---|
| `matcha.exe` | `matcha` | GUI | 双击 / Explorer 菜单 / 快捷方式 |
| `matcha.com` | `matcha-compc` | 控制台 | 命令行 `matcha …`：`PATHEXT` 中 `.COM` 先于 `.EXE` |
| `matcha-comp.exe` | `matcha-comp` | GUI | VCS：转交后立即返回 |

- `matcha.com` 是 `matcha-compc.exe` 的改名副本（Windows 按 PE 头加载，与扩展名无关），由
  `cargo xtask dist` 组装；`/?` 用被调用的名字自称。
- `matcha.exe` 输出（`apps/bcompare/src/console.rs`）：
  - 由 `matcha.com` 启动（等待标记）或开发/计时运行时，入口即 `AttachConsole(ATTACH_PARENT_PROCESS)`。
    `AttachConsole` 只填补 NULL 的 std 句柄，已重定向的保持不变，`> file` 仍写文件。
  - 面向用户的结果（用法、参数错误、不支持的调用、致命错误）先附着；仍无输出通道时弹消息框。
  - 其余诊断不主动附着，避免直接在交互式 cmd 运行时日志冒在提示符之后。
- 启动器按解析符号链接后的真实路径找同目录主程序，并跳过指向自身的候选（Unix 上同名）。

## 验证（2026-11-11，Windows 11 26100）

- PE 子系统字段由测试锁定：`matcha`=GUI、`matcha-comp`=GUI、`matcha-compc`=控制台。
- 真实控制台窗口（非管道）中：PowerShell 与 cmd 的 `matcha` 均解析到 `matcha.com`；
  `matcha /qc a b` 的错误文本出现在控制台缓冲区，退出码 100 传播；`2> file`、`/? > file`、管道均正常。
- ShellExecute 启动 `matcha.exe`（与 Explorer 双击同路径）无 conhost 子进程。

## 后续：无界面命令（change `text-patch-reports`）

`/qc`、`/report=…` 与 `@script` 都在 `matcha.exe` 进程内执行，不开窗口、不转交常驻实例；
经 `matcha.com` 运行时报告写到它的控制台（可重定向、可管道），退出码原样传回。直接从
Explorer/快捷方式运行时没有控制台：错误合并为一个消息框，写到标准输出的报告在执行前即失败
（退出码 100），写文件的报告照常生成。

## 代价与边界

- 交互式 shell 里直接敲 `matcha.exe …`（带后缀）不等待；`/?` 等文本会出现在提示符之后。
- Git Bash 不按 `PATHEXT` 解析，命令行需写 `matcha.com …`。
- 命令行 `matcha a b` 会阻塞到该比较关闭（参考实现同样如此）；不想等用 `matcha-comp`。
- 等待模式下 `matcha.exe` 附着在启动器的控制台上：关闭该控制台或 Ctrl+C 会结束它（改造前
  作为控制台子进程亦然）。

## macOS 预留

macOS 没有子系统之分（Finder 启动不开终端，终端启动自带 stdout），但 GUI 与命令行工具同名
`matcha`，因此 `cargo xtask dist` 的 macOS 布局为：

```
Matcha.app/Contents/Info.plist
Matcha.app/Contents/MacOS/{matcha, matcha-comp, matcha-compc}
bin/matcha       -> ../Matcha.app/Contents/MacOS/matcha-compc
bin/matcha-comp  -> ../Matcha.app/Contents/MacOS/matcha-comp
```

`console.rs` 的非 Windows 实现直接打印。待补：Bundle ID（当前占位 `dev.matcha.Matcha`）、
图标、签名与公证、`bc-instance` 的 Unix 单实例实现、经 LaunchServices 激活窗口。
