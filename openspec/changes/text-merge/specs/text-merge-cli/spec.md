# Spec Delta

## Purpose

定义命令行三方合并的行为：合并调用的参数形式、`/automerge` 无窗口自动合并及其退出码与输出文件、`/reviewconflicts`、GUI 合并时调用方的等待与退出码，使 CI 脚本和版本控制工具（Perforce P4V）可以直接集成。

## ADDED Requirements

### Requirement: 合并调用的参数形式
命令行给出 3 个文件路径时 SHALL 视为文本合并调用，依次为 left、right、center；给出 4 个时第 4 个为输出文件。两个文件路径加 `/center=<file>` SHALL 等同于 3 个路径的形式。`/mergeoutput=<path>` SHALL 指定输出文件，与第 4 个位置参数同时出现时 MUST 以 `/mergeoutput=` 为准。合并调用 SHALL 一律按文本合并处理，不按文件格式改用其他视图。P4V 的调用形式 `matcha %1 %2 %b %r`（theirs、yours、base、result）MUST 直接可用。

#### Scenario: 四参数打开合并视图
- **WHEN** 执行 `matcha theirs.txt yours.txt base.txt result.txt`
- **THEN** 打开文本合并视图，left 为 theirs.txt、right 为 yours.txt、center 为 base.txt，输出目标为 result.txt

#### Scenario: 三参数无输出目标
- **WHEN** 执行 `matcha theirs.txt yours.txt base.txt`
- **THEN** 打开文本合并视图，没有输出目标

#### Scenario: 以开关给出 center 与输出
- **WHEN** 执行 `matcha /center=base.txt /mergeoutput=out.txt a.txt b.txt`
- **THEN** 行为与 `matcha a.txt b.txt base.txt out.txt` 相同

#### Scenario: CSV 按文本合并
- **WHEN** 以三个 `.csv` 文件发起合并调用
- **THEN** 按行做文本合并，不打开表格视图

### Requirement: 无窗口自动合并
带 `/automerge` 的合并调用 SHALL 在发起进程内无窗口执行三方自动合并，不转交给已运行的实例，结果写入输出文件。没有输出目标、不是三方调用、或任一输入是文件夹、不存在、不可读、不是文本文件时 MUST 报错并以退出码 100 结束，不写任何文件。`/iu`、`/favorleft`、`/favorright`、`/force` SHALL 仅在带 `/automerge` 时生效。

#### Scenario: 全自动合并成功
- **WHEN** 执行 `matcha /automerge a.txt b.txt base.txt out.txt` 且没有冲突
- **THEN** 合并结果写入 out.txt，退出码为 0

#### Scenario: 已有窗口时自动合并
- **WHEN** 已有一个 Matcha 窗口在运行，执行 `matcha /automerge a.txt b.txt base.txt out.txt`
- **THEN** 合并在新进程内完成并退出，已运行的窗口不打开任何新标签页

#### Scenario: 缺少输出目标
- **WHEN** 执行 `matcha /automerge a.txt b.txt base.txt`
- **THEN** 标准错误说明需要输出文件，退出码为 100

#### Scenario: 输入不是文本
- **WHEN** 执行 `matcha /automerge a.xlsx b.xlsx base.xlsx out.xlsx`
- **THEN** 标准错误说明输入不是文本文件，退出码为 100，不写 out.xlsx

### Requirement: 自动合并的退出码与输出文件
无窗口自动合并的退出码 MUST 为：没有冲突且已写出为 0；有冲突、带 `/force` 并已写出带标记的输出为 14；有冲突且未写出为 101；错误为 100。未写出时，已存在的输出文件 MUST 保持原样。写出 SHALL 先写同目录临时文件再替换目标，不创建备份文件；输出路径与某个输入相同时 MUST 仍能正确写出。输出文件的既有内容 SHALL 不参与合并。成功或有冲突时 SHALL 向标准输出写一行结果，`/silent` 时不写；进程没有标准输出（由 GUI 程序启动）时 MUST 不写也不弹出任何窗口。

#### Scenario: 有冲突强制写出
- **WHEN** 合并存在冲突且带 `/automerge /force`
- **THEN** 输出文件含冲突标记，退出码为 14

