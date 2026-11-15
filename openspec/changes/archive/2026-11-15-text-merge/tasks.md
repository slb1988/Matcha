# Tasks

说明：全程约束——在当前分支上工作，不新建分支；`cargo test --workspace` 通过，自己改动的 crate `cargo clippy --all-targets` 零警告；不新增第三方依赖；`bc-script` / `bc-engine-text` 不依赖 `bc-gui`；只对自己改动的文件运行 rustfmt。每完成一个任务组执行 `tools/dev-build.ps1` 并当场修复构建失败；每组更新 `CHANGELOGS.md`（用户可感知的变化）；提交时机由用户决定（Conventional Commits）。各组依赖顺序为 2 → 3 → 4 → 5 → 6（第 5 组用到 4.1 的 `merge_spec()`，第 6 组用到第 3 组的合并模型）；第 1 组可与第 2 组并行，但 3.5 依赖 1.3 的样本。

## 1. 参考依据与功能文档（先行）

- [x] 1.1 新建 `docs/reference-parity/text-merge.md`：按 `text-compare.md` 的做法只读解包参考帮助，逐项对照 `commandstextmerge.html`、`sessiontextmerge*.html`、`viewtextmerge.html`、`using_text_merge.html`、`3-way_merge_concepts.html` 与 `command_line_reference.html` 的合并相关条目，每项标 已实现 / 有意未采纳 / 后置（本变更范围内的先标"进行中"）；验证：帮助页中的每个合并命令在表中都有映射，design 的 R1–R11 都能在该文档找到出处
- [x] 1.2 新增合并样本语料 `crates/bc-engine-text/tests/golden/merge/<case>/{left,right,center}.txt` 与 `tools/golden/gen_merge_expected.ps1`（沿用 `gen_expected.ps1` 的共享锁、`-ReferenceExe` / `-WorkerId`、finally 恢复与 `-DryRun`），对每个用例以参考工具运行 `/automerge`、`/automerge /force`、`/automerge /iu`、`/automerge /iu /favorleft` 四种组合，记录输出字节与退出码；用例至少覆盖 design 的 P1–P6 与 P8：两侧改动相距 0/1/2/3 行、同一行异改、同一位置插入不等长、同改、一侧删除一侧修改、仅不重要差异的单侧改动与冲突、输入行尾不一致、输入编码不一致、末行无换行、输出文件预先存在；验证：`-DryRun` 列出全部目标路径且不触碰参考配置
- [ ] 1.3 持锁运行采集并入库样本；把 P1–P6、P8 的结论（确认或修正）写入 `docs/reference-parity/text-merge.md`，与暂定语义不一致的同步改 design.md 与对应 spec；验证：每条结论指向一份具体样本，`git status` 无私人备份或二进制。无法运行参考工具时本项标注阻塞原因、其余任务按暂定语义继续，并在 feature 文档状态写明"未经参考样本确认"
  - **阻塞（2026-11-14）**：本机参考程序（`D:\Program Files\Beyond Compare 5\BCompare.exe`，5.0.6）在，但三种写法都打开 `left.txt <--> right.txt - Text Compare` 窗口而不是合并：`left right center out`、`left right center`、`/fv="Text Merge" left right center out`（带不带 `/automerge` 都一样），输出文件不产生。**原因未确认**：帮助里合并各页带 Pro 标记，推断是本机许可为不含三方合并的版本；按仓库约定没有读许可文件核对。`gen_merge_expected.ps1` 因此超时，没有产生样本。P1–P6、P8 保持暂定；参考工具能打开 Text Merge 之后重跑脚本即可解除。
- [x] 1.4 新建 `docs/features/text-merge.md`（套用 README 模板：功能简介、代码地图占位、关键语义、使用与验证、深入阅读；状态"进行中"），在 `docs/features/README.md` 索引登记；验证：结构符合维护约定模板

## 2. 引擎：三方对齐

