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

## patch 报告协议

`gen_patch_expected.ps1` 是 `gen_expected.ps1 -Layout patch` 的薄封装：锁、备份、两个临时命名会话与按字节恢复完全相同，只是每个用例改为执行 `text-report layout:patch options:patch-<normal|context|unified> output-to:<文件> "matcha-golden-<profile>"`。`-DryRun` 只列出目标路径，不取锁、不碰参考设置。

```powershell
./tools/golden/gen_patch_expected.ps1 -DryRun
./tools/golden/gen_patch_expected.ps1 -ReferenceExe '<本机参考程序 exe>' -WorkerId '<本次运行标识>' -LockTimeoutSeconds 30
./tools/golden/gen_patch_expected.ps1 -CorpusRoot crates/bc-core/tests/golden/text/patch-extra -ReferenceExe '<…>' -WorkerId '<…>'
```

语料：`text/2way` 30 例 × 2 套规则 × 3 种格式 = 180 份，另加 `005` 的 `title:` 样本与 `013/016/017/018/019/030` 的 `ignore-unimportant` 样本（`expected.<profile>.patch-unified.ignore-unimportant.txt`）共 13 份；`text/patch-extra/101–104` 是只为 patch 设计的多差异块探针（不进入 30 例 XML 回归，避免改变其“恰好 30 例”断言），各 6 份。2026-11-11 采集，参考进程退出码均为 0，每次运行后设置按字节恢复校验通过。

由样本归纳的格式（每条后括号为证据样本，路径相对 `crates/bc-core/tests/golden/text/`）：

- **整体风格**：GNU diff 的三种输出格式（`2way/005-insert-middle-three/expected.myers.patch-{normal,context,unified}.txt`）。
- **换行与编码**：每行以 CRLF 结尾，包括文件头（`2way/005…/expected.myers.patch-unified.txt`）；内容统一写成 UTF-8，与输入编码无关——UTF-16 输入也输出 UTF-8（`2way/025-utf16le-versus-utf8/…patch-unified.txt`）。
- **BOM**：任一输入带 BOM 时输出以 UTF-8 BOM 开头（左侧 BOM：`2way/024-utf8-bom/…`；两侧 BOM：`2way/027-utf16-endianness/…`；UTF-16 左侧：`2way/025…`、`026…`），否则无 BOM，包括含非 ASCII 的无 BOM UTF-8 输入（`2way/028-unicode-change/…`）。只有右侧带 BOM 与 ANSI 代码页输入**未采样**，按“任一侧”实现并在此标注未验证。
- **normal 文件头**：一行 `diff -r <左路径> <右路径>`，路径为会话里的绝对路径、反斜杠、不加引号（`2way/005…/expected.myers.patch-normal.txt`）。路径含空格时是否加引号**未采样**。
- **context/unified 文件头**：`*** <左路径>\t<时间>` + `--- <右路径>\t<时间>`（context），`--- <左路径>\t<时间>` + `+++ <右路径>\t<时间>`（unified）；时间为文件修改时间的本地时间 `YYYY-MM-DD HH:MM:SS.000000000 +hhmm`，小数秒恒为 9 个 0（`2way/005…/expected.myers.patch-{context,unified}.txt`）。
- **两侧无差异**：仍输出文件头，没有任何差异块（`2way/001-identical/…`、`2way/002-empty-both/…`）；仅最后一行有无换行不同也算无差异，且从不输出 `\ No newline at end of file`（`2way/022-final-newline/…`）；仅行尾 CRLF/LF 不同同样无差异（`2way/021-crlf-versus-lf/…`）。
- **上下文行数为 6**（不是 GNU 默认的 3）：差异块从首个差异前 6 行开始、到末个差异后 6 行结束，在文件开头处截断（`patch-extra/103-hunk-edges/…`：差异在 11/25/40 行，块为 5–17、19–31、34–46）。
- **合并规则**：相邻两处差异之间不变行 ≤ 12（= 2 × 6）时合并为同一块，≥ 13 时分块（`patch-extra/102-hunk-gaps/…` 间隔 8/9/12 合并、20 分开；`patch-extra/103…` 间隔 13、14 分开）。
- **文件末尾**：末个差异之后剩余不变行 ≤ 12 时，该块一直延伸到文件末尾；≥ 13 时只带 6 行（`patch-extra/102…` 剩 12 行 → 块至第 70 行；`patch-extra/104-hunk-tail/…` 剩 13 行 → 只到 27+6=33）。文件开头没有对应规则（`patch-extra/103…` 首个差异前 10 行，块仍从第 5 行开始）。
- **unified 块头**：`@@ -<起>,<行数> +<起>,<行数> @@`，行数为 1 时省略 `,1`（`2way/013-comment-change/…`：`@@ -1 +1 @@`），空范围写 `<起>,0` 且起始为前一行（`2way/003-empty-left/…`：`@@ -0,0 +1,2 @@`）；块内先列全部 `-` 行再列 `+` 行（`2way/012-unrelated-replacement/…`），行前缀为 ` `/`-`/`+` 后直接接正文。
- **context 块**：每块以 `***************` 开头，`*** <范围> ****` 与 `--- <范围> ----` 两段；范围为 `起,止`，单行只写一个数，空范围写 `0`（`2way/003…`：`*** 0 ****`）；行前缀 `  `、`- `、`+ `、`! `（两个字符）；某一侧只有上下文时该侧只写范围行、不列内容（`2way/005…` 左侧、`2way/006-delete-middle-three/…` 右侧）；同一差异段两侧都有行时两侧都用 `! `（`2way/012…`、`2way/013…`）。
- **normal 块**：`<左范围><a|c|d><右范围>`，范围为 `起,止` 或单行一个数，`a` 的左范围与 `d` 的右范围写前一行号（`2way/003…`：`0a1,2`；`2way/004-empty-right/…`：`1,2d0`）；左行 `< `、右行 `> `，`c` 块两侧之间一行 `---`（`2way/012…`）；不带上下文，每个差异段各成一块（`patch-extra/102…`）。
- **不重要差异**：默认按普通差异输出（`2way/013…patch-normal.txt`）；`ignore-unimportant` 时仅含不重要差异的段不输出，结果只剩文件头（`2way/013、016、017、018、019、030 …patch-unified.ignore-unimportant.txt`）。
- **`title:`**：不出现在纯文本 patch 中，输出与不带标题时逐字节相同（`2way/005…/expected.myers.patch-unified.title.txt` 对比 `expected.myers.patch-unified.txt`，仅时间戳相同）。
- **两套规则**：去掉文件头时间戳后，`2way` 全部 30 例与 `patch-extra` 4 例的 myers/standard 输出逐字节相同（含 XML 中唯一对齐差异用例 `2way/012…`：参考在两套规则下都按“改动”输出）。

回归测试只归一化两处：文件头里的时间戳，以及 `D:\Github\Matcha\target\golden-work\` 这个采集时的工作目录前缀；其余字节严格比较。

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
