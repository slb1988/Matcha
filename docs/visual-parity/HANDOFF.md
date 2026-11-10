# visual-parity 交接文档（Codex → pi k3.1-pro）

更新：2026-11-11，协调者（Claude Code，ObsidianVault 会话）。目的：Codex 额度不足，后续波次改由 Orca 编排 pi（默认模型 `k3.1-pro`，thinking max）执行。本文件是后续协调者与 worker 的入口。

## 1. 目标与范围

- 需求：Matcha 外观尽量一比一复刻参考工具（默认英文、浅色、100% 缩放、默认偏好）。真源：`openspec/changes/visual-parity/`（proposal、design D1–D8、tasks）。
- 用户方向（必须遵守）：
  - **抓大放小**：只做比较时始终可见的核心 UI；琐碎像素不打磨，做不到列为已知差异。
  - **范围**：共享 chrome + 文本比较主视图 + 文件夹比较主视图。**后置**：标签类（会话标签页、sheet 标签）、Home、表格/同步换肤、对话框、右键菜单、缩略图与行详情精调。
  - **串行**：一次只派一个实施 worker（共享桌面焦点与 CPU）。
  - **参考工具只看一遍**：采集已完成，后续一律离线使用 `docs/visual-parity/reference/*.png`，**不得再启动参考工具**。
- 红线：不提取/描摹参考工具图标资源（图标为 `chrome/icons.rs` 原创绘制）；产品内不出现参考工具名称；不读取授权/密钥文件。

## 2. 已完成

| 波 | 内容 | 证据 |
|---|---|---|
| 1A | 语料 `tools/visual/corpus/`、采集脚本、23 张参考截图（SHA256 两两不同）、`tokens.json`（34 色/43 度量/23 场景/菜单清单）、`compare.ps1`（并排图 + 命名区域偏差表） | `docs/worker-reports/visual-parity-wave1A.md`，合入 `11334ae` |
| 1B | `crates/bc-gui/src/chrome/`（fonts 跟随系统消息字体、theme 取 tokens、menubar/toolbar/pathbar/statusbar/splitter/scrollbar、原创 icons）、`snapshot.rs`（`--snapshot=<scene>` 确定性 PNG）、`tools/visual/snap-matcha.ps1` | `docs/worker-reports/visual-parity-wave1B.md`，HEAD `8ace066` |
| 2 | 文本比较主视图核心外观（任务组 3）——**状态见第 3 节** | `docs/worker-reports/visual-parity-wave2.md` |

关键实测：UI 字体 = 系统消息字体（本机 Microsoft YaHei UI 9pt = 12 px）；编辑器 Consolas 10pt；文本行高 15 px；面板 `#F0F0F0`、重要差异底 `#FCE6E6`、不重要差异底 `#EEEEFF`、工具栏选中 `#CCE8FF`/`#99D1FF`；客户区 1384×961。完整值见 `docs/visual-parity/tokens.md`。

## 3. Wave 2 收尾状态

**部分交付（outcome=failed，按紧急收尾指令停止）**，报告 `docs/worker-reports/visual-parity-wave2.md`（`5864733`）。

- 已完成并提交：3.1 菜单栏 + 大图标工具栏（旧按钮行全部迁移，迁移表见报告）、3.2 路径栏/信息头/行号与箭头槽/差异色/编辑器字体、3.3 窗格状态/行详情/状态栏/窗口标题。提交 `c5a3a2b`、`b4c0832`、`e618cd6`、`e87c3ba`。全仓 clippy 零警告、424 tests passed。
- 新增快照场景 `text-diffs-filter`、`text-cursor-on-diff`；并排图 `docs/worker-reports/assets/visual-wave2-*-side.png`。协调者目检：整体结构与参考高度一致。
- **未完成（任务 3.4）**：
  - 对照最大偏差 8 px，多数区域未达 ≤ 2 px；固定颜色样本 12/21 通过。完整四边表在 `target/visual/wave2-transaction/<scene>-compare.md`（未入库，需重跑 compare 生成）。
  - 目检可见差异：左窗格 EOF/缺行处缺斜纹/网纹；缩略图仍是旧导航条（参考为顶部微型文档，已后置可不做）；路径栏右侧小按钮样式；行详情箭头/行尾符形状。
  - `tools/visual/snap-matcha.ps1` 的 `ValidateSet` 未加入两个新场景；新场景未做两次快照字节一致复验。
  - 性能 A/B（文本 `--bench`、表格 `--bench-table`，基线 `8ace066` vs HEAD）未执行；Wave 1B 表格 +15.9% 未复测。

## 4. 剩余工作（按顺序，串行）

