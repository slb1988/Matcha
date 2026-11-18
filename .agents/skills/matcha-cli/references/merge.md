# Matcha 三方合并与 P4V

位置参数顺序为 `base theirs yours [output]`。以下 `bat` 示例用于 cmd；Git Bash 从仓库根目录调用 `Tools/matcha/matcha.com`，开关用 `-` 前缀。

## 合并命令

```bat
rem 打开合并界面（保存时写入 yours.txt，并同步 result.txt）
matcha base.txt theirs.txt yours.txt result.txt
matcha /center=base.txt /mergeoutput=out.txt left.txt right.txt

rem 无窗口自动合并（必须有输出目标）
matcha /automerge base.xlsx theirs.xlsx yours.xlsx result.xlsx
rem 指定键列
matcha /automerge /keys=1 base.xlsx theirs.xlsx yours.xlsx result.xlsx
rem 无冲突直接写出返回 0；有冲突才开界面
matcha /automerge /reviewconflicts base theirs yours result
rem CSV/TSV 按表格合并
matcha /tablemerge /automerge base.csv theirs.csv yours.csv result.csv
```

- **选择引擎**：三个输入都是 xlsx/xlsm → 表格合并；其余按行文本合并（CSV/TSV 加 `/tablemerge` 才按表格）；
  混合格式报错 100。`/mergeoutput=` 优先于第 4 个位置参数。
- **表格合并**：行按键列对齐（`/keys=` 1 起列号 > 自动推断 > 整行作键），逐单元格合并——
  两侧改同行不同列不冲突；同格异值、一侧删行另一侧改行、两侧新增同键异内容为冲突。
  xlsx 输出以 base 工作簿为模板只重写单元格数据：样式、列宽、合并单元格、图表、批注、宏保留，
  行增删后公式行引用随行重定位。**表格合并 `/force` 报错 100**（单元格放不下冲突标记）。
  结果行带实际键列，如 `Success (keys: 1 (inferred))`。
- **文本合并**：`/automerge /force` 有冲突也写出，标记为 `<<<<<<< <左标签>` … `=======` … `>>>>>>> <右标签>`（退出码 14）；
  界面保存按当前输出写出，**不插冲突标记**。
- `/automerge` 在发起进程内执行、不转交已有实例；写同目录临时文件再替换，不建备份，输出可等于某个输入。
  结果写 stdout 一行（`/silent` 时不写），错误写 stderr。
- `matcha.com` 和 `matcha.exe` 发起的合并都会等待：转交已有实例时等合并标签页关闭；
  自己是首个实例或 `/solo` 时等窗口关闭；无窗口合并等执行完成。

## P4V 合并工具

在 Preferences › Merge 配置：

| 字段 | 值 |
|---|---|
| Application | `<工作区>\Tools\matcha\matcha.exe` |
| Arguments（打开合并界面） | `%b %1 %2 %r` |
| Arguments（仅冲突时进入界面） | `/automerge /reviewconflicts %b %1 %2 %r` |

界面三栏为 BASE | LEFT（你的）| RIGHT（他们的），保存写 LEFT 即本地文件并同步 `%r`。
扩展名不是 xlsx 的 P4 临时文件按 ZIP 文件头识别为工作簿。

## 合并开关

```text
/automerge          发起进程内无窗口自动合并，必须有输出目标
/reviewconflicts    配合 /automerge：无冲突写出返回 0，有冲突不写并进入界面；优先于 /force
/force              仅 /automerge：文本冲突带标记写出（14）
/iu                 配合 /automerge 或 /reviewconflicts：不重要差异按相同处理
/favorleft /favorright  配合 /iu：被忽略的不重要冲突取指定侧；两者互斥
/center=<file>      两个输入 + 此开关 = 三方合并（共同祖先）
/mergeoutput=<path> 合并输出，优先于第 4 个位置参数
/tablemerge         CSV/TSV 按表格（键对齐逐格）合并；xlsx/xlsm 不需要
/keys=<n,...>       表格合并键列（1 起，按顺序为优先级），越界/重复/0 报错
```

合并调用里 `/ro*`、`/nobackups`、`/savetarget=`、`/vcs1..4` 接受但无效果，输入窗格恒只读。

自动合并成功写出返回 0；未强制写出的冲突返回 101，既有输出保持原样。完整的 GUI、自动合并与错误码见[退出码表](cli-reference.md#退出码)。
