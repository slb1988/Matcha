# 十六进制比较（Hex Compare）

> Hex Compare 已实现（openspec `hex-compare` 21/23；3.2 黄金对拍待参考机生成期望，7.2 门禁在 Mac 有 4 个无关既有夹具失败）。
> Picture/Version/Registry/Media compare 仍为规划中的骨架（`bc-engine-bin` 其余模块 `todo!()`）。

## 功能简介

- 三档字节对齐：**Complete**（默认，锚块骨架 + 小区间字节级细化）/ **Fast**（超大文件启发式，
  区间不细化降级孤儿段）/ **None**（逐位对齐，长度差尾部孤儿）；四分类统计
  same/difference/left orphan/right orphan，每侧合计 = 文件大小。
- 双栏 hex 视图：**单元格行打包**（默认 16 格/行，Fit to window 随宽度自适应）——孤儿字节占格、
  对侧斜纹占位格可在行中；行标签 = 该侧本行首个真实字节偏移；Diffs 过滤的差异区域带上下文
  （默认 16）且**区域连续流入同一打包序列**（行标签相位随之非 16 对齐）。
- 交互：Next/Previous Difference Bytes（Ctrl+N/P）、Go To（hex/dec、选侧、跨侧同步）、
  内联查找栏（hex/文本、大小写）、Replace、字节编辑（Insert/Overwrite、Cut/Copy/Paste/Delete、
  Select All、Undo/Redo、Copy to Other Side Ctrl+R）、Swap/Reload（脏确认）/Recompare、
  Hex Compare Info（四分类）、Session Settings 三页（Specs/Format/Comparison）。
- 加载分层：< 2 MB（可配）整载不加锁可编辑；≥ 阈值 mmap 只读（初版禁编辑/保存）。
- 入口：Home 会话卡、`*.bin` 掩码路由、`/fv="Hex Compare"`、Text/Table 的 Compare Files Using 互切、
  空会话逐侧打开。

## 代码地图

| 层 | 位置 | 内容 |
|---|---|---|
| 对齐原语 | `crates/bc-core/src/align.rs` | `ByteAlign` 三档 + 锚窗/细化上限常量（REA 校准） |
| 引擎 | `crates/bc-engine-bin/src/hex/` | `mod.rs`（HexRules/HexStats/HexMap/pack_rows/filtered_rows）、`content.rs`（加载/保存/写入门禁）、`jobs.rs`（后台对齐 + revision） |
| 视图 | `crates/bc-gui/src/hex/` | `view.rs`（控制器/命令/编辑/导航）、`render.rs`（网格模型+绘制）、`menu.rs`（五菜单）、`settings.rs`（三页设置）、`worker.rs`（后台 worker） |
| 黄金语料 | `crates/bc-core/tests/golden/hex/` | `corpus.rs` 确定性生成器（与 docs 语料逐字节一致） |

## 关键语义

- 参考行为真源：`docs/reference-parity/hex-compare.md`（含 REA 校准的对齐常量与实现对照补记）。
- Fast 档与参考工具的有意分叉：参考工具区间逐位对齐，Matcha 区间降级孤儿段（杜绝跨位移假 same；
  见 worker-report.md）。
- 验收与性能数字：`openspec/changes/hex-compare/worker-report.md`（256 MB Fast 对齐 16.6 s、
  取消响应 ≈262 ms、快照对照、已知差异）。

## 使用与验证

- GUI：Home 点 Hex Compare 卡，或把两个 `.bin` 拖入；CLI：`matcha a.bin b.bin` 或
  `matcha /fv="Hex Compare" a b`。
- 测试：`cargo test -p bc-core --lib byte_align`、`cargo test -p bc-engine-bin`、
  `cargo test -p bc-gui hex`（RawInput 回归）；256 MB 基准
  `cargo test --release -p bc-engine-bin --test large_file -- --ignored`。
- 黄金对拍（3.2 待办）：参考工具 Pro 实例按 `crates/bc-core/tests/golden/hex/README.md` 生成期望后启用。

## 深入阅读

- `crates/bc-engine-bin/README.md`
- `docs/reference-parity/hex-compare.md`
- `openspec/changes/hex-compare/`
