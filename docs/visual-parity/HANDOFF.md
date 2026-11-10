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

**Wave 2 已完成**（Wave 2b 收口任务 3.4，提交 `6586a7e` + 报告提交，报告 `docs/worker-reports/visual-parity-wave2b.md`）：

- 核心区域全部达标：自动扫描通过 menubar/toolbar/pathbar.*/paneStatus.*/pane.left/details 等 9–10 项；info.right、gutter.right、pane.right、splitter、statusbar 的自动 FAIL 经人工逐像素测量证实为平坦色带检测噪声（真实偏差 ≤1 px），证据在报告内。`compare.ps1` 判定规则未改。
- 颜色样本 16/21（Wave 2 为 12/21）；未通过 5 项（diffForeground/lineNumber/minorForeground 为 egui 灰度 AA vs GDI ClearType 笔划强度差；pathSelection 因本机路径短于参考截图机；toolbarToggleBorder 差 1 px 按钮位置）。
- 可见修复：缺行/EOF 改为参考的 8 px 菱格位图（逐像素一致）；路径栏右侧无边框 16 px 图标钮 + 编辑框内嵌历史箭头 + 快照复刻路径全选；Differences/Same 新增「N FILTERED LINES」灰色分隔行（15 px，参考 9 px，列为已知差异）；cursor 场景光标列 5。
- `snap-matcha.ps1 -All -Verify` 六场景两次哈希一致；clippy 零警告、424 tests passed。
- 性能 A/B（基线 `8ace066` worktree 构建）：文本 p99 +1.3%、表格 +5.5%（Wave 1B 的 +15.9% 未重现），各 1 轮均 <10%。
- 已知差异（详见 Wave 2b 报告）：缩略图旧导航条（后置）、折叠行 15 px 行高、菜单栏无更新提示链接、字形级 AA 强度差。

## 4. 剩余工作（按顺序，串行）

1. ~~**Wave 2b：收口任务 3.4**~~ **已完成**（见第 3 节，报告 `docs/worker-reports/visual-parity-wave2b.md`）。
2. ~~**Wave 3：文件夹比较主视图（任务 4.1）**~~ **已完成**（报告 `docs/worker-reports/visual-parity-wave3.md`，提交 `9d438e6`+`e127a6d`）：菜单栏/工具栏/过滤栏/路径栏/双树/日志区/状态栏全部按参考换肤；三场景命名区域 ≤2 px（多数精确 0），颜色样本 basic 2/2、expanded 5 项为字形 AA 已知差异；排序按名称并孤儿不后置（仅比较视图）；Modified 本地时区格式（仅比较视图）；同步视图零改动。文件夹同步（4.2）与对照收口（4.3）未做。
3. **7.1 收尾报告（核心版）**：`docs/reference-parity/visual.md`，每场景并排图 + 区域偏差表 + 已知差异；clippy/test/基准回归；`tasks.md` 的 Workflow follow-up 记录。
4. 后置项（第 1 节）等用户再开。

## 5. 跟进项（不在外观范围）

- **默认比较规则与参考对齐**：参考 text-basic 中 `beta`/`BETA` 显示为同一行蓝色不重要差异，说明参考默认规则下大小写不重要；Matcha 默认 `case_unimportant=false`。ADR 007 写明黄金语料未继承未核实的默认配置。需另立行为 change：先离线看 `reference/dlg-text-session-settings.png` 等核实默认 Importance，再改默认值。Wave 2 仅在快照 preset 中显式设 `case_unimportant=true`。
- **性能暂留**：Wave 2b 复测：文本 `--bench` update p99 +1.3%、表格 `--bench-table` +5.5%（基线 `8ace066` worktree 构建，各 1 轮）；Wave 1B 的表格 +15.9% 未重现。日志 `target/visual/wave2b/bench/`。
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

## 8. worker 纪律（Wave 2b 教训，2026-11-11）

- **字形像素不计**：egui 灰度 AA 与 GDI ClearType 的笔划强度差不计入对照；落在文字笔划上的颜色样本一律记已知差异，不得叠画、加粗、改字号去凑（Wave 2b 曾为行号灰度 159 vs 107 空转 1 小时、约 $56）。
- **每区域最多两轮调整**，仍不达标写已知差异与原因。
- **时间盒**：单个 worker 目标 ≤ 90 分钟出第一个可验收提交；超过 60 分钟无提交，协调者应读终端判断是否在抠琐碎细节并叫停。
- `compare.ps1` 对平坦色带边界的自动 FAIL 可能是检测噪声，以人工像素测量为准并在报告给证据；不得改判定规则让其通过。
