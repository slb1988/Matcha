# Tasks

## 1. 功能文档先行

- [x] 1.1 更新 `docs/features/text-merge.md`：功能简介加入行内差异高亮、语法着色、查找替换、横向滚动条、输入窗格选择复制；关键语义写明 base / 输出不做行内高亮、替换仅输出窗格且标手工、输入窗格只读；状态行标「进行中」；验收：与 `specs/text-merge-view/spec.md` 的五项新增需求一一对应

## 2. 语法着色与行内差异

- [x] 2.1 `text/render.rs::layout` 增加「重要差异色覆盖」参数（design 决定 1），文本比较调用点传 `None`；`cargo test -p bc-gui text::` 与文本比较快照（`text-*` 场景）逐像素不变
- [x] 2.2 `merge/view.rs`：四窗格传入 `TextContent::tokens(line)` 与 `SyntaxStyle::for_format`、`theme::editor_bold`；新增 View 菜单 Syntax Highlighting 勾选（`show_syntax` 默认 true）；`merge/view_tests.rs` 加测试：关闭开关后 layout 不再带语法色；`dev-build.ps1` 编译通过
- [x] 2.3 行内差异缓存 `inline: HashMap<(pane,row), InlineDiff>`：只算左 / 右窗格的改动或冲突行且本侧与 base 都有内容；左窗格用 `MERGE_LEFT`、右窗格 `MERGE_RIGHT`、冲突区段 `MERGE_CONFLICT`；单测覆盖：`price`→`cost` 只标该词、冲突区段用冲突色、整行新增不做行内、base / 输出窗格不做行内、缓存上限清空

## 3. 输入窗格选择与复制

- [x] 3.1 抽出四窗格通用的「指针 → (行, 字节)」命中函数；新增 `input_editors: [Editor; 3]` 与侧行号 → 对齐行索引（design 决定 3）；单击、拖选、Shift+单击、Ctrl+A 在输入窗格生效并以 `text_selection` 色绘制；`view_tests.rs` 用原始输入帧（先 2 帧预热）测试拖选两行得到正确选区
- [x] 3.2 输入窗格键盘：Ctrl+C 复制选中文本（保留原行尾）、方向键移动光标；字符 / Backspace / Delete / Enter / 粘贴 / IME 一律丢弃；测试：键入后内容与 `TextContent` 修订号不变，Ctrl+C 后剪贴板输出为所选两行
- [x] 3.3 输入窗格右键菜单 `input_context_items(pane)`：Take Left / Center / Right（所点行所在区段）、Copy（有选区才可用）、Select All；右键先定位所点行；测试：在 left 冲突行右键 Take Left 后未解决冲突数减一

## 4. 查找与替换

- [x] 4.1 接入 `FindPanel`：Search 菜单与快捷键 Ctrl+F / F3 / Shift+F3 / Ctrl+H；Find Next/Previous 在活动窗格内容上调用 `search::find_next`，命中设选区并 `goto_row`；无更多匹配写状态栏且不回绕；「仅在改动中」传改动 / 冲突区段的行区间；测试：right 窗格找到第 2 个匹配并定位、末尾再 F3 状态栏提示、仅在改动中跳过未改动行
- [x] 4.2 `MergeModel::replace_all`（在 `edit` 闭包内调用 `search::replace_all`）与单次 Replace；输入窗格活动时 Replace 禁用、查找条无替换行；`model_tests.rs` 测试：Replace All 3 处后各行 origin 为 Manual、一次 undo 全部还原、波及的冲突区段计为已解决；`cargo test -p bc-gui merge::` 通过

## 5. 横向滚动条

- [ ] 5.1 窗格区底部加共用横向滚动条（`chrome::scrollbar::show_horizontal_px`），内容放得下时隐藏并把 `scroll.x` 归零；纵向滚动条与状态栏位置相应上移；测试：注入长行后滚动条出现、拖到最右后 `scroll.x` 等于上限；放宽窗口后消失且偏移为 0

## 6. 集成检查与记录

- [ ] 6.1 重新生成 `merge-basic` / `merge-conflict` 快照并人工核对语法与行内着色、滚动条位置；`cargo test -p bc-gui -p bc-engine-text`、`cargo clippy -p bc-gui` 无新增失败或告警；`dev-build.ps1` 后用 `matcha base.rs theirs.rs yours.rs out.rs` 实机走一遍：看行内高亮、Ctrl+F 在 base 查找、输出 Replace All 后撤销、从 base 拖选复制、拖横向滚动条
- [ ] 6.2 `docs/features/text-merge.md` 状态改为已实现、从「尚未实现」删除这五项（保留输入窗格编辑、显示过滤等）；`CHANGELOGS.md` 顶部加条目；验收：文档与实际行为一致

## Workflow follow-up

- 评审通过后执行 `/opsx:archive text-merge-essentials`，把 delta 合并进 `openspec/specs/text-merge-view/spec.md`。
