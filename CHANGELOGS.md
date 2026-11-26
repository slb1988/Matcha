# CHANGELOGS

本文件记录 Matcha 的用户可感知变动（功能、行为、修复）。每次变动追加一条，最新在最上。

## 2026-11-26

- **启动更快：首个窗口不再等 PowerShell**（[`docs/features/gui-shell.md`](docs/features/gui-shell.md)）：读取系统界面字体原先要拉起隐藏的 PowerShell 并现编一段 C#（热启动约 0.44 s，开机后首次或刚更新 exe 时可达数秒，P4V 首次 diff 迟迟不弹窗主要卡在这里），现改为进程内直接调用 Win32（约 4 ms）。热启动到首帧由约 0.49 s 降到约 0.28 s；所选字体与字号不变。

## 2026-11-25

- **同一对文件再次从外部打开时复用已有标签页**（[`docs/features/gui-shell.md`](docs/features/gui-shell.md)）：Matcha 已在运行时，从 P4V、命令行或资源管理器菜单再次比较同一对文件（或文件夹），不再多开一个标题相同的标签页，而是切到已有的那一页，并按 Options › Disk Changes 检查两侧是否已在磁盘上变化（P4V 会重写同名临时文件）。路径按规范形式比较，Windows 上不区分大小写；左右互换视为不同比较，仍开新页。

## 2026-11-22

- **文本比较与文本合并：跳到未设置的书签时弹出消息面板并提示音**（[`docs/features/text-bookmarks.md`](docs/features/text-bookmarks.md)）：Go to Bookmark N（`Ctrl+N`）在书签 N 未设置时，状态栏上方显示浅黄色 **Bookmark N is not set.** 面板并播放系统提示音，光标不动，下一次按键或鼠标按下关闭；此前文本比较中这条提示不可见，看起来像按键没反应，文本合并只在状态栏显示文字。该面板不受 Options › Next Difference › Show message panel 控制。

- **更新[项目首页](README.md)**：以当前可用功能替换早期脚手架说明，补上 Windows 发行包下载、第一次拖放比较、PowerShell 示例和源码构建入口。

## 2026-11-21

- **文件夹比较与文件夹同步：差异 / 冲突导航到头时弹出消息面板并提示音**（[`docs/features/folder-compare-sync.md`](docs/features/folder-compare-sync.md)）：Next / Previous Difference 与 Next / Previous Conflict 没有目标时显示 **Difference not found.** 面板，取代状态栏的 "No further difference/conflict" / "No further conflict"；遵循 Options › Next Difference 的回绕与面板开关。

- **文本合并与表格合并：改动区段导航到头时弹出消息面板并提示音**（[`docs/features/text-merge.md`](docs/features/text-merge.md)、[`docs/features/table-merge.md`](docs/features/table-merge.md)）：Next / Previous Change Section 没有下一个改动区段（表格合并为所有 sheet 都走完）时显示 **Difference not found.** 面板，取代状态栏的 "No more changes below/above"；遵循 Options › Next Difference 的回绕（表格合并回到第一个/最后一个有改动的 sheet）与面板开关。

- **十六进制比较：导航找不到差异时弹出消息面板并提示音**（[`docs/features/hex-compare.md`](docs/features/hex-compare.md)）：Next / Previous Difference Bytes 越过末端时显示 **Difference not found.** 面板，取代状态栏的中文提示（"最后一个差异之后" / "第一个差异之前"）；遵循 Options › Next Difference 的回绕与面板开关。

- **修复十六进制比较 Next Difference 卡在单侧块上**：差异块只在对侧有字节（如右侧插入）时，活动侧光标留在原处，Ctrl+N 每次都回到同一块，永远到不了后面的差异和末端。现在记住上次导航落到的对齐行，光标未动就从该行继续。

- **表格比较：导航找不到差异时同样弹出消息面板并提示音**（[`docs/features/table-compare.md`](docs/features/table-compare.md)）：Next / Previous Difference Cell、Difference Row、Edit 越过末端时显示 **Difference not found.** 面板，取代状态栏里的 "No more differences / edits"；遵循 Options › Next Difference 的回绕与面板开关。

- **Options › Next Difference 页可用**：此前灰色不可点。新增两个开关并写入 `settings.txt`：**Wrap around to first difference**（默认关；开启后越过最后一个差异回到第一个，向上同理）与 **Show message panel**（默认开；关闭后找不到差异时不弹面板也不响）。各视图何时生效见同日其他条目。

