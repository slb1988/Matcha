# Matcha 调用与自动化速查

cmd/PowerShell 的 `matcha` 指 PATH 上的 `matcha.com`；Git Bash 写全 `Tools/matcha/matcha.com`，开关用 `-`。当前版本开关以 cmd 中的 `matcha /?` 为准。

## 启动行为

与 Beyond Compare 的三件套对应：主程序 `matcha.exe`（BCompare.exe）、立即返回的 `matcha-comp.exe`（BComp.exe）、控制台 `matcha.com`（BComp.com）。
- cmd/PowerShell 里敲 `matcha ...` 自动命中 `matcha.com`，默认就是“等待 + 退出码”语义。
- 启动器不解析参数，原样转发；找不到旁边的 `matcha.exe` 报退出码 103。
- 已有 matcha 窗口时新比较转交为它的新标签页；`/solo` 强制开新实例。

## 位置参数推断（无开关时）

```
matcha                              → Home 视图
matcha <dir> <dir>                  → 文件夹比较
matcha <file> <file>                → 按文件格式选视图（xlsx/xlsm/csv/tsv → 表格，二进制 → Hex，其余文本；/fv 可强制）
matcha <base> <theirs> <yours> [<output>] → 三方合并（P4V 顺序 %b %1 %2 %r）；xlsx/xlsm 为表格合并，其余文本合并
matcha "<会话名>"                    → 打开 GUI 里 Save Session 保存的命名会话
matcha @<script> [args…]            → 脚本模式（无界面执行）
matcha <file.bcpkg>                 → 导入设置包
matcha <file.diff|.patch>           → Text Patch
matcha -                            → 从 stdin 读入一个文件
```

## 通用开关

```text
/fv=<type>          强制视图类型，如 "Text Compare"
/ro /ro1 /ro2       只读（全部/左/右）
/edit               编辑模式打开
/solo               强制新实例（不转交给已运行的 matcha）
/silent             禁止交互；自动合并不写结果行并禁用 /reviewconflicts（错误仍报告）
/title1..4=<t>      左/右/中/输出窗格标题；前两个兼作冲突标记标签（默认文件名）
/filters=<masks>    文件夹会话的文件过滤
/expandall          展开全部文件夹
/sync               以 Folder Sync 打开
/nobackups          不建备份
/savetarget=<f>     保存目标
/vcs1..4=<path>     VCS 路径占位
```

报告用 `/report=<layout>`、`/reportoptions=<list>`、`/reportfile=<path>`，见[报告布局与示例](compare-reports.md)。三方合并另见[合并开关](merge.md#合并开关)。

## 资源管理器右键比较

`/shellmenu=on` 或 `/shellmenu=off` 注册或移除右键菜单后退出；运行一次 `matcha.exe` 也会自动注册。
菜单注册在 HKCU，无需管理员权限。Win11 从「显示更多选项」进入：
`Matcha: Select Left File/Folder for Compare` 记住左侧 → 对另一项选 `Matcha: Compare to "<名>"`；
同时选中两项用 `Matcha: Compare`。都以新标签页打开到已运行的 matcha。Tools › Options › Startup 可关闭。

## @script 脚本模式

```bat
rem 无界面顺序执行脚本命令
matcha @script.txt
```

脚本命令一览：`load`、`text-report`、`folder-report`、`hex-report`、`table-report`、
`file-report`、`select`、`compare [crc|binary|rules-based]`、`copy`、`copyto`、`move`、
`delete`、`rename`、`sync (update|mirror):<dir>`、`criteria`、`filter`、`expand`、`collapse`、
`log [none|normal|verbose]`、`option confirm:…`、`snapshot`、`beep`。

脚本里写报告：`text-report layout:patch options:patch-unified output-to:- <left> <right>`
（`output-to:-` = stdout；`output-to:<file>` 写文件）。表格：
`table-report layout:summary options:title-row:1 output-to:- <left.xlsx> <right.xlsx>`（`data-report` 同义）。

## 退出码

| 码 | 含义 |
|---|---|
| 0 | 成功；`/automerge` 无冲突且已写出；界面合并最后一次保存时无未解决冲突 |
| 1 | 二进制相同 |
| 2 | 规则相同（rules-based same） |
| 11 | 二进制不同 |
| 12 | 相似（只有不重要差异） |
| 13 | 规则不同 |
| 14 | `/automerge /force` 有冲突且已带标记写出；界面合并保存时仍有未解决冲突 |
| 100 | 错误（参数、输入、写出错误，或不支持的报告布局） |
| 101 | `/automerge` 有冲突未写出（既有输出保持原样）；界面合并从未保存 |
| 102 / 103 | 启动器无法等待 / 找不到 matcha.exe |
| 104~107 | 试用过期 / 脚本加载、语法、文件加载错误 |

`/report` 的 0 表示报告生成成功；按文件内容区分相同或不同，读取 `/qc` 的退出码。
