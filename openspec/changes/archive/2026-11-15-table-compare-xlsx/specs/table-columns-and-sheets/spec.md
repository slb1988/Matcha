# Spec Delta

## Purpose

定义比较列与 sheet 的组织方式：左右文件列如何映射成比较列、每列的 Handling、键列的定义与自动推断、列结构变化的识别，以及多 sheet 的配对。这是行对齐与单元格比较的输入。

## ADDED Requirements

### Requirement: 比较列映射
系统 SHALL 以"比较列"为单位组织显示与比较：每个比较列绑定左文件某列与右文件某列（可不同列号，可一侧为空），用户可 Move Up/Down 调整某侧列的比较顺序或整行移动比较列，Tidy 删除空映射；默认映射按列号一一对应。

#### Scenario: 左三对右五
- **WHEN** 用户把左第 3 列与右第 5 列映射为同一比较列
- **THEN** 网格该列左侧显示左文件第 3 列、右侧显示右文件第 5 列，并按该映射比较

### Requirement: 列 Handling
每个比较列 SHALL 有 Handling：Key、Use Default、Type（General 自动 / Text）、Unimportant、文本不重要项（字符大小写、空白）、日期容差、数值容差；会话有默认 Handling，Use Default 的列跟随默认。

#### Scenario: 数值容差
- **WHEN** 某列数值容差为 0.01，两侧为 `1.001` 与 `1.005`
- **THEN** 该单元格为不重要差异（容差内，与参考工具一致）；相差超过容差为重要差异

#### Scenario: 不重要列
- **WHEN** 列标记 Unimportant 且两侧不同
- **THEN** 该单元格为不重要差异（蓝色），开启 Ignore Unimportant 后视为相同

### Requirement: 键列
系统 SHALL 允许把任意比较列组合标记为 Key，优先级按列顺序；无 Key 时用全部比较列唯一标识行。

#### Scenario: 复合键
- **WHEN** 第 1、2 列为 Key
- **THEN** 行按 (列1, 列2) 的值配对，其他列差异不影响配对

### Requirement: 键列自动推断
打开表格时系统 SHALL 自动推断键列候选：对每列（及两列组合）计算两侧唯一性、两侧值集合的重合度与位置稳定性，给出得分最高的建议并在会话设置中标明"自动推断"；用户确认或修改后不再自动变更；推断失败时退回"全部列为键"。

#### Scenario: 典型 id 列
- **WHEN** 第一列两侧均唯一且 95% 的值在对侧出现
- **THEN** 自动把第一列设为 Key，并在状态区提示"键列：id（自动）"

#### Scenario: 无合适键
- **WHEN** 没有任何列或两列组合在两侧唯一
- **THEN** 不设 Key，按全部列对齐，提示用户可手动指定

### Requirement: 列对齐与结构变化
当两侧表头名不同或列数不同时，系统 SHALL 按表头名（归一化大小写与空白）匹配比较列，再对未匹配列按内容相似度匹配；识别并标注：列插入、列删除、列重命名（内容相似而名不同）、列移动（名同位置不同）；用户可在列设置中接受或改写。

#### Scenario: 中间插入一列
- **WHEN** 右侧在第 2 列后插入新列 `note`
- **THEN** 右侧 `note` 显示为"仅右侧列"，其后各列仍与左侧对应列配对，而不是整体错位飘红

#### Scenario: 列重命名
- **WHEN** 左侧 `qty` 与右侧 `quantity` 内容 98% 相同
- **THEN** 两列配对并标注"可能重命名"

### Requirement: sheet 配对
多 sheet 来源 SHALL 默认按名称（归一化大小写与空白）配对，未配对的 sheet 为单侧 sheet；用户可在 Sheets 设置中 Move Up/Down 改变配对，"左文件为源"决定以哪一侧顺序为准；同一文件可同时放两侧以比较自身的两个 sheet。

#### Scenario: 同名配对
- **WHEN** 左右都有 `Summary` 与 `Data`
- **THEN** 两对 sheet 各自配对，标签顺序按左文件

#### Scenario: 手动配对
- **WHEN** 用户把左 `2024` 与右 `2025` 对齐
- **THEN** 两者作为同一比较 sheet 显示
