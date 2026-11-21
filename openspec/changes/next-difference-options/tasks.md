# Tasks

每组结束时执行 `tools/dev-build.ps1` 并运行相关测试，通过后单独提交（只 stage 本组文件），同一提交更新对应 feature 文档与 `CHANGELOGS.md`。

## 1. 偏好、持久化与 Options › Next Difference 页

- [x] 1.1 在 `bc-session/src/prefs.rs` 新增 `NextDiffPrefs { message_panel, wrap_around }`（`Copy`，默认 `true`/`false`）并挂到 `Prefs.next_diff`；补默认值单元测试，`cargo test -p bc-session prefs` 通过
- [x] 1.2 在 `bc-gui/src/settings.rs` 增加 `navigation.show-message-panel`、`navigation.wrap-around` 的读写；补往返测试和"缺省 / 非法值取默认"测试，`cargo test -p bc-gui settings` 通过
- [x] 1.3 `dialogs/options.rs`：`PAGES` 中 Next Difference 标为已接入，加两个复选框（文案对齐参考）及说明行；照 `tweaks_page_edits_disk_change_prefs` 写测试，覆盖勾选后 OK 写回、Cancel 不写回，`cargo test -p bc-gui options` 通过
- [x] 1.4 更新 `docs/features/gui-shell.md` 的「偏好持久化」一节，列出两个新键；`CHANGELOGS.md` 加一条；dev-build 通过后提交

## 2. 共用面板状态与文本比较接入

- [x] 2.1 在 `chrome/message_panel.rs` 增加 `NotFoundPanel`（`begin_frame` / `not_found` / `height` / `paint` / `is_shown`）和 `NOT_FOUND` 常量；补单元测试：`message_panel` 关闭时 `not_found` 不显示；按键、文字、鼠标按下关闭面板；鼠标移动不关闭
- [x] 2.2 文本视图用 `NotFoundPanel` 替换 `notice` 字段；`navigate` 接收 `NextDiffPrefs`，按 design D2 实现回绕；删除不可见的 "No more differences" 消息
- [x] 2.3 扩展 `navigating_past_the_last_difference_shows_and_dismisses_the_message_panel`，并新增测试：回绕向下/向上、关闭面板后静默、两侧相同时开启回绕仍弹面板、First/Last Difference 不受回绕影响；`cargo test -p bc-gui --lib text::` 通过
- [x] 2.4 文件夹比较的浮动文本视图：`DirCompareView` 增加 `set_next_diff`，写入 `text_prefs.next_diff`；`app.rs` 在调用文件夹/同步视图的 `ui()` 前设置；补测试，确认浮动文本视图在开启回绕时会回绕
- [x] 2.5 更新 `docs/features/text-compare.md` 的消息面板段落（去掉"暂未提供选项/不回绕"，写明两个开关的效果）；`CHANGELOGS.md`；dev-build 后提交

## 3. 表格比较

- [x] 3.1 `table/view.rs` 持有 `NotFoundPanel`：在处理键盘之前调用 `begin_frame`，主体高度扣除面板高度，面板画在状态栏正上方
- [x] 3.2 `table/commands.rs::navigate` 按 design D2 回绕（差异单元格 / 差异行 / 编辑三种 kind，以及空表的情况），找不到时调用 `not_found`，删除 "No more differences / edits"
- [x] 3.3 在 `table/navigation_tests.rs` 补测试：三种 kind 的面板与回绕、关闭面板后静默；通过快捷键和菜单命令 id 各验证一次；验证状态栏不再出现旧文字；`cargo test -p bc-gui --lib table::` 通过
- [x] 3.4 更新 `docs/features/table-compare.md`、`CHANGELOGS.md`；dev-build 后提交

## 4. 十六进制比较

- [x] 4.1 `hex/view.rs`：增加 `NotFoundPanel` 字段和 `set_next_diff`；`app.rs` 在调用 `ui()` 前设置偏好（文件夹比较没有内嵌 hex 窗口）；在按键处理前调用 `begin_frame`，状态栏上方绘制面板并让出高度
- [x] 4.2 `next_difference` 按 design D2 回绕；找不到时调用 `not_found`，删除中文"最后一个差异之后"类提示
- [x] 4.3 在 `hex/view_tests.rs` 补测试：Ctrl+N/Ctrl+P、`nav.next` 工具栏 id、菜单命令三条路径；回绕；静默；两侧相同；`cargo test -p bc-gui --lib hex::` 通过
- [x] 4.4 更新 `docs/features/hex-compare.md`、`CHANGELOGS.md`；dev-build 后提交

## 5. 文本合并与表格合并

- [x] 5.1 `merge/view.rs`：持有 `NotFoundPanel` 并缓存 `next_diff`（`command(id)` 不带 prefs）；Next/Previous Change Section 按 D2 回绕，找不到时弹面板，删除 "No more changes below/above"；状态栏上方绘制面板
- [x] 5.2 `table_merge/view.rs`：同 5.1（跨 sheet 回绕到第一个 / 最后一个 sheet 的改动区段）
- [x] 5.3 在 `merge/view_tests.rs` 和表格合并测试中补面板、回绕、静默的测试；`cargo test -p bc-gui --lib merge:: table_merge::` 通过
- [x] 5.4 更新 `docs/features/text-merge.md`、`docs/features/table-merge.md`、`CHANGELOGS.md`；dev-build 后提交

## 6. 文件夹比较与文件夹同步

- [x] 6.1 `dir_view.rs`：持有 `NotFoundPanel`，在处理通知与键盘之前调用 `begin_frame`，`draw_status` 上方绘制面板并让出高度
- [x] 6.2 差异导航 `navigate(backwards, false)` 按 D2 回绕，找不到时弹面板，删除 "No further difference/conflict"
- [x] 6.3 冲突导航：`Command::FocusConflict` 增加 `wrap`，后台线程找不到时按 `current: None` 再找一次；仍找不到时回传新的 `Notice::NotFound`，UI 线程据此调用 `not_found`；删除 "No further conflict"
- [x] 6.4 补测试：差异与冲突的面板、回绕、静默；同步视图（`sync_view.rs`）中的 Next/Previous Difference 也走同一路径；`cargo test -p bc-gui --lib dir_view sync_view` 通过
- [x] 6.5 更新 `docs/features/folder-compare-sync.md`、`CHANGELOGS.md`；dev-build 后提交

## 7. 集成验证

- [ ] 7.1 `cargo test -p bc-gui -p bc-session` 全部通过；`grep` 确认代码中不再有 "No more differences"、"No more changes"、"No further conflict"、"No further difference" 这几条导航提示
- [ ] 7.2 在真实窗口中验证（按 real-window-capture 的方法截图）：文本、表格、十六进制、文本合并、文件夹比较各自越过末端时出现面板且能听到提示音；勾选回绕后不弹面板；重启后两个开关保留

## Workflow follow-up

- 实施完成并经用户确认后归档本变更（`/opsx:archive`），同步主 spec。
- 归档后再决定"书签未设置提示改用面板"是否另开变更。
