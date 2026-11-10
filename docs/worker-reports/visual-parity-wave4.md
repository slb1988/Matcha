# visual-parity Wave 4（UI 卡顿优化 + 缩略图靠齐）

## 结论

- **Part A（卡顿）**：先测量再修。渲染本身不是瓶颈（文本 110 MB 各交互帧 ≤3 ms），用户可感知的卡顿来自四类：①文本/文件夹视图滚轮被 egui 平滑后「拖尾滑行」；②启动与打开路径上同步的 PowerShell 主机查询（每次 150–550 ms）；③文件夹树线每帧为远处父行生成上万个点（100k 行展开时每帧 8–29 ms，**基线 `--bench-dir` 自身判定 pass=false**）；④状态栏每帧遍历全部行统计选中项。六项全部修复，每项附 before/after。
- **Part B（UI 靠齐）**：文本视图左侧 52 px 缩略图（时刻可见的大项）由旧导航条改为参考样式；三个文本场景缩略图区与参考**逐像素一致**（差异像素 2181→0 / 42484）；新增 `text-long-scrolled` 场景覆盖滚动后的视口框/遮罩。其余候选按「抓大放小」列为已知差异（原因见下）。
- REA：本机为 Windows，REA 的 `observe_native_ui` 返回 `capability_unavailable: Native macOS utilities require macOS`，无法观测参考工具实时 UI；按锁约定启动参考工具一次（≈1 分钟）即关闭，`BCompareProcesses=0 lockExists=False`。悬停/按下态因此无法测量，不臆造。
- 门禁：`cargo clippy --workspace --all-targets -- -D warnings` exit 0；`cargo test --workspace` 81 个目标 **434 passed / 0 failed / 5 ignored**；14 个触及 Rust 文件 `rustfmt --check` 无差异；`snap-matcha.ps1 -All -Verify` 9 场景两次哈希一致。引擎 crate 零修改，无新依赖，`compare.ps1`/tokens 未改。

## 提交

|hash|内容|
|---|---|
|`34e9541`|perf(gui): 文本/文件夹视图滚轮原始增量当帧应用（`chrome::wheel` 共享，表格复用）|
|`b56f1a2`|feat(gui): `MATCHA_FRAME_PERF` 帧时间直方图 + 各视图 first-ready 标记|
|`a944ac5`|perf(gui): 系统字体查询与窗口创建并行|
|`400ef58`|perf(gui): 文本修改时间标签移出 UI 线程|
|`f0e4b70`|perf(gui): 文件夹时钟+剩余空间合并为一次主机查询并与扫描并行|
|`082999e`|test(gui): 文本/文件夹交互延迟探针（`#[ignore]`）|
|`7147453`|perf(gui): 文件夹树线点阵按可见区裁剪|
|`0397629`|perf(gui): 文件夹状态栏选中统计缓存|
|`f1695d7`|feat(gui): 参考样式文本缩略图替换导航条|
|`ff2a46d`|feat(visual): `text-long-scrolled` 快照场景|
|（本报告提交）|报告、并排图、证据日志|

## Part A：测量方法

1. **真实应用、真实帧**：`MATCHA_FRAME_PERF=1`（新增，默认零开销）记录每帧 `ui` 耗时与帧间隔，>16 ms 实时打印，退出打印 p50/p90/p99/max；`MATCHA_PERF=1`（`MATCHA_TABLE_PERF` 别名）打点启动各阶段与首个 ready 帧。脚本：启动 → 等待 N 秒 → `taskkill`（正常关闭以打印汇总）。
2. **交互延迟探针**（可复现，`cargo test --release -p bc-gui --lib interaction_latency -- --ignored --nocapture`）：真实视图经 egui 完整 pass + tessellate 计时；文本用 110 MB 审计输入（2,141,335 行），文件夹用 Wave 3 的 100k 文件夹具；覆盖打开/对齐、首帧、60 帧滚轮、All/Differences/Same/Context 切换、导航、跳转、Expand/Collapse All、全选，以及每次后台往返期间的最差帧。
3. **滚轮轨迹探针**（常驻测试）：一帧注入 10 个滚轮刻度（=1200 pt），记录随后 21 帧的滚动位移。
4. **既有基准**：`--bench` / `--bench-table` / `--bench-dir`，基线为 `901e43b` 的 git worktree 构建。

原始日志：`docs/worker-reports/evidence/visual-parity-wave4/`。

## Part A：热点清单（按 ROI 排序）与 before/after

### 1. 滚轮拖尾滑行（文本、文件夹）— `34e9541`

egui 0.37 `WheelState` 对 Line 单位滚轮做指数平滑；表格视图在迭代 6 已绕过，文本与文件夹视图未覆盖。

|视图|before 轨迹（pt，逐帧）|after|
|---|---|---|
|文本|382, 643, 820, 941, 1024, 1080, 1118, …, 1199.6（20 帧后仍未到位）|1200 × 21 帧|
|文件夹|382, 643, 820, 941, 1024, 1080, …, 1200（第 19 帧到位）|1200 × 21 帧|

