# GUI 外壳与视图架构（GUI Shell）

> eframe/egui 桌面外壳：Home、文本/文件夹/同步/表格视图、单实例与资源管理器集成。
> 状态：**已实现**（视图群落地；视觉对齐 visual-parity 进行中 15/23）。对应 FR-G1。

## 功能简介

- 唯一依赖 GUI 工具包的 crate：`bc-gui`（eframe 0.37，glow 后端）；引擎 crate 全部平台中性。
- Home：路径框、双文件/文件夹拖放、最近文件对。会话树默认展开 New 与 Auto-saved；Auto-saved 下
  按本地日期分组（Today / Yesterday / N days ago / More than 6 days ago，仅 Today 默认展开），
  条目显示为 `left <--> right`。最近记录存于 `%APPDATA%\Matchaecent-text.txt`（v2：首行
  `# matcha recent v2`，每行 `纪元秒	左	右`；旧的两行一对格式以文件修改时间读入）。
- 比较视图（文本/表格/Hex/文件夹/同步）：把文件从资源管理器拖到哪一侧就替换哪一侧（按落点判定，
  与当前活动侧无关）。
- 视图：`text/`（文本比较）、`dir_view.rs`（文件夹）、`sync_view.rs`（同步）、
  `table/`（表格）；两个目录参数自动进入文件夹视图。
- 单实例（`bc-instance`）与 Windows 资源管理器菜单（`bc-shell`）见各自 crate。
- 基准与截图：`--bench=SECONDS`、`--bench-dir`、`--screenshot=target/x.bmp`
  （应用自身 framebuffer，BMP 直写）。

## 代码地图

| 位置 | 内容 |
|---|---|
| `crates/bc-gui/src/app.rs` | View 切换、全局 Prefs、关闭保护；接线集中于 View/HomeAction/ViewAction |
| `crates/bc-gui/src/home.rs` | 主页与拖放 |
| `crates/bc-gui/src/chrome/drop.rs` | 比较视图的系统拖放落点 → 左/右侧判定（各视图共用） |
| `crates/bc-gui/src/text/`、`table/`、`dir_view.rs`、`sync_view.rs` | 各功能视图（见对应 feature 文档） |
| `crates/bc-gui/src/chrome/`、`dialogs/`、`legend.rs`、`log_panel.rs` | 外框、对话框、颜色图例、虚拟化操作日志 |
| `crates/bc-instance`、`crates/bc-shell` | 单实例交接、Explorer 菜单 |

## 关键语义

- **线程模型**：live 编辑目标（含 Rc/RefCell）只在 UI 线程；worker 用快照，
  结果带修订号、过期丢弃（文本 350ms 防抖重对齐）。
- GUI 没有自己的 mmap/unsafe；只读锁与编码下沉到 `bc-core`/引擎，系统光标读取在 `bc-shell`。
- **剪贴板**：eframe 必须启用 `clipboard` feature，否则 egui-winit 退化为进程内假剪贴板，
  Ctrl+C/V、`copy_text`、`RequestPaste` 都不经过系统剪贴板。
- **路径框粘贴**：所有路径编辑框（`chrome::pathbar` 与文件夹/同步视图路径框）粘贴时去掉
  首尾空白及成对包住整段的双引号（Explorer「复制为路径」的格式）；Ctrl+V 与右键 Paste 同一规则，
  见 `pathbar::clean_pasted_path`。
- **系统文件拖放落点**（`chrome::drop`）：winit 投递拖放事件时不带坐标，且 Windows OLE 拖动期间
  窗口收不到鼠标移动，egui 指针位置在落下时是旧值或空（曾因此回落到活动侧：拖到右栏却换了左栏）。
  落点改取系统光标（`bc_shell::cursor_screen_pos` → `GetCursorPos`，按视口 inner rect 与
  pixels-per-point 换算到 egui 坐标；其他平台/测试回落 egui 指针）。判定取**最近的目标矩形**所属侧，
  所以拖到某侧路径栏、列头、状态条也算该侧；多文件拖入只取第一个。
  - 文本/表格：走与路径栏相同的 `Pending::Open`（该侧有未保存修改先确认）；确认框/对话框/单元格
    编辑器/表格设置打开时忽略拖放。
  - Hex：窗格、滚动条与地址栏都是目标（over-under 时两条地址栏都在上窗格之上）；该侧有未保存
    编辑时拒绝并提示，不静默丢弃；模态对话框打开时忽略。
  - 文件夹/同步：窗口按中缝分左右两半，只接受文件夹（文件给出提示）；落在浮动的内嵌文本窗口或
    子文件夹窗口上的拖放交给那个窗口自己的视图，其余由文件夹视图消费，避免内嵌视图误换侧。
- CJK 用系统微软雅黑后备，不捆绑字体。
- 截图只来自自身 framebuffer，不读其他窗口。

## 使用与验证

```powershell
cargo run --release -p bcompare -- <left> <right>
cargo run --release -p bcompare -- <left> <right> --bench=60 --screenshot=target/text.bmp
cargo test -p bc-gui --lib
```

## 深入阅读

- `crates/bc-gui/README.md`（结构清单最权威）
- `docs/ui-framework-decision.md`（egui 选型测量与结论）
- `docs/visual-parity/`（视觉对齐 tokens 与 HANDOFF）、`docs/reference-parity/visual.md`
- 单实例交接协议见 README"Open questions"与 `crates/bc-instance`
