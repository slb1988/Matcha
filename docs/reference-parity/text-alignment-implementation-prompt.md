# 任务：实现文本比较的行对齐引擎（行为规格）

## 背景

仓库 `D:\Github\Matcha` 是 Rust workspace。`crates/bc-core/src/align.rs` 已有对齐骨架：`QuickAlign`（行哈希 LCS）、`SmartAlign`（LCS 骨架 + 差异块内二次配对，现为启发式评分）、`AlignStrategy::{Quick, Smart, Patience, None}`、`AlignMap`/`AlignRow` 行映射结构；`crates/bc-engine-text/src/jobs.rs` 是驱动层。相关决策记录：`docs/decisions/003-smart-scoring.md`、`006-myers-display-packing.md`。

**任务**：把差异块的配对逻辑替换为下述精确行为规格，并用黄金差分测试验收。所有"参考行为"均以本文件为唯一契约，不需要也不允许访问任何外部工具。

## 术语

- **同行配对（Changed）**：左右各取一行形成同一输出行。
- **错行（orphan）**：一侧有行、另一侧为空位（gap）。
- **锚点**：两侧内容相等的行。
- **差异块**：两个锚点之间（或文件首尾到首个锚点）左右各一段未匹配行。

## 行为规格

### 1. 对齐策略

四种策略，会话级可选：

| 策略 | 内部 | 锚点算法 |
|---|---|---|
| Standard（默认） | Smart | 递归分节：在偏移窗口内找相等锚点，命中后递归处理锚点两侧子段 |
| Myers O(ND) | Quick | 全量行哈希 LCS |
| Patience | Patience | 前后缀裁剪 + 两侧唯一公共行 + 最长递增子序列（LIS） |
| Unaligned | None | 第 n 行对第 n 行 |

- **Skew tolerance**：为某行找匹配时检查的最大行数，默认 **2000**。
- 行"相等"判定走行内容的规范化哈希（沿用现有 importance/规则管线，不要新造）。

### 2. 差异块的同行/错行裁定

差异块落到无可用锚点时（`NeverAlignDiffs` 与 `UseCloseness` 为会话布尔项）：

1. `NeverAlignDiffs=True` → 块内**全部错行**：左侧整段 orphan，右侧整段 orphan；不做任何配对。
2. `NeverAlignDiffs=False` 且 `UseCloseness=False`（或 Myers 策略）→ **纯位置配对**：按原顺序把左第 i 行与右第 i 行配成 Changed（同行），共 min(左长,右长) 对；多余一侧错行。**不做相似度检查**。
3. `NeverAlignDiffs=False` 且 `UseCloseness=True` → 相似度 DP（见 §3）。
4. 退化保护：块内重复锚数量 ≥ 1001，或任一侧 > 5000 行 → 放弃 DP，退回 Standard 的递归分节处理。
5. 块发射分段上限：Smart 每段最多 **200** 行，Patience **2000** 行（发射时切片，不影响配对结果本身）。

### 3. Closeness 相似度 DP

对差异块内左 `nL` 行、右 `nR` 行做单调（不允许交叉）动态规划，最大化总分：

- **空位罚分 −20/行**（左孤儿、右孤儿均 −20）。
- **配对得分 score(l, r)**：
  - 行规范化文本做 **UTF-8 字节直方图**（区分大小写/空白的规范化沿用现有 importance 选项），`common` = 两个直方图的多重集交集大小（`Σ_b min(cntL[b], cntR[b])` 的等价一遍实现）；
  - `lenL`、`lenR` = 规范化文本字节长度；
  - `t = 3·common − lenL − lenR`；
  - `t < 1` → `score = max(4·t − 1, −40)`；
  - `t ≥ 1` → `score = 25·t / min(lenL, lenR)`（整数除法）；
  - 任一侧无特征（空规范化文本）→ `score = clamp(−4·(lenL+lenR) − 1, −40, −11)`；
  - 哈希相等的行 → `score = 2·(sigLenL + sigLenR) − (lenL ≠ lenR ? 1 : 0)`，双侧均空 → `−10`（`sigLen` = 有效字符长度）。
- **DP 转移平局规则**：默认方向取"配对"（对角）；仅当严格更小时才改取空位方向。效果：平局保留配对（tie→pair）。**完全不相似的等数块也会被配成全部同行**，这是有意的规格，不得"修正"。
- **回溯**：从终点回溯，方向 3=同行配对、2=右孤儿、1=左孤儿；同向连续段合并成一次块发射。
- 配对得分与空位罚的关系：配对胜出当且仅当总分更高，即隐含阈值"score > −40 才可能赢"。

### 4. 与骨架的衔接

- 锚点行恒为同行 Same/Unimportant；手动对齐覆盖（`AlignmentOverrides`）优先级最高，现有行为不变。
- Smart 先取 Quick 的 **未打包** 骨架（`align2_unpaired`），差异块内再按 §2/§3 重配；Quick/Myers 的展示层位置打包逻辑（ADR 006）保持不变。
- 保留 `CancelCheck` 取消点与现有的有界处理纪律；DP 为 O(nL·nR)，仅在 §2.4 阈值内执行。

## 需要修改的点

1. `crates/bc-core/src/align.rs`：`SmartAlignOptions` 与 `smart_window` 的评分/阈值替换为 §3 公式；`skew_tolerance` 默认改为 2000；删除旧的 0.7/0.3 加权与 0.5 阈值；实现平局保留配对。
2. `docs/decisions/003-smart-scoring.md` 与 `crates/bc-core/tests/golden/KNOWN_DIFFS.md`：撤销"不相似不强配"例外，行为以本规格为准。
3. 对外 API、黄金目录结构、报告格式不变。

## 验收标准（全部通过才算完成）

1. **既有黄金**：`cargo test -p bc-core` 全绿；正式黄金 Myers 30/30。
2. **新增区分性用例**（加入 `crates/bc-core/tests/golden/text/2way` 并生成期望）：
   - **交叉重排 2v2**：左 `alpha / commonAnchor / foo one / bar two / omega`，右 `alpha / commonAnchor / bar 2 / foo 1 / omega`。
     - Myers 与 Standard+closeness关：L3↔R3、L4↔R4 全为 Changed（同行）。
     - Standard+closeness开：仅 L4↔R3 为 Changed（同行）；L3 与 R4 均为 orphan（错行）。
   - **全不相似等数块**：closeness 开时仍全部同行 Changed（tie→pair）。
   - **2v3 不等数块**：配对 + 尾部孤儿的位置正确。
   - `NeverAlignDiffs=True`：同输入全部 orphan。
3. **单测**：`crates/bc-core/tests/align.rs` 直接断言 `AlignMap` 的 `(left, right, kind)` 行序列与上述期望一致。
4. **回归**：`cargo clippy -p bc-core -- -D warnings`；`cargo test --workspace` 无新增失败。

## 非目标

- 不改 GUI 渲染、报告导出、会话序列化格式。
- 不实现 3-way 对齐（`align3` 维持现状）。
- 不为本规格之外的行为差异兜底；规格未覆盖处保持现状并记录。
