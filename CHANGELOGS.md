# CHANGELOGS

本文件记录 Matcha 的用户可感知变动（功能、行为、修复）。每次变动追加一条，最新在最上。

## 2026-11-15

- **窗格分隔线可拖动并记住位置**（`docs/features/resizable-splitters.md`）：文本 / 表格 / Hex 比较的左右窗格（上下布局时为上下窗格）、
  文件夹与同步视图的左右两棵树（中缝两侧边缘都可拖）和底部日志面板、文本合并与表格合并上排三列之间的两条分隔线及输入区 / 输出区分界、
  Home 会话树宽度，现在都可以用鼠标拖动调整；悬停时光标变为调整形状，双击分隔线恢复默认。松开鼠标即写入
  `%APPDATA%\Matcha\layout.txt`，再次启动保持上一次的位置（同类视图的所有标签页共用；并排与上下布局各记一份）。
  每侧保留最小尺寸，窗口缩小时不会把某侧挤没。未拖动时布局与之前逐像素一致；快照 / 基准模式不读不写该文件。
- **文件夹 / 同步视图路径框跟随窗口宽度**：路径组合框（及同步视图的路径框、交换按钮）原为写死的像素位置，窗口变窄时会越界；
  现在随左右树分隔位置伸缩，默认窗口下尺寸不变。
- **Hex 比较**：并排布局的两条地址栏改为对齐各自窗格（此前两等分，与带缩略图列的窗格错位）；修复 Over-Under 布局下窗格
  从文件中段开始绘制、与上窗格不同行的问题。

- **表格 Text Details 对照条可交互**（openspec `table-details-interaction`）：网格下方的当前行对照条原来只显示、不响应鼠标，现在：
  拖拽列头边界调整列宽、双击边界按当前行内容自动适配（只改对照条，未调整的列仍跟随上方网格；Resize Columns to Fit、
  重新加载、切换 sheet 时恢复）；内容超宽时底部出现横向滚动条，鼠标滚轮在对照条上只滚对照条，光标列变化时自动滚到可见；
  单击某侧单元格切换到该侧并选中该格（键盘焦点交给上方该侧网格、网格横向滚到该格），Shift+单击行内扩选，单击列头换列，
  双击编辑，右键弹出同一表格菜单，Align With 待选时也可点对照条；悬停被截断或含换行的单元格显示完整内容。
- **修复：表格网格点击后的选中色**：点击 / Shift+单击单元格后，选中格（含多格选区）现在以选中色显示；此前只有焦点框，
  多格选区不可见。
- **右键菜单补齐**：表格网格新增右键菜单（Align With、Copy to Left/Right、Copy Cell to…、Cut/Copy/Paste/Delete、
  插删行列、Modify…、Open File / Open Clipboard / Open With / Save / Explorer），右键先选中所点的格；文件夹树右键菜单
  从 3 项扩充为完整清单（Open Folder / Open、Open/Close Subfolders、Set as Base…、Open in New View、Open With、Compare To…、
  Compare Contents、Copy/Move to Right|Left、Copy/Move to Folder、Delete、Rename、Attributes、Touch、Exclude、New Folder、
  Copy Filename、Ignored、Refresh Selection、Synchronize、Explorer），右键点在选区外时先选中所点一侧。条目与顺序按参考菜单，
  未实现的（Synchronize ▸、Open With ▸ Folder Sync）显示为禁用。表格 File 菜单的 Open With ▸ Associated Application 与 Explorer 现可用。
- **Session Settings 统一**：表格、文件夹/同步、Hex 的会话设置改用与文本相同的对话框（下划线页签、页面卡片、底部按钮行）。
  文件夹会话设置原来是一张可滚动的长表，各段各有一个应用按钮；现在按参考分为 Specs / Comparison / Handling / Name Filters /
  Other Filters / Misc 六页，改动先存草稿，OK / Apply 才生效（只重发变化的部分：过滤、规则重扫、换文件夹重载），Cancel / Esc 丢弃。
  表格会话设置为 OK / Cancel / Apply（原 "Apply to current comparison" 按钮）。
