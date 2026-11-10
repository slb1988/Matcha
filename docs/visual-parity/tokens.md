# 视觉 token 与证据说明

## 范围与状态

按协调者最后两次收窄要求，精度优先保证文本/文件夹比较核心区域；不再启动参考工具。Home、表格、同步、对话框、右键菜单、标签的进一步精调 **deferred**，保留已获取的真实截图与清单，不补拍。核心参考 23 张：21 张主客户区 1384×961；Session Settings 583×563；Options 859×655。

`dlg-find` 已按协调者批准改为 `text-find-bar`：BC 5.0.6 实际提供停靠查找条，而非独立顶层窗口。该场景的 `findbar` 为 `[0,70,1384,38]`，控件清单在 inventories.findbar。

## 核心几何（像素；左上角为客户区原点）

|区域|实测|
|---|---|
|菜单栏 / 工具栏|27 / 43 高|
|工具栏常规按钮 / 图标槽|All 边框 59×39；图标 16×16；按钮宽随标签变化|
|路径栏 / 信息头|26 / 27 高，起点 y70 / y96|
|缩略图栏|52 宽（实际缩略指示条 38 宽）|
|文本行 / 行号槽 / 箭头槽|15 高 / 基础场景20宽 / 20宽；长文件自适应加宽|
|每侧状态 / 行详情 / 全局状态|19 / 49 / 25 高|
|底部状态字段宽|293、392、92、607|
|文件夹过滤栏 / 路径栏 / 列头|45 / 26 / 24 高|
|文件夹行高 / 左列宽|20；Name 437、Size 78、Modified 150|

完整矩形、像素坐标和测量说明见 tokens.json。文字基线等细项以截图像素为准，不宣称不同文本栅格器字形逐像素一致。

## 颜色

34 项均含 `hex`、`scene`、`point` 和语义；不使用旧版猜测色。核心样例：面板 `#F0F0F0`、重要差异底 `#FCE6E6`、不重要差异底 `#EEEEFF`、选中工具按钮 `#CCE8FF`/边框 `#99D1FF`。`folderOrphan` 是小字号名称的实际核心像素 `#6A00D5`；前景字符仍受 ClearType 影响，不把它等同于不可见的内部颜色配置。

## 字体证据

- UI：原生 TUiComboBox/TUiStatusBar/Edit 的 WM_GETFONT + GetObject 均返回 **Microsoft YaHei UI，lfHeight=-12（96 DPI，9 pt）**。见 `supplemental/text-basic-fonts.json`。
- 系统来源核对：独立调用 `SPI_GETNONCLIENTMETRICS`，messageFont/menuFont 同为 Microsoft YaHei UI -12；`DEFAULT_GUI_FONT` 则为 SimSun -12。见 `supplemental/system-fonts.json`。因此实际 UI 字体与系统消息/菜单字体一致，支持“跟随系统消息字体”的实现方式；不声称反编译证明参考程序内部调用路径。
- 编辑器：Options → Appearance → File Views → Editor Font 截图明确为 **Consolas 10 pt**，见 `supplemental/options-editor-font.png`；未点 OK/Apply。
- 行号：数字核心高度7 px；42组 GDI 字体候选离线拟合中 Arial 6.5 pt 最接近（候选结果见 `line-number-font-estimate.json`）。**这是视觉拟合值，不是原生 HFONT 确认值**；不要把旧版猜测的 Consolas 9 pt 当真源。

## 菜单清单与 CHM

每个顶层菜单及可展开子菜单均有独立 PNG 与 `.tree.json`，来源是运行中的 TTBPopupWindow + MSAA；字段包含名称、助记符、快捷键、分隔线、勾选态、可用态和子菜单。`null` 表示 MSAA 未声明该字段，不推测快捷键。Explorer 是 Windows Shell 动态外部子菜单，只登记入口，不执行其动作。

|视图|CHM 命令目录条数（含 common）|UI 菜单项（含子菜单、不含分隔线）|
|---|---:|---:|
|Home|30|36|
|Text|135|136|
|Folder|144|134|
|Sync|77|88|
|Table|104|102|

完整逐名称差集在 `tokens.json.chmCrosscheck`：CHM 包含隐藏(*)、平台/许可/Source Control 条件命令及合并的斜杠名称，不能直接当默认可见菜单。实际树与 CHM 不一致的名称全部保留差集，不伪称一一相等。工具栏按真实截图登记条目、类型、默认态、分组；三类右键菜单也已有实际截图与 MSAA 数据，其进一步精调 deferred。

## 采集与已知限制

- 主窗口使用 PrintWindow 后按 GetClientRect/ClientToScreen 裁剪；菜单按协调者批准用独立弹出窗口 PrintWindow 合成，避免用户桌面悬浮层污染。菜单阴影未合成；不提取 exe 图标资源。
- 每场景显式选择对应输入和视图，失败不写正式 PNG；顶行、尺寸、菜单差异、双侧列头/网格、对话框 hwnd 均自检；全批次 SHA256 去重。末次日志在 `supplemental/capture-verification.log`。
- 部分文件夹场景底部显示鼠标悬停提示，正常分栏的早期真实证据保留为 `supplemental/folder-status-normal.png`；不为此重拍。时间、剩余磁盘量、加载耗时是动态字段，不纳入整图字节确定性声明。
- 脚本未读取授权/密钥文件、未写安装目录、未直接编辑设置文件或 BCSessions.xml。参考程序自身可能更新自动会话；未覆盖或恢复用户会话文件。BCPreferences.xml 元数据仍是 2025-04-27；没有可宣称的本次设置备份/恢复事务。

## compare

只依赖 PowerShell + System.Drawing。输出并排图、热图、命名区域四边偏差、颜色抽样；局部扫描背景/分隔线色带，容差2 px。平坦色带、窗口边缘裁切会产生多解，JSON 的 ambiguous 字段明确标记；不能把匹配结果解释为 OCR 或完全自动化视觉验收。自身比较16个区域偏差全0；整体平移(3,3)副本核心内部区域四边均检出3 px，边缘裁切区域最大4 px，正确标红。

