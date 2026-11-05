# Tasks

说明：本 change 在分支/worktree `folder-compare-sync` 上实现，与 `text-compare-core` 并行。全程约束：`cargo clippy --workspace --all-targets -- -D warnings` 零警告、`cargo test --workspace` 通过、除 `bc-vfs` 的 `windows-sys`（五个特性）外不新增第三方依赖、引擎 crate 不依赖 GUI、不改动 `tmp/` 与 vault。每个任务组至少一个 Conventional Commits 提交。

## 1. 本地文件系统与路径

- [x] 1.1 实现 `VfsPath::parse/join/parent/file_name`（本地路径、UNC、`scheme://` 往返、显示形式保持）；验证单测覆盖盘符、UNC、相对、`file://` 与非法输入
- [x] 1.2 实现 `LocalFs::list/metadata/open_read`：类型、大小、修改时间、DOS 属性、链接判定（reparse point）、无权限子目录带错误返回；验证单测在临时目录构造文件/文件夹/junction（Windows）并断言字段
- [x] 1.3 实现 `LocalFs::get/put/rename/make_dir`（流式复制、临时名+原子改名、同卷移动走重命名、跨卷复制+删除、可取消）与 `set_modified/set_attrs`；验证单测：复制保留时间戳与属性、取消后无半成品、Touch 指定时间往返
- [x] 1.4 实现 `LocalFs::delete`（永久/回收站，`windows-sys` 回收站，不支持时返回 `Unsupported`）、`crc32`（流式 + 取消）、`watch`（Windows `ReadDirectoryChangesW`，其他平台轮询）；验证单测：永久删除递归、CRC 与已知向量一致、监视能收到创建事件；回收站测试 `#[ignore]` 并在报告记录手动结果
- [x] 1.5 基准 `examples/list_bench.rs`：生成 10 万文件的临时树，测枚举+快速状态耗时；验证 < 1 s 顶层可见、全树枚举耗时写入报告

## 2. 比较树与准则

- [x] 2.1 实现 `DirTree` 分层子项表、名称键（大小写折叠/NFC/去扩展名可选）、`apply_listing`、脏标记状态汇总、展开/折叠、可见行派生缓存；验证单测：同名配对、孤儿、大小写、扩展名对齐、汇总颜色场景（folder-compare-tree spec）
- [x] 2.2 实现对齐覆盖（掩码/正则对、限定文件夹）与 Align With；验证单测：`x*.jpg`↔`y*.jpg` 配对、覆盖后重新扫描仍生效
- [x] 2.3 实现快速测试：大小、时间戳（容差/DST/时区）、文件名大小写、属性四项与状态推导（含较新侧标记）；验证单测覆盖 folder-criteria spec 的全部快速测试场景
- [x] 2.4 实现 `ContentCompareJob`：CRC / 二进制（分块、首个不同块提前退出）、"快速相同则跳过"、"内容覆盖快速"、修订号校验写回；验证单测：1 GB 稀疏文件前 4 KB 不同时读取量 < 1 MB；取消在 1 s 内返回
- [x] 2.5 接通规则比较：调用 `bc-engine-text` 公开 API 得到重要/不重要统计（若分支上尚无该 API，实现为 `Unsupported` 并在报告标注，合并后补）；验证单测或占位测试按实际情况二选一并写明
- [x] 2.6 实现版本比较（PE 版本资源读取，`bc_engine_bin::ver::read_version_info`）；验证单测对本机任一 `.dll` 读取到版本字符串
- [x] 2.7 实现 `Criteria::parse_script` 与反向序列化；验证单测：`criteria attrib:sh timestamp:2sec;IgnoreDST rules-based timezone:left+6` 往返

## 3. 过滤

