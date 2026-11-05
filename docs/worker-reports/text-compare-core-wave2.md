# text-compare-core — Wave 2 实施报告

## 结果

**任务组 4–5 的 8 项全部完成**：4.1–4.4、5.1–5.4 均已勾选；OpenSpec apply 进度 **20/40**。任务组 6 及以后没有开始，剩余 20 项。

最终验证：`cargo clippy --workspace --all-targets -- -D warnings` 零警告，`cargo test --workspace` **73 个测试通过、0 失败**（bc-core 50、bc-formats 21、bc-jobs 2），`cargo build --release -p bcompare` 通过。25 格式 fixture 的校验包含在一个覆盖全部格式的测试中，不把 25 个样本误计为 25 个 Rust test。

worker 代码、测试、文档均已分组提交。工作区另有**协调者修改的** `openspec/changes/text-compare-core/specs/file-format-library/spec.md`（最长匹配规则澄清），worker 没有编辑、暂存或提交它，留给协调者处理。

## 完成清单与证据

| 任务 | 实现及验证 |
|---|---|
| 4.1 | AlignMap checked/from_rows、三侧反向索引、差异区块导航、force_pair；[align.rs 测试](../../crates/bc-core/tests/align.rs) 验证连续顺序不变量、非法输入、首尾不循环、所有 60×60 强制配对组合 |
| 4.2 | Quick 直接展开 Myers runs；同一测试覆盖完全相同、中间插入 3 行、空输入、无对齐模式、手动锚点复用与清除 |
| 4.3 | 有界字符 LCS + 词 Jaccard、阈值、偏移、权重优先；[smart_align.rs](../../crates/bc-core/tests/smart_align.rs) 覆盖相似改动、不强配、DFM 权重、Unicode/长行；[tokenizer.rs](../../crates/bc-formats/tests/tokenizer.rs) 用真实 DFM 格式权重再做跨 crate 验证 |
| 4.4 | core CancelCheck、jobs CancelToken 桥接、diff/展开/索引/评分取消点；[alignment_cancel.rs](../../crates/bc-jobs/tests/alignment_cancel.rs) 和 [align_bench.rs](../../crates/bc-jobs/examples/align_bench.rs) 验证真实后台线程与取消；100 MB 级数字见下 |
| 5.1 | 每格式编译一次、Basic/Delimited/List/Columns、跨行状态、字面词首字符桶、大小写、转义与零宽保护；tokenizer 测试涵盖 CPP/BASH/COBOL/PYTHON/DFM、HTML/PERL/VB 前缀冲突、Unicode 固定列及最长匹配/等长顺序 |
| 5.2 | 注册顺序与 kind 限制、正向掩码、用户覆盖、事务式导入、用户 XML 导出、Matcha 设置目录与顺序持久化；[registry.rs](../../crates/bc-formats/tests/registry.rs) 覆盖重启、重复原子写、未知语法类上下文及导出再导入 |
| 5.3 | core XML 写入、来源树最小补丁、STX 保留与新建规范；[xml_roundtrip.rs](../../crates/bc-formats/tests/xml_roundtrip.rs) 验证 typed model / Node tree / 文本三层往返；[persist.rs](../../crates/bc-core/tests/persist.rs) 覆盖转义、空白、Unicode 节点名、header |
| 5.4 | [fixtures](../../crates/bc-formats/tests/fixtures/README.md) 中 25 份合成输入 + 25 份手写期望 JSON；[builtin_tokenizer_fixtures.rs](../../crates/bc-formats/tests/builtin_tokenizer_fixtures.rs) 检查每行 token 文本/byte span/元素/跨行状态，并核对全部 25 个唯一 kind/name |

5.3 写出器是 5.2 export/save 的前置依赖，因此实际先完成 5.3，再完成 5.2；没有以占位实现勾选任务。内置 XML 未修改，所有 fixtures 均为合成文本；无语法的非文本/转换格式明确测试空 token 序列，不宣称实现了转换器或二进制视图。

## 4.4 性能与取消

环境：Windows x86_64 MSVC，Rust/Cargo 1.99.1，release；两份输入各 **110,000,043 B**，合计 **220,000,086 B**，通过 Wave 1 的只读映射后端读取。原输入 `tmp/left.txt` / `tmp/right.txt` 没有写入。

