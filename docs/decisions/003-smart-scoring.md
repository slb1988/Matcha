# ADR 003：Smart 对齐评分（Wave 3b 黄金验收）

- 状态：Wave 3b 正式 30 用例验收：Myers 30/30，Standard + closeness 29/30（96.67%）。仍不是参考工具私有评分的精确复刻；唯一不相似强配差异经协调者接受并记录。
- 骨架：Myers 行哈希 diff；在纯左右孤立行区块内再配对，公共行与手动强制锚点不改动。
- 文本访问：AlignInput 借用 LineList；只为当前差异窗口构造有界字符/词特征，不为所有行复制 String。无文本时退化为 Quick 骨架。
- 评分：Unicode 字符 LCS 比率 `2*LCS/(lenL+lenR)` 占 0.7；字母数字/下划线词集 Jaccard 占 0.3。默认阈值 0.5，零相似度永不配对；空白/大小写容忍按选项执行。
- 限制：候选仅比较前 256 个字符（可设 1–1024），每窗口最多 128 行，候选偏移默认不超过 8。窗口内动态规划最大化配对分数并保持原始顺序；大区块/长行是有意的有界启发式，不承诺全局最优。
- 权重：达到相似阈值后，两行相同正权重的候选获得 `4*priority` 额外分数，使 DFM object 等高权重行优先于普通相似行；无关行不因权重被强配。
- 手动覆盖：独立 AlignmentOverrides 保存有序不交叉锚点，重算前后分区时复用，清除后恢复自动结果。
- 取消：单个候选比较和有界分块间检查；生成/索引每 4096 行检查。jobs→core 的 CancelCheck 桥接，不反向依赖作业调度器。
- 验证：bc-core 的 align/smart_align 测试覆盖改动配对、不相似行、偏移阈值、DFM 权重、Unicode/长行、手动覆盖复用与清除；bc-jobs 的 cancellation 测试和 align_bench 使用真实 CancelToken。
- 已验证：正式 Myers 100%、Standard ≥95%，同设置期望均由参考工具生成并入库。SmartAlignOptions 默认权重/阈值没有为通过语料而调松；极端窗口边界仍可通过后续语料继续验证。

## Wave 3 公开名称与报告行为发现

协调者确认的映射为：项目内部 Quick → 公开 Myers O(ND)，Smart → Standard + closeness，Patience → Patience Diff。定向 RTTI 显示 alignNone/alignPatience/alignQuick/alignSmart；Alignment 窗体资源分别把 AlignQuickEdit 标为 Myers、AlignSmartEdit 标为 Standard。序列化节点必须是 Rules Value="TTextRules"，命名会话放在 BCSessions 根级。

实际正常脚本观察：在 NeverAlignDiffs=False 时，Myers 和 Standard 都可能把相邻删/增内容报告成配对 Changed，包括完全不相似的三行；Myers 切换 NeverAlignDiffs=True 则报告左右孤立行。把它解释为展示/输出层打包是推断，未反汇编算法实现。

上述 Wave 3 阻塞已由 Wave 3b 明确决策解除：[ADR 006](006-myers-display-packing.md) 实现 Quick 默认展示打包并保留 unpaired 骨架；[ADR 007](007-golden-reference-settings.md) 固定普通重要性模板及长行预处理。Myers 黄金使用参考默认 NeverAlignDiffs=False，不用 NeverAlignDiffs=True 规避默认展示差异。Standard 对完全不相似内容保留项目规则，唯一例外见 [KNOWN_DIFFS](../../crates/bc-core/tests/golden/KNOWN_DIFFS.md)。
- 关联：[设计 D1/D9](../../openspec/changes/text-compare-core/design.md)、[bc-core](../../crates/bc-core/README.md)。