1. **Wave 2b：收口任务 3.4**（已派 pi，见 ObsidianVault `.claude/plans/Matcha编排-visual-parity-wave2b.md`）：核心区域偏差与颜色样本收敛（每区域最多两轮）、缺行斜纹/EOF 网纹、snap-matcha 场景集、性能 A/B。
2. **Wave 3：文件夹比较主视图（任务 4.1）**：菜单栏/工具栏（`inventories` 中 folder 视图条目：Home、Sessions▾、All、Diffs▾、Same、Files▾、Minor、Rules、Copy、Expand、Collapse、Select、Files、Refresh、Swap、Stop）、过滤栏（Filters 输入 + Filters/Peek）、路径栏、双树（列头 Name/Size/Modified，行高 20，列宽 437/78/150，展开符、文件夹/文件小图标、名称颜色：孤儿 `#6A00D5` 紫、不同为红、较新/较旧按 tokens）、日志区、状态栏。参考图：`reference/folder-basic.png`、`folder-expanded.png`、`folder-diffs-filter.png`。只动 `dir_view.rs`、`filters_bar.rs`、`app.rs` 最小接线、`snapshot.rs`；同步视图不动。
3. **7.1 收尾报告（核心版）**：`docs/reference-parity/visual.md`，每场景并排图 + 区域偏差表 + 已知差异；clippy/test/基准回归；`tasks.md` 的 Workflow follow-up 记录。
4. 后置项（第 1 节）等用户再开。

## 5. 跟进项（不在外观范围）

- **默认比较规则与参考对齐**：参考 text-basic 中 `beta`/`BETA` 显示为同一行蓝色不重要差异，说明参考默认规则下大小写不重要；Matcha 默认 `case_unimportant=false`。ADR 007 写明黄金语料未继承未核实的默认配置。需另立行为 change：先离线看 `reference/dlg-text-session-settings.png` 等核实默认 Importance，再改默认值。Wave 2 仅在快照 preset 中显式设 `case_unimportant=true`。
- **性能暂留**：Wave 1B 后表格 `--bench-table` update p99 中位数 +15.9%（绝对约 1.2 ms，远低于 4 ms 门槛），文本 +8.6%。复测数字见 Wave 2 报告。
- **fmt 历史债**：约 39 个历史文件 `cargo fmt --check` 不通过，未授权全仓 fmt；worker 只格式化自己触及的文件。

## 6. 编排方法（给下一任协调者）

- Run：`run_b4d3a81ffc50`（可继续用，或新建）。worktree 选择器：`id:b33aa5f6-08d1-4108-997e-9a64fb877e52::D:/Github/Matcha`（master）。
- 派发：`orca orchestration worker-start --spec "<spec 文本>" --worktree <上面选择器> --agent pi --json`（pi 默认模型即 `k3.1-pro`，thinking max；无需 `--model`）。spec 写成文件再读入。
- 一个 worker 收尾后再派下一个；同一 worktree 不允许两个提交者。
- 每次 `worker_done` 后**先机械核验再评审**：`git log/show --stat`（实现规模）、`rustfmt --check` 触及文件、`cargo clippy -D warnings`/`cargo test` 结果、快照 PNG 是否真实且与场景对应（`sha256sum` 去重 + 打开看图）、`compare.ps1` 并排图目检。曾出现 worker 报 succeeded 但交付空壳/复制图片充数，必须核验。
- 已知 pi 坑：pi worker 在 `fetch failed`/上游过载时可能停在 TUI 不发 `worker_done`，用 `orca terminal send --terminal <handle> --text "继续" --enter` 催；长时间无心跳先 `worker-show` 看 liveness，不要盲目重派。
- worker spec 模板参考：ObsidianVault `.claude/plans/Matcha编排-visual-parity-wave2.md`（Target / Change / Constraints / Observable acceptance 结构）。spec 中必须写明：不启动参考工具、抓大放小、只动指定文件、验收证据由 worker 自己产出、做不到就 ask 或如实报 failed。

## 7. 关键文件

- 规格：`openspec/changes/visual-parity/{proposal,design,tasks}.md`、`specs/*/spec.md`
- 外观真源：`docs/visual-parity/tokens.json`、`tokens.md`、`reference/*.png`、`supplemental/`（菜单树截图与 UIA 结构）
- 工具：`tools/visual/compare.ps1 -Reference <png> -Actual <png> -Scene <name>`、`tools/visual/snap-matcha.ps1 [-Scene|-All] [-Verify]`、`cargo run --release -p bcompare -- --snapshot=<scene>`
- 代码：`crates/bc-gui/src/chrome/`、`snapshot.rs`、`text/`、`dir_view.rs`、`app.rs`
- 报告：`docs/worker-reports/visual-parity-wave{1A,1B,2}.md`
