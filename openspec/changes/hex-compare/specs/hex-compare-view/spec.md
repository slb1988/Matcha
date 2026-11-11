# Spec Delta

## Purpose

定义 Hex Compare 会话的用户可感知行为：双栏十六进制视图的渲染模型、导航、过滤、查找、编辑、保存、会话设置与入口路由。渲染与统计数据来自 hex-byte-alignment 的对齐契约。

## ADDED Requirements

### Requirement: 双栏单元格行打包渲染
视图 SHALL 把字节对齐图按每行 N 个单元格（默认 16，可配置）打包成行：配对的两个字节占用两侧同一列；一侧的孤儿字节占格时，另一侧同列 MUST 画斜纹占位格（可位于行中间）。每侧行标签显示该侧本行**首个真实字节**的文件偏移（十六进制）；某行该侧全为占位格时标签留空（整行斜纹）。每个字节格同时给出 ASCII 预览列，不可打印字符显示为 `.`。

#### Scenario: 整行孤儿（16 字节插入）
- **WHEN** 右侧在 0x400 处插入恰好 16 字节
- **THEN** 右侧出现一行 16 个红色孤儿字节，左侧对位为一整行斜纹占位格且无地址标签；随后左侧行标签 0x400 与右侧行标签 0x410 配对同一内容

#### Scenario: 行中孤儿（5 字节插入）
- **WHEN** 右侧在 0x123 处插入 5 字节
- **THEN** 右侧该行的 5 个孤儿字节以红色显示在行中间，左侧同行同列为 5 个斜纹占位格；该插入点之后两侧行各自继续按每行 16 格打包，行标签相位随之偏移

#### Scenario: 行标签规则
- **WHEN** 任意对齐结果完成渲染
- **THEN** 每侧行标签严格等于该侧该行第一个真实字节的文件偏移；行内占位格不产生地址

### Requirement: 差异着色与状态栏
差异字节与孤儿字节 SHALL 以重要差异色（红底红字）着色，ASCII 列同步着色；相同字节不着色。状态栏 SHALL 显示整体结论（Binary differences / Binary same）、插入模式指示（Insert/Overwrite）与加载耗时；每侧窗格 SHALL 显示当前光标的字节地址，且两侧光标定位到同一逻辑字节。

#### Scenario: 差异着色
- **WHEN** 对齐结果包含 difference 或 orphan 字节
- **THEN** 这些字节在 hex 区与 ASCII 区均以红色系着色，状态栏显示 Binary differences

#### Scenario: 光标跨侧同步
- **WHEN** 用户把光标定位到左侧偏移 0x123
- **THEN** 右侧光标落在与左 0x123 配对的位置（如存在 5 字节插入则为右 0x128），两侧底部地址分别显示各自偏移

### Requirement: 视图布局与显示开关
视图 SHALL 支持 Side-by-Side（默认）与 Over-Under 两种窗格布局，并提供缩略图条（全文件差异概览）、File Info 头（时间戳与字节数、格式下拉）、工具栏、字节地址列的独立显示开关。工具栏 SHALL 提供 All/Diffs/Same 切换、Rules、Format、Copy、Next/Prev Diff、Swap、Reload 入口。

#### Scenario: 切换 Over-Under
- **WHEN** 用户选择 Over-Under Layout
- **THEN** 两窗格上下排列，渲染与对齐内容不变

#### Scenario: 关闭缩略图
- **WHEN** 用户关闭 Thumbnail
- **THEN** 缩略图条隐藏，其余区域布局自适应填充

### Requirement: 显示过滤与上下文
视图 SHALL 提供 Show All（默认）/ Show Differences / Show Same 三种过滤。Show Differences 模式下，每个差异区域 SHALL 附带可配置数量的上下文相同字节（默认 16，Layout 对话框的 Context count），各区域连同上下文按区域起点重新打包行。

#### Scenario: 只看差异
- **WHEN** 用户切到 Show Differences，差异分别位于 0x100 与 0x400
- **THEN** 视图仅显示各差异区域及其上下文字节，隐藏中间大段相同内容

### Requirement: 布局设置
Layout 对话框 SHALL 提供 Bytes per group（默认 8，hex 区分组显示）、Groups per item、Fit to window（默认开，自动确定每行单元格数）与 Context count（默认 16）。

#### Scenario: 关闭 Fit to window 后的固定行宽
- **WHEN** 用户关闭 Fit to window 并设置每行 8 字节
- **THEN** 每行按 8 格打包，行标签与相位规则不变

