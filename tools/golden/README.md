# 黄金语料与 GUI 验证工具

## 文本黄金语料生成

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

### 正式模式

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

### 已验证的配置协议

- 参考版本 5.0.6.30713，路径由参数传入；命名 TTextCompareSession 直接位于 BCSessions 根下，不放在无名默认/历史文件夹中。
- 规则节点必须有 `Value="TTextRules"`。
- Myers：AlignKind=alignQuick、UseCloseness=False、NeverAlignDiffs=False。
- Standard：AlignKind=alignSmart、UseCloseness=True、NeverAlignDiffs=False。
- 两者 SkewTolerance=2000，使用相同重要性模板。
- CaseSig、LeadWSSig、EmbeddedWSSig、TrailWSSig 与对应 unimportant 元数据取反；CompareEndings 与 compare_endings 同值；NonWSSig=True，OrphansAlwaysSig=False。
- ElementSigs 的 item 为 `_N`，包含 ElementStr 名称和 ValueBool。Comment 可按用例设为 False，其余显式 True。并非直接的 Comment 子节点或布尔数组。

定向只读 RTTI/资源证据及正反验证见 [ADR 006](../../docs/decisions/006-myers-display-packing.md)、[ADR 007](../../docs/decisions/007-golden-reference-settings.md)。只检查了获准的对齐、普通重要性及行长属性，没有分析授权逻辑或修改安装目录。

### 超长行

`029-long-line` 保持原始 12011 字符单行。参考格式默认 CharPerLine=4096 会人工断行，因此生成器临时添加仅匹配 `.matcha-golden-long` 的格式，CharPerLine=65536，不含语法/转换器，使用字面格式名称。该格式放在匹配顺序前端但不匹配任何其他文件；检测到同名或同掩码的已有用户格式会拒绝运行。

本设置文件与会话文件使用相同独占锁、备份与恢复协议。没有缩短输入，也没有在测试中合并参考工具输出的人工行。

### 可选模式与验证

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

## 文件夹黄金与 GUI 验证工具

### 黄金采集

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/golden/gen_dir_expected.ps1 -WorkerId <本次实际 worker 标识>
cargo test -p bc-engine-dir --test golden_dir -- --nocapture
```

参数可指定 ReferenceExe / OutputRoot；默认使用已获授权的本机安装路径。
只运行目录比较、内容比较与报告命令，不运行复制/删除/同步脚本。
输入先在系统临时目录生成，随后只把小体积合成输入、manifest、XML 期望和哈希入库。

#### 跨 worker 互斥与恢复

1. CreateNew 独占创建 `%TEMP%/matcha-reference-tool.lock`，写入 owner、UTC 时间与 PID。
2. 失败最多等待 10 分钟后退出，调用方稍后重试；**不删除别人/旧 worker 的锁来抢占**。
3. 持锁后、每次调用前及配置写入前确认无其他 BCompare.exe 运行。
4. Flatten 需要唯一临时命名会话：只备份/写入 BCSessions.xml，不读取或修改
   BCFileFormats.xml、其他设置或任何授权文件。
5. 命名会话放在 BCSessions 根下；Rules 必须带 Value="TDirRules"；
   本机实测 Flatten 发布值为 State/DirHandling="dirFlatten"。
6. 每次 /solo /silent 调用按句柄等待退出；超时只停止本脚本拥有的进程。
7. finally 原样恢复 BCSessions.xml 字节及原 mtime，验证逐字节相同，再清临时目录和自有锁。
   如果恢复失败/出现外部进程，保留备份和锁并报错，不能宣称恢复成功或开始下一位 worker。
8. 不将用户配置内容纳入仓库。provenance.json 只记录程序/生成器哈希、版本、时间与 case 数。

`-CaseName` 可重放单例；`-Probe` 只跑 same。
timezone-offset 是本机参考脚本拒绝的文档例子，只有显式 CaseName 才尝试；
正常套件不制造它的期望，详见语料 KNOWN_DIFFS.md。

### 真实 native 目录基准

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/golden/bench_dir.ps1
```

脚本先构建最新 GUI，再生成两侧各 100,000 个临时文件，运行：
`matcha --bench-dir <left> <right>`。
600 个计时样本之前有 30 个 warm-up；所有行 22 px，spacing 在 show_rows 之前归零，
随机跳转覆盖完整行索引，统计使用实际绘制范围。验收为 update p99 <4 ms。
完成后退出/清理临时树；可显式 KeepFixture 供人工复查，结束后请清理它。

截图来自 egui 的 native framebuffer 回调，不依赖 PrintWindow（OpenGL 常返回黑图）。
BMP 由小型无依赖写出器生成，再由系统组件转成 PNG；不会捕捉其他窗口。

### 普通两目录启动与可交互桌面复测

```powershell
powershell -File tools/golden/gui_smoke.ps1 -Action Start
powershell -File tools/golden/gui_smoke.ps1 -Action Capture -Name folder-waveC-smoke
powershell -File tools/golden/gui_smoke.ps1 -Action Close
```

只控制当前 worktree 的 matcha.exe 和脚本自己创建的临时数据；Close 只关闭该 PID。
Click 在窗口不是前台时拒绝，不把无效鼠标消息伪装为成功。
F12 请求当前 viewport 的诊断截图；该机制不尝试解锁桌面或捕获其他应用。
当前环境物理输入受限，组件/worker 回归与 native 截图为协调者批准的等价验收；
项目负责人应在可交互桌面按 worker 报告的人工清单复测。
