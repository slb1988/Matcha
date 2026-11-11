# 黄金语料差异登记

## 正式验收结果

- Myers：30/30（100%），无白名单。
- Standard + closeness：29/30（96.67%），正式差异 **1** 条。
- 60 份 `expected.myers.xml` / `expected.standard.xml` 是参考工具按同设置生成的原始 XML，没有手工合并行、修改状态或改写内容。
- 以下唯一差异保留项目“不相似不强配”要求，经协调者明确接受；不降低相似度阈值来强配无关内容。

| strategy | case | 原因与证据 |
|---|---|---|
| standard | 012-unrelated-replacement | 参考默认按位置显示三行 Changed；项目将完全不相似的内容保留为三行删除加三行插入。见下面的完整输入与双方输出。 |

## 012-unrelated-replacement

设置：Standard + closeness，SkewTolerance=2000，NeverAlignDiffs=False；普通文本，空白/大小写重要，行尾忽略，所有语法元素重要，OrphansAlwaysSig=False。

左输入（行号 1–5）：

```text
begin
AAAA
BBBB
CCCC
end
```

右输入（行号 1–5）：

```text
begin
1111
2222
3333
end
```

下列输出的左右行号均为 **1-based**，`-` 表示空侧；最后一列为原始三态（0=Same，1=Unimportant，2=Important）。

### 项目 Standard 输出（8 行）

<!-- project-output:standard:012-unrelated-replacement -->
```text
1|1|Same|0
2|-|LeftOnly|2
3|-|LeftOnly|2
4|-|LeftOnly|2
-|2|RightOnly|2
-|3|RightOnly|2
-|4|RightOnly|2
5|5|Same|0
```

### 参考 Standard 输出（5 行）

<!-- reference-output:standard:012-unrelated-replacement -->
```text
1|1|Same|0
2|2|Changed|2
3|3|Changed|2
4|4|Changed|2
5|5|Same|0
```

原始参考报告：[expected.standard.xml](text/2way/012-unrelated-replacement/expected.standard.xml)。两侧文本文件与报告同目录。Myers 展示层按参考打包，本用例的 Myers 输出已经逐行相等；不能把这个 Standard 例外扩展为 Myers 例外。

## patch 报告

patch 黄金（`expected.<profile>.patch-*.txt`，协议见 `tools/golden/README.md` 的“patch 报告协议”）**不登记任何例外**。上表唯一的对齐差异 standard / `012-unrelated-replacement` 在 patch 中不可见：patch 以“连续的非相同行”为一个差异段，项目的三行删除加三行插入与参考的三行改动都成为同一个 `2,4c2,4` 段，context 两侧都用 `! `，unified 都是先三行 `-` 再三行 `+`；参考自身在两套规则下的 patch 输出也逐字节相同。`bc-report` 的黄金回归对该用例按普通用例严格比较。

## 历史诊断（不构成额外白名单）

Wave 3 的诊断记录及其原始 XML 保留在 tools/golden/evidence：

- Myers 默认替换打包：Wave 3b 已在 Quick 展示层实现；SES 未改变，Smart/NeverAlignDiffs 使用 unpaired 入口。
- PAS 内在不区分大小写：按协调者澄清改为 Unimportant（不是 Same）。
- 忽略行尾时 EOF 无换行：按协调者澄清改为 Same。
- 长行默认 4096 字符人工断行：临时专用格式 CharPerLine=65536 消除该预处理差异，不修改用例输入或原始报告。

正式白名单只能包含上表这一项；回归同时校验该例外的项目完整输出，避免把任意新错误藏在同一用例名下。
