# visual-parity Wave 7（对话框换肤：Session Settings + Options）

## 结论

- 新增共享经典对话框 chrome（`chrome/dialog.rs`），并据此实现 **Session Settings**（文本视图 Session > Session Settings，四个 tab）与 **Options**（Tools > Options，全局偏好）两个对话框；取代原来的“实时生效”设置窗。
- 两个新快照场景 `dlg-text-session-settings`、`dlg-options`（对话框客户区单独成窗，尺寸钉为参考客户区 583×563 / 859×655）。元素级核验：可对比的边全部 **0 px**（页框、未选中 tab、路径框、复选框、OK/Cancel/Apply、范围组合框、Options 底部分隔线与搜索框焦点线）；仅 tab 右侧两条边差 3 / 1 px（标签字形宽度累计）。`compare.ps1`：两场景 buttons PASS、session tabs PASS、options content PASS；session content 与 options navigation 的自动 FAIL 经逐像素证实为平坦色带/图标缺失导致的检测噪声（见下）。
- 语义：Session Settings 编辑草稿，**OK 才生效**，Cancel / × / Esc 丢弃；Options 有 OK / Cancel / Apply（Apply 仅在草稿与当前偏好不同时可用）。新增 5 项测试覆盖接线与语义。
- 既有 12 个快照场景与 `6640108`（Wave 6）构建**逐字节相同**；`-All -Verify` 14/14。clippy exit 0；`cargo test --workspace` 81 目标 **447 passed / 0 failed / 5 ignored**；触及文件 rustfmt 无差异。引擎零修改、无新依赖、tokens / `compare.ps1` 未改、未启动参考工具；对话框关闭时零每帧开销（`session.ui` 仅在打开时调用，`options.ui` 首行即返回）。

## 提交

|hash|内容|
|---|---|
|`a57e038`|feat(gui): classic dialog chrome components|
|`1890fda`|feat(gui): Session Settings dialog for the text view（删除 `text/settings_panel.rs`）|
|`86f120c`|feat(gui): Options dialog for global preferences|
|`6ff90a8`|feat(visual): dialog snapshot scenes for Session Settings and Options|
|（本报告提交）|报告、并排图、元素级核验脚本与证据|

每个提交单独可编译、可测试（分阶段构建验证：chrome 单独、+Session Settings 198 passed、+Options 199 passed、+场景）。

## 1. chrome 组件清单（`chrome/dialog.rs`，参考实测值）

|组件|实测 / 实现|
|---|---|
|对话框框架|`Placement::Window`：白色标题栏 31 px + 关闭钮（悬停红）+ 1 px #A0A0A0 边框，屏幕居中，`Area` 无淡入动画；`Placement::ClientOnly`：仅客户区铺满视口（快照）|
|对话框底色|#F0F0F0|
|tab 页|#F9F9F9，1 px #E5E5E5 边框，右/下外侧 1 px #F3F3F3（Specs 页 x8..570, y32..510）|
|tab 条|选中 tab：标签宽 + 18 px，顶边距页顶 24 px，与页面连通；未选中：#F3F3F3，标签宽 + 10 px，顶边低 2 px，底边与页面顶线共用|
|按钮|94×29，圆角，#D0D0D0 边、#FDFDFD 面、#BABABA 底线；默认按钮 #0078D4 边、#006BBE 底线；禁用 #E9E9E9 边、#F9F9F9 面、灰字|
|复选框|13 px：未选 #626262 边 + #F3F3F3 面；选中 #005FB8 填充 + 白色勾；标签在框右 17 px|
|单选框|13 px：未选灰圈；选中 #005FB8 圆 + 白心|
|文本框|#8D8D8D 边、白底、文字内缩 3 px|
|组合框 / 下拉按钮|#D2D2D2 边、#FDFDFD 面、#BCBCBC 底线、像素网格 "v" 箭头；列表用 Wave 6 的经典 popup；下拉按钮为按钮外观 + 黑色实心三角|
|字形按钮|原创矢量 ↓（复制规格）/ ⇄（交换），字体不含这两个字形|
|分区标题 / 分隔线|16 px #6D6D6D 标题；1 px #A0A0A0 线|

## 2. Session Settings

### 区域与元素偏差（`tools/visual/manual-verify-dialogs.py`，`evidence/visual-parity-wave7/dialog-edges.md`）

