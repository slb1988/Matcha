# ADR 003：Smart 对齐评分（初版已实现，待黄金语料调参）

- 状态：Wave 2 实现并通过合成用例；不是参考工具精确评分复刻，黄金语料仍属任务组 7。
- 骨架：Myers 行哈希 diff；在纯左右孤立行区块内再配对，公共行与手动强制锚点不改动。
- 文本访问：AlignInput 借用 LineList；只为当前差异窗口构造有界字符/词特征，不为所有行复制 String。无文本时退化为 Quick 骨架。
- 评分：Unicode 字符 LCS 比率 `2*LCS/(lenL+lenR)` 占 0.7；字母数字/下划线词集 Jaccard 占 0.3。默认阈值 0.5，零相似度永不配对；空白/大小写容忍按选项执行。
- 限制：候选仅比较前 256 个字符（可设 1–1024），每窗口最多 128 行，候选偏移默认不超过 8。窗口内动态规划最大化配对分数并保持原始顺序；大区块/长行是有意的有界启发式，不承诺全局最优。
- 权重：达到相似阈值后，两行相同正权重的候选获得 `4*priority` 额外分数，使 DFM object 等高权重行优先于普通相似行；无关行不因权重被强配。
- 手动覆盖：独立 AlignmentOverrides 保存有序不交叉锚点，重算前后分区时复用，清除后恢复自动结果。
- 取消：单个候选比较和有界分块间检查；生成/索引每 4096 行检查。jobs→core 的 CancelCheck 桥接，不反向依赖作业调度器。
- 验证：bc-core 的 align/smart_align 测试覆盖改动配对、不相似行、偏移阈值、DFM 权重、Unicode/长行、手动覆盖复用与清除；bc-jobs 的 cancellation 测试和 align_bench 使用真实 CancelToken。
- 待验证：黄金 Quick 逐行一致与 Smart ≥95% 指标、极端窗口边界和最终调参；不得把当前合成测试宣称为黄金验收。
- 关联：[设计 D1/D9](../../openspec/changes/text-compare-core/design.md)、[bc-core](../../crates/bc-core/README.md)。
