# Spec Delta

## Purpose

定义报告生成后送往何处以及如何写出：写入文件、写到标准输出（Matcha 扩展，便于终端查看、重定向与管道），对本阶段不支持的目标与输出选项的处理，以及输出字节的一致性要求。

## ADDED Requirements

### Requirement: 写入文件
`output-to:<文件名>` SHALL 把完整报告写入该文件，路径按当前目录解析；文件已存在时覆盖。所在文件夹不存在或文件无法写入时该命令失败（退出码 100），且不留下不完整的报告文件。文件内容（字符编码、BOM、换行符）SHALL 与参考工具写出的同类报告一致。

#### Scenario: 覆盖已有文件
- **WHEN** `report.patch` 已存在，脚本以 `output-to:report.patch` 生成报告
- **THEN** 文件内容被新报告完整替换

#### Scenario: 文件夹不存在
- **WHEN** 以 `output-to:"no-such-dir\r.patch"` 生成报告
- **THEN** 该命令失败，标准错误输出说明无法写入，退出码 100，`no-such-dir` 下没有残留文件

### Requirement: 写到标准输出
`output-to:-` SHALL 把报告写到进程的标准输出（Matcha 扩展，参考工具无此目标）。写出的字节 SHALL 与同一报告写入文件时完全相同，因此 `> 文件` 重定向的结果等于 `output-to:<文件>` 的结果。同一脚本中多条写到标准输出的报告按执行顺序依次输出，彼此之间不插入额外内容。

#### Scenario: 重定向等价于写文件
- **WHEN** 分别以 `output-to:-`（重定向到 `a.patch`）与 `output-to:b.patch` 生成同一报告
- **THEN** `a.patch` 与 `b.patch` 逐字节相同

#### Scenario: 管道
- **WHEN** 执行 `matcha /report=patch-unified a.txt b.txt | findstr "^+"`
- **THEN** 只显示新增行，管道另一端读到的是完整的统一格式报告

### Requirement: 报告与诊断分流
标准输出 SHALL 只承载报告内容；错误、警告与日志 SHALL 写到标准错误输出或日志文件，从而重定向标准输出得到的文件中只有报告。

#### Scenario: 有失败命令的脚本
- **GIVEN** 脚本两条 `text-report` 都写到 `output-to:-`，第一条的比较对象不存在
- **WHEN** 以 `matcha @s.txt > out.patch` 运行
- **THEN** `out.patch` 只含第二条命令的报告，错误说明出现在标准错误输出

### Requirement: 无可用标准输出
进程没有可用的标准输出（例如从资源管理器或快捷方式启动且未重定向）时，写到标准输出的报告命令 SHALL 失败（退出码 100），并以可见方式说明原因，而不是静默丢弃报告。

#### Scenario: 从快捷方式运行脚本
- **WHEN** 通过快捷方式运行 `matcha.exe @s.txt`，脚本以 `output-to:-` 生成报告
- **THEN** 出现说明无可用标准输出的错误提示，退出码 100

### Requirement: 尚不支持的目标与输出选项
`output-to:printer` 与 `output-to:clipboard` SHALL 被识别并以“尚不支持”整体拒绝（退出码 100）。`output-options:` 中的 `html-color`、`html-mono`、`html-custom:<样式表>` SHALL 同样被拒绝。`print-color`、`print-mono`、`print-portrait`、`print-landscape`、`wrap-none`、`wrap-character`、`wrap-word` SHALL 被接受且不改变纯文本报告。未知输出选项为语法错误（退出码 106）。

#### Scenario: 剪贴板
- **WHEN** 脚本行为 `text-report layout:patch output-to:clipboard a.txt b.txt`
- **THEN** 不执行任何命令，标准错误输出说明剪贴板输出尚不支持，退出码 100

#### Scenario: HTML
- **WHEN** 脚本行为 `text-report layout:patch output-to:r.html output-options:html-color a.txt b.txt`
- **THEN** 不执行任何命令，说明 HTML 输出尚不支持，退出码 100

#### Scenario: 换行选项
- **WHEN** 以 `output-options:wrap-word` 写纯文本 patch 到文件
- **THEN** 内容与不带该选项时逐字节相同
