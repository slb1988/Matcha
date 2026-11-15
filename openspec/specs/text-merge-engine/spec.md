# text-merge-engine Specification

## Purpose
定义文本合并引擎的行为：逐行的改动类型、冲突区段的判定、输出的默认内容、忽略不重要差异与 favor 的作用、冲突的解决、自动合并的结果，以及输出字节（冲突标记、行尾、编码）。

## Requirements

### Requirement: 行类型分类
对齐完成后系统 SHALL 为每个对齐行确定类型：两侧都与 center 相同为未变；仅 left 不同为左改；仅 right 不同为右改；两侧都不同且彼此相同为同改；两侧都不同且彼此也不同为异改。一侧为空隙而 center 有行（或相反）SHALL 视为不同。默认情况下任何差异都算不同，不论其重要性。

#### Scenario: 单侧修改
- **WHEN** 某行只有 left 与 center 不同，right 与 center 相同
- **THEN** 该行类型为左改

#### Scenario: 两侧做了相同修改
- **WHEN** 某行 left 与 right 都与 center 不同，且 left 与 right 内容相同
- **THEN** 该行类型为同改

#### Scenario: 不重要的单侧改动仍被采纳
- **WHEN** 规则设置前导空白不重要，left 只改了某行的缩进，right 未动，且未开启忽略不重要差异
- **THEN** 该行类型为左改，输出取 left 的内容

### Requirement: 冲突区段
连续的非未变行 SHALL 构成一个改动段。改动段含异改行，或同时含左侧改动与右侧改动时，MUST 判为冲突区段。相邻两个改动段之间的未变行数不超过 `conflict_gap`（默认 2），且一段带左侧改动、另一段带右侧改动时，两段连同其间的未变行 SHALL 并为一个冲突区段。同改行 SHALL 不计为任何一侧的改动。冲突区段 SHALL 带重要性：区段内全部差异都不重要时为不重要，否则为重要。

#### Scenario: 同一行被两侧改成不同内容
- **WHEN** left 与 right 把 center 的同一行改成了不同内容
- **THEN** 该行所在的改动段是一个冲突区段

#### Scenario: 相距很近的两侧改动
- **WHEN** left 修改了第 5 行，right 修改了第 7 行，第 6 行三方相同，`conflict_gap` 为 2
- **THEN** 第 5–7 行构成一个冲突区段

#### Scenario: 相距较远的两侧改动
- **WHEN** left 修改了第 5 行，right 修改了第 20 行，其间各行三方相同
- **THEN** 没有冲突区段

#### Scenario: 同一位置插入不等长内容
- **WHEN** left 在某位置插入 3 行，right 在同一位置插入 1 行不同内容
- **THEN** 这 3 个对齐行同属一个冲突区段，而不是只有第 1 行冲突

#### Scenario: 两侧做了相同修改不算冲突
- **WHEN** left 与 right 对同一段做了完全相同的修改，附近没有其他改动
- **THEN** 没有冲突区段

### Requirement: 默认输出内容
未经用户决定时，输出 SHALL 逐行取：未变取 center，左改取 left，右改取 right，同改取 left，异改取 center；所取一侧在该行为空隙时该行不产生输出行。此规则在冲突区段内同样适用。

#### Scenario: 无冲突的合并结果
- **WHEN** 所有改动均为单侧改动或同改，且没有冲突区段
- **THEN** 输出包含两侧的全部改动

#### Scenario: 近距离冲突保留两侧改动
- **WHEN** left 修改第 5 行、right 修改第 7 行并因相距过近构成冲突区段
- **THEN** 默认输出的第 5 行取 left、第 7 行取 right，该区段仍计为未解决冲突

#### Scenario: 重叠冲突保持 center
- **WHEN** 某行为异改
- **THEN** 默认输出在该行取 center 的内容

### Requirement: 忽略不重要差异与 favor
开启忽略不重要差异时，重要性为不重要的差异 SHALL 按相同处理：它们不构成改动，相应行取 center。仅由不重要差异构成的冲突区段 SHALL 不计入冲突；此时若 favor 为左或右，该区段 MUST 改取 favored 侧的内容，否则取 center。未开启忽略不重要差异时，favor SHALL 不改变输出内容。

#### Scenario: 不重要差异不与实质修改冲突
- **WHEN** 规则设置大小写不重要并开启忽略不重要差异，left 某行仅大小写与 center 不同，right 对该行做了实质修改
- **THEN** 该行类型为右改，不构成冲突

