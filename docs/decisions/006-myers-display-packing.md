# ADR 006：Myers 展示行打包

- 状态：Wave 3b 按协调者明确决定实现，正式黄金 Myers 30/30 通过。
- 分层：`diff::MyersDiff` 的 SES、前后缀裁剪与大输入回退完全不变。`QuickAlign` 默认只在展示行展开层处理没有 Equal 锚点的连续 Delete/Insert 区段，按原顺序将左右各第 n 行配成 Changed，多余一侧仍为 orphan；不引入相似度算法。
- 边界：不会跨越 Equal 行或手动锚点打包，每侧索引仍恰好出现一次且严格递增。打包直接从 DiffOp 展开，按最终行数预留容量，不复制第二份全量行表；仍检查取消。
- 默认接口：align2 / align2_with_overrides / align2_cancellable 返回打包后的展示行，匹配参考 Myers 在 NeverAlignDiffs=False 时的默认报告。
- 原始入口：align2_unpaired / align2_unpaired_cancellable 保留 SES 孤立行。Smart 使用它作为二次配对骨架；engine 的 NeverAlignDiffs=True 也使用它。显式手动锚点仍优先。
- 名称：内部 Quick 对应公开 Myers O(ND)，内部 Smart 对应 Standard + closeness。由 TBcAlignKind 枚举、Alignment 窗体的 AlignQuickEdit/Myers 与 AlignSmartEdit/Standard 名称以及受控 XML 报告共同确认。
- 非目标：不为了匹配参考 Standard 的无关内容打包而改变项目“不相似不强配”要求。该唯一差异记录在 [KNOWN_DIFFS](../../crates/bc-core/tests/golden/KNOWN_DIFFS.md)，包含输入与双方完整行序列。
- 证据：[core 对齐测试](../../crates/bc-core/tests/align.rs)、[engine 开关测试](../../crates/bc-engine-text/tests/map_jobs.rs)、[正式黄金](../../crates/bc-core/tests/golden/README.md)。
