# Proposal

## Why

文件夹比较是对比工具的第二条腿（FR-D1…FR-D5），也是 CI/脚本用户最常用的入口；`bc-engine-dir`、`bc-vfs::LocalFs` 目前全是 `todo!()`，GUI 没有文件夹视图。该能力与文本引擎只有"规则比较"一处弱依赖，可以在 `text-compare-core` 收尾的同时在独立 worktree 并行实现，缩短 M1 的整体周期。

## What Changes

- 实现本地文件系统提供者 `LocalFs`（列目录、元数据、复制/移动/删除（含回收站）/重命名/新建目录、CRC32、设置时间戳与属性、目录变更监视），以及 Windows 的 DOS 属性与 junction/symlink 处理。
- 实现双树文件夹比较引擎：按名配对（大小写、扩展名对齐、Unicode 规范化、手动对齐覆盖）、后台扫描、状态与颜色语义、展开/折叠、列、导航、文件名查找。
- 实现比较准则管线：快速测试（大小、时间戳容差/DST/时区、文件名大小写、属性）→ 内容测试（CRC / 二进制 / 规则比较）→ 版本；"快速测试相同则跳过"、"内容结果覆盖快速测试"、"忽略不重要差异"。
- 实现过滤：名称过滤（参考工具完整掩码语法，含字符集与相对掩码）、其他过滤（日期/大小/属性/内容）、13 种显示过滤、4 种文件夹显示过滤、临时取消过滤（teal 着色）、过滤预设。
- 实现文件操作：复制/移动/交换/复制到文件夹/移动到文件夹/删除/重命名/属性/Touch/排除/新建文件夹/复制文件名/标记忽略，带确认对话框语义（yes-to-all）、操作日志、后台执行与取消、操作后局部刷新。
- 实现文件夹同步：五种预设方法 + 自定义、预览与逐项动作覆盖（Leave Alone / Copy / Delete）、冲突导航、执行与日志。
- GUI：文件夹比较视图（虚拟化树、列、颜色、图例、显示过滤工具栏、Filters 编辑框、日志面板）与文件夹同步视图（方法下拉、预览、Sync Now）。
- 黄金语料：用参考工具脚本（`load` + `expand all` + `folder-report layout:xml`）生成期望，覆盖准则与过滤场景（脚本引擎可全程无 GUI）。
- 不在本 change：Folder Merge（3 路目录，后置）、远程 VFS、压缩档作为文件夹、快照、SCC 子菜单、脚本引擎本身（`session-and-cli` 负责，但本 change 的内部命令 API 要能被它直接调用）。

## Capabilities

### New Capabilities

- `folder-compare-tree`: 双树模型、配对规则、状态与颜色、扫描/展开/导航/查找、视图列与图例。
- `folder-criteria`: 快速测试与内容测试的准则管线、时间戳容差、版本比较、覆盖规则。
- `folder-filters`: 名称掩码语法、其他过滤、显示过滤、文件夹显示过滤、预设与临时取消。
- `folder-operations`: 对选区的文件操作、确认与日志、后台执行、对过滤内容的处理。
- `folder-sync`: 同步方法、预览、逐项覆盖、执行与冲突。
- `local-filesystem`: 本地文件系统提供者的行为（属性、回收站、时间戳、junction、CRC、监视）。

### Modified Capabilities

（无。`text-compare-core` 的能力不变；规则比较通过其公开 API 调用。）

## Impact

- 代码：`crates/bc-vfs`（`LocalFs`、`VfsPath`、`FileAttrs`）、`crates/bc-engine-dir`（全部模块）、`crates/bc-gui`（新增 `dir_view`、`sync_view`、图例/列/日志组件，`app.rs` 最小接线）、`crates/bc-core/tests/golden/dir/*`。
- 依赖：允许在 `bc-vfs` 新增 `windows-sys`（仅 `Win32_Foundation`、`Win32_Storage_FileSystem`、`Win32_UI_Shell`、`Win32_System_IO`、`Win32_Globalization` 五个特性，`cfg(windows)`），用于回收站删除、DOS 属性与 reparse point；其他 crate 不得引入新依赖。
- 并行约束：在独立 git worktree/分支 `folder-compare-sync` 开发；`bc-engine-text` 的规则比较接口以 `master` 合并后的版本为准，合并前用显式 `Unsupported` 占位。
- 与 `text-compare-core` 共享：`bc-core` 的取消接口、掩码解析原语 `split_mask_list`、`Crc32`。
