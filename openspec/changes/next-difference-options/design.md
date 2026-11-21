# Design

## Context

- 文本比较已有 `chrome::message_panel::MessagePanel`（`bd5b2f0`）：浅黄面板、48 px 高，首次绘制时调用 `bc_script::platform::beep()`（`cfg!(test)` 下不响）。文本视图自己持有 `notice: Option<MessagePanel>`，每帧在处理键盘之前检查关闭条件，主体区域高度扣掉面板高度。
- 各视图找不到差异的位置：

  | 视图 | 入口 | 现状 |
  |---|---|---|
  | 文本 | `text/view.rs::navigate` | 已弹面板 |
  | 表格 | `table/commands.rs::navigate(forward, kind)` | 状态栏 "No more differences / edits" |
  | 十六进制 | `hex/view.rs::next_difference` | 状态栏中文提示 |
  | 文本合并 | `merge/view.rs` 中的 "Next/Previous Change Section" 命令 | 状态栏提示 |
  | 表格合并 | `table_merge/view.rs` 中的同名命令 | 状态栏提示 |
  | 文件夹 | `dir_view.rs::navigate`（差异，UI 线程）；`Command::FocusConflict`（冲突，后台线程经 `Notice::Message` 回传） | 状态栏提示 |

- 偏好走 `bc_session::prefs::Prefs`，由 `bc-gui/src/settings.rs` 以 `key=value` 行读写 `settings.txt`。Options 对话框编辑草稿，OK/Apply 时写回。
- `app.rs` 调用视图的方式不统一：文本、表格、文本合并、表格合并的 `ui(ui, prefs)` 带 `&mut Prefs`；十六进制和文件夹（含同步）的 `ui(ui)` 不带。
- 文件夹比较里浮动打开的文本视图用的是 `DirCompareView` 自带的 `text_prefs: Prefs::default()`，不是应用偏好。这是既有缺口。

## Goals / Non-Goals

**Goals:**
- 六类视图共用同一套"找不到"处理：显示、提示音、关闭、让出高度。逻辑只写一份，避免各视图各写一遍关闭条件。
- 偏好到达每个执行差异导航的地方，包括文件夹比较里浮动打开的文本视图和十六进制视图。

**Non-Goals:**
- 不修复 `text_prefs` 其余字段（颜色、字体等）不跟随应用偏好的问题，只同步本次新增的 `next_diff`。
- 不把面板做成通用通知系统（错误、保存成功等仍走状态栏）。

## Decisions

### D1：在 `message_panel.rs` 增加 `NotFoundPanel` 状态封装

```rust
#[derive(Default)]
pub struct NotFoundPanel(Option<MessagePanel>);
impl NotFoundPanel {
    pub fn begin_frame(&mut self, ctx: &egui::Context); // 有按键/文字/鼠标按下 → 关闭
    pub fn not_found(&mut self, prefs: NextDiffPrefs);   // message_panel 开 → 显示（重置 beep）
    pub fn height(&self) -> f32;                         // 0 或 HEIGHT
    pub fn paint(&mut self, ui: &egui::Ui, rect: Rect);  // 无面板时什么也不做
    pub fn is_shown(&self) -> bool;
}
```

各视图持有一个 `NotFoundPanel` 字段，替换文本视图现在的 `notice`。文案常量 `NOT_FOUND = "Difference not found."` 放在同一模块。

- 备选方案：放到应用级（`app.rs` 统一在状态栏上方画）。否决原因：文件夹比较的浮动窗口有自己的状态栏，应用级画不到；而且各视图的状态栏矩形是自己算的。

### D2：回绕在各视图内部实现，形式统一为"找不到就从另一端再找一次"

```text
target = find(from current)
if target.is_none() && prefs.wrap_around { target = find(from edge) }   // edge = 起点之前/终点之后
match target { Some(t) => goto(t), None => panel.not_found(prefs) }
```

