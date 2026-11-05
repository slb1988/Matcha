# 黄金语料生成（Wave 3 准备状态）

`gen_expected.ps1` 用参考工具的正常脚本接口生成原始 XML；Rust 回归不依赖参考工具安装。当前 **30 个合成输入已齐，正式 60 份同设置期望尚未生成**，不能宣称一致率。

## 已验证接口

- 参考可执行文件版本 5.0.6.30713，安装路径由 `-ReferenceExe` 显式传入。
- 命名 `TTextCompareSession` 必须直接放在 `BCSessions` 根下，不是默认/历史 `TSessionFolder` 的 Items 中。
- 规则节点必须是 `<Rules Value="TTextRules">`；只有 `<Rules>` 会被忽略。
- 对齐属性直接在 Rules 下：`AlignKind`、`NeverAlignDiffs`、`SkewTolerance`、`UseCloseness`。
- 对齐枚举：`alignNone`、`alignPatience`、`alignQuick`、`alignSmart`。
- 窗体资源把 `AlignQuickEdit` 标为 Myers O(ND)，`AlignSmartEdit` 标为 Standard；Standard 可开启 closeness。
- `load <default>` 后，`text-report layout:xml ... "matcha-golden-myers"` / `"matcha-golden-standard"` 可成功读取这两个专用会话。

定向只读证据（文件偏移，不是运行时地址）：TLineRules 的 AlignKind 属性记录 `0xe7d396`，NeverAlignDiffs `0xe7d3ca`，SkewTolerance `0xe7d404`，UseCloseness `0xe7d43c`；枚举类型 TBcAlignKind `0xdfef80`；Alignment 窗体资源的 Standard/AlignSmartEdit `0x2f14585` 附近，Myers/AlignQuickEdit `0x2f14602` 附近。只检查这类配置 strings/RTTI/资源，没有分析授权逻辑，也未访问任何授权或密钥文件。

## 当前阻塞

参考 Myers 默认 `NeverAlignDiffs=False` 将相邻删/增行按位置报告为 Changed；项目 Quick 当前保持孤立行。`evidence/` 中有两个用例的原始诊断报告，另有 `NeverAlignDiffs=True` 报告证明该开关生效。Standard 对完全不相似的替换也会显示配对 Changed，这与项目“不强配”的明确要求不同。

待协调者回答的消息：`msg_7ff4080efbd7`。需要决定 Quick 展示打包还是为 Myers 黄金显式开启 NeverAlignDiffs，以及授权核实普通重要性字段以填写每个用例的同配置规则模板。尚未进行这种额外静态检查或擅改已验收的 Quick/Smart 语义。

因此正常生成模式要求 `profiles/<case>.rules.xml`（或 case.properties 的 `native_rules` 文件名）；模板缺失时 **在写用户配置之前失败**，不猜测空白/大小写/元素重要性。模板根必须是 `Rules Value="TTextRules"`，不得含 DOCTYPE；生成器只覆盖四个已经证实的对齐属性。规则模板尚待上述确认，未伪造占位模板。

## 使用

```powershell
./tools/golden/gen_expected.ps1 -ReferenceExe '<本机参考工具 exe>' -ComparisonTemplates ./tools/golden/profiles
```

生成器读取 UTF-8 `.txt` 模板与 `case.properties`，在 `target/golden-work` 转换 BOM、UTF-16 与 CR/LF 风格，再针对两个临时命名会话各生成一个原始报告。它不触碰 tmp 基准输入。

仅用于诊断、不能作为正式黄金验证的已跑通命令：

```powershell
./tools/golden/gen_expected.ps1 -ReferenceExe '<本机参考工具 exe>' -DiagnosticInheritRules -OutputRoot ./target/golden-diagnostic -CaseNames 011-similar-identifier,012-unrelated-replacement
```

加 `-MyersNeverAlignDifferences` 可验证未打包的 Myers 输出。诊断模式不强求尚未核实的重要性配置，所以强制禁止输出到正式语料目录。

## 配置安全与恢复

脚本只读取/备份/写入显式的 BCSessions.xml；必须先退出参考工具。每次先做带时间戳的原字节备份到 `tools/golden/backup/`（已忽略，不提交私人会话数据），临时只替换两个 `matcha-golden-*` 节点，其余 XML 保留。每轮进程结束后再改配置，整个生成过程 finally 恢复运行前原字节；超时只终止脚本自己启动的进程。

调查结束时，已在核对非工具会话节点完全相同后，恢复到最初的 `backup/20261105-222756/BCSessions.xml`，并逐字节验证一致。若以后需要手动恢复，先关闭参考工具，再把相应时间戳下的 BCSessions.xml 复制回显式 SessionsPath；勿覆盖备份之后的用户新改动。不要复制或读取目录里的其他文件，特别是授权/密钥文件。

诊断模式已完整生成 30×2 份 XML（保存在 target/golden-all-diagnostic），共享 Rust 解析器逐份验证 60/60 可解析；其中 9 份典型原始报告保存在 evidence/。另观察到尾换行差异在参考默认报告中为 same，与当前引擎的 Unimportant 有待核实。

`evidence/` 是诊断材料，不在 Rust 的正式 oracle 枚举范围内，不能用它凑满 30 个用例或声称达成一致率。正常模式仍缺已确认的重要性配置模板；默认规则诊断不是同设置的正式验收。
