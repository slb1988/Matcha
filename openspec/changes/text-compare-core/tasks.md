# Tasks

说明：每个任务以一次 agent 会话可完成为粒度；完成判据写在任务内。全程约束：`cargo clippy --workspace --all-targets` 零警告、`cargo test --workspace` 通过、不引入 GUI/异步依赖到引擎 crate、不改动 vault 分析文件。实现某模块后删除该文件顶部的 `#![allow(unused_variables)]` 豁免范围（改为仅剩余 `todo!()` 处局部 `#[allow]`）。

## 1. 依赖与基线

- [x] 1.1 在 workspace 加入 `regex`、`xxhash-rust`（xxh64）、`encoding_rs`，分别接入 `bc-core`；验证 `cargo tree -p bc-core -e normal` 不含 GUI/异步 crate 且 `cargo check --workspace` 通过
- [x] 1.2 为 `bc-core`、`bc-formats`、`bc-engine-text` 建立 `tests/` 单测骨架与 criterion 之外的轻量基准入口（`cargo test`/`cargo run --example`），验证空测试集可运行
- [x] 1.3 把骨架阶段 README 中"遗留问题"里与本 change 相关的条目（STX 写规则、`TLinesTextGrammarItem`、Smart 评分）转为 `docs/decisions/` 下的决策记录占位；验证文档链接可达

## 2. bc-core 行模型、编码与哈希

- [ ] 2.1 实现 `line::LineParser` / `LineList::from_bytes` / `LineWriter`：逐行行尾、末行无换行、主导风格检测；验证单测覆盖 CRLF/LF/CR/混合/空文件/仅一行无换行的往返逐字节相等
- [ ] 2.2 实现 `LineList` 的 insert/delete/replace/set_ending 及对应 `EditItem`（`TSsLineList*Item` 等价）；验证单测：任意操作序列 undo 后缓冲区与原始相等
- [ ] 2.3 实现 `edit::EditStack`（push/coalesce/group/undo/redo/save point/limit）；验证单测覆盖分组嵌套、保存点脏判定、超限截断
- [ ] 2.4 实现 `encoding::detect/decode/encode`（BOM、UTF-8 有效性、NUL 密度二进制判定、`encoding_rs` 代码页映射表）；验证单测：UTF-8 有/无 BOM、UTF-16LE/BE、GBK、1252、二进制样本按规格判定，保存保持 BOM 策略
- [ ] 2.5 实现 `hash::LineHasher`（空白/大小写标准化 + xxh64）与 `Crc32`；验证单测：标准化等价行哈希相等、不等价行不等，CRC 与已知向量一致
- [ ] 2.6 实现 `LineList` 的只读 mmap 后端与首次编辑时的写时拷贝（设计 D6）；验证基准：打开 100 MB 文件 < 200 ms，编辑一行后内存增长 < 文件大小的 10%

## 3. bc-core 正则与 diff