- **文本比较：导航找不到差异时弹出消息面板并提示音**（[`docs/features/text-compare.md`](docs/features/text-compare.md)）：Next / Previous Difference、Next / Previous Difference Section、First / Last Difference 没有可去的差异时，底部状态栏上方弹出浅黄色 **Difference not found.** 面板并播放系统提示音；此前只写入不可见的内部消息，看起来像按键没反应。下一次按键或鼠标按下即关闭。遵循 Options › Next Difference 的两个开关（含从文件夹比较打开的内嵌文本窗口）：开启 Wrap around 时越过末端回到另一端的差异，关闭 Show message panel 时静默。

## 2026-11-19

- **文本比较与文本合并：编号书签 0–9**（[`docs/features/text-bookmarks.md`](docs/features/text-bookmarks.md)）：Search › Toggle Bookmark（`Shift+Ctrl+0…9`）在活动窗格的光标行放置书签，同一行再按即移除，一行最多一个书签；Go to Bookmark（`Ctrl+0…9`）切到书签所在侧并定位到该行（被显示过滤隐藏时先显示全部），未设置时光标不动；Clear Bookmarks 清除全部。书签行在文本区右缘显示编号徽标、缩略导航条上有刻度。书签随本侧编辑上下平移（撤销同理），重载保留行号、换文件清除该侧、Swap Sides 随内容换侧；不写入命名会话。文本合并视图的 Search 菜单同样新增这三项与快捷键：书签挂在对齐行上并记住放置时的窗格，跳转时激活该窗格并把四个窗格滚到该行，Take 与输出编辑不移动书签。为让 `Ctrl+0` / `Shift+Ctrl+0` 专用于书签 0，关闭了 egui 自带的 `Ctrl+=` / `Ctrl+−` / `Ctrl+0` 整体界面缩放（从未作为功能提供；编辑器字号仍在 Tools › Options 与 View › Text display 中调整）。

- **文本合并视图：行内差异、语法着色、查找替换、横向滚动条、输入窗格选择复制**（[`docs/features/text-merge.md`](docs/features/text-merge.md)）：
  LEFT / RIGHT 窗格的改动行只把与 BASE 不同的字符着色（左侧青、右侧紫、未解决冲突红），整行新增仍按整行着色；
  四个窗格按文件格式语法着色，View › Syntax Highlighting 可关闭。Search 菜单新增 Find…（`Ctrl+F`）、Find Next（`F3`）、
  Find Previous（`Shift+F3`）、Replace…（`Ctrl+H`）：在活动窗格查找、不回绕，替换只对输出窗格，被替换的行标为手工、一次撤销全部还原。
  任一窗格内容超宽时窗格区底部出现四窗格共用的横向滚动条。三个输入窗格可点击、拖选、`Shift`+点击、`Ctrl+A`、`Ctrl+C`，
  右键菜单有 Take Left / Center / Right、Copy、Select All；输入窗格仍然只读。

- **文件在外部被修改后弹框提示重载**（[`docs/features/disk-change-reload.md`](docs/features/disk-change-reload.md)）：文本、表格、十六进制比较（含文件夹比较里打开的文件对窗口）在窗口重新获得焦点或切换到该标签页时检查两侧文件；发现变化弹出 **File changed on disk, reload?**，列出变化的文件，有未保存编辑时注明会被丢弃。**Reload** 重载并保留当前行与滚动位置，**Keep Current** 保留当前内容且同一份磁盘内容不再重复询问。Tools › Options › **Tweaks** 新增两个开关：检查（默认开）与「不会丢弃修改时自动重载」（默认关）。自己保存不算外部修改；文件被删除只在状态栏提示。合并视图不参与。

- **修复文本比较打开文件期间外部程序无法保存它**：此前文本文件以只读内存映射打开并独占写入，在编辑器里保存正被比较的文件会报「文件被另一进程占用」。现在小于 64 MiB 的文本文件读入内存、不再持有句柄；更大的文件为性能仍按映射打开（已知限制，见上述功能文档）。

- **GitHub 发布流程**（[`matcha-publish`](.agents/skills/matcha-publish/SKILL.md)）：按上一正式 Release 到目标提交整理中文变更说明；分支领先时先列出待推提交并确认，发布后只读核验 tag、正文和各附件的 SHA-256。只说「打包」仍只做本地构建。

