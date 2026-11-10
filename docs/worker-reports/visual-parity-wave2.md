# visual-parity Wave 2（按协调者要求提前收尾）

## 结论

- 完成 3.1–3.3；3.4 只完成图标/箭头修正、三场景与离线对照，**区域 ≤2px / 性能 A/B 尚未验收，保持未勾选**。
- 协调者 `msg_c9221e8e2075` 要求额度紧张立即收尾，已停止新校准和基准轮次。整体任务 outcome 为 failed（部分交付），不是实现全部通过。
- 最终 `cargo clippy --workspace --all-targets -- -D warnings` exit 0；`cargo test --workspace` exit 0，累计 **424 passed**；触及 Rust 文件 rustfmt 检查 exit 0。新增测试保留旧编辑/保存/颜色自定义行为，并验证菜单动作、工具栏鼠标过滤、快照真实投影及核心布局。
- 无新增依赖；引擎、bc-core、bc-session 零修改；未访问 ObsidianVault，未启动参考工具。文件夹/表格/Home 未重排。共享 chrome 改动只影响使用它的组件。

## 完成清单与提交

|任务|提交|结果|
|---|---|---|
|3.1|`c5a3a2b`|七个顶层菜单完整静态库存、未实现叶子禁用、已有动作接线；图标工具栏替换旧按钮行；文本中移除 app Home 顶条|
|3.2|`b4c0832`|路径/历史/浏览/保存、时间/字节/格式/编码/行尾、52px 缩略栏、40px 双槽、15px 行高、差异文字/背景、斜纹/网纹|
|3.3|`e618cd6`|两侧行列/类别、两行详情、四栏状态、文件名标题；调试状态仅基准模式显示|
|3.4 部分|`e87c3ba`|原创图标加粗、Painter 下拉箭头、三场景、并排图、几何/颜色记录；未完成性能与像素门槛|

所有提交含 Codex Co-Authored-By。报告提交另列于 worker_done，不自引用。

## 旧入口 → 新入口

|旧入口|新入口|
|---|---|
|First difference / Previous / Next / Last|Search → First difference / Previous Difference Section / Next Difference Section / Last difference；Next/Prev 工具按钮；原 F7/Shift+F7 导航保留|
|Copy to Left / Copy to Right|Edit 同名项；工具栏 Copy 复制至另一侧|
|Swap Sides / Recompare|工具栏 Swap、Session → Swap Sides / Recompare Files（Ctrl+F5）|
|Find / Replace|Search → Find / Replace，Ctrl+F/Ctrl+H；Find Next/Previous + F3/Shift+F3|
|Settings / File formats|Rules、Session Settings；Format 主按钮/Tools → File Formats；Format 下拉绑定已有格式选择|
|Save active|File → Save File / Ctrl+S；每侧路径栏 Save|
|All / Differences / Same / Context|工具栏 All/Diffs/Same/Context 与 View 同名项，状态同步|
|Ignore unimportant|Minor / View → Ignore Unimportant Differences|
|Over-Under|View → Over-Under / Side-by-Side Layout|
|Line numbers / Visible whitespace / Syntax highlighting / Font size|View 同名项；View → Text display → Font size|
|Align With / Clear manual alignment / Compare Parent Folders|Edit → Align With / Clear manual alignment；Session → Compare Parent Folders（脏文档禁用）|
|Go to 行列 + Go|Search → Go To（Ctrl+G），保留行列输入和 Go|
|旧每侧 File 下 Open / Save as / Save target / Reload / Clipboard / Read-only / 编码 / BOM / 行尾转换|File 主菜单 + File properties（当前侧），路径栏打开/保存、信息头编码；Edit → Convert File → Line Endings 接原转换动作|
|旧格式下拉 Detected / Plain text / 自定义格式|每侧信息头格式下拉，工具栏 Format 下拉|
|旧多行状态：编码、行尾、只读、修改、解码错误、差异计数、消息、保存目标|Session → Text Compare Info（含行尾/最终行尾不一致警告）；基础编码/行尾与行列同时在头部/窗格状态|
|app Home 顶条|工具栏 Home；Sessions 下拉可回 Home，其余未实现会话操作禁用|

不新增 Overwrite 编辑功能，当前编辑器只支持 Insert，状态如实显示 Insert。路径浏览仍调用项目原有路径输入窗口，不伪称原生选择器。

## 三场景证据与偏差

- [text-basic](assets/visual-wave2-text-basic-side.png)
- [text-diffs-filter](assets/visual-wave2-text-diffs-filter-side.png)
- [text-cursor-on-diff](assets/visual-wave2-text-cursor-on-diff-side.png)

三张均已重新打开人工查看。命令 `matcha --snapshot=<scene>` 后运行 `tools/visual/compare.ps1 -Reference docs/visual-parity/reference/<scene>.png -Actual target/visual/matcha/<scene>.png -Scene <scene>`；不修改参考或比较脚本。

