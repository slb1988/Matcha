# Matcha 比较报告

以下 `bat` 示例在 cmd 中运行，`matcha` 指已加入 PATH 的 `Tools/matcha/matcha.com`。Git Bash 的完整调用见下方 RawData 示例。

## 文本 patch

```bat
rem 输出到 stdout
matcha /report=patch-unified a.txt b.txt
matcha /report=patch-unified /reportfile=out.patch a.txt b.txt
```

`/report=` 布局：`patch`（= `patch-normal`，`diff` 风格，默认）、`patch-context`、
`patch-unified`（`@@`，6 行上下文；上下文相同时，hunk 正文与 `git diff` 一致）。
`/reportoptions=ignore-unimportant` 忽略不重要差异（注释差异默认算不重要）。
输出头形如 `--- <绝对路径>\t<时间戳>` / `+++ ...`，可直接给 `patch`/`git apply`（注意改成相对路径）。

## 读懂 unified diff 输出

```
@@ -旧起始行,旧行数 +新起始行,新行数 @@     行数为 1 时省略（-1 即 -1,1）
 空格前缀 = 上下文行（定位用）   - = 删除   + = 新增
```

patch 定位靠「hunk 头行号粗定位 + 上下文行精确纠偏」：文件行号偏移时工具会搜索上下文自动
对位（offset）；但触及第 1 行 / 文件尾的 hunk 会锚定，不允许偏移。`git apply` 无 fuzz，
GNU `patch` 默认允许两端 2 行上下文不匹配。

## 表格差异报告

```bat
rem 表格差异报告到 stdout
matcha /report=summary left.xlsx right.xlsx
rem 任一侧是工作簿时 patch* 自动改为同一报告
matcha /report=patch-unified left.xlsx right.xlsx
rem CSV/TSV 用 summary 按表格比；用 patch* 仍是文本 patch
matcha /report=summary a.csv b.csv
rem 第 1 行表头附进列名
matcha /report=summary /reportoptions=title-row:1 old.xlsx new.xlsx
```

输出（UTF-8、LF）首段为总结论，然后逐 sheet → 行 → 单元格，只列差异：

```text
result: different
sheets: 1 compared, 1 different, 0 same, 0 left-only, 0 right-only
legend: "~" changed row, "-" left-only row, "+" right-only row; cell lines are "column (ref): left -> right"

=== sheet "Data": different
key: id
rows: 1 changed, 0 unimportant, 0 left-only, 1 right-only, 1 same
~ id=1  (left row 2, right row 2)
    price (C2): 1 -> 1.5
+ id=3  (right row 4)
    id=3 | name=c | price=3
```

- `result:` 为 `same` / `different` / `unimportant differences only`；只有不重要差异的行写 `~ … (unimportant)`。
- `~` 改动行、`-` 仅左、`+` 仅右；行号是 Excel 行号（1 起，含表头）。
- 键列自动推断；推断不出时 `key: (none; rows aligned by content)`，按内容对齐，
  同位置内容有差异的行仍显示为 `~` 并附单元格明细。
- `columns:` 小节列出列插入 / 删除 / 重命名、移动。空值写 `(empty)`，单侧行里的 `|` 写作 `\|`。
- `title-row:<n>`：表头没被识别、列名只是字母时，把第 n 行（Excel 行号）的文字附上：
  行标题 / 单侧行写 `A[Name]=…`；单元格行直接写标题 `Name (F70): old -> new`（引用里已有列字母）；
  识别出的表头列不变。可与其他选项逗号连用：`/reportoptions=ignore-unimportant,title-row:1`。

## 判断文件是否相同

```bat
rem rules-based（默认），打印一行结论
matcha /qc a.txt b.txt
rem 还有 size / crc
matcha /qc=binary a.txt b.txt
rem 表格按单元格判定：同值不同字节 → Rules-based same（2）
matcha /qc old.xlsx new.xlsx
```

默认 `/qc` 按规则比较：实测两份内容不同的 RawData xlsx 返回 `Rules-based differences`（13），同一文件返回 `Rules-based same`（2）。
`/qc=binary` 比较字节：xlsx 是 ZIP 工作簿，单元格相同的文件也可能返回 11。其余结果见[退出码表](cli-reference.md#退出码)。

## RawData 的 P4 历史对比

在仓库根目录用 Git Bash 执行。先从 CL 中找到工作簿及其 revision，再用 `p4 print -q -o` 导出两版文件；这样保留二进制内容，也不必切换工作区版本。P4 的相对路径以当前目录为准，在子目录中查询可用完整 depot 路径。以下以 CL 148367 为例：

```bash
p4 changes -m 15 -s submitted "Main/RawData/....xlsx"
p4 describe -s 148367 | grep '\.xlsx'

src=Main/RawData/PLEntityTagMapping_Character.xlsx
out=Main/Saved/MatchaTest/148367
mkdir -p "$out"
p4 print -q -o "$out/r137.xlsx" "$src#137"
p4 print -q -o "$out/r138.xlsx" "$src#138"

# 先看单元格是否变化（2 = 规则相同，13 = 规则不同），再看具体改了哪里
Tools/matcha/matcha.com -qc "$out/r137.xlsx" "$out/r138.xlsx"
Tools/matcha/matcha.com -report=summary -reportoptions=title-row:1 "$out/r137.xlsx" "$out/r138.xlsx"
```

2026-11-12 用 7 个真实 CL 验证，覆盖五种形态：数值调参（`~` + 单元格 old→new）、
清空行（键值本身被改 → 正确的 `-`/`+` 对）、批量新增（`+` 带键值列表）、多 sheet 工作簿（逐 sheet 结论）、
大表追加（键列推断失败时按内容对齐，`+` 行附完整内容）。2026-11-18 用 11-18 版复测 `/qc` 与 summary，结论一致。
[历史实测报告](test-rawdata-xlsx-20261112.md)保留了各批次的原始输出与解读；其中 11-12 版的 xlsx `/qc` 二进制结论是旧版行为。