- **sheet 标签换新**：表格比较的 sheet 标签从状态栏下方移到网格与状态栏之间，改为下划线页签加比较状态点；表格合并共用同一组件。
  工作簿一侧的文件信息行不再显示分隔符（原来总显示 "Tab"）。
- **文件操作对话框文字化**：标题与选项不再显示内部名称（CopyTo、All、None、CopyOther），改为 Copy to Folder、Both sides、
  Keep relative folder structure、Copy to Other Side 等。
- **开发工具**：`tools/visual/snap-matcha.ps1 -All` 改为从 `matcha /?` 读取全部快照场景（原写死 17 个，现 28 个），并兼容
  Windows PowerShell 5.1；新增快照场景 `dlg-table-session-settings`、`dlg-folder-session-settings`。
- **visual-parity 收口（仓库管理）**：按用户 11-15 的决定（现代设计系统、非经典 Windows 复刻）修订规格为“结构对齐参考、样式走设计系统”，
  tasks 23/23。

## 2026-11-15

- **OpenSpec 归档（仓库管理，无功能变化）**：table-merge 归档至 `openspec/changes/archive/2026-11-15-table-merge`，
  新建主 spec `table-merge-cli`、`table-merge-engine`、`table-merge-view`；text-merge 带 4 项未完成的验证任务（参考样本采集、
  P4V 人工端到端）归档至 `2026-11-15-text-merge`，新建主 spec `text-merge-cli`、`text-merge-engine`、`text-merge-view`、
  `three-way-alignment`，合并语义仍为未经参考样本确认的暂定语义。剩余活跃 change：visual-parity（外观方向定为现代风设计系统）。

- **表格合并：删除输出行**：输出网格里 `Ctrl+Delete`（Edit ▸ Delete Output Row）把选中行从合并结果中去掉，
  该行上的冲突计为已解决，可撤销；之后对该行 Take 任一侧会把它带回。双增冲突行不可删除（用 Take Center 表示两行都不要）。
- **表格合并：横向滚动停在整列边界**：输入窗格较窄、跳到靠右的改动格时，左侧不再露出被截去一半的格子
  （此前只露出半截文字，紧贴行号像乱码）。
- **文档修正**：表格合并的示例命令（feature 文档、CLI 速查）仍是旧的 `theirs yours base` 顺序，按 11-14 的 P4V 顺序
  改为 `base theirs yours [output]`；CLI 速查里"xlsx 输出不保留样式"改为模板写出保留样式；文本合并工具栏按钮名改为 Next/Prev Change。
- **仓库管理**：table-merge 的 GUI 视图、`/reviewconflicts` 与等待/退出码管道补齐 app 层、真实 exe 转交与快照
  （`table-merge-basic`/`table-merge-conflict`）测试，tasks 全部勾完；text-merge / table-merge 的 spec 与已落地行为对齐
  （改动区段导航、按格式分派、模板写出、P4V 参数顺序）。

## 2026-11-15

- **OpenSpec 归档（仓库管理，无功能变化）**：已完成的 5 个 change（text-compare-core、
  folder-compare-sync、table-compare-xlsx、text-patch-reports、hex-compare）归档至
  `openspec/changes/archive/2026-11-15-*`，22 个 capability 主 spec 建立于 `openspec/specs/`。
  剩余活跃 change：table-merge（17/25）、text-merge（30/34）、visual-parity（15/23），
  后续事项见 `docs/worker-reports/openspec-remaining-handoff.md`。

## 2026-11-15

- **路径栏失焦/切换路径后保住水平滚动位置**：此前单行路径框一失焦就跳回开头（egui 内部把
  滚动偏移归零），长路径移到末尾看了文件名、一点别处或切换路径就再也看不见。现在持焦时的
  可视位置被记下，失焦与切换路径（新路径更短则钳到末尾）都按原位置显示；选区与光标不保留，
  点击重新聚焦的当帧回到开头并恢复正常的落点映射与光标跟随。文本/表格/Hex/文件夹视图的
  路径栏与合并视图标题条只读路径框统一生效。

