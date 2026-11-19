# Proposal

## Why

文件在 Matcha 外部被修改（IDE 保存、`git checkout`/`git pull`、构建产物重新生成）后，已打开的比较视图既不提示也不刷新，用户看到的是过期差异，甚至可能把旧内容保存回去、覆盖外部改动。参考工具在 Options › Tweaks 提供「Check for files changed on disk」与「Automatically reload unless changes will be discarded」两个开关，是对比工具的基础能力。

## What Changes

- 文件视图（文本比较、表格比较、十六进制比较，含文件夹比较里以浮动窗口打开的文件对）记录每侧文件载入时的磁盘标识（修改时间 + 大小）。
- 主窗口重新获得焦点、或切换到某个文件视图标签页时，检查该视图两侧文件是否在磁盘上变化：
  - 已变化且开启自动重载、该侧没有未保存编辑 → 静默重载该侧，状态栏说明；
  - 否则弹出 "File changed on disk, reload?" 确认，列出变化的文件；选择不重载后，同一份磁盘内容不再重复询问。
- 磁盘变化触发的重载保留当前行与滚动位置（越界时收敛到末行），不跳到第一个差异。
- Matcha 自己保存某侧后刷新该侧磁盘标识，不会把自己的保存当成外部修改。
- 文件被删除或变得不可读时只在状态栏提示，不弹框、不清空当前内容。
- Tools › Options › Tweaks 页从占位变为可用，提供上述两个开关；设置写入 `settings.txt`，重启保持。默认：检查开启，自动重载关闭。

## Capabilities

### New Capabilities
- `disk-change-detection`: 文件视图对外部磁盘修改的检测、提示、自动重载与相关偏好设置（跨文本 / 表格 / 十六进制视图的统一行为）。

### Modified Capabilities
（无：既有视图 spec 中的手动 Reload 行为不变，新行为统一由新能力承载。）

## Impact

- `crates/bc-gui`：`text/view.rs`、`table/`（`view.rs`、`commands.rs`）、`hex/view.rs` 增加磁盘标识记录与检查入口；`app.rs` 在焦点恢复 / 标签切换时调度检查；`dir_view.rs` 把检查转发给浮动的文件视图；`dialogs/options.rs` Tweaks 页；`settings.rs` 新增两个键。
- `crates/bc-session`：`Prefs` 新增磁盘变化相关偏好。
- 不涉及文本 / 表格合并视图、文件夹比较视图本身（见 design 的 Non-Goals）。
- 无新依赖；不改命令行与退出码。
- 文档：新增 `docs/features/disk-change-reload.md`，更新 `docs/features/README.md` 索引、`gui-shell.md` 的偏好持久化小节与 `CHANGELOGS.md`。
