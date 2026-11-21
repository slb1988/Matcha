# GUI 外壳与视图架构（GUI Shell）

> eframe/egui 桌面外壳：Home、文本比较/文本合并/文件夹/同步/表格视图、单实例与资源管理器集成。
> 状态：**已实现**（视图群落地；外框为 Matcha 现代设计系统，见 ADR 401；组件库与可替换配色见 ADR 402；分隔线可拖动并记忆）（2026-11-15 更新）。对应 FR-G1。

## 功能简介

- 唯一依赖 GUI 工具包的 crate：`bc-gui`（eframe 0.37，glow 后端）；引擎 crate 全部平台中性。
- Home：路径框、双文件/文件夹拖放、最近比较。会话树默认展开 New 与 Auto-saved；Auto-saved 下
  按本地日期分组（Today / Yesterday / N days ago / More than 6 days ago，仅 Today 默认展开），
  条目显示为会话类型图标 + `left <--> right`，双击按记录时的类型重开；
  悬停显示类型、两侧完整路径与最后打开时间（`Opened: yyyy-mm-dd hh:mm`）。
  - **记录时机**：任一标签页的会话两侧都已加载（或换了路径/类型）时记一条，覆盖所有入口——命令行、
    Home 拖放与双击、已保存会话、空会话在路径栏填好两侧、Compare Files Using、文件夹比较/同步互切；
    五种类型（Folder Compare / Folder Sync / Text / Table / Hex）都记。文件夹会话以真正加载的基准
    文件夹为准（路径栏输入中的文字不算），不存在的文件夹不记；截图与基准运行不记。
  - 同类型同路径只留一条（移到最前），最多 50 条；行数超出时会话树可用滚轮按整行滚动。
  - 存于 `%APPDATA%\Matcha\recent.txt`（v3：首行 `# matcha recent v3`，每行
    `纪元秒	类型	左	右`，类型为 `folder` / `sync` / `text` / `table` / `hex`）。
    该文件不存在时读入旧版 `recent-text.txt`（v2 `纪元秒	左	右`，或更早的两行一对格式，
    以文件修改时间为准），按路径推断类型后迁移写入 `recent.txt`；旧文件保留不动。
  有已保存会话时，New 与 Auto-saved 之间多一个 **Saved** 分组（命名会话，见 [saved-sessions.md](saved-sessions.md)）；
  已命名的标签页标题显示会话名。
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
- **右键菜单**（visual-parity 6.2）：文本窗格、表格网格、文件夹树三处的右键菜单按参考菜单
  （`docs/visual-parity/supplemental/*-context.tree.json`）排列条目、顺序与分隔线；已实现的条目复用菜单栏同名命令
  与启用规则，未实现的禁用；右键点在选区外先选中所点项（表格：格；文件夹：该行所点一侧）。同步视图的行保留短菜单。
- **Session Settings**（visual-parity 6.1）：文本、表格、文件夹/同步、Hex 统一为 `chrome::dialog` 框架——标题、
  下划线页签、页面卡片、底部 OK / Cancel（表格与文件夹另有 Apply）；均编辑草稿，OK/Apply 才生效，Cancel / × / Esc 丢弃。
  快照场景：`dlg-text-session-settings`、`dlg-table-session-settings`、`dlg-folder-session-settings`。
- **可拖动分隔线**：各视图窗格之间、文件夹/同步日志面板、合并视图输入/输出区、Home 会话树均可拖动调整，
  位置存 `%APPDATA%\Matcha\layout.txt`、重启保持，双击复位，见 [resizable-splitters.md](resizable-splitters.md)。
- **偏好持久化**：Tools › Options（Home 与各比较视图共用同一份 `Prefs`）的设置——差异颜色、编辑器字体与字号、
  默认 Tab 宽度、默认代码页、保存前备份及后缀、Tweaks 页的磁盘变化检测两个开关（`files.check-changed-on-disk` /
  `files.auto-reload`，见 [disk-change-reload.md](disk-change-reload.md)）、Next Difference 页的两个开关（`navigation.wrap-around`
  默认关 / `navigation.show-message-panel` 默认开，见 [text-compare.md](text-compare.md)「找不到差异时的消息面板」）——在 OK / Apply（以及 View › Text display 改字号）后写入
  `%APPDATA%\Matcha\settings.txt`（首行 `# matcha settings v1`，之后 `键	值`；颜色为 `#RRGGBB`；
  未识别的键原样保留），启动时读回；无效值忽略、用默认值。快照/基准模式不读不写。Explorer 右键菜单开关仍单独存于
  `shell-menu.txt`。实现：`crates/bc-gui/src/settings.rs`，帧末 `MatchaApp::save_prefs` 检测变化后保存。