- [ ] 3.1 实现完整掩码语法（`? * [set] [!set] [[]`、尾部 `.`、`*.*` 匹配无扩展名、相对前缀 `p\ .\ ...\`）与四组包含/排除组合求值；验证单测覆盖 file_masks 帮助中的每个示例与 folder-filters spec 场景
- [ ] 3.2 实现其他过滤（日期绝对/相对天数、大小单位、文本内容包含/不包含、DOS 属性）与"排除受保护文件"默认、单侧被过滤的 teal 规则；验证单测：七天前、单侧过滤
- [ ] 3.3 实现 13 种显示过滤、4 种文件夹显示过滤（含扁平化虚拟树）、Suppress Filters、Ignore Unimportant；验证单测：每种显示过滤对 8 种状态组合的真值表、扁平化配对
- [ ] 3.4 实现 `filter` 脚本参数解析（掩码串、`cutoff:`、`attrib:`、`size:`、protected）与过滤预设持久化模型；验证单测：帮助中的 7 个 filter 示例

## 4. 文件操作

- [ ] 4.1 实现 `OpPlan` 规划（复制/移动/交换/到文件夹/删除/重命名含掩码与正则/属性/Touch/新建文件夹/排除/忽略/复制文件名），遵守过滤与"包含隐藏项"覆盖；验证单测：规划结果（源/目标/确认类型）对 folder-operations spec 各场景
- [ ] 4.2 实现 `FileOpJob` 执行：`Interaction` 确认（Yes/No/All）、覆盖较新提示、回收站退回永久删除、进度、取消保留已完成部分、`OpLog` 事件流；验证单测用假 `Interaction` 回放 Yes-to-All / No-to-All，取消后计数正确
- [ ] 4.3 实现操作后的局部刷新（受影响节点与祖先）、Refresh/Full Refresh/Refresh Selection、自动刷新（监视事件驱动）；验证单测：复制 1 个文件只重新读取 1 行及祖先
- [ ] 4.4 实现脚本等价 API：`copy/copyto/move/moveto/delete/rename/touch/attrib/select` 的参数到 `OpPlan` 的映射；验证单测：帮助中每条命令的示例可解析并生成预期计划

## 5. 同步

- [ ] 5.1 实现 `SyncPlan::build`：五种预设 + 自定义映射、文件夹汇总动作、冲突判定（Update Both 双侧变化）、`visible`/`create-empty`；验证单测：folder-sync spec 的 Update Right / Mirror to Right / 冲突场景
- [ ] 5.2 实现逐项覆盖与 Exclude、冲突导航、计划→`OpPlan` 翻译与执行、同步统计；验证单测：覆盖单项后执行结果；`mirror:all` 报错
- [ ] 5.3 实现 `sync` 脚本参数解析；验证单测：`sync create-empty mirror:left->right`

## 6. GUI

- [ ] 6.1 实现 `dir_view.rs` 虚拟化树表：左右对称列、中央状态列、颜色/空心/黄色/teal、DOS 属性括号、列显示与排序、Legend 窗口；验证手动：10 万文件树滚动流畅，`--bench-dir` 基准 update p99 < 4 ms
- [ ] 6.2 实现工具栏与菜单：Filters 编辑框与预设、显示过滤与文件夹显示过滤、Suppress Filters、Ignore Unimportant、Swap/Back/Forward/Browse/Up One Level/Set as Base/Compare Parent、展开折叠、选择命令、Find Filename；验证手动对照 folder-compare-tree 与 folder-filters spec 场景
- [ ] 6.3 实现操作命令与确认对话框（Yes/No/All）、进度条、Log 面板、Copy to Folder/Move to Folder 目标选择、Rename/Attributes/Touch 对话框；验证手动对照 folder-operations spec
- [ ] 6.4 实现 Open/Compare To/Quick Compare：同名或任选两文件以文本视图打开（复用 `master` 合并前的原型或合并后的正式控件）；验证手动：跨行比较打开新视图
- [ ] 6.5 实现 `sync_view.rs`：方法下拉、Rules/Filters 入口、预览列表与动作图标、逐项覆盖菜单、Next/Previous Conflict、Sync Now、统计；验证手动对照 folder-sync spec
- [ ] 6.6 Home 入口与命令行：Home 页增加"文件夹比较"卡片与拖放两个文件夹即比较；`matcha <dirL> <dirR>` 直接进入文件夹视图；验证 `cargo run --release -p bcompare -- <dirL> <dirR>`

## 7. 黄金语料与对照

- [ ] 7.1 编写 `tools/golden/gen_dir_expected.ps1`：生成 ≥ 20 个目录对用例（孤儿、较新、容差内/外、DST、属性、CRC 不同、规则比较仅注释、过滤掩码、相对掩码、扁平化），用参考工具脚本 `load/criteria/filter/expand all/folder-report layout:xml` 产出期望并入库；验证脚本在本机跑通
- [ ] 7.2 编写期望解析与回归测试 `bc-engine-dir/tests/golden_dir.rs`（语料为空时跳过）；验证状态逐行一致率 100%，不一致写 `tests/golden/dir/KNOWN_DIFFS.md`
- [ ] 7.3 编写 `docs/reference-parity/folder-compare.md` 与 `folder-sync.md`：按 D10 逐条命令/选项记录已实现/未采纳(理由)/后置；验证覆盖帮助主题中的每一条命令

## 8. 集成

- [ ] 8.1 全量回归（clippy `-D warnings`、test、10 万文件基准、黄金一致率）并写报告；验证数字写入 `docs/worker-reports/folder-compare-sync-*.md`
- [ ] 8.2 与 `master` 合并：rebase 或 merge 后重新跑全量回归，解决 `bc-gui/app.rs`、`home.rs` 冲突并接通 2.5 规则比较；验证合并后 `cargo test --workspace` 全绿且两个视图都可打开

## Workflow follow-up

- 评审通过后归档本 change；`session-and-cli` 以其内部命令 API 为基线。
