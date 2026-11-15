# text-reports Specification

## Purpose
定义文本比较报告的生成：脚本命令 `text-report` 的参数与比较对象、`patch` 布局的三种格式与报告选项，以及免写脚本的 `/report` 命令行快捷方式；报告内容与参考工具一致，以采集的黄金样本为准。

## Requirements

### Requirement: text-report 命令形式
系统 SHALL 接受 `text-report layout:<布局> [options:<选项>] [title:<标题>] output-to:<目标> [output-options:<选项>] [<比较对象>]`，参数顺序任意，`layout:` 与 `output-to:` 必填。`layout:` 本阶段只支持 `patch`；参考工具的其他布局（`side-by-side`、`summary`、`interleaved`、`statistics`、`xml` 以及已弃用的 `over-under`、`composite`）SHALL 被识别并以“尚不支持”拒绝（退出码 100），其他值为语法错误（退出码 106）。

#### Scenario: 缺少 output-to
- **WHEN** 脚本行为 `text-report layout:patch a.txt b.txt`
- **THEN** 视为语法错误，退出码 106

#### Scenario: 尚不支持的布局
- **WHEN** 脚本行为 `text-report layout:side-by-side output-to:r.txt a.txt b.txt`
- **THEN** 不执行脚本任何命令，标准错误输出说明 side-by-side 布局尚不支持，退出码 100

### Requirement: 比较对象
本阶段 `text-report` SHALL 要求比较对象为行末的两个文件名，按当前目录解析。缺少比较对象（参考工具中表示“脚本当前选中的文件”，依赖本阶段不支持的 `load`/`select`）或只给一个名称（参考工具中表示已保存会话）SHALL 以“尚不支持”拒绝（退出码 100）。任一文件无法读取时该命令失败（退出码 107）；任一侧是文件夹时该命令失败（退出码 100）。

#### Scenario: 相对路径
- **WHEN** 在 `D:\work` 执行的脚本含 `text-report layout:patch output-to:- a.txt sub\b.txt`
- **THEN** 比较 `D:\work\a.txt` 与 `D:\work\sub\b.txt`

#### Scenario: 会话名
- **WHEN** 脚本行为 `text-report layout:patch output-to:- "My Session"`
- **THEN** 标准错误输出说明按会话出报告尚不支持，退出码 100

### Requirement: 比较语义
报告 SHALL 采用与文本比较视图相同的比较结果：按文件名选择文件格式，使用该格式的默认比较规则与默认对齐方式，行尾差异按规则不算差异。内容无法按文本解析（二进制）时该命令失败，退出码 100。

#### Scenario: 仅行尾不同
- **WHEN** 左文件为 CRLF 行尾、右文件内容相同但为 LF 行尾
- **THEN** patch 报告不含任何差异块

### Requirement: patch 布局格式
`patch` 布局 SHALL 支持 `options:` 中的 `patch-normal`（默认）、`patch-context`、`patch-unified` 三种格式，分别对应普通、上下文、统一差异格式。每种格式的文件头、差异块头、上下文行数、行前缀、无末尾换行的表示、换行符与字符编码 SHALL 与参考工具对同一输入生成的报告逐字节一致，时间戳与运行环境相关的路径除外（在黄金比对中归一化）。

#### Scenario: 统一格式
- **GIVEN** 左文件 `one,two,three,four` 四行，右文件在 `two` 之后插入 `new A`
- **WHEN** 生成 `text-report layout:patch options:patch-unified output-to:- left.txt right.txt`
- **THEN** 输出含 `---`/`+++` 文件头与一个 `@@` 差异块，块内插入行以 `+new A` 表示，其余与参考样本逐字节一致

#### Scenario: 默认格式
- **WHEN** 未给出 `patch-*` 选项
- **THEN** 输出与指定 `patch-normal` 时逐字节相同

#### Scenario: 黄金语料
- **WHEN** 对仓库 30 个两路文本黄金用例分别生成三种格式
- **THEN** 每份输出在归一化时间戳与路径后与入库的参考样本一致

