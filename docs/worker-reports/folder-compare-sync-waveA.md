# folder-compare-sync — Wave A 实施报告

## 结果与范围

任务组 **1–3 共 16 项完成**，`tasks.md` 的 1.1–1.5、2.1–2.7、3.1–3.4
全部为 [x]，OpenSpec apply 为 **16/34**。任务组 4 及以后未开始，剩余 18 项。

最终验证：
- `cargo clippy --workspace --all-targets -- -D warnings`：退出码 0，零 warning。
- `cargo test --workspace`：**101 passed / 0 failed / 1 ignored**。
- ignored 的回收站测试已另行手动执行并通过（不重复计入 101）。
- `cargo build --release -p bcompare`：退出码 0。
- `git diff --check` 通过。

工作仅在独立 worktree/分支执行；未进入主工作树或 vault，未读取/复制任何授权文件，
未写安装目录。只从指定 CHM 解出与本轮相关的帮助页到系统临时目录作行为核对。
没有修改 bc-gui、bc-engine-text、bc-session，也没有实现操作/同步算法；
ops.rs、sync.rs 的改动仅为 rustfmt 格式变化。

## 完成清单与证据

| 项 | 实现与实际验证 |
|---|---|
| 1.1 | VfsPath 保留 bare/显式 URL 显示拼写，解析返回 Result；[local.rs 测试](../../crates/bc-vfs/tests/local.rs) 覆盖盘符、UNC、相对、file:///、远程 scheme、非法空值/控制字符，以及 join/parent/file_name |
| 1.2 | std 枚举/元数据/读流；DOS 属性与 reparse 判断；真实临时 junction 测试不跟随/开启跟随，以及移除链接不动目标；icacls 临时拒绝子目录 RD 后验证其 error 字段与可读兄弟共存，测试随后撤销拒绝 ACL |
| 1.3 | 流式 64 KiB 复制、同目录临时文件/原子发布、显式 overwrite、目录树、rename 与跨卷错误分支的复制+删除；测试时间戳 100 ns 往返、属性/只读保留、put、rename、多级目录、复制取消清理临时文件且不改旧目标、拒绝复制到源内部 |
| 1.4 | 递归永久删除、流式 IEEE CRC32、Windows overlapped ReadDirectoryChangesW；测试标准 123456789 → cbf43926、取消、创建/重命名/删除事件及 watcher drop 不挂起；回收站测试见下 |
| 1.5 | [list_bench.rs](../../crates/bc-engine-dir/examples/list_bench.rs) 生成两侧各 100,000 文件，实际枚举并写入 DirTree 快速状态；顶层阈值与 100,000 相同文件计数断言通过，数字见下 |
| 2.1 | arena NodeId + 每文件夹子项表，大小写/NFC/去扩展名键、listing 替换与删除、脏祖先摘要、红紫同时存在、空心/黄色标记、展开折叠、可见缓存；[tree_criteria.rs](../../crates/bc-engine-dir/tests/tree_criteria.rs) 覆盖配对/孤儿/汇总/缓存/刷新后大小写改名/已删子树 |
| 2.2 | AlignmentRule 通配符捕获对应、正则捕获、相对文件夹限定与 Align With；测试 x1/x2 ↔ y1/y2、正则 left-42 ↔ right-42、重新扫描保留覆盖；重对齐退役旧 NodeId，晚到结果无效 |
| 2.3 | 独立大小、时间戳容差、精确 DST 1h、整小时倍数时区忽略、左右时区偏移、文件名大小写、所选属性；测试默认忽略 archive、容差边界、左右较新与版本/内容覆盖 |
| 2.4 | ContentCompareJob + typed ContentResult，CRC/二进制分块、短读正确、早停、quick-same 零读跳过、覆盖与 revision 写回；[content.rs 测试](../../crates/bc-engine-dir/tests/content.rs) 的 1 GiB 稀疏文件/真实线程取消数字见下 |
| 2.5 | **按分支约定保留 RulesBased → VfsError::Unsupported**；明确占位测试已通过，不冒称已接通文本引擎；合并后补接 |
| 2.6 | bc-engine-bin 的 ver 模块以安全 Rust 读取 PE32/PE32+ 资源目录及 VS_VERSION_INFO/StringFileInfo，不加载/执行 DLL；malformed table 测试和本机 kernel32.dll 版本字符串测试通过 |
| 2.7 | Criteria::parse_script / to_script 及 GUI 独立开关扩展字段；要求的完整 criteria 示例往返与非法字段/互斥项验证通过 |
| 3.1 | split_mask_list 只负责拆项，完整字符集/否定集/[[]/尾句点/*.*、三种路径前缀与四组组合在 dir::filter；[filters.rs](../../crates/bc-engine-dir/tests/filters.rs) 覆盖 file_masks 全部示例及额外交集/排除优先场景 |
| 3.2 | 日期绝对/相对天数、大小、DOS 属性、默认受保护项排除、流式 UTF-8 文本包含/不包含；测试本地零点前 7 天严格边界、跨 64 KiB/UTF-8 字符边界匹配及单侧 teal |
| 3.3 | 13 显示过滤 × 8 状态真值表（另测 Error）、四种文件夹显示、纯视图行、Flatten 重名配对、Suppress Filters/Ignore Unimportant；测试树比较状态不被视图更改 |
| 3.4 | FilterSettings::apply_script 事务式局部更新；帮助六例加批准的 size:>1MB 第七例；FilterPresets 用 bc-core XML 往返，验证命名、替换、转义与非法模型 |

额外 LoadDirJob 已提供递归 listing 的 typed payload 和取消/进度入口，使用 std 工作线程接口，
没有实施 bc-jobs::JobManager/调度器，也未声称已完成 GUI 后台调度。

## 1.5：两侧各 10 万文件基准

环境：Windows x86_64 MSVC，Rust/Cargo 1.99.1；release optimized + debuginfo。
每侧 100 个目录，每目录 1,000 个空文件，共 **200,000 个文件**；
全部在系统临时目录生成，退出自动清理，不占用仓库 fixtures/tmp。
测试未清空 OS 缓存，不能作为冷盘性能承诺。

```powershell
cargo run --release -p bc-engine-dir --example list_bench
```

| 测量 | 初测 | 最终复测 |
|---|---:|---:|
| 数据生成（不计入比较耗时） | 44.894979 s | 36.042571 s |
| 顶层列举 + 配对/快速状态 + 100 行可见 | 0.024071 s | **0.015446 s** |
| 两侧 200,000 文件枚举/元数据 | 7.631665 s | **6.583755 s** |
| 全树配对/快速状态/脏摘要 | 0.171492 s | **0.181416 s** |
| 顶层到全树完成合计 | 7.827488 s | **6.780862 s** |
| 展开 100,100 行的基本行缓存派生 | 0.001773 s | **0.003141 s** |
| 完整文件夹过滤视图派生 | — | **0.031733 s** |
| 完整视图缓存命中（含返回 Vec 拷贝） | — | **0.002824 s** |

顶层 <1 s 的断言两次均通过。内容比较不纳入快速状态计时，未运行 GUI 滚动性能测试。
完整本地日志：`target/folder-list-bench-final.log`。

## 2.4：读取量与取消

```powershell
cargo test --release -p bc-engine-dir --test content -- --nocapture
```

- 两个实际逻辑大小 **1,073,741,824 B** 的临时文件，Windows fsutil 只对这两个文件设置 sparse。
- 首字节不同（二进制）：左 **65,536 B**、右 **65,536 B**，总计 **131,072 B < 1 MiB**；
  第一个不同块后立即退出，没有扫描剩余空间。
- CRC 取消：worker 首次进入取消检查点通知主线程，继续运行 10 ms 后取消，
  从请求到 join 返回 Cancelled 最终 **0.137 ms**（debug 初测 0.967 ms），均远低于 1 s。
- quick-same skip 用不存在的路径验证仍返回 skipped 且 bytes_read=[0,0]，确保没有偷偷打开内容。
- revision 测试覆盖刷新、重对齐、较新结果覆盖旧结果及摘要红色清除。
- 本地日志：`target/folder-content-final.log`。

## 回收站与版本实测

手动命令：

```powershell
cargo test -p bc-vfs recycle_bin_manual -- --ignored --nocapture
cargo test -p bc-engine-bin ver:: -- --nocapture
```

回收站测试执行两次均通过；不仅断言源文件消失，还通过 Shell.Application 的
Recycle Bin 命名空间查询到该测试文件，最终得到 `$RM9LKAA.txt` 对应回收项。
没有清空回收站，也没有读取回收站中的其他文件内容。

版本测试读取系统 kernel32.dll 得到：
`FileVersion=10.0.26100.3912 (WinBuild.160101.0800)`。
该测试只读资源，不执行 DLL、不调用版本 DLL 动态加载入口。

## 新增依赖与特性

唯一新增的第三方直接依赖：workspace `windows-sys = "0.61"`，锁定现有 0.61.2，
**仅 bc-vfs 的 cfg(windows) 引用**。Cargo.lock 未增加第三方包或更新包版本；
只增加 bc-vfs → bc-core/windows-sys 和 bc-engine-dir → bc-engine-bin 的内部边。

原三个特性经 coordinator ask 明确放宽为五个：

1. Win32_Foundation
2. Win32_Storage_FileSystem
3. Win32_UI_Shell
4. **Win32_System_IO**：ReadDirectoryChangesW / GetOverlappedResult / CancelIoEx
5. **Win32_Globalization**：NormalizeString

理由：使用生成绑定而非手写 FFI；unsafe 只在 bc-vfs/windows.rs 的窄边界，附生命周期说明。
没有为引擎添加 GUI、异步运行时、Unicode/日期/文件监视/回收站库。
`cargo tree -p bc-engine-dir --edges normal` 已核对无 GUI/异步依赖。

## 接口调整（供后续 merge/worker）

1. **bc-core：零修改。** 原样复用 CancelCheck/NeverCancel、Crc32、Pattern、split_mask_list、persist。
2. **bc-jobs：零修改。** 原样使用 CancelToken 的 CancelCheck 桥接、JobContext/Progress/JobResult。
   没有修改线程池、JobManager，也没有改变 text AlignmentOverrides。
3. **VfsPath**：parse 从占位 Self 返回改为 VfsResult<Self>；保存显式 scheme 显示形式。
   VfsManager 按不区分大小写解析 scheme；现有同步 Vfs trait 方法签名未改。
4. **VfsEntry**：新增 `error: Option<String>`，保留无权限子项；构造者需补字段。
5. **LocalFs**：新增 CopyOptions、get_cancellable/put_cancellable/rename_cancellable/
   crc32_cancellable；取消参数为 &dyn CancelCheck，返回 VfsError::Cancelled。
   overwrite 默认 false；无覆盖原子发布不覆盖竞态创建的目标。
6. **DirTree**：递归 owned children 改为 arena NodeId/子项表；通过 root()/node()/nodes()/find()
   只读访问。apply_listing 返回 VfsResult；配置变化由 set_pairing/add_alignment/align_with 触发重对齐。
   原列表/准则变化使 revision 失效；重对齐不重用退役 NodeId。
7. **Criteria**：新增 compare_size/compare_filename_case/skip_content_if_quick_same/
   content_overrides_quick；GUI 扩展字段显式脚本拼写确保往返不丢失；快速状态与内容状态分开存储。
8. **ContentCompareJob**：携带 Criteria、quick_status、NodeId、revision；返回 ContentResult
   （status/bytes_read/skipped/versions_equal），调用 apply 校验修订号。RulesBased 仍明确 Unsupported。
9. **过滤与视图**：NameFilter::parse 返回 Result、编译一次；OtherFilters 增加相对日期与内容项，
   默认 exclude_protected=true。FilterClock 由调用方提供本地零点/UTC 偏移；
   apply_filters 接受已完成内容判定，不在 UI 路径隐式读文件。
   view_rows(ViewOptions) 返回每侧源 NodeId、teal 与有效状态；Flatten 不修改原树。
10. **PE**：只实现 bc-engine-bin/src/lib.rs 的 ver 模块；其他非文本引擎仍为原占位。
    解析畸形资源返回 InvalidData；非 PE/无版本资源返回空列表。

## 已确认的歧义、限制与后续建议

- [ADR 005](../decisions/005-folder-pairing-and-filters.md)：包含文件×包含文件夹取交集、
  显式 ...\\ 越过包含限制但不越过排除、本地时间上下文、七个脚本例的来源。
  Flatten 重名按相对路径排序一一配对已批准，**待任务组 7 黄金语料核实**。
- [ADR 006](../decisions/006-recycle-policy.md)：固定盘添加 FOF_WANTNUKEWARNING，
  无法回收时可能弹系统永久删除确认；取消返回 Unsupported。
  **任务 4.2 的永久删除回退必须再次确认**，不得把取消等同许可。
  未修改用户回收站策略进行破坏性测试；未来可改为 IFileOperation 的严格无 UI 后端。
- 非 Windows 开启 NFC 明确 Unsupported（已批准）；其余 Unix 分支有源码实现，
  但本机只安装 Windows Rust target，**未声称完成 Linux/macOS 编译或运行验收**。
- 规则比较的占位是本轮明确授权例外；合并 text-compare-core 后仍需接入 Importance 统计。
- 日期解析覆盖 ISO、US 和英文月份示例；地区/历史 DST 偏移由上层提供。
  文本内容过滤目前为流式 UTF-8，非法编码显式报错，不做有损隐藏。
- 跨卷移动的 EXDEV/ERROR_NOT_SAME_DEVICE 分支已实现，但本机验收主要覆盖同卷 rename；
  不冒称已经做真实跨卷/UNC 共享的写操作。
- Flatten 新虚拟配对仅可复用其自身的快速结果，不能借用原树不同配对的内容结果；
  后续 GUI 内容任务须绑定虚拟配对身份。
- 已完成模块移除 blanket unused_variables；剩余豁免只限未实施的操作/同步模块。
- 工作区另有 **协调者修改、未由 worker 暂存/提交** 的 proposal.md/design.md
  （五特性授权更新）；交付保留给协调者处理。

## 提交

所有提交均有 `Co-Authored-By: pi <noreply@pi.dev>`。

| 阶段 | commit |
|---|---|
| 必须先做的 OpenSpec 原稿提交 | `e921f58f49c703a5c60d1b698f6812f8f3f8cfe5` |
| 任务组 1 主实现 | `bace4bcea5c41bb40198ecae682a746a012f1c96` |
| 任务 1.5 基准 | `b0aad42881cf9aea5dd73b74135039e856c1ea11` |
| 任务组 2 | `0dadaead5fd59352e54a8520b29e05a4e1f06c6a` |
| 任务组 3、回收站安全策略补充 | `ee20804c698cb338761f578b73b85147322e8762` |
| 最终取消优先级/缓存视图性能补充 | `4db9cdea6cbd2d54968a7d042f5991278c98588c` |

本报告另作 docs 提交，任务组 4–8 不作完成承诺。

## 最终验证输出（最后 10 行）

### cargo clippy --workspace --all-targets -- -D warnings

退出码 0；完整本地日志 `target/folder-waveA-clippy-final.log`。

```text
    Checking bc-engine-text v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-engine-text)
    Checking bc-engine-bin v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-engine-bin)
    Checking bc-engine-dir v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-engine-dir)
    Checking bc-session v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-session)
    Checking bc-report v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-report)
    Checking bc-gui v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-gui)
    Checking bc-script v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-script)
    Checking bcomp v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\apps\bcomp)
    Checking bcompare v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\apps\bcompare)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.96s
```

### cargo test --workspace

退出码 0；101 passed / 0 failed / 1 ignored；末尾为 doctest 汇总，
完整本地日志 `target/folder-waveA-test-final.log`。

```text

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests bcomp

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### cargo build --release -p bcompare

退出码 0；完整本地日志 `target/folder-waveA-release-final.log`。

```text
   Compiling bc-vfs v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-vfs)
   Compiling bc-engine-bin v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-engine-bin)
   Compiling bc-engine-text v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-engine-text)
   Compiling bc-engine-dir v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-engine-dir)
   Compiling bc-session v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-session)
   Compiling bc-report v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-report)
   Compiling bc-gui v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-gui)
   Compiling bc-script v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\crates\bc-script)
   Compiling bcompare v0.1.0 (C:\Users\admin\orca\workspaces\Matcha\folder-compare-sync\apps\bcompare)
    Finished `release` profile [optimized + debuginfo] target(s) in 3.53s
```