- **右下角缩放把手**（`chrome::size_grip`）：有状态栏的视图（除 Home 外全部）在窗口右下角画 22×22 px 的把手，4 行斜向三角点阵（10 个 2×2 深灰点 + 白色高光，比原先 6 个浅灰点醒目）。悬停显示 ↘↖ 缩放光标并浮起圆角底色，按下点阵变为强调色并交给系统拖动缩放（`ViewportCommand::BeginResize(SouthEast)`，效果等同抓窗口右下角边框，但命中区远大于几像素的系统边框）。窗口最大化/全屏时隐藏。由 `app.rs` 在中央面板之后统一绘制于前景层，各视图不再各画装饰点。
- **sheet 标签**：`chrome::sheet_tabs` 下划线页签 + 状态圆点，表格比较（网格与状态栏之间）与表格合并共用。
- **换肤预留**：`theme::Palette` + `theme::palette(ctx)`；组件经它取色，`Theme::with_palette(..).apply(ctx)`
  即可整体换色（目前只有默认浅色 `Palette::LIGHT`，无换肤 UI）。
- 内容画布：表格空白区/行详情区与文本末行以下为灰色画布 + 点阵底纹，数据区白底；
  窗格上方文件信息行按文字实际宽度流式排列（不再用固定像素槽位）。
- Home：会话类型以卡片一行排开（窗口过窄时才分页，圆形箭头翻页）；侧栏扁平，选中为圆角胶囊。
- **应用图标**（抹茶叶）：一片抹茶叶，中间叶脉就是左右两栏的分隔线，两侧侧脉是被比较的“行”，
  逐对镜像对齐；只有右侧一条错位并标为橙色——Matcha 找出的差异。名字双关：*Match*·a（匹配）、
  抹茶 ≈ 抹差（抹平差异）。`matcha.exe`、`matcha-comp.exe`、`matcha-compc.exe`（及其副本 `matcha.com`）
  内嵌 `assets/icon/matcha.ico`（16–256 px，小尺寸减少侧脉、加粗线条单独绘制），资源管理器、快捷方式与
  任务栏显示该图标；窗口标题栏 / Alt-Tab 由 `ViewportBuilder::with_icon` 设置同源的 256 px PNG。
- 基准与截图：`--bench=SECONDS`、`--bench-dir`、`--screenshot=target/x.bmp`
  （应用自身 framebuffer，BMP 直写）。

## 代码地图

| 位置 | 内容 |
|---|---|
| `crates/bc-gui/src/app.rs` | View 切换、全局 Prefs、关闭保护；接线集中于 View/HomeAction/ViewAction |
| `crates/bc-gui/src/home.rs` | 主页与拖放 |
| `crates/bc-gui/src/chrome/drop.rs` | 比较视图的系统拖放落点 → 左/右侧判定（各视图共用） |
| `crates/bc-gui/src/settings.rs` | Options 偏好读写（`settings.txt`） |
| `crates/bc-gui/src/text/`、`crates/bc-gui/src/merge/`、`table/`、`dir_view.rs`、`sync_view.rs` | 各功能视图（文本合并为 `View::TextMerge`；见对应 feature 文档） |
| `crates/bc-gui/src/chrome/`、`dialogs/`、`legend.rs`、`log_panel.rs` | 外框、对话框、颜色图例、虚拟化操作日志 |
| `crates/bc-gui/src/chrome/size_grip.rs` | 右下角窗口缩放把手（命中区、点阵纹理、按下发起系统缩放） |
| `crates/bc-gui/src/chrome/theme.rs` | 设计 token（颜色/圆角/阴影/字号）与全局 egui 样式 |
| `crates/bc-gui/src/chrome/modal.rs`、`chrome/controls.rs` | 现代确认弹框；主/次/危险按钮与标签 chip |
| `crates/bc-gui/src/chrome/widgets.rs`、`chrome/window.rs` | 流式布局控件（Button/Checkbox/Radio/ListRow/Combo/text_field/menu_button/dialog_buttons）；浮动窗口 |
| `crates/bc-gui/src/chrome/popup.rs`、`chrome/toolbar.rs` | 共享弹出菜单与打开状态（`menu`/`menu_below`/`context_menu`、`item::*`）；工具栏下拉 `dropdown_menu` |
| `crates/bc-instance`、`crates/bc-shell` | 单实例交接、Explorer 菜单 |
| `tools/icon/gen_icon.py` → `assets/icon/` | 应用图标生成（`python -I tools/icon/gen_icon.py`）：`matcha.ico`、`matcha-256.png`、`matcha-1024.png` |
| `apps/bcompare/build.rs`、`apps/bcomp/build.rs`、`*/matcha.rc` | `embed-resource` 把 `matcha.ico` 编进 Windows exe（需 Windows SDK `rc.exe`；其他平台空操作） |
| `crates/bc-gui/src/lib.rs` `app_icon` | 窗口图标（`include_bytes!` 256 px PNG） |

