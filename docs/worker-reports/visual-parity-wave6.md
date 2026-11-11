# visual-parity Wave 6（右键菜单换肤 + 行详情精调 + Find bar 对照）

## 结论

- **右键菜单换肤**（`b68f4c0`、`a59b6f7`）：新增共享的 `chrome::popup`（按参考下拉实测几何），菜单栏下拉改由它绘制，文件夹视图全部右键入口（比较网格行、同步布局行与列头）改用 `popup::context_menu`，菜单项/动作/启用条件不变。新增 `text-menu-edit` 场景（下拉打开态）做确定性证据：弹出框顶边与 4 条分隔线 y 坐标与参考**逐像素一致**（25 / 73 / 209 / 303 / 376），宽 278 vs 275。文本视图**没有任何右键菜单**（核实：`text/` 下无 `context_menu`/`secondary_clicked`），按“只换肤不改内容”不新增。
- **行详情精调**（`47521eb`）：text-basic / text-cursor-on-diff 的 details 区域 `compare.ps1` PASS（0,0,1,0）；元素级核验：文本行、文本框右缘、分隔线、底部高光、占位短横 **0 px**，最差 2 px（原创矢量箭头尖端 AA / 行尾符左侧邻接字形 AA）。差异行上采用面板同款差异底色与字色，活动侧显示参考的 2 px 光标条（cursor-on-diff 文本框 x71..105、光标条 x98..99 与参考完全一致）。
- **Find bar 对照**（`d50bd49`）：改为参考的工具栏下方停靠条（37 px，路径栏随之下移）；新增 `text-find-bar` 场景。组合框边框、选中文本、历史/四个开关/Prev/Next/展开按钮框、下移后的路径栏边框全部 **0 px**，最差 1 px（原创放大镜图形）；`compare.ps1` findbar 区域 PASS。
- 既有场景：除文本场景的 details 区域外**逐字节不变**（与 `eb6a5dc` worktree 构建逐场景比对，差异包围盒全部落在 y887..928）；`snap-matcha.ps1 -All -Verify` **12/12**。
- 门禁：clippy `-D warnings` exit 0；`cargo test --workspace` 81 目标 **443 passed / 0 failed / 5 ignored**；11 个触及 Rust 文件 rustfmt 无差异。引擎零修改、无新依赖、tokens/`compare.ps1` 未改、未启动参考工具。

## 提交

|hash|内容|
|---|---|
|`b68f4c0`|feat(gui): classic popup shared by dropdowns and right-click menus|
|`a59b6f7`|feat(visual): text-menu-edit snapshot scene with the dropdown open|
|`47521eb`|feat(gui): align the text details strip with the reference|
|`d50bd49`|feat(gui): reference-style docked find bar in the text view|
|（本报告提交）|报告、证据图、元素级测量与基准日志|

## 1. 右键菜单 / 下拉菜单

### 实测几何（离线自 `reference/text-menu-*.png`）与实现

|项|参考|实现|
|---|---|---|
|底色 / 边框|#F0F0F0 / 1 px #A0A0A0，无阴影（PrintWindow 截图不含阴影）|同；egui `Area` 关闭淡入（经典菜单即时出现）|
|项高 / 分隔|21 px / 10 px，#D7D7D7 线在分隔顶 +4，左右各内缩 3 px|同|
|上下内边距|边框内上 1 px、下 3 px|同|
|标签 / 快捷键|标签距左边框 27 px；快捷键右对齐距右边框 24 px，与标签间 47 px|同|
|宽度|逐项（标签 + 47 + 快捷键）取最大，再加左右边距（无快捷键的长标签不叠加快捷键列）|同|
|子菜单箭头|4×7 实心三角，距右缘 10..13 px|同（原为 "▸" 字符，字体缺字显示为方框）|
|禁用色|≈#9F9F9F|同|
|打开态标题|#F9F9F9 填充 y2..24，无边框；下拉顶边在菜单栏顶 +25|同（原为悬停蓝 + 蓝边框）|
|鼠标打开|无高亮项|同（原为高亮首项；键盘 Alt+助记符打开仍高亮首项）|

勾选/单选标记在左侧图标槽中央（x+12.5）原创矢量绘制。悬停高亮色参考截图未提供，沿用主题 HOVER_BLUE（未测量，不臆造）。

### 证据（确定性替代：下拉打开态快照场景 `text-menu-edit`）

`menubar::request_open` 在下一帧以“鼠标点击”方式打开指定菜单；before 图由在 `eb6a5dc` worktree 仅加入同一钩子（不含新 popup）构建得到。`tools/visual/manual-verify-menu.py` 输出（`evidence/visual-parity-wave6/menu-dropdown.txt`）：

|项|参考|before|after|
|---|---|---|---|
|弹出框左/右边框 x|89 / 363|91 / 355（白底，首项蓝高亮）|91 / 368|
|顶边 y|25|27|**25**|
|分隔线 y|73, 209, 303, 376|459 一条（7 px 分隔、22 px 行）|**73, 209, 303, 376**|
|底色|#F0F0F0|#FFFFFF|**#F0F0F0**|
|打开态标题 x|89..122|蓝色悬停样式|91..126|

