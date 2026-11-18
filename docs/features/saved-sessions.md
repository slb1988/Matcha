# 命名会话（Saved Sessions）

> 把当前比较（类型 + 两侧路径 + 部分会话设置）存成有名字的会话，之后从 Home 或命令行一键重开。
> 状态：**已实现**（文本 / 表格 / 十六进制 / 文件夹比较 / 文件夹同步；单元测试 + app 流程测试）（2026-11-18 更新）。
> 对应 FR-X1。

## 功能简介

- **保存**：文本、表格、十六进制、文件夹比较与文件夹同步视图的 Session 菜单里，
  **Save Session**（Shift+Ctrl+S）与 **Save Session As...** 可用；文件夹视图工具栏 Sessions 下拉的 Save Session 同样可用。
  - 标签页尚未命名时两者都弹 **Save Session As** 对话框：名称默认为标签标题（`左 <--> 右`），
    同名已存在时提示"将被替换"；Enter 或 Save 确认，Esc / Cancel 取消。
  - 已命名的标签页按 Save Session 直接覆盖，不再询问；Save Session As 可另存为新名称，标签页随之改名。
  - 保存后标签标题与窗口标题显示会话名（`<会话名> - Text Compare - Matcha`）。
  - 文件夹视图内嵌文本窗口里的 Save Session 保存所在的文件夹会话。
- **保存的内容**：会话类型、左右路径、修改时间；文本比较另存对齐规则（Session Settings › Alignment 的
  算法 / Never align differences / Skew tolerance / Use closeness，写成参考键名）与 View › Show Context。
  其余会话设置（重要性、替换、表格/文件夹规则等）暂不保存，重开时用默认值。
- **打开**：
  - Home 会话树在 New 与 Auto-saved 之间显示 **Saved** 分组：命名文件夹（抽屉图标）与其下的会话，
    双击会话在新标签页打开；会话树过滤框同时按名称与两侧路径过滤（过滤时平铺匹配的会话）。
  - 命令行 `matcha "<会话名>"`（参数既不存在、也不像路径时按会话名处理）：打开该会话；
    已有常驻实例时转交给它开新标签页。找不到会话时打开 Home 并显示错误。
  - 文件对会话打开时同时记入 Auto-saved 最近比较。
- **删除**：在 Home 选中已保存会话，点会话工具栏的 **−**，在确认框中点 **Delete**
  （Esc / Cancel 取消；Enter 不会删除）。只删会话记录，不动被比较的文件。
- 不支持打开的类型（文本合并、图片、媒体等，或缺一侧/非本地路径）给出错误而不崩溃；
  合并视图的 Save Session 仍为禁用。

## 代码地图

| 层 | 位置 | 内容 |
|---|---|---|
| 会话库 | `crates/bc-session`（`folder.rs`、`xml.rs`） | `BCSessions.xml` 读写、`SessionManager`（见 [sessions-and-formats.md](sessions-and-formats.md)） |
| 视图 ↔ 会话 | `crates/bc-gui/src/sessions.rs` | 会话库路径、`store`/`find`/`remove`、`from_view`/`open_view`、Home 树条目 |
| 起名对话框 | `crates/bc-gui/src/dialogs/save_session.rs` | Save Session As |
| 应用接线 | `crates/bc-gui/src/app.rs` | `Tab::session`、`save_session`/`finish_save`、Shift+Ctrl+S、`saved_view`、启动与转交 |
| 视图菜单 | `text/view.rs`、`table/chrome_ui.rs`、`hex/menu.rs`+`hex/view.rs`、`dir_view.rs`、`sync_view.rs` | 启用菜单项并上报 `SaveSession { as_new }` |
| Home | `crates/bc-gui/src/home.rs` | Saved 分组、双击打开、"−" 删除确认 |
| 命令行 | `apps/bcompare/src/routing.rs`（`saved_session`）、`main.rs` | `LaunchOptions::session`、`RemoteOpen::session` |

## 关键语义

- 会话库文件 `%APPDATA%\Matcha\BCSessions.xml`，与 `settings.txt`、`recent-text.txt` 同目录；格式与参考工具兼容。
- 名称在整个会话库中唯一：同名保存替换原会话，并沿用原会话中本项目不建模的内容（规则其余键、State 其余键、
  Specs 其余子元素）；原会话所在的命名文件夹保持不变，新会话放在根下。
- Home 不列无名文件夹（参考工具内部的自动保存区）与无名的按类型默认会话。
- 快照 / 基准模式不读不写会话库（与偏好、分隔线位置一致）；单元测试每个线程使用独立的临时会话库。

## 使用与验证

```powershell
cargo test -p bc-session
cargo test -p bc-gui --lib -- sessions save_session hex_and_folder home_lists
cargo test -p bcompare -- saved_session a_bare_name
```

手工：打开任意文本比较 → Session › Save Session As... → 回到 Home 看到 Saved › 会话名 → 双击重开；
命令行 `target\debug\matcha.exe "<会话名>"`。

## 深入阅读

- [sessions-and-formats.md](sessions-and-formats.md)（会话文件格式与兼容规则）、`crates/bc-session/README.md`
- [gui-shell.md](gui-shell.md)（标签页、Home）
