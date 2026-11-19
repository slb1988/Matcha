# Tasks

## 1. 功能文档先行

- [x] 1.1 新建 `docs/features/text-bookmarks.md`（模板五节），写清放置 / 移除 / 同行取代、跳转、清除、显示位置、编辑平移规则、重载 / 换文件 / 交换、合并视图锚点、不持久化与 Ctrl+0 说明；`docs/features/README.md` 索引加一行（状态「进行中」），`text-compare.md` 与 `text-merge.md` 加链接；验收：与 `specs/text-bookmarks/spec.md` 各需求一一对应

## 2. 书签数据模型与平移规则

- [ ] 2.1 新增 `bc-gui/src/text/bookmarks.rs`：`Bookmarks<A>`（toggle / get / clear / at / is_empty，design 决定 1）；单测覆盖 spec「放置与移除书签」四个场景（放置、再次切换移除、移动编号、同行换编号）与 clear
- [ ] 2.2 实现平移函数 `shift(lines_old, lines_new, line) -> line`（design 决定 2）；单测：行数不变不平移、上方插入 3 行 50→53、删除 48–52 时 50→48、删除到文件末尾时钳制到末行、书签在变化区间之后按增量平移、变化区间内保持相对偏移；`cargo test -p bc-gui bookmarks` 通过

## 3. 文本比较接入

- [ ] 3.1 菜单：把 Toggle / Go to 两个子菜单的子项 action_id 区分为 `Toggle Bookmark N` / `Go to Bookmark N`（标签仍为 `Bookmark N`）；`update_menus` 启用规则：Toggle 子项恒可用、Go to 子项仅已设置、Clear 仅非空；`command()` 实现三类命令；键盘映射 Shift+Ctrl+0–9 / Ctrl+0–9；`view_tests.rs` 测试菜单启用状态随书签变化
- [ ] 3.2 Go to：切活动侧、光标到行首、被显示过滤隐藏时先展开、`goto_row` 使其可见；未设置写状态栏；测试：书签在右侧第 400 行、从左侧第 10 行 Ctrl+3 后 active=右、光标行 400 且在视口内；Ctrl+7 未设置时光标不动、状态栏提示
- [ ] 3.3 编辑跟随：每帧按修订号校正（design 决定 2）；测试：左侧上方插行后书签行文字不变、删除书签行后落点正确、右侧编辑不影响左侧书签、撤销后回到原行
- [ ] 3.4 重载 / 换文件 / 交换（design 决定 3）：测试：右侧 Open File 清除右侧书签保留左侧、Reload 钳制到新行数、Swap Sides 后书签换侧且行号不变；命名会话保存后重开无书签
- [ ] 3.5 绘制：文本区右缘编号徽标、缩略导航条刻度（design 决定 4）；测试：差异区块首行放书签后点击复制箭头仍执行复制；横向滚动后徽标仍在窗格内；`dev-build.ps1` 编译通过

## 4. 文本合并接入

- [ ] 4.1 Search 菜单追加 Toggle Bookmark ▸ / Go to Bookmark ▸ / Clear Bookmarks 与快捷键，锚点 `(pane, row)`（输出窗格取光标行的 `out_line(line).row`）；Go to 激活锚点窗格并 `goto_row`；锚点窗格文本区右缘画徽标；`merge/view_tests.rs` 测试：base 窗格冲突行放书签 1 后滚到末尾、Ctrl+1 激活 base 并定位；对该区段 Take Right 后书签行不变；`cargo test -p bc-gui merge::` 通过

## 5. Ctrl+0 与全局设置

- [ ] 5.1 应用启动时关闭 `zoom_with_keyboard`（design 决定 6）；`app_tests.rs` 测试：注入 Ctrl+0 后 `zoom_factor` 仍为 1 且文本视图跳到书签 0；现有快照场景逐像素不变

## 6. 集成检查与记录

- [ ] 6.1 `cargo test -p bc-gui`、`cargo clippy -p bc-gui` 无新增失败或告警；`dev-build.ps1` 后实机：文本比较放 3 个书签、跨文件跳转、在上方插行后跳转仍落在原文字、Swap Sides、Clear；合并视图放书签后 Take 再跳转；确认 Ctrl+0 / Shift+Ctrl+0 可用
- [ ] 6.2 `docs/features/text-bookmarks.md` 状态改为已实现并同步 README 索引；`CHANGELOGS.md` 顶部加条目（含关闭 Ctrl+= / Ctrl+− 界面缩放的说明）；验收：文档与实际行为一致

## Workflow follow-up

- 评审通过后执行 `/opsx:archive text-bookmarks`，新建主规格 `openspec/specs/text-bookmarks/spec.md`。
