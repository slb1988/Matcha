# Design

## Context

动机见 `proposal.md - Why`。当前状态：

- `bc-engine-dir` 有完整的类型骨架（`DirTree`/`DirNode`/`CompareStatus`、`Criteria`、`NameFilter`/`OtherFilters`/`DisplayFilter`、`DirRules`、`SyncPlan`/`SyncAction`、`DirOp`、四个 job），全部 `todo!()`。
- `bc-vfs` 的 `Vfs` trait 与 `VfsManager` 已定，`LocalFs` 全为 `todo!()`；`VfsPath::parse/join/parent/file_name` 为 `todo!()`。
- `bc-core`（wave 1–2）已可用：`Crc32`、`pattern::{Pattern, glob_to_pattern, split_mask_list}`（单项掩码原语，组合求值明确留给本 change 的 `NameFilter`）、`cancel::CancelCheck`、`persist`。`bc-jobs::CancelToken` 实现了 `CancelCheck`；`JobManager` 仍是骨架。
- `bc-gui` 只有文本原型与 Home；`text-compare-core` 的 wave 3/4 会在 `master` 上改 `app.rs`、`home.rs`、新增文本控件。
- 本 change 在独立 git worktree/分支开发，与 `text-compare-core` 并行；合并时的冲突面要尽量小。
- 参考资料：帮助主题 `commandsdir`、`commandsdirsync`、`sessiondir*`、`sessiondirsync*`、`dir_*`、`file_masks`、`dlgdir*`（本 design 已按 D10 全部扫过，见"未采纳项"）。

## Goals / Non-Goals

**Goals:**

- 本地两目录的比较/过滤/操作/同步可日常使用，10 万文件秒级出树，内容比较后台化可取消。
- 内部命令 API（load/criteria/filter/select/op/sync）直接可被后续脚本引擎调用，行为与 GUI 一致。
- 黄金语料用参考工具脚本全自动生成（无需 GUI），覆盖准则与过滤。

**Non-Goals:**

- Folder Merge（3 路目录）、远程/压缩档/快照提供者、SCC 子菜单、Open With 外部程序注册表、会话持久化与 CLI 解析（`session-and-cli`）。
- 文本规则比较本身（调用 `bc-engine-text` 的公开 API）。

## Decisions

### D1. 树模型：按相对路径键的分层表 + 按需派生行

- 选择：`DirTree` 内部以每个文件夹一个"子项表"（名称键 → `DirNode` 索引）存储，视图行按展开状态与过滤按需扁平化为 `Vec<RowRef>` 缓存，失效于展开/过滤/状态变化。名称键在 Windows 上为大小写折叠（可按准则改为精确），并带可选的 Unicode NFC 规范化与"去扩展名键"以支持两个对齐开关。
- 理由：10 万文件下重建整棵扁平行表是 O(n)，但只在用户操作时发生；状态汇总用脏标记自底向上传播，避免全树重算。
- 备选：直接 `Vec<DirNode>` 树递归渲染。深树递归与过滤组合会反复遍历，放弃。

### D2. 准则管线分两级：目录级快速测试同步算，内容测试走 job

- 选择：枚举完成即算快速状态（大小/时间戳/属性/大小写）；内容测试（CRC/二进制/规则）按"快速测试相同则跳过"与"覆盖"规则由 `ContentCompareJob` 批量执行，结果通过修订号校验写回节点；二进制比较分块读两侧并在首个不同块提前退出；CRC 走 `bc_core::hash::Crc32` 流式。
- 规则比较：调用 `bc_engine_text` 的"按格式比较两文件并返回 Importance 统计"的公开入口；在 `master` 合并前该入口不存在，用 `ContentCompare::RulesBased => Err(Unsupported)` 占位并在合并后接通（任务 2.5）。
- 时间戳：容差、DST（恰好 ±1 h）、时区（整小时倍数）三条规则独立实现并有单测；文件系统精度（FAT 2 s）由默认容差 2 s 覆盖。

### D3. 掩码：完整语法在 bc-engine-dir 实现，复用 bc-core 原语