## 2026-11-15

- **界面风格统一（组件库重构）**：此前工具栏下拉（Sessions / Format / Show Differences …）是一摞
  egui 按钮、各类浮窗是 egui 默认灰标题栏，与整体设计不搭。现在：
  - 所有工具栏下拉、组合框列表、路径历史、菜单按钮都用与菜单栏相同的弹出菜单（勾选/单选标记
    当前值），贴在按钮正下方；再点按钮收起。路径栏「历史」图标改为打开历史列表（原先误开打开对话框）。
  - Go To、Info、File formats、表格/Hex Session Settings、文件夹规则、查找、Save log、图例、
    合并另存等浮窗统一为圆角卡片：加粗左对齐标题、× / Esc 关闭、默认居中。
  - 按钮、勾选框、单选、列表、组合框、输入框统一为设计系统控件（抹茶绿强调色）；Session Settings /
    Options 的标签页、箭头、导航与外框，文本查找栏，均去掉经典 Win32 外观。
  - Hex 对话框文案改为英文（OK/Cancel、Previous/Next、Text、Match case、Byte address）。
  - 文本文件信息行的格式/编码下拉不再截断（如 "Everything Else"、"UTF-8 BOM"）。
  - File formats 面板的导入/导出改用系统文件对话框。
  - 预留换肤：外框配色集中为 `theme::Palette`（见 ADR 402），暂无换肤 UI。

## 2026-11-14

- **浏览/打开/另存改用系统原生文件对话框**：路径栏浏览按钮与右键 Browse（Alt+B）、Open File...、
  Save File As...、Save target...、Export as CSV...、表格报告导出，以及文件夹比较的 Browse for Folder，
  现在都弹 Windows 资源管理器同款对话框（替代原先简陋的应用内「File path」路径输入窗）。
  文本/表格/Hex/文件夹四个视图风格统一：标题 `<动作> - Left Side / Right Side`，起始目录与预填文件名
  取该侧当前路径，过滤器含 `All files (*.*)`，对主窗口模态。

- **BREAKING：三方合并按 P4V `%b %1 %2 %r` 取参，界面改为 BASE | LEFT | RIGHT，保存写本地文件**：
  - 位置参数 `matcha <base> <theirs> <yours> [<output>]`（原为 `<left> <right> <center> [<output>]`）：
    base → BASE，theirs → RIGHT，yours（本地文件）→ LEFT。此前 P4V 调起时 base 被放进 LEFT、本地文件被当作 BASE。
    `/center=` + 两个路径（`<left> <right>`）、`/mergeoutput=` 的写法不变。P4V Arguments 用 `%b %1 %2 %r`，
    旧的 `%1 %2 %b %r` 配置需要更新。
  - 文本/表格合并上排三栏改为 BASE | LEFT | RIGHT（原为 LEFT | BASE | RIGHT）；OUTPUT 标题条不再显示路径，
    只留 not saved 等状态。
  - 界面保存默认写到 LEFT 的文件（本地真实路径），给了输出文件（`%r` / `/mergeoutput=`）时再同步复制一份；
    原来只写输出文件、没有输出文件时询问路径。`/automerge` 不变，仍必须有输出目标。

- **表格合并的选中格详情条与全文查看**：输出网格之下新增详情条，显示活动窗格选中格的完整内容
  （换行、内部滚动、可选中复制）；悬停任意格显示完整内容 tooltip——110 px 列宽截断的长 JSON 可直接读完。
- **表格合并的滚动改为逐窗格独立**：滚轮只滚指针下的窗格；每个超宽窗格底部自带横向滚动条（以列为单位），
  宽的 OUTPUT 不再被输入窗格的滚动位置拖走（整行 12 列可一屏看全）；纵向四窗格共用保持行对齐。
  方向键移动选中格、改动导航与打开落点按各窗格宽度横向自动滚动保证选中格可见；
  改动导航与打开落点落在区段首行的第一个改动格。

