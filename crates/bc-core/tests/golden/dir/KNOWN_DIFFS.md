# Folder golden 已知差异

## 已批准的不一致：0 条

任务 8.2 合并 master 后，规则比较经 bc-engine-text 接通（默认文本规则，按文件名选格式），
rules-comments 的 example.cpp ↔ example.cpp 得到 Different，与原生 XML 的 diff 一致。
回归现在要求全部 33 个文件行一致，任何不一致直接失败。

历史：Wave C 时该行为 Unsupported，是协调者批准的唯一例外。语料 case.xml 仍保留采集时的
knownDiff="rules-unavailable" 注记；为不改动带 provenance 的生成器与语料字节，没有重写它，
测试也不再据此放行任何行。

两份输入只有行注释文字不同；保留本机参考工具实际报告的 diff。默认文本规则下注释不重要
（2026-11-17 起，与参考文本比较界面的默认一致），同一对为“相似”，开启 ignore-unimportant 视为相同；
`criteria rules-based comments-important` 时为 Different（引擎单测覆盖）。原生 XML 状态不区分
重要/不重要差异（“相似”同样报告为 diff），因此这一点没有原生 oracle。

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