```powershell
cargo run --release -p bc-jobs --example align_bench -- tmp/left.txt tmp/right.txt
```

| 运行 | Quick 对齐 | Smart 对齐 | Quick 取消 | Smart 取消 |
|---|---:|---:|---:|---:|
| 初测 | 0.099027 s | 0.238122 s | 0.111 ms | 0.054 ms |
| 加内存验收复测 | 0.091613 s | 0.247843 s | 0.083 ms | 0.052 ms |
| 最终复测 | **0.093124 s** | **0.249894 s** | **0.089 ms** | **0.060 ms** |

最终加载两文件合计 195.664 ms，哈希合计 84.187 ms；上表对齐计时包含 worker spawn/join、Myers、Smart（对应模式）、row 展开和反向索引，不包含加载/哈希与 PowerShell 内存采样。即使把加载/哈希一起算入，Quick 约 0.373 s、Smart 约 0.530 s，仍远低于 5 s / 15 s 阈值。两种模式均得到 2,141,334 个输出行。

取消演示不是预先取消：worker 首次进入检查点通知主线程，继续运行 10 ms 后主线程调用真实 `CancelToken::cancel`，测量请求到 join 返回 `CoreError::Cancelled` 的延迟；均小于 1 s。普通回归还用确定性检查计数覆盖 prefix、分块与输出阶段。

额外内存验收（最终输出仍存活时，减去加载前的 `Get-Process.PrivateMemorySize64`）：

| 模式 | 私有内存增长 | 相对于两输入总字节 |
|---|---:|---:|
| Quick | 388,431,872 B | 1.7656× |
| Smart | 389,394,432 B | 1.7700× |

均低于规格的 **2×（440,000,172 B）**。这是进程私有提交内存的采样，不是 OS 全时段峰值或冷盘测试；benchmark 在超过 2×、超时或没有真正取消时直接失败。Smart 使用原 row 数组就地压缩，避免复制第二份百万行数组，反向索引在重建前释放。

## 接口调整

1. **文本访问**：`AlignInput::lines: Option<&LineList>` 借用文本，Smart 不创建整文件的逐行 String 列表；只有当前有界差异窗口建立字符/词特征。无文本时保留 Quick 骨架。文本/哈希/权重长度不一致会报错。
2. **手动对齐**：独立 `AlignmentOverrides` 保存有序非交叉锚点；`align2_with_overrides` / `align2_cancellable` 在锚点前后分别重算。外层会话须保留该结构并在 Realign 复用；`AlignMap::force_pair` 是一次性现有 map 重排，不代替会话锚点存储。`clear` 后恢复自动输出。
3. **取消**：`bc_core::cancel::CancelCheck` 和 `CoreError::Cancelled`；`bc_jobs::CancelToken` 实现该接口。新增的内部依赖方向严格为 **jobs → core**，没有 core → jobs。JobManager、线程池与调度器仍未实施；基准只用 std scoped threads。未来 worker 应发送 LineList 快照，而非 UI 线程的 Rc/RefCell 编辑目标。
4. **Smart 参数**：默认字符权重 0.7 / 词权重 0.3，阈值 0.5，skew 8，候选前 256 字符（可设 1–1024），128 行窗口；匹配相同正权重且已满足相似阈值时加权。`Tokenizer::line_weight` 将格式规则转成可直接传给 AlignInput 的 LineWeight。详见 [ADR 003](../decisions/003-smart-scoring.md)。
5. **格式来源**：`FileFormat::source_xml: Option<String>` 采用 serde default / None 时省略；手写 PartialEq 只比较语义字段。原始树的标记、属性节点顺序、显式默认值、未知子节点保留，修改仅补丁已变化字段。
6. **注册表**：`upsert` 现在返回 Result，验证语法/权重模式；`move_to` 管理优先级，`is_builtin` 区分用户覆盖。`save_user_dir/load_user_dir` 写入 `user-formats.xml` 和 `order.json`；用户根目录是 `Matcha/formats`，可传自定义 Path。原子性按单文件提供，两个文件不是一个整体事务；错误不会被静默丢弃。
7. **正则**：增加 `Pattern::literal`，并为可安全优化的 `match_at` 增加前缀快速否定；保留完整前文的锚点/词边界检查。不增加第三方依赖。