#### Scenario: 有冲突不写出
- **WHEN** 合并存在冲突，带 `/automerge` 而不带 `/force`，out.txt 已存在
- **THEN** out.txt 的内容与修改时间不变，退出码为 101

#### Scenario: 输出覆盖输入
- **WHEN** 执行 `matcha /automerge a.txt b.txt base.txt b.txt` 且没有冲突
- **THEN** b.txt 被替换为合并结果，退出码为 0

#### Scenario: 由没有控制台的程序启动
- **WHEN** P4V 直接启动 `matcha.exe /automerge …` 且合并成功
- **THEN** 不出现消息框或其他窗口，进程以退出码 0 结束

#### Scenario: 静默
- **WHEN** 带 `/silent` 执行无窗口自动合并
- **THEN** 标准输出没有结果行，退出码不变

### Requirement: 有冲突时转入界面
带 `/automerge /reviewconflicts` 且存在冲突时，系统 SHALL 不写输出文件并打开文本合并视图，退出码由界面合并的结果决定；没有冲突时行为与单独的 `/automerge` 相同。`/reviewconflicts` SHALL 优先于 `/force`；带 `/silent` 时 `/reviewconflicts` MUST 不生效。

#### Scenario: 能自动合并就不开窗
- **WHEN** 执行 `matcha /automerge /reviewconflicts a.txt b.txt base.txt out.txt` 且没有冲突
- **THEN** 不出现窗口，out.txt 写入合并结果，退出码为 0

#### Scenario: 有冲突时打开合并视图
- **WHEN** 同一命令但存在冲突
- **THEN** 打开文本合并视图供用户解决，调用进程等到该标签页关闭

### Requirement: GUI 合并时调用方等待
不带 `/automerge` 的合并调用打开界面时，调用进程 SHALL 阻塞到合并标签页关闭，不论它是把请求转交给了已运行的实例、自己成为常驻实例、还是以 `/solo` 独立运行，也不论是经 `matcha.exe` 还是 `matcha.com` 启动。

#### Scenario: 已有实例时仍然等待
- **WHEN** 已有一个 Matcha 窗口在运行，P4V 以 `matcha.exe %1 %2 %b %r` 调起合并
- **THEN** 已运行的窗口打开合并标签页，P4V 启动的进程直到该标签页关闭才退出

#### Scenario: 没有实例时
- **WHEN** 没有 Matcha 在运行，P4V 以 `matcha.exe %1 %2 %b %r` 调起合并，用户保存后关闭这唯一的标签页
- **THEN** 窗口关闭，进程退出

### Requirement: GUI 合并的退出码
GUI 合并结束时调用进程的退出码 MUST 为：最后一次保存输出时没有未解决冲突为 0；最后一次保存输出时仍有未解决冲突为 14；从未保存输出为 101；合并视图无法打开为 100。

#### Scenario: 保存无冲突结果
- **WHEN** 用户解决全部冲突、保存并关闭标签页
- **THEN** 结果文件已写入，调用方收到退出码 0

#### Scenario: 放弃合并
- **WHEN** 用户不保存直接关闭标签页
- **THEN** 结果文件未被写入，调用方收到退出码 101

#### Scenario: 输入无法打开
- **WHEN** 合并调用的某个输入文件不存在
- **THEN** 界面显示错误，调用方收到退出码 100

### Requirement: 合并调用与其他开关
`/solo` SHALL 使合并在独立窗口中打开。`/title1`、`/title2`、`/title3`、`/title4` SHALL 分别作为 left、right、center、输出窗格的标题，其中前两个同时用作冲突标记的标签。`/ro`、`/ro1`、`/ro2`、`/nobackups`、`/savetarget=`、`/vcs1..4` 出现在合并调用中时 MUST 被接受且不报错。

#### Scenario: 标题
- **WHEN** 执行 `matcha /title1=Theirs /title2=Yours a.tmp b.tmp base.tmp out.txt`
- **THEN** left 与 right 窗格的标题显示为 Theirs 与 Yours 而不是临时文件路径
