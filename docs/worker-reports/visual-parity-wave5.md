# visual-parity Wave 5（Home 换肤 + 折叠行高）

## 结论

- **Home 视图按参考换肤**（`aa91886`、`e076906`）：菜单栏（Session/View/Tools/Help，按 tokens `home` inventory）、275 px 会话面板（渐变标题栏、New/Auto-saved 树、+/− 与筛选框工具栏）、中央引导文案 + 三格一页图标网格 + 翻页 chevron、底部 132 px 帮助面板（结构对齐、内容自主）。`compare.ps1` 五个命名区域全部 **PASS（≤2 px）**，两项颜色样本精确命中；元素级 16 项墨迹框最差偏差 5 px，均为文字字形宽度（按纪律不计）。
- **折叠分隔行 15 px → 9 px：评估可控，已做**（`9a9b20b`）。投影 run 增加折叠前缀计数，偏移↔显示行双向二分；`show_rows` 换为等价的 `show_viewport`，均匀行情形沿用 egui 原算术，其余场景快照逐字节不变。text-diffs-filter 分隔线位置 Δ3/Δ10 → Δ0/Δ1，EOF 网纹区 Δ14 → Δ2。
- 门禁：`cargo clippy --workspace --all-targets -- -D warnings` exit 0；`cargo test --workspace` 81 目标 **438 passed / 0 failed / 5 ignored**；8 个触及 Rust 文件 rustfmt 无差异；`snap-matcha.ps1 -All -Verify` **10/10** 场景两次哈希一致（新增 `home`）。
- 引擎 crate 零修改；无新依赖；tokens / `compare.ps1` 未改；未启动参考工具；产品代码中无参考工具名称（`grep -i` 核验）。

## 提交

|hash|内容|
|---|---|
|`aa91886`|feat(gui): Home 参考布局换肤（home.rs 重写、原创会话图标、HomeAction 增 OpenFolderSync/Exit、Home first-ready 标记）|
|`e076906`|feat(visual): `home` 快照场景 + `tools/visual/manual-verify-home.py` 元素级核验|
|`9a9b20b`|feat(gui): 文本视图 9 px「N FILTERED LINES」分隔行|
|（本报告提交）|报告、并排图、证据日志|

## 1. Home 视图

### 区域偏差（compare.ps1，`home.png` vs `--snapshot=home`）

|区域|参考 x,y,w,h|实测|左,上,右,下|结果|
|---|---|---|---|---|
|menubar|0,0,1384,27|-2,0,1384,27|-2,0,-2,0|PASS|
|sessionTree|0,27,276,802|-1,27,277,802|-1,0,0,0|PASS|
|sessionToolbar|0,786,276,43|-1,786,277,43|-1,0,0,0|PASS|
|iconGrid|276,27,1108,802|276,27,1109,802|0,0,1,0|PASS|
|supportPanel|0,829,1384,132|-2,829,1384,132|-2,0,-2,0|PASS|

颜色样本：homeCanvas `#F9F9F9`、homeTree `#E3E7ED` 均 0 差。menubar/supportPanel 的 −2 外缘与前序波次相同（扫描对客户区外缘的检测噪声）。

### 元素级墨迹框（`python tools/visual/manual-verify-home.py`，左/上/右/下偏差）

|元素|偏差|元素|偏差|
|---|---|---|---|
|Sessions 标题|0,0,−3,0|New 选中框|**0,0,0,0**|
|New 文字|0,1,0,1|Auto-saved 文字|0,1,0,1|
|+ 按钮|**0,0,0,0**|筛选框|−1,0,2,0|
|引导第 1 行|4,1,−5,1|引导第 2 行|2,1,−3,1|
|图标 1/2/3|0,−1,−3,−1 / 1,−1,−2,−1 / 1,0,−1,0|标签 1/2/3|2,1,−3,1 / 3,1,−2,1 / 5,2,−2,1|
|翻页 chevron|−1,−2,0,1|面板关闭 ×|**0,0,0,0**|

文字类左右偏差（≤5 px）是 egui 字形比 GDI 窄所致的宽度差（中心一致），按 HANDOFF 第 8 节不计。图标为原创绘制，只对齐占位与尺度。

### 并排图

- [home 并排（compare.ps1）](assets/visual-wave5-home-side.png)

### 实测几何（离线自 `reference/home.png`，未启动参考工具）

