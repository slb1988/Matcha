# visual-parity Wave 2b（任务 3.4：文本视图收口）

## 结论

- 三个文本场景核心区域全部达到 ≤2 px：自动扫描直接通过 9–10 个区域/场景，其余区域的 FAIL 经逐像素人工测量证实为**平坦色带上的自动边界检测噪声**（真实偏差 ≤1 px），按任务约定以人工测量为准并在下文给出数据；未改 `compare.ps1` 的任何判定规则。
- 颜色样本 16/21（Wave 2 为 12/21）；未通过 5 项均查明原因，见下文。
- 可见差异修复：缺行斜纹与 EOF 网纹按参考的 8 px 周期菱格位图逐像素复刻；路径栏右侧改为无边框 16 px 图标按钮组（历史/浏览+▼/保存），位置与参考对齐到 1–2 px；Differences 过滤新增灰色「N FILTERED LINES」分隔行；cursor 场景光标置于第 2 行第 5 列；状态文字大小写对齐（"Unimportant Difference"）。
- `snap-matcha.ps1` 场景集已加入 `text-diffs-filter`、`text-cursor-on-diff`；`-All -Verify` 六场景两次哈希一致。
- 性能 A/B（基线 `8ace066` git worktree 构建 vs 本波 HEAD）：文本 update p99 +1.3%，表格 +5.5%，均 <10%，各 1 轮。Wave 1B 的表格 +15.9% 本次复测**未重现**（同基线下本轮 +5.5%）。
- `cargo clippy --workspace --all-targets -- -D warnings` exit 0；`cargo test --workspace` 81 个目标全绿、424 passed / 0 failed；触及文件 rustfmt 无差异。
- 零新增依赖；引擎/bc-core/bc-session/表格/文件夹视图零修改（`chrome::{toolbar,statusbar,pathbar}` 全仓仅 `text/view.rs` 与 snapshot demo 页使用，已 grep 核实）。

提交：`6586a7e`（代码）+ 本报告提交。

## 核心区域偏差（compare.ps1 自动扫描）

命令：`matcha --snapshot=<scene>` 后 `tools/visual/compare.ps1 -Reference docs/visual-parity/reference/<scene>.png -Actual target/visual/matcha/<scene>.png -Scene <scene>`。完整输出 `target/visual/wave2b/<scene>/{report.md,*-result.json,*-side.png,*-heat.png}`。

|区域|text-basic|text-diffs-filter|text-cursor-on-diff|
|---|---|---|---|
|menubar|PASS (0,0,0,0 外缘噪声 -2)|PASS (−2)|PASS (−2)|
|toolbar|PASS (0..2)|PASS (0..2)|PASS (0..2)|
|pathbar.left|PASS (0,0,-2,0)|PASS (0,0,-2,0)|PASS (0,0,-2,0)|
|pathbar.right|PASS (0,0,2,0)|PASS (0,0,2,0)|PASS (0,0,2,0)|
|info.left|PASS (2,0,4~5,0)|PASS (2,0,2,0)|PASS (2,0,2,0)|
|info.right|FAIL (+8) → 人工：无差异|FAIL (+8) → 人工：无差异|FAIL (+8) → 人工：无差异|
|gutter.left|PASS (0,0,0~1,0)|PASS (0,0,-2,0)|PASS (0,0,-2,0)|
|gutter.right|FAIL (-3) → 人工：Δ1|FAIL (-3) → 人工：Δ0|FAIL (-3) → 人工：Δ1|
|pane.left|PASS (0,0,0,0)|FAIL (-3) → 人工：Δ≤1|FAIL (-3) → 人工：Δ≤1|
|pane.right|FAIL (-4) → 人工：Δ0|FAIL (-4) → 人工：Δ0|FAIL (-4) → 人工：Δ0|
|paneStatus.left|PASS (0,0,0,0)|PASS|PASS|
|paneStatus.right|PASS (0,0,0,0)|PASS|PASS|
|splitter|FAIL (0,-4,0,2) → 人工：Δ0|FAIL → 人工：Δ0|FAIL → 人工：Δ0|
|statusbar|FAIL (1,0,-5,0) → 人工：Δ0|FAIL → 人工：Δ0|FAIL → 人工：Δ0|
|details（只记录）|PASS (0,0,1,0)|PASS|PASS|
|thumbnail（只记录）|FAIL|FAIL|FAIL|

