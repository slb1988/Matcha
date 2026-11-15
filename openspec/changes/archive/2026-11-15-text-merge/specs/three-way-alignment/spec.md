# Spec Delta

## Purpose

定义三个文本序列以 center（base）为轴的三方对齐行为：对齐结果的结构与不变量、孤儿行的摆放规则、与双方比较在策略和重要性规则上的一致性，以及重对齐的等价性。

## ADDED Requirements

### Requirement: 以 center 为轴的三方对齐
系统 SHALL 把 left 与 center、center 与 right 分别做双方对齐，再以 center 行为轴调和为统一的三列行表。每行 SHALL 由 `(left?, center?, right?)` 三个可空行号组成，至少一个非空。结果 MUST 满足：每一侧的每个行号恰好出现一次且自上而下递增；left 与 center 的配对集合等于两者单独做双方对齐的配对集合，center 与 right 同理。

#### Scenario: 仅一侧插入
- **WHEN** left 相对 center 在第 10 行后插入 3 行，right 与 center 相同
- **THEN** 对齐结果在 center 第 10 行所在行之后出现 3 个只有 left 行号的行，这 3 行的 center 与 right 为空隙

#### Scenario: 一侧与 center 完全相同
- **WHEN** left 与 center 内容完全相同
- **THEN** 每个 center 行都与同号的 left 行同行，center 与 right 的配对和空隙与两者单独做双方比较的结果相同

#### Scenario: 一侧修改某行
- **WHEN** left 与 center 相同，right 把 center 第 5 行改成了不同内容，且 center 与 right 的双方对齐把这两行配成同一行
- **THEN** 三方行表中 left 第 5 行、center 第 5 行与被修改的 right 行在同一行

### Requirement: 未配对行的摆放
未与任何 center 行配对的 left 行或 right 行（孤儿行）SHALL 放在其前后两个 center 行之间，不得越过相邻的 center 行。落在同一区间的 left 孤儿行与 right 孤儿行 SHALL 自上而下按位置并排在同一行，多出的行单独成行。

#### Scenario: 两侧在同一位置插入
- **WHEN** left 在 center 第 10 行后插入 3 行，right 在同一位置插入 1 行
- **THEN** 该区间共 3 行：第 1 行同时含 left 与 right 的插入行，第 2、3 行只含 left 的插入行；三行的 center 均为空隙

### Requirement: 对齐策略与双方比较一致
三方对齐 SHALL 使用与双方比较相同的对齐设置（Standard / Myers / Patience / Unaligned、closeness、skew、never align differences、行权重）与比较规则；忽略大小写、空白等影响行哈希的规则 MUST 同样作用于三份输入。

#### Scenario: 忽略行尾空白时的对齐
- **WHEN** 规则设置行尾空白不重要，left 某行仅行尾空白与 center 不同
- **THEN** 这两行配对在同一行，不产生孤儿行

#### Scenario: Unaligned
- **WHEN** 对齐策略为 Unaligned
- **THEN** 第 n 行由三份输入各自的第 n 行组成，较短的一侧在超出部分为空隙

### Requirement: 重对齐的等价性
对同一组输入与规则，重对齐作业的结果 MUST 与全量三方对齐的结果完全相同；实现可以只重算局部，但不得因此产生不同的行表。对齐被取消时 MUST 不发布部分结果。

#### Scenario: 重对齐与全量一致
- **WHEN** 对同一组输入先执行全量三方对齐，再执行重对齐
- **THEN** 两次得到的行表逐行相同

#### Scenario: 取消
- **WHEN** 三方对齐执行过程中收到取消
- **THEN** 作业以取消结束，会话中原有的行表保持不变
