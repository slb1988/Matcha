# Proposal

## Why

审阅长文件差异时，用户经常需要在几处相隔很远的位置之间来回对照（例如函数定义与调用处、合并冲突与其依赖改动）。文本比较的 Search 菜单已按参考列出 Toggle Bookmark / Go To Bookmark（0–9）/ Clear Bookmarks，但全部禁用；文本合并视图连菜单项都没有。参考工具在文本比较与文本合并中都提供编号书签。

## What Changes

- 文本比较视图与文本合并视图提供 10 个编号书签（0–9）：
  - Toggle Bookmark N（Shift+Ctrl+N）：在当前行放置书签 N；当前行已是书签 N 时移除。一行最多一个书签，书签 N 只存在一处。
  - Go To Bookmark N（Ctrl+N）：跳到书签 N 所在行并滚动到可见；未设置时状态栏提示。
  - Clear Bookmarks：移除本视图全部书签。
- 书签行在所属窗格显示带编号的标记，缩略导航条上显示刻度。
- 文本比较中书签挂在某一侧的行上，随本侧编辑上下平移；重载保持（越界收敛到末行），换文件时清除该侧书签，Swap Sides 时随内容换侧。
- 书签只在会话打开期间有效，不写入命名会话。
- 解除 Ctrl+0 被界面缩放快捷键占用的问题，使 Ctrl+0 / Shift+Ctrl+0 可用于书签 0。副作用：egui 默认的 Ctrl+= / Ctrl+− 整体界面缩放随之关闭（从未作为功能记录；编辑器字号仍在 Options 与 View › Text display 中调整）。

## Capabilities

### New Capabilities
- `text-bookmarks`: 文本比较与文本合并视图中的编号书签：放置、跳转、清除、显示，以及书签随编辑与重载的位置保持规则。

### Modified Capabilities
（无）

## Impact

- `crates/bc-gui/src/text/`：`view.rs`（书签状态、命令、编辑后平移、换文件 / 交换 / 重载处理、菜单启用）、`render.rs`（窗格内标记）、`nav_bar.rs`（刻度）、键盘映射。
- `crates/bc-gui/src/merge/view.rs`：Search 菜单新增书签项、快捷键、窗格标记。
- 新增共用小模块（书签集合与平移规则），便于两视图复用与单测。
- 应用级：关闭 egui 的键盘缩放快捷键（`zoom_with_keyboard`）。
- 无新依赖；不改命令行、会话文件格式。
- 文档：新增 `docs/features/text-bookmarks.md`、更新 `docs/features/README.md` 索引，`text-compare.md` / `text-merge.md` 加链接，`CHANGELOGS.md`。
