# Design

## Context

- 见 proposal.md 的 Why。现状：`bc-script` 的 `parser`（`parse_script`/`tokenize`/`parse_arguments`/`parse_report_arguments`）与 `runner`（`Script::load`/`run`/`execute`/`log`）是 `todo!()`；`bc-report` 有完整的请求模型（`ReportRequest`、`Layout`、`ReportOptions`、`Output`、`OutputOptions`），但 `TextReportRenderer::render` 与 `deliver` 是 `todo!()`，且请求里没有“比较对象”，`Output` 没有标准输出。
- 已有可复用件：`/qc` 已在 `matcha.exe` 内无界面运行（`routing::Route::QuickCompare`、`runner::quick_compare_files`），输出走 `apps/bcompare/src/console.rs`（附着父控制台、重定向保持、无通道时消息框）；`bc-engine-text::TextMap` 提供对齐后的行（左右行号、`DiffKind`、`Importance`、所属差异段）；`bc-core::encoding::{detect, decode}` 做 BOM/UTF-8/代码页识别。
- 参考依据：`target/reference-help/scripting_reference.html`、`scripts.html`（只读解包的帮助）；黄金语料 `crates/bc-core/tests/golden/text/2way/`（30 例 × `myers`/`standard` 两套规则，已有参考 `layout:xml` 输出）与 `tools/golden` 的参考工具锁协议。

## Goals / Non-Goals

**Goals:**
- 一套解析器 + 执行器，后续 `session-and-cli` 只需补命令；一套报告投递，后续布局/HTML/剪贴板只需补渲染与目标。
- `/report` 与等价脚本走同一条执行路径，保证逐字节一致。
- patch 三种格式以参考样本逐字节为准，可回归。

**Non-Goals:**
- 不实现脚本状态窗口或任何 GUI；不改变 GUI 进程的单实例行为。
- 不在本 change 解析参考其他命令的完整参数语法（只识别关键字并整体拒绝）。

## Decisions

### D1 脚本与 `/report` 在 `matcha.exe` 内执行，与 `/qc` 同一模式
新增路由 `Route::Script` 与 `Route::Report`，排在 GUI 与单实例之前；常驻实例收到转交请求时一律拒绝这两类调用（与 `/qc` 相同）。
- 备选：在控制台启动器 `matcha.com` 里执行。否决：启动器刻意不依赖引擎（无头构建机可用），且从快捷方式/资源管理器运行脚本不经过启动器。

### D2 两阶段：先整体解析校验，再执行
`parse_script` 产出全部 `ScriptLine` 后才开始执行，满足规格“执行前整体校验”。解析错误分两类：`Syntax`（未知命令、参数不合法 → 106）与 `Unsupported`（参考命令但本阶段不支持、或不支持的布局/目标/HTML → 100），各带行号。`CommandKind` 已列全 29 个关键字，`parse_arguments` 对不在范围内的种类直接返回 `Unsupported`，不必为它们写参数语法。
- 备选：逐行解析逐行执行（参考工具的行为更接近这样）。否决：会在拒绝前产生部分副作用（已写出的报告），与规格冲突；且本阶段可拒绝的命令很多，整体拒绝更可预期。

### D3 报告请求补齐“比较对象”与标准输出
`ReportRequest` 增加 `comparison: Option<Comparison>`（`Files(PathBuf, PathBuf)` | `Session(String)`），`Output` 增加 `Stdout`（脚本写法 `output-to:-`）。解析器按参考语义区分：两个名称 → `Files`，一个 → `Session`，无 → `None`；后两者在本阶段由解析器判为 `Unsupported`。路径在执行时按进程当前目录解析（脚本与快捷方式一致）。
- 备选：把比较对象放在 `Command::TextReport` 旁边而不进 `ReportRequest`。否决：所有 `*-report` 共用这组“通用报告参数”，放进请求模型避免每个命令各自携带。

### D4 patch 渲染：统一的差异块模型 + 三个格式器
渲染分两步：(1) 从 `TextMap` 的行序列提取“编辑段”（连续的左删/右增/改动行区间，`ignore-unimportant` 时把 `Importance::Unimportant` 的行当作相同），再按上下文行数合并为差异块；(2) 三个格式器把差异块写成 normal（`NaM`/`NcM`/`NdM` + `<`/`>`/`---`）、context（`***`/`---` 文件头与 `***************` 块）、unified（`---`/`+++` 文件头与 `@@` 块）。上下文行数、文件头内容（路径写法、时间戳格式）、“无末尾换行”标记、换行符与编码一律由黄金样本确定，格式器里集中为常量/小函数，样本更新时只改一处。
- 备选：绕过 `TextMap` 自己跑一遍 LCS 生成 diff。否决：报告必须与文本视图的对齐/重要性一致（规格“比较语义”），而且已有 Myers 30/30 的对齐回归可直接继承。