## 5.3 XML 差异与内置 JSON 体积

[ADR 001](../decisions/001-stx-write-rule.md) 已更新完整的新建 STX 规则、源树补丁策略和验证结论：

- 全部 25 格式读→写→读语义模型相等。
- Node 树（含顺序、Value、STX、未知子节点）相等。
- 逐行去掉缩进/行尾空白后原文件和输出完全一致；实际差异仅为原 CRLF → 写出 LF，没有实体拼写变化或默认属性增删。
- 另有无来源新建格式、修改已知字段时保留未知节点/异常 STX 的测试。
- 通用 Node 模型仍只建模 Value 属性；不声称保留任意额外 XML attribute。格式方言中的未知“属性子节点”会保留。

经协调者批准接受来源元数据带来的内置 JSON 增长：

| 阶段 | builtin_formats.json |
|---|---:|
| Wave 1 / 新增来源前 | 40,278 B |
| Wave 2 最终 | 148,926 B |
| 增量 | **108,648 B（约 106.1 KiB）** |

JSON 由 build.rs 生成而非额外纳入版本控制；体积约为原来的 3.70×。没有新引入第三方 crate，根 Cargo.toml 未改动；Cargo.lock 仅增加 jobs 对现有 core 的内部依赖关系。

## 规格澄清、剩余事项

- Tokenizer 最初的严格顺序与内置 PYTHON 三引号 / HTML `<` 对 `<!--` 等真实数据冲突。通过两次 ask，协调者最终批准统一“**最长匹配，等长按顺序**”；Columns 保留固定列优先级，并自行修改格式库 spec。完整依据见 [ADR 004](../decisions/004-tokenizer-precedence.md)，worker 没有改 specs/design。
- 这些分词/Smart 规则属于有测试的行为推断，**尚未做任务组 7 的参考工具黄金对照**，不冒称 Smart ≥95% 已达标。
- Smart 的有界窗口/长行截断及 Wave 1 Myers 大输入回退可能不同于全局最优；后续黄金语料需记录触发降级的样本与权重调参结果。
- 3 路 align3 与 Hex ByteAlign 保持局部 todo 豁免，符合本轮边界；已完成的 core/formats 文件没有 blanket unused_variables 豁免。
- 本轮批准范围内无未完成任务、无尚待回答的技术问题；任务组 6–9 的 20 项仍待后续 wave。
- 未访问 ObsidianVault，未修改 tmp 输入、内置格式 XML、其他 engine/gui crate；无授权/试用/注册逻辑或新增产品品牌逻辑。

## 提交

| 范围 | commit |
|---|---|
| 任务组 4 | `eb254dbd9e101455d0b6f387f94a819d065c052f` |
| 任务组 5 | `47ff276126f70c7d337a9df96d33f333a458a68c` |
| 4.4 最终取消点/内存验收补充 | `413c0369438eb675e09c4d751eb8569a0332abf9` |

均带 `Co-Authored-By: pi <noreply@pi.dev>`；本报告另作 docs 提交。所有权外没有修改其他 crate：bc-jobs 仅改 CancelToken 桥接、其内部依赖/README、相关测试与基准。协调者的 spec 澄清未纳入上述提交。

## 最终命令输出（最后 10 行；不足 10 行则全部列出）

### cargo clippy --workspace --all-targets -- -D warnings

退出码 0、零 warning；此处为最终提交状态的增量检查输出。

```text
    Checking bc-formats v0.1.0 (D:\Github\Matcha\crates\bc-formats)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.55s
```

### cargo test --workspace

退出码 0，共 73 个测试通过；尾部为无 doctest 的 crate。完整本地日志：`target/wave2-test.log`。

```text
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests bcomp

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```

### cargo build --release -p bcompare

退出码 0。

```text
    Finished `release` profile [optimized + debuginfo] target(s) in 0.26s
```

### 最终基准尾部

```text
bench.quick.seconds=0.093124
bench.quick.rows=2141334
bench.quick.private_growth_bytes=388431872
bench.quick.private_input_ratio=1.7656
bench.quick.cancel_ms=0.089
bench.smart.seconds=0.249894
bench.smart.rows=2141334
bench.smart.private_growth_bytes=389394432
bench.smart.private_input_ratio=1.7700
bench.smart.cancel_ms=0.060
```
