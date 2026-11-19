# Spec Delta

## ADDED Requirements

### Requirement: 输入窗格行内差异高亮
左、右输入窗格中，与 base 有改动且两侧（本窗格与 base）在该对齐行上都有内容的行，系统 SHALL 逐字符比较本窗格与 base 的这一行，用所属侧的强调色（左侧改动色、右侧改动色；未解决冲突区段用冲突色）着墨不同的字符，相同的字符保持普通着色。仅在不重要元素（如默认规则下的注释）上的差异 SHALL 用不重要差异色着墨。base 窗格与输出窗格 MUST NOT 做行内高亮。

#### Scenario: 左侧改了一个词
- **GIVEN** base 某行为 `let total = price * qty;`，left 同一对齐行为 `let total = cost * qty;`
- **WHEN** 打开合并视图
- **THEN** left 窗格该行只有 `cost` 以左侧改动色着墨，其余字符为普通色

#### Scenario: 冲突区段
- **GIVEN** 某区段两侧都改了同一行且内容不同
- **WHEN** 查看 left 与 right 窗格的该行
- **THEN** 两个窗格中与 base 不同的字符都以冲突色着墨

#### Scenario: 整行新增
- **WHEN** left 在某处新增一行，base 在该对齐行上是空隙
- **THEN** 该行不做逐字符高亮，仍按左侧改动整行着色

### Requirement: 合并窗格语法着色
四个窗格 SHALL 按文件格式库为该文件识别出的语法着色，元素风格与文本比较视图一致。视图 SHALL 提供 View › Syntax Highlighting 开关，默认开启；关闭时四个窗格都只用普通文字色（差异与来源底色不受影响）。行内差异着墨 SHALL 优先于语法颜色。

#### Scenario: 源代码着色
- **WHEN** 合并三个 `.rs` 文件
- **THEN** 四个窗格中的关键字、字符串、注释按文本比较的同一风格着色

#### Scenario: 关闭语法着色
- **WHEN** 用户取消勾选 View › Syntax Highlighting
- **THEN** 四个窗格的文字都改为普通色，改动与冲突的底色保持不变

### Requirement: 合并视图查找
视图 SHALL 提供 Find…（Ctrl+F）、Find Next（F3）、Find Previous（Shift+F3），在活动窗格的内容中查找，选项与文本比较的查找条相同（字面 / 正则、区分大小写、全词、仅在改动中）。找到时 SHALL 选中匹配文本并把四个窗格滚动到其对齐行；到达首尾没有更多匹配时 SHALL 在状态栏提示且 MUST NOT 自动回绕。「仅在改动中」SHALL 只在改动或冲突区段的行中查找。

#### Scenario: 在右侧输入窗格查找
- **GIVEN** 活动窗格是 right
- **WHEN** 用户按 Ctrl+F 输入 `timeout` 回车
- **THEN** right 窗格中光标之后第一个 `timeout` 被选中，四个窗格滚动到该行

#### Scenario: 没有更多匹配
- **WHEN** 当前匹配已是活动窗格中最后一个，用户再按 F3
- **THEN** 选区不变，状态栏提示未找到更多匹配

### Requirement: 输出窗格替换
视图 SHALL 提供 Replace…（Ctrl+H），仅在活动窗格为输出窗格时可用；活动窗格为输入窗格时该命令 MUST 禁用。Replace 替换当前匹配并查找下一个，Replace All 替换输出窗格中全部匹配。替换 SHALL 作为可撤销编辑，被替换的行 SHALL 标为手工并以手工色显示；被替换波及的未解决冲突区段按「输出窗格编辑」规则视为已解决。

#### Scenario: 全部替换
- **GIVEN** 输出窗格中有 3 处 `foo`
- **WHEN** 用户在输出窗格执行 Replace All，把 `foo` 替换为 `bar`
- **THEN** 3 处都变为 `bar`，所在行以手工色显示；一次撤销恢复全部 3 处

#### Scenario: 输入窗格不可替换
- **WHEN** 活动窗格是 left
- **THEN** Replace… 菜单项禁用，查找条不显示替换行

### Requirement: 横向滚动条
任一窗格中可见内容的宽度超过其窗格宽度时，视图 SHALL 在窗格区底部显示一条横向滚动条，四个窗格共用同一横向偏移；拖动滑块、点击轨道或在其上滚动滚轮 SHALL 同步横移四个窗格。所有内容都放得下时 SHALL 隐藏该滚动条并把横向偏移归零。

#### Scenario: 长行
- **GIVEN** left 中有一行宽于其窗格
- **WHEN** 用户把横向滚动条拖到最右
- **THEN** 四个窗格同时横移，该长行的末尾在 left 窗格中可见

#### Scenario: 内容变窄
- **WHEN** 用户放宽窗口直到所有行都能完整显示
- **THEN** 横向滚动条消失，四个窗格的横向偏移回到 0

### Requirement: 输入窗格选择与复制
三个输入窗格 SHALL 支持单击放置光标、拖选、Shift+单击扩展选区与 Ctrl+A 全选本窗格内容，选区以文本比较相同的选区色显示；Ctrl+C SHALL 把选中文本（保留原行尾）写入系统剪贴板。输入窗格 MUST 保持只读：键入、粘贴、删除都不改变内容。输入窗格 SHALL 提供右键菜单：Take Left、Take Center、Take Right（作用于所点行所在区段）、Copy（无选区时禁用）、Select All。

#### Scenario: 从 base 复制两行
- **WHEN** 用户在 base 窗格拖选两行并按 Ctrl+C
- **THEN** 剪贴板内容为这两行文本，base 内容不变

#### Scenario: 输入窗格键入无效
- **GIVEN** 光标在 right 窗格
- **WHEN** 用户键入字符或按 Delete
- **THEN** right 内容不变，选区保持

#### Scenario: 输入窗格右键 Take
- **WHEN** 用户在 left 窗格某冲突区段的行上右键选择 Take Left
- **THEN** 输出窗格该区段替换为 left 的内容，未解决冲突数减一
