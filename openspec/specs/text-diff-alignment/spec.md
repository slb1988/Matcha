# text-diff-alignment Specification

## Purpose
定义两份文本之间行级差异的计算与对齐行为：哪些行被视为相同、哪些被配对为"已修改"、哪些是插入/删除，以及用户如何在差异间导航和手动修正对齐。这是文本比较视图、文件夹规则比较与后续 3 路合并共同依赖的行为契约。

## Requirements

### Requirement: 行级差异计算
系统 SHALL 以"行"为基本单位比较左右两份文本，输出一组有序的对齐行：每行要么左右都有内容（相同或已修改），要么只有一侧有内容（插入或删除）。所有左侧行与右侧行 MUST 各自恰好出现一次且保持原始顺序。

#### Scenario: 完全相同的文件
- **WHEN** 左右两份文本逐行相同
- **THEN** 输出的每一行都标记为"相同"，不存在插入、删除或已修改行

#### Scenario: 中间插入若干行
- **WHEN** 右侧文本等于左侧文本在第 10 行后插入 3 行
- **THEN** 前 10 行与其后的原有行均配对为"相同"，新增的 3 行标记为"仅右侧"，左侧对应位置显示为空隙

#### Scenario: 顺序保持
- **WHEN** 任意两份文本完成对齐
- **THEN** 按输出顺序读取左侧行索引得到严格递增序列，右侧同理

### Requirement: 对齐策略可选
系统 SHALL 以参考工具的公开名称提供对齐策略：Standard（默认，可勾选"closeness matching"把相似的改动行配对）、Myers O(ND)（纯最长公共子序列）、Patience，以及"不对齐"（第 n 行对第 n 行）模式，并允许每个比较会话独立选择。内部实现名 Smart/Quick/Patience 与之一一对应，不向用户暴露。

#### Scenario: 默认使用 Standard
- **WHEN** 用户未更改对齐设置打开比较
- **THEN** 采用 Standard 策略且 closeness matching 开启

#### Scenario: 切换策略后重新对齐
- **WHEN** 用户把策略从 Standard 切为 Myers O(ND)
- **THEN** 系统重新计算对齐并刷新视图，文件内容不变

#### Scenario: 关闭 closeness matching
- **WHEN** 用户在 Standard 下取消 closeness matching
- **THEN** 差异区块内不再做相似行配对，改动显示为删除加插入

#### Scenario: 不对齐模式
- **WHEN** 用户选择"不对齐"
- **THEN** 第 i 行左侧与第 i 行右侧配对，较长一侧多出的行显示为插入

### Requirement: Standard 对齐把相似的改动行配对
在 Standard 策略开启 closeness matching 时，一个差异区块内若左侧某行与右侧某行高度相似（仅少量字符不同），系统 SHALL 将它们配对为同一输出行并标记为"已修改"，而不是分别标记为删除与插入。

#### Scenario: 单字符改动
- **WHEN** 左侧为 `total = price * qty` 而右侧为 `total = price * quantity`，其余行相同
- **THEN** 这两行出现在同一输出行并标记为"已修改"

#### Scenario: 不相似的替换
- **WHEN** 一个差异区块内左侧 3 行与右侧 3 行没有任何相似之处
- **THEN** 系统 MUST 不强制逐行配对，可以输出 3 行删除后接 3 行插入

### Requirement: 大文件可完成对齐
系统 SHALL 在 100 MB 级文本（约 200 万行）上完成对齐而不耗尽内存，并在对齐进行中保持界面可响应。

#### Scenario: 两个 100 MB 文件
- **WHEN** 用户打开两个各约 100 MB 的文本文件
- **THEN** 对齐在后台执行，界面可滚动查看已加载内容，完成后视图刷新为对齐结果，进程私有内存增长不超过输入总大小的 2 倍

### Requirement: 差异导航
系统 SHALL 支持从当前位置跳到上一个/下一个差异区块（连续的非"相同"行构成一个区块），到达首尾时按 `difference-navigation-feedback` 的规则处理：默认提示 "Difference not found." 并保持位置；开启回绕时回到另一端的差异区块。

#### Scenario: 下一差异
- **WHEN** 光标位于第 1 个差异区块且用户执行"下一差异"
- **THEN** 视图滚动到第 2 个差异区块的首行并使其可见

#### Scenario: 已在最后一个差异
- **WHEN** 光标位于最后一个差异区块且用户执行"下一差异"，未开启回绕
- **THEN** 位置不变，状态栏上方弹出 "Difference not found." 消息面板并播放提示音

#### Scenario: 开启回绕后越过最后一个差异
- **WHEN** 已开启 Wrap around to first difference，光标位于最后一个差异区块且用户执行"下一差异"
- **THEN** 视图定位到第 1 个差异区块的首行

### Requirement: 手动对齐覆盖（Align With）
系统 SHALL 允许用户选中左侧一行再选中右侧一行，强制二者出现在同一输出行；覆盖在重新比较后仍然生效，直到用户清除。

#### Scenario: 强制配对
- **WHEN** 用户对左侧第 40 行与右侧第 55 行执行 Align With
- **THEN** 这两行出现在同一输出行，其前后内容分别重新对齐

#### Scenario: 清除覆盖
- **WHEN** 用户清除全部手动对齐
- **THEN** 对齐结果恢复为自动策略的输出

### Requirement: 黄金语料一致性
对于项目黄金语料库中的每个 2 路用例，系统的对齐输出 SHALL 与参考工具在相同策略（Myers O(ND) 对应内部 Quick，Standard+closeness 对应内部 Smart）下产出的期望对齐一致。

#### Scenario: 语料回归
- **WHEN** 执行黄金语料回归测试
- **THEN** 每个用例的输出行序列（左索引、右索引、类别）与期望文件逐行相等
