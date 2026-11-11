# hex-compare 交接文档

交接时间：2026-11-11；整理者：pi k3.1-pro（Windows 会话，OpenSpec explore 模式）。下一棒：Mac 上的 REA 静态分析。本文档是该任务的唯一入口。

## 1. 当前状态

- **规格已完成**：`openspec/changes/hex-compare/`（proposal / design D1–D7 / tasks 7 组 23 项 / specs：`hex-byte-alignment`、`hex-compare-view`），`openspec validate` 通过。
- **实现未开始**。实现入口：`$openspec-apply-change`。
- 参考行为真源：`docs/reference-parity/hex-compare.md`（命令逐项 H01–H34 + 证据边界），截图与语料在同名目录（`hex-compare/`），语料含确定性生成器（`corpus/gen_corpus.py`，SHA-256 校验逐字节可复现）。

## 2. 已锁定的关键行为结论（Windows 运行时证据）

1. **渲染 = 对齐图按每行 N 格做单元格打包（cell packing）**：孤儿字节占格、对侧斜纹占位格可在行中；行标签 = 该侧本行首个真实字节偏移（全占位行留空）；奇数长度孤儿段使后续行相位整体偏移。证据：`24-odd-insert-all.png`、`23-odd-insert.png`、`06-insertion-closeup.png`。
2. **四分类统计**：same / left orphan / right orphan / difference；每侧合计 = 文件大小。参考语料实测 4120/32/16/8（`20-info.png`）。
3. **对齐策略三档**：Complete（默认，复杂全量）/ Fast（大文件启发式）/ None（逐位）；文件锁阈值默认 2 MB（小文件整载不加锁）。
4. Diff 过滤上下文默认 16 字节；Layout：bytes per group 8、Fit to window 默认开；查找为内联栏（hex/文本）；Copy to Other Side Ctrl+R；状态栏含 Insert/Overwrite 与每侧光标地址。
5. 黄金对拍可用脚本 `hex-report layout:side-by-side|summary|interleaved options:display-all|display-mismatches|display-matches,line-numbers output-to:...`（CHM `scripting_reference.html`）。

## 3. Mac 上的 REA 任务（剩余）

目标二进制：`D:\Program Files\Beyond Compare 5\BCompare.exe`（需从 Windows 机拷贝到 Mac；PE x86_64，SHA-256 `75ea69772796c909a6659f23037f779de3399eccaa2332969f6dcc28c7bf9826`）。

注意：REA 的**原生 UI 观测/截图工具仅支持 macOS 目标**（Windows 上已实测报 `capability_unavailable`，无实时观测价值）；PE 静态分析（Ghidra provider）跨平台可用，但该 40 MB Delphi 二进制在 Windows 会话中分析超时未完成——在 Mac 上让其跑完再查。

待核对清单（结果回填 `hex-compare.md` 的"后续补充项"与 `bc-engine-bin` 骨架注释）：

1. 类名映射：`THexContent`、`THexMap`、`THexMapAlign`、`THexMapByteAlign`（`TBcByteAlign`）、`THexRules`、`THexCompareSession`、`THexFiltered`。
2. Fast 与 Complete 的算法边界：锚块大小、区间上限、是否存在 skew tolerance 参数及其默认值。
3. View 菜单两个未展开子菜单的完整项集合：**Little/Big Endian Values**（数值类型清单）与 **Byte Address = 00000000**（地址格式选项）。
4. `hex-report` 各 layout（side-by-side/summary/interleaved）的文本输出格式细节（供黄金对拍断言用）。

## 4. 环境与注意事项

- Windows 机上的 BC 实例仍开着（`l2.bin <--> r2.bin` 标签页，Diffs 过滤态），可继续交互或直接关闭。
- Snipaste 曾在采集中途吞按键（截图 overlay），后自行退出；若再采集先确认它不在截图模式。
- BC 对误投按键敏感（曾致进程退出）；交互脚本在 `tmp/hex-ref/scene.ps1`（未提交，含置顶/焦点/发键/截图一体化）。
- `tmp/hex-ref/chm/` 有 CHM 全量解包（479 文件，未提交）；已引用的主题结论都在 `hex-compare.md`。
- 红线沿用：不提取/描摹参考工具图标与位图资源；截图与语料仅作内部工程证据（私有仓库）。
