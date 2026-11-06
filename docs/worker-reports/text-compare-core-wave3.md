# text-compare-core — Wave 3 实施报告

## 结论：部分完成，outcome = failed

本轮 **6.1–6.7 全部完成，7.2 完成**；任务 7.1、7.3 尚未达到验收，保持未勾选。OpenSpec 总进度 **28/40**，本轮 10 项完成 8 项，未开始任务组 8 及以后。

**未完成的关键结果是正式黄金语料验收**：30 个合成输入和生成器/解析器已入库，诊断模式实际生成了全部 60 份参考 XML，且共享 Rust 解析器验证 60/60 可解析；但这些报告继承参考默认重要性设置，并非各用例要求的已核实同设置期望。正式 corpus 的 expected XML 数量仍为 **0/60**，不能报告 Myers 100% 或 Standard ≥95%。

待确认的问题是 `msg_7ff4080efbd7`：参考默认 Myers 的输出行打包与项目 Quick 不同，需要决定展示层打包或明确 NeverAlignDiffs 配置；同时需要确认普通重要性发布字段检查，以生成同设置规则模板。四次各 10 分钟 ask 等待及后续 check 未收到答复，已发送 escalation；没有自行改变已验收算法或伪造期望。本报告按协调者的失败交付约定保留所有已完成实现与证据。

## 完成与未完成清单

| 任务 | 状态 | 证据 |
|---|---|---|
| 首步 spec 提交 | 完成 | 单独提交协调者的最长匹配澄清，见提交表 |
| 6.1 | 完成 | [content 测试](../../crates/bc-engine-text/tests/content.rs)：编码/BOM/混合行尾往返、错误计数、编码切换、缓存失效、物化保存且保留 undo/redo、外部映射锁失败不截断 |
| 6.2 | 完成 | [importance 测试](../../crates/bc-engine-text/tests/importance.rs)：语法掩码、关键字格式大小写优先、空白/EOL、时间戳/双向/全词规则、混合区块；另补规则匹配先于全局 case fold 的回归 |
| 6.3 | 完成 | [inline 测试](../../crates/bc-engine-text/tests/inline.rs)：count/total、纯插入、碎片 gap 合并、getValue/setValue、Unicode byte span、注释及混合大小写重要性、500 KB 行有界降级 |
| 6.4 | 完成 | [map_jobs 测试](../../crates/bc-engine-text/tests/map_jobs.rs)：区块/三态/导航/统计、忽略不重要、复制插删区块、EOF 拼接、一次 undo 恢复文本与 map；12 B 紧凑行及 110 MB 基准见下 |
| 6.5 | 完成 | 同一测试验证真实 std worker 调用 Job::run、过期结果拒绝、窗口重算与全量结果相等、插入后后缀索引平移；安全锚点不可信时退化全量 |
| 6.6 | 完成 | [search 测试](../../crates/bc-engine-text/tests/search.rs)：TextMap 提供差异范围、方向/大小写/全词/正则、捕获替换、多行替换、零宽进展、一组 undo 全部恢复、只读拒绝 |
| 6.7 | 完成 | [conflict 测试](../../crates/bc-engine-text/tests/conflict.rs)：区块边界/上下文/行号/三态/EOL/转义往返、格式校验；协议在 [engine README](../../crates/bc-engine-text/README.md) |
| 7.1 | **未完成** | [生成器](../../tools/golden/gen_expected.ps1) 可跑诊断并生成全部 60 XML；正式模式仍缺已核实的同配置规则模板，预检明确拒绝猜测 |
| 7.2 | 完成 | [core 回归](../../crates/bc-core/tests/golden_align.rs)、[共享解析器](../../crates/bc-core/tests/golden/support/mod.rs)、[engine 回归](../../crates/bc-engine-text/tests/golden_importance.rs)；已验证真实 same/different/similar/orphan/leftsimilar、空 corpus 跳过、部分 corpus 拒绝 |
| 7.3 | **未完成** | [30 个输入](../../crates/bc-core/tests/golden/text/2way) 已覆盖要求类型，但正式期望及一致率验收未完成；[KNOWN_DIFFS](../../crates/bc-core/tests/golden/KNOWN_DIFFS.md) 区分诊断与正式白名单 |