左缘 2 px 与宽度 3 px 偏差来自菜单栏标题文字宽度累积（字形宽度，不计）。三联图：[参考 / before / after](assets/visual-wave6-menu-dropdown-3way.png)。

### 换肤范围

|入口|处理|
|---|---|
|文件夹比较网格行右键（Select row / Open file pair / Expand / collapse）|`popup::context_menu`，打开时记录指针所在行；动作映射到原 `context_command`|
|同步布局行右键（同三项）|同上|
|同步布局列头右键（可选列开关 + 说明）|列开关改为勾选项、说明为禁用项；切换逻辑抽为 `set_column`，与 View 菜单共用|
|文本视图|无右键菜单（不新增）|
|菜单栏下拉（文本/文件夹/Home）|改由同一 `popup::cascade` 绘制|

测试：`chrome::popup` 2 项（几何、禁用项、点击派发、Esc/点外关闭）；`dir_view` 新增右键行菜单端到端测试（右键 → 选 "Select row" → 选中整行）；菜单栏既有测试改为“鼠标打开不高亮、↓ 后 Enter 派发”的驱动方式，覆盖不变。性能：菜单关闭时每个入口每帧仅一次 `get_temp`（无分配），菜单项闭包只在打开时调用。

## 2. 行详情

### 元素级核验（`tools/visual/manual-verify-details.py`，左/上/右/下偏差）

|元素|text-basic before|text-basic after|text-cursor-on-diff after|
|---|---|---|---|
|顶部 #A0A0A0 线 / 行分隔 / 底部高光|0 / 0 / 0|0 / 0 / 0|0 / 0 / 0|
|文本行 1 / 2|1,−1,0,−1 / 同|**0,0,0,0 / 0,0,0,0**|蓝色字形 AA（不计）|
|行尾符|−2,−2,2,2（黑色 ¶）|−2,0,0,1（左值为相邻 'a' 的 AA）|—|
|文本框右缘|+6|**0**|x71..105 与参考一致（逐像素扫描）|
|箭头 1 / 2|−1,1,−3,−2 / 同|0,0,−2,0 / 2,0,0,0|同左|
|第三行占位|3,−6,−1,−7（黑色 1 px 文本 "—"）|**0,0,0,0**|**0,0,0,0**|
|最差|7|**2**|字形外 **2**|

`compare.ps1` details 区域：两场景均 PASS (0,0,1,0)，色差均值 5.92 → 3.12。

### 变更

- 文本左移 1 px、下移 1 px（与参考字形起点一致）。
- CRLF 行尾符：参考为灰色（#A0A0A0）7×7 小方框符号，原为黑色 "¶"；现为原创像素网格绘制。LF/CR 保留文字符号，改为同一灰色。
- 等宽字体步进：GDI 微调后为 7 px，egui 为 ≈7.33 px；行尾符与光标条位置按整数步进网格换算（`round(adv)/adv`），使其落在参考的格位上。
- 当前行为差异行时：文本框用面板同款行底色（第二行取与 #FCFCFC 的逐通道最小值，复现参考 #EEEEFC）、文字用差异字色；#C0C0C0 左缘只在相同行显示；活动侧绘制参考的 2 px #C0C0C0 光标条。
- 侧箭头保持原创矢量（红线：不复刻参考位图），只把外框对到参考占位（x56..67、10 行、行中线）。
- 第三行占位改为 2 px #858585 短横（x70..86, y926..927）。

三联放大图：[text-basic](assets/visual-wave6-details-text-basic-3way.png)、[text-cursor-on-diff](assets/visual-wave6-details-cursor-3way.png)。新增测试 `details_take_the_current_rows_difference_colors_and_caret_bar`。

## 3. Find bar

### 元素级核验（`tools/visual/manual-verify-findbar.py`）

|元素|参考|实测|偏差|
|---|---|---|---|
|放大镜|10..24, 82..96|9..24, 81..95|−1,−1,0,−1（原创图形）|
|组合框边框（聚焦蓝）|41..695, 78..102|同|0|
|选中文本底|44..75, 81..97|同|0|
|组合框内箭头|683..690, 89..92|同|0|
|历史按钮|702..729, 77..103|同|0|
|开关 .* / Aa / 全词 / 换行|742/772/802/832 起，各 28 宽|同|0|
|Prev / Next 按钮|872..915 / 918..963|同（底边 103 vs 104）|0,0,0,−1|
|Prev / Next 文字|882..905 / 928..953|883..905 / 928..953|≤1|
|展开按钮|1349..1377, 77..103|同|0|
|下移后路径栏边框|y108|y108|0|

