# Spec Delta

## Purpose

定义表格行如何配对：以键列的值为标识，只对键匹配的行配对，支持排序后对齐与参考工具公开的对齐策略，未匹配行为孤儿。

## ADDED Requirements

### Requirement: 仅按键配对
系统 SHALL 只把键值相等（按键列 Handling 的大小写/空白/容差规则比较）的行放在同一行；键值不同的行 MUST 不配对，各自成为"仅左侧"或"仅右侧"行。

#### Scenario: 键匹配
- **WHEN** 左 `(7, apple, 1.0)` 与右 `(7, apple, 1.2)`，键为第 1 列
- **THEN** 两行配对，第 3 列标为差异

#### Scenario: 键不匹配
- **WHEN** 右侧没有键为 8 的行
- **THEN** 左侧键 8 的行为"仅左侧"，右侧对应位置为空隙

### Requirement: 重复键
键值重复的行 SHALL 按出现顺序依次配对，多出的为孤儿；状态区提示重复键数量。

#### Scenario: 左侧两行同键
- **WHEN** 左侧有两行键为 3，右侧一行键为 3
- **THEN** 第一对配对，左侧第二行为"仅左侧"，提示重复键 1 处

### Requirement: 排序后对齐
"排序后对齐"开启时系统 SHALL 先按键列（多键按优先级）稳定排序两侧再配对，显示顺序为排序后顺序；关闭时按文件原顺序对齐。

#### Scenario: 乱序文件
- **WHEN** 两侧行顺序不同但键集合相同且开启排序
- **THEN** 所有行配对为相同，无孤儿

### Requirement: 对齐策略
系统 SHALL 提供 Standard（默认，可开 closeness matching）、Myers O(ND)、Patience、Unaligned 四种策略，语义与文本比较一致：策略作用于键序列；Unaligned 为第 n 行对第 n 行。

#### Scenario: Unaligned
- **WHEN** 选择 Unaligned
- **THEN** 第 i 行与第 i 行配对，键是否相同只影响单元格着色

### Requirement: 大表性能
系统 SHALL 在 100 万行 CSV 上完成键对齐而界面可响应，对齐在后台可取消。

#### Scenario: 百万行
- **WHEN** 两侧各 100 万行、10 列
- **THEN** 对齐在后台完成，期间可滚动已加载内容，可取消
