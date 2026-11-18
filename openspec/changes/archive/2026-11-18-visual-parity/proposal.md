# Proposal

## Why

text-compare-core、folder-compare-sync、table-compare-xlsx 三条线行为已对齐参考工具，但 GUI 仍是 egui 默认外观：一排平铺按钮、默认字体、没有菜单栏/大图标工具栏/路径栏/信息头/行详情/状态栏，用户从参考工具迁移时第一眼就不对。用户要求**外观尽量一比一复刻参考工具**（默认英文、浅色主题、100% 缩放、默认字体与颜色），让熟悉参考工具的人零学习成本。

## What Changes

- 建立**视觉参考与回归基础设施**：固定语料、参考工具客户区截图采集（持互斥锁）、Matcha 确定性快照（`--snapshot=<scene>` + 固定客户区尺寸）、并排/差异热图/区域几何对照工具；度量结果落成 `docs/visual-parity/tokens.{md,json}`（颜色、几何、字体、菜单/工具栏/状态栏清单），作为本 change 的外观真源。
- 建立**外观基础层** `bc-gui/src/chrome/`：系统字体（Segoe UI / 编辑器等宽字体，CJK 回退）与 DPI、主题 token、经典 Windows 风格的菜单栏、大图标+文字工具栏（切换态、下拉分体按钮、分隔线）、路径组合框栏、信息头、分栏状态栏、分割条、滚动条、原创图标集。
- 逐视图 1:1 换肤：**文本比较**（菜单、工具栏、路径栏、信息头、行号/箭头槽、差异底色与红字、缺行斜纹、EOF 后网纹、缩略图栏、每侧光标状态、行详情、状态栏、标题）→ **文件夹比较/同步** → **Home 会话启动页 + 表格比较** → **对话框、弹出菜单与右键菜单**。
- 所有已实现行为保留；外观改造不得改变任何引擎结果，不得使文本/表格基准退化超过 10%。
- 不在本 change：十六进制/图片等尚无视图的比较类型、暗色主题、高 DPI 非 100% 的像素对齐（只保证可用）、原生 Win32 控件替换 egui。

## Capabilities

### New Capabilities

- `visual-reference-harness`：参考采集、Matcha 快照、对照工具与 token 真源。
- `ui-chrome-theme`：字体、DPI、主题 token 与共享外观组件（菜单栏、工具栏、路径栏、状态栏、图标集）。
- `text-view-appearance`：文本比较视图的逐区域外观。
- `folder-view-appearance`：文件夹比较与文件夹同步视图的逐区域外观。
- `home-table-appearance`：Home 会话启动页与表格比较视图的逐区域外观。
- `dialog-appearance`：会话设置、选项、查找/替换、文件格式、操作确认等对话框与弹出/右键菜单外观。

### Modified Capabilities

（无。已有 change 的行为需求不变；本 change 只约束其呈现。）

## Impact

- 代码：`crates/bc-gui`（新增 `chrome/`，各视图换肤）、`apps/bcompare/src/main.rs`（`--snapshot`/`--window-size`）、`tools/visual/`、`docs/visual-parity/`。
- 引擎 crate 零修改；不新增第三方依赖（若 SVG/图标渲染确需依赖，先 `ask`）。
- 红线：不提取、不描摹参考工具的图标/位图资源；产品内不出现参考工具名称；不读取任何授权/密钥文件；参考截图仅作内部工程证据（私有仓库）。