- **窗口右下角缩放把手更大、更醒目**（`docs/features/gui-shell.md`）：原先只是 6 个浅灰装饰点，抓不准；现在是 22×22 px 的把手区域、10 点斜向三角纹理，悬停出现缩放光标并高亮，按住拖动即可缩放窗口（交给系统处理，手感同抓窗口边角）。最大化时隐藏；Home 以外的视图都有。

- **文本比较：拖选越过窗格上下边缘时竖直跟随滚动**（`docs/features/text-compare.md`）：拖到窗格上方或下方（水平滚动条）以外，视图逐行往上/下滚，越远越快，选区跟着扩展到新露出的行；两侧窗格同帧移动。

- **文本比较：拖选越过窗格左右边缘时水平跟随滚动**（`docs/features/text-compare.md`）：水平滚动条在中间时，向左（越过行号栏）或向右（越过竖向滚动条）拖选，视图会逐步跟着移动，指针越出越远滚得越快，选区随之扩展到新露出的文字；指针回到文本区或松开即停。

## 2026-11-18

- **修复文本比较窗口缩窄后右侧页面与滚动条被裁掉**：双窗格和底部行详情跟随窗口实际宽高，工具栏在窄窗口换行；文件信息头不再撑宽编辑区。

- **文本比较：滚动不再换行或打断选择**。点击、拖动上方窗格的滚动条保留当前对比行、选区与编辑焦点；底部详情滑块从抓住的位置开始拖动，不再必须拖过滑块中心才响应。

- **修复文本比较底部行详情无法横向滚动**：此前底部短横只是占位图形。当前行超宽时可拖动滑块、点轨道或左右按钮，也可在详情区滚动滚轮查看被遮住的文字；上方编辑窗格不受影响。