以下为脚本实际报告的四边最大绝对偏差（px），**不是通过表**。固定布局测试确认菜单27/工具43、路径26/信息27、左窗格 x52/y123，底部19/49/25；扫描仍因边框/滚动条/字号与平坦背景等差异偏移，不能用布局常量覆盖实测失败。

|区域|basic|diffs|cursor|
|---|---:|---:|---:|
|details|7|1|7|
|gutter.left|3|3|3|
|gutter.right|3|3|3|
|info.left|2|4|2|
|info.right|8|8|8|
|menubar|0|1|0|
|pane.left|4|4|4|
|pane.right|4|4|4|
|paneStatus.left|8|8|8|
|paneStatus.right|5|5|5|
|pathbar.left|2|2|2|
|pathbar.right|4|4|4|
|splitter|8|8|8|
|statusbar|4|4|4|
|thumbnail|7|3|7|
|toolbar|8|8|2|

颜色固定坐标：text-basic **12/21 通过**，其余两场景脚本未配置颜色样本（0），不可称为颜色全通过。非文字大面板 panel/menubar/pane/eof/gutter/minorBg/diffRow/paneStatus/details/statusbar 使用对应 token；差异字色 #FC3030、minor #0000FF、斜纹 #E6E6E6 等在代码中直接绑定，但字形/纹理落点与参考不同，不能把常量一致当固定像素抽样通过。

## 已知差异 / 未完成

- 自动扫描最大8px，多数区域未达≤2px；splitter 在平坦色带甚至给出负宽候选，已保留报告原值，不篡改。完整四边表与颜色样本：`target/visual/wave2-transaction/<scene>-compare.md`。
- 缩略图仍是旧导航桶伸展显示，不是参考的顶部微型文档；宽度/位置对齐，内容未精调。行详情没有按字符复刻，箭头/行尾符不同。
- egui 灰度字体、原创矢量图标、组合框/滚动条/状态分栏边框不等同 Win32；水平滚动条短文件仍占位。路径真实根目录不同、默认选择高亮不同。
- 三个文本快照显式设置 `case_unimportant=true`，获协调者 `msg_3554b5ea9977` 批准；正常启动保持旧默认，beta/BETA 可能分成左右孤儿行。这是另立的默认规则跟进项，不用快照掩盖普通启动差异。
- Diffs 快照确实关闭 Context 并过滤，但没有参考隐藏行分隔提示，光标未自动移动到首个可见差异；cursor 场景选了真实第2行，列1而参考为列5；这些交互/细节未继续打磨。
- 新增场景可直接 CLI 运行；未越权改 `snap-matcha.ps1` 的 ValidateSet，它仍只含 Wave1B 场景。未做新增场景重复字节一致性复验。
- 时间在加载时用隐藏 PowerShell 格式化系统本地时间，失败显示明确 UTC 回退；无逐帧I/O。首次 clippy items-after-tests 与 deprecated Id 错误、工具栏坐标迁移测试失败已修复并最终全量通过；原日志留在事务目录。

## 性能 A/B（未完成）

A=`8ace066` 已从 Git archive 解包并成功构建，B=`e87c3ba` 已成功构建；基准二进制与原有110MB文本/69MB表格语料已定位。**没有开始测量轮次**，收到协调者停止指令后未运行任何新基准；无可报告 p99 或百分比。Wave1B 的表格 +15.9% 仍未复测，不声称改善或达标。

## 最后十行

`cargo clippy --workspace --all-targets -- -D warnings`，exit0：
```text
    Checking bc-gui v0.1.0 (D:\Github\Matcha\crates\bc-gui)
    Checking bcompare v0.1.0 (D:\Github\Matcha\apps\bcompare)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.83s
```
`cargo test --workspace`，exit0（stdout 最后十行）：
```text

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```
触及 Rust 文件 `rustfmt --edition 2021 --check --config skip_children=true <files>` exit0，stdout/stderr 空；不格式化未触及的历史文件。

## 可回滚事务（只作用副本）

TARGET 始终是 `D:/Github/Matcha`，原始 `69d91cd` 完整归档未改。四角色：
- `D:/Github/Matcha/target/visual/wave2-transaction/MODIFIED_FILE.tar`
- `D:/Github/Matcha/target/visual/wave2-transaction/DIFF_FILE.patch`
- `D:/Github/Matcha/target/visual/wave2-transaction/VERIFICATION.txt`
- `D:/Github/Matcha/target/visual/wave2-transaction/ROLLBACK.sh`

同一 `cargo test -p bc-gui text:: -- --nocapture`：BASELINE 18 passed / exit0；MODIFIED 21 passed / exit0；ROLLBACK 18 passed / exit0（独立副本恢复后）。Git binary patch 已重建修改归档，ROLLBACK.sh 已实际执行，恢复 SHA256=BASELINE；重应用副本 SHA256=MODIFIED。四角色重新打开验证后交付。
