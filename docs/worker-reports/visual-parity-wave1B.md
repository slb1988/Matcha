# visual-parity Wave 1B 返工报告

## 结论与已确认范围

- 任务组 2.1–2.5 完成。接手时遗留代码有真实编译错误（PNG IHDR 数组类型、egui 0.37 `Context::run`）；已重写交互占位和快照接线，未原样提交。
- 2026-11-10 串行恢复后的协调者范围：UI 跟随系统消息字体、编辑器 Consolas 10pt；先接入现有 tokens；图标优先文本/文件夹；快照保留四场景，删除 `table-basic`；不重排真实视图布局。不是原始五场景/四视图全量像素验收。
- **clippy 全工作区零警告、workspace 421 tests passed；19 个 chrome 单测通过。** 触及的 Rust 文件 rustfmt 零差异。全仓 fmt 仍有 39 个历史文件差异，协调者明确不授权全仓格式化。
- **表格 A/B p99 +15.91% 超过 10% 阈值**，协调者接受为本波暂留项（消息 `msg_e4ea5c6650eb`）：绝对 p99 1.224 ms <4 ms，Wave 2 文本视图重绘时复测；不声称性能完全达标。文本 +8.61% 达标。

## 完成与证据

| 项 | 实现及观察 |
|---|---|
| 2.1 | `fonts::install` 查询 SPI_GETNONCLIENTMETRICS 的 lfMessageFont；系统实际返回 `Microsoft YaHei UI`, 12 px, `C:/WINDOWS/Fonts/msyh.ttc`（TTC index 1）。Consolas 10pt=13.333 px；微软雅黑 CJK 与内置字体兜底；缺失路径和族顺序单测。 |
| 2.2 | `Theme::apply` 扁平一像素边框、零圆角、经典选择、收紧间距；真实 tokens JSON 已合入，实际引用条目逐项断言，不再是字符串存在性检查。 |
| 2.3 | 菜单 Alt 助记符单字下划线、鼠标开关/外部关闭、方向键/Enter/Esc、嵌套子菜单、禁用项、Check/Radio/右侧快捷键；工具栏互斥切换/独立切换/分体与 Dropdown 独立返回；路径历史、分栏状态、拖动 splitter、简易 scrollbar。 |
| 2.4 | 34 个原创 Painter 几何图标，无文本占位；每项矢量绘制且形状指纹互不相同。目录展示 16px/12px/禁用态。既有额外图标保留，不再扩展其他视图。 |
| 2.5 | 真实 TextCompareView/DirCompareView、正式 corpus、文件夹展开预置；后台 ready + 两个完整静止帧，观察器不替换 eframe 重绘回调，仅排除 harness 自己的采样唤醒。PNG 落盘后退出，错误/超时非零；默认客户区 1384×961，1000×700 实测成功。 |

四场景 `snap-matcha.ps1 -All -Verify` 连续两次 SHA256 一致；均为 1384×961 PNG：

| 场景 | SHA256（两次相同） |
|---|---|
| icon-catalog | `13E4CBFA84791A6C55908436547AE819D27BA307148397E837324D78966DA416` |
| chrome-demo | `EE4F39217049163CE729F008AC64A0701F04B6F1F4C63C1545E022C0EFE860F1` |
| text-basic | `72CFFC3B0EAB6E0467F9814A91454BD06AF1C173A87741F9D63AA8EADD9865A7` |
| folder-basic | `ED207E3C6B5864D1F3DF3F8E45C2D7A6A9B8ECE5D911C9E1E706060385060D8E` |

审阅图（已打开检查）：[图标目录](assets/visual-wave1B-icon-catalog.png)、[组件演示](assets/visual-wave1B-chrome-demo.png)、[真实文本视图](assets/visual-wave1B-text-basic.png)。真实文本仍保留原按钮布局，文件夹仍保留原树；这是本波边界，不是套演示页伪装真实快照。

命令：`cargo run --release -p bcompare -- --snapshot=<scene>`；`tools/visual/set-corpus-mtime.ps1`；`tools/visual/snap-matcha.ps1 -All -Verify`。默认输出 `target/visual/matcha/<scene>.png`；`--snapshot-out`、`--window-size=WxH` 可覆盖。移除场景 `--snapshot=table-basic` 实测 exit 1，stderr `unknown snapshot scene: table-basic`。

## 组件 API（Wave 2/3 直接接线）

