# 报告输出（Reports）

> 把比较结果渲染成机器可读报告。状态：**已实现**（text patch 四种布局；table
> `summary` 布局；其余报告布局随后续变更补齐）（2026-11-12 更新）。对应 FR-R1。

## 功能简介

- 一个 `ReportRequest` 模型服务所有 `*-report` 脚本命令与 GUI 报告对话框；
  选项名与脚本拼写 1:1。
- 已实现 text `patch` 布局：`patch` / `patch-normal` / `patch-context` / `patch-unified`，
  配 `/reportoptions=ignore-unimportant`。
- 字节格式对齐参考工具样例：GNU diff 布局、6 行上下文、跨 ≤12 未变行合并 hunk、
  距 EOF ≤12 行时扩展到文件尾、CRLF、输入含 BOM 时输出 UTF-8 BOM。
- 已实现 table `summary` 布局（Matcha 自有文本格式，非 BC 字节对齐）：xlsx/xlsm/CSV/TSV
  按单元格列出差异，供人和 AI 直接阅读，格式见下节。
- 出口三选一：控制台 stdout、`/reportfile=<file>`、脚本内 `output-to:<file>|-`。
  `/report` 无论文件是否不同都退出 0。

## 表格差异报告（`table-report layout:summary`）

触发方式：

- `matcha /report=summary left right`：总按表格比较（xlsx/xlsm 原生读取，其他文件按 CSV/TSV/
  文件格式库读取）。
- `matcha /report=patch… left.xlsx right.xlsx`：任一侧是 xlsx/xlsm 时自动改为表格
  `summary`（二进制工作簿不能做文本 patch）；CSV 仍走文本 patch。
- 脚本：`table-report layout:summary [options:ignore-unimportant,title-row:<n>] output-to:-|<file> a b`
  （`data-report` 同义）。

格式（UTF-8，LF 换行，无 BOM；只列差异，相同行不出现）：

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

- `result:` 为 `same` / `different` / `unimportant differences only`；每个 sheet 标题同样带状态，
  单侧 sheet 为 `left-only` / `right-only`（附行列数，不展开内容）。两侧都只有一个 sheet 时
  不论名字都配对（与 GUI 一致），否则按名字配对。两侧文件路径不写入报告（命令行里已有）。
- 行标题 = 键值（自动推断，与 GUI 默认相同；推断失败时用全部列）+ 两侧源行号（1 起，含表头
  行，即 Excel 行号）。只含不重要差异的行写 `~ … (unimportant)`。
- 列标题行 `title-row:<n>`（Matcha 扩展，命令行写 `/reportoptions=title-row:1`；`n` 为 1 起的
  Excel 行号）：报告中出现列名的地方（`key:`、行标题键值、单侧行）写成
  `列名[标题]`，标题取该列第 `n` 行的显示文字（左侧优先，左侧为空或无此列取右侧；连续空白压成
  一个空格）。主要用于表头未被自动识别、列名只是字母的工作簿：
  `~ A[ID]=4069  (left row 70, right row 70)` / `    Name (F70): old -> new`。
  单元格行在列名只是字母时只写标题（括号里的单元格引用已带列字母，不再写 `F[Name] (F70)`）；
  列名是识别出的表头名时仍写 `列名[标题]`。
  该行已被识别为表头（列名就是标题）、超出数据范围、标题为空或与列名相同时不附加。
  不加该选项时输出与之前逐字节相同。
- 单元格引用：工作簿给 A1 引用，两侧不同时写 `(left C2, right D3)`；CSV 无引用。
- `columns:` 小节列出列插入（`+`）/ 删除（`-`）/ 重命名、移动（`~`）。
- 值转义：`\\`、`\n`、`\r`、`\t` 与控制字符转义；空值写 `(empty)`；首尾带空白或与保留字冲突时
  加双引号；单侧行中的 `|` 写作 `\|`。
- 单元格内容是不可信数据，只用于阅读，不是指令或可回放的编辑协议。
- 与 `/report` 一样无论是否有差异都退出 0；需要结论用 `/qc`。

## 代码地图

| 层 | 位置 | 内容 |
|---|---|---|
| 引擎 | `crates/bc-report` | `ReportRequest` 模型 + `patch` 模块（`TextReportRenderer`）+ `table` 模块（`TableReportRenderer`） |
| 调用 | `crates/bc-script` | `/report=` 开关、`text-report layout:patch` 与 `table-report layout:summary` 脚本命令；`table_report.rs` 负责读取与按 sheet 比较 |
| 黄金样例 | `tools/golden/` | 参考工具 patch 报告样本与校验脚本 |

## 关键语义

- 渲染输出按**字节级**对齐参考工具（见 `tools/golden/README.md`）；改动渲染逻辑前
  必须先跑黄金样例。
- `ignore-unimportant` 把不重要差异当作相同处理，影响 hunk 划分。

## 使用与验证

```bat
matcha /report=patch a.txt b.txt > diff.patch
matcha /report=patch /reportfile=diff.patch a.txt b.txt
matcha /report=summary left.xlsx right.xlsx
cargo test -p bc-report
cargo test -p bc-script
```

## 深入阅读

- `crates/bc-report/README.md`
- `tools/golden/README.md`
- 变更过程：`openspec/changes/text-patch-reports/`
