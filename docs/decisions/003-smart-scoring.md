# ADR 003：Smart 对齐评分（closeness DP）

- 状态：按 [文本对齐规格](../reference-parity/text-alignment-implementation-prompt.md) 实现，取代 Wave 3b 的启发式评分。正式黄金 Myers 34/34、Standard + closeness 34/34（含规格验收用例 031–034，其参考采集与规格逐行一致）；无正式例外。
- 骨架：Smart 先取 Myers 的未打包骨架（`align2_unpaired`），Patience（`PatienceAlign`）取 patience diff 骨架；公共行与手动锚点（`AlignmentOverrides`，优先级最高）不改动。两者都只在两个锚点之间的差异块内重配。
- 差异块裁定（`SmartAlignOptions`）：`never_align_diffs` → 左右整段错行；否则 `use_closeness = false` → 第 n 行对第 n 行配对，不做相似度检查，多余一侧错行；否则走 closeness DP。Myers 不评分，展示层按 [ADR 006](006-myers-display-packing.md) 位置打包。
- 行得分：规范化文本（默认去空白；大小写按 importance 的 case 选项折叠）的 UTF-8 字节直方图，`common` 为多重集交集，`t = 3·common − lenL − lenR`；`t < 1` → `max(4t − 1, −40)`，`t ≥ 1` → `25t / min(lenL, lenR)`；一侧规范化为空 → `clamp(−4(lenL + lenR) − 1, −40, −11)`；哈希相等 → `2(sigL + sigR) − (原始字节长度不等 ? 1 : 0)`，双侧规范化为空 → −10。
- DP：单调、不允许交叉；每个错行 −20。每格先取配对，只有严格更大才改取空位方向，先试右错行再试左错行。因此平局保留配对，完全不相似的等数块也全部同行（tie→pair）；2v2 交叉重排只保留后一对（L4↔R3）。回溯同向连续段合并成一次发射，按 Smart 200 行 / Patience 2000 行切片（只影响取消检查粒度，不影响配对）。
- 退化保护：块任一侧 > 5000 行，或左侧在右侧出现的重复锚行 ≥ 1001 → 不做 DP，改为分节：左侧逐行在当前右侧位置之后 `skew_tolerance`（默认 2000）行内找第一个相等行作锚点，锚点间子段若满足 DP 阈值则走 DP，否则按位置配对。
- 取消与边界：DP 每行检查取消；方向表 `(nL+1)(nR+1)` 字节，在阈值内最多约 25 MB；行特征是稀疏字节直方图，不复制整行 `String`。没有行文本的调用方（只给哈希）在 closeness 开时保留错行骨架。

## 规格未覆盖、保持现状的部分

- 格式权重（DFM `object` 等 `LineWeight`）：两行权重相同时在 DP 得分上加 `4 × weight`。无权重或权重为 0 时与规格公式完全一致；黄金语料不受影响。
- `sigLen` 取规范化后的字符数；哈希相等分支里的长度不等判断使用原始行字节长度（规范化后的长度在哈希相等时通常相同）。
- 退化分节的叶子在仍超阈值时按位置配对；参考实现在这一分支的确切行为没有规格。
- Unaligned 不受 `NeverAlignDiffs` 影响；3-way 对齐不在本次范围内。

## 已撤销

- Wave 3b 的“字符 LCS 0.7 + 词 Jaccard 0.3、阈值 0.5、偏移 8、128 行窗口”启发式及“不相似不强配”项目规则。`012-unrelated-replacement` 的唯一 Standard 例外随之消除，见 [KNOWN_DIFFS](../../crates/bc-core/tests/golden/KNOWN_DIFFS.md)。
- 引擎在 Standard + closeness 关时改走 Myers 未打包骨架的做法：现按规格 §2.2 按位置配对。

## 名称

内部 Quick → 公开 Myers O(ND)，Smart → Standard，Patience → Patience，None → Unaligned。

- 关联：[设计 D1/D9](../../openspec/changes/text-compare-core/design.md)、[bc-core](../../crates/bc-core/README.md)、[ADR 007](007-golden-reference-settings.md)。
