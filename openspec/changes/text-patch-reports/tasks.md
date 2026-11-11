# Tasks

说明：全程约束——`cargo clippy --workspace --all-targets` 零警告、`cargo test --workspace` 通过；不新增第三方依赖（`bc-script` 仅新增对工作区已有 `windows-sys` 的 `cfg(windows)` 引用）；`bc-script`/`bc-report` 不依赖 `bc-gui`；只对自己改动的文件运行 rustfmt（仓库存在既有格式漂移，不做全量 `cargo fmt`）。每个任务组至少一个 Conventional Commits 提交。

## 1. 参考样本采集（先行，确定 patch 字节格式）

- [x] 1.1 新增 `tools/golden/gen_patch_expected.ps1`：完全沿用 `gen_expected.ps1` 的共享锁、备份、临时命名会话（`matcha-golden-myers`/`matcha-golden-standard`）与 finally 恢复流程，对 30 个两路用例 × 2 套规则 × `patch-normal|patch-context|patch-unified` 生成 `expected.<profile>.patch-<fmt>.txt`，并加采“两侧相同”“含非 ASCII 的 UTF-8”“UTF-16 输入”“无末尾换行”“带 `title:`”样本；提供不调用参考工具的 `-DryRun`；验证 `-DryRun` 列出 180 + 边界样本的目标路径且不触碰参考配置
- [x] 1.2 在本机持锁运行采集（需要本机参考程序；按 README 协议传 `-ReferenceExe`/`-WorkerId`），入库原始字节；验证每份样本非空（相同文件样本除外）、设置文件恢复校验通过、`git status` 无私人备份或二进制
- [x] 1.3 从样本归纳并写入 `tools/golden/README.md` 的“patch 报告协议”节：三种格式的文件头（路径写法、时间戳格式）、上下文行数、块头写法、无末尾换行标记、换行符、输出编码与 BOM、相同文件时的输出、`title:` 是否出现；对齐差异用例在 `crates/bc-core/tests/golden/KNOWN_DIFFS.md` 追加对应 patch 条目；验证 README 中每条结论都指向一份具体样本

## 2. 报告模型与 patch 渲染（bc-report）

- [x] 2.1 `ReportRequest` 增加 `comparison: Option<Comparison>`（`Files`/`Session`），`Output` 增加 `Stdout`；移除 crate 级 `allow(unused_variables)` 中已不再需要的部分；验证单测：`Layout::parse` 的弃用别名、`Output`/`Comparison` 构造与相等性
- [x] 2.2 实现差异块提取：从 `TextMap` 行序列得到编辑段，`ignore_unimportant` 时把不重要行当作相同，按 1.3 确定的上下文行数合并为块；验证单测：插入/删除/改动/首尾差异/相邻块合并/全同/仅不重要差异（开与关）
- [x] 2.3 实现 normal / context / unified 三个格式器（文件头、块头、行前缀、无末尾换行、换行符集中在一处），接入 `TextReportRenderer::render`（非 `patch` 布局返回 `UnsupportedLayout`）；验证单测用手写小例逐字节断言三种格式
- [x] 2.4 实现报告编码（按 1.3 的规则把渲染文本编码为字节）；验证单测覆盖 ASCII、非 ASCII UTF-8、UTF-16 输入三类样本的字节一致
- [x] 2.5 新增黄金回归测试：用与 XML 黄金测试相同的规则模板构造 `TextMap`，渲染 30 × 2 × 3 份并与入库样本比对（只归一化头部时间戳与 `target/golden-work` 路径前缀），`KNOWN_DIFFS` 登记项按登记断言；验证 `cargo test -p bc-report` 全部通过且样本缺失时报错而非跳过
- [x] 2.6 实现 `deliver`：文件目标写同目录临时文件后重命名覆盖、失败删除临时文件；标准输出目标写入调用方提供的写出器并 flush，`BrokenPipe` 作为写出失败返回；`printer`/`clipboard` 返回不支持；验证单测：覆盖已有文件、目录不存在时无残留、标准输出字节与文件字节相同、BrokenPipe 不 panic
- [x] 2.7 更新 `crates/bc-report/README.md`：patch 布局、`Output::Stdout`、本阶段不支持项；验证 README 与 `report-output`/`text-reports` 规格的范围一致

## 3. 脚本解析（bc-script）

