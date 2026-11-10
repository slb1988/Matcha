# Design

## Context

- 现状：`bc-gui` 用 egui 0.37 默认 Visuals；各视图自行在顶部摆按钮行（见 `docs/worker-reports/assets/folder-integration-text.png`）；已有 `--screenshot=<file>` 帧缓冲截图（`app.rs` `request_screenshot` → `bench::save_screenshot`）。
- 参考：本机 `D:\Program Files\Beyond Compare 5`（5.0.6，默认英文、浅色、默认偏好）。协调者已采一张种子截图 `tmp/visual-seed/text-basic-window.png`（含标题栏的整窗，1400×1104，语料 `tmp/visual-seed/{l,r}.txt`），可作初值参考，正式真源以任务组 1 产出为准。
- 种子截图观察到的结构（自上而下）：菜单栏 `Session File Edit Search View Tools Help`（右侧可能有链接文字，不复刻更新提示）；大图标工具栏（图标在上、文字在下，切换态浅蓝底+蓝框，分体按钮带 ▾，组间竖分隔线）：`Home | Sessions▾ | All Diffs Same | Context Minor | Rules Format▾ | Copy | Next Section Prev Section | Swap Reload`；每侧路径栏（可编辑组合框 + 历史/浏览▾/保存图标按钮）；每侧信息头（修改时间、字节数、格式▾、编码▾、行尾）；左侧窄缩略图栏；编辑窗格（行号槽 + 差异箭头槽，差异行粉底、差异字符红字，缺行斜纹，EOF 后网纹，窗格灰底 `#F0F0F0` 类）；每侧窗格状态（`1: 1`、`Default text`）；行详情两行（➪ 左 / ⇦ 右，含可见行尾符）；底部状态栏（差异图标 + `N difference section(s)`、`Same`、`Insert`、`Load time: x seconds`）。

## Goals / Non-Goals

**Goals:** 默认英文浅色 100% 缩放下，同语料同客户区尺寸的截图逐区域对齐；颜色 token 精确；菜单/工具栏/状态栏条目与顺序一致；可由脚本回归。

**Non-Goals:** 字形逐像素一致（DirectWrite ClearType 与 egui 灰度栅格化不同）；暗色主题；非 100% DPI 的像素对齐；暂无视图的比较类型。

## Decisions

### D1. 对照单位是"客户区截图 + 命名区域"，不是整窗像素

- 参考侧：脚本启动参考工具、`MoveWindow` 到固定外框后用 `GetClientRect`/`ClientToScreen` 裁客户区（`PrintWindow` PW_RENDERFULLCONTENT；弹出菜单等 `PrintWindow` 拿不到的用屏幕拷贝）。Matcha 侧：`--snapshot=<scene>` 用帧缓冲截图，`--window-size=WxH` 设客户区同尺寸。
- 每个场景在 `tokens.json` 里登记命名区域（如 `menubar`、`toolbar`、`pathbar.left`、`info.left`、`gutter.left`、`pane.left`、`details`、`statusbar`）的参考矩形；对照工具从 Matcha 截图中按同一规则测出区域边界（背景色/分隔线检测）并给出偏差。
- 验收：区域边界 ±2 px；token 颜色在非抗锯齿像素上精确一致（实现上允许 ΔRGB ≤ 2）；整图差异热图供人工评审，不设整体像素阈值。

### D2. token 真源：`docs/visual-parity/tokens.json`

- 结构：`colors`（命名颜色 → hex）、`metrics`（行高、工具栏按钮宽高、图标尺寸、分隔线、内边距、各栏高度）、`fonts`（UI 字体/字号、编辑器字体/字号、行号字体）、`inventories`（每视图菜单树含快捷键与分隔线、工具栏条目/类型/默认切换态、状态栏字段、右键菜单）、`scenes`（场景 → 语料、操作步骤、区域矩形）。
- `bc-gui/src/chrome/theme.rs` 以常量镜像 tokens.json（不运行时读文件）；单测断言二者一致，防漂移。

### D3. 字体：运行时加载系统字体，不随包分发

