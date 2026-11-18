# visual-parity 交接文档

更新：2026-11-11（第二次交接），协调者：pi k3.1-pro（ObsidianVault 会话）总控+review，实施 worker：Orca 编排 **claude opus high**。上一棒：Codex → pi k3.1-pro（2026-11-11 首次交接）。本文件是后续协调者与 worker 的唯一入口。

## 0. 2026-11-15 收口

用户决定外观走现代设计系统、不做经典 Windows 1:1（design D9）。剩余任务组按“结构对齐参考、样式走设计系统”收口：
三类右键菜单按参考清单落地、文本/表格/文件夹/Hex Session Settings 统一对话框框架（文件夹按参考帮助六页）、
sheet 标签共享组件、文件操作对话框文字化。tasks 23/23，详见 tasks.md 与 `docs/reference-parity/visual.md`「方向修订」。
下文第 1–7 节是旧方向（1:1 复刻）时期的交接记录，保留作历史。

## 1. 目标与范围

- 需求：Matcha 外观尽量一比一复刻参考工具（默认英文、浅色、100% 缩放、默认偏好）。真源：`openspec/changes/archive/2026-11-18-visual-parity/`（proposal、design D1–D9、tasks；已归档，主 spec 见 `openspec/specs/*-appearance`、`ui-chrome-theme`、`visual-reference-harness`）。
- 用户方向（必须遵守）：
  - **抓大放小**：只做比较时始终可见的核心 UI；琐碎像素不打磨，做不到列为已知差异。
  - **串行**：一次只派一个实施 worker（共享桌面焦点与 CPU）。
  - **参考工具一律离线**：采集已完成，用 `docs/visual-parity/reference/*.png` + `tokens.json|md`。**不得再启动参考工具**（2026-11-11 曾授权 REA 实测一次，结论：REA 原生 UI 观测仅支持 macOS，Windows 上对本项目无实时观测价值，授权不再延续）。
- 红线：不提取/描摹参考工具图标资源（图标为 `chrome/icons.rs` 原创绘制）；产品内不出现参考工具名称；不读取授权/密钥文件。

## 2. 全景状态（截至 Wave 9）

**全部主视图与共享 chrome 已完成换肤**：文本（W2/2b）、文件夹（W3）、表格（W8）、同步（W9）、Home（W5）、菜单栏下拉与右键菜单（W6）、对话框 chrome + Session Settings + Options（W7）、性能/卡顿（W4）。快照场景 17 个全部 `-Verify` 通过；`cargo test --workspace` 451 通过；clippy 零警告。

| 波 | 内容 | 报告 |
|---|---|---|
| 1A/1B | 语料/采集/tokens/chrome 基础/snapshot 设施 | `visual-parity-wave1A.md` / `wave1B.md` |
| 2/2b | 文本比较主视图核心外观 + 收口 | `visual-parity-wave2.md` / `wave2b.md` |
| 3 | 文件夹比较主视图换肤 | `visual-parity-wave3.md` |
| 7.1 | 核心版收尾报告 | `docs/reference-parity/visual.md` |
| 4 | UI 卡顿优化（6 项热点修复）+ 参考样式文本缩略图 | `visual-parity-wave4.md` |
| 5 | Home 换肤 + 折叠分隔行 9 px（变高行） | `visual-parity-wave5.md` |
| 6 | 经典 popup（菜单栏/右键菜单）+ 行详情精调 + 停靠 Find bar | `visual-parity-wave6.md` |
| 7 | 对话框 chrome + Session Settings（文本）+ Options | `visual-parity-wave7.md` |
| 8 | 表格比较主视图换肤 | `visual-parity-wave8.md` |
| 9 | 文件夹同步视图换肤 | `visual-parity-wave9.md` |

关键实测：UI 字体 = 系统消息字体（本机 Microsoft YaHei UI 9pt = 12 px）；编辑器 Consolas 10pt；文本行高 15 px、表格行高 18 px；面板 `#F0F0F0`、重要差异底 `#FCE6E6`、不重要差异底 `#EEEEFF`；客户区 1384×961。完整值见 `docs/visual-parity/tokens.md`。

## 3. 剩余工作（候选，按可见度排序；均需用户再开）

1. **表格/同步的 Session Settings 对话框**：复用 `chrome/dialog.rs`，参照 W7 文本版做法。
2. **CJK 列宽拟合**（W8 遗留）：表格列宽按 ASCII 拟合，CJK 内容未对齐。
3. **行为向（非外观）**：默认比较规则对齐——参考默认 `case_unimportant=true`，Matcha 默认 false。需另立行为 change（先离线核实 `reference/dlg-text-session-settings.png` 的 Importance 默认态——注意该参考图只有 Specs tab，证据不足，可能需协调者决策）。W8 另有「参考默认表头/键规则目前只在快照 preset 生效」的同类跟进。
4. **小项**：折叠行 [+] 展开钮（W5 已知差异）；Edit 菜单内容/禁用态与参考对齐（W6 只换肤未对齐内容）；sheet 标签（表格多 sheet，原后置项）。
5. **会话标签页不做**：参考截图单会话下无标签栏，Matcha 亦无——不是差距。

## 4. 跟进项

