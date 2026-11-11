# 文本行对齐策略：Beyond Compare 5.0.6 逆向实证

来源：Ghidra 12.1.4 反汇编 `BCompare.exe`（sha256 `75ea6977…7bf9826`，源文件 `TextMap.pas` / `BcAlignLines.pas`，断言路径可见）+ 运行时 XML 报告探针。CHM `sessiontextalignment.html` 作交叉印证。

## 设置项（TLineAlignDefFrame DFM + 会话 XML 键名）

| 设置 | XML 键 | 默认 | 语义（CHM） |
|---|---|---|---|
| 算法 | `AlignKind` = `alignNone`/`alignSmart`/`alignQuick`/`alignPatience` | alignSmart（Standard） | Unaligned=第 n 行对第 n 行；Standard=递归分节；Myers=全量 LCS；Patience=Bram Cohen |
| Never align differences | `NeverAlignDiffs` | False | True → 重要差异显示为删/增块（错行），不做 changed 配对 |
| Skew tolerance | `SkewTolerance` | **2000**（`TBcAlignLines+vm10` 返回常量 2000，DFM 标签印证） | 为一行找匹配时检查的最大行数 |
| Use closeness matching | `UseCloseness` | 会话可配；golden 管线 standard 固定 True | 对剩余错配行按相似度配对 |

## 同行 vs 错行的裁定规则（实证）

相等锚点行恒同行（same）。差异块（两锚点之间，无公共锚时落入 `FUN_01204580` 的无锚分支）：

1. `NeverAlignDiffs=True`（对象字段 0x80）→ 左右各成孤儿块（全错行）；不同行相似度恒 −5000。
2. `NeverAlignDiffs=False` 且 closeness 关（字段 0xac=0）→ **按位置顺序配对 min(左余,右余) 行为 Changed（同行），不做任何相似度检查**；多余侧错行。Myers 同此（展示打包，与 ADR 006 一致）。
3. closeness 开（0xac=1）→ 相似度 DP（`FUN_01204080`）：
   - 空位罚 **−20/行**；配对要胜过双空位（−40）。
   - 行得分（`FUN_012020d0`）：哈希相等 → `(有效长L+有效长R)*2 − (长度不等?1:0)`，空行 −10；哈希不等 → 规范化文本（UTF-8，0xfde9）**字节直方图交集** common（`FUN_01202010`），`t = 3·common − lenL − lenR`；`t<1` → `max(4t−1, −40)`；`t≥1` → `25·t / min(lenL,lenR)`；一侧无特征 → `clamp(−4(lenL+lenR)−1, −40, −11)`。
   - DP 构造平局保留配对方向（strict `<` 才改方向）→ **完全不相似的等数块也会被打成同行**（tie→pair）。回溯同向连续段合并为一次块发射；DP 单调，不允许交叉配对。
   - 退化：块内重复锚 ≥1001 或任一侧 >5000 → 退回 Standard 递归（`FUN_01203560`）。
4. 孤儿/配对块发射（`FUN_01201840`，0xffffffff=缺侧）：Smart 每次最多 **200** 行/段，Patience **2000** 行/段。

## 运行时探针（tools 外，一次性证据）

语料：L=`alpha/commonAnchor/foo one/bar two/omega`，R=`alpha/commonAnchor/bar 2/foo 1/omega`（块内 2v2 交叉重排，无内锚点）。

| 设置 | 结果 |
|---|---|
| Standard + closeness **False** | 位置配对：L3↔R3、L4↔R4 均 different（同行） |
| Myers | 同上（位置打包） |
| Standard + closeness **True** | 相似 DP：L4「bar two」↔R3「bar 2」同行 different；L3「foo one」、R4「foo 1」均孤儿（错行）——单调 DP 只能保留一对交叉，平局取后者 |

证据：`/tmp/bcalign/r-{close,noclose,myers}.xml`（会话节点已用后恢复 BCSessions.xml）。

## 对 Matcha 的差距

`crates/bc-core/src/align.rs` `SmartAlignOptions`（ADR 003）为启发式：字符 LCS 0.7 + 词 Jaccard 0.3、阈值 0.5、skew 8、128 行窗口。实证差异：

- skew tolerance 应为 **2000**（现 8）。
- 评分应改为字节直方图交集 + `t=3c−lenL−lenR` 公式与 −20 gap DP（现 0.7/0.3 加权）。
- closeness 关 / Myers 的纯位置打包已在 ADR 006；**closeness 开的平局强配**（tie→pair，含完全不相似）与 ADR 003「不相似不强配」项目规则冲突——KNOWN_DIFFS 唯一例外即源于此。若裁决「优先按参考行为」，需改配对平局方向并撤销该例外。
- 退化阈值：重复锚 ≥1001、侧 >5000 退 Standard；孤儿块分段 200（Smart）/2000（Patience）。

## 验收判断

- 黄金差分：`tools/golden/gen_expected.ps1`（standard=alignSmart+closeness True+skew 2000）。应新增区分性用例：① 交叉重排 2v2（区分相似 DP 与位置配对，期望=上表）；② 全不相似等数块（区分 tie→pair 与不 strong 配）。
- 引擎单测直接断言 `AlignMap` 行号序列 ↔ BC XML 报告 `ltid/rtid`。
- 反编译证据留存：`tmp/ghidra-align-{decomp,core,core2,core3,core4}.txt`（函数地址见文件头注释）。
