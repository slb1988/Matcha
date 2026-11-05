# ADR 004：最长词法匹配与固定列优先级

- 状态：Wave 2 经协调者明确批准的统一规则；这是对参考工具行为的推断，待任务组 7 黄金语料核实，不宣称已完成黑盒对照。
- 规则：同一位置所有 Basic / Delimited / List 项选择最长词法命中；等长才按语法顺序，先出现者优先。Delimited 在选取时比较起始符匹配长度，选中后再消费闭合/行末跨度。List 项内部同样最长命中。
- 动机：内置 PYTHON 的单引号项排在三引号之前；HTML Keyword List 的 `<` 排在 `<!--` Comment 之前；PERL q/qq 形式可能与 Identifier 前缀重叠，VB rem 与 Identifier 同理。仅对同元素 Delimited 做局部例外仍不能正确处理 HTML，因此协调者选择统一规则取代最初局部方案。
- 规格澄清：协调者已决定将“先匹配者优先”场景澄清为“最长匹配，等长先出现者优先”；worker 不修改 specs/design。
- Columns：按原语法优先级覆盖其固定区间，截断更低优先级的跨界 token。COBOL 第 73–80 列 LineNum 不能被从较早列开始的长 Identifier 吞没。列按 Unicode 标量计数，Tab 计一个字符；输出仍是 UTF-8 字节区间。
- List：纯字面词项按首字符分桶并检查词端点，避免把 echoing 当作 echo 关键字；非 ASCII 使用 Unicode 正则大小写检查的安全慢路径。
- Delimited：状态保存开放项索引；StopEOL 在行末清除，其他未闭合项继续下一行。转义字符跳过后继字符；转义与终止符相同时，只有双写才转义，单个仍关闭。
- Tokenizer 每格式只编译一次。Pattern::match_at 增加可用时的前缀快速否定，避免在不匹配位置无意义搜索整条长行；依赖前文的词边界仍用原上下文校验。零宽起始匹配不产出 token，也不阻止前进。
- 不修改内置 XML；未知语法类型继续报错。
- 证据：[tokenizer 测试](../../crates/bc-formats/tests/tokenizer.rs) 和 [25 格式语料](../../crates/bc-formats/tests/fixtures/README.md) 覆盖多行注释/字符串、最长匹配与等长顺序、固定列、Unicode、零宽匹配。
