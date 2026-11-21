# Proposal

## Why

找不到下一个/上一个差异时，各视图的反馈不统一。文本比较（`bd5b2f0`）已经会弹出黄色 **Difference not found.** 面板并播放提示音；其他视图只在状态栏改一行字。具体是：表格写 "No more differences / edits"，十六进制写中文"最后一个差异之后"，合并视图写 "No more changes below/above"，文件夹比较写 "No further difference/conflict"。这些文字容易被忽略，看起来像按键没有反应。
Options 对话框里早就列出了 **Next Difference** 页，但一直未接入偏好，参考软件的 "Show message panel" 和 "Wrap around to first difference" 两个开关都没有提供。

## What Changes

- 所有比较/合并视图的「下一个/上一个差异」类导航找不到目标时，统一在状态栏上方弹出消息面板。面板文案固定为 **Difference not found.**，同时播放一次系统提示音；用户的下一次按键或鼠标按下关闭面板。涉及：
  - 文本比较（已完成，本次补开关）；
  - 表格比较（差异单元格 / 差异行 / 编辑）；
  - 十六进制比较（差异字节）；
  - 文本合并、表格合并（改动区段）；
  - 文件夹比较及文件夹同步（差异 / 冲突）。
- 原先状态栏里的"已无更多差异"类文字不再显示，十六进制视图的中文提示随之移除。
- Options › **Next Difference** 页接入偏好，两个复选框写入 `settings.txt`：
  - **Show message panel**（默认开）：关闭后找不到差异时既不弹面板也不响，与参考软件一致；
  - **Wrap around to first difference**（默认关）：打开后，向下找不到就回到第一个差异，向上找不到就回到最后一个差异，不弹面板。整个比较都没有差异时仍按"找不到"处理。
- First / Last Difference 不受回绕影响。

不在本次范围：
- 书签未设置（"Bookmark N is not set"）改用面板提示，待本次落地后再定；
- "When loading new files, go to first difference"、"Go to next difference after copying"、"Next Difference Files" 三个参考选项。

## Capabilities

### New Capabilities
- `difference-navigation-feedback`：跨视图的差异导航末端行为，包括消息面板（文案、提示音、关闭时机）、回绕规则，以及 Options › Next Difference 两个偏好的默认值与持久化。

### Modified Capabilities
- `text-diff-alignment`：「差异导航」要求中的"提示已无更多差异"细化为消息面板，并增加回绕。
- `hex-compare-view`：「差异导航与定位」要求原写"越过最后一个后给出末端提示且不回卷"，改为遵循回绕偏好与消息面板。

## Impact

- `crates/bc-session/src/prefs.rs`：新增 `NextDiffPrefs`，挂在 `Prefs` 上。
- `crates/bc-gui/src/settings.rs`：新增两个持久化键。
- `crates/bc-gui/src/dialogs/options.rs`：Next Difference 页从"未接入"改为可编辑。
- `crates/bc-gui/src/chrome/message_panel.rs`：在现有面板之上加共用的「找不到」处理（显示/关闭/占位高度）。
- 视图：
  - `text/view.rs`
  - `table/commands.rs`、`table/chrome_ui.rs`、`table/view.rs`
  - `hex/view.rs`
  - `merge/view.rs`
  - `table_merge/view.rs`
  - `dir_view.rs`（含后台线程的冲突导航通知）
  - `app.rs`：hex / 文件夹视图的 `ui()` 不带 `Prefs`，由宿主同步偏好。
- 文档：
  - `docs/features/` 下的 text-compare、table-compare、hex-compare、text-merge、table-merge、folder-compare 各 feature 文档，以及 gui-shell 的偏好持久化一节；
  - `CHANGELOGS.md`。
- 无新依赖：提示音沿用已依赖的 `bc_script::platform::beep()`。