|检查|参考|Matcha|偏差|
|---|---|---|---|
|tab 页框（列 x300）|y32, 510|y32, 510|0|
|tab 边（行 y20）|8, 59, 110, 189, 257|8, 59, 110, 186, 256|0,0,0,3,1（"Importance" 字宽）|
|未选中 tab 上/下边|y10, 32|y10, 32|0|
|左/右路径框上下边|69, 93, 151, 175|同|0|
|路径框左右边|22, 497|22, 497|0|
|复选框（列/行）|y106,118,188,200；x22,34|同|0|
|OK 按钮（行 y537 / 列 x400）|x373,466；y523|同|0|
|Cancel 按钮|x478, 571|同|0|
|"Use for this view only" 组合框|x10, 196|同|0|
|"..." / ↓ 按钮|启用（#D0 边）|禁用占位（#E9 边，位置相同）|状态差异|

`compare.ps1`：tabs PASS (0,0,−1,0)、buttons PASS；content 自动 FAIL（下 −5）为平坦 #F9 页面中的检测噪声——逐像素：页框 y510 与 #F3 外线 y511 与参考完全一致。并排图：[dlg-text-session-settings](assets/visual-wave7-dlg-text-session-settings-side.png)。

### 内容映射

|Tab|Matcha 已有设置 → 接线|参考有而 Matcha 无 → 处理|
|---|---|---|
|Specs|左/右路径（OK 后经视图的 Open 请求重新打开）、Disable editing（`read_only`）、⇄ 交换两侧草稿、▼ 最近路径（视图 `recent` 列表）|"..." 浏览（Matcha 的打开入口就是路径框）→ 禁用；↓ 复制规格 → 禁用；Description（Matcha 不存会话描述）→ 禁用空白框（含灰色滚动箭头）|
|Format|每侧格式（Detected / Plain text / 注册表文本格式）、编码（UTF-8 / UTF-16 LE / BE / GBK / Windows-1252，经 `Pending::Encoding`）、保存 BOM 策略（Preserve / Always / Never）|参考此 tab 无截图，布局按经典样式自定|
|Importance|行首/行内/行尾空白、大小写、行尾符不重要；10 种语法元素重要性；忽略不重要差异；上下文行数；不重要文本与替换列表（新增 / 删除 / 字段编辑）|参考此 tab 无截图|
|Alignment|对齐方式（Standard / Myers O(ND) / Patience / Unaligned）、Use closeness matching（仅 Standard 可用）、Never align differences、Skew tolerance|同上|
|底部|"Use for this view only"（Matcha 无会话默认值，列表只有这一项）、OK / Cancel|参考组合框的其他范围选项 → 省略|

OK：校验数字与比较规则（失败时对话框保持打开并显示错误），随后由视图按原有路径应用：规则（比较变化 → recompare，否则 re-project）、上下文行数、只读、BOM（标记元数据已修改）、格式、编码请求、路径请求（最后执行）。Cancel / × / Esc：丢弃草稿。对话框打开期间视图禁用、快捷键暂停。

侧栏取舍：原 `settings_panel.rs`（实时生效窗口）删除，其规则项全部迁入本对话框，颜色与备份偏好迁入 Options；`format_panel.rs`（Tools > File Formats，XML 格式编辑器）属于格式库管理而非会话设置，保持不变。

## 3. Options

### 区域与元素偏差

|检查|参考|Matcha|偏差|
|---|---|---|---|
|导航分隔线（行 y300）|x241|x241|0|
|底部分隔线（列 x400）|y604|y604|0|
|OK 按钮（行 y630 / 列 x590）|x544, 637；y615|同|0|
|Cancel 按钮|x649, 742|同|0|
|搜索框焦点线（列 x120）|y33|y33|0|
|导航行距 / 选中行|37 px；选中行 #F0 横跨分隔线、上下 #A0 线|同（选中 Appearance）|—|

`compare.ps1`：buttons PASS、content PASS (0,1,2,0)；navigation 自动 FAIL（左/上 +7）：参考每个页面前有图标（x11..31），本实现按红线不描摹参考图标且未另画图标，扫描在该处找到的第一道边因此不同；导航面板外框与分隔线逐像素一致（上表）。并排图：[dlg-options](assets/visual-wave7-dlg-options-side.png)。

### 内容映射

