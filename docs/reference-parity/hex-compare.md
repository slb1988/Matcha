# Hex Compare 公开行为对照

## 范围与证据口径

本清单是 `hex-compare` change 的参考行为真源，证据来自两类只读采集：

1. **运行时截图**（本目录 `*.png`，`*-annotated.png` 为编号标注版）：在 Beyond Compare 5（Windows 5.x，PE x86_64，SHA-256 `75ea6977…`）上用构造语料实际驱动得出。语料已随本目录提交（`corpus/`，含确定性生成器 `gen_corpus.py`，SHA-256 校验可复现）：
   - `corpus/left.bin`/`corpus/right.bin`：4,160 与 4,144 字节，含 8 处单字节改动（0x100/0x104/0x108、0x200/0x204/0x208/0x20C、0x300）、16 字节插入（0x400，`00..0F`）、32 字节删除（原始坐标 0x5F0–0x60F）。
   - `corpus/l2.bin`/`corpus/r2.bin`：2,048 与 2,053 字节，含 5 字节插入（0x123，`AA BB CC DD EE`）——故意取非 16 倍数长度验证行打包模型。
2. **CHM 帮助只读解包**（7z 解 `BCompare.chm`，主题 `commandshex.html`、`sessionhex*.html`、`formathex*.html`、`viewhex.html`、`scripting_reference.html`）。命令清单逐段保留原顺序，含隐藏标记 `*`（默认隐藏）。

边界声明：

- REA 静态核对已完成（macOS 版 5.0.4.30422 arm64 切片，符号表 + RTTI + 反汇编；该版为 Lazarus/FPC 构建，未裁剪符号），结论见文末「后续补充项」。Windows PE 侧分析因超时未完成，由 macOS 证据替代；两侧为同一代码库，类名与算法常量一致。
- 首次采集中一个实例因按键误投产生过内存态字节污染（0x410 处渲染异常），该实例其余截图仅保留已逐字节核对过的可视区域（`01-main.png`，0x000–0x220 区间与生成器一致）。后续采集均在干净实例上完成。
- 截图仅作内部工程证据（私有仓库）；不把参考工具图标/位图描摹进产品。

## 关键行为模型（运行时证据）

### 渲染模型：对齐图按"单元格"打包成行

**结论（由 `24-odd-insert-all.png` 证实）**：左右两栏不是各自独立按文件偏移分行，而是把字节对齐图按每行 N 格（默认 16，可配）打包：

- 配对字节同格相邻；一侧的孤儿字节占格，另一侧同位置画**斜纹占位格**（可出现在行中间，见标注 ①②）。
- 每侧行标签 = 该侧本行**首个真实字节**的文件偏移；某行该侧全是占位格时标签留空（整行斜纹，见 `06-insertion-closeup.png` ②）。
- 奇数长度插入后，后续行的地址相位可整体偏移（左栏插入点后按 …B 相位分行，右栏保持 16 对齐，见 ③④）；Diffs 模式下各差异区域连同上下文的单元格**连续流入同一打包序列**（区域不打断行相位：左栏标签 0x1F7 = 前一区域 9 格余量与后一区域前 7 格同行，见 `22-diffs-only.png` ④）。
- 状态栏左右角各有一个**当前字节地址**（如左 `00000123` / 右 `00000128`），光标跨侧同步到同一逻辑字节（`23-odd-insert.png` ④）。

### 字节状态四分类

`20-info.png`（Hex Compare Info）给出权威分类与计数：**same / left orphan / right orphan / difference**。与语料精确吻合：4,120 same + 32 left orphan + 16 right orphan + 8 difference（左右各自合计 = 文件大小）。差异字节红底红字（RGB 约 255,227,227 底色），孤儿字节同样红色；ASCII 列同步着色。状态栏汇总文案为 `Binary differences` / `Binary same`。

### 对齐策略（Comparison 页，`17-settings-comparison.png`）

- **Complete**（默认）：复杂算法做全量字节对齐。
- **Fast**：简单快速的启发式，面向超大文件。
- **None**：逐字节位置对齐（offset 对 offset）， orphan 只出现在 EOF 截断处。
- "Files up to this size are loaded without file locks"（默认 2 MB）：小文件整载入内存、不加文件锁（允许外部编辑）；大文件加锁/流式。

