# table-compare-xlsx — Wave 7 权威总报告（4.4 + 6.1 收口）

日期：2026-11-06。基线 master `e3784b2`（Wave 6 后 21/23）。本波完成 4.4 与 6.1，**累计 23/23 全部勾选**，strict validate 通过；change **不归档**（评审归档另行），specs 未改。

执行模型：pi `k3.1-pro`（实际 provider `pl-anthropic`；早期 kimi 通道受并发限制失败后换通道重试恢复）。Orca 多 worker 编排：主实现 worker（本报告主要作者）、Search 子项 worker、只读审计/对抗 worker、独立 validation worker；总控多轮反例 review（Orca run `run_2db49a5f647e` 起的若干 dispatch），每轮 blocking 均先写失败回归再修复。

## 子报告与原始证据

- 实施：`docs/worker-reports/table-compare-xlsx-wave7-implementation.md`（覆盖矩阵、修复清单、逐轮 review 收口记录）；日志 `target/table-wave7/implementation/` 与 `target/table-wave7/review-final/`。
- Search 子项：`docs/worker-reports/table-compare-xlsx-wave7-search.md`；日志 `target/table-wave7/search/`。
- 引擎对抗回归：`docs/worker-reports/table-compare-xlsx-wave7-engine-review.md`（F1–F4）、`table-compare-xlsx-wave7-audit.md`；日志 `target/table-wave7/review/`、`audit/`。
- 6.1 独立验收：`docs/worker-reports/table-compare-xlsx-wave7-validation.md`；门禁日志 `target/table-wave7/validation/`，A/B 原始轮次与汇总 `target/table-wave7/audit/bench-*`。

## 进度：21/23 → 23/23

| 任务 | 内容 | 状态 |
|---|---|---|
| 4.4 | 查找/替换、会话设置五页、Swap/Reload/Recompare、单侧替换、独立保存/xlsx 只读、sheet 配对/色点/Info、摘要导出、format_important 接 GUI | ✅ 本波完成 |
| 6.1 | 全量回归 + 同轮 A/B 基准 | ✅ 独立验收通过 |

## 主要修复（4.4 实施侧，均先反例后修复）

1. **Swap Sides 全状态正确性**：引擎 `TableMap::swap_sides` 镜像转置（sheets/映射/列注释方向/锚点/状态/摘要左右计数），安装态保持 aligned 不重比；`TableEdit` 方向奇偶使跨 Swap 的 Undo/Redo 先转置快照再恢复；未提交编辑器草稿随物理侧转置；`find.last` 失效。
2. **单侧替换的原子性与保留**：Open File/Open Clipboard/Reload/Settings 改 staged 提交——worker 成功才动 files/options/pairs，失败保留旧会话；未替换侧（含非活动缓存 sheet）的编辑与 Undo 历史按 sheet 源索引重新挂接保留；替换侧历史按确认清空。
3. **pending_side 状态机**：保留对的替换侧 sheet 未装载时显示诚实空位；成功发布才清 flag；占位期间 `is_ready` 为 false、写/保存拒绝且不取消加载；`load_pending_pair` 统一 select_sheet/Swap/Recompare/投影入口。
4. **扫描守卫**：只在"非活动且非缓存的待扫描项"存在时启动；staged 事务期间不启动（无 generation 防污染）；失败明示错误且不忙重试，用户动作恢复。
5. **设置正确性**：非法映射先验证后应用（不污染默认规则）；换格式真实重解析；换源按新 names 重配对并发布 `loaded.active`；rules-only Apply 恢复草稿确认保护。
6. **保存/导出**：失败重排程重比（不再永久 stale）；保存不取消 sheet 扫描；xlsx 只读提示与 CSV 出口；陈旧摘要拒绝。
7. **键推断标注**：状态区与 Columns 页显示解析键列名（如 `id（自动）`）。
8. **Search（子项 worker）**：无命中/同值替换不建空历史、换词/换侧重置位置、F3/按钮/Ctrl+F 入口；另有 1 项 RawInput 面板流由主 worker 补。
9. **format_important**：Session Settings → Columns 页复选框接入会话规则（U→I→Ignore→Values 档全链路输入回归）。

## 新增测试：69 项（58 GUI + 11 引擎）

