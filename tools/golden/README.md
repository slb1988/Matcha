# 正式黄金语料生成

## 表格采集与普通设置协议

`gen_table_expected.ps1` 处理 `crates/bc-core/tests/golden/table` 中 30 个合成用例（27 CSV、3 个临时生成的 xlsx，含多 sheet）。沿用下述共享锁；只临时添加普通 `TTableCompareSession`，finally 恢复指定会话 XML 原字节并校验。不读取其他配置或授权文件；图片与工作簿只生成到忽略的 target，不提交参考图标或二进制 fixture。

```powershell
./tools/golden/gen_table_expected.ps1 -ReferenceExe '<本机参考程序 exe>' -WorkerId '<本次运行标识>' -LockTimeoutSeconds 30
cargo test -p bc-engine-table --test golden_table -- --nocapture
```

已经定向检查并由原生报告验证的表格协议（不需要再次逆向或猜属性名）：

- `TTableCompareSession` 直接放在 `BCSessions` 根下，`Rules Value="TTableRules"`。
- 脚本先 `load <default>`，再 `table-report layout:side-by-side options:display-all output-options:html-color ... "matcha-golden-table"`。直接 `load` 表格会话不是合法的表格脚本入口，会报要求 Folder Compare。
- `ColKeys/_N Value="0;1"`：N 为零基 sheet，值为零基比较列集合，**分号**分隔，不是逗号。
- `ColHandling/_N/_M`：N 为 sheet、M 为比较列。普通属性为 `AsText`、`Insig`、`StrCaseInsig`、`StrWSInsig`、`NumTolerance`、`NumToleranceRelative`、`DateTimeTolerance`。默认 Handling 在 Rules 下使用相同名称的 `Col` 前缀。
- `AlignDef/_N/Kind` 使用 `alignNone/alignPatience/alignQuick/alignSmart`；`AlignSort` 为分号分隔的 sheet 集合；还显式设置 NeverAlignDiffs、UseCloseness、SkewTolerance。
- 多 sheet 的键/Handling/AlignDef 分别写入对应 N，不能只配置第一张表后声称所有 sheet 使用相同设置。
- 正式语料显式按 Data 首行与当前原生报告对齐；日期语料明确记录 MDY / 分隔符，不将地区解释差异靠状态归一化掩盖。
- 原生多行字段报告可能含 NUL；使用可逆 `expected.html.json` 字符串封装并验证解码字节完全一致。普通 HTML 保持原字节；绝不修改状态或合并行。
- 每次运行先删除自有 target 中的旧报告并要求新报告非空，避免把遗留结果误认成新采集。提交前仍需检查没有图标、私人备份或二进制文件。
- 唯一 Unaligned 例外完整签名与 95% 行/格双门槛见 corpus 的 KNOWN_DIFFS。该报告路径差异不等同于参考 GUI 行为结论。

## 表格逐命令清单校验

`docs/reference-parity/table-compare.md` 逐段覆盖 D9。对只读解包的帮助运行：

```text
python tools/golden/check_table_parity.py --help-dir <帮助目录>
python tools/golden/test_table_parity.py
```

覆盖 77 个命令段落（保留重复菜单前缀）、15 个主题；兼容帮助声明的 ISO-8859-1 编码。此检查只证明清单完整，不能当作所有功能已实现的证据。

## 文本线既有结果

Wave 3b 已完成 **30 个合成用例 × Myers/Standard 两种策略 = 60 份原始 XML**。Myers 30/30（100%），Standard + closeness 29/30（96.67%）；唯一“不相似不强配”差异在 [KNOWN_DIFFS](../../crates/bc-core/tests/golden/KNOWN_DIFFS.md) 保存输入与双方完整输出，并由测试钉住，不能扩大成任意错误白名单。

## 正式模式

```powershell
./tools/golden/gen_expected.ps1 -ReferenceExe '<本机参考程序 exe>' -WorkerId '<当前 worker terminal handle>'
```

默认流程：