- [x] 2.1 `bc-core::align`：实现策略无关的三方调和函数（design D1 的桶规则），`QuickAlign` / `SmartAlign` / `PatienceAlign` 的 `align3` 各自调用自己的 `align2` 两次后委托它；验证：`cargo test -p bc-core` 新增单测（单侧插入、同位置两侧插入、一侧删除、空 center、三份相同）与属性测试 I1–I3
- [x] 2.2 `bc-engine-text::map`：`AlignedData` 增加平行 center 列、`center_hashes` 与 center revision；`row()` 填 `center`，`TextMap::text_at(Side::Center, …)` 可用，`is_stale` / `set_alignment` 计入 center；验证：新增单测；`size_of::<CompactRow>()` 断言不变；双方黄金测试（`cargo test -p bc-engine-text`、`-p bc-core`）全部通过
- [x] 2.3 实现 `Align3Job::execute`（与 `Align2Job` 相同的 `align_range` 管线跑 left↔center、center↔right，再调和、分类、建区段；支持取消）与 `Job::run`；`Realign3Job` 以全量重对齐实现；验证：`cargo test -p bc-engine-text` 覆盖 three-way-alignment 规格的全部 scenario（含 Unaligned、忽略行尾空白、重对齐与全量逐行相同、取消后原行表不变）
- [x] 2.4 双方比较无回归：运行既有滚动基准（`--bench`）对比改动前后；验证：帧时间与行表内存无可见回归，数字记入提交说明
  - 结果（2026-11-14，`--bench=300`，text-long 语料，各跑两次）：改动前 update_ms 均值 0.832 / 0.874、p95 1.294 / 1.425；改动后 0.824 / 0.837、p95 1.268 / 1.338。`CompactRow` 大小由编译期断言锁定为 12 字节。

## 3. 引擎：合并模型与自动合并

- [x] 3.1 `TextMergeRules`：增 `conflict_gap`（默认 2），删 `conflict_on_center_change`；实现 `MergeState::classify` 的逐行类型（未变/左改/右改/同改/异改，含 `ignore_unimportant`）；验证：分类矩阵单测覆盖"行类型分类"与"忽略不重要差异与 favor"的 scenario
- [x] 3.2 实现冲突区段判定与 `conflicts(ignore_unimportant)`（改动段、同段两侧改动、`conflict_gap` 合并、同改不计、区段重要性）；验证：单测覆盖"冲突区段"的全部 scenario，外加 `conflict_gap = 0` 与链式合并（左-右-左三段）
- [x] 3.3 实现默认输出与解决：逐行取舍、被忽略的不重要冲突取 center / favored 侧、`resolve`（取 left/right/center，整段替换）与"接受当前内容"；验证：单测覆盖"默认输出内容""冲突解决"的 scenario，含同位置不等长插入取右后左侧 3 行都不出现
- [x] 3.4 实现 `render_output` 与 `auto_merge`：冲突标记（标签、标记行行尾、缺行尾时补行尾）、行尾逐行保留、编码与 BOM 三方取舍、不可表示时报错、`withheld`；验证：单测覆盖"自动合并结果""冲突标记""输出的行尾与编码"的 scenario，手写小例逐字节断言
- [ ] 3.5 黄金回归测试：对 1.2 的语料用默认规则运行四种组合，与入库样本比对输出字节与退出码，差异按 `KNOWN_DIFFS` 的做法登记；验证：`cargo test -p bc-engine-text` 通过，样本缺失时报错而不是跳过（1.3 被阻塞时本项同样标注阻塞）
  - **阻塞（2026-11-14）**：没有参考样本（见 1.3）。`tests/golden_merge.rs` 已就位：样本齐全时逐字节比对输出与退出码，部分缺失时报错；目前在 20 个用例 × 4 种组合上固定的是暂定语义（每个用例的冲突区段数），不是参考对齐。
- [x] 3.6 更新 `crates/bc-engine-text/README.md`（合并模型、冲突区段、规则字段）与 `docs/features/text-merge.md` 的关键语义；验证：README 的规则字段与 `TextMergeRules` 一致

## 4. CLI：`/automerge`