- UI：`C:\Windows\Fonts\segoeui.ttf`，按参考度量的字号（预期 9pt = 12 px @96 DPI）；编辑器：按参考 Options 中默认编辑器字体与字号（任务组 1 确认）；CJK 回退 `msyh.ttc`。找不到系统字体时回退 egui 内置字体并记日志，不 panic。
- `pixels_per_point` 跟随系统缩放；快照模式强制 1.0。
- egui 灰度栅格化与 ClearType 的差异记为已知差异，不追求字形像素一致；但字号、基线、行高、字距度量必须一致（影响区域几何）。

### D4. 共享外观组件 `bc-gui/src/chrome/`

- `theme.rs`（token、全局 `Visuals`/`Style`：面板灰底、1 px 边框、无圆角、经典选中蓝）、`fonts.rs`、`menubar.rs`（顶层菜单 + 下拉，助记符下划线按 Alt 显示，快捷键右对齐，分隔线，勾选/单选标记）、`toolbar.rs`（大按钮：图标上文字下；普通/切换/分体▾/禁用；分组分隔线；悬停与按下态）、`pathbar.rs`（可编辑组合框 + 图标按钮）、`statusbar.rs`（分栏、凹陷分隔）、`splitter.rs`、`icons.rs`（原创图标）、`scrollbar.rs`（经典样式）。
- 各视图只声明"本视图的菜单树/工具栏条目/状态字段"数据并接到既有动作，不各自画按钮。现有按钮行的功能全部迁到菜单或工具栏，迁移清单写进报告，任何功能不得因换肤丢失入口。

### D5. 图标：原创矢量，槽位一致

- 用 egui `Painter` 程序化绘制（或项目内自绘 SVG 交由已有依赖渲染），尺寸、槽位、主色系与参考一致，语义隐喻可通用（房子、公文包、星号、≠、=、≈、箭头），**不提取参考工具资源位图，不逐像素描摹**。
- 图标目录场景 `--snapshot=icon-catalog` 用于评审。

### D6. 确定性快照

- 场景注册表 `bc-gui/src/snapshot.rs`：场景名 → 语料路径、视图初始化、预置操作（展开、选择、切换过滤、打开菜单），等待后台比较完成后截图；禁用光标闪烁与动画；时间戳显示使用语料文件真实 mtime（采集脚本把语料 mtime 固定为 `2026-01-02 03:04:05`）。
- `tools/visual/snap-matcha.ps1` 批量产出 `target/visual/matcha/<scene>.png`；`tools/visual/compare.ps1` 产出 `target/visual/compare/<scene>-{side,heat}.png` 与 `report.md`。

### D7. 波次与并行

- Wave 1：任务组 1（独立 worktree 分支 `sunlaibing/visual-ref`，只写 `tools/visual/`、`docs/visual-parity/`）与任务组 2（master）并行；组 2 先用种子截图取初值。
- Wave 2：合并组 1 后，master 上做组 3（文本视图）并按 tokens 校准组 2 的初值。
- Wave 3：组 4（文件夹，独立 worktree 分支）与组 5（Home+表格，master）并行，各自只动自己的视图文件，`app.rs` 只做最小接线；组 4 结束合回 master。
- Wave 4：组 6（对话框/菜单）与组 7（收尾回归与对照报告）。
- 同一 worktree 任一时刻只允许一个提交者；参考工具调用一律持 `%TEMP%\matcha-reference-tool.lock`。

### D8. 不变量

- 引擎 crate 与 `bc-core`/`bc-session` 零修改；行为回归测试全部保留并通过。
- 文本 `--bench` update p99、表格 `--bench-table` update p99 相对换肤前不退化超过 10%（同轮 A/B）。
- `cargo clippy --workspace --all-targets -- -D warnings` 零警告，`cargo test --workspace` 全绿。

## Risks / Trade-offs

- egui 不是原生控件：组合框、滚动条、菜单阴影只能近似；以区域几何与颜色为准，交互细节以参考行为为准。
- 字形栅格化差异会让热图在文字处始终有噪点：评审看并排图，热图只用来找几何/颜色偏差。
- 参考工具采集依赖解锁的交互桌面；锁屏时采集任务应 `ask` 而非绕过。
