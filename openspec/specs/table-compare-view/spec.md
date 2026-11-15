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

### Requirement: Text Details 列宽调整
Text Details 对照条 SHALL 允许拖拽列头边界调整列宽（边界 ±3 px 命中、悬停显示水平调整光标、最小 20 px），双击列头边界 SHALL 把该列宽度适配为当前行两侧单元格与列头内容的最大宽度（夹在 20–1000 px）。对照条列宽为独立覆盖值：未覆盖的列跟随上方网格列宽，对照条内的调整 MUST NOT 改变上方网格列宽。`Resize Columns to Fit`、切换 sheet 与重新加载 MUST 清除全部覆盖值。

#### Scenario: 拖拽对照条列边界
- **WHEN** 用户把对照条 H 列右边界向右拖动 200 px
- **THEN** 对照条 H 列变宽 200 px，后续列随之右移，上方两个网格的 H 列宽度不变

#### Scenario: 未覆盖列跟随上方网格
- **WHEN** 对照条只调整过 H 列，用户随后在上方网格拖宽 F 列
- **THEN** 对照条 F 列同步变宽，H 列保持对照条中设置的宽度

#### Scenario: 双击边界自动适配
- **WHEN** 当前行左侧 H 列文本为 80 字符、右侧为 60 字符，用户双击对照条 H 列右边界
- **THEN** 对照条 H 列宽度变为容纳 80 字符所需宽度（不超过 1000 px）

#### Scenario: Resize Columns to Fit 清除覆盖
- **WHEN** 对照条 H 列已被手动调宽，用户执行 Resize Columns to Fit
- **THEN** 对照条 H 列恢复为上方网格的 H 列宽度

### Requirement: Text Details 独立水平滚动
Text Details 对照条 SHALL 拥有独立于上方网格的水平滚动位置，内容超出宽度时底部显示可点击、可拖动的横向滚动条；鼠标在对照条上滚动滚轮（纵向或横向）SHALL 水平滚动对照条且 MUST NOT 滚动上方网格。光标列因任意操作改变时，对照条 SHALL 以最小滚动量使光标列可见；光标列未变时 MUST NOT 自动改变对照条滚动位置。

#### Scenario: 滚轮滚动对照条
- **WHEN** 鼠标悬停在对照条上并向下滚动滚轮
- **THEN** 对照条内容向左滚动（显示右侧列），上方网格的水平与垂直滚动位置不变

#### Scenario: 光标列变化时对照条跟随
- **WHEN** 对照条滚动在 A–C 列，用户在上方网格用方向键把光标移到 J 列
- **THEN** 对照条滚动到 J 列完全可见（列宽超过视口时其左边缘对齐视口左边缘）

#### Scenario: 手动滚动不被吸回
- **WHEN** 用户拖动对照条滚动条离开光标列，且之后未移动光标
- **THEN** 对照条保持用户设置的滚动位置

### Requirement: Text Details 单元格选择联动
单击 Text Details 对照条某侧单元格 SHALL 把活动侧切到该侧、光标移到（当前行，该列）、把键盘焦点交给该侧上方网格，并使上方网格水平滚动到该单元格可见；Shift+单击 SHALL 以现有锚点在当前行内扩展选区；单击对照条列头 SHALL 把光标移到活动侧当前行的该列。上方网格选区覆盖当前行的单元格 SHALL 在对照条活动侧以选中色显示。Align With 待选状态下单击对照条另一侧 SHALL 与单击上方网格对应行等效。

#### Scenario: 单击右侧单元格切换焦点
- **WHEN** 活动侧为左、光标在第 136 行 F 列，用户单击对照条右侧行的 H 列
- **THEN** 活动侧变为右，光标为第 136 行 H 列，右侧上方网格获得键盘焦点并滚动到 H 列可见，随后按右方向键光标移到 I 列

#### Scenario: Shift+单击扩展选区
- **WHEN** 光标在 F 列，用户 Shift+单击对照条同侧的 H 列
- **THEN** 选区为当前行 F–H 三格，上方网格与对照条均以选中色显示这三格

#### Scenario: 单击列头
- **WHEN** 活动侧为左，用户单击对照条列头 I
- **THEN** 光标移到当前行 I 列，活动侧仍为左

### Requirement: Text Details 编辑与上下文菜单
双击 Text Details 对照条单元格 SHALL 等同在上方网格中对该单元格执行 Modify（编辑器显示在上方网格对应位置，只读侧给出与上方网格相同的提示）。右键对照条单元格 SHALL 按上方网格规则处理选区（所点格在选区内保留选区，否则先选中所点格），然后弹出与该侧网格相同的上下文菜单，菜单命令作用于该侧。

#### Scenario: 双击编辑
- **WHEN** 左侧为可编辑 CSV，用户双击对照条左侧行的 G 列
- **THEN** 上方左侧网格滚动到当前行 G 列并打开单元格编辑器

#### Scenario: 右键菜单
- **WHEN** 用户右键对照条右侧行的 H 列
- **THEN** 光标移到右侧当前行 H 列，并弹出包含 Copy to Left 的表格上下文菜单

### Requirement: Text Details 悬停完整文本
鼠标悬停在 Text Details 对照条单元格上且其文本被截断或包含换行时，视图 SHALL 显示该单元格完整文本的提示，保留原始换行；提示内容超过 4000 字符时 MUST 截断并以省略号结尾。文本完整可见且无换行时 MUST NOT 显示提示。

#### Scenario: 截断单元格悬停
- **WHEN** H 列宽 150 px、文本为 120 字符，鼠标悬停在对照条该格上
- **THEN** 显示包含完整 120 字符的提示

#### Scenario: 完整可见不提示
- **WHEN** 鼠标悬停在文本完整显示且无换行的单元格上
- **THEN** 不显示提示