### Requirement: 相同文件的报告
两侧没有任何差异时，报告 SHALL 与参考工具对相同文件生成的 patch 报告一致（不含差异块），且命令视为成功。

#### Scenario: 内容完全相同
- **WHEN** 对两个内容相同的文件生成 patch 报告
- **THEN** 命令成功，输出不含差异块，与参考样本一致

### Requirement: 报告选项
`options:` 为逗号分隔的列表。`ignore-unimportant` SHALL 把仅含不重要差异的部分视为相同而不出现在 patch 中。`display-*`、`line-numbers`、`strikeout-left-diffs`、`strikeout-right-diffs` 在参考工具中不作用于 patch 布局，SHALL 被接受且不改变输出。已弃用的 `stats-descriptive`、`stats-tabular` SHALL 被接受且不改变 patch 输出。未知选项为语法错误（退出码 106）。`title:` SHALL 被接受，其在 patch 输出中的位置与参考样本一致（参考样本不显示时则不显示）。

#### Scenario: 忽略不重要差异
- **GIVEN** 一种文件格式的规则把注释标记为不重要，两文件只有注释不同
- **WHEN** 以 `options:patch-unified,ignore-unimportant` 生成报告
- **THEN** 输出不含差异块；去掉 `ignore-unimportant` 时含该注释差异

#### Scenario: 无关选项
- **WHEN** 以 `options:patch-unified,line-numbers,display-mismatches` 生成报告
- **THEN** 输出与只用 `options:patch-unified` 时逐字节相同

### Requirement: /report 命令行快捷方式
系统 SHALL 支持 Matcha 扩展开关，免写脚本生成文本报告：`matcha /report=<布局> [/reportoptions=<选项>] [/reportfile=<路径>] <左文件> <右文件>`。`<布局>` 本阶段接受 `patch`、`patch-normal`、`patch-context`、`patch-unified`（后三者等价于 `patch` 加对应 `patch-*` 选项）；`/reportoptions=` 与脚本 `options:` 取值相同；无 `/reportfile=` 时输出到标准输出。其输出 SHALL 与等价的单行 `text-report` 脚本逐字节相同，且不打开窗口、不转交已运行的实例。

#### Scenario: 打印到终端
- **WHEN** 执行 `matcha /report=patch-unified a.txt b.txt`
- **THEN** 统一格式 patch 写到标准输出，与 `text-report layout:patch options:patch-unified output-to:- a.txt b.txt` 的输出逐字节相同

#### Scenario: 写入文件
- **WHEN** 执行 `matcha /report=patch /reportfile=out\diff.patch a.txt b.txt`
- **THEN** 普通格式 patch 写入 `out\diff.patch`，标准输出无报告内容

#### Scenario: 尚不支持的布局
- **WHEN** 执行 `matcha /report=side-by-side a.txt b.txt`
- **THEN** 标准错误输出说明该布局尚不支持，退出码 100

### Requirement: 快捷方式的退出码
`/report` 成功生成报告时退出码 SHALL 为 0，无论两侧是否有差异（判断是否相同用 `/qc`）。文件无法读取时退出码 107；参数不足、比较对象为文件夹、布局不支持或写出失败时退出码 100。

#### Scenario: 有差异仍为成功
- **WHEN** 对两个不同的文件执行 `matcha /report=patch a.txt b.txt`
- **THEN** 退出码 0

#### Scenario: 缺少右文件
- **WHEN** 执行 `matcha /report=patch a.txt`
- **THEN** 标准错误输出说明需要两个文件，退出码 100

### Requirement: file-report 本阶段不支持
`file-report` SHALL 被识别为合法命令，但本阶段以“尚不支持”整体拒绝（退出码 100）：参考工具中它只提供 side-by-side 与 summary 布局，二者均不在本阶段范围内。

#### Scenario: file-report
- **WHEN** 脚本含 `file-report layout:summary output-to:- a.txt b.txt`
- **THEN** 不执行任何命令，标准错误输出说明 `file-report` 尚不支持，退出码 100