- **表格合并键推断容忍全空键**：键列含空白单元格（分隔行）时不再退化为整行作键——非空键在三方各自唯一、
  且与 center 重合过半的列即取为键（如 EquipmentDefinitionTable 的 Name 列，545 个空白键行按序配对）。
  此前此类文件的一处单元格改动显示成「删行 + 增行」且整行作键，现在正确显示为键对齐行的单元格级改动。
- **合并视图的改动导航**：文本/表格三方合并的「Next Conflict / Prev Conflict」改为「Next Change / Prev Change」
  （`Ctrl+N` / `Ctrl+P`），逐个走改动区段（含冲突，不再只跳未解决冲突）；无冲突的自动合并改动也能一键定位。
  打开合并视图的落点改为第一个冲突，没有冲突则第一个改动。

- **合并视图标题条路径可复制**：文本/表格三方合并的 LEFT / BASE / RIGHT / OUTPUT 标题条路径从纯绘制
  文本改为只读地址框（`chrome::pathbar::show_readonly`）——可选中、Ctrl+C 复制，右键菜单 Copy / Select All，
  选区内右击保留选区（与可编辑路径框同一套快照机制）。表格合并中路径框持焦时，方向键 / Delete / F2 / Enter
  不再误作用于输出网格（Ctrl 快捷键照常）。

- **表格合并的 xlsx 输出保留样式与版式**：以 base 工作簿为模板写出，只重写单元格数据——字体/填充/边框/数字格式、列宽、行高、
  合并单元格、冻结窗格、条件格式、图表、批注、定义名称、宏都原样保留；只在 left/right 出现的新样式并入。行增删后公式、
  合并单元格、定义名称、表格与图表系列的行引用随行移动（同 Excel 插删行），并强制打开时重算。
  修正：一侧插删行后 Excel 改写的合计公式（如 `SUM(C2:C6)`→`SUM(C2:C5)`）不再被当成改动而与另一侧冲突。
  新增表格合并测试语料 24 例（`crates/bc-script/tests/golden/table_merge/`）。

- **新增表格三方合并（xlsx/xlsm/CSV）**。三个输入都是 xlsx/xlsm 的合并调用（含 P4V `%1 %2 %b %r`）改为按键对齐的
  行、逐单元格合并：两侧改同一行的不同列不再冲突，同格异值、一侧删行另一侧改行、两侧新增同键异内容为冲突。
  键列 `/keys=1,3` 指定，否则自动推断，推断不出整行作键；多 sheet 按名配对。`/automerge` 无冲突写出 0、有冲突不写 101、
  错误 100，结果行带实际键列；`/reviewconflicts` 有冲突时打开新的表格合并视图（LEFT / BASE / RIGHT 三网格 + 可编辑 OUTPUT）。
  xlsx 输出重写工作簿：保留值、公式（含缓存结果）、数字格式与 sheet，xlsm 带回宏工程，**不保留**样式/列宽/图表；
  CSV 输出保真写回。工作簿也按 ZIP 文件头识别（版本控制临时文件名）。详见 `docs/features/table-merge.md`。
  **BREAKING（仅命令行返回值）**：xlsx/xlsm 三方调用原先按文本合并产出无意义结果，现在走表格合并；工作簿与非工作簿
  混合、表格合并带 `/force` 报错 100。CSV/TSV 默认仍按行文本合并，`/tablemerge` 才按表格合并。

- **开发开关 `--screenshot` 支持文本合并视图**：合并视图打开即视为就绪（对齐在打开时同步完成），
  截图不再因等待 TextCompare 就绪信号而失效。

