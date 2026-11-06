# folder-compare-sync — Wave C 实施报告

## 结果与范围

**任务组 6–7 共 9 项完成**，6.1–6.6、7.1–7.3 全部勾选；
OpenSpec apply 为 **32/34**。任务组 8（全变更整体验收/合并）保持未勾选，未合并 master。

最终验证：
- `cargo clippy --workspace --all-targets -- -D warnings`：退出码 0，零 warning。
- `cargo test --workspace`：**143 passed / 0 failed / 1 ignored**。
- `cargo build --release -p bcompare`：退出码 0。
- native 两侧各 10 万文件基准：**update p99 = 0.721 ms < 4 ms**，600 个样本。
- 黄金：**28 目录对，32/33 文件行整体一致（96.969697%）**；
  **已支持范围 32/32 = 100%**；4/4 文件夹结构配对一致；
  **KNOWN_DIFFS 为 1 条**（已批准的规则比较 Unsupported）。
- 文件夹比较帮助清单 110 个命令/引用条目、同步 50 个，逐条状态均有记录。
  这不是宣称 160 个独立功能全部实现；未采纳/后置有明确理由。

最后阶段原 Orca Dispatch 返回 consumer_fenced。已按工具契约停止旧派发生命周期消息，
未重试 check 或冒发 worker_done；后续收尾依据用户直接的“continue”完成。
本报告说明本地代码/验收结果，不代表修改了旧 Dispatch 的调度状态。

## 完成清单与证据

| 项 | 实现 / 验证 |
|---|---|
| 6.1 | dir_view 以固定 22 px 行高和 show_rows 绘制左右列、中央内容结果、颜色、目录空心/黄标/红紫摘要、DOS 后缀、链接/错误提示；列头排序/右键选列、Legend；11 个 GUI tests 中有 10 万行虚拟化断言，native 基准与真实帧截图见下 |
| 6.2 | Filters/Other filters、XML 预设、13 种显示过滤（Show None 默认隐藏）、4 种比较目录显示、Suppress/Ignore Unimportant；Swap/Back/Forward/Browse/Up/Set Base/Compare Parent；子目录展开、选择/反选/限定侧、Find；子视图保留原基准，组件回归覆盖 |
| 6.3 | 路径/操作/范围/确认对话框，Yes/No/YesToAll/NoToAll 桥接 Interaction；OpLog、进度、取消、文件名剪贴板请求；后台执行不阻塞绘制；回归验证等待 UI 答复前不覆盖、取消不授权删除，路径选取用文本框+最近列表 |
| 6.4 | 同行或跨行两文件、Compare To、size/CRC/binary Quick Compare；文本原型在后台打开，再以独立浮动视图显示；跨行左右身份、CRC 与 Quick Compare 回归通过 |
| 6.5 | 五预设/自定义映射、Rules/Filters、预览/目录动作摘要、覆盖/Exclude、冲突导航、Sync Now、统计；基线保留本视图内成功同步历史，不掩盖未解决冲突；导航可展开祖先，纯展开不清覆盖 |
| 6.6 | Home 文件夹卡片与两目录拖放；两个目录参数自动选择 Folder Compare；普通 native 两目录启动实际通过，拖放原始输入回归通过；main 只增加目录 benchmark 参数与验证 |
| 7.1 | gen_dir_expected.ps1 实际运行 28 组 /solo /silent load/criteria/filter/expand/report；输入先在系统临时目录生成；Flatten 用临时根级命名会话；独占锁、无其他参考进程检查、字节备份/恢复均实际通过 |
| 7.2 | golden_dir.rs 的有界报告 XML 解析与临时物化回归；33 文件状态行、4 目录结构行；明确保留 1 行 Unsupported，不把例外冒称通过；无其他差异 |
| 7.3 | [folder-compare](../reference-parity/folder-compare.md) 与 [folder-sync](../reference-parity/folder-sync.md) 覆盖 D10 命令与选项；110/50 条命令标签核对无遗漏，另列共享参数、未采纳与后置项 |

新增测试数：GUI 11、engine view_integration 4、golden_dir 2，共 17；
基线 126 + 17 = 143。唯一 ignored 仍是 Wave A 回收站人工测试，不重复计入自动通过数。

## Native 基准与真实截图