#### Scenario: 被忽略的不重要冲突取 center
- **WHEN** 开启忽略不重要差异，两侧对同一行做了不同的、但都不重要的修改，favor 未设置
- **THEN** 该区段不计入冲突，输出取 center

#### Scenario: favor 决定被忽略的不重要冲突
- **WHEN** 同上，且 favor 为右
- **THEN** 该区段不计入冲突，输出取 right

### Requirement: 冲突解决
系统 SHALL 支持把一个区段解决为取 left、取 right 或取 center：该区段的输出整体替换为所取侧在区段内的各行，区段不再计入未解决冲突。系统 SHALL 支持在不改变输出内容的前提下把冲突区段标为已解决。解决操作 MUST 只影响目标区段。

#### Scenario: 取一侧解决
- **WHEN** 对同一位置插入不等长内容形成的冲突区段选择取 right
- **THEN** 该区段的输出恰为 right 插入的 1 行，left 插入的 3 行都不出现，未解决冲突数减一

#### Scenario: 接受当前内容
- **WHEN** 对一个近距离冲突区段选择接受当前内容
- **THEN** 输出不变，该区段不再计入未解决冲突

### Requirement: 自动合并结果
自动合并 SHALL 返回未解决的冲突区段列表与输出字节。没有未解决冲突时输出为合并结果。存在未解决冲突且未要求强制标记时 MUST 不产生输出字节，并标明输出被扣留。存在未解决冲突且要求强制标记时，输出为合并结果，其中每个未解决冲突区段以冲突标记块代替。

#### Scenario: 无冲突
- **WHEN** 没有冲突区段
- **THEN** 结果的冲突列表为空，输出为合并后的全文

#### Scenario: 有冲突且未强制
- **WHEN** 存在未解决冲突且未要求强制标记
- **THEN** 结果列出冲突区段，输出为空并标明被扣留

#### Scenario: 三份输入完全相同
- **WHEN** left、right、center 内容逐字节相同
- **THEN** 没有冲突，输出与 center 逐字节相同

### Requirement: 冲突标记
强制标记时，每个未解决冲突区段 SHALL 渲染为：一行 `<<<<<<< ` 加 left 标签，区段内 left 的各行，一行 `=======`，区段内 right 的各行，一行 `>>>>>>> ` 加 right 标签。标签 SHALL 为调用方给出的标题，未给出时为对应文件的文件名。标记行 MUST 各占一行：区段内某侧最后一行没有行尾时，在随后的标记行之前补一个行尾。标记行的行尾 SHALL 取 center 中第一个带行尾的行的行尾，center 没有时依次取 left、right，都没有时为 LF。

#### Scenario: 强制写出带标记冲突
- **WHEN** left 与 right 把 center 的第 3 行分别改成 `A`、`B`，文件名为 `theirs.txt`、`yours.txt`，要求强制标记
- **THEN** 输出在该位置依次为 `<<<<<<< theirs.txt`、`A`、`=======`、`B`、`>>>>>>> yours.txt` 五行，其余行为正常合并结果

#### Scenario: 一侧删除
- **WHEN** 冲突区段内 left 删除了这些行，right 修改了它们，要求强制标记
- **THEN** 标记块中 `<<<<<<<` 行之后紧接 `=======` 行，其后是 right 的各行

### Requirement: 输出的行尾与编码
输出的每一行 SHALL 保留其来源行的行尾字节；只有输出的最后一行可以没有行尾，来源行没有行尾而在输出中不是最后一行时 MUST 补上行尾。输出的编码与 BOM SHALL 按三方取舍：两侧都与 center 相同取 center 的，只有一侧不同取该侧的，两侧都不同且彼此不同取 center 的并给出警告。所选编码无法无损表示输出文本时 MUST 报错而不是写出有损结果。

#### Scenario: 行尾逐行保留
- **WHEN** 规则不比较行尾（默认），center 与 right 使用 CRLF，left 使用 LF 并修改了第 5 行的文字
- **THEN** 输出的第 5 行以 LF 结尾，其余行以 CRLF 结尾

#### Scenario: 无行尾的末行后面又有内容
- **WHEN** center 与 left 的最后一行 `b` 没有行尾，right 在其后追加了一行 `c`
- **THEN** 输出中 `b` 与 `c` 各占一行

#### Scenario: 一侧改了编码
- **WHEN** center 与 right 为 ANSI 编码，left 为带 BOM 的 UTF-8
- **THEN** 输出为带 BOM 的 UTF-8

#### Scenario: 编码无法表示
- **WHEN** 所选编码为 ANSI，而输出中取自另一侧的某行含该代码页无法表示的字符
- **THEN** 合并报错，不产生输出