| 模块 | 数据、调用、返回值 |
|---|---|
| fonts | `FontPlan::from_dir` / `load` 供回退测试；`install(ctx)` 自动读消息字体；`snapshot_scale(ctx)` 固定 1.0。 |
| theme | `Theme::default().apply(ctx)`；颜色、菜单/工具栏/状态栏/编辑行高/字号字段。常量镜像 tokens，不在运行时读仓库 JSON。 |
| menubar | `MenuSpec { title, mnemonic, items }`；`MenuItem { label, mnemonic, shortcut, kind, enabled, action_id }`；kind 为 Action/Check/Radio/Submenu/Separator。`show(ui,&menus) -> Option<String>`。禁用叶子不触发；快捷键列是展示文本，全局快捷键仍由视图负责。 |
| toolbar | `ToolItem { label,action_id,kind,enabled,selected,icon,group }`；Normal/Toggle/Split/Dropdown；`show -> ToolbarResponse { clicked,dropdown }`。`apply_toggle(&mut items,id)`：Some(group) 同组互斥，None 独立切换。 |
| pathbar | `show_with_history(ui,&mut path,&history) -> PathbarResponse { action,submitted }`；action 为 path-submit/history-select/history/browse/save。调用者负责 I/O 和历史持久化，左右栏使用 `push_id`。 |
| statusbar | `StatusField::fixed(text,width)` / `flexible(text)`，可设 `icon`；`show` 等分剩余宽度并绘凹陷边界。 |
| splitter | `vertical(ui,id) -> Response`，调用者使用 `drag_delta` 更新左右宽度。 |
| scrollbar | `show(ui,min..=max,&mut first_row) -> Response`；箭头、轨道和滑块定位，值被约束在范围内。 |
| icons | `Icon::ALL`, `Icon::label`, `paint(painter,rect,icon)`。 |
| snapshot | `REGISTRY: [Scene;4]`，含 view/preset；`prepare_options` 绑定正式语料，`Capture` 负责静止帧与 PNG。 |

```rust
let result = chrome::toolbar::show(ui, &tools);
if let Some(id) = result.clicked {
    chrome::toolbar::apply_toggle(&mut tools, &id);
    // match id.as_str() { "swap" => existing_swap(), ... }
}
if let Some(id) = result.dropdown {
    // 打开此动作的菜单，不执行主体动作。
}
if let Some(id) = chrome::menubar::show(ui, &menus) {
    // 分发到现有视图命令；不要把业务动作写进 chrome。
}
```

## token 初值与已接入真源

种子坐标为原始整窗（1400×1104，顶部标题栏约 31px）；正式点坐标为 `docs/visual-parity/reference/text-basic.png` 客户区。初值注释语义：seed, calibrate in group 3；本轮只做便宜的已测 token 接入。

| Theme 字段 | 种子初值/取样坐标 | 当前 tokens 项、值、客户区取样点 |
|---|---|---|
| panel_fill | #F0F0F0 @(500,500) | `panel` #F0F0F0 @[10, 30] |
| window_fill | #FFFFFF @(25,35) | `menubar` #FFFFFF @[500, 10] |
| border | #A0A0A0 @(60,500) | `separator` #A0A0A0 @[400, 123] |
| text | #000000 @(20,45) | `textForeground` #000000 @[93, 125] |
| selected_fill | #CCE8FF @(160,68) | `toolbarToggle` #CCE8FF @[150, 35] |
| selected_border | #99D1FF @(153,65) | `toolbarToggleBorder` #99D1FF @[145, 35] |
| difference_fill | #FFE3E3 @(110,238) | `diffRow` #FCE6E6 @[1000, 205] |
| difference_text | #FF0000 @(125, 233)（种子红字核心） | `diffForeground` #FC3030 @[766, 202] |

度量：菜单 27px（种子 y31–57；客户区 y0–26）；工具栏 43px（种子 y58–100；客户区 y27–69）；图标槽16×16（正式 x167–182,y32–47）；编辑行高15（正式文字起点 y128/143/158）；状态栏从种子约24校正为25（正式 y936–960）。UI 12px/编辑器13.333px来自原生字体与 Options 证据，不谎称按颜色像素测得字号。字号与行高后续按真实视图校准，现有视图硬编码字号尚不在本波重排。

## A/B（同一机器、同一语料，600 帧+30预热）

