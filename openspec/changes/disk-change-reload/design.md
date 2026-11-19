# Design

## Context

- 三个文件视图都已有手动重载：文本 `TextCompareView::request(Pending::Reload(s) | ReloadBoth)`（有未保存编辑时先走保存确认）、表格 `request(Pending::Reload)`（`Staged::Reload`，失败保留旧比较）、十六进制 `reload_guarded(force)`。
- 文本的 `open_side` 会重置该侧 `Editor` 并置 `land_on_first_diff = true`，即重载后会跳到第一个差异——磁盘变化触发的重载不能沿用这一点（spec「重载保持位置」）。
- 各视图载入时已读过 `metadata.modified()`（文本 `FileInfo.modified_label`、表格 `chrome_ui.rs`、十六进制 `view.rs`），但没有保存可比较的标识。
- 偏好分两处：`bc_session::prefs::Prefs`（内存模型）与 `bc-gui/src/settings.rs`（`%APPDATA%\Matcha\settings.txt` 键值持久化）。Options 对话框 `PAGES` 中 `("Tweaks", false)` 是未接线占位。
- `bc-vfs` 有 `PollWatcher`/Windows watcher，但它是目录级快照，用于文件夹树；本功能只关心少数已打开文件。
- 参考帮助（`optionstweak.html`）：在回到文件视图时检查；"File changed on disk, reload?"；自动重载仅当不会丢弃修改。

## Goals / Non-Goals

**Goals:**
- 三类文件视图共用一套检测与决策逻辑，视图只提供「每侧路径 / 标识 / 是否有未保存编辑」与「保持位置地重载某侧」两个能力。
- 检查只在焦点恢复与标签切换时做一次 `metadata` 调用，空闲时零开销。

**Non-Goals:**
- 持续监听（文件系统通知 / 定时轮询）：参考也只在回到视图时检查；避免后台线程与通知风暴。
- 文本合并 / 表格合并：输入通常是 VCS 临时文件，输出目标的既有内容按 spec 本就被忽略；重载会重跑合并、丢掉解决进度，语义需另行设计。
- 文件夹比较视图的自动刷新（已有 Refresh；目录级变化另立 change）。
- 保存时的「磁盘内容比载入时新，确定覆盖？」冲突提示（可作为后续增强）。

## Decisions

1. **磁盘标识 = (修改时间, 大小)**，放在 `bc-gui` 新模块 `disk_watch.rs`：
   `struct DiskStamp { modified: Option<SystemTime>, len: u64 }`，`fn stamp(path) -> io::Result<DiskStamp>`。
   - 不做内容哈希：大文件代价高；mtime+size 与参考一致，足以覆盖编辑器保存和 git 检出。
   - 每侧保存 `loaded: Option<DiskStamp>`（载入 / 保存成功后更新）与 `declined: Option<DiskStamp>`（用户拒绝时记下当时的磁盘标识）。
   - 判定：当前标识 ≠ `loaded` 且 ≠ `declined` → 变化；`metadata` 失败 → 「消失或不可读」，只出状态栏消息，且同一状态只提示一次。

2. **视图侧接口用 trait，决策集中在一处**：
   ```text
   trait DiskTracked {
       fn tracked_sides(&self) -> Vec<TrackedSide>;   // 路径、loaded、declined、dirty
       fn reload_side_keep_position(&mut self, side) -> Result<(), String>;
       fn set_declined(&mut self, side, DiskStamp);
   }
   fn check(view: &mut dyn DiskTracked, prefs) -> CheckOutcome  // AutoReloaded / Ask(list) / Gone(msg) / Nothing
   ```
   文本、表格、十六进制各实现一次；`check` 只写一次、单测覆盖全部分支（无需 GUI）。
   - 备选：每个视图各自实现判定 —— 三份重复逻辑，测试也要三份，放弃。

3. **触发点在 `app.rs`**：每帧读 `ctx.input(|i| i.viewport().focused)`，记住上一帧值；`false→true` 或活动标签页索引变化时，对活动标签页调 `check`。`View::FolderCompare` 转发给其内部打开的浮动文件视图（`dir_view.rs` 的 `text: Option<Box<TextCompareView>>` 等）。快照 / 基准模式（已固定 `focused = false`）不会触发。

4. **确认框复用现有 modal**（`chrome/modal.rs`）：标题 "File changed on disk, reload?"，正文逐行列出文件；某侧 dirty 时追加 "Unsaved changes to <name> will be lost."；按钮 Reload / Keep Current。确认框以视图字段 `disk_prompt: Option<Vec<side>>` 驱动，存在时 `check` 直接返回 `Nothing`（spec：不叠加）。

5. **保持位置的重载**：
   - 文本：新增 `reload_keep_position(s)`：记下 `current_row`、`scroll`、活动侧光标行；调用 `open_side` 后立即把 `land_on_first_diff` 置回 `false`，并在新对齐完成后按记下的值恢复（行号钳制到行数-1）。dirty 侧被确认重载时，丢弃编辑即可（用户已在确认框中同意），不再走「保存后重载」确认。
   - 表格：沿用 `Staged::Reload` 流程，完成回调里恢复选中格与滚动（钳制）。
   - 十六进制：`reload_guarded(force = true)` 后恢复偏移与滚动（钳制到新长度）。

6. **文本比较不再锁住小文件**（实现中发现，用户确认）：`TextContent::open` 原本总是经 `LineList::open_mapped`
   打开——Windows 上以 `FILE_SHARE_READ` 持有句柄并做只读映射，外部程序无法写入、替换或删除文件，检测永远不会触发。
   改为：文件小于 `MAP_THRESHOLD = 64 MiB` 时 `std::fs::read` 读入内存（`LineList::from_bytes`），不留句柄；
   达到阈值才映射。表格与十六进制视图实测不锁文件，不需要改。
   - 备选：全部读入内存——超大文件的打开速度与内存占用退化；映射时放开写共享——映射中的文件仍不能被截断或替换，且映射内容可能在脚下变化。

7. **偏好**：`Prefs` 新增 `disk_changes: DiskChangePrefs { check: bool = true, auto_reload: bool = false }`；`settings.txt` 新键 `files.check-changed-on-disk`、`files.auto-reload`，读写与现有 `backups.*` 同方式（非法值退回默认）。Options 的 Tweaks 改为 `("Tweaks", true)`，页面只放这两个复选框（其余参考 Tweaks 项不在本 change）。

## Risks / Trade-offs

- [mtime 精度：同一秒内两次修改且大小不变会漏检] → 与参考同等限制；NTFS mtime 为 100ns 精度，实际很少发生。
- [网络盘 / 慢盘上 `metadata` 阻塞 UI] → 每次检查只针对活动视图的至多两个文件，且只在焦点事件时；如实测超过 50 ms 再改为后台线程。
- [外部工具以「先删后写」方式保存，检查恰在中间] → 先得到「不存在」状态栏提示；下次切回时文件已存在且标识不同，按变化处理（spec 已覆盖）。
- [表格重载期间又触发检查] → 重载中的视图（worker 运行中）`tracked_sides` 返回空，跳过本次检查。

## Migration Plan

纯新增；旧 `settings.txt` 没有新键时取默认值（检查开启），升级后用户即获得提示行为。回滚只需移除新键读取，不影响其他设置。