### 视图与交互要点（截图 + CHM 一致）

- 布局：Side-by-Side（默认）/ Over-Under；缩略图条（Thumbnail）、File Info 头、工具栏可开关（`13-menu-view.png`）。
- 过滤：Show All / Show Differences / Show Same；Diffs 模式带上下文字节（Layout 对话框 `Context count`，默认 16，`21-layout.png`）。
- 布局：Bytes per group（默认 8，8+8 分组显示）、Groups per item、Fit to window（默认开）、Context count。
- 导航：Next/Previous Difference Bytes（Ctrl+N / Ctrl+P）；Go To（字节地址，Decimal/Hex、Left/Right 侧选择，`18-goto.png`）。
- 查找：内联查找栏（非对话框），Prev/Next、大小写开关、hex/文本模式切换（`19-find.png`）；另有 Replace（Ctrl+H）。
- 编辑：Insert/Overwrite 模式指示（状态栏 `Insert`）；Cut/Copy/Paste/Delete；Select All；**Copy to Right/Left/Other Side（Ctrl+R，按选中字节复制到对侧）**；Undo/Redo。
- View 菜单独有：**Current Byte Address**（显示当前字节地址）、**Little/Big Endian Values**（把选中字节解释为 LE/BE 数值列表）。
- 会话设置三页（`15/16/17`）：Specs（左右路径、Disable editing、Description、换侧按钮、作用域下拉 "Use for this view only"）、Format（左右文件格式，"Detected (Everything Else)"，Same as left file format）、Comparison（对齐策略 + 文件锁阈值）。
- Hex Compare Report（`26-report.png`）：Side-by-side/Summary/Interleaved × All/Differences/Same，Include byte addresses，输出 Printer/HTML/Plain text。脚本侧对应 `hex-report layout:... options:display-all|display-mismatches|display-matches,line-numbers output-to:...`（`scripting_reference.html`）——**黄金语料可用其生成**。

## 命令逐项：`commandshex.html`

状态列沿用既有约定：本 change / 已批准后置 / 范围外。

### Session

| 编号 | 原帮助段落 | 状态 | 说明 |
|---|---|---|---|
| H01 | `Save Session` / `Save Session As` | 已批准后置 | 随 session-and-cli 统一会话持久化；菜单可置灰。 |
| H02 | `Session Settings` | 本 change | 三页对话框（Specs/Format/Comparison），作用域下拉仅 "Use for this view only"（全局默认随持久化后置）。 |
| H03 | `Locked` | 已批准后置 | 会话锁定，随统一会话命令评审。 |
| H04 | `Clear Session` | 已批准后置 | 同上。 |
| H05 | `Swap Sides` | 本 change | 左右互换并重比。 |
| H06 | `Reload Files` | 本 change | 重读两侧，脏时先提示保存。 |
| H07 | `Recompare Files` | 本 change | 保留编辑、丢弃可撤销性，重新对齐。 |
| H08 | `Hex Compare Report` | 已批准后置 | 报告输出统一走后续报告 change；脚本 `hex-report` 仅用于黄金语料。 |
| H09 | `Hex Compare Info` | 本 change | 四分类字节统计对话框。 |
| H10 | `Compare Files Using` | 部分 | 与 Text/Table Compare 互切；Version/Picture 等其他类型范围外。 |
| H11 | `Compare Parent Folders` | 已批准后置 | 父文件夹联动随文件夹线已有约定后置。 |

### File

| 编号 | 原帮助段落 | 状态 | 说明 |
|---|---|---|---|
| H12 | `Open File` | 本 change | 单侧换文件（路径框）。 |
| H13 | `Open File on File System *` / `Open File Using Profile *` | 能力合并 / 范围外 | 本地能力并入 Open File；VFS profile 范围外。 |
| H14 | `Open Clipboard` | 本 change | 剪贴板内容载入一侧。 |
| H15 | `Open With` / `Associated Application` / `Text Edit` | 已批准后置 | 外部程序统一后置。 |
| H16 | `Save File` / `Save File As` | 本 change | 单侧保存（含暂存+改名落盘）；只读/禁编辑侧拒绝。 |
| H17 | `Save File on File System As *` / `Save File Using Profile As *` | 能力合并 / 范围外 | 同上。 |
| H18 | `Explorer` | 已批准后置 | 文件管理器联动后置。 |