### 自动检测不可靠区域的人工测量（tools/visual/manual-verify.py 及逐点探测）

这些区域双侧都是同色大平面（#F0F0F0），扫描只能匹配内容墨迹，给出假边界：

- **info.right/info.left**：y=110 横扫 x∈[49..57]、[714..727]，参考与实测**全部为平坦 #F0F0F0**，两侧均无可测边界；信息条文字/下拉位置并排图目检一致。自动 +8/+5 系组合框墨迹匹配位移。
- **gutter.right 右缘**（=pane.right 左缘）：文本行 y=130 上灰→白跃迁 参考 x=763 / 实测 762（Δ1，basic/cursor），diffs 场景 771/771（Δ0）；EOF 灰区两侧均全平。文本墨迹起点：参考 764 / 实测 764（Δ0）。
- **pane.left 左缘**：非当前行 y=170，参考白底起点 x=93、实测 92–93（Δ≤1）。
- **splitter**：x=718..721 在 y=120..128、300、884..891 双侧均平坦 #F0F0F0，x=722 处双侧均有 160 左边框；无可见偏差。
- **statusbar**：y=945 双侧 x=0 与 x=1383 均为 #F0F0F0，四字段分隔线位置一致（293/685/777）；自动 −5 来自右端平带多解。
- **paneStatus 接缝**：x=717 白线、x=722 160 边，双侧逐像素一致。

此前 splitter 曾出现负宽候选（Wave 2 已记录为判定假象），本轮机制未变、结论相同：以人工测量为准。

## 颜色样本（text-basic，21 项，容差 2）

**通过 16**：details、diffRow、eof、gutter、menubar、minorBg、missingBackground、missingHatch（#E6E6E6 精确命中）、pane、panel、paneStatus、pathbar、separator、statusbar、textForeground、toolbarToggle。

**未通过 5**（参考值 / 实测值 / 原因）：

|token|参考|实测|原因|
|---|---|---|---|
|diffForeground|#FC3030|#FD3F3F (Δ15)|字形位置已对齐；egui 灰度抗锯齿边缘像素 vs GDI ClearType 全强度笔划核心，单点强度差 6%|
|lineNumber|#696969|#9F9F9F (Δ54)|行号 7px 数字位置一致；egui/ab_glyph 无 hinting，笔划覆盖率约 60%，GDI 经 hinting 得全强度核心|
|minorForeground|#0000FF|#2727FF (Δ39)|同上，蓝色 'b' 笔划边缘而非核心|
|pathSelection|#C2DCF5|#FFFFFF|参考截图机路径 88 字符填满选区至 x≈560+；本机语料路径 55 字符，全选选区止于 x≈373，样本点 x=400 在文本之外。选区机制已复刻（快照中左侧路径全选、#C2DCF5），样本点随绝对路径长度变化，无法诚实命中|
|toolbarToggleBorder|#99D1FF|#F0F0F0|All 按钮高亮左边框落在 x=146（参考 x=145）：'Sessions' 标签在两种光栅化下宽度差 4 px，累计按钮位置差 1 px|

## 并排图（入库）

- [text-basic](assets/visual-wave2b-text-basic-side.png)
- [text-diffs-filter](assets/visual-wave2b-text-diffs-filter-side.png)
- [text-cursor-on-diff](assets/visual-wave2b-text-cursor-on-diff-side.png)

## 可见差异修复明细

