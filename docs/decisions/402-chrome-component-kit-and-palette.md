# ADR 402：外框组件库（widgets / window / 下拉菜单）与可替换配色

状态：已实施（2026-11-15）。承接 [ADR 401](401-modern-design-system.md)。

## 问题

ADR 401 把 token、确认弹框（`chrome::modal`）、菜单/右键菜单（`chrome::popup`）、工具栏、固定布局
对话框控件（`chrome::dialog`）改成了现代风格，但其余界面仍直接用 egui 自带控件，与之格格不入：

- 工具栏下拉（Sessions / Format / Show Differences …）是 `egui::Area` 里竖排的 egui 按钮或
  `selectable_label`——带边框的按钮堆在一个弹层里，而不是菜单行（文本/表格/Hex/文件夹/同步五处）。
- 约 25 个浮动窗口用 `egui::Window` 默认外观：灰色标题栏、居中标题、折叠箭头。
- `ui.menu_button`、`egui::Popup`（路径历史、过滤器预设、换行符转换、同步方法）是 egui 默认弹层。
- 约 200 处 `ui.button` / `checkbox`（灰色方框）/ `radio` / `ComboBox` / `selectable_label` / 裸 `TextEdit`。
- 固定布局对话框（Session Settings / Options）仍有经典 Win32 方角标签页、像素箭头、黑色字形、
  白色方标题栏 + 红色 ×；文本查找栏是蓝色选区 + 方角按钮；Hex 对话框混用中文按钮文案。

同时颜色全部是 `pub const`，将来想换肤（深色/高对比）只能逐个改调用点。

## 决定

1. **流式布局组件库 `chrome::widgets`**：`Button`（`.primary()` / `.danger()` / `.selected()` /
   `.dropdown()` / `.large()`）、`Checkbox`、`RadioButton`、`ListRow`、`Combo`（列表即共享弹出菜单，
   `.flat()` 用于信息行内联）、`text_field` / `field`、`menu_button`、`dialog_buttons`、`section`、`hint`。
   API 与 egui 同名对照（见模块文档表格），实现 `egui::Widget`，遵守 `ui.is_enabled()`。
   视图代码**不再直接使用** egui 的按钮/勾选框/单选/下拉/可选标签/菜单按钮/Popup。
2. **浮动窗口 `chrome::window::Window`** 取代 `egui::Window`：与 `Modal` 同一张卡片
   （白底、发丝线、14 px 圆角、对话框阴影），44 px 标题区（加粗左对齐标题 + 幽灵 ×），默认居中，
   × 与 Esc 关闭（嵌入整套比较视图的浮窗用 `.close_on_escape(false)`）。非阻塞；确认类仍用 `Modal`。
   固定布局对话框的 `chrome::dialog::window` 外框同步改为这张卡片。
3. **下拉菜单统一走共享弹出菜单**：`popup` 新增通用的打开状态机（`open_menu` / `menu` /
   `menu_below` / `toggle_menu_below`，点在属主控件上不算点外部，可反复切换）与 `popup::item::*`
   构造器；右键菜单（`context_menu`）、组合框、菜单按钮、路径历史都基于它。工具栏下拉用
   `toolbar::dropdown_menu`（`ToolbarResponse.dropdown_rect` 作锚点，菜单贴在按钮下方）。
   路径栏的「历史」图标与内嵌箭头打开同一个历史菜单（此前图标误开打开文件对话框）。
4. **换肤预留**：`theme::Palette`（外框语义色全集）+ `theme::palette(ctx)`。`Theme::apply` 把配色装入
   egui 上下文并据此生成全局 visuals；`Theme::with_palette` 是换肤入口。新组件（widgets、window、
   popup、controls、dialog 的标签页/外框、查找栏、Options 导航）一律经 `palette(ctx)` 取色；
   `pub const` token 只是默认浅色方案 `Palette::LIGHT` 的取值。**本期不做换肤 UI**，也不强制迁移
   存量直接引用常量的绘制代码——将来实现换肤时，按需把剩余常量引用迁到 `palette(ctx)`。
   差异内容色（红/蓝差异、文件夹状态色）属内容语义，不在配色内。
5. 视觉回归新增 `--snapshot=widget-gallery`：一个 `Window` 内陈列全部 widgets，外加一个展开的下拉菜单。
6. 界面文案统一英文（Hex 的「确定/取消/上一个/下一个/文本/大小写/字节地址」改为 OK/Cancel/
   Previous/Next/Text/Match case/Byte address）。

## 后果

- 新代码规则：浮窗用 `Window`，确认用 `Modal`，控件用 `widgets`，菜单用 `popup`；
  仅 `DragValue`、`Slider`、多行 `TextEdit`、`ScrollArea`、`collapsing` 保留 egui 原生（全局 visuals 已按 token 着色）。
- 紧凑控件高 28 px（对话框底栏按钮 32 px，与 `Modal` 一致）；表格设置面板因此改为页面内滚动 + 固定底栏
  Apply，表格合并的工作表标签条由 24 px 增至 32 px。
- `dialog::TITLE_HEIGHT` 由 31 改为 44（与 `Window` 一致），固定布局对话框整体高 13 px。
- 文件夹「Recent」列表改为下拉菜单；File formats 面板的导入/导出改用系统文件对话框。