文本两侧窗格在同一帧使用同一偏移（`first_rows` 断言 0 帧错位）；Ctrl+滚轮仍不滚动；窗格外不滚动；表格行为不变（共享实现，原有测试全绿）。

### 2. 启动到可交互（PowerShell 主机查询）— `a944ac5` `400ef58` `f0e4b70`

小语料（排除 IO），三轮：

|阶段|before|after|
|---|---|---|
|字体 SPI 查询（PowerShell+Add-Type）|319–361 ms，阻塞 UI 线程|仍 ~330 ms，但与窗口创建（~195 ms）并行；UI 线程等待 148–173 ms|
|文本 `modified_label` ×2|286–315 ms，阻塞 UI 线程（每次打开文本比较都发生，含从 Home/文件夹视图打开）|后台线程；ready 后 ~180 ms 标签出现|
|文件夹 clock + free×2（串行，挡在后代扫描之前）|199–218 + 503–549 ms|一次合并查询 245–293 ms，与扫描并行|
|**文本 first-ready 帧**|**819 / 846 / 902 ms**|**369 / 374 / 391 ms**（−55%）|
|**文件夹 first-ready 帧**|**1237 / 1249 / 1344 ms**|**595 / 603 / 688 ms**（−50%）|

快照模式等待修改时间标签就绪后再截图，九场景保持确定。盘符写入脚本前校验为纯字母。

### 3. 文件夹树线点阵 — `7147453`

每个可见行从父行图标画点线到本行，每 2 px 一个 1×1 矩形；父行在上方 1000 行时，单行就生成 ~10k 个裁剪区外的形状。改为点线从「首个可能触及裁剪区的点」开始（保持 2 px 相位），同 x 的兄弟行跳过已画区段。像素不变（folder 三场景哈希不变）。

|100k 行展开|before|after|
|---|---|---|
|滚轮帧（Expand All 后）p50 / max|10.41 / 19.17 ms|0.41 / 0.59 ms|
|滚轮帧（Show All 后）p50 / max|**28.60 / 35.67 ms**|0.41 / 0.70 ms|
|后台往返期间最差帧|19.95–23.09 ms|0.97–1.60 ms|
|`--bench-dir` update p50 / p99 / max|8.049 / 21.323 / 28.885 ms（**pass=false**）|0.279 / 0.533 / 0.790 ms（pass=true）|

注：Wave 3 报告的 `--bench-dir` p99 0.561 ms 早于树线肘形提交 `e127a6d`；基线 `901e43b` 实测已回归到 21 ms 且未被发现，本波修复。

### 4. 状态栏选中统计 — `0397629`

每帧遍历全部行 × BTreeSet 查找。改为按（快照指针，选择指纹=数量+顺序无关混合和）缓存，O(选择数) 整数运算取指纹。新增等大小选择替换的失效测试。

|100k 行全选|before|after|
|---|---|---|
|滚轮帧 p50 / max|10.00 / 11.77 ms|0.80 / 2.36 ms|
|选择变化当帧|≈10 ms|≈10 ms（单帧一次性重算，保留）|

### 排除项（有数据，不修）

- 空闲重绘：text/folder/table/home 空闲 5–6 s 仅 7–15 帧（间隔 p99 = 整个空闲期），无空转。
- 文本 110 MB：首帧 7.8 ms，滚轮 p50 0.87 / max 1.81 ms，过滤切换 ≤2.4 ms，导航 ≤1.9 ms，跳转 ≤1.8 ms；打开 288 ms + 后台对齐 872 ms（UI 不阻塞）。
- 文件夹 100k 加载 8–12 s 为后台扫描，期间 UI 帧 ≤3.6 ms。
- 文本视图分隔条为固定布局，无拖拽开销可测；同步视图本期不换肤，未触及。

### 基准回归（基线 `901e43b` worktree 构建 vs HEAD）

|基准|A p99|B p99|Δ|
|---|---:|---:|---:|
|`--bench` 文本（3 轮，后 2 轮为最终 HEAD）|1.518 / 1.386 / 1.355|1.372 / 1.402 / 1.415|−9.6% / +1.2% / +4.4%（<10%）|
|`--bench-table`|1.338|1.168|−12.7%|
|`--bench-dir`|21.323|0.533|−97.5%|

## Part B：UI 差距清单（按时刻可见程度）与处理

|#|差距|可见度|处理|
|---|---|---|---|
|1|文本左侧缩略图为旧导航条（全高色带、黄色当前行线）|始终可见|**已修**（`f1695d7`），见下|
|2|Differences/Same 折叠分隔行 15 px vs 参考 9 px|过滤时可见|已知差异：统一行高虚拟滚动（`show_rows`）前提，改为变高行需重写滚动/投影坐标，超出抓大放小|
|3|菜单栏缺 "New version available..." 链接|始终可见|已知差异：属更新通知，Matcha 无对应功能，不放假链接|
|4|工具栏/分隔条/窗格悬停、按下态|交互时|已知差异：REA 在 Windows 不支持原生 UI 观测，静态参考无悬停态，不臆造颜色|
|5|长文本缩略图多 2 行（179/180、199/200 被拆为删除+插入）|长文件可见|已知差异：引擎对齐行为（bc-engine-text），本波不得修改引擎|
|6|右窗格滚动条底部三个小导航钮（x≈1374,y≈815–857）|始终可见、极小|未做（琐碎）|
|7|字形级 AA 强度差|—|按纪律不计|

