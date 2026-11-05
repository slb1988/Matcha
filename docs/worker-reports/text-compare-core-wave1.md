# text-compare-core — Wave 1 实施报告

## 结果

任务组 **1、2、3 共 12 项全部完成**；`tasks.md` 的 1.1–1.3、2.1–2.6、3.1–3.3 均为 `[x]`，OpenSpec apply 进度为 **12/40**。任务组 4–9 的 28 项保持未完成，本轮没有开始其算法/GUI/集成实现。

最终验证：`cargo clippy --workspace --all-targets` 零警告，额外 `-- -D warnings` 通过；`cargo test --workspace` **37 个测试通过、0 失败**（bc-core 35、bc-formats 2；含 1,000 轮随机 LCS 对照和 3,969 对短二元序列穷举）；`cargo build --release -p bcompare` 通过。所有提交均包含 `Co-Authored-By: pi <noreply@pi.dev>`。

## 完成清单与证据

| 任务 | 实现/验证 |
|---|---|
| 1.1 | workspace 声明并由 bc-core 引用 regex、xxhash-rust/xxh64、encoding_rs；`cargo tree -p bc-core -e normal` 无 GUI/异步运行时，`cargo check --workspace` 通过 |
| 1.2 | 三个 crate 的 `tests/baseline.rs` 空集成测试入口及 `examples/{core,formats,text}_baseline.rs`；对应 cargo test 和 cargo run --example 实际运行通过 |
| 1.3 | [STX](../decisions/001-stx-write-rule.md)、[未知多行语法项](../decisions/002-lines-grammar.md)、[Smart 评分](../decisions/003-smart-scoring.md) 决策占位及 crate README 链接；实施时检查 13 个链接有效，交付时再次检查全部 29 个相关链接有效 |
| 2.1 | [line.rs 测试](../../crates/bc-core/tests/line.rs)：CRLF/LF/CR/混合/空文件/仅一行无换行逐字节往返、主导风格、预览限制、强制行尾与写入错误 |
| 2.2 | [line_edit.rs](../../crates/bc-core/tests/line_edit.rs)：insert/delete/replace/单行及全文件行尾修改，1,000 次确定性随机操作逐步 undo/redo，快照独立且未修改字节共享 |
| 2.3 | [edit.rs](../../crates/bc-core/tests/edit.rs)：合并、嵌套分组、逆序 undo/顺序 redo、保存边界、不同行为分支、保存点被逐出、限制下降和 clear |
| 2.4 | [encoding.rs](../../crates/bc-core/tests/encoding.rs)：UTF-8/16LE/16BE、GBK/1252、全部映射页 ASCII 往返、BOM 策略、二进制阈值、非法序列计数和不可表示输出错误 |
| 2.5 | [hash.rs](../../crates/bc-core/tests/hash.rs)：独立空白开关、Unicode 小写、无效 UTF-8 字节不混淆、批量 xxh64 与已知空输入向量、IEEE CRC32 已知向量/分块更新 |
| 2.6 | [mmap.rs](../../crates/bc-core/tests/mmap.rs)：Windows 映射/锁生命周期、共享冲突降级、快照/COW、空/不存在文件；[line_bench.rs](../../crates/bc-core/examples/line_bench.rs) 的 110 MB 实测见下 |
| 3.1 | [pattern.rs](../../crates/bc-core/tests/pattern.rs)：方向词边界、转义/字符类、大小写、byte offset、captures、替换、非法语法与掩码表；[builtin_patterns.rs](../../crates/bc-formats/tests/builtin_patterns.rs) 遍历全部 25 格式并编译 **74** 个正则字段 |
| 3.2 | [diff.rs](../../crates/bc-core/tests/diff.rs)：空/相同/全不同/交错、1,000 轮随机与朴素 LCS 等价、短序列穷举、公共边裁剪、100,000 项分块及移位输入 |
| 3.3 | 同一 diff 测试：经典移动唯一块、唯一锚点优先于重复行 LCS、子区间重新计数唯一性、无锚点 Myers 回退、空输入 |

## 2.6 基准

环境：Windows x86_64 MSVC，`rustc 1.99.1 (58cf8f9c3 2026-10-13)`，`cargo 1.99.1`，release（optimized + debuginfo）。输入只读：`tmp/left.txt`，**110,000,043 字节 / 2,119,483 行**，实际后端为 `Mapped`。

