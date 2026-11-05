# ADR 002：未知多行语法项（待调查）

- 状态：决策占位；待获得可验证输入后修订。
- 背景：`TLinesTextGrammarItem` 存在于参考类模型，但 25 种内置格式均未使用；不能推断为普通 Basic 或 Delimited。
- 当前决定：拒绝未知类型并报告格式名与类型名，不丢弃或猜测匹配行为；本轮不实现此类。
- 验证：后续格式导入测试覆盖未知语法类报错；新增支持须有多行示例与预期元素区间。
- 关联：[格式库规格](../../openspec/changes/text-compare-core/specs/file-format-library/spec.md)、[bc-formats](../../crates/bc-formats/README.md)。