- [x] 4.1 `bc-script::cli`：`CliArgs::merge_spec()`（归一 `Merge3` / `Merge4` / `Files + /center=`，`/mergeoutput=` 优先于第 4 个位置参数），更新 `/?` 用法文本；验证：cli 单测覆盖四种写法与优先级
- [x] 4.2 `routing.rs`：新增 `Route::AutoMerge`（位于 `/qc` 之后、`unsupported` 之前；缺输出目标或不是三方调用 → `Unsupported`），`unsupported()` 放行合并调用并拒绝被转交的 `/automerge`；改写既有单测 `help_and_unsupported_invocations`；验证：routing 单测覆盖 `/automerge` + 3/4 路径、`/solo` 不改变路由、常驻实例拒绝 `/automerge`、`a b c` 不再是 Unsupported
- [x] 4.3 `bc_script::runner::auto_merge_files`（仿 `quick_compare_files`）：校验输入（文件夹/缺失/不可读/二进制 → 错误）、按 center 文件名选格式、默认规则、对齐、`auto_merge`、原子写出（输出路径可等于某个输入、不建备份、101 时不触碰已有文件）；验证：`cargo test -p bc-script` 以临时文件覆盖 0 / 14 / 101 / 100 各分支、输出覆盖输入、已有输出文件在 101 时内容与修改时间不变
- [x] 4.4 `main.rs`：`Route::AutoMerge` 执行与输出（结果行写 stdout、`/silent` 不写、错误写 stderr、混合行尾与编码警告）；验证：以构建产物跑 `matcha.com /automerge …` 的集成测试（或手工脚本）核对四个退出码与 `/silent`
- [x] 4.5 文档：`docs/features/cli-and-scripting.md`、`crates/bc-script/README.md`、`apps/bcompare/README.md`、`.agents/skills/matcha-cli/SKILL.md`、`docs/features/text-merge.md`（使用与验证）、`CHANGELOGS.md`；验证：文档中的开关语义与 text-merge-cli 规格一致

## 5. GUI 接线：启动、等待与退出码

- [x] 5.1 `LaunchOptions` 与 `RemoteOpen` 增加合并载荷（`MergeSpec` + 标题 + 规则开关）；`main.rs` 的 `accept` 从 `merge_spec()` 构造；验证：转交路径单测——合并请求被投递、缺路径时回错误
- [x] 5.2 合并调用隐式等待：`hand_off` 的 `wait` = 等待标记或 `merge_spec().is_some()`；`remote::Outcome` 携带结果码，`accept` 的 `Completion` 原样回复；验证：`bc-instance` / `main` 层单测——未设等待标记的合并转交在标签页关闭前不返回，回复码等于标签页结果码；非合并调用行为不变
- [x] 5.3 首个实例与 `/solo` 路径：`run_with_remote` 返回启动标签页关闭时的结果码，`run_gui` 以它退出；非合并启动仍为 0；验证：`app_tests` 覆盖"启动标签页为合并、关闭时结果码被记录"，手工核对 `matcha.exe /solo a b c d` 的 `%ERRORLEVEL%`
- [x] 5.4 `View::TextMerge` 占位接入 `app.rs`（开页、关页、退出流程、标题），打开失败（缺失/二进制）回 Home 显示错误并回复 100；验证：`matcha a b c` 与 `matcha a b c d` 打开合并标签页，输入缺失时调用方得到 100

## 6. GUI：文本合并视图

- [x] 6.1 `text/render.rs` 窗格绘制抽取为可被两种视图调用的函数（不改行为），文本比较视图改为调用它；验证：既有 text 快照场景逐像素不变，`--bench` 无回归；超出预期时按 design D12 的退路处理并在此记录
  - **按 D12 的退路处理（2026-11-14）**：`pane()` 是 535 行、与 `TextCompareView` 的投影 / 行内差异 / 查找 / 手工对齐状态深度耦合的方法，抽取等于重写文本比较的热路径。没有做抽取：合并视图自带一份精简绘制（`merge/view.rs`），只把 `render.rs` 的 `layout` 与 `hatch` 改为 `pub(crate)` 供其调用。文本比较的代码路径与行为没有变化（既有 text 快照场景与基准未受影响，见 2.4）。合并窗格因此没有语法着色与行内差异高亮。