- **新增文本三方合并视图**。上排 LEFT / BASE / RIGHT 三个只读输入窗格，底部 OUTPUT 可编辑；
  四窗格共用行网格与滚动位置，右侧一条纵向滚动条，打开即定位第一个冲突。左改动 / 右改动 / 冲突 /
  手工编辑分别着色，未解决冲突行有红色竖条，状态栏显示冲突区段数与当前行类型。
  支持 Take Left / Center / Right、接受当前内容后跳到下一冲突、冲突导航与统一撤销/重做；
  输出复用文本编辑器的输入法、选区与剪贴板，手工编辑视为解决涉及的冲突，新增行在输入窗格留空隙。
  有输出目标时自打开起算未保存，关闭/退出有 Don't Save / Cancel / Save 确认；无目标时保存询问路径，
  未解决冲突保存前确认 Save Anyway，始终写当前输出、不加冲突标记。
- **新增 `/automerge` 无窗口自动合并与 `/reviewconflicts` 冲突复核**。自动合并在发起进程内执行，
  不转交已有实例，必须给出输出目标；无冲突写出返回 0，有冲突不写出为 101（既有输出保持原样），
  `/force` 带 `<<<<<<< left` / `=======` / `>>>>>>> right` 标记写出为 14，参数/输入/写出错误为 100。
  `/reviewconflicts` 无冲突时仍不开窗，有冲突先不写输出再转入界面，优先于 `/force`，但被 `/silent` 禁用。
  `/iu`、favor 在 `/automerge` 或 `/reviewconflicts` 下生效：不重要差异可忽略，favor 仅为被忽略的不重要
  冲突选取指定侧；左右 favor 互斥。`/force` 只用于自动合并，界面保存不注入标记。
  写出用同目录临时文件再替换、不建备份、不读旧输出，目标可与输入相同；结果为 stdout 一行，`/silent`
  或无标准输出时不写也不弹结果框，错误写 stderr（无控制台时为消息框）。
- **行为变化：`matcha a b c [d]` 从“不支持、退出码 100”变为打开合并视图**。也支持
  `/center=<file> [/mergeoutput=<path>] <left> <right>`，`/mergeoutput=` 优先于第 4 个位置参数；
  一律文本合并，CSV/TSV 按行。合并调用总是等待：`matcha.exe` 与 `matcha.com` 都等待合并标签页关闭，
  转交已有实例同样适用；首个实例或 `/solo` 要等整个窗口关闭（仅合并一页时关页即关窗）。
  界面结果码为最后一次保存无未解决冲突 0、带未解决冲突保存 14、从未保存 101、打不开 100。
  P4V 配置 Application 为 `matcha.exe`、Arguments 为 `%1 %2 %b %r`，可前置 `/automerge /reviewconflicts`；
  立即返回的 `matcha-comp.exe` 不能用于合并。上述合并语义**未经参考样本确认**，采集阻塞与验收见
  `docs/features/text-merge.md`、`docs/reference-parity/text-merge.md`。

## 2026-11-13

- **修复：一侧 xlsx 公式没有缓存值时表格比较大面积误报**。由不重算的工具保存的工作簿，公式单元格
  只有 `<f>` 没有缓存结果（`<v/>`），以前按空值比较，结果与另一侧（如 0.3）全部不同。
  现在把这种单元格的值视为「未知」：两侧都是公式且至少一侧无缓存值时，按公式文本判定（相同视为一致）；
  内容对齐的行哈希在含此类单元格的列上同样按公式文本计算，行仍能对齐。GUI 与
  `/report=summary` 共用同一引擎，一并修复。实测 StaticMeshProperty #443/#444：884 行误报 → 12 行
  差异，与参考工具一致。
- **修复：内容对齐时稀疏行配到错误的行**。无键（按内容对齐）时差异块内的行配对原来复用文本的字节直方图
  相近度：短行对所有长行都封底到同一分数，平局落到最后一个候选（例：左侧 `Entity.LeafPile` 被配给
  右侧新增的 `Entity.Feast.Roast.Boar`）。现改为按列比较非空单元格是否一致的单元格相近度
  （与文本相近度同一打分形状），左右行正确配对、新增行显示为右侧孤行；超大差异块仍走原算法。
