# table-compare-view Specification

## Purpose
定义表格比较视图的用户可感知行为：双网格联动、导航、显示过滤、列操作、编辑、查找、保存、sheet 标签与差异摘要导出，使体验与参考工具 Table Compare 一致并补充结构化摘要。

## Requirements

### Requirement: 双网格联动
视图 SHALL 并排（或上下）显示两个网格，同一比较行在两侧同高，水平与垂直滚动同步；表头显示比较列名（有列名时）或列号；行首显示行状态色点；提供缩略图、Text Details 面板（显示当前单元格两侧完整文本/公式）、File Info 面板与工具栏开关。

#### Scenario: 水平滚动联动
- **WHEN** 用户在左网格水平滚动到第 20 列
- **THEN** 右网格同步到对应比较列

### Requirement: 差异导航
视图 SHALL 提供 Next/Previous Difference Cell、Next/Previous Difference Row、Next/Previous Edit、Go To（行、列），首尾有提示；Ignore Unimportant 影响导航目标。

#### Scenario: 下一差异单元格
- **WHEN** 光标在相同单元格上执行 Next Difference Cell
- **THEN** 光标移到行优先、列次序的下一个差异单元格并滚入视野

### Requirement: 显示过滤与列显示
视图 SHALL 提供 Show All / Show Differences / Show Same / Show None、Ignore Unimportant Differences、Hide Same Columns、Resize Columns to Fit、Visible Whitespace、Row Numbers、Side-by-Side / Over-Under Layout。

#### Scenario: 隐藏相同列
- **WHEN** 开启 Hide Same Columns
- **THEN** 没有任何差异的比较列被隐藏，表头提示隐藏列数

### Requirement: 编辑
视图 SHALL 支持单元格就地编辑（Modify）、Cut/Copy/Paste/Delete、Delete Row/Column、Insert Row Before/After、Insert Column Before/After、Sort（对选中列排序）、Copy to Other Side、Copy Row to Other Side、Select All；全部可撤销，编辑后重新比较该行（键改变时重新对齐）。

Sort SHALL 按选中且可见的比较列顺序做稳定多键排序，映射到当前侧源列并按其 Handling 解释值。原始选区跨比较行时，仅重排其中仍可见、在当前侧存在的源行，并回填这些源行位置；否则在有有效可见选区时排序当前侧全表。未选中的源行 MUST 保持原位；排序键比较相等的行在升序和降序中均保持原源行相对顺序。空选区、全为空隙或顺序未变化时 MUST 不生成编辑历史。数值/日期容差不是排序相等关系；排序不改写 KeySelection 或“排序后对齐”设置。

#### Scenario: 复制单元格到对侧
- **WHEN** 对一个差异单元格执行 Copy to Right
- **THEN** 右侧单元格变为左侧值，该单元格变为相同，可撤销

#### Scenario: 修改键值
- **WHEN** 用户把某行键值改为对侧存在的键
- **THEN** 重新对齐后该行与对侧对应行配对

#### Scenario: 多列整侧排序
- **WHEN** 用户在同一比较行选择两列，第一列映射到源第 3 列、第二列映射到源第 2 列，并执行 Sort
- **THEN** 当前侧全表按源第 3 列、源第 2 列的优先级稳定排序，另一侧不变，整次排序可一次撤销/重做

#### Scenario: 过滤后的跨行选区排序
- **WHEN** 跨行选区覆盖源行 1、3、5，源行 2、4 被显示过滤隐藏，并执行 Sort
- **THEN** 只重排源行 1、3、5 并回填原位置；源行 2、4 保持原位；若过滤后仅剩一行，不扩展为整表排序

### Requirement: Align With
用户 SHALL 可选中一行再点击对侧一行强制配对，覆盖键对齐；清除覆盖后恢复。

#### Scenario: 强制配对
- **WHEN** 对键不同的两行执行 Align With
- **THEN** 两行并排显示并按单元格比较

### Requirement: 查找与替换
视图 SHALL 提供 Find / Replace / Find Next / Find Previous（字面/正则、大小写、全词、方向、指定侧），命中定位到单元格。

#### Scenario: 查找命中
- **WHEN** 查找 `overdue`
- **THEN** 光标定位到首个包含该文本的单元格

### Requirement: 会话命令与保存
视图 SHALL 提供 Save Session / Session Settings（Specs/Format/Columns/Alignment/Sheets）、Swap Sides、Reload、Recompare、Table Compare Info（统计）、Compare Files Using、Compare Parent Folders、Open File / Open Clipboard、两侧独立 Save / Save As（分隔文本）；xlsx 侧按 table-source-formats 只读规则。

#### Scenario: 统计
- **WHEN** 打开 Table Compare Info
- **THEN** 显示每个 sheet 的相同/不重要/重要/仅左/仅右行数与列数

### Requirement: sheet 标签
多 sheet 时视图底部 SHALL 显示标签页，带该 sheet 的状态色点，切换标签切换比较 sheet，导航在当前 sheet 内。

#### Scenario: 切换 sheet
- **WHEN** 用户点击带红点的标签
- **THEN** 网格切换到该 sheet，Next Difference Row 在其中导航

### Requirement: 差异摘要导出
视图 SHALL 可导出当前 sheet 的结构化差异摘要（文本或 CSV）：按键列出每一行的状态与变化的比较列（旧值 → 新值），以及列结构变化（插入/删除/重命名/移动）；摘要内容与视图当前的 Ignore Unimportant 设置一致。

#### Scenario: 导出摘要
- **WHEN** 两侧仅键 7 的 `price` 从 1.0 变为 1.2
- **THEN** 摘要包含一条 `7 | price | 1.0 -> 1.2`，无其他行

### Requirement: 大表交互性能
视图 SHALL 只渲染可见行列；100 万行 × 10 列的网格滚动与导航不出现可感知卡顿。

#### Scenario: 滚动基准
- **WHEN** 对两份 100 万行 CSV 执行滚动基准
- **THEN** update p99 不超过 4 ms
