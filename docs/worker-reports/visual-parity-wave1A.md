# visual-parity Wave 1A 返工报告

实现提交：`8a9eb7a7a520e080b32eeaf5da50628a0a08b290`（基于 `7e3fb18`；本报告在后续文档提交中落盘）。

按协调者最终收窄范围完成任务组 1.1–1.4：优先文本/文件夹核心 UI；其余精调 deferred。参考工具已停止，最终批次结束后未再启动。

## 结果

- 23 张真实参考图、23 个不同 SHA256；21 张主客户区 1384×961，两个独立对话框按实际尺寸登记。旧重复图全部替换；经批准将 dlg-find 改为 text-find-bar。
- 语料重复生成哈希差异 0，最大文件 24,004 bytes；真实 Tab、行尾差异和同名较新/较旧配对齐备。
- tokens：34 个颜色（均有来源坐标）、43 个几何指标、3 类字体、6 类清单、23 个场景。完整菜单与子菜单来自真实 PNG + MSAA；CHM/UI 条数：Home 30/36、Text 135/136、Folder 144/134、Sync 77/88、Table 104/102；逐名称差集及隐藏/平台/动态项原因在 tokens.json.chmCrosscheck。
- UI 原生字体是 Microsoft YaHei UI -12（9pt），与 SPI 系统消息/菜单字体一致，而 DEFAULT_GUI_FONT 为 SimSun -12；编辑器 Options 明确为 Consolas 10pt。行号字体是离线拟合值，不冒充原生查询。
- PowerShell 解析错误 0；未改引擎/GUI 源码，也未修改 snap-matcha.ps1。未运行无关 Rust 构建。

## 对照自测（literal stdout，全部 exit 0）
```text
Compared text-basic; pixel differences=0; regions=16; max deviation=0; color samples=21
Compared folder-basic; pixel differences=0; regions=12; max deviation=0; color samples=2
Compared text-basic; pixel differences=113933; regions=16; max deviation=4; color samples=21
```
自身区域偏差全0；平移(3,3)副本的核心内部边界检出3px，窗口边缘裁切部分最大4px并标红。结果 JSON 已入库 supplemental；并排图、热图及偏差表位于 `target/visual/compare/report.md`。

## 场景证据

命令公共前缀为 BCompare.exe /solo，语料基目录 tools/visual/corpus；完整绝对 argv、自检细项见 reference/capture-manifest.json。