命令（以仓库根为当前目录）：

```powershell
cargo run --release -p bc-core --example line_bench -- tmp/left.txt
```

| 运行 | 打开 + 行索引耗时 | 编辑前进程私有内存 | 编辑后进程私有内存 | 增量 |
|---|---:|---:|---:|---:|
| 首次实施验证 | 98.072 ms | 83,640,320 B | 83,640,320 B | 0 B / 0% |
| 最终复测 1 | 96.955 ms | 83,595,264 B | 83,595,264 B | 0 B / 0% |
| 最终复测 2 | 97.192 ms | 83,595,264 B | 83,595,264 B | 0 B / 0% |
| 最终复测 3 | 96.142 ms | 83,595,264 B | 83,595,264 B | 0 B / 0% |

均满足 **打开 <200 ms、编辑一行增量 <输入 10%（11,000,004 B）**。计时包含打开、映射与全量行索引，不包含 PowerShell 采样；缓存未强制清空，因此不是冷盘性能承诺。内存以同一进程 `Get-Process.PrivateMemorySize64` 采样；编辑保留了 before/after 撤销快照，并执行 undo/redo。0 B 是 OS 私有提交页粒度下的增量，不表示零堆分配；修改行及少量 piece 描述符复用了已有堆页，未复制整文件。测试同时以未改行指针不变确认底层共享。

## 新增依赖

仅新增以下三个 workspace 第三方依赖，并在 bc-core 引用：

- `regex = "1.12"`（锁定 1.13.1）。
- `xxhash-rust = { version = "0.8", features = ["xxh64"] }`（锁定 0.8.19）。
- `encoding_rs = "0.8"`（锁定 0.8.44）。

mmap 使用仓库原有的 `memmap2 = "0.9"`（锁定 0.9.12），仅增加 bc-core 的 workspace 引用；没有新增锁库、GUI、异步运行时或基准库。bc-core 的 normal tree 只含上述库、quick-xml 及其纯计算/系统基础传递依赖。

## 接口调整与协调者确认

