# GUI 外壳与视图架构（GUI Shell）

> eframe/egui 桌面外壳：Home、文本比较/文本合并/文件夹/同步/表格视图、单实例与资源管理器集成。
> 状态：**已实现**（视图群落地；外框为 Matcha 现代设计系统，见 ADR 401；组件库与可替换配色见 ADR 402）（2026-11-15 更新）。对应 FR-G1。

## 功能简介

- 唯一依赖 GUI 工具包的 crate：`bc-gui`（eframe 0.37，glow 后端）；引擎 crate 全部平台中性。
- Home：路径框、双文件/文件夹拖放、最近文件对。会话树默认展开 New 与 Auto-saved；Auto-saved 下
  按本地日期分组（Today / Yesterday / N days ago / More than 6 days ago，仅 Today 默认展开），
  条目显示为 `left <--> right`。最近记录存于 `%APPDATA%\Matcha\recent-text.txt`（v2：首行
  `# matcha recent v2`，每行 `纪元秒	左	右`；旧的两行一对格式以文件修改时间读入）。
  记录与读入时统一还原 Windows `canonicalize` 产生的 `\\?\` verbatim 前缀，地址栏只见常规路径。
- 比较视图（文本/表格/Hex/文件夹/同步）：把文件从资源管理器拖到哪一侧就替换哪一侧（按落点判定，
  与当前活动侧无关）。
- 视图：`text/`（文本比较）、`merge/`（文本合并，`View::TextMerge`）、`dir_view.rs`（文件夹）、
  `sync_view.rs`（同步）、`table/`（表格）；两个目录参数自动进入文件夹视图。
  文本合并为上排三个只读输入窗格、底部可编辑输出窗格，共用行网格与滚动位置，见 [text-merge.md](text-merge.md)。
- 单实例（`bc-instance`）与 Windows 资源管理器菜单（`bc-shell`）见各自 crate。
- **视觉设计系统**（ADR 401）：冷中性灰表面 + 发丝线 + 圆角 + 柔和阴影，品牌强调色"抹茶绿"；
  token 集中在 `chrome/theme.rs`。确认类弹框（文本/表格/Hex 未保存、Hex 重载、文件操作确认、
  About）统一用 `chrome/modal.rs`：遮罩卡片、语气徽标、条目列表、Enter=主按钮 / Esc=取消。
- **外框组件库**（ADR 402）：视图不直接用 egui 自带控件。浮动窗口 = `chrome::window::Window`
  （与 Modal 同款卡片、加粗左对齐标题、× / Esc 关闭、默认居中）；按钮/勾选框/单选/列表行/组合框/
  文本框/菜单按钮/对话框底栏 = `chrome::widgets`；所有下拉（工具栏下拉、组合框列表、路径历史、
  菜单按钮、右键菜单）= `chrome::popup` 共享菜单（勾选/单选标记当前值，点外部/Esc 关闭，
  再点属主按钮收起）。工具栏下拉贴在按钮正下方（`toolbar::dropdown_menu`）。
  路径栏的历史图标与内嵌箭头打开同一个历史菜单。仅 DragValue/Slider/多行 TextEdit/ScrollArea 保留 egui 原生。
- **换肤预留**：`theme::Palette` + `theme::palette(ctx)`；组件经它取色，`Theme::with_palette(..).apply(ctx)`
  即可整体换色（目前只有默认浅色 `Palette::LIGHT`，无换肤 UI）。
- 内容画布：表格空白区/行详情区与文本末行以下为灰色画布 + 点阵底纹，数据区白底；
  窗格上方文件信息行按文字实际宽度流式排列（不再用固定像素槽位）。
- Home：会话类型以卡片一行排开（窗口过窄时才分页，圆形箭头翻页）；侧栏扁平，选中为圆角胶囊。
- 基准与截图：`--bench=SECONDS`、`--bench-dir`、`--screenshot=target/x.bmp`
  （应用自身 framebuffer，BMP 直写）。

## 代码地图

| 位置 | 内容 |
|---|---|
| `crates/bc-gui/src/app.rs` | View 切换、全局 Prefs、关闭保护；接线集中于 View/HomeAction/ViewAction |
| `crates/bc-gui/src/home.rs` | 主页与拖放 |
| `crates/bc-gui/src/chrome/drop.rs` | 比较视图的系统拖放落点 → 左/右侧判定（各视图共用） |
| `crates/bc-gui/src/text/`、`crates/bc-gui/src/merge/`、`table/`、`dir_view.rs`、`sync_view.rs` | 各功能视图（文本合并为 `View::TextMerge`；见对应 feature 文档） |
| `crates/bc-gui/src/chrome/`、`dialogs/`、`legend.rs`、`log_panel.rs` | 外框、对话框、颜色图例、虚拟化操作日志 |
| `crates/bc-gui/src/chrome/theme.rs` | 设计 token（颜色/圆角/阴影/字号）与全局 egui 样式 |
| `crates/bc-gui/src/chrome/modal.rs`、`chrome/controls.rs` | 现代确认弹框；主/次/危险按钮与标签 chip |
| `crates/bc-gui/src/chrome/widgets.rs`、`chrome/window.rs` | 流式布局控件（Button/Checkbox/Radio/ListRow/Combo/text_field/menu_button/dialog_buttons）；浮动窗口 |
| `crates/bc-gui/src/chrome/popup.rs`、`chrome/toolbar.rs` | 共享弹出菜单与打开状态（`menu`/`menu_below`/`context_menu`、`item::*`）；工具栏下拉 `dropdown_menu` |
| `crates/bc-instance`、`crates/bc-shell` | 单实例交接、Explorer 菜单 |

## 关键语义

- **线程模型**：live 编辑目标（含 Rc/RefCell）只在 UI 线程；worker 用快照，
  结果带修订号、过期丢弃（文本 350ms 防抖重对齐）。
- GUI 没有自己的 mmap/unsafe；只读锁与编码下沉到 `bc-core`/引擎，系统光标读取与原生对话框属主窗口在 `bc-shell`。
- **剪贴板**：eframe 必须启用 `clipboard` feature，否则 egui-winit 退化为进程内假剪贴板，
  Ctrl+C/V、`copy_text`、`RequestPaste` 都不经过系统剪贴板。
- **路径框粘贴**：所有路径编辑框（`chrome::pathbar` 与文件夹/同步视图路径框）粘贴时去掉
  首尾空白及成对包住整段的双引号（Explorer「复制为路径」的格式）；Ctrl+V 与右键 Paste 同一规则，
  见 `pathbar::clean_pasted_path`。
- **路径框右键菜单**：选区内右击保留选区（Cut/Copy/Delete 可用）——egui 在右键按下帧就折叠
  选区，故恢复发生在按下帧而非 `secondary_clicked` 的释放帧；点在选区外则按系统惯例收拢选区。
  合并视图标题条用只读变体 `pathbar::show_readonly`（同一套选区快照机制，菜单只有 Copy / Select All）。
- **路径框失焦保持水平滚动位置**：egui 的单行 TextEdit 失焦即把内部滚动偏移归零（私有字段无法
  直写），`pathbar::preserve_view_offset` 在持焦帧反推并记录实际偏移、失焦帧按记录值抹掉重画文本，
  长路径移到末尾看文件名后切换焦点/路径不再跳回开头；选区与光标不保留，点击重新聚焦的当帧回开头、
  恢复正常落点映射。可编辑框与只读框（含合并视图标题条）统一生效。
- **系统原生文件对话框**（`chrome::native_dialog`，`rfd`；Windows 为资源管理器同款 IFileOpenDialog /
  IFileSaveDialog / 选择文件夹）：路径栏浏览按钮、右键 Browse（Alt+B）、Open File... / Save File As... /
  Save target... / Export as CSV... / 报告导出，以及文件夹比较的 Browse for Folder 全部走它，
  不再有应用内路径输入窗。标题统一 `<动作> - Left Side / Right Side`（如
  `Open File on File System - Left Side`），起始目录与预填文件名取该侧当前路径（不存在则退到最近的
  存在上级）；文件过滤器末项恒为 `All files (*.*)`。对话框在帧内同步弹出，属主为本进程前台窗口
  （`bc_shell::dialog_owner`），故对主窗口模态。测试构建不弹窗：`native_dialog::mock` 预置返回值并记录请求。
- **系统文件拖放落点**（`chrome::drop`）：winit 投递拖放事件时不带坐标，且系统拖动期间窗口
  收不到鼠标移动（Windows OLE 拖动、macOS 拖拽会话皆然），egui 指针位置在落下时是旧值或空
  （曾因此回落到活动侧：拖到右栏却换了左栏）。落点改取系统光标（`bc_shell::cursor_screen_pos`
  → Windows `GetCursorPos` / macOS `CGEventCreate`，按视口 inner rect 与 pixels-per-point 换算到
  egui 坐标；其他平台/测试回落 egui 指针）。判定取**最近的目标矩形**所属侧，
  所以拖到某侧路径栏、列头、状态条也算该侧；多文件拖入：Hex 成对设为左右，其余视图取第一个。
  - 文本/表格：走与路径栏相同的 `Pending::Open`（该侧有未保存修改先确认）；确认框/对话框/单元格
    编辑器/表格设置打开时忽略拖放。
  - Hex：窗格、滚动条与地址栏都是目标（over-under 时两条地址栏都在上窗格之上）；单文件落哪侧换
    哪侧，同时拖入两个文件成对设为左右（原子：任一侧有未保存编辑或加载失败整体拒绝）；有未保存
    编辑时拒绝并提示，不静默丢弃；模态对话框打开时忽略。
  - 文件夹/同步：窗口按中缝分左右两半，只接受文件夹（文件给出提示）；落在浮动的内嵌文本窗口或
    子文件夹窗口上的拖放交给那个窗口自己的视图，其余由文件夹视图消费，避免内嵌视图误换侧。
- CJK 用系统微软雅黑后备，不捆绑字体。
- **未保存确认**：按钮为「Don't Save（左，危险）/ Cancel / Save（右，主）」；标题随触发动作变化
  （closing / quitting / reloading / opening another file …），列出受影响的一侧及文件名与完整路径。
  Enter = Save，Esc = Cancel；点遮罩不关闭（避免误触丢弃）。
- 新代码只引用 `theme` 语义 token，不写颜色字面量；差异内容色（红/蓝）不属于外框，保持不变。
- 截图只来自自身 framebuffer，不读其他窗口。

## 使用与验证

```powershell
cargo run --release -p bcompare -- <left> <right>
cargo run --release -p bcompare -- <left> <right> --bench=60 --screenshot=target/text.bmp
cargo test -p bc-gui --lib
# 视觉检查（快照模式会屏蔽鼠标键盘输入，仅用于截图）
target/debug/matcha.exe --snapshot=text-unsaved-modal --snapshot-out=target/visual/modal.png
# 组件库陈列（Window + 全部 widgets + 展开的下拉菜单）
target/debug/matcha.exe --snapshot=widget-gallery --snapshot-out=target/visual/gallery.png
```

## 深入阅读

- `crates/bc-gui/README.md`（结构清单最权威）
- `docs/ui-framework-decision.md`（egui 选型测量与结论）
- `docs/decisions/401-modern-design-system.md`（为何离开像素对齐、token 与弹框规范）
- `docs/decisions/402-chrome-component-kit-and-palette.md`（组件库、下拉菜单统一、换肤预留）
- `docs/visual-parity/`（历史：经典外观对齐 tokens 与 HANDOFF）、`docs/reference-parity/visual.md`
- 单实例交接协议见 README"Open questions"与 `crates/bc-instance`