- **发布 [Matcha v0.1.0](https://github.com/slb1988/Matcha/releases/tag/v0.1.0)**：Windows x64 发行包已可下载；下一次发布的版本号为 `0.1.1`。

- **应用图标：抹茶叶**（`docs/features/gui-shell.md`）：一片抹茶叶，中间叶脉即左右两栏的分隔线，
  两侧侧脉逐对对齐、唯独右侧一条错位并标橙——寓意找出差异（*Match*·a / 抹茶 ≈ 抹差）。
  `matcha.exe`、`matcha-comp.exe`、`matcha-compc.exe`（含 `matcha.com`）内嵌多尺寸图标，资源管理器与任务栏显示；
  窗口标题栏 / Alt-Tab 也换成同一图标（此前为 eframe 默认图标）。图标由 `tools/icon/gen_icon.py` 生成。

- **Home 会话树悬停显示路径详情**（`docs/features/gui-shell.md`）：Auto-saved 条目只显示文件名，
  此前要双击打开才知道比的是哪两个路径；现在悬停即显示会话类型、左右完整路径与最后打开时间。
  Saved 分组中的命名会话悬停同样显示类型与两侧完整路径。

- **修复：文件夹比较里的「Text Compare — selected files」浮动窗口不断变高**：状态栏控件固定画 26 px，
  而文本 / 表格 / 合并视图只给它留了参考布局的 25 px；多出的 1 px 让可缩放浮动窗口每帧按内容长高 1 px，
  窗格下方的空白区越拉越大。状态栏现在不超出调用方给的高度（上限仍为 26 px）。

- **后台任务框架收敛**（内部重构，无行为变化；ADR 008）：文本 / 十六进制 / 表格视图各自复制的后台 worker
  合并为 `bc_jobs::Worker<T>`（一个 job 一个线程，关闭视图时取消并等待结束）；删除从未实现、无人调用的
  JobManager / 线程池 / 资源池脚手架与 crossbeam 依赖。

- **Home Auto-saved 记录所有会话类型**（`docs/features/gui-shell.md`）：此前只有文件对比较会记入
  Auto-saved，文件夹比较 / 文件夹同步从不记录；从空会话在路径栏填路径打开的比较也不记录；
  记录里也没有类型，表格 / 十六进制比较重开时按扩展名重新猜。现在任一标签页两侧加载完成即按类型记录
  （五种类型、所有打开入口），条目前显示类型图标，双击按原类型重开；上限由 12 条提高到 50 条，
  会话树超出时可用滚轮滚动。记录改存 `recent.txt`（v3，带类型），首次启动自动从旧的
  `recent-text.txt` 迁移。

- **文本合并：顺序 Take、按行 Take、按选区 Take、输出窗格右键菜单**（`docs/features/text-merge.md`）：
  Edit 菜单新增 **Take Left Then Right / Take Right Then Left**（冲突两边都要时按顺序保留两侧各行）与
  **Take Left / Center / Right Line**（Shift+Ctrl+L / B / R，只换光标所在一行；多行冲突区段仍保持未解决）。
  Take 系列现在作用于输出窗格的选中行，没有选区时仍为当前区段。输出窗格新增右键菜单，
  集中提供各 Take、Accept Current Content 与剪切 / 复制 / 粘贴 / 全选。

- **命名会话：保存、Home 重开、命令行按名称打开**（`docs/features/saved-sessions.md`）：
  文本 / 表格 / 十六进制 / 文件夹比较 / 文件夹同步视图的 Session › **Save Session**（Shift+Ctrl+S）与
  **Save Session As...** 此前一直是灰的，现在可用：首次保存弹起名框（默认名为标签标题），已命名的标签页再次保存直接覆盖；
  保存类型、两侧路径，文本比较另存对齐规则与 Show Context。Home 会话树新增 **Saved** 分组，双击重开，
  选中后按 **−** 并确认即可删除。命令行 `matcha "<会话名>"` 打开该会话（此前报 not supported），
  常驻实例存在时转交为新标签页。

- **会话文件读写落地**（`docs/features/sessions-and-formats.md`）：`bc-session` 此前的会话持久化全是占位，
  现可读写参考工具格式的 `BCSessions.xml`（嵌套文件夹、按类型默认会话、`LastModified`、Specs、
  文本对齐规则、State），未知类与元素原位保留；本机 34 份参考工具真实会话文件逐字节往返。
  会话库位于 `%APPDATA%\Matcha\BCSessions.xml`，支持插入/替换（名称全树唯一）、删除与原子写盘。
  界面与命令行入口随后接入。

- **visual-parity 变更归档**（文档/规格，无行为变化）：23/23 完成的 visual-parity 移入
  `openspec/changes/archive/2026-11-18-visual-parity`，新建 6 个主 spec（`ui-chrome-theme`、`text-view-appearance`、
  `folder-view-appearance`、`home-table-appearance`、`dialog-appearance`、`visual-reference-harness`）；
  功能索引中 GUI 外壳状态改为"已实现"。至此无活跃 openspec change。

## 2026-11-17

- **Options 设置重启后保留**（`docs/features/gui-shell.md`「偏好持久化」）：此前 Tools › Options 里的差异颜色、
  默认 Tab 宽度、默认代码页、备份设置只在本次运行有效，重启即恢复默认；现在与编辑器字体一起保存在
  `%APPDATA%\Matcha\settings.txt`，启动时读回。

- **文本比较打开定位首个差异时保留上文**（`docs/features/text-compare.md`）：此前首个差异行贴在窗格顶边、看不到前文，
  现在其上方保留最多 5 行上下文（窗格较矮时不超过可见行数的三分之一）。

- **文本显示字体：中文对齐、去掉"粗体感"，可选字体并保存**（`docs/features/text-compare.md`）：
  文本比较/合并里的中文（微软雅黑回退）此前比英文大一号、下沉约 2 px 且底部被裁，现按 GDI 字体链接方式缩放并对齐基线；
  字形 gamma 由 0.7 改回 1.0，Consolas 不再显得像粗体（整个界面文字随之略细）。
  Tools › Options › Appearance 新增 **Editor font**：从本机等宽字体中选字体族、设字号（6–36 pt），立即生效，
  保存在 `%APPDATA%\Matcha\settings.txt`，重启后沿用。

- **表格比较打开即定位到首个差异格**（`docs/features/table-compare.md`）：打开文件对、切换 sheet 标签或换一侧文件后，
  比较完成时自动跳到第一个差异行的第一个改动格并滚动到可见（此前总停在左上角，需再按 Next Difference）；
  就绪前已移动光标/编辑则不跳，重新对齐与 Recompare 不受影响。

- **文本比较打开即定位到首个差异**（`docs/features/text-compare.md`）：打开文件对或换一侧文件后，
  首次比较完成时自动跳到第一个差异区段（不再停在第 1 行、需要再按 Next Difference）；
  尊重 Ignore Unimportant；比较完成前已编辑/移动光标则不跳，编辑后重新对齐不受影响。

- **Compare Files Using 按文件决定可用项，三个视图统一**（`docs/features/gui-shell.md`）：
  此前各视图写死白名单——表格视图只能切 Text（xlsx 切过去打不开）、Hex 有实现却是灰的；文本视图切不了 Table。
  现在文本 / 表格 / 十六进制视图共用一个子菜单与规则：Hex 总可用，Table 要求表格或文本内容，Text 要求文本内容
  （xlsx 只能切 Hex，csv 可切 Text / Hex），当前视图与未实现的 Media / Picture / Version 置灰。

- **xlsx/csv 等表格默认走表格比较，文件类型路由统一到一处**（`docs/features/gui-shell.md`、`table-compare.md`）：
  此前两个文件先做二进制嗅探再判表格，xlsx（ZIP 包）被误开成十六进制比较。现在命令行、Home 打开/拖放、
  Auto 新会话、文件夹比较打开文件对统一调 `bc_formats::route`，顺序为表格 → 十六进制 → 文本；
  xlsx/xlsm（含版本控制临时文件名，按包内容识别）、csv/tsv/tab 与用户表格格式均打开表格比较。
  文件夹比较里打开表格或二进制文件对改为新开对应类型的标签页（此前表格进内嵌文本窗口、二进制只给提示）。
  `/qc` 规则比较对表格按单元格判定（同值不同字节为 same）；脚本报告/合并与 GUI 共用同一套工作簿判定与表格读取选项。

- **注释默认不重要，差异按行着色**（`docs/features/text-compare.md`）：与参考默认一致，
  比较规则默认把注释（Comment）的差异视为不重要——GUI、CLI/脚本与文件夹规则比较共用；
  只改注释的文件在文件夹规则比较中为“相似”而非“不同”。文件夹规则比较新增 `comments-important`
  开关改回旧行为（`comments-unimportant` 仍可解析，等同默认）；文本比较可在 Session Settings > Importance 勾回。
  差异区块改为按行着色：只有注释改动的行淡紫底蓝字，代码改动行粉底红字（此前整块按最高重要性着色）。

- **文本比较：不可覆盖的箭头变灰、显示只读标记、语法风格按语言抽象**（`docs/features/text-compare.md`）：
  目标侧只读（文件带只读属性，如 Perforce 未 checkout 的文件）时，指向该侧的复制箭头改为灰色，
  该侧信息栏显示可点击的 **Read-only**（打开 File properties 取消只读）；此前箭头仍为黄色、点击无反应难以理解。
  语法着色抽象为 `SyntaxStyle`（每类元素的颜色/粗体/斜体/差异行是否保留）+ 按语言覆盖表，
  单侧差异行中的注释保留注释灰色，不再被整行染成差异红。

- **文本比较：行号栏箭头与文字外观对齐参考**（`docs/features/text-compare.md`）：
  行号栏复制箭头改为每个差异区块只在首行画一个更大的黄色渐变块箭头，区块其余行画灰色竖括号线（末行带脚），
  此前是每行一个灰/淡黄小箭头、难以辨认；本侧为空隙的区块也显示箭头（点击即删除对侧这些行）；
  目标侧只读时箭头保持黄色，点击在状态栏说明原因。文字：正文纯黑，语法配色改为参考默认
  （注释灰、指令栗色、字符串深绿、关键字粗体），单侧差异行整行用差异色；全局字形 gamma 加重笔画，
  文字更清晰；行号改为中灰。

## 2026-11-16

- **AGENTS.md 新增提交纪律**：每次修改告一段落（编译、测试通过）自动在当前分支本地提交一版；推送仍需确认。

- **Esc 改为逐个关闭标签页**（`docs/features/gui-shell.md`）：页面无弹层时 Esc 只关闭当前标签页
  （等价于 Ctrl+W，未保存页先弹保存确认），多个标签页需逐次按 Esc 一个个关闭，关掉最后一页才退出进程；
  此前一次 Esc 会退出整个应用、关掉全部标签页。弹层优先消费 Esc 的语义不变。

- **页面中按 Esc 退出进程**（`docs/features/gui-shell.md`）：没有任何弹层打开时按 Esc 即退出应用
  （等价于关窗与 Ctrl+Q；有未保存编辑的页面仍逐页弹保存确认，任一处取消即中止退出）。
  弹层语义不变且优先：菜单栏菜单、工具栏/右键下拉、组合框列表、查找条、单元格编辑器、
  Session Settings 等对话框与未保存确认框仍由 Esc 关闭/取消自身，不会连带退出——多层弹层叠加时
  一次 Esc 只退一层。

- **路径栏首次进入默认全选路径**（`docs/features/gui-shell.md`）：左键点入或键盘聚焦任一路径框
  （文本/表格/Hex 路径栏、合并视图标题条、文件夹/同步视图路径框）时整段路径自动全选，
  可直接输入替换或 Ctrl+C 复制；再次点击/拖拽则正常落点、圈选子串。首次按下直接拖拽仍按圈选处理，
  不会被全选抢走锚点；右键聚焦不触发。同时路径框的文本选区底色由绿色改为与正文窗格一致的
  经典选区蓝（#99C9EF），选中文字保持正文色。

- **文本比较 / 合并输出窗格：I 形光标 + 更醒目的选区**（`docs/features/text-compare.md`、`docs/features/text-merge.md`）：
  指针移到文本区时变为系统的 I 形插入光标（拖选途中保持，行号栏仍为箭头）；选中文本的底色由半透明淡绿
  改为不透明的选区蓝（#99C9EF），在红/蓝差异行底色上也一眼可辨。

- **文本比较行号栏箭头可以点击合并了**（`docs/features/text-compare.md`）：此前差异行旁的黄色箭头只是装饰，点上去只会移动光标。
  现在点击差异区块任一行的箭头，即把**整个区块**从这一侧覆盖到另一侧；某侧选中若干行后，这些行（含相同行）的箭头变为
  **蓝色、尖端带竖线**的选区箭头，点击只把**选中的行**覆盖到对侧对齐位置，完成后清除选区。两者都是一次可撤销编辑（Ctrl+Z），
  点箭头不移动光标；悬停显示手形光标与提示（Copy section to Right / Copy selected lines to Right 等）。
  目标侧只读时箭头变灰、点击无效；编辑后重新对齐期间区块箭头暂不显示、选区箭头为灰色。

## 2026-11-15

- **Hex 比较大文件内存降到原来的约 1/15**（`docs/features/hex-compare.md`）：对齐结果原先为每个字节存一行记录，
  两个约 18 MB 的 exe 比较时进程峰值约 2.2 GB；现在只存连续段（配对 / 左孤儿 / 右孤儿），同例峰值约 140 MB，
  对齐也从约 1.6 秒缩短到约 0.6 秒，结果不变。256 MB 文件的 Fast 对齐从约 16 秒缩短到约 4.5 秒。

- **Hex 比较大文件提速、首屏不再等比较**（`docs/features/hex-compare.md`）：两个约 18 MB 的 exe 此前要等约 13 秒对齐完成、
  再卡住界面约 2 秒才出现内容，之后滚动只有约 8 帧/秒。现在打开即显示两侧字节（对齐完成前按偏移并排、不着色，
  状态栏显示 Comparing…，可直接滚动浏览），后台比较完成后自动换成对齐结果并停留在原先浏览的字节；
  同一对文件比较约 1.4 秒完成，滚动每帧约 5 ms。显示过滤切换、跳到文件尾也不再卡顿。
- **修复：相近的大二进制文件被判为几乎全部不同**：对齐此前会被远处偶然重复的短片段（如重编译 exe 里的常见指令序列）
  带偏，此后整份文件都配不上——上例两个 exe 只认出 4.5 万个相同字节、其余约 1800 万字节双侧孤儿。现在先用两侧都只出现一次的
  片段建立保序骨架，再在骨架之间逐段对齐，同例认出约 1385 万个相同字节。小文件（参考语料）结果不变。

- **修复：比较 exe 等二进制文件不再报错**（`docs/features/hex-compare.md`）：此前路由只看 `*.bin` 扩展名，
  exe / dll / 无扩展名的二进制被送进文本比较，打开时报 “Cannot open: invalid input: binary input: use Hex comparison”。
  现在路由加入内容嗅探（读前 8 KiB，与文本引擎相同的二进制判定规则），命令行 `matcha a.exe b.exe`、Home 拖入 / 最近记录、
  单实例转交等入口都会直接打开 Hex 比较。文件夹比较中双击二进制文件对不再尝试内嵌文本窗口，提示改用 Hex Compare 会话；
  文本会话路径栏输入二进制路径时的报错改为指向 Session > Compare Files Using > Hex Compare。

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