- **新增表格报告真实数据回归语料** `docs/reference-parity/table-report/corpus/`：12 个来自
  ProjectLungfish RawData 配表管线的真实 xlsx P4 revision 对（单格修改、数值调参、整行清空、
  批量追加、多 sheet、表中间插入、大表追加、公式无缓存值、表尾修改+新增等形态），
  附 `run_corpus.py` 一键与 baseline 对拍（退出码可进 CI），用例语义与历史缺陷记录见目录 README。

## 2026-11-12

- **Hex 比较：缩略图导航列更宽、视口与当前位置更醒目**。迷你图内框由列内 24 px 加宽到 34 px
  （差异横线更长更易看）；视口框对称越过内框两侧、2 px 灰色描边（颜色对齐参考工具），
  最小高度 9 px（大文件下不再退化成一条横线）；新增列左缘蓝色三角指针指示活动侧光标位置
  （对齐参考工具）。列宽 52 px、红色差异横线语义与点击/拖动定位不变。

- **修复：macOS 上系统文件拖放永远落到左侧**。系统拖动期间窗口收不到鼠标移动（Windows OLE 拖动、
  macOS 拖拽会话皆然），egui 指针停在旧位置，落点判定回落到活动侧（通常左侧）；此前系统光标修复
  只覆盖了 Windows。现 `bc_shell::cursor_screen_pos` 增加 macOS 实现（`CGEventCreate` 读系统光标，
  按窗口 pixels-per-point 换算物理像素），文本/表格/Hex/文件夹视图的落点判定在 macOS 上同步生效。
- **Hex 比较：同时拖入两个文件自动成对设为左右**（此前多文件只取第一个）。成对替换是原子的：任一侧
  有未保存编辑或文件加载失败都整体拒绝并在状态栏提示；单文件拖放行为不变（落点在哪侧换哪侧）。

- **表格报告：去掉首行格式标识与两侧路径**。`/report=summary`（表格差异报告）不再输出
  `MATCHA-TABLE-REPORT 1` 首行与 `left:` / `right:` 路径行（路径在命令行里已有，无任何程序
  消费该标识）；报告现在以 `result:` 行开头，其余格式不变。

- **修复：地址栏光标/文本不垂直居中**。egui 0.37 起 TextEdit 文本默认改为 LEFT_TOP 对齐，
  地址栏把编辑框撑高到 28 px 后文本贴顶（光标顶在框上沿）。现显式设 `Align2::LEFT_CENTER`，
  影响文本/表格/Hex 共用的路径框（`chrome::pathbar`）。

