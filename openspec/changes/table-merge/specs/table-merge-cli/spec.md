# Spec Delta

## Purpose

定义命令行表格三方合并的行为：按输入格式把合并调用分派到表格合并（xlsx/xlsm 自动、csv/tsv 需显式开关）、`/automerge` 无窗口自动合并及其退出码与输出文件、`/reviewconflicts` 转界面决策、`/keys=` 显式键、GUI 合并时调用方的等待与退出码，使 CI 脚本和版本控制工具可以集成表格合并。

## ADDED Requirements

### Requirement: 表格合并的路由
合并调用（3 或 4 个文件路径，或 2 个路径加 `/center=`）的输入均为 xlsx/xlsm 工作簿时 SHALL 按表格合并处理；工作簿 SHALL 按扩展名识别，扩展名不是已知表格/文本扩展名时按文件头（ZIP）识别。输入均不是工作簿时 SHALL 默认按文本合并处理，仅当带 `/tablemerge` 开关时按分隔文本表格做表格合并。工作簿与非工作簿混合、或 `/tablemerge` 遇到 xls/ods 时 MUST 报错并以退出码 100 结束。`/mergeoutput=` 与第 4 个位置参数同时出现时 MUST 以 `/mergeoutput=` 为准。

#### Scenario: xlsx 自动进入表格合并
- **WHEN** 执行 `matcha theirs.xlsx yours.xlsx base.xlsx result.xlsx`
- **THEN** 打开表格合并视图，输出目标为 result.xlsx

#### Scenario: CSV 默认仍走文本合并
- **WHEN** 执行 `matcha a.csv b.csv base.csv out.csv`
- **THEN** 按行做文本合并（既有行为不变）

#### Scenario: CSV 显式走表格合并
- **WHEN** 执行 `matcha /tablemerge a.csv b.csv base.csv out.csv`
- **THEN** 按表格合并处理

#### Scenario: 版本控制临时文件名
- **WHEN** 合并调用的某个 xlsx 输入以 `book.xlsx#3` 之类不以 .xlsx 结尾的名字传入
- **THEN** 按文件头识别为工作簿，仍按表格合并处理

#### Scenario: 混合格式
- **WHEN** 合并调用的输入既有 .csv 又有 .xlsx
- **THEN** 报错说明输入格式不一致，退出码为 100

### Requirement: 无窗口表格自动合并
带 `/automerge` 的表格合并调用 SHALL 在发起进程内无窗口执行，不转交已运行的实例。没有输出目标、任一输入不存在/不可读/不是受支持表格时 MUST 报错并以退出码 100 结束，不写任何文件。退出码 MUST 为：没有冲突且已写出为 0；有冲突未写出为 101；错误为 100。未写出时已存在的输出文件 MUST 保持原样。成功或有冲突时 SHALL 向标准输出写一行结果，`/silent` 时不写。

#### Scenario: 全自动合并成功
- **WHEN** 执行 `matcha /automerge a.xlsx b.xlsx base.xlsx out.xlsx` 且没有冲突
- **THEN** out.xlsx 写入合并结果，退出码为 0

#### Scenario: 有冲突不写出
- **WHEN** 合并存在冲突且带 `/automerge`，out.xlsx 已存在
- **THEN** out.xlsx 内容与修改时间不变，退出码为 101

#### Scenario: `/force` 不适用于表格合并
- **WHEN** 表格合并调用带 `/automerge /force`（无论是否带 `/reviewconflicts`）
- **THEN** 报错说明表格合并不支持冲突标记，退出码为 100，不写任何文件

#### Scenario: 结果行带键
- **WHEN** 表格自动合并成功且未带 `/silent`
- **THEN** 标准输出的结果行包含实际使用的键列，如 `Success (keys: 1 (inferred))`

### Requirement: 显式键开关
`/keys=<列号,…>` SHALL 指定表格合并的键列（1-based 比较列号，按给出顺序定优先级），仅在表格合并调用中生效。列号越界、重复或全部无效时 MUST 报错并以退出码 100 结束。未给出 `/keys=` 时按引擎的键推断与退化规则。

#### Scenario: 指定键后自动合并
- **WHEN** 执行 `matcha /automerge /keys=1 a.xlsx b.xlsx base.xlsx out.xlsx`
- **THEN** 以第 1 列为键完成三方对齐与合并

#### Scenario: 键列无效
- **WHEN** 执行 `matcha /automerge /keys=99 a.xlsx b.xlsx base.xlsx out.xlsx` 且输入没有第 99 列
- **THEN** 报错说明键列无效，退出码为 100

### Requirement: 有冲突时转入界面
带 `/automerge /reviewconflicts` 且存在冲突时，系统 SHALL 不写输出文件并打开表格合并视图，自动合并已应用的结果（含 favor）保留在界面状态中，退出码由界面合并结果决定；没有冲突时行为与单独的 `/automerge` 相同。带 `/silent` 时 `/reviewconflicts` MUST 不生效。

#### Scenario: 有冲突时打开表格合并视图
- **WHEN** 执行 `matcha /automerge /reviewconflicts a.xlsx b.xlsx base.xlsx out.xlsx` 且存在冲突
- **THEN** 打开表格合并视图供用户解决，调用进程等到该标签页关闭

### Requirement: GUI 表格合并的等待与退出码
不带 `/automerge` 的表格合并调用打开界面时，调用进程 SHALL 阻塞到合并标签页关闭，不论请求被转交给已运行实例、自己成为常驻实例、还是以 `/solo` 独立运行。退出码 MUST 为：最后一次保存时没有未解决冲突为 0；最后一次保存时仍有未解决冲突为 14；从未保存为 101；视图无法打开为 100。P4V 的调用形式 `matcha %b %1 %2 %r` 指向 xlsx 文件时 MUST 直接可用。

#### Scenario: 保存后退出
- **WHEN** P4V 以 `matcha.exe %b %1 %2 %r` 调起 xlsx 合并，用户解决冲突、保存并关闭标签页
- **THEN** 结果文件已写入，调用方收到退出码 0

#### Scenario: 放弃合并
- **WHEN** 用户不保存直接关闭标签页
- **THEN** 结果文件未被写入，调用方收到退出码 101

### Requirement: 表格合并与其他开关
`/favorleft`、`/favorright`、`/iu` SHALL 仅在带 `/automerge` 时生效，语义与引擎规格一致。`/title1..4` SHALL 分别作为 left、right、center、输出窗格的标题。`/ro`、`/ro1`、`/ro2`、`/nobackups`、`/savetarget=`、`/vcs1..4` 出现在表格合并调用中时 MUST 被接受且不报错。

#### Scenario: favor 自动解决
- **WHEN** 执行 `matcha /automerge /favorright a.xlsx b.xlsx base.xlsx out.xlsx` 且冲突均只含不重要差异
- **THEN** 冲突按 right 取舍写出，退出码为 0