- [ ] 3.1 实现 `pattern::Pattern`（`regex` 后端 + 方言改写 `\<` `\>` 等，大小写开关，find_at/match_at/replace_all）与 `glob_to_pattern`；验证单测：25 种内置格式的全部正则可编译，掩码 `*.c;*.h;-*.bak;obj\` 的匹配表
- [ ] 3.2 实现 `diff::MyersDiff`（前后缀裁剪、O(ND)、超大输入分块回退阈值）；验证单测：空/相同/全不同/交错插入用例，随机序列与朴素 LCS 结果等价（属性测试 1000 轮）
- [ ] 3.3 实现 `diff::PatienceDiff`；验证单测：唯一行锚点用例与经典 patience 示例

## 4. bc-core 对齐

- [ ] 4.1 实现 `AlignMap::from_rows`、反向索引、`next/prev_difference`、`force_pair`；验证单测：顺序保持不变量（规格"顺序保持"）、导航首尾提示
- [ ] 4.2 实现 `QuickAlign::align2`（LCS 骨架 → 行）；验证单测：规格场景"中间插入 3 行"、"完全相同"
- [ ] 4.3 实现 `SmartAlign::align2`：区块内相似度配对（字符 LCS 比率 + 词 Jaccard）、权重锚点、偏移容忍、不相似不强配；验证单测：规格场景"单字符改动配对"、"不相似替换不强配"、DFM `object` 锚点
- [ ] 4.4 实现对齐的后台分块与取消检查（配合 `bc-jobs::CancelToken`），验证基准：两份 100 MB 合成文本 Quick 对齐 < 5 s、Smart < 15 s（release），期间可取消

## 5. bc-formats 分词与格式解析

- [ ] 5.1 实现 `grammar::Tokenizer`：语法项编译（Basic/Delimited/List/Columns）、逐行扫描、跨行 Delimited 状态、先匹配者优先、大小写；验证单测：CPP 跨行注释、BASH 字符串转义、COBOL 73–80 列、PYTHON 三引号、DFM 关键字大小写
- [ ] 5.2 实现 `FormatRegistry::resolve/upsert/import_xml/export_xml` 与用户格式持久化目录；验证单测：掩码顺序优先、用户格式覆盖内置、未知语法项类型报错含格式名；导出再导入等价
- [ ] 5.3 实现 `bcff::write_bcfileformats` 与 `bc_core::persist::write_xml`（STX 规则按读取时记录的 `Node::stx` 回写）；验证单测：内置 XML 读→写→读 的模型相等，且与原文件的差异仅限空白/属性顺序（记录到决策文档）
- [ ] 5.4 实现 25 种内置格式的分词冒烟语料（每种格式一个样例文件 + 期望元素序列）；验证 `cargo test -p bc-formats` 全绿

## 6. bc-engine-text 引擎

- [ ] 6.1 实现 `TextContent::from_bytes/to_bytes`（检测 → 解码 → 分行；保存时编码/BOM/行尾策略）；验证单测：规格 `text-encoding` 全部场景
- [ ] 6.2 实现 `importance::classify_pair/propagate_block` 与 `ElementImportance`、不重要文本规则、替换规则（含双向/全词）；验证单测：规格 `text-importance` 的三态、关键字大小写优先、时间戳替换、双向替换、混合区块
- [ ] 6.3 实现 `inline::inline_diff`（字符 diff → 碎片合并 → 词边界吸附 → 区间重要性 → 超长行退化）；验证单测：规格 `text-inline-diff` 全部场景与 500 KB 单行耗时上界
- [ ] 6.4 实现 `TextMap`：行/区块/三态构建、`text_at`、导航（含忽略不重要）、`difference_counts`、`copy_section`（作为 `EditItem`）；验证单测：忽略不重要开关影响导航与计数；复制区块后该块变为相同且可撤销
- [ ] 6.5 实现 `jobs::Align2Job/Realign2Job`（快照 → 哈希 → 对齐 → 分级；脏范围扩展到锚点的窗口化重算）；验证单测：局部重算结果在脏范围外与全量一致
- [ ] 6.6 实现 `search::find_next/replace_all`（字面/正则/全词/方向/仅差异；全部替换为一个撤销组）；验证单测：仅差异查找只命中非相同行，replace_all 一次 undo 全部恢复
- [ ] 6.7 实现 `TextMap::to_conflict_text()`：把指定区块导出为带行号、三态、前后文的纯文本块（为后续 AI 冲突包预留）；验证单测：导出可被重新解析回区块边界

## 7. 黄金语料

- [ ] 7.1 编写语料生成脚本 `tools/golden/gen_expected.ps1`：用本机已安装的参考工具以脚本模式对 `tests/golden/text/2way/*` 生成 `text-report layout:xml`（Quick 与 Smart 各一份）；验证脚本在本机跑通并产出期望文件（期望文件入库，脚本不依赖参考工具存在才能跑测试）
- [ ] 7.2 编写期望文件解析器与回归测试 `bc-core/tests/golden_align.rs`、`bc-engine-text/tests/golden_importance.rs`；验证测试在语料目录为空时跳过、有语料时逐行比对
- [ ] 7.3 建立首批 30 个合成用例（插入/删除/移动块/相似改动/纯注释改动/缩进改动/行尾差异/空文件/超长行/混合编码）并生成期望；验证 Quick 一致率 100%、Smart 一致率 ≥ 95%，不一致用例归档到 `tests/golden/KNOWN_DIFFS.md`

## 8. bc-gui 文本比较视图

- [ ] 8.1 实现 `TextView` 控件基础：按 `TextMap` 行渲染左右窗格、空隙行、行号栏、三态与行内着色（颜色来自 `Prefs::DiffColors`）、滚动同步、上下/左右布局切换；验证手动：打开两份差异文件着色符合规格；`--bench` 两侧 100 MB update p99 < 4 ms
- [ ] 8.2 实现显示过滤 All/Differences/Same/Context（含折叠分隔符展开）与差异导航命令；验证手动对照规格场景；单测折叠区段计算
- [ ] 8.3 实现缩略图导航条（色块、可视区标记、点击跳转）；验证手动：点击 70% 处跳到对应行
- [ ] 8.4 实现编辑：光标、选区、键盘输入/删除、剪切复制粘贴、IME 提交事件、只读侧拒绝、编辑映射到 `EditStack`，防抖触发 `Realign2Job`；验证手动：补字后 0.5 s 内变相同；撤销恢复着色；Windows 中文 IME 可输入
- [ ] 8.5 实现保存/另存为/另存目标/备份与关闭提示、状态区（行列/编码/行尾/格式/只读/已修改）、编码与格式手动切换菜单；验证手动：仅保存右侧、无 BOM UTF-8 保存后逐字节仅改动处不同
- [ ] 8.6 实现查找/替换面板与 Copy to Left/Right、Swap Sides、Reload、Recompare、拖放到窗格、从剪贴板打开；验证手动对照规格 `text-compare-view` 对应场景
- [ ] 8.7 实现会话设置面板（对齐策略、重要性开关与元素表、不重要文本/替换规则、上下文行数）并即时生效；验证手动：切换 Quick/Smart 与忽略不重要立即刷新
- [ ] 8.8 把 Home 页接到真实视图（最近文件、拖放两文件即比较），移除原型 `text_view` 中被取代的代码并保留 `--bench`；验证 `cargo run --release -p bcompare` 无原型残留且基准可运行

## 9. 集成验证

- [ ] 9.1 按设计 D10 的对照清单逐项核对 `commandstext.html`/`sessiontext*.html`/`viewtext.html` 中的命令与选项，在 `docs/reference-parity/text-compare.md` 记录"已实现/有意未采纳(理由)/后置"；验证文档覆盖帮助主题中的每一条命令
- [ ] 9.2 全量回归：`cargo clippy --workspace --all-targets` 零警告、`cargo test --workspace` 通过、黄金语料一致率达标、`--bench` 指标不低于骨架阶段；验证把结果写入 `docs/ui-framework-decision.md` 的"正式控件复测"小节

## Workflow follow-up

- 评审通过后归档本 change，并以其 specs 为基线开启 `folder-compare-sync`。
