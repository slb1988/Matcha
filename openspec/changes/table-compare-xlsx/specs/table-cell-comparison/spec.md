# Spec Delta

## Purpose

定义已配对行中每个单元格的比较规则：按列 Handling 的类型与容差判定相同/不重要/重要，值、公式、格式三层独立标记，以及行与 sheet 的状态汇总。

## ADDED Requirements

### Requirement: 单元格三态
每个比较列的单元格对 SHALL 归为相同、不重要差异、重要差异之一：列为 Unimportant 时任何差异为不重要；文本列按"大小写/空白不重要"选项规范化后比较；数值列在容差内为相同；日期列在日期容差内为相同；General 类型按两侧内容自动判定数值/日期/文本。

#### Scenario: 大小写不重要
- **WHEN** 列开启"字符大小写不重要"，两侧 `Apple` 与 `apple`
- **THEN** 单元格相同

#### Scenario: 空白不重要
- **WHEN** 列开启"空白不重要"，两侧 `a  b` 与 `a b`
- **THEN** 单元格相同

#### Scenario: 日期容差
- **WHEN** 日期容差 1 天，两侧 `2026-01-01` 与 `2026-01-02`
- **THEN** 单元格相同；相差 2 天则为重要差异

### Requirement: 值、公式、格式分层
对 xlsx 来源，每个单元格 SHALL 分别比较显示值、公式文本与格式（数字格式、字体加粗/斜体、填充色）；视图提供"只看值"（默认）、"值+公式"、"值+公式+格式"三档；公式层差异默认为重要，格式层差异默认为不重要且可配。

#### Scenario: 值同公式不同
- **WHEN** 两侧值都是 6，公式分别为 `=A1+A2+A3` 与 `=SUM(A1:A3)`
- **THEN** "只看值"下相同；"值+公式"下为重要差异并在详情面板显示两侧公式

#### Scenario: 仅格式不同
- **WHEN** 两侧值与公式相同，仅一侧加粗
- **THEN** 仅在"值+公式+格式"档显示为不重要差异

### Requirement: 行状态汇总
行状态 SHALL 为其单元格的最高级别（任一重要→重要；否则任一不重要→不重要；否则相同）；孤儿行为重要（Ignore Unimportant 不影响孤儿）；行首色点显示行状态，即使差异列已水平滚出视野。

#### Scenario: 混合行
- **WHEN** 一行有一个不重要差异单元格与一个重要差异单元格
- **THEN** 行色点为红色（重要）

### Requirement: sheet 状态汇总
sheet 标签 SHALL 显示该 sheet 的最高状态色点（红/蓝/无），统计给出每个 sheet 的重要/不重要/孤儿行数。

#### Scenario: 标签色点
- **WHEN** 某 sheet 只有不重要差异
- **THEN** 其标签显示蓝色点

### Requirement: 忽略不重要差异
Ignore Unimportant Differences 开启时系统 SHALL 在导航、计数、显示过滤与标签色点上把不重要差异按相同处理，原始判定 MUST 保留。

#### Scenario: 开关
- **WHEN** 开启后执行 Next Difference Cell
- **THEN** 跳过仅不重要差异的单元格
