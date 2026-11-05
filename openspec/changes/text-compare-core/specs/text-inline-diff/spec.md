# Spec Delta

## Purpose

定义已配对的"已修改"行内部如何标出具体不同的字符区间，使用户一眼看出一行里改了什么，并与重要性分级配合着色。

## ADDED Requirements

### Requirement: 行内差异区间
对于每一对标记为"已修改"的行，系统 SHALL 分别给出左侧与右侧的差异字符区间列表；区间之外的字符在两侧对应相同。

#### Scenario: 单词替换
- **WHEN** 左侧 `return count + 1;` 右侧 `return total + 1;`
- **THEN** 左侧高亮 `count`，右侧高亮 `total`，其余字符不高亮

#### Scenario: 纯插入
- **WHEN** 右侧比左侧多出一段 `, verbose=True`
- **THEN** 右侧高亮该段，左侧在对应位置无高亮区间

### Requirement: 相邻碎片合并
当两个差异区间之间仅隔少量相同字符（默认 3 个以内）时，系统 SHALL 合并为一个区间，避免"斑马纹"式高亮。

#### Scenario: 隔一个字符的两处改动
- **WHEN** 左侧 `a1b2c` 右侧 `x1y2c`
- **THEN** 高亮 `a1b` 与 `x1y` 各为一个区间，而不是四个单字符区间

### Requirement: 优先词边界
当一处差异完全落在单个词（字母数字与下划线构成）内部时，系统 SHALL 把该区间扩展到整个词；由相邻碎片合并产生的区间 MUST NOT 再向同一词的未变前后缀扩展（因此 `a1b2c`→`x1y2c` 仍高亮 `a1b`/`x1y`）。

#### Scenario: 词内一字符不同
- **WHEN** 左侧 `getValue` 右侧 `setValue`
- **THEN** 高亮整个 `getValue` / `setValue`，而非仅 `g` / `s`

### Requirement: 区间带重要性
每个行内区间 SHALL 携带其重要性（重要/不重要），由所覆盖的语法元素与会话规则决定，视图据此用不同颜色绘制。

#### Scenario: 注释内改动
- **WHEN** 行内差异完全位于 Comment 元素内且 Comment 不重要
- **THEN** 该区间以不重要颜色高亮

### Requirement: 超长行可处理
系统 SHALL 对长度超过阈值（默认 10 000 字符）的行退化为整行高亮而不是逐字符计算，并保证单行计算时间有上界。

#### Scenario: 压缩 JS 单行
- **WHEN** 两侧各有一行 500 KB 的压缩脚本且不同
- **THEN** 行内高亮覆盖整行，视图不出现可感知卡顿