- 选择：`NameFilter::parse` 用 `bc_core::pattern::split_mask_list` 拆项，再自行解析 `[set]`、`[!set]`、`[[]`、尾部 `.`、相对前缀（`p\`、`.\`、`...\`）生成匹配器；组合求值：排除优先，仅排除时隐式全包含，包含文件的祖先与基准始终包含。匹配对象是相对路径（分隔符统一为 `/` 内部表示，显示仍用系统分隔符）。
- 理由：wave 1 已明确 bc-core 只做单项原语；组合语义属于文件夹会话。
- 备选：把完整语法下沉到 bc-core。文本会话用不到，放弃。

### D4. 显示过滤与文件夹显示过滤作为纯函数

- 选择：`DisplayFilter::shows(status)` 与 `FolderDisplay::{AlwaysShow, FilesAndStructure, FilesOnly, Flatten}` 都是对节点状态的纯判定，扁平化模式生成以文件名为键的虚拟树。Suppress Filters 是一个视图级布尔，只改变"被过滤"节点的可见性与 teal 着色，不改变树。

### D5. 文件操作：先规划后执行，一律 job

- 选择：每个操作先生成 `OpPlan`（逐项源/目标/类型/需要的确认），GUI 或脚本用 `Interaction` 回答确认（Yes/No/All），再由 `FileOpJob` 执行；复制用"临时名 + 原子改名"保证可取消；执行结果以事件流写入 `OpLog`，并产出"受影响节点列表"供局部刷新。
- 理由：规划/执行分离使脚本的 `option confirm:` 与 GUI 对话框共用同一机制，也让同步预览直接复用 `OpPlan`。

### D6. 同步计划 = 状态 × 方法 → 动作，覆盖存于计划

- 选择：`SyncPlan::build` 把每个条目的状态（左新/右新/左孤儿/右孤儿/不同/相同/冲突）按方法映射为动作；用户覆盖写入计划项；Update Both 下双侧均变化的条目标为冲突并默认 Leave Alone；执行时把计划翻译成 `OpPlan` 交给 D5。

### D7. LocalFs：std 为主，Windows 特性用 `windows-sys` 收窄

- 选择：枚举/复制/重命名/时间戳用 `std::fs`；DOS 属性、回收站（`SHFileOperationW` 或 `IFileOperation`）、reparse point 判定用 `windows-sys`（仅五个特性：Foundation、Storage_FileSystem、UI_Shell、System_IO、Globalization），全部 `cfg(windows)`；非 Windows 回收站返回 `Unsupported`。目录监视用 `ReadDirectoryChangesW`（Windows）/轮询对比（其他）。
- 理由：避免更重的 `trash`/`notify` 依赖，保持依赖面可审计。

### D8. GUI：新增模块，不改文本控件

- 选择：`bc-gui` 新增 `dir_view.rs`（虚拟化树表：行高固定，仅渲染可见行，列可排序）、`sync_view.rs`、`legend.rs`、`log_panel.rs`、`filters_bar.rs`；`app.rs` 只新增一个 `View::FolderCompare` / `View::FolderSync` 分支与 Home 入口按钮。合并到 `master` 时冲突面限于 `app.rs`/`home.rs` 的几行。
- 颜色：灰/黑/紫/红/teal/黄 来自 `bc_session::prefs` 新增的 `FolderColors`（默认值取参考工具）。

### D9. 黄金语料：脚本全自动

- 选择：`tools/golden/gen_dir_expected.ps1` 生成测试目录树（含时间戳、属性、内容差异），用参考工具脚本 `load <left> <right>`、`criteria …`、`filter …`、`expand all`、`folder-report layout:xml output-to:<file>` 生成期望；测试解析 XML 比对每行状态。无需 GUI，不受桌面锁定影响。

### D10. 参考工具行为对照清单（本 change 扫描结果）

扫描：`commandsdir.html`、`commandsdirsync.html`、`sessiondir*.html`（specs/comparison/handling/namefilter/otherfilters/misc）、`sessiondirsync*.html`、`dir_understanding_the_display`、`dir_filtering_the_view`、`dir_reconciling_differences`、`dir_sync_*`、`file_masks`、`dlgdiralignwithitem`、`dlgdirotherfilteritem`、`dlgdirfind`。

### 未采纳项

| 参考工具项 | 处理 |
|---|---|
| Source Control 子菜单（Add/Check In/Get…） | 丢弃（SCC 不在范围） |
| Open With > 关联程序 / 外部程序列表、Explorer 右键菜单 | 后置到 `session-and-cli`（需要 Open With 注册表） |
| Archive handling（zip 作为文件夹）、Touch local files when copying to FTP | 后置（VFS 提供者范围） |
| Copy NTFS file permissions、Maintain DOS 8.3 aliases | 后置：选项保留在模型中，实现标 `Unsupported` |
| Merge Base Folders / Folder Merge | 后置 |
| Other Filters 的"源代码控制状态" | 丢弃 |
| Folder Compare Report / File Compare Report | 后置到 `hex-and-reports`（本 change 只生成黄金用的报告，由参考工具产出） |
| Set as Base Folders 的"Open in New View" | 本 change 实现为新建视图，标签页管理随 `session-and-cli` |

## Risks / Trade-offs

- [并行分支与 `text-compare-core` 合并冲突] → GUI 只加新模块；`bc-gui/app.rs` 改动限于分支与入口；合并由合并 worker 处理并全量回归。
- [10 万文件的枚举与状态汇总耗时] → 枚举按文件夹为 job 单位并行（受 `ResourcePool` 限制同一卷并发），汇总脏标记传播；基准在任务 1.5 验证。
- [回收站/属性的 Win32 调用错误处理] → 所有 Win32 失败映射为 `VfsError` 并带 `GetLastError` 文本；单测用临时目录，回收站测试标 `#[ignore]` 仅本机手动跑。
- [时间戳精度与时区规则误判] → 三条规则独立单测，黄金语料包含 DST/时区样例。
- [规则比较在合并前不可用] → 明确 `Unsupported` 并在报告列出；合并后任务 2.5 接通。

## Migration Plan

- 无用户数据迁移；新视图通过 Home 入口与命令行两文件夹参数进入。
- 分支 `folder-compare-sync` 从 `master` 当前 HEAD 创建；完成后由一次合并提交回 `master`，合并前后各跑一次全量 clippy/test/基准。

## Open Questions

- 默认时间戳容差是否取 2 秒（参考工具默认值待在设置对话框核实，不影响接口）。
- `Ignore Folder Structure` 下重名文件的配对细则（多个同名文件在不同子目录），待黄金语料核实。