- [x] 6.2 新增 `bc-gui/src/merge/`：四窗格布局（三个只读输入 + 输出）、共用对齐行网格的滚动联动、路径/标题显示、四个主题色位的着色、状态栏未解决冲突数；验证：新增快照场景 `merge-basic`、`merge-conflict`，视觉走查
- [x] 6.3 输出缓冲模型：独立 `TextContent`、逐输出行的来源与所属对齐行、输出多行/零行时的空隙行；验证：单测覆盖默认输出与行网格映射
- [x] 6.4 冲突导航与解决：上一/下一冲突、Take Left / Center / Right、接受当前内容（工具栏与快捷键），撤销/重做；验证：`view_tests` 覆盖"冲突导航与解决"的全部 scenario
- [x] 6.5 输出窗格编辑：复用 `Editor`，手工行标记与着色，手工编辑使冲突区段视为已解决，Take 覆盖手工内容；验证：`view_tests` 覆盖"输出窗格编辑"的 scenario
- [x] 6.6 保存与关闭：写输出目标 / 另存、不读既有输出、带输出目标自打开起未保存、未解决冲突时的确认、结果码 0 / 14 / 101；验证：保存路径测试覆盖"保存与关闭"的 scenario 与 text-merge-cli"GUI 合并的退出码"
- [x] 6.7 `/automerge /reviewconflicts`：有冲突且无 `/silent` 时转入与普通合并调用相同的 GUI 路径，并把 `/iu`、favor 带入会话；验证：routing 与 main 层测试覆盖"有冲突时转入界面"的 scenario
- [x] 6.8 文档：`docs/features/text-merge.md`（状态、代码地图、P4V 配置片段与三条启动路径的等待说明、`matcha-comp.exe` 不可用于合并）、`docs/features/text-compare.md` 状态、`docs/features/README.md`、`docs/features/gui-shell.md`、`apps/bcomp/README.md`、`docs/reference-parity/text-merge.md` 各项状态、`CHANGELOGS.md`；验证：文档与行为一致

## 7. 集成验收

- [x] 7.1 全量 `cargo test --workspace` 与 `tools/dev-build.ps1`；验证：测试全绿、无新增警告
- [ ] 7.2 P4V 端到端（人工）：配置 `matcha.exe %1 %2 %b %r`，在"没有实例在运行 / 已有实例在运行"两种情况下分别走"解决并保存""不保存关闭"；再配置 `matcha.exe /automerge /reviewconflicts %1 %2 %b %r` 走"无冲突""有冲突"；记录 P4V 以什么判定合并完成（退出码还是结果文件是否改动）以及 `%r` 是否预先存在，写入 feature 文档；同时在参考工具上观察 design 的 P7，写入 parity 文档；验证：六种组合的实际结果与 text-merge-cli 规格一致
  - **待人工（2026-11-14）**：需要 P4V 与真人操作窗口，本次实施无法完成。已有的自动化证据：`apps/bcompare/tests/hand_off.rs` 用真实的 `matcha.exe` 验证了"已有实例时合并调用等待并返回标签页结果码"、`/automerge` 的 0 / 14 / 101 / 100、`/automerge /reviewconflicts` 的两条分支；`crates/bc-gui/src/app_tests.rs` 验证了首个实例路径的结果码。未验证：P4V 自身以什么判定合并完成、`%r` 是否预先存在、参考工具的 P7。
- [ ] 7.3 端到端冒烟：base/theirs/yours 三份样例验证无冲突自动合并、冲突 GUI 解决、CSV 按文本合并；验证：手工检查输出与退出码
  - **部分完成（2026-11-14）**：无冲突自动合并与 CSV 按文本合并已由真实可执行文件 / runner 测试覆盖；冲突的界面解决只有无头测试与快照渲染（`merge-basic`、`merge-conflict` 两张 PNG 已人工看过），没有在真实窗口里手工走一遍。

## 归档说明（2026-11-15）

- 用户决定暂不推进剩余 4 项、直接归档：1.3 / 3.5（本机参考工具 BC 5.0.6 打不开 Text Merge，样本无法采集）、
  7.2（P4V 人工端到端）、7.3（真实窗口手工走冲突解决）均保持未勾，**带 4 项未完成归档**。
- 后果：合并语义仍是暂定语义、未经参考样本确认（`docs/features/text-merge.md` 已如此标注）；`golden_merge.rs` 的
  20 用例 × 4 组合固定的是暂定行为。日后换到能打开 Text Merge 的环境，重跑 `tools/golden/gen_merge_expected.ps1`
  并以新 change 补参考对齐。注意 7.2 原文的 `%1 %2 %b %r` 已于 2026-11-14 改为 P4V 顺序 `%b %1 %2 %r`。
- `text-merge-cli` 的"按格式分派"已在本 delta 内写实（table-merge 先行归档，主 spec `table-merge-cli` 已存在）。