A=`c4ead65`，独立临时基线 `target/visual/baseline-c4ead65` 构建；B 为本报告实现（`4d7b353`，格式化不改变基准二进制语义）。文本 `tmp/{left,right}.txt` 各110,000,043 bytes；表格 `target/table-wave7/audit/inputs/table-{left,right}.csv` 各69,258,868 bytes。串行顺序第一轮 A→B，第二轮 B→A，第三轮 A→B；测量期间无参考工具、编译或测试并行。首轮超阈值后依协调者要求补到三轮，未筛掉任何一轮。

| update p99 ms | A 三轮 | B 三轮 | A/B 中位数 | 变化 |
|---|---|---|---|---|
| text | [1.208, 1.277, 1.167] | [1.405, 1.311, 1.312] | 1.208 / 1.312 | +8.61% |
| table | [1.056, 0.989, 1.172] | [1.291, 1.224, 1.172] | 1.056 / 1.224 | +15.91% |

表格均值中位数 0.634→0.661 ms，p50 0.592→0.591 ms；文本均值 0.655→0.673 ms，p50 0.556→0.560 ms。表格 p99 超10%已获暂留，不进行本波热路径改动。日志：`target/visual/wave1b-transaction/bench-{text,table}-{baseline,modified}-{1,2,3}.log`。

## fmt / clippy / test 与依赖

触及文件：`rustfmt --edition 2021 --check --config skip_children=true <owned files>` exit 0，stdout/stderr 空；全仓 `cargo fmt --all -- --check` exit 1，仅未触及历史文件（下附清单），协调者不授权全仓 fmt。

`cargo clippy --workspace --all-targets -- -D warnings` exit 0，最后十行：
```text
    Checking bc-engine-table v0.1.0 (D:\Github\Matcha\crates\bc-engine-table)
    Checking bc-session v0.1.0 (D:\Github\Matcha\crates\bc-session)
    Checking bc-report v0.1.0 (D:\Github\Matcha\crates\bc-report)
    Checking bc-gui v0.1.0 (D:\Github\Matcha\crates\bc-gui)
    Checking bc-script v0.1.0 (D:\Github\Matcha\crates\bc-script)
    Checking bcomp v0.1.0 (D:\Github\Matcha\apps\bcomp)
    Checking bcompare v0.1.0 (D:\Github\Matcha\apps\bcompare)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.96s
```

`cargo test --workspace` exit 0（421 passed，无失败），最后十行：
```text
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests bcomp

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```

`cargo fmt --all -- --check` 最后十行（已触及文件检查另行 exit 0）：
```text
 pub fn header() -> XmlHeader {
-    XmlHeader { version: 2, min_version: 2, comment: None }
+    XmlHeader {
+        version: 2,
+        min_version: 2,
+        comment: None,
+    }
 }

 /// Parse a complete `BCSessions.xml` document.
```

授权 lint：demand.rs map_or→is_none_or、ReadOptions 初始化；inference.rs 删除冗余 into_iter，too_many_arguments 带热路径理由 allow；store.rs large_enum_variant 带避免分配理由 allow（未 Box 化）。追加授权：demand_proto.rs 未使用变量/多余 cast；demand_access.rs mut/lifetime；mixed_store.rs 默认字段初始化/from_ref；GUI lazy_tests.rs 删除未使用导入与变量。只做机械修复和相应文件格式化，不删除/放宽原断言。

依赖授权：`image = =0.25.10` PNG-only 直接依赖、`serde_json` workspace dev-dependency。Cargo.lock 前后 `(name,version)` 集合差为 **空集**；`dependency-tree.txt` 与 `dependency-delta.txt` 留证，没有新增 crate 版本。

## 已知限制与合并注意

- 本波只建立基础组件；真实文本/文件夹的菜单迁移、头信息、差异纹理、字号行高、UTC/本地时间显示的完整校准留后续。chrome-demo 是交互组件审阅页，不是参考布局全量复刻。
- egui 灰度字形与 Windows ClearType 不同；非100% DPI只保证可用。滚动条为简易自绘，缺原生长按箭头重复和 Win32 精确滑块度量；splitter 只返回拖动量。菜单阴影、组合框不是原生控件。
- SPI 查询通过一次隐藏 PowerShell/C# PInvoke，保留 Rust unsafe 禁令，但增加启动阶段开销；PowerShell被禁用/字体未注册时回退。快照跨机器/不同系统字体版本不保证字节一致。
- 保留当前 worker 原有未提交源码的完整归档；回滚实验只在副本运行，未回滚 live master。未读取其他工作树或 ObsidianVault，未启动参考工具；A/B基线是本任务新建 target 下的允许临时 worktree。
- 参考组 `11334ae` 已由协调者合入，未改其脚本/JSON。任务组2复选框全勾。报告自带截图、真实测试输出和数值，不把验收留给协调者。

