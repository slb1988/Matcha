# hex-compare worker report（验收记录，2026-11-12 Mac arm64）

## 7.1 大文件性能

语料：确定性 xorshift64 伪随机 256 MB 双份（中部 4 KB 插入 + 尾部 1 KB 翻转），release 构建，Mac arm64。

| 指标 | 实测 |
|---|---|
| Fast 档全量对齐 256 MB | **16.6 s**（same=268,430,326；orphan 分段 5,130 左 / 9,226 右，Fast 粗糙分段为规格允许行为） |
| 对齐中取消响应延迟 | **≈ 262 ms**（200 ms 后置取消，总返回 < 300 ms，规格要求 < 5 s） |
| 视图滚动 | 手工虚拟化（像素偏移 + 可见行裁剪），滚动不与内容规模耦合 |

内存安全：锚索引按哈希量化采样（>64 MB 时 h%8==0 入库，封顶约 n/8 项；小文件 S=1 与逐字节一致）。

复跑：`cargo test --release -p bc-engine-bin --test large_file -- --ignored --nocapture`。

## 7.3 快照对照（Matcha vs 参考 5.0.4.30422）

场景：`hex-basic` / `hex-insertion` / `hex-diffs-filter`（`--snapshot` 无头采集，与参考同一语料字节）。

**一致项**：菜单栏（Session/File/Edit/Search/View 五菜单结构）、工具栏命令集与单选组、File Info 头（字节数+时间戳）、双栏网格（8 位大写 hex 地址标签、8+8 分组、ASCII 列）、差异/孤儿红底红字、斜纹占位格（可行中）、整行孤儿无标签、状态栏（Binary differences | Insert | Load time | 每侧光标地址）、跨侧光标同步（0x400/0x410）、Diffs 过滤的区域+上下文与连续相位（0x1F7 型标签相位复现）。

**几何/着色差异（有意或后续跟进）**：
- 每行格数：参考截图 16（其窗口较窄）；Matcha 默认 Fit to window 随窗口宽度（本机 2x 窗口下约 44）。行为一致、几何不同。
- 缩略图条位置：参考工具在最左；Matcha 在右缘（沿用自身外壳惯例）。
- 光标单元格无着色框（状态栏地址有）；如要对齐可后续补绘。
- File Info 为只读摘要行，不含参考工具的可编辑路径下拉（路径编辑走 File > Open File）。

## 门禁（7.2）

- `cargo clippy --workspace --all-targets -- -D warnings`：零警告通过。
- `cargo test --workspace`：除 bc-engine-dir 的 4 个 **Mac 平台既有失败**（与 hex-compare 无关，首跑即现、未接触其代码与夹具：`golden_directory_statuses_and_structure`、`directory_copy_preserves_metadata_after_copying_children`、`rename_case_only_and_collision_checks`、`copy_touch_attrib_move_exchange_rename_and_session_effects`，均为 Windows 夹具语义）外全绿；Windows 侧不受此影响。
- `cargo build --release -p bcompare`：通过，`target/release/matcha` 产出。

## 已知阻塞

- **3.2 hex-report 黄金对拍**：期望输出须由参考工具 Pro 实例生成（本机 Standard，`@script` exit 107；GUI 报告采集因机器占用中止）。生成步骤与脚本已备于 `crates/bc-core/tests/golden/hex/README.md`；拿到期望后补对拍断言与 `KNOWN_DIFFS.md`。
- 参考工具 Fast 档实际语义为"区间逐位对齐"（REA 反汇编），本实现按设计 D1 取更保守的孤儿段（不产生跨位移假 same）——有意分叉，记录在案。