### Edit

| 编号 | 原帮助段落 | 状态 | 说明 |
|---|---|---|---|
| H19 | `Undo` / `Redo` | 本 change | 字节编辑历史。 |
| H20 | `Copy to Right/Left`（`Copy to Other Side`） | 本 change | 选中字节复制到对侧对齐位置（Ctrl+R）。 |
| H21 | `Cut / Copy / Paste / Delete` | 本 change | 标准编辑；插入模式影响语义。 |
| H22 | `Select All` | 本 change | 选中当前窗格全部可见字节。 |

### Search

| 编号 | 原帮助段落 | 状态 | 说明 |
|---|---|---|---|
| H23 | `Next/Previous Difference Bytes` | 本 change | 差异字节导航（Ctrl+N/Ctrl+P）。 |
| H24 | `Next/Previous Difference Files`、`Copy File to * and Open Next Difference` | 已批准后置 | 仅子会话；随父文件夹联动后置。 |
| H25 | `Find` / `Replace` / `Find Next` / `Find Previous` | 本 change | 内联查找栏，hex/文本两种模式。 |
| H26 | `Go To` | 本 change | 按字节地址定位（十进制/十六进制、选侧）。 |

### View

| 编号 | 原帮助段落 | 状态 | 说明 |
|---|---|---|---|
| H27 | `Show All / Show Differences / Show Same` | 本 change | 显示过滤；Diffs 带上下文。 |
| H28 | `Current Byte Address` | 本 change | 显示当前字节地址（子菜单）。 |
| H29 | `Little Endian Values` / `Big Endian Values` | 本 change | 选中字节的数值解释列表。 |
| H30 | `Byte Addresses` | 本 change | 地址列开关。 |
| H31 | `Display Font *` 及字号三项 | 已批准后置 | 全局字体设置统一后置。 |
| H32 | `Layout` | 本 change | Bytes per group / Groups per item / Fit to window / Context count。 |
| H33 | `Side-by-Side / Over-Under Layout` | 本 change | 窗格布局切换。 |
| H34 | `Thumbnail` / `File Info` / `Toolbar` | 本 change | 三个显示开关。 |

## 后续补充项

### REA 静态核对结论（2026-11-11，macOS 5.0.4.30422 arm64，`BCompare` 切片 SHA-256 `a1bad5f7…`）

分析工件在 `tmp/hex-ref-mac/`（未提交脚手架，随机器临时目录；结论已全部落在本文档）。

1. **类名映射全部证实**（FPC mangled 符号 + RTTI）：`THexContent`（HEXCONTENT 单元）、`THexMap`、`THexMapAlign`、`THexMapByteAlign`（HEXMAP 单元）、`THexRules`（HEXRULES 单元，`SetAlignKind(THexAlignKind)`）、`THexCompareSession`、`THexFiltered`。注意 `THexMapByteAlign` 与 `TBcByteAlign`（BCALIGN 单元）是**两个并存类**：Complete 的小区间字节级细化实际调用 `TBcByteAlign.Run(job, [左段, 右段])`。另有完整辅助生态可对名：`THexSideBySideLayouter`/`THexSummaryLayouter`/`THexInterleavedLayouter`（正好对应报告三布局）、`THexActions`/`THexCompareActions`/`THexCompareCommands`、`THexFind*`、`THexMap*Item` 编辑操作族、`THexSession*`/`THexView*`。
2. **对齐策略枚举与算法边界**（反汇编 `THEXMAPALIGN.RUN` 及其嵌套例程证实）：
   - `THexAlignKind = (alignNone=0, alignFast=1, alignComplete=2)`（RTTI o2s 表逐字证实）；None → 全文件 `RUNNONE` 逐位；Fast/Complete 共享同一锚块主干（64 KB 分块读取；**50 字节锚窗**，Rabin 滚动哈希 base 257、mod 2¹⁶——`EXPMOD(257,50)` 逐指令验证；EOF 侧 50 字节余量；mismatch 前先输出公共前缀 same 段）。
   - **Complete**（默认）：mismatch 区间两侧均 **< 5000 字节** → `TBcByteAlign` 字节级 diff；任一侧 ≥ 5000 → `RUNRANGE` 锚点重同步（递归二分 + `SEQFIND` 顺序搜索，**skew = 10**，第 5 参数立即数证实）。
   - Matcha 对应实现（2026-11-12）：`bc_core::align::resync_gap`——沿对角线逐字节配对（same/difference），失配时在 ±10 字节内找连续 4 字节相等作重同步点，跳过字节为孤儿，尾部余量为孤儿。最短匹配长度 4 为 Matcha 取值（参考未核对）；递归二分未复刻。实测两张不同 PNG（154625 / 420870 字节）开头 0x30 字节的配对与参考截图逐字节一致；参考在随机区段内重同步更频繁（孤儿段更多），属已知差异。
   - **Fast**：mismatch 区间不做字节级细化，直接 `RUNNONE` 逐位对齐（即锚块之间按位置对应）。
   - skew 值 10 为硬编码、UI 不暴露；设计 D6「skew_tolerance 归入 Complete 区间上限参数」维持不变，校准值以此为准（5000 为字节级细化上限、10 为大区间重同步 skew）。
