# 文本比较（Text Compare / Edit）

> 核心功能：两个文本文件的差异对齐、着色、编辑与导航。状态：**已实现**（双向比较、编辑、
> 行内差异、重要性；三路合并 FR-T7 仍是脚手架）。对应 FR-T1…T8。

## 功能简介

- 打开两个文件 → 计算行级对齐（`TextMap`），按 `DiffKind` 着色；支持左/右或上/下布局。
- 行内字符级差异（inline diff，FR-T2）、不重要差异（Unimportant，蓝色语义）与重要性传播。
- 就地编辑：UTF-8 字节光标、选区、IME、undo/redo；保存可独立单侧进行。
- 语法高亮来自文件格式库（见 [sessions-and-formats.md](sessions-and-formats.md)）。
- 视图投影：All / Differences / Same / Context 区段折叠。
- 鼠标点窗格任意位置（包括只有对侧有内容的空隙行）都会把焦点和活动侧切到该窗格；
  空隙行上光标落到其后本侧第一行行首（`TextCompareView::place_in_gap`）。

## 代码地图

| 层 | 位置 | 内容 |
|---|---|---|
| 引擎 | `crates/bc-engine-text` | `TextContent`（单侧内容+EditStack）、`TextRules`、`TextMap`/`TextMapRow`、`importance`、`inline`、`merge`（脚手架）、`jobs`（Align2/Realign2/Align3/Realign3）、`search` |
| GUI | `crates/bc-gui/src/text/` | `view.rs`（控制器/导航/保存）、`render.rs`+`gutter.rs`（自绘）、`editor.rs`、`projection.rs`、`nav_bar.rs`（缩略条）、`worker.rs`（350ms 防抖 Realign2Job） |
| 启动 | `apps/bcompare` | `matcha <left> <right>` 直接进入 |

## 关键语义

- **重要性三态**：`Same / Unimportant / Important`；块级传播规则见 spec §6 与
  `docs/decisions/003-smart-scoring.md`。
- Live `TextContent` 只在 UI 线程；后台 worker 只拿 `snapshot()`（Send/Sync），
  过期修订号的结果直接丢弃。
- diff 显示经过 Myers  packing 与词吸附：见 ADR 005、006。
- 行模型、编码与 mmap 后端在 `bc-core`：见 ADR 001、002。
- 系统文件拖放：拖到哪一侧（窗格、路径栏、列头均可）就替换哪一侧；规则见 [gui-shell.md](gui-shell.md)「系统文件拖放落点」。

## 使用与验证

```powershell
cargo run --release -p bcompare -- a.txt b.txt                 # GUI
cargo test -p bc-engine-text                                   # 引擎测试
cargo run --release -p bc-engine-text --example text_baseline  # 轻量基线
```

## 深入阅读

- `crates/bc-engine-text/README.md`（API 与缓存/快照约束）
- ADR：`docs/decisions/001…007`（STX 写规则、mmap、智能评分、tokenizer 优先级等）
- 行为对齐：`docs/reference-parity/text-compare.md`、`text-alignment-recovered.md`
- 变更过程：`openspec/changes/text-compare-core/`