## 可回滚事务证据（四个绝对角色路径）

- `D:/Github/Matcha/target/visual/wave1b-transaction/MODIFIED_FILE.tar`
- `D:/Github/Matcha/target/visual/wave1b-transaction/DIFF_FILE.patch`
- `D:/Github/Matcha/target/visual/wave1b-transaction/VERIFICATION.txt`
- `D:/Github/Matcha/target/visual/wave1b-transaction/ROLLBACK.sh`

相同检查 `cargo test -p bc-gui chrome:: -- --nocapture` 在归档解包副本执行：**BASELINE exit101**（接手遗留 E0308/E0599）；**MODIFIED exit0，19 passed**；**ROLLBACK exit101**（恢复同样原始错误），恢复归档 SHA256 与 BASELINE 相同。`ROLLBACK.sh TARGET_ARCHIVE` 从 sibling BASELINE_FILE.tar 恢复原字节，已执行；DIFF_FILE.patch 是原生 Git binary patch，已在隔离仓库重建 MODIFIED_FILE.tar 并校验哈希。逐字 stdout/stderr、命令、退出状态、哈希在 VERIFICATION.txt；首个 patch 验证命令因父仓库相对路径跳过导致 assertion exit1，改为隔离仓库后 exit0，未掩盖错误。

## 实施提交

- `0b45ac9` feat(gui): load Windows UI and CJK fallback fonts
- `119e526` feat(gui): define seed classic theme and verify tokens
- `b644cff` feat(gui): implement keyboard menus and classic chrome controls
- `621aae2` chore(table): fix clippy lints
- `2641fe6` fix(gui): follow system message font and measured theme tokens
- `e696ed3` feat(gui): paint original toolbar and path icon catalog
- `4d7b353` feat(gui): capture deterministic PNGs of real comparison views

均含 Codex Co-Authored-By；本报告证据提交 hash 随 worker_done 回执给出，避免自引用 commit hash。

<details><summary>未授权修改的历史 fmt 文件（39）</summary>

```text
apps\bcomp\src\lib.rs
crates\bc-engine-bin\src\lib.rs
crates\bc-engine-dir\examples\dir_ui_fixture.rs
crates\bc-engine-dir\src\content.rs
crates\bc-engine-dir\src\view.rs
crates\bc-engine-dir\tests\content.rs
crates\bc-engine-dir\tests\filters.rs
crates\bc-engine-table\src\align.rs
crates\bc-engine-table\src\columns.rs
crates\bc-engine-table\src\map.rs
crates\bc-engine-table\src\model.rs
crates\bc-engine-table\src\regional.rs
crates\bc-engine-table\src\source.rs
crates\bc-engine-table\src\workbook.rs
crates\bc-engine-table\tests\batch_copy.rs
crates\bc-engine-text\examples\text_baseline.rs
crates\bc-formats\examples\formats_baseline.rs
crates\bc-formats\tests\xml_roundtrip.rs
crates\bc-gui\src\bench_dir.rs
crates\bc-gui\src\dialogs\scope.rs
crates\bc-gui\src\table\commands.rs
crates\bc-gui\src\table\copy_tests.rs
crates\bc-gui\src\table\details.rs
crates\bc-gui\src\table\edit_tests.rs
crates\bc-gui\src\table\eval_tests.rs
crates\bc-gui\src\table\session_tests.rs
crates\bc-gui\src\table\tests.rs
crates\bc-gui\src\table\worker.rs
crates\bc-gui\src\text\view.rs
crates\bc-jobs\src\lib.rs
crates\bc-jobs\src\manager.rs
crates\bc-jobs\src\pool.rs
crates\bc-script\src\cli.rs
crates\bc-script\src\command.rs
crates\bc-script\src\exit_code.rs
crates\bc-session\src\prefs.rs
crates\bc-session\src\session.rs
crates\bc-session\src\workspace.rs
crates\bc-session\src\xml.rs
```
</details>