- GUI（`bc-gui/src/table/`）：session_tests 25、sheet_tests 7、review_tests 14、search_tests 12（Search worker 交付），合计 58；53 → **111**（已经静态 `#[test]` 计数与 `cargo test --lib table:: -- --list` 核实）。
- 引擎（`bc-engine-table/tests/`）：session_history 6（对抗 worker）+ swap_orientation 5 = **11**。
- 关键缺陷反例均先复现旧代码失败再修复（含 e3784b2 独立快照复跑 Swap 用例），日志见 `target/table-wave7/implementation/pre-fix-*.log`、`review-final/pre-fix-*.log`；其余新增用例为覆盖性回归（固有通过），不逐个冒称旧码失败。

## 最终门禁（validation worker 独立执行，证据链见其报告）

- `cargo clippy --workspace --all-targets -- -D warnings`：**0 警告**。
- `cargo test --workspace`：**372 passed / 0 failed / 3 ignored**；表格 GUI **111 passed / 0 failed**。
- 黄金：`golden_table` 3 tests ok，29/30 用例；**行 81/83 = 97.59%，格 236/242 = 97.52%（≥95%）**；唯一差异仍为既有 KNOWN_DIFFS 的 Unaligned 报告路径，未扩大。
- `cargo build --release --workspace` 成功；parity 校验单测 3/3；`openspec validate --strict` valid；`git diff --check` 干净。
- 源码指纹前后一致（208 个 .rs + Cargo 清单逐字节不变）；基线 A 为 e3784b2 快照独立构建（SHA256 `f63125ce…`），候选 B SHA256 `f124ca26…`。

## 同轮 A/B 基准（每侧每线 3 轮，30 预热 + 600 帧，交替轮序）

| 线 | 侧 | update mean（median, ms） | update p99 median（ms） | frame CPU p99 median（ms） |
|---|---|---|---|---|
| 文本 110 MB/侧 | A（e3784b2） | 0.655 | 1.174 | 1.785 |
| 文本 110 MB/侧 | B（候选） | 0.650 | **1.160（B/A −1.2%）** | 1.814（+1.6%） |
| 表格 100 万×10 | A | 0.634 | 1.126 | 2.628 |
| 表格 100 万×10 | B | 0.634 | **1.075（<4 ms 门槛；B/A −4.5%）** | **2.664（<8 ms 门槛）** |

文本线逐轮区间重叠但非完全相等（A p99 1.112–1.247，B 1.158–1.253），均在 5% 告警线内，未发现退化。

## 边界与未测项（不冒称）

- 参考工具 77 个帮助段落**不是**全部功能实现声明；C01/C02 会话持久化、C09 报告、C11 仅接 Text、C12 父文件夹、C17–C26 中 Open With/Explorer/Profile 等按 D9/ADR 202 既定后置或范围外，详见 `docs/reference-parity/table-compare.md` 逐行口径。
- 物理鼠标与原生 IME 候选窗按 ADR 202 留人工点验；3 个 ignored 测试（两项表格压力、一个 Windows 手动回收站）本轮未单独运行。
- 大表 Find/Replace、Save/Export 为前台同步路径，百万级耗时未测（已记录为未测风险）；Find 命中定位/替换语义由控制器+输入注入回归覆盖。
- Format 页选项与会话共享 ReadOptions 应用路径；本轮实际回归分隔符改动生效，Type/编码/区域其余组合由引擎既有单测覆盖，未做逐项 UI 组合回归；全平台 locale 组合未测。
- xls/ods 与外部转换器明确不支持；xlsx 写回与格式层扩展另立 change。

## 提交

本波以两个本地 Conventional Commits 收口（代码 SHA 见提交后回填；不 push、不归档、不 amend）：

1. 代码/测试（15 个 Rust 文件）：`fix(table): complete session and workbook state acceptance` → 提交 SHA 见下节回填。
2. 文档与工具：`test(table): record final xlsx acceptance and same-round benchmarks` → 提交 SHA 见下节回填。

### 提交回填

- 代码提交：`283b47e8e7c356a0b5e7d66769a238b1ed266081`（`fix(table): complete session and workbook state acceptance`，15 个 Rust 文件，+4613/−190）
- 文档提交：见包含本报告的 `test(table)` 提交；按规则不在本文自引用其 SHA、不留待填占位。
