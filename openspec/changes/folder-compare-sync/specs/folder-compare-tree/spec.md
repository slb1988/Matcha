# Spec Delta

## Purpose

定义文件夹比较的双树模型与用户可感知行为：两侧条目如何按名配对、状态颜色含义、后台扫描与展开、导航与查找、视图列与图例，使体验与参考工具的 Folder Compare 一致。

## ADDED Requirements

### Requirement: 按名配对的双树
系统 SHALL 并排显示左右两个基准文件夹的树，同名条目（文件或文件夹）出现在同一行；只在一侧存在的条目为孤儿，另一侧留空。Windows 上名称比较默认不区分大小写，可在准则中开启"比较文件名大小写"。

#### Scenario: 同名文件配对
- **WHEN** 左右都有 `src/main.rs`
- **THEN** 两者出现在同一行，父文件夹 `src` 也在同一行

#### Scenario: 孤儿文件
- **WHEN** 仅右侧有 `notes.txt`
- **THEN** 该行左侧为空，右侧条目以孤儿颜色显示

#### Scenario: 大小写不同的名称
- **WHEN** 左侧 `README.md`、右侧 `readme.md` 且未开启文件名大小写比较
- **THEN** 两者配对为同一行

### Requirement: 配对规则扩展
系统 SHALL 支持"对齐不同扩展名的同名文件"、"对齐 Unicode 规范化形式不同的名称"两个开关，以及会话级对齐覆盖（左右名或掩码对，可正则，可限定相对文件夹）和 Align With 手动对齐。

#### Scenario: 扩展名对齐
- **WHEN** 开启扩展名对齐，左侧 `logo.gif`、右侧 `logo.png`
- **THEN** 两者配对为同一行并按内容准则比较

#### Scenario: 对齐覆盖掩码
- **WHEN** 覆盖规则为 `x*.jpg` ↔ `y*.jpg`
- **THEN** `x1.jpg` 与 `y1.jpg` 配对，`x2.jpg` 与 `y2.jpg` 配对

### Requirement: 状态与颜色语义
每个条目 SHALL 归为以下状态之一并按默认颜色显示：未知/较旧（灰）、相同（黑）、孤儿（紫）、较新或不同（红）；被过滤但因对侧可见而显示的条目为 teal。文件夹颜色 SHALL 汇总其内容：含较新/不同则红，含孤儿则紫，两者都有则同时体现；未扫描完成的文件夹显示为空心，自动扫描关闭且未展开的文件夹显示为黄色。中央列显示内容比较结果图标。

#### Scenario: 文件夹汇总颜色
- **WHEN** 某文件夹内有一个右侧较新的文件和一个左侧孤儿
- **THEN** 该文件夹同时显示红与紫的提示

#### Scenario: 内容结果覆盖
- **WHEN** 时间戳不同但内容比较判定相同且"内容结果覆盖快速测试"开启
- **THEN** 该行按"相同"着色，中央列显示相同图标

### Requirement: 后台扫描与展开
加载会话后系统 SHALL 在后台递归扫描子文件夹以便着色；可配置"加载时展开子文件夹"与"仅展开有差异的子文件夹"；提供 展开/折叠、展开/折叠所有子文件夹、Expand All / Collapse All。顶层孤儿文件夹默认不扫描，可配置扫描以报告大小。

#### Scenario: 仅展开差异
- **WHEN** 开启"仅展开有差异的子文件夹"并加载
- **THEN** 只有包含不同/孤儿条目的文件夹被展开

#### Scenario: 扫描中的显示
- **WHEN** 后台扫描尚未到达某文件夹
- **THEN** 该文件夹显示为空心，扫描完成后变为汇总颜色

### Requirement: 基准文件夹操作
系统 SHALL 提供 Swap Sides、Back/Forward（基准文件夹历史）、Browse for Folder（左/右）、Up One Level（左/右/双侧）、Set as Base Folder / on Other Side / Set as Base Folders、Compare Parent Folders，以及从比较打开 Folder Sync（Sync Base Folders）。

#### Scenario: 上一级
- **WHEN** 用户对双侧执行 Up One Level
- **THEN** 两侧基准文件夹变为各自父目录并重新比较，Back 可回到之前的基准

### Requirement: 打开与比较条目
双击或 Open SHALL 在新视图比较同一行的两个文件（视图类型由文件格式决定）；选中两个任意文件执行 Open 比较它们；Compare To 允许先选一项再点另一项；Open With 支持 Text Edit / Text Patch / 关联程序；Quick Compare 对所选文件弹出快速比较结果。

#### Scenario: 跨行比较
- **WHEN** 用户 Ctrl 选中左侧 `a.txt` 与右侧 `b.txt` 后执行 Open
- **THEN** 新视图以两者为左右打开

### Requirement: 选择命令
系统 SHALL 提供 Select All、Select All Files、Select Newer（左/右/双侧）、Select Orphans（左/右/双侧）、Invert Selection；点击中央列同时选择两侧；Shift+方向键把选区限定到一侧。

#### Scenario: 选择右侧较新
- **WHEN** 用户执行 Select Newer > Right Side
- **THEN** 所有可见的右侧较新文件被选中，其他不选

### Requirement: 导航与文件名查找
系统 SHALL 提供上一/下一差异、Find Filename（大小写、全词、正则、环绕、向上、指定侧）、Find Next/Previous Filename，以及 Refresh / Full Refresh / Refresh Selection。

#### Scenario: 查找文件名
- **WHEN** 用户查找 `*.lock` 的正则 `\.lock$` 并指定右侧
- **THEN** 定位到右侧第一个匹配的条目，Find Next 继续

### Requirement: 列、图例与日志
视图 SHALL 支持可选列（大小、时间戳、CRC、版本、属性、组、所有者、VCS），点击列头排序，右键列头选择显示列；提供 Legend 窗口、Log 面板、Toolbar 开关；Windows 上文件名后以括号显示 DOS 属性字母（r/h/a/s）。

#### Scenario: 按大小排序
- **WHEN** 用户点击"大小"列头
- **THEN** 当前文件夹内条目按大小排序，再次点击反向

### Requirement: 大目录性能
系统 SHALL 在 10 万文件的目录树上秒级显示树结构，内容比较在后台进行且可取消，界面保持可响应。

#### Scenario: 10 万文件
- **WHEN** 加载两侧各约 10 万文件的树
- **THEN** 顶层在 1 秒内可见，滚动与展开不出现可感知卡顿，后台比较进度可见且可取消