### 缩略图实现（参考实测值）

- 52 px 条：顶行白高光；凹陷轨道 x11..40（#A0A0A0 上/左边、白色下/右边）；内容 x16..39，左/右文件各 12 px；每行 1 px，超出可用高度按像素分桶取最重要标记。
- 差异墨色 = 偏好颜色向白插值 25.5%（默认红/蓝 → #FF4141 / #4141FF，与参考一致）；孤儿行用重要色（参考如此）。
- 视口外整体向 #F0F0F0 罩 20%（白→#FCFCFC，#FF4141→#FC6464，与参考实测一致）；视口框 #808080 x7..44；当前行 #0078D7 九行三角 x1..5（用无羽化 mesh，egui 会把 1×1 矩形羽化掉）。
- 视口行范围排除边缘折叠行与 `show_rows` 的超扫描行（参考框只覆盖实际显示的行）。
- 差异按行程存储（对齐后一次 O(N)），像素桶按轨道高度缓存，滚动帧 O(轨道高度)。点击/拖动按同一几何映射到行。

### 缩略图区（x0..51, y70..886）与参考差异像素数（单通道 >2）

|场景|before（901e43b）|after|
|---|---:|---:|
|text-basic|2181|**0**|
|text-diffs-filter|2183|**0**|
|text-cursor-on-diff|2129|**0**|
|text-long-scrolled|（新场景）|193，全部位于第 5 项对齐差异导致的行（y262–284、443–451）|

`compare.ps1` 对 thumbnail 区仍给出自动 FAIL（3,0,0,2）：其 ±8 px 边缘扫描窗口越出 52 px 条进入路径框/详情行。参考对自身运行 `compare.ps1` 该区为 PASS(0,0,0,0)，条带本身逐像素一致（上表），判定规则未改。

## 并排图（入库）

- 全场景并排（compare.ps1）：[text-basic](assets/visual-wave4-text-basic-side.png)、[text-diffs-filter](assets/visual-wave4-text-diffs-filter-side.png)、[text-cursor-on-diff](assets/visual-wave4-text-cursor-on-diff-side.png)、[text-long-scrolled](assets/visual-wave4-text-long-scrolled-side.png)
- 缩略图放大（参考 | before | after，3×）：[text-basic](assets/visual-wave4-text-basic-thumbnail-zoom.png)、[text-diffs-filter](assets/visual-wave4-text-diffs-filter-thumbnail-zoom.png)、[text-long-scrolled（参考 | after）](assets/visual-wave4-text-long-scrolled-thumbnail-zoom.png)

## 快照确定性

`snap-matcha.ps1 -All -Verify`（日志 `evidence/visual-parity-wave4/snap-verify.log`）9/9 `verified=True`。Part A 各提交后 8 个既有场景与基线 `901e43b` 构建逐字节相同（文本三场景基线哈希 82D32314…/406CB242…/1F78AD85…，文件夹三场景与 Wave 3 记录相同）；Part B 后文本场景哈希按预期变化（缩略图），文件夹/图标/chrome 场景不变：

```text
icon-catalog        A65C53F7…   folder-basic        BCFE37E8…
chrome-demo         532DC16A…   folder-expanded     86977D7E…
text-basic          5BF499C7…   folder-diffs-filter 060B1F8C…
text-diffs-filter   746A96F7…   text-long-scrolled  E7B376B1…
text-cursor-on-diff 31EEB946…
```

## 门禁输出

`cargo clippy --workspace --all-targets -- -D warnings`，exit 0：

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.39s
```

`cargo test --workspace`，exit 0：81 个测试目标，**434 passed / 0 failed / 5 ignored**（新增：文本/文件夹滚轮轨迹、树线点相位、状态栏统计失效、缩略图像素桶；2 个 ignored 为延迟探针）。

触及 Rust 文件 `rustfmt --edition 2021 --check --config skip_children=true` 全部无差异：`app.rs`、`chrome/{fonts,mod,wheel}.rs`、`dir_view.rs`、`lib.rs`、`perf.rs`、`snapshot.rs`、`table/grid.rs`、`text/{nav_bar,render,status_bar,view,view_tests}.rs`。

## 合规

未读写 ObsidianVault；引擎 crate 零修改；无新依赖；tokens/`compare.ps1` 未改；`snap-matcha.ps1` 仅新增场景名。参考工具：启动前确认无 BCompare 进程、按 `%TEMP%\matcha-reference-tool.lock` 约定加锁，REA 观测失败后立即关闭，清理后 `BCompareProcesses=0 lockExists=False`；未提取图标/位图，未读授权文件。