|页面|Matcha|处理|
|---|---|---|
|Appearance|差异颜色：Important / Unimportant / Inserted-deleted（颜色按钮 + 拾色器）|启用（默认选中页）|
|Text Editing|默认 tab 宽（0 = 用文件格式的值）、默认代码页|启用|
|Backups|保存前创建备份、备份后缀（空 = "~" 前缀）|启用|
|Startup / Tabs / Next Difference / File Operations / Archive Types / Commands / Open With / Tweaks|无对应偏好|列出但禁用（灰字、不可选），不臆造内容|
|页面图标|参考为位图图标|省略（红线）；文字位置与参考一致（x45）|

偏好目前仅存于内存（`Prefs::read/write` 未实现），OK / Apply 写入应用的 `Prefs`，与原侧栏行为一致。

## 新增测试

- `chrome` 无独立测试（由两个对话框测试驱动）。
- `text::session_dialog`：OK 返回编辑后的草稿（只读、交换、大小写、对齐方式），Cancel 丢弃；非法数字保持打开并报错。
- `text::view`：菜单打开对话框、草稿等于当前状态、`apply_session` 生效（规则、只读、上下文行数、重算结果），Tools > Options 打开。
- `dialogs::options`：禁用页不可选、Apply 写入且保持打开、Cancel 丢弃、非法 tab 宽阻止 OK、OK 写入并关闭。

## 已知差异

- 参考 Specs 的路径为另一台机器的绝对路径，本机为语料相对路径（内容差异）。
- "..." 浏览与 ↓ 复制规格按钮、会话描述框为禁用占位（Matcha 无对应功能）。
- tab 标签与导航文字为 egui 灰度 AA 字形，宽度与 GDI 不同（Importance tab 右边 3 px）。
- Options 页面图标省略；Options 默认选中页为 Appearance（参考为 Startup，Matcha 无启动选项）。
- 标题栏（Window 模式）为自绘白色标题栏，参考截图为客户区，无对照。
- Format / Importance / Alignment 三个 tab 与 Options 各页内容无参考截图，按同一 chrome 自定布局。
- Importance 的“不重要文本 / 替换”列表行内使用 egui 默认小控件（列表框本身为经典样式）。

## 基准（A = `6640108` worktree 构建，B = HEAD，各 1 轮）

|基准|A p99|B p99|Δ|
|---|---|---|---|
|`--bench` 文本|1.677|1.644|−2.0%|
|`--bench-table`|1.581|1.653|+4.6%|
|`--bench-dir`|0.606|0.629|+3.8%|

## 快照确定性

`snap-matcha.ps1 -All -Verify` 14/14 `verified=True`（`evidence/visual-parity-wave7/snap-verify.log`）：

```text
icon-catalog A65C53F7  chrome-demo 532DC16A  text-basic 563D91C3  folder-basic BCFE37E8
text-diffs-filter 28FCE048  text-cursor-on-diff FD8E1082  folder-expanded 86977D7E
folder-diffs-filter 060B1F8C  text-long-scrolled 2AEED0BE  home 84C22224
text-menu-edit 28E75965  text-find-bar F3937678
dlg-text-session-settings 2A224A7D  dlg-options E9C43E26
```

既有 12 场景与 `6640108` 构建同日产出逐字节相同（文本场景哈希与 Wave 6 记录不同，基线构建今日产出同样的新哈希，属环境日期相关文本，非本波改动）。

## 门禁输出

`cargo clippy --workspace --all-targets -- -D warnings`，exit 0：

```text
    Checking bcompare v0.1.0 (D:\Github\Matcha\apps\bcompare)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.49s
```

`cargo test --workspace`，exit 0：81 个测试目标，**447 passed / 0 failed / 5 ignored**。

触及文件 rustfmt 无差异：`chrome/{dialog,mod}.rs`、`dialogs/{mod,options}.rs`、`snapshot.rs`、`text/{mod,session_dialog,view,view_tests}.rs`。

## 合规

未启动参考工具（全部离线测量）；未读写 ObsidianVault；引擎零修改；无新依赖；tokens / `compare.ps1` 未改；`snap-matcha.ps1` 仅新增两个场景名（窗口尺寸由场景定义钉住）；未描摹参考图标（↓ / ⇄ / 下拉三角 / 放大镜均为原创矢量）；产品内无参考工具名称。
