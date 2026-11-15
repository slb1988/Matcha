# Spec Delta

## Purpose

定义表格合并引擎的行为：三个表格（left / right / center）按键列的三方行对齐、逐（行, 列）的改动分类与三类冲突判定、默认输出内容、冲突解决、自动合并结果，以及合并结果的写出（CSV 保真写回、xlsx 新写、多 sheet 并集）。

## ADDED Requirements

### Requirement: 键的来源与退化
合并 SHALL 以键列标识行的身份：键可由调用方显式指定（1-based 比较列号，可多个，按优先级排序）；未指定时 SHALL 按既有键推断得出；推断不出可用键时 MUST 退化为全部比较列作键（整行身份），并照常完成合并。显式指定的键列越界或不存在时 MUST 报错。合并全程 SHALL 向调用方暴露实际使用的键列。

#### Scenario: 显式键
- **WHEN** 调用方指定第 1 列为键
- **THEN** 行身份由第 1 列值决定，其余列的差异按单元格分类

#### Scenario: 无键退化
- **WHEN** 未指定键且推断失败
- **THEN** 整行内容作键，任一单元格不同的行不再对齐为同一实体，合并退化为行增删语义

### Requirement: 三方键对齐
系统 SHALL 以 center 为轴对三方按键对齐：每个 center 行产生一行对齐结果，带上与之键相同的 left、right 行（可为空隙）；未与 center 配对的 left 行与 right 行 SHALL 按键互相配对（同键成一行），其余单独成行。键值在任一侧重复时 MUST 报告重复计数，重复组内按出现顺序配对。配对 MUST 只依赖键列与规则，不受行的物理顺序影响。对齐结果的行序 SHALL 为 center 的物理行序；center 没有的行排在其所在一侧前一个已配对行之后（单侧调换行序不合并）。列 SHALL 按位置对齐，列数取三方最大值，较窄一侧缺少的格按空单元格处理。

#### Scenario: 三方同键对齐
- **WHEN** left、center、right 各有键为 7 的行，物理位置不同
- **THEN** 三行对齐到同一行

#### Scenario: 仅一侧新增的行
- **WHEN** left 有一行键为 9，center 与 right 都没有
- **THEN** 该行对齐为（left=9, center=空隙, right=空隙），位于 left 中它前一行所对应的 center 行之后

#### Scenario: 两侧新增同键的行
- **WHEN** left 与 right 都新增键为 9 的行，物理位置不同
- **THEN** 两行对齐为同一行（left=9, center=空隙, right=9）

#### Scenario: 一侧追加一列
- **WHEN** left 在末尾追加一列并填了值，center 与 right 没有该列
- **THEN** 该列各格类型为左改，输出包含该列

### Requirement: 单元格类型分类
对齐完成后系统 SHALL 为每个（对齐行, 比较列）确定类型：两侧都与 center 相同为未变；仅 left 不同为左改；仅 right 不同为右改；两侧都不同且彼此相同为同改；两侧都不同且彼此不同为异改。相同性 SHALL 按表格规则判定（大小写、空白、数值容差、日期容差、按公式比较等，与双方比较一致），并同时比较值与公式文本（只改公式也算改动）；数字格式不参与。一侧为空隙时该格按行级结构判定处理，不参与单元格分类。

#### Scenario: 两侧改同一行的不同列
- **WHEN** 某行 left 只改了数量列、right 只改了价格列
- **THEN** 数量格类型为左改、价格格类型为右改，整行无冲突

#### Scenario: 容差内的差异不算改动
- **WHEN** 价格列规则容差为 0.01，left 与 center 的价格相差 0.005，right 未动
- **THEN** 该格类型为未变

### Requirement: 行级结构判定
对齐行的结构 SHALL 判定为：仅 left 有的行为左增；仅 right 有的行为右增；left 与 right 都有而 center 没有且内容相同为同增，内容不同为双增冲突；center 有而仅一侧缺失（删除）、另一侧与 center 相同为删除；center 有、一侧删除、另一侧有实质改动为删改冲突。双增冲突的"内容不同" SHALL 指任一比较列分类为异改。

#### Scenario: 单侧删除且对侧未动
- **WHEN** center 有键 7 的行，left 删除了它，right 与 center 相同
- **THEN** 结构判定为删除，输出不含该行，不构成冲突

#### Scenario: 删除对上修改
- **WHEN** center 有键 7 的行，left 删除了它，right 修改了它的价格
- **THEN** 结构判定为删改冲突

#### Scenario: 两侧新增同键不同内容
- **WHEN** left 与 right 都新增键为 9 的行，但名称列不同
- **THEN** 结构判定为双增冲突

### Requirement: 默认输出内容
未经用户决定时，输出 SHALL 逐格取：未变取 center，左改取 left，右改取 right，同改取 left，异改取 center；左增取 left 行，右增取 right 行，同增取 left 行，删除不输出该行；未解决的删改冲突 SHALL 保留 center 行，未解决的双增冲突 SHALL 不输出任一新行。

#### Scenario: 无冲突的合并结果
- **WHEN** 所有改动均为单侧改动或同改
- **THEN** 输出包含两侧的全部改动，无冲突