- **性能基线**：W4 后文本/表格/文件夹 bench 全部健康（dir p99 0.53 ms）。W8 表格 p99 +55%（1.4→2.0–2.4 ms）系参考密度（每帧单元格 +67%）所致，协调者裁决接受为已知差异——**性能判断以 16 ms 帧预算为准，不看相对百分比**。日常回归用 `--bench`/`--bench-table`/`--bench-dir` + `MATCHA_FRAME_PERF=1` 帧直方图。
- **REA**：claude 用户级 MCP 已配置 `rea`（`npx rea-agents@6.3.0 mcp`，与本机 pi 配置一致）；Windows 上 `observe_native_ui` 等原生观测返回 capability_unavailable，仅二进制静态分析可用。
- **fmt 历史债**：约 39 个历史文件 `cargo fmt --check` 不通过，未授权全仓 fmt；worker 只格式化自己触及的文件。

## 5. 编排方法（给下一任协调者）

- Run：`run_9eca4b78596e`（W4–W9 在用；新建亦可）。worktree 选择器：`id:b33aa5f6-08d1-4108-997e-9a64fb877e52::D:/Github/Matcha`（master）。
- 派发新 session：`orca orchestration worker-start --spec "$(cat <spec文件>)" --worktree <选择器> --agent claude --model opus --effort high --json`；核对回执 `launch.effective` 与 requested 一致才算数。
- 复用 worker（连续迭代期推荐）：先 `task-create --spec ...`，再 `worker-start --task <id> --terminal <handle> --worktree <选择器>`（**复用终端也必须带 --worktree**，否则报 terminal_worktree_mismatch）。worker_done 后 `worker-retain` 保终端。
- spec 一律写文件再读入（模板见 ObsidianVault `.claude/plans/Matcha编排-visual-parity-wave{4..9}.md`）：Target / Change / Constraints / Observable acceptance；写明抓大放小、不启动参考工具、验收证据由 worker 产出、做不到就 ask 或如实 failed。
- 等待：`orca orchestration check --wait --types "worker_done,escalation,question" --timeout-ms 880000 --json`（心跳行用 `grep -v '"_keepalive"'` 滤掉）；超时是 checkpoint 不是失败；`worker-show` 看 liveness，worker 提问用 `reply --id <msg_id>` 回答后 ack 整批。
- 每次 `worker_done` 后**先机械核验再评审**：`git log --stat`、clippy、test 总数核对、`snap -All -Verify` 日志计数、assets PNG `sha256sum` 去重 + 并排图目检。曾出现 worker 报 succeeded 但交付空壳，必须核验。
- 成本汇报：收尾跑 `python <ObsidianVault>/.claude/skills/orchestration-ops/orca_coordinator.py usage <run_id>`（注意它把 claude worker 误记为 pi，claude 实际 cost 不在其口径内）。
- claude worker 坑：`claude mcp add` 在 git-bash 下要给 `/d /c` 加 `MSYS2_ARG_CONV_EXCL="*"`，否则被转成路径。

## 6. 关键文件

- 规格：`openspec/changes/archive/2026-11-18-visual-parity/{proposal,design,tasks}.md`、主 spec `openspec/specs/*/spec.md`
- 外观真源：`docs/visual-parity/tokens.json`、`tokens.md`、`reference/*.png`、`supplemental/`
- 工具：`tools/visual/compare.ps1 -Reference <png> -Actual <png> -Scene <name>`、`snap-matcha.ps1 [-Scene|-All] [-Verify]`（场景列表取自 `matcha /?` 的注册表，2026-11-15 起 28 个；用 `pwsh` 运行——从 Git Bash 启动的 Windows PowerShell 5.1 会因 PSModulePath 继承自 PS7 而找不到 `Get-FileHash`）、`manual-verify-*.py`（元素级测量）
- 代码：`crates/bc-gui/src/chrome/`（含 wave4 `wheel.rs`、wave6 `popup.rs`、wave7 `dialog.rs`）、`snapshot.rs`、`text/`、`table/`、`dir_view.rs`、`sync_view.rs`、`home.rs`、`dialogs/`、`app.rs`
- 报告：`docs/worker-reports/visual-parity-wave{1A,1B,2,2b,3,4,5,6,7,8,9}.md` + `assets/` + `evidence/`
- 性能设施：`MATCHA_FRAME_PERF=1`（帧直方图）、`MATCHA_PERF=1`（启动打点）、`cargo test --release -p bc-gui --lib interaction_latency -- --ignored --nocapture`

## 7. worker 纪律（沿用 + 增补）

- **字形像素不计**：egui 灰度 AA 与 GDI ClearType 的笔划强度差不计入对照；落在文字笔划上的颜色样本一律记已知差异，不得叠画、加粗、改字号去凑。
- **每区域最多两轮调整**，仍不达标写已知差异与原因。
- **时间盒**：单个 worker ≤ 90 分钟出第一个可验收提交；超 60 分钟无提交，协调者读终端判断是否在抠琐碎细节并叫停。
- `compare.ps1` 对平坦色带边界的自动 FAIL 可能是检测噪声，以人工像素测量为准并在报告给证据；不得改判定规则让其通过。
- 性能回归按绝对帧时间裁决（16 ms 预算），相对百分比只作线索；bench 超 10% 先补测到 3 轮交叉再下结论。
- PowerShell 脚本改动注意勿引入 BOM（W4 踩过，会破坏执行）。