`compare.ps1`（text-find-bar）：findbar PASS (1,0,2,0)、details PASS、menubar/toolbar PASS；pane.left (−4) 与 statusbar (−5) 的自动 FAIL 为 Wave 2b 已记录的平坦色带检测噪声——逐像素：y400 左窗格边框双方均在 x52，内容起点差 ≤1 px。并排图：[text-find-bar](assets/visual-wave6-text-find-bar-side.png)、[停靠条横带对照](assets/visual-wave6-findbar-strip.png)。

### 功能映射（功能不变）

|原入口|新入口|
|---|---|
|Find 文本框 + Next / Previous 按钮|组合框（Enter=Next、Shift+Enter=Prev）+ Prev / Next 按钮|
|Regex / Case / Whole words 勾选框|.* / Aa / 全词 三个开关按钮（开启态用工具栏选中色）|
|Backwards / Differences only 勾选框|展开按钮下拉中的勾选项（经典 popup）|
|Replace 文本框 + Replace / Replace all|下拉 "Replace" 显示第二行（替换框 + Replace / Replace All）；Search ▸ Replace...（Ctrl+H）直接带替换行打开|
|× 关闭|Esc，或下拉 "Close Find Bar"|
|（无）|历史按钮：最近 12 个查找串（经典 popup）|

换行（wrap）开关按参考显示但禁用：搜索引擎无该选项，引擎零修改。新增测试 `bar_buttons_toggles_and_dropdown_keep_find_behaviour`；既有查找/替换测试直接驱动数据字段，照常通过。

## 已知差异

- 文本视图菜单内容/启用态与参考 Edit 菜单不同（多出 Copy to Left 等、Undo 可用、无菜单图标）——菜单内容不在本波范围；参考菜单项图标为位图，不复刻。
- 悬停高亮色、子菜单弹出偏移无参考截图，沿用主题色/右缘 −3 px 的合理值。
- 行详情：侧箭头为原创矢量（与参考位图渐变不同）；text-diffs-filter 中参考在过滤后把光标移到首个可见差异行，Matcha 光标停在被隐藏的首行，故该场景详情内容不同（过滤行为，不在换肤范围）。
- Find bar：放大镜与开关字形为原创绘制；egui 文本框获得焦点后的选中渲染由 egui 负责，快照中因路径框快照钩子抢焦点，选中态由 find bar 的“打开即全选”提示自绘（真实使用时首帧即获得焦点）。
- 下拉/右键菜单首次出现的那一帧不接收输入（egui `Area` 尺寸测量帧），真实交互无感。

## 基准（A = `eb6a5dc` worktree 构建，B = HEAD）

|基准|A p99|B p99|Δ|
|---|---|---|---|
|`--bench` 文本（3 轮交错）|1.546 / 1.642 / 1.739（均值 1.642）|1.746 / 1.568 / 1.579（均值 1.631）|**−0.7%**（首轮 +12.9% 超阈后补测）|
|`--bench-table`|1.404|1.390|−1.0%|
|`--bench-dir`|0.679|0.713|+5.0%|

本轮机器负载高于 Wave 5（同一基线绝对值偏高），以同时段 A/B 为准。日志 `evidence/visual-parity-wave6/bench-*.log`。

## 快照确定性

`snap-matcha.ps1 -All -Verify` 12/12 `verified=True`：

```text
icon-catalog A65C53F7  chrome-demo 532DC16A  text-basic 3ACED20F  folder-basic BCFE37E8
text-diffs-filter B17600DC  text-cursor-on-diff 5AD26155  folder-expanded 86977D7E
folder-diffs-filter 060B1F8C  text-long-scrolled 01F9DD91  home 84C22224
text-menu-edit 399E5ECE  text-find-bar 8A5B4364
```

与 `eb6a5dc` 构建逐场景比对：icon-catalog / chrome-demo / folder-basic / folder-expanded / folder-diffs-filter / home **逐字节相同**；text-basic、text-diffs-filter、text-cursor-on-diff、text-long-scrolled 的差异包围盒分别为 (52,887)-(119,928)、(52,887)-(119,928)、(52,887)-(112,928)、(52,887)-(552,928)，全部位于 details 区域 [52,887,1332,49] 内。

## 门禁输出

`cargo clippy --workspace --all-targets -- -D warnings`，exit 0：

```text
    Checking bcompare v0.1.0 (D:\Github\Matcha\apps\bcompare)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.57s
```

`cargo test --workspace`，exit 0：81 个测试目标，**443 passed / 0 failed / 5 ignored**。

触及文件 `rustfmt --edition 2021 --check --config skip_children=true` 无差异：`app.rs`、`chrome/{menubar,mod,popup}.rs`、`dir_view.rs`、`snapshot.rs`、`text/{find_panel,render,status_bar,view,view_tests}.rs`。

## 合规

未启动参考工具（全部离线测量）；未读写 ObsidianVault；引擎零修改；无新依赖；tokens / `compare.ps1` 未改；`snap-matcha.ps1` 仅新增两个场景名；未描摹参考图标/位图（箭头、行尾符、放大镜、开关字形均为原创矢量/像素网格）；产品内无参考工具名称。
