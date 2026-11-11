# Proposal

## Why

命令行已经能不开窗口地判断两个文件是否相同（`/qc`），但还拿不到“差在哪里”：脚本模式（`@script` + `text-report`）和报告渲染（`bc-report`）仍是 `todo!()`。CI、代码审查和批处理需要把文本差异以 patch 形式打印到终端或写入文件，而且全程不出现 GUI。上一轮已把 GUI/控制台拆成 `matcha.exe` + `matcha.com`（ADR 301），输出与退出码通道已就绪，现在补上内容本身。

## What Changes

- 脚本模式无界面执行：`matcha @script.txt [args]` 在发起进程内运行，不开窗口、不转交常驻实例；`matcha.com` 下等待、输出与退出码沿用 ADR 301 的通道。
- 脚本语法核心：一行一条命令、关键字不区分大小写、`#` 注释、行尾 `&` 续行、双引号参数、`%1`…`%9` 命令行参数、`%NAME%` 环境变量、`%date%`/`%time%`/`%fn_time%` 动态变量；语法错误返回 106，脚本无法读取返回 105。
- 本 change 支持的脚本命令：`text-report`、`log`、`option`、`beep`。其余参考命令（含 `file-report`：参考工具中它只有 side-by-side 与 summary 布局）能被识别，但执行前即以“尚不支持”整体拒绝，不执行任何一行。
- 文本报告 `patch` 布局：`patch-normal`（默认）、`patch-context`、`patch-unified` 三种格式，支持 `ignore-unimportant` 与 `title:`；比较对象为脚本行末的一对文件名（`<comparison>`）。格式以参考工具采集的黄金样本为准。
- 报告输出：`output-to:<文件名>` 写文件；Matcha 扩展 `output-to:-` 写到标准输出（可重定向、可管道）。`printer`、`clipboard` 与 `output-options:html-*` 在本 change 明确报“不支持”。
- 命令行快捷方式（Matcha 扩展，免写脚本）：`matcha /report=patch[-normal|-context|-unified] [/reportoptions=<options>] [/reportfile=<path>] <left> <right>`，默认输出到标准输出。
- 黄金语料：沿用 `tools/golden` 的参考工具锁协议，采集三种 patch 格式在现有 30 个两路文本用例上的原始输出并入库；Rust 回归只读入库样本。
- 不在本 change：side-by-side / interleaved / summary / statistics / xml 布局，HTML 输出，打印与剪贴板，folder-report 及 `load`/`expand`/`select`/`filter`/`criteria` 等文件夹脚本命令，表格/十六进制等其他报告，`<comparison>` 为已保存会话名，脚本状态窗口（Matcha 始终无界面运行脚本）。

## Capabilities

### New Capabilities

- `script-execution`: 脚本文件的加载、语法、参数与变量替换、无界面执行、日志与选项、命令支持范围以及退出码。
- `text-reports`: 文本比较报告——`text-report` 的参数、比较对象、`patch` 布局三种格式与报告选项，以及 `/report` 命令行快捷方式。
- `report-output`: 报告的投递目标（文件、标准输出）与不支持目标的处理，以及输出编码与写入语义。

### Modified Capabilities

（无。仓库尚无主规格；`text-compare-core` 的对齐与重要性语义原样复用，不改变其要求。）

## Impact

- 代码：`crates/bc-script`（`parser`、`runner`、`command` 的报告参数与比较对象、`cli` 新开关）、`crates/bc-report`（`ReportRequest` 增加比较对象与标准输出目标、`TextReportRenderer` 的 patch 布局、`deliver`）、`apps/bcompare`（脚本与 `/report` 路由，沿用 `console` 输出通道）。
- 测试与语料：`crates/bc-core/tests/golden/text/2way/*/expected.patch-*.txt`、`tools/golden` 新采集脚本；`apps/bcompare/tests` 端到端用例（经 `matcha.com`）。
- 依赖：不新增第三方依赖。
- 路线图：这是 `session-and-cli`（脚本引擎）与 `hex-and-reports`（报告）的首个纵向切片；本 change 定下的解析器、执行器与报告投递接口供两者后续扩展，不另起一套。
