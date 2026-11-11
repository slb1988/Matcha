# 报告输出（Reports）

> 把比较结果渲染成机器可读报告。状态：**已实现**（text patch 四种布局；
> 其余报告布局随后续变更补齐）。对应 FR-R1。

## 功能简介

- 一个 `ReportRequest` 模型服务所有 `*-report` 脚本命令与 GUI 报告对话框；
  选项名与脚本拼写 1:1。
- 已实现 text `patch` 布局：`patch` / `patch-normal` / `patch-context` / `patch-unified`，
  配 `/reportoptions=ignore-unimportant`。
- 字节格式对齐参考工具样例：GNU diff 布局、6 行上下文、跨 ≤12 未变行合并 hunk、
  距 EOF ≤12 行时扩展到文件尾、CRLF、输入含 BOM 时输出 UTF-8 BOM。
- 出口三选一：控制台 stdout、`/reportfile=<file>`、脚本内 `output-to:<file>|-`。
  `/report` 无论文件是否不同都退出 0。

## 代码地图

| 层 | 位置 | 内容 |
|---|---|---|
| 引擎 | `crates/bc-report` | `ReportRequest` 模型 + `patch` 模块（`TextReportRenderer`） |
| 调用 | `crates/bc-script` | `/report=` 开关与 `text-report layout:patch` 脚本命令 |
| 黄金样例 | `tools/golden/` | 参考工具 patch 报告样本与校验脚本 |

## 关键语义

- 渲染输出按**字节级**对齐参考工具（见 `tools/golden/README.md`）；改动渲染逻辑前
  必须先跑黄金样例。
- `ignore-unimportant` 把不重要差异当作相同处理，影响 hunk 划分。

## 使用与验证

```bat
matcha /report=patch a.txt b.txt > diff.patch
matcha /report=patch /reportfile=diff.patch a.txt b.txt
cargo test -p bc-report
```

## 深入阅读

- `crates/bc-report/README.md`
- `tools/golden/README.md`
- 变更过程：`openspec/changes/text-patch-reports/`