- **修复：双击 Auto-saved 记录后地址栏带 `\\?\` 前缀**。记录最近比较时 `canonicalize` 在
  Windows 返回 verbatim 路径（`\\?\C:\…`）并原样写入 recent-text.txt；现统一还原为常规写法
  （`\\?\UNC\host\share` 转为 `\\host\share`），存量 recent 文件里带前缀的记录读入时一并清理。
- **修复：地址栏选区内右击会丢选区（Cut/Copy/Delete 全灰）**。右键「按下」帧 egui TextEdit
  就把选区折叠成落点光标，帧末的选区快照随之被覆盖，等「释放」帧菜单打开时快照已毁；现
  按下帧即从事件落点恢复快照选区，释放帧的恢复变为幂等重复。点在选区外仍按系统编辑控件
  惯例收拢选区。影响文本/表格/Hex 共用的路径框（`chrome::pathbar`）。

- **表格报告：`title-row` 下单元格行去掉冗余列字母**。加了 `/reportoptions=title-row:<n>` 且列名只是
  字母时，单元格行从 `K[AttachConfigs] (K1172): …` 改为 `AttachConfigs (K1172): …`（引用里已有
  列字母）。`key:`、行标题键值、单侧行仍写 `K[AttachConfigs]`；不加该选项时输出不变。
- **修复：Hex 比较大段不同内容时对齐错乱**。两个整体内容不同的二进制文件（如两张不同的 PNG）
  找不到 50 字节公共锚块、差异区间 ≥ 5000 字节时，Complete 档原来把左侧全部字节记为左孤儿、
  再把右侧全部记为右孤儿（左栏满屏红、右栏满屏斜纹）。现按参考工具 `RUNRANGE` 语义沿对角线
  逐字节配对：相同为 same、不同为 difference，失配处在 ±10 字节内找到连续 4 字节相等即重同步
  （跳过的字节记为孤儿），较长一侧尾部为孤儿。154 KB vs 420 KB 两张 PNG 对齐约 0.1 s，
  开头两行与参考截图逐字节一致。
- **修复：Hex 比较地址栏无法选中/复制**。两侧路径框 egui id 相同（`columns` 子 ui 未隔开作用域），
  点击与拖选都进不了编辑框；现每侧独立 id。另外路径框/查找框持焦时，Ctrl+A/C/X/V/Delete 等
  不再落到十六进制字节上（之前 Ctrl+C 会用 hex dump 覆盖剪贴板，粘贴会写进文件字节）。

- **界面：内容画布底纹，数据区与空白区分得清**。表格比较中，表格之外的空白区、行详情区，以及文本比较
  文件末行以下，改为比外框深一级的冷灰"画布"并带极淡点阵底纹；数据单元格铺白底，像一张纸放在
  画布上；行号列与列头用浅灰表头色。滚动条滑块改为可见的灰色圆角（之前白底上看不见）。
- **修复：文件信息行文字重叠**。表格/文本窗格上方的"修改时间 / 字节数 / 格式 / 编码 …"原按参考
  像素槽位摆放，日期较长（如 `11/6/2026 10:41:26 AM`）或字号变化时会互相压住；改为按实际文字宽度
  顺序排列。

- **界面：全新现代视觉设计**（ADR 401）。外框不再复刻经典 Win32 外观：冷中性灰表面、发丝分隔线、
  圆角控件与柔和阴影，品牌强调色"抹茶绿"；菜单栏/弹出菜单/工具栏/标签条/状态栏/路径框/滚动条/
  设置对话框控件全部改用统一设计 token（`chrome/theme.rs`）。差异内容颜色不变。
- **界面：确认弹框重做**。"未保存修改"等弹框改为遮罩 + 居中卡片：警示徽标、按动作区分的标题
  （"Save changes before closing?" 等）、列出受影响的 LEFT/RIGHT 文件名与路径；按钮为
  「Don't Save / Cancel / Save」，Enter 保存、Esc 取消。表格、Hex（未保存/重载）、文件操作确认、
  About 同样改用新弹框；Hex 弹框文案由中文统一为英文。
- **界面：Home 页**。标题"Start a comparison" + 说明；五种会话类型以卡片一行排开（不再一页 3 个
  加翻页箭头，窗口过窄时才分页）；会话侧栏扁平化、选中为圆角胶囊、筛选框带占位提示。
- **修复：快照模式窗口无法关闭**。快照场景含未保存状态时，截图后的自动关闭会被"未保存拦截"挡住，
  窗口一直挂着（且快照模式屏蔽输入，按钮点不动）。快照模式现在跳过该拦截。

## 2026-02-12

- **修复：快照场景清单缺 3 个 hex 场景**。hex-compare 合入时 `REGISTRY` 增加了 hex-basic/hex-insertion/hex-diffs-filter，但帮助文本用的 `SCENES` 清单未同步，`--help` 的 scenes 列表漏掉它们，且 registry 测试一直失败。`SCENES` 已补齐，测试改为按名字断言（不再依赖注册表顺序）。
- **修复：表格内容对齐下变更行被拆成孤行**。未设置键列（键推断无结果、报告显示 `key: (none; rows aligned by content)`）时，仅个别单元格有差异的行被错误显示为左右各一条孤行（left-only/right-only），而非 `~` 变更行；CLI 报告与 GUI 同样受影响。原因是行对齐的校验拆分对"全列哈希"生效，任何单元格变更都会拆散已由相似度配对的行。现在校验拆分仅对真实键选择（手动键、显式"全部列"、"按列号"）生效；键推断失败保持空"自动"（内容对齐），相似行正常配对为变更行。显式"全部列"语义不变（任意单元格变更仍视为整行不同）。
