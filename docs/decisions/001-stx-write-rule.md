# ADR 001：STX 写出规则（待验证）

- 状态：决策占位；任务 5.3 验证后补充结论。
- 背景：`ElementStr`、名称与描述常含 `&#2;`，`Text`、`Mask`、`FindText`、`StopText` 通常不含；不能把它推广为所有字符串的规则。
- 当前决定：读取时保留 `Node::stx`；写回已有节点按该标志恢复，不凭属性名猜测。新建节点的规则待写兼容测试确认。
- 验证：内置 XML 读→写→读模型等价，记录空白/属性顺序之外的差异；不宣称字节级兼容。
- 关联：[设计 D5 之后的迁移/非目标说明](../../openspec/changes/text-compare-core/design.md)、[bc-core](../../crates/bc-core/README.md)、[bc-formats](../../crates/bc-formats/README.md)。
