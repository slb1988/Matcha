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

## Wave 3 公开名称与报告行为发现

协调者确认的映射为：项目内部 Quick → 公开 Myers O(ND)，Smart → Standard + closeness，Patience → Patience Diff。定向 RTTI 显示 alignNone/alignPatience/alignQuick/alignSmart；Alignment 窗体资源分别把 AlignQuickEdit 标为 Myers、AlignSmartEdit 标为 Standard。序列化节点必须是 Rules Value="TTextRules"，命名会话放在 BCSessions 根级。

实际正常脚本观察：在 NeverAlignDiffs=False 时，Myers 和 Standard 都可能把相邻删/增内容报告成配对 Changed，包括完全不相似的三行；Myers 切换 NeverAlignDiffs=True 则报告左右孤立行。把它解释为展示/输出层打包是推断，未反汇编算法实现。

当前 Quick 的纯孤立行输出与参考默认报告不同；Standard 的完全不相似强配也与项目明确要求不同。消息 msg_7ff4080efbd7 正等待协调者决定是否加入 Quick 展示层打包或给 Myers 黄金明确使用 NeverAlignDiffs=True；未擅改已验收算法/默认阈值。重要性配置的普通字段检查及同设置模板也待确认，正式 30 用例一致率尚未计算。详见 [生成器与原始诊断证据](../../tools/golden/README.md)。
- 关联：[设计 D1/D9](../../openspec/changes/text-compare-core/design.md)、[bc-core](../../crates/bc-core/README.md)。