各视图的"从另一端找"：
- 文本：`next_difference(row)` 的起点换成"第 0 行之前"，向上时换成 `row_count`。复用现有 First/Last Difference 的查找。
- 表格：`next_row` 与单元格扫描的起点换成 `Cursor { row: 0, column: 0 }` 之前或末尾。kind 为编辑时用 `edited_rows`。
- 十六进制：`summary.next_difference` / `prev_difference` 从行首或行尾开始。
- 合并：`model.next_change` 从首行之前或末行之后开始。
- 文件夹差异：`indices` 从 `0` 或 `row_count()` 开始。
- 文件夹冲突：`Command::FocusConflict` 增加 `wrap: bool`；后台线程找不到时，以 `current: None` 再找一次。找不到时回传新的 `Notice::NotFound`（不再是 `Notice::Message("No further conflict")`），UI 线程处理通知时调用 `panel.not_found(prefs)`。

- 备选方案：抽一个通用的 `fn wrap_search<T>(…)`。否决原因：六处的"位置"类型各不相同（行、单元格、字节、节点），泛型后反而难读；上面三行的模式足够短，重复可以接受。

### D3：偏好传递

- `Prefs` 新增 `next_diff: NextDiffPrefs { message_panel: bool, wrap_around: bool }`，实现 `Copy`，默认值为 `{ true, false }`。
- 带 `&mut Prefs` 的视图直接读 `prefs.next_diff`。命令分发函数如果没有 prefs 参数（如 `hex::command(id)`、`merge::command(id)`），就在视图里缓存一份 `next_diff`，每帧 `ui()` 开头更新。
- 十六进制、文件夹（含同步）的 `ui()` 不带 prefs：新增 `set_next_diff(NextDiffPrefs)`，`app.rs` 在调用 `ui()` 前设置。文件夹视图再把它写进 `text_prefs.next_diff`，并转给浮动打开的十六进制视图。
- 备选方案：把所有 `ui()` 签名统一成带 `&mut Prefs`。否决原因：改动面大，涉及测试辅助函数和快照/基准入口，与本变更无关。

### D4：持久化键

`settings.txt` 新增 `navigation.show-message-panel` 和 `navigation.wrap-around`（布尔，沿用 `parse_bool`）。缺省时取默认值，未知值忽略，与 `files.check-changed-on-disk` 的处理一致。

### D5：状态栏旧文字

- 找不到时不再写入原来的状态栏提示。
- 表格的 `message_is_notice()`、十六进制的 headline 逻辑保持不变，只是不再收到这类消息。
- 文本视图内部的 `self.message = "No more differences"` 一并删除，因为它对用户不可见，只被测试断言过；测试改为断言面板。

### D6：关闭条件与键盘处理的顺序

`begin_frame` 必须在视图处理本帧键盘与命令之前调用。这样"再按一次 Ctrl+N 仍找不到"会先关闭、再重新弹出（并再响一次）；点击工具栏按钮时，按下那一帧关闭，松开那一帧重新弹出。文本视图已按此实现，其余视图照搬：

- 表格：`table/view.rs::ui` 中处理 `keyboard` 之前；
- 十六进制：`hex/view.rs::ui` 中处理按键之前；
- 合并：`merge/view.rs::ui`、`table_merge/view.rs::ui` 开头；
- 文件夹：`dir_view.rs::ui` 中 `drain notices` 与键盘处理之前。

## Risks / Trade-offs

- [文件夹冲突的结果由后台线程异步返回，面板会晚一两帧出现；如果这期间用户按了键，可能出现"刚关就弹"] → 可以接受，等同于用户在面板出现前已经开始下一次操作；不做额外抑制。
- [单元测试里的提示音] → `cfg!(test)` 跳过；`apps/` 下的集成测试如果驱动 GUI 可能会响，目前没有这类测试，出现时再加环境开关。
- [面板占用 48 px，矮窗口里内容区会短暂变小] → 与参考软件一致；主体高度 `max(top)` 已防止出现负值。
- [各视图分别接入，遗漏某个入口（如工具栏按钮与快捷键走不同函数）] → 每个视图的测试同时覆盖"快捷键"和"菜单命令 id"两条路径。

## Migration Plan

无数据迁移：旧的 `settings.txt` 没有新键，按默认值处理，行为与 `bd5b2f0` 之后的文本比较一致。回滚时直接撤销提交即可，多出的键会被旧版本忽略：`settings::save` 先读出整个文件，只覆盖自己认识的键，未知键原样保留（已核对 `settings.rs`）。