#### Scenario: 异改格保留 base
- **WHEN** 某格 left 改为 12、right 改为 15、center 为 10，冲突未解决
- **THEN** 默认输出该格为 10，且该格计为未解决冲突

### Requirement: 冲突区段
相邻的含冲突对齐行 SHALL 聚合为冲突区段，用于导航与整体解决；区段内每行可含多个冲突格与结构冲突。区段 SHALL 带重要性：段内全部差异按规则都不重要时为不重要，否则为重要。

#### Scenario: 相邻冲突行成段
- **WHEN** 第 5、6 行各有异改格，第 7 行起无冲突
- **THEN** 第 5–6 行构成一个冲突区段

### Requirement: 冲突解决
系统 SHALL 支持把冲突解决为取 left、取 right 或取 center：作用于单个冲突格时只替换该格；作用于冲突行或区段时替换其全部未解决冲突格（结构冲突按所取侧决定行去留与内容）；解决后不再计入未解决冲突。系统 SHALL 支持在不改变输出内容的前提下把冲突标为已解决（接受当前内容）。解决操作 MUST 只影响目标格/行/区段。

#### Scenario: 区段取右
- **WHEN** 对一个含删改冲突与两个异改格的区段选择取 right
- **THEN** 该区段输出 right 的行与格内容，未解决冲突数相应减少

### Requirement: 忽略不重要差异与 favor
开启忽略不重要差异时，不重要的差异 SHALL 按相同处理，不构成改动与冲突。仅由不重要差异构成的冲突区段不计入冲突；此时若 favor 为左或右，该区段 MUST 改取 favored 侧的内容，否则取 center。未开启忽略不重要差异时，favor SHALL 不改变输出内容。列的重要性 SHALL 来自其列处理规则。

#### Scenario: 不重要列的两侧改动不冲突
- **WHEN** 备注列标为不重要且开启忽略，left 与 right 对该列改了不同内容，其余列三方相同
- **THEN** 无冲突，favor 未设置时输出该列取 center

### Requirement: 自动合并结果
自动合并 SHALL 为不依赖界面的纯函数，返回未解决冲突列表与输出模型。没有未解决冲突时输出为合并结果。存在未解决冲突时 MUST 扣留输出并标明被扣留；表格合并不提供冲突标记形式的有损写出。

#### Scenario: 有冲突扣留输出
- **WHEN** 存在未解决冲突
- **THEN** 结果列出冲突区段，输出被扣留

#### Scenario: 三份输入相同
- **WHEN** 三方内容按规则完全相同
- **THEN** 没有冲突，输出与 center 相同

### Requirement: 多 sheet 合并
工作簿合并 SHALL 按 sheet 名配对三方 sheet，逐个独立合并；仅一侧新增的 sheet SHALL 整体取该侧；一侧删除而另一侧未变的 sheet SHALL 不输出；一侧删除而另一侧有改动的 sheet SHALL 视为删改冲突，未解决时保留 center 版本。输出的 sheet 集合 SHALL 为按上述规则取舍后的并集，顺序以 center 为准，新增 sheet 依次追加。

#### Scenario: 一侧新增 sheet
- **WHEN** right 新增一个 center 没有的 sheet
- **THEN** 输出包含该 sheet 的 right 版本

### Requirement: 合并结果的写出
CSV/TSV 输出 SHALL 走逐记录保真写回（未改字段、分隔符、行尾原样），格式（分隔符、引号、编码、BOM）取自 center。xlsx 输出 SHALL 以 center 工作簿为模板写出（2026-11-14 按用户要求修订，见 design D3 二次修订）：只重写单元格数据，样式、列宽、行高、合并单元格、冻结窗格、条件格式、数据验证、图表、图片、批注、定义名称与宏工程 SHALL 原样保留，每格沿用其来源侧的样式；行增删后公式、合并单元格、定义名称、表格与图表系列的行引用 SHALL 随行重定位（被删行的单格引用为 `#REF!`）；模板写出失败时 SHALL 退回新写工作簿（保留值、公式、数字格式与 sheet）并给出未保留样式的警告；输出为 .xlsx 而 center 含宏工程时 MUST 去掉宏并警告。写出 SHALL 先写同目录临时文件再替换目标；输出路径与某个输入相同时 MUST 仍能正确写出；输出文件的既有内容 SHALL 不参与合并。输入文件本身 MUST 不被修改，除非输出路径显式指向它。

#### Scenario: 输出覆盖 right
- **WHEN** 输出目标等于 right 的源路径且无冲突
- **THEN** 该文件被替换为合并结果，其余输入不变

#### Scenario: xlsx 写出保留样式并重定位行引用
- **WHEN** 合并 xlsx，left 在合计行之上插入一行，center 有粗体表头、列宽与图表
- **THEN** 结果文件保留表头样式、列宽与图表，合计行公式的范围随插入行扩展

#### Scenario: 文本表格表头一致
- **WHEN** center 判定有表头，left 的首行恰好不像表头
- **THEN** left 仍按有表头读取，三方行号不错位