1. **BOM 信息**：采用批准的 `encode(text, encoding, policy, original_has_bom)` 四参数签名；Preserve 使用 `Detection.has_bom`。新增 `decode_with_errors -> Result<(String, usize), CoreError>`；`decode` 保留 String 包装。占位符计数不把原文合法 U+FFFD 算为错误；不能表示的保存字符报错，而非静默替换。
2. **mmap 安全**：按批准的 [mmap 决策](../decisions/002-mmap-backend.md)，Windows FILE_SHARE_READ 打开并持有句柄到最后一份映射快照释放；共享冲突错误 32 降级 owned，非 Windows 一律 owned。实际后端由 `LineList::backend()` 提供，避免把存储类型混进编码 Detection。映射期间正常写/删/截断会被拒绝，原路径保存前需释放映射/历史引用或另存。bc-core 为 `deny(unsafe_code)`，唯一 unsafe 块是有 SAFETY 注释的 `Mmap::map`。
3. **行编辑**：新增 `SharedLines`、已经应用的 `LineEdit`、便宜的 LineList 快照和 `insert_default/set_all_endings`。未修改 mmap 字节与行索引保留共享，首次编辑只增加分段描述及修改行。直接更改目标时不能保留不一致的旧撤销历史。
4. **掩码边界（已通过两轮 ask 明确允许据此完成 3.1）**：`glob_to_pattern` 只接受一个正向掩码；`split_mask_list` 纯解析为 `MaskSpec { pattern, exclude, folder }`，剥离前导负号及目录尾标记。整体匹配串 `*.c;*.h;-*.bak;obj\` 先拆四项再验证逐项匹配表；分号列表/负项直接传入 glob 时明确报错。正负项组合求值、负项独占时的全选、目录后代递归归后续 `bc-engine-dir::NameFilter`，本轮不实现；目录原语仅匹配显式尾斜杠名称。
5. **任务 1.2 额外范围**：经协调者授权，bc-engine-text 只新增 `tests/baseline.rs`、`examples/text_baseline.rs` 并更新 README 的基准入口与 Smart 决策链接；没有触碰该引擎算法。

## 未完成项及规格/设计建议

- 本轮批准范围内无未完成项、无尚待回复的问题。任务组 4–9 不在本次范围，仍有 28 项。
- Myers 默认对裁剪后超过 16,384 项的输入采用内容定义块锚点与最多 1,024 项窗口，保序/完整覆盖不变，但不保证全局最短脚本；`block_fallback_threshold = 0` 可强制精确 LCS。建议后续 Quick/黄金语料测试明确记录是否触发降级。
- 二进制判定阈值在函数文档固定为前 8 KiB NUL >1% 或不可打印 ASCII 控制字节 >10%，BOM 优先。规格未给阈值数值，后续如要用户可配可扩展参数，不应隐式改变已有编码检测结果。
- mmap 的 Windows 保存前释放引用约束必须在后续引擎/GUI 保存任务中处理；非 Windows owned 后端已实现编译分支，但本机仅执行 Windows 验收，未声称完成非 Windows 性能测试。
- STX 新建节点写规则、多行未知语法类、Smart 精确评分仍是已有待研究事项，只建立决策占位；没有修改 specs/design/proposal。
- 已实现的 bc-core 模块不再受 crate 顶部 unused_variables 豁免；剩余豁免只局部标记 align 和 persist writer 的原有 todo 入口，未实施任务组 4/5。
- 未读取/修改 ObsidianVault，未改动 tmp 输入；未新增授权、试用、注册或参考产品品牌逻辑。

## 提交记录

| 阶段 | commit |
|---|---|
| 首次脚手架基线 | `196772fb885eaf4fe60520ae35273d5115d5e34a` |
| 任务组 1 | `b1d1169d8833308496b46ae8b6494f33b5e2007e` |
| 任务组 2 | `368a0775bd81e990b7262bcc3d09079368927f34` |
| 任务组 3 | `86a8b017b6d30f7d91024294798cc534ed6d7bea` |

本报告另作 docs 提交。除授权的 bc-engine-text 三个文件外，没有修改其他 crate 的实现；根 Cargo.toml/Cargo.lock、bc-core、bc-formats 的 README/测试、docs/decisions 和 tasks 复选框属于本轮修改。

## 验证输出（各命令最后 10 行）

### cargo clippy --workspace --all-targets

退出码 0，未出现 warning。

```text
    Checking bc-engine-bin v0.1.0 (D:\Github\Matcha\crates\bc-engine-bin)
    Checking bc-engine-table v0.1.0 (D:\Github\Matcha\crates\bc-engine-table)
    Checking bc-engine-dir v0.1.0 (D:\Github\Matcha\crates\bc-engine-dir)
    Checking bc-session v0.1.0 (D:\Github\Matcha\crates\bc-session)
    Checking bc-report v0.1.0 (D:\Github\Matcha\crates\bc-report)
    Checking bc-gui v0.1.0 (D:\Github\Matcha\crates\bc-gui)
    Checking bc-script v0.1.0 (D:\Github\Matcha\crates\bc-script)
    Checking bcomp v0.1.0 (D:\Github\Matcha\apps\bcomp)
    Checking bcompare v0.1.0 (D:\Github\Matcha\apps\bcompare)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.59s
```

### cargo test --workspace

退出码 0，全部 37 个测试通过；末尾为无 doctest 用例的 crate，完整日志为本地构建产物 `target/wave1-test.log`。

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
   Compiling bc-engine-text v0.1.0 (D:\Github\Matcha\crates\bc-engine-text)
   Compiling bc-engine-bin v0.1.0 (D:\Github\Matcha\crates\bc-engine-bin)
   Compiling bc-engine-table v0.1.0 (D:\Github\Matcha\crates\bc-engine-table)
   Compiling bc-engine-dir v0.1.0 (D:\Github\Matcha\crates\bc-engine-dir)
   Compiling bc-session v0.1.0 (D:\Github\Matcha\crates\bc-session)
   Compiling bc-report v0.1.0 (D:\Github\Matcha\crates\bc-report)
   Compiling bc-gui v0.1.0 (D:\Github\Matcha\crates\bc-gui)
   Compiling bc-script v0.1.0 (D:\Github\Matcha\crates\bc-script)
   Compiling bcompare v0.1.0 (D:\Github\Matcha\apps\bcompare)
    Finished `release` profile [optimized + debuginfo] target(s) in 5.78s
```

额外 `cargo clippy --workspace --all-targets -- -D warnings` 同样退出码 0；`git diff --check` 通过。过程中的例子同名产物冲突已通过使用 crate 专属 example 名修复，无遗留失败。
