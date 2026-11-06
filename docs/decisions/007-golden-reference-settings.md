# ADR 007：同设置黄金生成与重要性澄清

## 普通比较规则

经协调者批准，仅定向只读检查 TTextRules 的普通发布属性、元素标志元数据与 TTextFormat 行长属性；未分析授权逻辑、未访问密钥文件、未写安装目录。参考版本 5.0.6.30713，下面为原文件偏移而非运行时地址。

| 项目元数据 | 参考属性 | 属性记录偏移 | 写入规则 |
|---|---|---|---|
| case_unimportant | CaseSig | 0xf8c912 | 取反 |
| compare_endings | CompareEndings | 0xf8c944 | 原值 |
| 元素重要性 | ElementSigs | 0xf8c97d | `_N/ElementStr` 名称 + `ValueBool` 布尔 |
| embedded_unimportant | EmbeddedWSSig | 0xf8c9b3 | 取反 |
| leading_unimportant | LeadWSSig | 0xf8ca1c | 取反 |
| 默认未分类文本重要 | NonWSSig | 0xf8ca50 | True |
| 插删可由元素降级 | OrphansAlwaysSig | 0xf8ca83 | False |
| trailing_unimportant | TrailWSSig | 0xf8caf5 | 取反 |

ElementSigs 不是直接的 Comment 子节点或布尔数组。已用无空白的注释删除做正反探针：`<_0><ElementStr Value="Comment"/><ValueBool Value="True"/></_0>` 产生 leftorphan，而 False 产生 leftsimilar。每个模板显式写全部 10 类元素，除指定的 Comment 不重要外均为 True。

`build_rule_templates.ps1` 从 case.properties 确定性生成 30 个 profiles/*.rules.xml。正式 gen_expected 默认调用它，再注入已证实的 AlignKind、NeverAlignDiffs=False、SkewTolerance=2000、UseCloseness 设置。Myers 和 Standard 每例使用同一重要性模板；不继承未核实的默认配置。

## 三态澄清（已获批准，规格由协调者更新）

- 格式声明元素不区分大小写时，字节不同但内在 case 归一化相等应为 **Unimportant**，而不是 Same；仍不能被全局“大小写重要”升级为 Important。行内区间也保持该不重要性。对齐哈希仍可折叠格式大小写。
- 忽略行尾时，EOF 无换行与有换行也归 **Same**；开启行尾比较时仍为 Important。逐行行尾与 EOF 状态继续完整保存，不因比较忽略而改写文件。
- 两项既有断言经协调者授权同步纠正，没有只在黄金测试中特殊归一化或删掉失败用例。

## 超长行的同输入预处理

公开帮助 formattextconversion 指出 characters-per-line limit 会人工断行。TTextFormat.CharPerLine 的发布记录在 0xf808e4，默认 4096；它使 12011 字符的用例被参考工具读成三行，而项目保留一条物理行。

生成器仅为专用 `.matcha-golden-long` 扩展名临时添加格式，将 CharPerLine 设为 65536，放在用户格式列表前部防止宽泛掩码抢先匹配。模板输入字节与内容未改变，未在解析器中合并参考行，也未手工改报告。实际正式 XML 中该用例恢复为一行。新格式名称使用字面值，不加 STX；带 STX 的新名称在本机未生效，这一额外兼容性发现需后续核实 bc-formats 的新建名称策略，本波没有越权修改该 crate。

## 共享工具与恢复

每次调用参考工具或读取/写入本次设置快照都在 `%TEMP%/matcha-reference-tool.lock` 独占 CreateNew 租约内；锁记录 worker id、PID、UTC 时间。争用最多等待 600 秒，不擅自删除其他持有者的锁；finally 释放自己的锁。持锁后检查没有其他参考进程。

BCSessions.xml 与（如需长行格式）BCFileFormats.xml 都先备份；仅临时修改两个命名会话及专用格式；全部子进程退出后恢复原字节，原本不存在的格式文件则删除临时文件，并在持锁时验证恢复。私有备份继续被 gitignore，不入库。

证据：[生成器](../../tools/golden/README.md)、[锁测试](../../tools/golden/test_reference_lock.ps1)、[60 份原始期望](../../crates/bc-core/tests/golden/text/2way)。