3. **View 菜单两个子菜单完整项集合**（macOS 5.0.4 运行时 AX 枚举，zh-CN 界面；与截图 `13-menu-view.png` 互证）：
   - `Little Endian Values` / `Big Endian Values` 各 5 项：`1 字节`（负值时带括号有符号表示，如 `165 (或 -91)`）、`2 字节`、`4 字节`、`浮点单精度`、`浮点双精度`；数值随当前光标位置实时计算填入标签。
   - `字节地址 = 00000000`（Current Byte Address）为动态标签菜单项，显示当前字节地址。
   - View 菜单全序：显示全部/显示差异/显示相同 ｜ 字节地址=xxx ｜ 小尾值 ▸ ｜ 大端字节序值 ▸ ｜ ─ ｜ 字节地址 ｜ 布局... ｜ ─ ｜ 边并排布局 / 上-下布局 / 缩略图 / 文件信息 / 工具栏。
4. **`hex-report` 脚本 token 集**（字符串表证实）：`layout:side-by-side|summary|interleaved`；`options:display-all|display-matches|display-mismatches|display-mismatches-no-orphans|line-numbers`（`display-mismatches-no-orphans` 为 CHM 未载、二进制实有多出的选项）；报告对话框控件集与之一一对应（布局三选、选项下拉、包含字节地址、报告标题、输出=打印机/HTML/纯文本、加亮差异=使用颜色、折行=无/词组/字符，纯文本默认折行字符）。**输出文本样例未采集**：本机 BC 为 Standard 版（`IsPro=0`），`@script` 被禁（exit 107），GUI 报告采集因用户占用机器中止；不影响 tasks 3.2——黄金期望本就约定在 Windows Pro 实例用脚本生成。

- `shot_hexcompare.png`（CHM 官方截图）与上述运行时截图行为一致，可作为交叉佐证。

## 实现对照补记（2026-11-12，hex-compare 任务 7.3）

Matcha 快照场景 `hex-basic` / `hex-insertion` / `hex-diffs-filter`（无头采集，与本目录语料同字节）与参考截图 `01-main.png` / `06-insertion-closeup.png` / `22-diffs-only.png` 逐区域对照：命令结构、行打包、着色、占位格、行标签相位、过滤行为全部吻合；每行格数随 Fit-to-window 窗口宽度（参考截图 16 格源于其窗口宽度，行为一致）与光标格着色框为已知几何差异。缩略图列（最左 52 px、红色像素横线、视口框拖动定位）与每窗格右缘 16 px 竖滚动条已按参考实测补齐（2026-11-12 后续，见 worker-report 补记）；过滤模式下缩略图映射当前显示行（参考映射全文件坐标）为已知偏离。细节见 `openspec/changes/hex-compare/worker-report.md`。

缩略图列后续调整（2026-11-12）：迷你图内框加宽为列内 x=10..44（有意宽于参考实测 16..40）；视口框对称越过内框两侧各 5 px（x=5..49）、2 px 描边、最小高度 9 px（占比极小时保持框形），颜色与参考一致（灰 128）；并补齐参考的列左缘蓝色当前位置三角指针（亮色 #0078D7 / 暗色 #3199FF）。列宽 52 px、红色像素横线语义与点击/拖动定位不变。