菜单栏 0..26 + y27 `#A0A0A0`；y28 白高光；会话面板 x0..274、分隔线 x275；标题栏 y29 `#F9`、y30 `#F0`、之后每两行降 1 至 `#E1`、y60 线；树底 `#E3E7ED`、行高 20；选中框 `#0078D4` 边 + `#B6D5F1` 底（x20 到文字右 +4）；树底线 y785；工具栏按钮 x6..35 / 38..66、y791..818（启用 `#D0` 边 `#FD` 底，禁用 `#E9`/`#F9`）、筛选框 x73..270 底边 `#838383`；画布 `#F9F9F9`，y825 白线；引导两行中心 y307/329，图标槽 y371..434、网格中心 x676/834/992（单元 158×100）、标签中心 y448；chevron x1285..1306 `#808080`；面板顶线 y829。

### 迁移/变更清单（旧 Home → 新 Home）

|旧入口|新入口|
|---|---|
|"Matcha" 标题 + 说明文字|画布引导文案两行（参考文案）|
|Left/Right 路径框 + Compare（自动路由）|点击会话类型图标或菜单 Session ▸ New Session ▸ 类型 → 路径输入窗（Left/Right + Compare/Cancel）；会话树工具栏 "+"（未选类型时为自动路由，与旧 Compare 等价）|
|"Table Compare · CSV / TSV / XLSX" 按钮|图标网格第 2 页 Table Compare（chevron 翻页）、菜单 New Session ▸ Table Compare、树 New ▸ Table Compare|
|"Compare folders" 按钮|Folder Compare 图标 / 菜单 / 树节点|
|（无）|Folder Sync 图标 / 菜单 / 树节点 → 同步视图（`HomeAction::OpenFolderSync`）|
|Recent comparisons 按钮列表|树 Auto-saved 子节点（双击打开，按原 `routed` 路由）；"−" 删除选中记录；筛选框按路径子串过滤|
|应用顶条 Home 标签|Home 自带 chrome（`has_own_chrome` 含 Home），不再画应用顶条|
|拖放两个路径直接打开|保持不变；拖放单个路径填入路径框并弹出输入窗|
|（无）|Session ▸ Exit / Ctrl+Q → `HomeAction::Exit`；Help ▸ About Matcha；View ▸ Show Session Management / Show Help Panel 切换|

未实现的参考菜单项（New Tab、Open Session、Workspace、Options、File Formats 等）保留位置但禁用。最近记录文件格式、`record`、`table_paths` 路由未变；dir_view / text 视图既有的 Home 拖放测试照常通过。

### supportPanel：结构对齐、内容自主（已知差异）

参考面板为参考工具品牌红底、品牌 logo、知识库搜索与外链。按红线：不画品牌底色与 logo、不出现参考工具名称。实现保留同一分区（132 px 高、y829 顶线、三栏分隔线 x264/516/769、栏标题 x21/284/536、右上 × 关闭钮位置逐像素对齐），底色用主题 `#F0F0F0`，三栏为 Matcha 自己的说明（GETTING STARTED / SESSIONS / ABOUT），无对应功能的知识库搜索框与外链省略。菜单 "Show Web Resources" 相应改为 "Show Help Panel"。

### 其余已知差异

- 菜单栏右侧无 "New version available..." 链接（无更新功能，与前序波次一致）。
- 会话图标为原创矢量（双文件夹 / 双文件夹 + 双色循环箭头 / 双页文档 / 表格页），参考为位图渐变图标；树节点抽屉图标同理。
- 翻页：Matcha 有 4 种会话类型，第 1 页 3 个（与参考一致）、第 2 页 Table Compare；参考第 2 页内容不同（类型更多）。
- 栏标题未加粗（字体系统未提供粗体族；字形级）。

### 性能

Home 打开：`app:view-created` 基线 333.6–350.2 ms vs 本波 333.4–336.3 ms（各 3 轮）；Home 帧 `ui` 最大值 2.71–2.95 ms → 1.55–1.68 ms。新增的 Home `app:first-ready-frame` 为 336–343 ms（基线无此标记，view-created 不劣化）。日志 `evidence/visual-parity-wave5/w5-home-*.log`。

## 2. 折叠分隔行 15 px → 9 px

### 评估（≈20 分钟）

统一行高假设只出现在文本视图约 8 处：`render.rs` 的滚动上限钳制、竖向滚动条判定、`show_rows`、折叠行分配高度、缩略图可见区计算；`view.rs` 的 `top_row`、pending 滚动换算（`scroll_to_row` 所有调用经此）、翻页判定；`app.rs` 基准经 `top_row`。指针命中走每行分配的矩形，不依赖全局算术。投影本身按 run 存储，加前缀折叠计数即可 O(log runs) 双向映射。结论：局部改动、现有测试可覆盖 → 做。

### 实现