最终 `cargo clippy --workspace --all-targets -- -D warnings` 零警告、`cargo test --workspace` 100 个测试函数通过、`cargo build --release -p bcompare` 通过。**其中两个正式黄金入口走了“oracle 缺失”跳过分支；这不是黄金一致率通过。** 其余测试含真实诊断 XML 的解析及当前 Myers 孤立行模式对照。

## TextMap 110 MB 基准

命令：

```powershell
cargo run --release -p bc-engine-text --example text_map_bench -- tmp/left.txt tmp/right.txt
```

Windows x86_64 MSVC / Rust 1.99.1 / release，两输入各 **110,000,043 B**，合计 **220,000,086 B**，纯文本格式。计时中的 map 构建包括 worker 执行、哈希、对齐、分级、紧凑化及应用结果；不含单独列出的内容加载或 PowerShell 内存采样。

| 指标 | 初测 | 最终复测 |
|---|---:|---:|
| 两侧内容加载 | 420.957 ms | 259.718 ms |
| TextMap 构建 | 833.171 ms | **811.881 ms** |
| 输出行数 | 2,141,334 | 2,141,334 |
| map 自身存储（含可复用哈希/区块） | 60,394,168 B | 60,394,168 B |
| 进程私有内存增长 | 226,844,672 B | **227,627,008 B** |
| 增量 / 输入总字节 | 1.031112× | **1.034668×** |

低于 2× 私有内存预算。内部行是两个 u32 索引 + kind/importance 的 12 B 记录；公开 TextMapRow 按需派生，没有再保存完整 AlignMap 与 TextMapRow 两套百万行数组。基准未强制清空 OS 缓存，内存为结果存活时的 PrivateMemorySize64 增量，不是全时段峰值；语法复杂格式的缓存成本不能由此纯文本基准直接推断。

## 接口调整与下层最小修改

### bc-engine-text

- `TextContent::lines` 改为 SharedLines；内容/格式 revision 独立追踪。格式通过 set_format 更新，异步消费者只拿 Send/Sync ContentSnapshot，不发送 Rc 编辑目标。
- SyntaxCache 编译一次 tokenizer；每 256 行保存状态检查点，哈希扫描不保留全文件 token 列表；差异行 token 留存，mask-only reclassify 不重新分词。取消检查可中止状态链扫描。
- 保存分为 safe save_as 与显式 materialize_and_save。原路径保存前必须取消/join 相关 worker 并释放外部映射快照；物化覆盖当前/原始/undo/redo/group，共享 map 只复制一次，不清除历史。残留外部锁时原文件保持完整，可释放后重试。
- TextMap.rows 是派生迭代器、text_at 返回借用 guard；align_map 是显式昂贵兼容物化，不应每帧调用。copy_section 返回已应用的 EditItem，需记录到目标历史。
- Align2Job/Realign2Job 接受快照、规则和独立 AlignmentOverrides；结果含 revision，旧结果被拒绝。Realign 使用已保存的哈希与安全唯一 Same 锚点、修正插删后的后缀索引；找不到可靠边界或设置改变则全量。
- 查找/替换返回 Result；差异专属范围通过 TextMap 便捷方法提供，裸 search API 缺上下文会报错。替换在全部规划完后逆序应用为一个 undo 组。
- ignore_unimportant 只改变视图/导航/计数，不影响缓存有效性或原始 conflict 导出。
- Conflict v1 是可转义的 UTF-8 纯文本，包含区块与 context 半开范围、1-based 行号、三态、来源侧、EOL 与文本；解析验证结构，不是 AI 接入或导入编辑 API。

### 下层逐项列出

| 文件 | 本轮必要调整 |
|---|---|
| `bc-core/src/line.rs` | 全局唯一内容 revision；共享 slice/replace_range；MaterializeContext 与 materialize_owned；LineEdit 的区间替换与历史物化 |
| `bc-core/src/edit.rs` | EditItem 默认 materialize 回调；EditGroup/EditStack 遍历 undo/redo/open groups，保留历史语义 |
| `bc-core/src/pattern.rs` | 捕获感知、可过滤的 replace_all_if，保留原锚点上下文及零宽前进 |
| `bc-core/src/diff.rs` | 为已有 Patience diff 暴露 cancellable 入口；未改 Myers/Smart 默认算法或阈值 |
| `bc-formats/src/grammar.rs` | Cow 格式持有与 Tokenizer::into_owned，供长期缓存使用 |
| engine Cargo.toml / Cargo.lock | 仅增加已存在 workspace quick-xml 的 dev 引用；没有新增第三方包或版本 |
| `bc-jobs` | 无修改；JobManager 仍不实现 |

