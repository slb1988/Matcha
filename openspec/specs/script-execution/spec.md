# script-execution Specification

## Purpose
定义 `@script` 脚本文件如何被加载、解析和无界面执行：语法、参数与变量替换、日志与选项、本阶段支持的命令范围，以及与参考命令行退出码表一致的结果。

## Requirements

### Requirement: 无界面执行脚本
系统 SHALL 在命令行参数以 `@<脚本路径>` 形式给出脚本时，在发起进程内执行该脚本：不打开任何窗口，不显示脚本状态窗口，不转交给已运行的实例。脚本执行结束即进程结束，退出码为脚本结果。

#### Scenario: 已有窗口时运行脚本
- **WHEN** 已有一个 Matcha 窗口在运行，用户在终端执行 `matcha @report.txt a.txt b.txt`
- **THEN** 脚本在新进程内执行完毕并退出，已运行的窗口不打开任何新标签页

#### Scenario: 经控制台启动器运行
- **WHEN** 用户在 cmd 中执行 `matcha @report.txt`（解析到 `matcha.com`）
- **THEN** 终端等待脚本结束，`%ERRORLEVEL%` 为脚本退出码

### Requirement: 脚本文件定位与读取
系统 SHALL 按相对当前目录解析脚本路径；文件不存在或无法读取时 SHALL 不执行任何命令，在标准错误输出说明原因，退出码为 105（Error loading script）。脚本文本 SHALL 按 BOM 识别 UTF-8/UTF-16，无 BOM 时按 UTF-8 读取，非法 UTF-8 字节按系统 ANSI 代码页解码。

#### Scenario: 脚本不存在
- **WHEN** 执行 `matcha @missing.txt`
- **THEN** 标准错误输出包含 `missing.txt`，退出码 105

### Requirement: 脚本基本语法
系统 SHALL 按行处理脚本：每行一条命令；命令关键字与选项名不区分大小写；空行与 `#` 之后的内容忽略；行末 `&` 表示命令延续到下一行；参数以空白分隔，双引号包围的参数可含空格且引号不属于参数值。

#### Scenario: 续行与引号
- **GIVEN** 脚本两行 `TEXT-REPORT layout:patch &` 与 `  output-to:"My Report.txt" "a b.txt" c.txt`
- **WHEN** 脚本执行
- **THEN** 它被当作一条 `text-report` 命令，输出文件为 `My Report.txt`，比较对象为 `a b.txt` 与 `c.txt`

#### Scenario: 注释
- **WHEN** 一行为 `# 生成补丁` 或命令后跟 `# 说明`
- **THEN** `#` 及其后内容不参与解析

### Requirement: 参数与变量替换
系统 SHALL 在解析每行前进行替换：`%1`…`%9` 为命令行中脚本名之后、不以 `/` 开头的第 1…9 个参数（缺失时为空串）；`%NAME%` 为同名环境变量（名称大小写敏感，未定义时保持原文）；`%date%` 为当前日期 `yyyy-mm-dd`，`%time%` 为当前时间（系统区域格式），`%fn_time%` 为 `hh-mm-ss`。

#### Scenario: 命令行参数
- **GIVEN** 脚本行 `text-report layout:patch output-to:- "%1" "%2"`
- **WHEN** 执行 `matcha @diff.txt /silent left.txt right.txt`
- **THEN** 比较对象为 `left.txt` 与 `right.txt`（`/silent` 不计入 `%1`）

#### Scenario: 环境变量
- **WHEN** 脚本行含 `output-to:"%TEMP%\report.txt"`
- **THEN** 报告写到当前用户临时目录下的 `report.txt`

### Requirement: 执行前整体校验
系统 SHALL 在执行第一条命令前解析整份脚本。未知命令或参数不合法 SHALL 视为语法错误：不执行任何命令，标准错误输出给出行号与原因，退出码 106。参考工具的其他命令（如 `load`、`select`、`sync`、`folder-report`）SHALL 被识别为合法关键字，但在本阶段 SHALL 以“尚不支持”整体拒绝：不执行任何命令，给出行号与命令名，退出码 100。

#### Scenario: 拼错的命令
- **WHEN** 第 3 行为 `text-reprot layout:patch output-to:- a b`
- **THEN** 不执行任何命令，标准错误输出包含 `line 3`，退出码 106

#### Scenario: 尚不支持的命令
- **WHEN** 第 1 行为 `load "C:\left" "C:\right"`，第 2 行为合法的 `text-report`
- **THEN** 第 2 行不执行，标准错误输出说明 `load` 尚不支持并给出行号，退出码 100

### Requirement: 本阶段支持的命令
系统 SHALL 执行以下命令：`text-report`（见 text-reports 能力）、`log`、`option stop-on-error`、`beep`。`option confirm:` 的三个取值 SHALL 被接受且无副作用（本阶段没有需要确认的操作）。

#### Scenario: beep
- **WHEN** 脚本包含 `beep`
- **THEN** 发出系统提示音，即使带有 `/silent`

### Requirement: 错误处理与退出码
脚本所有命令成功时退出码 SHALL 为 0。某条报告命令无法读取比较对象时，该命令失败并记录错误；默认继续执行后续命令；`option stop-on-error` 生效后，失败的命令 SHALL 终止脚本。只要有命令因无法读取文件而失败，退出码 SHALL 为 107（Script failed to load file or folder）；其他运行时失败（如无法写出报告）退出码 SHALL 为 100。

#### Scenario: 比较对象不存在
- **WHEN** 脚本唯一一行的 `text-report` 比较对象 `a.txt` 不存在
- **THEN** 标准错误输出说明 `a.txt` 无法读取，退出码 107

#### Scenario: stop-on-error
- **GIVEN** 脚本先 `option stop-on-error`，随后两条 `text-report`，第一条的比较对象不存在
- **WHEN** 脚本执行
- **THEN** 第二条不执行，退出码 107；去掉 `option stop-on-error` 时第二条照常生成报告，退出码仍为 107

### Requirement: 日志
`log` SHALL 控制日志级别与位置：初始不记录；`log normal`/`log verbose` 开始记录，默认写入当前目录 `Log.txt`（覆盖），`log <级别> <文件>` 指定文件，`append:<文件>` 追加。日志 SHALL 记录每条命令的开始与结果及错误；`verbose` 另记录替换后的完整命令行。无论是否记录日志，错误 SHALL 同时写到标准错误输出。

#### Scenario: 追加日志
- **WHEN** 脚本以 `log normal append:run.log` 开头并执行一条成功的 `text-report`
- **THEN** `run.log` 原内容保留，末尾新增该命令的开始与成功记录

### Requirement: 静默开关
`/silent` SHALL 被接受；由于脚本始终无界面执行，它不改变报告与退出码，只抑制 Matcha 自身的进度提示（若有）。

#### Scenario: 带 /silent 的脚本
- **WHEN** 执行 `matcha @diff.txt /silent a.txt b.txt` 且脚本把报告写到 `output-to:-`
- **THEN** 标准输出内容与不带 `/silent` 时逐字节相同
