# ADR 003：Smart 对齐评分（待调参）

- 状态：决策占位；任务 4.3 与黄金语料阶段收敛。
- 背景：精确评分尚无充分证据，现有选项名称是暂定接口；不在本轮实现 Smart 对齐。
- 当前决定：遵循设计 D1，以 Myers 行哈希 LCS 为骨架，在差异区块内结合字符 LCS 比率、词 Jaccard、格式权重锚点和偏移容忍重配对，不强制配对不相似行。
- 待定：截断长度、相似度阈值与各权重，必须通过合成语料和黄金回归调参，不能假称精确复刻。
- 验证：Quick 语料逐行一致，Smart 目标 ≥95%；归档剩余差异；公开可预测的 Quick 退路。
- 关联：[设计 D1/D9](../../openspec/changes/text-compare-core/design.md)、[bc-core](../../crates/bc-core/README.md)、[bc-engine-text](../../crates/bc-engine-text/README.md)。
