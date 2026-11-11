# 黄金语料差异登记

## 正式验收结果

- Myers：34/34（100%），无白名单。
- Standard + closeness：34/34（100%），**无正式差异**。
- 68 份 `expected.myers.xml` / `expected.standard.xml` 是参考工具按同设置生成的原始 XML，没有手工合并行、修改状态或改写内容。
- `031`–`034` 是 [文本对齐规格](../../../../docs/reference-parity/text-alignment-implementation-prompt.md) 的验收用例（交叉重排 2v2、不相似平局配对、2v3 尾部孤儿、`never_align_diffs=true`）。期望先按规格写出，2026-11-11 用参考程序采集后 8 份报告的行序列与规格逐行一致，已以原始采集替换。

| strategy | case | 原因与证据 |
|---|---|---|

## 已撤销：012-unrelated-replacement

原例外记录“项目不相似不强配，参考按位置显示三行 Changed”。对齐规格 §3 规定 closeness DP 平局保留配对（tie→pair），完全不相似的等数块也打成同行；新评分下 `AAAA/1111` 每对得分 −33，高于两个空位的 −40，项目输出与参考的 5 行逐行相同。该例外与“不相似不强配”项目规则一并撤销，见 [ADR 003](../../../../docs/decisions/003-smart-scoring.md)。

## patch 报告

patch 黄金（`expected.<profile>.patch-*.txt`，协议见 `tools/golden/README.md` 的“patch 报告协议”）**不登记任何例外**，覆盖 `text/2way` 全部 34 例（031–034 的 patch 样本与 XML 同日采集）。

## 历史诊断（不构成额外白名单）

Wave 3 的诊断记录及其原始 XML 保留在 tools/golden/evidence：

- Myers 默认替换打包：Wave 3b 已在 Quick 展示层实现；SES 未改变，Smart/NeverAlignDiffs 使用 unpaired 入口。
- PAS 内在不区分大小写：按协调者澄清改为 Unimportant（不是 Same）。
- 忽略行尾时 EOF 无换行：按协调者澄清改为 Same。
- 长行默认 4096 字符人工断行：临时专用格式 CharPerLine=65536 消除该预处理差异，不修改用例输入或原始报告。

正式白名单为空；若将来需要登记例外，必须同时给出 `<!-- project-output:… -->` 与 `<!-- reference-output:… -->` 两份完整行序列，回归会钉住两者。