### Requirement: 差异导航与定位
视图 SHALL 提供 Next/Previous Difference Bytes（Ctrl+N / Ctrl+P）在差异字节间循环提示式导航，以及 Go To 对话框（字节地址，十进制或十六进制、可选侧）。导航目标滚入视野并定位光标。

#### Scenario: 差异间往返
- **WHEN** 光标位于首个差异之前，连续执行 Next Difference Bytes
- **THEN** 依次定位到每个差异字节；越过最后一个后给出末端提示且不回卷或按参考行为提示

#### Scenario: Go To 十六进制地址
- **WHEN** 用户在 Go To 中选择 Hex 并输入 `400`，侧选择 Right
- **THEN** 右侧光标定位到右 0x400，左侧同步到配对位置，目标滚入视野

### Requirement: 查找与替换
视图 SHALL 提供内联查找栏（Ctrl+F）：支持十六进制字节序列与文本两种模式、大小写开关、Prev/Next；Replace（Ctrl+H）在当前侧执行替换。查找不命中时状态栏或查找栏给出提示，不修改内容。

#### Scenario: 查找字节序列
- **WHEN** 用户在 hex 模式查找 `DE AD BE EF` 且内容中存在
- **THEN** 首个命中位置被选中并滚入视野，Find Next 定位下一处

### Requirement: 字节编辑与保存
视图 SHALL 支持直接编辑：Insert/Overwrite 两种模式（状态栏指示）、Cut/Copy/Paste/Delete、Select All（当前窗格全部可见字节）、Undo/Redo、Copy to Other Side（Ctrl+R，把选中对齐区段复制到对侧）。每侧可独立保存/另存（暂存+改名落盘）；标有 Disable editing 的侧 MUST 拒绝编辑与保存。Open Clipboard 把剪贴板内容载入指定侧。编辑、保存失败与只读冲突 MUST 给出明确提示且不破坏另一侧状态。

#### Scenario: Copy to Other Side 消除差异
- **WHEN** 用户选中左侧一段字节执行 Copy to Right
- **THEN** 右侧对应位置被覆盖为选中内容，重对齐后该段分类变为 same，且可一次 Undo 还原

#### Scenario: 禁编辑侧拒绝修改
- **WHEN** 某侧在 Session Settings 勾选 Disable editing
- **THEN** 该侧不接受键入/粘贴/删除/被复制覆盖，保存命令对该侧不可用

### Requirement: 会话设置三页
Session Settings 对话框 SHALL 含三页：Specs（左右文件路径与浏览、换侧、每侧 Disable editing、Description、作用域下拉至少含 "Use for this view only"）、Format（左右文件格式，默认 Detected (Everything Else)，可勾选 Same as left file format）、Comparison（对齐策略 Complete/Fast/None 与文件锁阈值，默认 2 MB）。

#### Scenario: 调整对齐策略生效
- **WHEN** 用户在 Comparison 页把 Complete 改为 None 并确认
- **THEN** 当前会话按新策略重新对齐；选择 "Use for this view only" 时不影响其它会话

### Requirement: 比较信息统计
Hex Compare Info 对话框 SHALL 显示四分类字节计数：same / left orphan / right orphan / difference byte(s)。

#### Scenario: 统计与对齐结果一致
- **WHEN** 对参考语料打开 Hex Compare Info
- **THEN** 显示 4120 same byte(s)、32 left orphan byte(s)、16 right orphan byte(s)、8 difference byte(s)

### Requirement: 加载、大文件与文件锁
视图 SHALL 整文件载入小于阈值（默认 2 MB，可配）的内容且不加文件锁（允许外部程序编辑）；超过阈值的内容经内存映射流式访问并持有文件锁。加载与对齐耗时分栏显示在状态栏。

#### Scenario: 小文件不加锁
- **WHEN** 打开 100 KB 文件
- **THEN** 该文件不被加写锁，外部程序可同时修改它；重载后按新内容重对齐

### Requirement: 入口与路由
系统 SHALL 提供 Hex Compare 的完整入口：Home 会话卡与新建会话菜单、按内置 hex 文件格式掩码（初版至少含 `*.bin`）的自动路由、命令行 `/fv="Hex Compare"` 强制会话类型、以及从 Text/Table Compare 经 Compare Files Using 的互切。未命中掩码的未知类型仍路由到 Text Compare。空会话 SHALL 允许先建视图再逐侧打开文件。

#### Scenario: 按掩码路由
- **WHEN** 用户在 Home 或命令行打开两个 `.bin` 文件
- **THEN** 打开 Hex Compare 会话

#### Scenario: 会话互切
- **WHEN** 用户在 Text Compare 中选择 Compare Files Using > Hex Compare
- **THEN** 同一对文件在原位或新标签页以 Hex Compare 重新打开