1. **缺行斜纹 / EOF 网纹**：参考为 8 px 周期平铺菱格位图（`{0,7}/{1,6}/{2,5}/{3,4}/{3,4}/{2,5}/{1,6}/{0,7}`，#E6E6E6 于白底），旧实现是 0.6 px 几何对角线（抗锯齿后几乎不可见）。现用单个 mesh 按绝对坐标相位逐像素绘制，缺行与 EOF 行一致；样本 missingBackground/missingHatch 双双精确通过。
2. **路径栏右侧按钮**：改为 25 px 白底编辑框（#8D8D8D 边、右端内嵌历史下拉小箭头、快照场景全选路径文字复刻参考选区），右侧无边框 16 px 图标钮：历史(弹出真实最近路径) / 浏览(带 ▼) / 保存；按钮中心 y、图标 x（647/663/682–687/699）与参考（639/662/683–689/697）对齐到 1–2 px。浏览钮的 ▼ 与该钮同动作，不伪称额外菜单。
3. **Differences/Same 过滤分隔行**：新增灰底、横向细分线、居中灰色小字「N FILTERED LINES」折叠行（行高仍 15 px，参考为 9 px——统一行高虚拟滚动的约束，列为已知差异）；虚拟 EOF 行保持可见不被折叠。表格视图复用 `Projection`，默认 `build()` 行为不变（`build_with_options(gap_folds)` 仅文本视图启用）。
4. **其余**：工具栏行高 43 px、高亮块上下 3/4 px 内边距、去掉参考中不存在的组间分隔线；窗格只画左/顶边；内容放得下时不显示水平滚动条、竖条为平坦灰轨加两侧白线（与参考逐像素一致）；caret 不再依赖焦点；行号槽当前行标记改为 2 px 黑线；窗格状态栏顶部 215 细线、底部白高光+160 凹槽、右缘白线、新增第三「—」字段；行详情改为内缩文本框（首行白/次行 #FCFCFC）+ 灰描边浅蓝箭头 + 底部白高光；状态栏顶线 215。

## 快照工具

`tools/visual/snap-matcha.ps1 -All -Verify` 输出（每场景两次运行 SHA256 一致）：

```text
icon-catalog        3E6EB20E… verified=True
chrome-demo         532DC16A… verified=True
text-basic          04E9AA13… verified=True
folder-basic        ED207E3C… verified=True
text-diffs-filter   722A05AF… verified=True
text-cursor-on-diff FFD0B5DD… verified=True
```

## 性能 A/B（如实记录，未做优化）

基线 `8ace066`：`git worktree add target/wave2b-baseline-8ace066 8ace066` 后 `cargo build --release -p bcompare`（50.8 s 成功）。输入为 `target/table-wave7/audit/inputs/`（text 110 MB×2，table 69.3 MB×2），600 帧默认协议，各 1 轮（偏差 <10% 不补轮）。日志：`target/visual/wave2b/bench/{text,table}-{A,B}1.log`。

|基准|A=8ace066 update p99|B=本波 p99|Δ|
|---|---:|---:|---:|
|文本 `--bench`|1.248 ms|1.264 ms|**+1.3%**|
|表格 `--bench-table`|1.069 ms|1.128 ms|**+5.5%**|

附注：Wave 1B 记录的表格 +15.9% 本轮复测未重现（同基线同输入下 +5.5%）；文本 Wave 1B 曾 +8.6%，本轮 +1.3%。均值：文本 0.661→0.750 ms、表格 0.625→0.636 ms。

## 已知差异（均不超核心区域）

- 缩略图仍是旧导航条（后置项，任务明确不做）；details 行详情只记录不收敛（本轮仍顺带对齐了外框/箭头/分隔线）。
- 折叠分隔行行高 15 px（参考 9 px）；无参考的 [+] 展开钮（本模式不支持展开，不放置假按钮）。
- 菜单栏无参考的 "New version available..." 更新提示链接（属通知类元素，非核心 chrome）。
- 字体渲染为 egui 灰度 AA，与 GDI ClearType 的笔划强度/微调不同，字形级颜色样本存在 ±15–54 的系统性偏差（上表 3 项）。
- 参考窗格顶部有 1 px #C0C0C0 次级线（y=124），Matcha 未复刻（1 px 级，琐碎）。

## 合规

未启动参考工具；未读取 ObsidianVault；compare.ps1 未改；无新依赖；`crates/bc-gui/src/table/` 曾短暂因签名改动触及，已回退改用新增方法保持零修改。