### D5 渲染与投递分离；先渲染完整字节再投递
`Renderer` 产出完整的 `Report`，再按字符编码规则编码为字节，`deliver` 只负责写出：
- 文件：写到同目录临时文件后重命名覆盖，失败时删除临时文件，满足“不留下不完整的报告文件”。
- 标准输出：执行器不直接调用 `std::io::stdout()`，而是持有调用方注入的 `&mut dyn Write`（应用层传入真实 stdout，测试传入 `Vec<u8>`），写完 `flush`。错误与日志同样经注入的 stderr 写出器，保证报告与诊断分流。
- 应用层在执行任何写到标准输出的命令前用 `console::attach` + `has_stream(Info)` 判断是否有可用标准输出；没有则该命令失败（退出码 100），错误经 `console::tell` 呈现（无控制台时即消息框）。
- 备选：投递时再逐行写出（省内存）。否决：报告体积与比较输入同阶，文本比较本就整份载入；整体渲染让“失败不留残缺文件”与“stdout 与文件字节相同”都简单可证。

### D6 脚本读取与变量替换
- 脚本字节用 `bc-core::encoding::detect`（BOM → UTF-8/UTF-16；无 BOM 时 UTF-8，含非法序列则按系统 ANSI 代码页）解码。
- 替换在分词前对整行做：`%1`…`%9`（脚本名之后、不以 `/` 开头的参数）→ `%date%`/`%time%`/`%fn_time%` → `%NAME%` 环境变量（大小写敏感，未定义保持原文）。本地时间与区域时间格式在 Windows 上经 `windows-sys`（`GetLocalTime`、`GetTimeFormatEx`）取得；`bc-script` 新增对已有工作区依赖 `windows-sys` 的 `cfg(windows)` 引用，非 Windows 用 UTC 与 `HH:MM:SS` 兜底。
- 备选：沿用 GUI 文件夹视图取本地时钟的方式。否决：它通过启动外部进程取时钟，不适合无界面脚本。

### D7 `/report` 快捷方式即“合成一行脚本”
`cli` 新增开关 `/report=<布局>`、`/reportoptions=<选项>`、`/reportfile=<路径>`；应用层把它们与两个位置参数合成一个 `ReportRequest`（布局 `patch-unified` 等展开为 `layout:patch` + 对应选项），交给与脚本相同的执行器执行一条 `TextReport` 命令。选项与布局的合法性检查复用解析器的同一函数，因此报错文字与退出码也与脚本一致。

### D8 退出码汇总
执行器记录每条命令的结果：全部成功 → 0；存在“无法读取比较对象” → 107；否则存在其他运行时失败 → 100。`option stop-on-error` 在首个失败处终止。加载失败 105、语法 106、不支持 100 在执行前确定。快捷方式等价于单命令脚本。

### D9 黄金样本采集与回归
新增 `tools/golden/gen_patch_expected.ps1`，完全沿用 `gen_expected.ps1` 的锁、备份、临时命名会话与恢复流程，对 30 个两路用例 × 两套规则（`matcha-golden-myers`/`matcha-golden-standard`）× 三种格式执行 `text-report layout:patch options:patch-<fmt> output-to:<用例目录>\expected.<profile>.patch-<fmt>.txt <会话名>`，原始字节入库；另采集“相同文件”与“非 ASCII/UTF-16 输入”样本以确定编码。Rust 回归读入库样本，用与 XML 黄金测试相同的规则模板构造 `TextMap` 后渲染比对；时间戳与绝对路径在比对前归一化。已登记于 `KNOWN_DIFFS.md` 的对齐差异用例沿用同一登记，不另开白名单。

## Risks / Trade-offs

- [参考 patch 头部含时间戳/本机路径，难以逐字节比] → 比对前用固定规则归一化两处（只替换头部行中的时间戳与 `target/golden-work` 前缀），其余字节严格比较；归一化规则写在测试里并注明来源样本。
- [报告编码可能随输入编码或系统代码页变化] → 专门采集非 ASCII 与 UTF-16 输入样本后再定编码规则；未确认前不对外承诺除 ASCII 外的字节一致。
- [解析器整体拒绝会让“含 load 的参考脚本”在 Matcha 上完全不跑] → 错误信息给出行号与命令名，并在 README 说明本阶段的命令范围；这是有意的可预期性取舍。
- [标准输出写入大报告时被下游管道提前关闭（如 `| more` 退出）] → 视为写出失败（退出码 100），不 panic；`BrokenPipe` 不额外打印堆栈。
- [`/report` 是 Matcha 扩展，未来参考工具若增加同名开关可能冲突] → 名称集中在 `cli` 的一处，帮助文本标注为 Matcha 扩展。

## Migration Plan

纯新增能力，无数据迁移。`/qc` 与 GUI 行为不变；`routing::unsupported` 里的“scripts are not supported yet”分支被新路由取代。回滚即撤销本 change 的提交。

## Open Questions

- 参考 patch 报告在 Windows 上的换行符（CRLF/LF）与无末尾换行的写法：由 D9 采集确定，不影响接口与任务划分。
- `%time%` 在区域设置为 12 小时制时是否带 AM/PM：由 `GetTimeFormatEx` 默认格式决定，若与参考不同再以样本校正。
