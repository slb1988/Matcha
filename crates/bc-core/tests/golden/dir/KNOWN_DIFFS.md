# Folder golden 已知差异

## 已批准的不一致：1 条

| case | 配对行 | 原生 XML | 本分支 | 原因与处理 |
|---|---|---|---|---|
| rules-comments | example.cpp ↔ example.cpp | diff | Unsupported | 文本规则入口须合并 text-compare-core 后接通；协调者批准本波明确例外，不伪造规则结果 |

两份输入只有行注释文字不同；保留本机参考工具实际报告的 diff，
不自行把它改写为“相似”。报告的文件状态没有展开重要/不重要的全部细节。
回归要求这个例外仍然恰好一行；新增差异直接失败，例外被解决后也要求更新本文件/断言。

## 采集限制（不是候选状态不一致，不计入上述 1 条）

帮助中的 `criteria timestamp:2sec size timezone:left+6` 在本机参考程序返回
exit 106：Unexpected argument(s) "timezone:left+6"。
该样本没有可用 oracle，未制造 expected.xml，也不计入状态一致率分母。
时区行为已有成功的 timezone:ignore 样本；偏移参数仍由引擎解析/往返单测覆盖。

重放诊断：
`powershell -File tools/golden/gen_dir_expected.ps1 -CaseName timezone-offset -WorkerId <owner>`。
预期非零退出并在 target/folder-reference-timezone-offset-error.log 保留失败日志。

## 证据边界

- 原生 XML 的 filecomp 有 status；foldercomp 没有独立状态属性。
  因此报告文件行状态一致率，同时单独核对文件夹配对结构；不编造原生文件夹颜色断言。
- Flatten 的重复文件名配对已在无共享路径和有共享路径两种输入中观测：
  按各侧相对路径排序配对，不优先同内容或同完整相对路径。
- 本轮 GUI 物理桌面输入受限，独立列于 worker 报告，不混入黄金已知差异数。