## 关键语义

- **文件对的会话类型统一路由**（`bc_formats::route::pair_view`）：命令行两个文件、Home 打开/拖放、
  Auto 新会话、文件夹比较里打开文件对都调这一处，顺序固定 **表格 → 十六进制 → 文本**：
  表格 = xlsx/xlsm（扩展名对不上时按包内含 `xl/workbook.xml` 识别，覆盖版本控制临时文件名）
  或格式库里无外部转换的表格格式（`*.csv`、用户表格格式；格式库未匹配时 `*.tsv`/`*.tab`）；
  十六进制 = `*.bin` 或前 8 KiB 嗅探为二进制；其余文本。工作簿是 ZIP，必须先于二进制嗅探判定。
  需要外部转换的 `*.xls` 读不了，按内容落到十六进制。`/fv=` 仍可强制类型。
- **Session › Compare Files Using**（`crates/bc-gui/src/using.rs`，文本 / 表格 / 十六进制视图共用）：
  用另一类视图原位重开当前文件对。可用性只看文件（`bc_formats::route::can_open`），不再各视图写白名单：
  当前视图自身灰；两侧须为磁盘文件；Hex 总可用；Table 要求两侧是表格或文本内容；Text 要求两侧是文本内容
  （xlsx 不能用 Text，csv 可以）。Media / Picture / Version Compare 尚未实现，恒灰。
  文本视图有未保存编辑时拒绝切换并在状态栏提示。
- **线程模型**：live 编辑目标（含 Rc/RefCell）只在 UI 线程；worker 用快照，
  结果带修订号、过期丢弃（文本 350ms 防抖重对齐）。
- GUI 没有自己的 mmap/unsafe；只读锁与编码下沉到 `bc-core`/引擎，系统光标读取与原生对话框属主窗口在 `bc-shell`。
- **剪贴板**：eframe 必须启用 `clipboard` feature，否则 egui-winit 退化为进程内假剪贴板，
  Ctrl+C/V、`copy_text`、`RequestPaste` 都不经过系统剪贴板。
- **路径框首次进入默认全选**：左键点入或键盘/程序化聚焦路径框（`chrome::pathbar` 可编辑与只读变体、
  文件夹/同步视图路径框）时整段路径全选，对齐浏览器地址栏；再次点击或拖拽正常落点圈选。
  左键点入的全选延迟到「未拖拽的释放」帧（按下即拖是圈选手势，不能被全选覆盖锚点）；
  右键聚焦不触发（随后的左键落点不应被抢走），见 `pathbar::select_all_on_first_focus`。
  路径框文本选区用与正文窗格一致的经典选区蓝 #99C9EF（`TEXT_SELECTION`），选中文字保持正文色；
  主题全局的选区绿（`SELECTION`）只留给树/表格的行选中态。
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
- **Esc 分层语义**：弹层优先——菜单栏菜单、各类下拉/右键菜单、组合框列表、查找条、单元格
  编辑器、对话框与确认框各自消费 Esc（关闭/取消自身，帧内 `consume_key`）；页面无弹层时 Esc =
  关闭当前标签页（等价于 Ctrl+W：未保存页先弹保存确认，Cancel 则保留该页），多标签时一次只关一页，
  关掉最后一页即退出进程。
  实现在 `app.rs` 帧末：检查仍未被消费的 Esc 才走 `close_tab(active)`；快照模式屏蔽输入不受影响。
- 新代码只引用 `theme` 语义 token，不写颜色字面量；差异内容色（红/蓝）不属于外框，保持不变。
- 截图只来自自身 framebuffer，不读其他窗口。

## 使用与验证

```powershell
cargo run --release -p bcompare -- <left> <right>
cargo run --release -p bcompare -- <left> <right> --bench=60 --screenshot=target/text.bmp
cargo test -p bc-gui --lib
# 应用图标：改 tools/icon/gen_icon.py 后重新生成；exe 图标可用 [System.Drawing.Icon]::ExtractAssociatedIcon 提取核对
python -I tools/icon/gen_icon.py --preview   # 另写 tmp/icon-preview.png 各尺寸对照
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