其余 engine/gui/vfs crate 未改。三路合并与三路 jobs 保持后续范围，unused_variables 豁免只留在三路占位方法。

## 黄金一致率、证据和阻塞

| 项目 | 当前结果 |
|---|---|
| 合成用例输入 | 30/30，均已入库 |
| 正式同设置期望 | **0/60** |
| Myers/Quick 一致率 | **未验证，不能报 100%** |
| Standard/Smart 一致率 | **未验证，不能报 ≥95%** |
| 完整诊断生成 | 60/60，位于 ignored target/golden-all-diagnostic |
| 诊断 XML 解析 | 60/60 成功 |
| 入库原始诊断 XML | 9 份，位于 tools/golden/evidence，非正式 oracle |
| KNOWN_DIFFS | 正式条目 0；诊断记录 4，不作为回归白名单 |

已通过公开 CHM 和正常脚本确认 text-report XML。锁屏时未尝试解锁；在协调者批准后只对配置 strings/RTTI/Alignment 窗体资源做定向只读检查，没有反汇编算法/授权逻辑，也没有读取或复制任何授权/密钥文件。

发现并黑盒确认：

1. 命名会话直接位于 BCSessions 根级；根内无名 TSessionFolder 是默认/历史内容，不是新增命名会话的位置。
2. Rules 必须带 `Value="TTextRules"`。对齐属性为 AlignKind/NeverAlignDiffs/SkewTolerance/UseCloseness。
3. 内部 alignQuick 对应公开 Myers O(ND)，alignSmart 对应 Standard；Standard + closeness 为项目 Smart 的参考策略。alignNone 控制探针与标准对齐产生不同 XML，证实设置路径生效。
4. 参考 Myers 默认 NeverAlignDiffs=False 把替换报告为 Changed，True 才是当前 Quick 的孤立行输出。Standard 也会把完全不相似替换配对，与项目明确“不强配”要求不同。
5. 仅尾换行差异的默认诊断 XML 为 same，而当前引擎保留 Unimportant；需要核实行尾选项/报告语义，不能通过偷偷修改用例来掩盖。

这些结果及文件偏移在 [tools/golden/README](../../tools/golden/README.md) 和 [ADR 003](../decisions/003-smart-scoring.md)。重要性普通字段的进一步检查尚待批准，因此没有猜测模板；正常生成模式会在改用户设置前拒绝缺少的 verified template。诊断模式强制禁止写入正式 corpus。

待决消息 `msg_7ff4080efbd7` 的选择是：保持 Myers SES，在 Quick 输出层加入展示打包；或正式 Myers 黄金明确指定 NeverAlignDiffs=True。还需核实普通重要性模板及 EOF 语义，再生成/逐行验证 60 份正式期望，最终填写实际一致率和白名单。

## 参考会话配置安全

最初的 BCSessions.xml 备份位于本机 `tools/golden/backup/20261105-222756/BCSessions.xml`。该目录已 gitignore，仓库只跟踪 backup/.gitignore，**没有提交私人会话内容**。

手工探针后，已比较排除两个工具节点的树，确认其他会话不变，再恢复初始文件；后续生成器每次都先备份、只临时替换两个工具会话、在 finally 恢复本次原字节。交付前再次确认当前 BCSessions.xml 与最初备份逐字节相等。所有 worker 启动的参考进程均已结束。

如需恢复：先退出参考工具，再把适当时间戳下的 BCSessions.xml 复制回明确的 SessionsPath；不要覆盖备份之后的新用户更改，也不要读取/复制目录中的其他文件。脚本不写安装目录，不接触 tmp 基准输入。

## 规格澄清与后续建议

