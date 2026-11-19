# 文本书签（Text Bookmarks）

> 文本比较与文本合并视图中的 0–9 编号书签：在长文件的几处位置之间快速往返。
> 状态：**进行中**（文档先行；数据模型、文本比较 / 合并接入、Ctrl+0 解占用待实现）（2026-11-19 更新）。

## 功能简介

- 10 个编号书签（0–9），每个视图（标签页）各自一套：
  - **Toggle Bookmark N**（`Shift+Ctrl+N`，Search › Toggle Bookmark ▸ Bookmark N）：在活动窗格的光标行放置书签 N；
    该行已是书签 N 时移除；N 原在别处时从原处移过来；该行已有另一个编号 M 时用 N 取代 M（一行最多一个书签）。
  - **Go To Bookmark N**（`Ctrl+N`，Search › Go to Bookmark ▸ Bookmark N）：激活书签所在窗格，光标到该行行首，
    各窗格滚到对齐位置使该行可见；该行被显示过滤（Show Differences / Show Same / 折叠）隐藏时先恢复为可见。
    书签 N 未设置时状态栏提示 `Bookmark N is not set`，光标不动。
  - **Clear Bookmarks**（Search 菜单）：移除本视图的全部书签；没有书签时禁用。
- 快捷键按物理数字键识别（Shift+数字的逻辑键是符号）；同时按着 Alt 时（AltGr = Ctrl+Alt，德语等布局用它输入 `{ [ ] }`）不触发书签。
- 菜单状态：Toggle 子项恒可用；Go to 子项只有已设置的编号可用；Clear Bookmarks 仅有书签时可用。
- 显示：书签行在所属窗格**文本区右缘**（纵向滚动条左侧）画一个强调色圆角徽标，内写编号；不随横向滚动移动，
  不占行号栏，所以不会遮挡行号栏的复制箭头及其点击区。文本比较的缩略导航条在书签所在行画一道强调色短刻度
  （画在该侧的那一列）。
- 书签只在视图打开期间有效：不写入命名会话 / 工作区，关闭再重开没有书签。

## 代码地图

| 层 | 位置 | 内容 |
|---|---|---|
| 数据模型 | `crates/bc-gui/src/text/bookmarks.rs` | `Bookmarks<A>`（toggle / get / clear / at / is_empty，锚点类型泛型）、`shift`（编辑前后行表比对的平移规则）、`Tracker`（按修订号驱动的每侧基准） |
| 文本比较 | `crates/bc-gui/src/text/view.rs` | 书签状态、`command()` 中 `Toggle Bookmark N` / `Go to Bookmark N` / `Clear Bookmarks`、`update_menus` 启用规则、键盘映射、换文件 / 重载 / 交换处理 |
| 文本比较菜单 | `crates/bc-gui/src/text/menu.rs` | Toggle / Go to 子菜单子项的 action_id 区分（标签仍为 `Bookmark N`） |
| 绘制 | `crates/bc-gui/src/text/render.rs`、`nav_bar.rs` | 文本区右缘编号徽标（`render::bookmark_badge`）、缩略导航条刻度 |
| 文本合并 | `crates/bc-gui/src/merge/view.rs` | Search 菜单书签项与快捷键、锚点 `(pane, row)`、徽标 |
| 应用 | `crates/bc-gui/src/app.rs` | 启动时关闭 egui 键盘缩放（`zoom_with_keyboard = false`），释放 Ctrl+0 |
| 测试 | `text/bookmarks.rs`（单测）、`text/view_tests.rs`、`merge/view_tests.rs`、`app_tests.rs` | 规则、菜单、跳转、编辑跟随、重载 / 换文件 / 交换、合并锚点、Ctrl+0 |

## 关键语义

- **锚点**：文本比较中书签挂在某一侧的**行号**上 `(side, line)`；文本合并中挂在**对齐行**上并记住放置时的活动窗格
  `(pane, row)`（输出窗格取光标所在输出行所属的对齐行）。
- **文本比较编辑跟随**（只在书签所在侧编辑时）：
  - 每帧比较该侧内容修订号；变化时对比编辑前后的行表：行数不变则不平移（Replace All、换行符转换等原位修改）；
    行数变化时求相同前缀 `p` 与相同后缀 `s`，变化区间之前的书签不动，之后的按行数增量平移，
    落在区间内的：新区间非空则保持相对区间起点的偏移（钳制到新区间末行），新区间为空（纯删除）则落到删除位置 `p`；
    最后钳制到最后一行。
  - 结果：上方插入 / 删除行时书签随所在行移动；书签所在行被删除时落到删除位置处的行（越过末行则落在末行）；
    撤销 / 重做按同一规则；另一侧的编辑不改变本侧书签。
- **重载**（Reload Files、磁盘变化重载，同一路径）：书签行号保留，超出新行数的收敛到末行。
- **换文件**（Open File 打开另一个文件、Open Clipboard、拖放）：清除该侧全部书签，另一侧保留。
- **Swap Sides**：书签随内容换到另一侧，行号不变。
- **文本合并**：输入窗格只读，对齐行编号从不改变；输出编辑与 Take 不改变书签的对齐行。Go to 激活锚点窗格并把四个窗格
  滚到该对齐行（光标放到该对齐行的第一行输出；锚点为输入窗格时该窗格的光标也落到该行）。
- **Ctrl+0**：egui 默认用 Ctrl+= / Ctrl+− 整体缩放界面、Ctrl+0 复位缩放（egui 0.37 在每帧末尾处理），
  Ctrl+0 会同时复位缩放与跳到书签 0。Matcha 启动时关闭该功能（`MatchaApp::configure_input`），Ctrl+0 / Shift+Ctrl+0
  只用于书签 0，界面缩放恒为 1；副作用是 Ctrl+= / Ctrl+− 整体界面缩放随之关闭（从未作为功能记录；
  编辑器字号仍在 Tools › Options 与 View › Text display 中调整）。现有 28 个快照场景前后逐像素相同。
- 不做：书签持久化、十六进制 / 表格视图书签、无编号书签、书签列表、书签间 Next / Previous。

## 使用与验证

- 单元测试：`cargo test -p bc-gui bookmarks`（放置 / 移除 / 移动 / 同行取代 / 清除；平移规则各情形）。
- 视图测试：`cargo test -p bc-gui text::view::tests::bookmark`、`cargo test -p bc-gui merge::`、
  `cargo test -p bc-gui app_tests`（Ctrl+0 不缩放且跳到书签 0）。
- 手动：`matcha a.txt b.txt`，Shift+Ctrl+1/2/3 在两侧放书签，Ctrl+1/2/3 跳转；在书签上方插行后再跳转仍落在原文字；
  Swap Sides、Clear Bookmarks；合并视图放书签后 Take Right 再跳转。

## 深入阅读

- 变更过程与设计决定：`openspec/changes/text-bookmarks/`（design.md 决定 1–6）
- 文本比较总览：[text-compare.md](text-compare.md)；文本合并：[text-merge.md](text-merge.md)
- 参考帮助（书签命令）：本地 CHM 解包 `tmp/hex-ref/chm`
