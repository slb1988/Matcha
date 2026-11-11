---
name: matcha-cli
version: 1.0.0
description: Matcha 命令行速查与使用引导：matcha.exe / matcha.com / matcha-comp.exe 三件套分工、/report 无窗口 diff 输出（patch 四种布局）、/qc 快速比较与退出码、@script 脚本模式、GUI 启动方式。当用户问 matcha 命令行怎么用、matcha.com 是什么、如何用命令行比较文件/输出 diff、或在 CI/脚本里集成文件比较时使用。
---

# Matcha 命令行速查

发行产物在 `target/dist/matcha-<version>-windows-x86_64/`（由 `cargo xtask dist` 生成）。
下文 `matcha` 均指把该目录加入 PATH 后直接调用；未加 PATH 时用全路径。

## 三件套分工（对应 BC 的 BComp.exe / BComp.com / BCompare.exe）

| 文件 | 子系统 | 行为 | 用途 |
|---|---|---|---|
| `matcha.exe` | GUI | 主程序，全部比较逻辑 | 双击打开 GUI |
| `matcha-comp.exe` | GUI | 转交给主程序后**立即返回** | VCS 集成（git difftool 等），不闪控制台 |
| `matcha.com` | 控制台 | 转发参数给旁边的 `matcha.exe`，**等待比较结束并传播退出码** | 命令行交互、CI/脚本 |

关键点：

- cmd/PowerShell 里敲 `matcha ...` 经 PATHEXT（`.com` 先于 `.exe`）自动命中 `matcha.com` —— 即默认就是"等待+退出码"语义。
- 启动器不解析参数，原样转发；找不到旁边的 `matcha.exe` 报退出码 103。
- `matcha /?` 输出完整帮助（由 matcha.com 打印）。

## 位置参数推断（无开关时）

```
matcha                          → Home 视图
matcha <dir> <dir>              → 文件夹比较
matcha <file> <file>            → 按文件格式选视图（或 /fv 强制）
matcha <l> <r> <center> [<out>] → 文本合并（可带输出文件）
matcha <name>                   → 已保存的会话 / 工作区
matcha @<script> [args…]        → 脚本模式（无界面执行）
matcha <file.bcpkg>             → 导入设置包
matcha <file.diff|.patch>       → Text Patch
matcha -                        → 从 stdin 读入一个文件
```

## 三个最常用形态（均已实测）

### 1. GUI 比较并等待结果

```bat
matcha D:\Temp\bctest\a.txt D:\Temp\bctest\b.txt
```

打开文本比较窗口，**阻塞到你关闭标签页**，退出码反映比较结果（见退出码表）。

### 2. 无窗口输出 diff 到控制台（Matcha 扩展）

```bat
matcha /report=patch-unified a.txt b.txt            :: 输出到 stdout
matcha /report=patch-unified /reportfile=out.patch a.txt b.txt
```

`/report=` 布局：`patch`（与 `patch-normal` 相同，`diff -r` 风格，是默认格式）、
`patch-context`、`patch-unified`（`@@`，与 `git diff` 的 hunk 正文逐字节一致，默认 6 行上下文）。
`/reportoptions=ignore-unimportant` 可忽略不重要差异。

输出头形如 `--- <绝对路径>\t<时间戳>` / `+++ ...`，可直接给 `patch`/`git apply` 用（注意改成相对路径）。

### 3. 只要退出码不要输出（CI 判断两个文件是否相同）

```bat
matcha /qc a.txt b.txt          :: rules-based（默认），打印一行结论
matcha /qc=binary a.txt b.txt   :: 还有 size / crc
```

## 退出码（对应 BC 约定）

| 码 | 含义 |
|---|---|
| 0 | 成功 |
| 1 | 二进制相同 |
| 2 | 规则相同（rules-based same） |
| 11 | 二进制不同 |
| 12 | 相似（只有不重要差异） |
| 13 | 规则不同 |
| 14 | 合并有冲突 |
| 100 | 未知错误 |
| 101 | 合并冲突且未写出 |
| 102 / 103 | 启动器无法等待 / 找不到 matcha.exe |
| 104~107 | 试用过期 / 脚本加载、语法、文件加载错误 |

**`/report` 模式退出码恒为 0**（报告生成成功即 0），要拿"是否不同"必须改用 `/qc`。

## 常用开关速查（`/` 或 `-` 前缀均可）

```
/fv=<type>        强制视图类型，如 "Text Compare"
/ro /ro1 /ro2     只读（全部/左/右）
/edit             编辑模式打开
/iu               忽略不重要差异
/solo             强制新实例（不转交给已运行的 matcha）
/silent           禁止一切交互弹窗（脚本必备）
/title1=<t> /title2=<t>  面板标题（1..4：左、右、中、输出）
/filters=<masks>  文件夹会话的文件过滤
/automerge /mergeoutput=<path> /force /favorleft /favorright   合并相关
/sync             以 Folder Sync 打开
/vcs1..4=<path>   VCS 路径占位
/savetarget=<f>   保存目标
```

## @script 脚本模式

```bat
matcha @script.txt        :: 无界面顺序执行脚本命令
```

脚本命令一览：`load`、`text-report`、`folder-report`、`hex-report`、`table-report`、
`file-report`、`select`、`compare [crc|binary|rules-based]`、`copy`、`copyto`、`move`、
`delete`、`rename`、`sync (update|mirror):<dir>`、`criteria`、`filter`、`expand`、`collapse`、
`log [none|normal|verbose]`、`option confirm:…`、`snapshot`、`beep`。

脚本里写报告：`text-report layout:patch options:patch-unified output-to:- <left> <right>`
（`output-to:-` = stdout；`output-to:<file>` 写文件）。

## 已知坑（实测踩过）

1. **Git Bash / MSYS 里必须用 `-` 前缀**：`/report=...` 会被路径转换吃掉，变成位置参数，
   报 `Text Merge is not supported yet`。cmd/PowerShell 下 `/` 和 `-` 都行。
2. **`/report` 退出码恒 0**，判断差异用 `/qc`。
3. patch 上下文是 **6 行**（BC 风格，`bc-report/src/patch.rs` 的 `CONTEXT = 6`），
   比 `git diff` 默认 3 行的 hunk 大。
4. `side-by-side` / `summary` / `interleaved` / `statistics` / `xml` 布局已定义但**未实现**，
   会报 `the side-by-side layout is not supported yet`。
5. 报告写 stdout 需要进程有控制台——用 `matcha.com`（而不是 `matcha-comp.exe`）调，
   或加 `/reportfile=` / `output-to:<file>`。

## 读懂 unified diff 输出

```
@@ -旧起始行,旧行数 +新起始行,新行数 @@     行数为 1 时省略（-1 即 -1,1）
 空格前缀 = 上下文行（定位用）   - = 删除   + = 新增
```

patch 定位靠「hunk 头行号粗定位 + 上下文行精确纠偏」：文件行号偏移时工具会搜索上下文自动
对位（offset）；但触及第 1 行 / 文件尾的 hunk 会锚定，不允许偏移。`git apply` 无 fuzz，
GNU `patch` 默认允许两端 2 行上下文不匹配。
