# table-compare-xlsx — Wave 7：6.1 最终独立验收

日期：2026-11-06。验收人：pi `k3.1-pro`（provider `pl-anthropic`）。对象：源码两次冻结后的未提交候选（base = master `e3784b2`，本轮改动未 commit，候选身份以源码指纹清单为准）。实施/Search/审计 worker 均已 settled；本 worker 只做定向格式化与验收，未改行为源码、未弱化测试、未 commit/push。

## 环境

- 硬件：12th Gen Intel Core i7-12700KF，64 GB RAM；Windows 11（NT 10.0.26100）。
- 工具链：rustc/cargo 1.99.1；release profile = optimized + debuginfo。
- 基线 A：`target/table-wave7/baseline-build/release/matcha.exe`，SHA256 `f63125ce32b89102e8938e76ed479ac2bc50b0c8c59ea244d1ff523cf95d7c93`——由 e3784b2 `git archive` 快照独立构建（证据链见 `target/table-wave7/audit/environment.txt`）。
- 候选 B：`target/release/matcha.exe`，SHA256 `f124ca26b625e0f97dec50eed726153df24dd5857cc002e6e721ada9514e6f85`（本轮冻结源码 release 构建）。
- 源码指纹：`source-manifest-before.txt` / `source-manifest-after.txt`（crates/+apps/ 全部 208 个 .rs + 根 Cargo.toml/Cargo.lock 的 SHA256；不含 target/、.git/、tmp/）。**构建前与 bench 后逐字节一致**，期间无行为源码变化。
- 输入审计：文本 `audit/inputs/text-{left,right}.txt` 各 110,000,043 B（SHA `b8041ce6…`/`2b2d6627…`，自 tmp/ 只读复制）；表格 `table-{left,right}.csv` 各 69,258,868 B、1,000,000 行 × 10 列（SHA `1c7f090e…`/`53c033e4…`）。runner 运行前后复验输入 SHA 不变，验收后再次人工核对一致。

## 门禁（全部保存完整日志与 exit 码于 `target/table-wave7/validation/`）

| 门禁 | 结果 | 日志/exit |
|---|---|---|
| `cargo clippy --workspace --all-targets -- -D warnings` | **0 警告** | final-clippy.log / .exit=0 |
| `cargo test --workspace` | **372 passed / 0 failed / 3 ignored** | final-test-workspace.log / .exit=0 |
| 其中表格 GUI（table::） | **111 passed / 0 failed** | 同上 |
| `cargo test -p bc-engine-table --test golden_table -- --nocapture` | 3 tests ok；29/30 用例；**行 81/83 = 97.59%，格 236/242 = 97.52%（≥95%）**；唯一差异为既有 KNOWN_DIFFS 的 Unaligned 报告路径，**未扩大**（differences=1） | final-golden.log / .exit=0 |
| `cargo build --release --workspace` | 成功 | final-release.log / .exit=0 |
| `tools/table_wave7_parser_tests.ps1` | **11/11**（wave6 真实日志精确读出 text p99=1.347 / table p99=1.314；缺字段/帧数不足/非 0 退出/超时/轮次不足/A-A 同哈希均拒绝） | final-parser.log / exit 0 |
| `python tools/golden/test_table_parity.py`（命令对照校验器正确单测调用） | Ran 3 tests，OK | final-parity.log / .exit=0 |
| `openspec validate table-compare-xlsx --strict` | valid | final-openspec.log / .exit=0 |
| `git diff --check` | 干净 | final-git-diff-check.log / .exit=0 |
| 定向 `rustfmt --check`（本波 15 个 Rust 文件，edition 2021 + skip_children，不递归其他线） | 干净（本 worker 仅格式化了自有 session_history.rs/search_tests.rs 两处，主 worker 文件原本干净） | final-rustfmt-before/after.log / .exit=0 |
| 测试数量口径更正 | 实施报告“session 26、sheet 8”实为 **session 25、sheet 7**；总数 97（53+44）正确，经 `cargo test --lib table:: -- --list` 核实；冻结后总数升至 111（主 worker 修复期新增 review/sheets 测试）。仅改报告口径，未改测试 | — |

3 个 ignored 为既有项（两项表格压力、一个 Windows 手动回收站测试），本轮未单独运行，不冒充已测。

## 同轮交替 A/B（`tools/table_wave7_ab_bench.ps1 -Runs 3`，每侧每线 3 轮，30 预热 + 600 帧，轮序 run1/3=A,B、run2=B,A）

原始轮次：`target/table-wave7/audit/bench-{text,table}-{A,B}-run{1,2,3}.log`；汇总 `bench-ab-summary.csv`、`bench-ab-results.json`、`bench-ab-manifest.json`；解析回归保证无字段丢失。抽查 raw 日志与汇总一致（如 A-text run1 p99=1.247、B-table run2 p99=1.110）。

| 线 | 侧 | update mean（median, ms） | update p99 每轮（ms） | update p99 median | frame CPU p99 median |
|---|---|---|---|---|---|
| 文本 | A（e3784b2 基线） | 0.655 | 1.247 / 1.112 / 1.174 | **1.174** | 1.785 |
| 文本 | B（候选） | 0.650 | 1.253 / 1.158 / 1.160 | **1.160** | 1.814 |
| 表格 百万×10 | A | 0.634 | 1.005 / 1.195 / 1.126 | **1.126** | 2.628 |
| 表格 百万×10 | B | 0.634 | 1.075 / 1.110 / 1.016 | **1.075** | 2.664 |

判定：

- 表格线：B update p99 median **1.075 ms < 4 ms**、CPU p99 **2.664 ms < 8 ms** ✓（A 同样达标）。
- 文本线同轮 A/B：update p99 median B/A = **−1.2%**，update mean B/A = **−0.8%**，CPU p99 B/A = **+1.6%**；逐轮**区间重叠但非完全相等**（A p99 区间 1.112–1.247 ms，B 1.158–1.253 ms）。均在 5% 告警线内，**未发现文本线退化**；不需追加轮次。
- 表格线 B 相对 A update p99 −4.5%（更快），无回退。
- 输入/二进制 SHA 运行前后不变；源码指纹前后一致。

## 结论与边界

冻结候选在全部既定门禁与同轮 A/B 上通过；4.4/6.1 的勾选、报告合并与 commit 仍由总控执行。既有边界不冒称：3 个 ignored 未跑；物理鼠标/原生 IME 候选窗按 ADR 202 留人工点验；大表 Find/Save/Export 为前台同步路径，其耗时未在本轮测量（已记录为未测风险）；xls/ods 与外部转换器仍明确不支持。
