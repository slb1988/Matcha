# 磁盘变化检测与重载（Disk Change Reload）

> 已打开的文件被外部程序修改后，切回 Matcha 时弹框询问是否重载（或按设置自动重载）。
> 状态：**已实现**（单元测试 + 视图集成测试 + app 焦点/标签切换流程测试；真实窗口焦点切换弹框已手动确认）（2026-11-19 更新）。

## 功能简介

- **何时检查**：主窗口重新获得焦点、或切换到某个文件视图标签页时，检查该视图两侧来自磁盘的文件
  （修改时间或大小与载入时不同即视为变化）。不在后台持续监听，与参考 Options › Tweaks 的行为一致。
  剪贴板内容等没有磁盘路径的一侧不参与。
- **询问**：发现变化弹出 **File changed on disk, reload?**，列出变化的文件；某侧有未保存编辑时注明重载会丢弃这些修改。
  按 **Reload** 重载列出的各侧，按 **Keep Current** 保留当前内容——同一份磁盘内容（时间、大小都没再变）不再重复询问，
  文件再被改一次才会再问。两侧同时变化只弹一个框。
- **自动重载**（默认关闭）：开启后，变化的一侧若没有未保存编辑，直接重载并在状态栏说明；有编辑的一侧仍弹框询问。
- **保持位置**：因磁盘变化触发的重载保留当前行与滚动位置（越界收敛到末行），不会跳到第一个差异。
  手动 Reload（F5）行为不变。
- **自己保存不算变化**：在 Matcha 中保存某侧后以新的磁盘状态为基准，不会再提示。
- **文件消失或不可读**：只在状态栏提示，保留当前内容，不弹框。
- **覆盖视图**：文本比较、表格比较、十六进制比较，以及文件夹比较中打开的文件对窗口。
  文本合并、表格合并与文件夹比较视图本身不参与（重载会重跑合并、丢失解决进度；文件夹视图用 Refresh）。
- **设置**：Tools › Options › **Tweaks**：
  - **Check for files changed on disk**（默认开）
  - **Automatically reload unless changes will be discarded**（默认关；前者关闭时置灰）

  写入 `%APPDATA%\Matcha\settings.txt` 的 `files.check-changed-on-disk` / `files.auto-reload`。

## 代码地图

| 层 | 位置 | 内容 |
|---|---|---|
| 判定核心 | `crates/bc-gui/src/disk_watch.rs` | `DiskStamp`（修改时间 + 大小）、`DiskTracked` 视图接口、`check` 决策（自动重载 / 询问 / 消失 / 无） |
| 视图接入 | `crates/bc-gui/src/text/view.rs`、`table/`、`hex/view.rs` | 每侧载入 / 保存后的基准标识、拒绝记忆、保持位置的重载 |
| 触发 | `crates/bc-gui/src/app.rs` | 焦点恢复与标签切换时对活动标签调检查；文件夹视图转发给其文件对窗口 |
| 设置 | `crates/bc-session` `Prefs::disk_changes`、`crates/bc-gui/src/settings.rs`、`dialogs/options.rs`（Tweaks 页） | 两个开关与持久化 |

## 关键语义

- 变化判定只看修改时间与大小，不做内容哈希；同一秒内大小不变的两次修改可能漏检（与参考同等限制）。
- **文本比较不锁文件**：小于 64 MiB 的文本文件读入内存、不持有句柄，比较期间外部程序可以照常保存、替换或删除它
  （此前 Windows 上的只读映射会让外部保存直接失败）。**已知限制**：≥ 64 MiB 的文本文件为性能仍按只读映射打开，
  打开期间外部写入会被系统拒绝（`TextContent::MAP_THRESHOLD`）。表格与十六进制视图本来就不锁文件。
- 拒绝记忆按「拒绝时的磁盘标识」记录，标识再变即重新询问。
- 确认框打开期间不会叠加第二个确认框。
- 有未保存编辑的一侧在确认重载后编辑被丢弃（确认框已说明），不再走「先保存？」询问。

## 使用与验证

- 单元测试：`cargo test -p bc-gui disk_watch`（决策分支）、`text::` / `table::` / `hex::`（各视图接入）、`app`（触发点）。
- 实机：`target\debug\matcha.exe a.txt b.txt`，在记事本中修改 b.txt 保存后切回 Matcha → 弹框；
  Tweaks 开启自动重载后再改 → 静默重载、状态栏提示。

## 深入阅读

- 变更过程：`openspec/changes/archive/2026-11-19-disk-change-reload/`（proposal / design / tasks）
- 规格：`openspec/specs/disk-change-detection/spec.md`
- 参考帮助：Options › Tweaks（`tmp/hex-ref/chm/optionstweak.html`）