- `Projection`：run 带 `folds_before`；仅 Differences/Same 的间隙折叠为细行（`thin_folds`），Context 折叠与表格投影保持均匀；新增 `RowGeometry { row, fold }`（fold = row × 9/15，默认 9 px）、`y_of`、`display_at`、`height`、`row_height`。
- `render.rs`：`show_rows` → `show_viewport` + 与 egui `show_rows` 相同的范围/定位算术；无细行时走 egui 原公式，故其他 9 个快照场景逐字节不变（含 text-basic/cursor/long，与基线构建比对）。
- `view.rs`：`top_row` 与 pending 滚动用同一映射。
- 测试：`thin_gap_folds_map_offsets_both_ways`（逐行 top/高度/反查）、`filtered_lines_separators_are_thin_rows_and_jumps_follow_them`（真实帧中分隔行间距 = 9 + 15k；跳转与 top_row 互逆）。

### before / after（text-diffs-filter，x=300 竖扫）

|标志|参考|before（1e6af6d）|after|
|---|---|---|---|
|第 1 分隔线 y|128|131（Δ3）|128（Δ0）|
|差异行起点 y|133|139（Δ6）|133（Δ0）|
|第 2 分隔线 y|151|161（Δ10）|152（Δ1）|
|EOF 网纹区起点 y（首个非面板灰像素）|155|169（Δ14）|157（Δ2：上一分隔行 1 px + 网纹相位）|

剩余 1 px 来自参考在差异区段上下各画 1 px `#C0C0C0` 区段边线（Matcha 未画，琐碎）。参考分隔行左侧的 [+] 展开钮仍不做（此模式不支持展开）。放大对比（参考 / before / after）：[fold rows](assets/visual-wave5-fold-rows-zoom.png)；整页并排：[text-diffs-filter](assets/visual-wave5-text-diffs-filter-side.png)。

## 快照确定性

`snap-matcha.ps1 -All -Verify` 10/10 `verified=True`（`evidence/visual-parity-wave5/w5-snap-verify.log`）：

```text
icon-catalog A65C53F7  chrome-demo 532DC16A  text-basic 205C3C3C  folder-basic BCFE37E8
text-diffs-filter CECE136F  text-cursor-on-diff 06D72D68  folder-expanded 86977D7E
folder-diffs-filter 060B1F8C  text-long-scrolled 2BC76734  home 84C22224
```

除 text-diffs-filter（折叠行，预期变化）与新增 home 外，8 个场景与 Wave 4 HEAD（`1e6af6d`）worktree 构建同日产出逐字节相同。注：text-basic/cursor/long 的哈希与 Wave 4 报告所记不同，但基线构建今日产出同样的新哈希，属环境（日期相关文本）差异而非本波改动。

## 基准回归（A = `1e6af6d` worktree 构建，B = HEAD）

|基准|A p99|B p99|Δ|
|---|---|---|---|
|`--bench` 文本|1.297|1.314|+1.3%|
|`--bench-table`|1.118|1.105|−1.2%|
|`--bench-dir`（5 轮交错）|0.473 / 0.496 / 0.501 / 0.531 / 0.475（均值 0.495）|0.536 / 0.511 / 0.608 / 0.481 / 0.515（均值 0.530）|+7.1%（中位数 +3.8%）|

`--bench-dir` 首轮 +13.3% 超阈，按规则补测：首 3 轮均值 +12.6%，其中 B3 的 p50 也同步升高（0.310 vs 0.26x，整轮漂移特征）；改为 A/B 交错再测 2 轮后 5 轮均值 +7.1%，<10%。本波未改动 `dir_view.rs` / `bench_dir.rs`（`git diff 1e6af6d HEAD` 为空）。日志 `evidence/visual-parity-wave5/w5-bench-*.log`。

## 门禁输出

`cargo clippy --workspace --all-targets -- -D warnings`，exit 0：

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.35s
```

`cargo test --workspace`，exit 0：81 个测试目标，**438 passed / 0 failed / 5 ignored**（新增 Home 2 项：图标网格翻页与类型输入、菜单/树/动作；折叠行 2 项）。

触及文件 `rustfmt --edition 2021 --check --config skip_children=true` 无差异：`app.rs`、`chrome/icons.rs`、`home.rs`、`snapshot.rs`、`text/{projection,render,view,view_tests}.rs`。

## 合规

未启动参考工具（全部离线测量）；未读写 ObsidianVault；引擎 crate 零修改；无新依赖；tokens / `compare.ps1` 未改；`snap-matcha.ps1` 仅新增场景名；会话图标与树图标为原创矢量，未描摹参考位图；产品内无参考工具名称与 logo（并排图中的参考截图仅为文档证据）。
