# ADR 001：STX 写出规则与最小 XML 补丁

- 状态：Wave 2 已实现，任务 5.3 验证通过；新建节点规则是基于内置数据的兼容推断，不宣称涵盖所有外部资源。

## 原始节点

`persist::write_xml` 严格按 `Node::stx` 决定 Value 前是否写 `&#2;`，不根据属性名重新猜测。名称、未知节点与显式默认值保持。只转义属性中必需的 `&`、`<`、双引号，保留合法的单引号与 `>`，从而不把内置资源的字面 `>` 改写成 `&gt;`；属性里的空白按现有读取器的非归一化语义往返。

原 FileFormat 只有语义字段，无法保存被省略的默认值、未知属性节点和节点顺序。经协调者批准增加 `source_xml: Option<String>`（serde default / None 时省略），保存每个格式的规范化源 Node 树；PartialEq 只比较语义字段。写入时比较来源模型与当前模型，仅对改变的已知节点打补丁，不删除未知节点，不重写未改属性的 STX 标志或顺序。这里的“属性”指该持久化方言的子属性节点；通用 XML 的非 Value 属性仍不属于 Node 模型。

## 新建节点规范

| 节点/属性 | 新建时 STX |
|---|---|
| 顶层格式名称 Value | true |
| Description 的 `_N` 字符串 | true |
| Grammar 项的 ElementStr | true |
| Grammar 项类型 Value、Text、StopText、Items 词项 | false |
| Mask、FindText、Priority、ConvertKind | false |
| 布尔、列号、Tab、Escape、表格字符码 | false |

已读取节点以原标志为准，即使与上表不同。新建属性使用规范，既有属性修改值仍保留原标志。

## 5.3 实测差异

`crates/bc-formats/tests/xml_roundtrip.rs` 的内置 XML 读→写→读测试同时证明：

1. 25 个 FileFormat 的语义模型相等。
2. 全部 Node 树（节点顺序、Value、STX、未知子节点）相等。
3. 原文件和写出文件逐行剥除缩进/行尾空白后完全一致；没有属性顺序改变、实体拼写改变、默认值增删或其他文本内容差异。

实际差异仅为行尾 CRLF→LF（写出器使用 LF；缩进仍为 Tab）。测试不是只比较对象后忽略文本差异。新建无来源格式另有全部 25 格式往返测试；修改名称/语法/掩码/布尔值时保留未知节点与异常 STX 的补丁测试也通过。

内置 JSON 因来源元数据增加体积，准确字节增量记录在 Wave 2 报告；这是有意的写兼容成本。

## Wave 3b 追加观察（待后续专项修正）

为长行黄金临时创建用户格式时，名称带 `&#2;` 的新格式未被参考工具采用，改用纯字面名称后同一 Mask/CharPerLine 生效。此观察说明上表“新建格式名称 STX=true”对任意用户名称的推断不能直接作为原生用户格式写兼容保证；Wave 3b 生成器使用字面名称。bc-formats 的既有源码不在本轮修改权限中，尚未改变其规范化新建名称策略；需要独立检查资源标记与用户文字的区别。已读取节点按原标志往返的结论不受影响。

关联：[设计](../../openspec/changes/text-compare-core/design.md)、[bc-core](../../crates/bc-core/README.md)、[bc-formats](../../crates/bc-formats/README.md)。