1. `build_rule_templates.ps1` 从 case.properties 生成 `profiles/*.rules.xml`，显式指定大小写、三类空白、行尾、全部 10 类元素的重要性。可独立运行该构建器，不需要参考工具。
2. 独占创建 `%TEMP%/matcha-reference-tool.lock`，写 worker id、PID、UTC 时间；争用默认最多等待 600 秒，可用 `-LockTimeoutSeconds` 设更短等待并由调用方重试。不会删除其他持有者的锁。
3. 持锁检查没有其他参考进程，备份显式的 BCSessions.xml；需要长行格式时也备份 BCFileFormats.xml（记录其原本是否存在）。不访问目录中的其他配置或任何授权/密钥文件。
4. 将 UTF-8 源模板按 BOM/UTF-16/行尾元数据转换到 `target/golden-work`；只临时写入两个命名会话 `matcha-golden-myers` / `matcha-golden-standard`。
5. 执行 `load <default>` 和 `text-report layout:xml options:display-all ... <named-session>`，原始报告直接写入各用例目录；不合并行、不改状态、不重写报告。
6. 全部自有子进程退出后，finally 恢复设置原字节；原本不存在的格式文件会删除。持锁验证恢复，最外层 finally 删除自己的锁。备份目录已 gitignore，不能提交私人会话内容。

Rust 回归只读已入库的 XML，**不调用参考工具**。全部 oracle 缺失时空语料测试仍能跳过；部分生成则报错。当前仓库的正式入口不再走跳过分支。

## 已验证的配置协议

- 参考版本 5.0.6.30713，路径由参数传入；命名 TTextCompareSession 直接位于 BCSessions 根下，不放在无名默认/历史文件夹中。
- 规则节点必须有 `Value="TTextRules"`。
- Myers：AlignKind=alignQuick、UseCloseness=False、NeverAlignDiffs=False。
- Standard：AlignKind=alignSmart、UseCloseness=True、NeverAlignDiffs=False。
- 两者 SkewTolerance=2000，使用相同重要性模板。
- CaseSig、LeadWSSig、EmbeddedWSSig、TrailWSSig 与对应 unimportant 元数据取反；CompareEndings 与 compare_endings 同值；NonWSSig=True，OrphansAlwaysSig=False。
- ElementSigs 的 item 为 `_N`，包含 ElementStr 名称和 ValueBool。Comment 可按用例设为 False，其余显式 True。并非直接的 Comment 子节点或布尔数组。

定向只读 RTTI/资源证据及正反验证见 [ADR 006](../../docs/decisions/006-myers-display-packing.md)、[ADR 007](../../docs/decisions/007-golden-reference-settings.md)。只检查了获准的对齐、普通重要性及行长属性，没有分析授权逻辑或修改安装目录。

## 超长行

`029-long-line` 保持原始 12011 字符单行。参考格式默认 CharPerLine=4096 会人工断行，因此生成器临时添加仅匹配 `.matcha-golden-long` 的格式，CharPerLine=65536，不含语法/转换器，使用字面格式名称。该格式放在匹配顺序前端但不匹配任何其他文件；检测到同名或同掩码的已有用户格式会拒绝运行。

本设置文件与会话文件使用相同独占锁、备份与恢复协议。没有缩短输入，也没有在测试中合并参考工具输出的人工行。

## 可选模式与验证

- `-ComparisonTemplates <dir>`：显式使用自有的已核实模板，缺失或结构不符时在写设置前失败。
- `-CaseNames ...`：只重生成指定用例；正式回归仍要求完整 60 份。
- `-MyersNeverAlignDifferences`：仅用于显式验证该选项；正式默认黄金不启用它。
- `-DiagnosticInheritRules -OutputRoot <非正式目录>`：继承默认比较规则的诊断模式，强制禁止覆盖正式 corpus。旧的 9 份诊断证据保留在 evidence，不属于正式白名单。

```powershell
./tools/golden/test_reference_lock.ps1
cargo test -p bc-core --test golden_align -- --nocapture
cargo test -p bc-engine-text --test golden_importance -- --nocapture
```

锁测试使用独立临时文件，不调用参考程序、不读写设置。若进程异常终止，应由协调者确认锁持有进程确已退出后再处理残留锁；不要按文件年龄擅自抢占。恢复设置前同样必须持有全局锁并关闭参考程序，使用本次时间戳备份，避免覆盖备份后用户的新改动。