- 第一项要求的 file-format-library spec 澄清已独立提交。
- 内部 Quick/Smart 与公开策略名的映射、词边界/gap 优先级得到协调者确认；[ADR 005](../decisions/005-inline-diff-word-snap.md) 记录多碎片合并不再吸附未变词尾。
- 协调者后续修改的 text-diff-alignment/text-inline-diff spec，以及另一个 folder-compare-sync change，仍留在工作区，worker 没有编辑或纳入提交。
- 8.x UI 保存必须遵守 [mmap 保存协议](../decisions/002-mmap-backend.md)，撤销/其他途径修改后需检查 map stale 并调度 Realign；本轮没有开始 8.x。
- 未访问 ObsidianVault，未修改 tmp 输入，未新增授权、试用或注册逻辑。

## 提交记录

| 阶段 | commit |
|---|---|
| 指定 spec 澄清 | `44810622016e67240f008f9dba12c47aa5a693a5` |
| 任务组 6 主实现 | `92a0452751cddb1ac2882f382f3fc739ee715744` |
| 6.x 大小写边界修复 | `59d1ec6ee3847c830a2a35a4d2d91651bda7cc54` |
| 6.x 视图开关缓存修复 | `8242a10b0e4de769e863632d6078faed5f35dfa0` |
| 任务组 7 准备与生成器 | `a3a80218ca33fdc5d4f01a86d03420a9e441e5a2` |
| 7.2 真实 minor 状态/生成器修复 | `fa10aca3e9c565a823af7606fa56ce7a56027c0d` |
| 诊断发现与剩余差异 | `1e52759ffe12681d2616ad478e207442d896d7b3` |

所有提交均有 `Co-Authored-By: pi <noreply@pi.dev>`；本报告另作 docs 提交。输入中的有意尾空白及原始 XML CRLF 通过文件级 gitattributes 保留，未“修复”测试数据以迎合 whitespace check。

## 命令输出最后 10 行

### cargo clippy --workspace --all-targets -- -D warnings

退出码 0，零 warning。

```text
    Checking bc-core v0.1.0 (D:\Github\Matcha\crates\bc-core)
    Checking bc-engine-text v0.1.0 (D:\Github\Matcha\crates\bc-engine-text)
    Checking bc-engine-dir v0.1.0 (D:\Github\Matcha\crates\bc-engine-dir)
    Checking bc-session v0.1.0 (D:\Github\Matcha\crates\bc-session)
    Checking bc-report v0.1.0 (D:\Github\Matcha\crates\bc-report)
    Checking bc-gui v0.1.0 (D:\Github\Matcha\crates\bc-gui)
    Checking bc-script v0.1.0 (D:\Github\Matcha\crates\bc-script)
    Checking bcomp v0.1.0 (D:\Github\Matcha\apps\bcomp)
    Checking bcompare v0.1.0 (D:\Github\Matcha\apps\bcompare)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.21s
```

### cargo test --workspace

退出码 0；100 个函数通过，其中正式黄金两入口因 oracle 全缺失跳过。最后为无 doctest 的 crate；完整日志在 target/wave3-test.log。

```text
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests bcomp

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```

### cargo build --release -p bcompare

退出码 0，不足 10 行则列全部。

```text
   Compiling bc-engine-text v0.1.0 (D:\Github\Matcha\crates\bc-engine-text)
   Compiling bc-engine-dir v0.1.0 (D:\Github\Matcha\crates\bc-engine-dir)
   Compiling bc-session v0.1.0 (D:\Github\Matcha\crates\bc-session)
   Compiling bc-report v0.1.0 (D:\Github\Matcha\crates\bc-report)
   Compiling bc-gui v0.1.0 (D:\Github\Matcha\crates\bc-gui)
   Compiling bc-script v0.1.0 (D:\Github\Matcha\crates\bc-script)
   Compiling bcompare v0.1.0 (D:\Github\Matcha\apps\bcompare)
    Finished `release` profile [optimized + debuginfo] target(s) in 2.58s
```

### TextMap 最终基准

```text
   Compiling bc-engine-text v0.1.0 (D:\Github\Matcha\crates\bc-engine-text)
    Finished `release` profile [optimized + debuginfo] target(s) in 1.93s
     Running `target\release\examples\text_map_bench.exe tmp/left.txt tmp/right.txt`
bench.content_load_ms=259.718
bench.text_map_ms=811.881
bench.rows=2141334
bench.map_storage_bytes=60394168
bench.difference_sections=(21851, 0)
bench.private_growth_bytes=227627008
bench.private_input_ratio=1.034668
```