环境：Windows x86_64 MSVC，Rust/Cargo 1.99.1，eframe/egui 0.37，release。
每侧 100 个目录 × 1,000 个 2 B 合成文件，共 **200,000 个物理文件**；
比较模型是 100,000 个配对文件 / 100,100 个展开行。

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/golden/bench_dir.ps1
```

脚本构建最新程序，生成系统临时树，运行 `matcha --bench-dir <left> <right>`，
验证输出并清理树。30 个 warm-up 后计时 600 个 App::ui 样本；
截图采集在 warm-up，图片写出由后台线程完成。没有修改 text_view.rs 或 bench.rs。

| 指标 | 交付复测 |
|---|---:|
| fixture 生成（不计入 UI 帧） | 58.995266 s |
| measured samples | 600 |
| update p50 | 0.445 ms |
| update p95 | 0.620 ms |
| **update p99** | **0.721 ms** |
| update max | 0.935 ms |
| 每次最多实际绘制行数 | 32 |
| native 总时间（含加载、warm-up、计时） | 30.942 s |
| benchmark process exit / pass | 0 / true |

spacing 在 show_rows **之前**设为零，实际绘制 range 用于 top-row 统计，
避免把 22 px 跳转映射到含额外 spacing 的错误索引。
测量为默认大小/UTC 时间列、合成小文件；不是冷盘承诺或所有列组合的统一 SLA。

- [最终 10 万文件 framebuffer](assets/folder-waveC-bench.png)
- [普通两目录启动 framebuffer](assets/folder-waveC-smoke.png)
- 本地日志：target/folder-waveC-bench-delivery.log、folder-waveC-bench.log。

最初 Win32 PrintWindow 对 OpenGL 返回黑色区域，未把黑图作为通过证据；
改用 egui native framebuffer 回调并实际读图核对。只捕捉本应用 viewport，
没有捕捉其他应用窗口，也没有尝试解锁桌面。

## 黄金采集与一致率

参考程序版本 **5.0.6.30713**。
原程序 SHA-256：
`75ea69772796c909a6659f23037f779de3399eccaa2332969f6dcc28c7bf9826`。
完整来源见 [provenance.json](../../crates/bc-core/tests/golden/dir/provenance.json)。

28 case 覆盖：
孤儿、左右较新、容差内/边界/外、DST 开关、整小时时区忽略、
大小、CRC 相同/不同、binary 同内容覆盖时间差、binary 不同保留较新方向、
DOS 属性、归档默认、包含/排除/目录/相对路径/字符集/无扩展名掩码、
Flatten、重复名称、共享相对路径重名、仅注释变化的规则比较。

语料目录 154 个文件、**27,993 B**（采集时统计，含 manifest/报告/说明/哈希）。
输入/报告行尾不做 Git 转换；CRLF 是有效 fixture 字节，不被当作尾随空白错误。

| 口径 | 结果 |
|---|---|
| 总体文件状态 | 32 / 33 = 96.969697% |
| 已支持文件状态 | 32 / 32 = 100% |
| 文件夹结构配对 | 4 / 4 |
| 已批准已知差异 | 1 行 rules-comments：原生 diff → Unsupported |
| 其他不一致 | 0 |

原生 foldercomp 没有 status 属性，因此不编造文件夹颜色一致率；
XML 文件状态也不提供完整的重要/不重要差异统计。
仅注释输入保留本机原生报告的实际 diff，没有为了符合预想改成“相似”。

生成时 CreateNew 独占 %TEMP%/matcha-reference-tool.lock，记录 owner/UTC/PID；
检查没有其他 BCompare.exe。只在需要 Flatten 时临时写 BCSessions.xml：
节点直接放根下，Rules 带 TDirRules，State/DirHandling=dirFlatten。
finally 已验证原始字节及 mtime 恢复；最终输出 settings_restored=true。
未读取/复制授权文件，未修改安装目录、BCFileFormats.xml 或其他设置文件。

文档 timezone:left+6 在该程序返回 106 Unexpected argument；该失败不生成假期望，
不进入有效 oracle 分母；timezone:ignore 已成功覆盖。此采集限制不混入候选的 1 条已知差异。
见 [KNOWN_DIFFS](../../crates/bc-core/tests/golden/dir/KNOWN_DIFFS.md)、
[ADR 105](../decisions/105-folder-golden-evidence.md)。

## 接口调整

- **bc-core 源码 / bc-jobs / bc-engine-text：零修改**；bc-core 只新增获准的目录黄金 fixtures。
  Cargo.toml/Cargo.lock 未改，第三方依赖零新增。
- UI 单 worker 独占 DirTree，latest-only Arc 快照跨线程；旧大快照由 worker 回收。
  文件 I/O、内容比较、日志/预设写入都不在绘制热路径。
- Work 携带独立 CancelToken；取消覆盖运行中和排队任务，关闭会唤醒 worker，
  不等待无意义的 recv timeout，更不会把取消确认当成 Yes。
- FolderNavigation 显式携带 compare/sync 目标；独立目录子视图保留原基准，
  新视图不等同于已经实现会话持久化/完整标签管理。
- read_entry_details/cache_details 支持 CRC/PE 版本可选列；其他 provider 字段无值就显示不可用。
- find_side 与侧限定刷新修复手动目录对齐后的名称遮蔽；重放按每侧原始路径，
  精确 Align With 优先，重载保留已建立的对齐覆盖。
- Flatten 内容缓存按左右 NodeId + 双 revision；文件操作指向显示配对的真实对侧，
  孤儿保留原相对路径，碰撞拒绝不猜目录；同步不提供 Flatten。
- 内容不同不再抹掉 newer 方向（原生黄金证明）；中央内容图标独立。
  skipped 结果不写入虚假的 content_status=Same；仅版本比较也可后台执行。
- 循环/不可读目录可见且带错误标记，不能通过反复 ScanRequired 绕过保护；
  取消确认按 Cancelled 处理。
- 本线决定集中在 [ADR 104](../decisions/104-folder-ui-and-virtual-pairs.md)。

## 合并注意事项（仅说明，不执行任务组 8）

以下行号以本报告生成时版本为准：

| 文件 | 最小接线区域 |
|---|---|
| bc-gui/src/app.rs | 15–16：两个 Box<View> 变体；22/40/60–61：bench_dir 与目录路由；74–75：标题分支；118–130：驱动/绘制；133–145：新增 Home action 所需借用与目录入口；147–157：显式 folder navigation、benchmark 收口 |
| bc-gui/src/home.rs | 10：OpenFolderCompare；65–78：文件夹卡片及两目录拖放 |
| bc-gui/src/lib.rs | 25–32：新增模块导出；43：LaunchOptions.bench_dir |
| apps/bcompare/src/main.rs | 20–23：先识别 --bench-dir，避免被旧 --bench 前缀吞掉；help 与 48–51 参数验证 |
| bc-session/src/prefs.rs | 27–43：只新增 FolderColors 及默认值，未动 Prefs 解析/其他字段 |

text_view.rs 与 bench.rs 字节未改。文件打开适配集中在新 dir_view 的
Command::OpenText / Notice::Text / text 子窗口；当前原型可 Send。
正式文本控件若使用 Rc/RefCell，合并时应只跨线程发送不可变模型/加载结果，
**不要直接把正式 GUI 编辑控件放到 worker**。

原型仍有原有能力限制；正式 Text Edit/Patch、外部关联、会话保存/只读继承、
全局快捷键/工具设置、报告/打印、SCC/远程/archive 等，按对照清单明确后置/未采纳。
不把它们写成 Wave C 已实现，也不擅自修改文本线或共享会话代码。

## 手动验收与负责人复测

| 验证类别 | 本轮状态 |
|---|---|
| native 窗口启动与实际 framebuffer 视觉核对 | 通过 |
| 10 万文件 native 绘制与完整索引跳转 | 通过 |
| egui Context::run_ui + worker 的选择/查找/过滤/确认/取消/跨行打开/同步等 | 通过 |
| **物理桌面鼠标/键盘操作** | **环境受限（采证时桌面锁定），待人工验收** |
| 系统剪贴板、原生回收站极端警告交互 | 待可交互桌面人工复测；本轮验证命令管线与 fake 回退确认 |

协调者已批准前三类作为 6.x 等价验收，明确禁止冒称物理输入通过或尝试解锁。

人工步骤（只用可丢弃的系统临时目录）：
1. 运行 tools/golden/gui_smoke.ps1 -Action Start，或 cargo run --release -p bcompare -- 两个目录。
2. Home 卡片/两目录拖放；核对同名、孤儿紫、较新红/较旧灰、属性后缀、展开/折叠。
3. 点击列头/右键选列、Legend；测试掩码、Other size 过滤的一侧 teal、Suppress、
   Ignore Unimportant、三种普通目录显示及 Flatten。
4. Ctrl/Shift 选择、Select All 后 Shift 左右限定侧；Find 的 case/whole/regex/wrap/up/side；
   Back/Forward、Up、Set Base、Compare Parent 新视图。
5. 在临时文件上 Copy/Move/Exchange/Rename/Touch/Attributes；分别选 Yes/No/All，
   核对覆盖较新/只读提示；取消较大操作，检查已完成部分、日志与局部刷新。
6. 同行/跨行 Open、Compare To、Quick Compare；核对左右实际路径；子视图关闭后原基准仍在。
7. Sync 五预设/自定义、逐项 Leave/Copy/Delete/Exclude、Next/Previous Conflict、Sync Now；
   完成一次后修改两侧再刷新，核对基线冲突。只用临时树测试镜像删除。
8. 检查 Copy Filename 的系统剪贴板、日志保存与重新打开后的过滤预设。
9. tools/golden/gui_smoke.ps1 -Action Close 只关闭自有 PID 并清理该临时树。

## 提交

均含 Co-Authored-By: pi <noreply@pi.dev>。

| 范围 | commit |
|---|---|
| 任务组 6 主实现 | 76806ddddd1d147aa88e945f65456ed37fe942fe |
| 侧限定选择/对话框默认值补充 | a75d8dcdc7d835883f025adb7fbeb186033d733c |
| 独立文件夹子视图补充 | 0642ad02754294bf231ffff19aa8fb9da03ed55c |
| 任务组 7 黄金与对照文档 | 5cfc851859fccf03175f69afd48b81099b69f4a5 |

本报告及最终截图另作 docs 提交。未进行 merge/rebase/archive，任务组 8 保持待办。

## 最终输出（最后 10 行；不足则全文）

### clippy --workspace --all-targets -- -D warnings

```text
    Checking bcompare v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\apps\bcompare)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.83s
```

### cargo test --workspace

```text

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests bcomp

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### cargo build --release -p bcompare

```text
   Compiling bc-gui v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-gui)
   Compiling bcompare v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\apps\bcompare)
    Finished `release` profile [optimized + debuginfo] target(s) in 6.73s
```