|场景|视图/语料与操作|尺寸|SHA256前12位|自检|
|---|---|---|---|---|
|home|/solo ; collapse Auto-saved; select New (no session files edited)|1384×961|36803ADB3D48|PASS|
|text-basic|/solo /fv="Text Compare" text-basic-left.txt,text-basic-right.txt; 打开对应输入，比较完成|1384×961|C6585340F7EA|PASS|
|text-diffs-filter|/solo /fv="Text Compare" text-basic-left.txt,text-basic-right.txt; toolbar Diffs; Context off|1384×961|AAD81FE84C3E|PASS|
|text-long-scrolled|/solo /fv="Text Compare" text-long-left.txt,text-long-right.txt; left pane; PageDown x3|1384×961|574AF95C5893|PASS|
|text-cjk-tabs|/solo /fv="Text Compare" text-cjk-tabs-left.txt,text-cjk-tabs-right.txt; 打开对应输入，比较完成|1384×961|9203EE17DBD2|PASS|
|text-crlf-lf|/solo /fv="Text Compare" text-crlf-left.txt,text-crlf-right.txt; 打开对应输入，比较完成|1384×961|7D0C9FCF61C9|PASS|
|text-cursor-on-diff|/solo /fv="Text Compare" text-basic-left.txt,text-basic-right.txt; left pane line 2|1384×961|291020E89E7D|PASS|
|text-menu-session|/solo /fv="Text Compare" text-basic-left.txt,text-basic-right.txt; open top-level session menu|1384×961|4180D8EA6BE0|PASS|
|text-menu-file|/solo /fv="Text Compare" text-basic-left.txt,text-basic-right.txt; open top-level file menu|1384×961|24FCEA35EF03|PASS|
|text-menu-edit|/solo /fv="Text Compare" text-basic-left.txt,text-basic-right.txt; open top-level edit menu|1384×961|F65487FFC9FE|PASS|
|text-menu-search|/solo /fv="Text Compare" text-basic-left.txt,text-basic-right.txt; open top-level search menu|1384×961|D5E54340CC99|PASS|
|text-menu-view|/solo /fv="Text Compare" text-basic-left.txt,text-basic-right.txt; open top-level view menu|1384×961|793CC5F6555F|PASS|
|text-menu-tools|/solo /fv="Text Compare" text-basic-left.txt,text-basic-right.txt; open top-level tools menu|1384×961|F622108B68EA|PASS|
|text-menu-help|/solo /fv="Text Compare" text-basic-left.txt,text-basic-right.txt; open top-level help menu|1384×961|67F65BB44355|PASS|
|folder-basic|/solo /fv="Folder Compare" folder-left,folder-right; 打开对应输入，比较完成|1384×961|FEDC91134901|PASS|
|folder-expanded|/solo /fv="Folder Compare" folder-left,folder-right; Ctrl+A; Edit > Expand All|1384×961|E2017240F2D3|PASS|
|folder-diffs-filter|/solo /fv="Folder Compare" folder-left,folder-right; toolbar Diffs|1384×961|083D35FE4E77|PASS|
|folder-sync|/solo /fv="Folder Sync" folder-left,folder-right; 打开对应输入，比较完成|1384×961|4BA63C46A3F4|PASS|
|table-basic|/solo /fv="Table Compare" table-left.csv,table-right.csv; 打开对应输入，比较完成|1384×961|31E406EA783A|PASS|
|table-cursor-on-diff|/solo /fv="Table Compare" table-left.csv,table-right.csv; left grid differing cell|1384×961|AFDD7A09BFA8|PASS|
|dlg-text-session-settings|/solo /fv="Text Compare" text-basic-left.txt,text-basic-right.txt; Session > Session Settings (observed menu row)|583×563|73862450718C|PASS|
|dlg-options|/solo /fv="Text Compare" text-basic-left.txt,text-basic-right.txt; Tools > Options|859×655|90796186FDA6|PASS|
|text-find-bar|/solo /fv="Text Compare" text-basic-left.txt,text-basic-right.txt; Search > Find; docked bar (coordinator approved)|1384×961|CF8F611F71B3|PASS|

## 清理、安全与保留项

```text
BCompareProcesses=0; lockExists=False
```
- 末次采集完整日志：`supplemental/capture-verification.log`。菜单由主窗口与真实 TTBPopupWindow 分别 PrintWindow 合成，不含桌面悬浮层；菜单阴影未合成。
- 未读取授权/密钥文件、未写安装目录、未提取 exe 图标或位图、未进入主目录或 ObsidianVault；未直接编辑设置文件/BCSessions.xml。参考应用自身可能更新自动会话，未覆盖用户会话文件；本次没有设置文件写入/恢复事务可供宣称。
- Home/表格/同步/对话框/右键/标签的进一步精调按协调者要求 deferred；没有缺失 PNG。文件夹截图含悬停状态提示，正常状态分栏另保留早期真实截图；不再补拍。行号名义字体未原生确认，估计与边界扫描多解均在 tokens.md 标明。

## 可回滚比较器事务

改变符号：`Find-RegionShift`、`Find-Boundary` 与区域/颜色报告分支。原始比较器字节保留，patch 重建哈希与修改文件一致；回滚在独立副本执行并验证恢复原始 SHA256。

```text
BASELINE (exit 0): Compared text-basic; pixel differences=0
MODIFIED (exit 0): Compared text-basic; pixel differences=0; regions=16; max deviation=0; color samples=21
ROLLBACK (exit 0): Compared text-basic; pixel differences=0
```
原始/回滚 SHA256：`744a2f2c61243370f3f1c0eb1560a72ff9c2f91333bba7bd7332c2b73f347038`。
首次 patch 哈希校验因 Git autocrlf 转换失败，已统一修改文件 CRLF 并重跑；错误与修正保留在 VERIFICATION.txt。

- MODIFIED_FILE: `C:\Users\admin\orca\workspaces\Matcha\visual-ref\tools\visual\compare.ps1`
- DIFF_FILE: `C:\Users\admin\orca\workspaces\Matcha\visual-ref\docs\visual-parity\transaction\compare.patch`
- VERIFICATION: `C:\Users\admin\orca\workspaces\Matcha\visual-ref\docs\visual-parity\transaction\VERIFICATION.txt`
- ROLLBACK: `C:\Users\admin\orca\workspaces\Matcha\visual-ref\docs\visual-parity\transaction\ROLLBACK.sh`