- [x] 3.1 实现 `Script::load` 的读取与解码（`bc-core::encoding::detect`，BOM/UTF-8/ANSI 回退），读取失败映射 105；验证单测：UTF-8 BOM、UTF-16LE、含 ANSI 字节的脚本、文件不存在
- [x] 3.2 实现 `tokenize` 与 `parse_script` 的行处理：`#` 注释（引号内的 `#` 保留）、行末 `&` 续行（逻辑行号取首行）、双引号分组、空行；验证单测覆盖 script-execution 规格“脚本基本语法”的场景
- [x] 3.3 实现变量替换：`%1`…`%9`（排除 `/` 开关）、`%date%`/`%time%`/`%fn_time%`（Windows 经 `windows-sys` 的 `GetLocalTime`/`GetTimeFormatEx`，其他平台 UTC 兜底）、`%NAME%` 环境变量（大小写敏感、未定义保持原文）；为 `bc-script` 加 `cfg(windows)` 的 `windows-sys` 依赖（启用所需特性）；验证单测：参数缺失为空串、`/silent` 不计入、未定义变量原样、日期格式 `yyyy-mm-dd`
- [x] 3.4 解析错误分 `Syntax`（106）与 `Unsupported`（100）两类并带行号；`parse_arguments` 对本阶段范围外的 25 个命令（含 `file-report`）返回 `Unsupported`；验证单测：拼错命令 → 106、`load` → 100 且报出行号与命令名
- [x] 3.5 实现 `parse_report_arguments`：`layout:`（仅 `patch` 可执行，其他参考布局 → `Unsupported`，未知 → `Syntax`）、`options:`（patch 格式、`ignore-unimportant`、可接受但无效果的选项、弃用选项、未知 → `Syntax`）、`title:`、`output-to:`（文件名、`-`；`printer`/`clipboard` → `Unsupported`；缺失 → `Syntax`）、`output-options:`（`html-*` → `Unsupported`，print/wrap 接受）、比较对象（两个 → `Files`，一个/无 → `Unsupported`）；验证单测逐条覆盖 text-reports 与 report-output 规格中的参数场景
- [x] 3.6 实现 `log`、`option`（`stop-on-error`、`confirm:` 三值）、`beep` 的参数解析；验证单测：`log normal`、`log verbose append:run.log`、`option confirm:yes-to-all`、非法取值 → 106
- [x] 3.7 更新 `crates/bc-script/README.md`：本阶段支持的命令、两类错误与退出码、变量替换规则、`output-to:-` 为 Matcha 扩展；验证 README 的命令清单与 `parse_arguments` 一致

## 4. 脚本执行（bc-script runner）

- [x] 4.1 实现执行器：注入标准输出/标准错误写出器；`text-report` 读取两侧文件（按文件名选格式、默认规则与对齐）→ 渲染 → 投递；文件不可读 → 107 类失败、文件夹或二进制 → 100 类失败；按 D8 汇总退出码；`stop-on-error` 首错终止；验证单测（写出器用 `Vec<u8>`）：成功 0、缺文件 107、两条命令首条失败时有/无 stop-on-error 的差异、多条 `output-to:-` 按序拼接且无额外字节
- [x] 4.2 实现 `log`：初始关闭、默认 `Log.txt` 覆盖、指定文件、`append:` 追加、`verbose` 记录替换后的命令行；错误始终同时写标准错误；验证单测在临时目录断言日志内容
- [x] 4.3 实现 `beep`（Windows `MessageBeep`，经 `windows-sys`；其他平台写 BEL 到标准错误）；验证单测仅断言执行成功且不写标准输出

## 5. 命令行与应用接线（apps/bcompare）

- [x] 5.1 `cli` 新增 `/report=`、`/reportoptions=`、`/reportfile=` 开关并更新 `/?` 用法文本（标注 Matcha 扩展）；验证 `bc-script` 的 cli 单测：取值解析、`patch-unified` 展开、非法值报错、`/?` 文本包含三项
- [x] 5.2 路由新增 `Route::Script` 与 `Route::Report`（位于 GUI/单实例之前），`routing::unsupported` 去掉“scripts are not supported yet”、改为拒绝被转交的脚本与报告请求；验证 routing 单测：`@s.txt`、`/report=patch a b`、`/report=patch a`（报错路由）、`/solo` 不改变路由
- [x] 5.3 `main` 接线：执行前 `console::attach`；含 `output-to:-` 或无 `/reportfile` 的报告在无可用标准输出时失败（`console::tell` 呈现，退出码 100）；真实 stdout/stderr 注入执行器；验证经 `matcha.com` 的端到端测试（`apps/bcompare/tests`）：脚本到标准输出、到文件、`/report` 与等价脚本字节相同、105/106/107/100 各一例、已有常驻实例时不转交
- [ ] 5.4 更新文档：根 `README.md`（发行布局一节加脚本与 `/report` 用法）、`apps/bcompare/README.md`、ADR 301 的“命令行”说明；验证文档中的每条示例命令都在 5.3 的端到端测试或 6.1 的手工验证中实际运行过

## 6. 集成验证

- [ ] 6.1 `cargo xtask dist` 后在真实 cmd 与 PowerShell 窗口（非管道）验证：`matcha /report=patch-unified a.txt b.txt` 显示在控制台、`> file` 与管道正常、`matcha @s.txt` 的退出码；证据写入 `docs/worker-reports/evidence/text-patch-reports/`；验证证据含控制台缓冲区与退出码
- [ ] 6.2 全量回归：`cargo clippy --workspace --all-targets` 零警告、`cargo test --workspace` 通过、`openspec validate text-patch-reports --strict` 通过；验证三条命令的输出记录在 6.1 的证据目录

## Workflow follow-up

- 评审通过后归档本 change（`openspec archive text-patch-reports`），并确认 `openspec/specs/` 生成 `script-execution`、`text-reports`、`report-output` 三个主规格。
