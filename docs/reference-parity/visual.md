# 外观对照：核心版（visual-parity 7.1）

更新：2026-11-11。范围按用户收窄（抓大放小）：共享 chrome + 文本比较主视图 + 文件夹比较主视图。后置项见 `docs/visual-parity/HANDOFF.md` 第 1 节。

## 口径

- 参考：默认英文、浅色、100% 缩放，客户区 1384×961；一次性采集的 23 张参考图 `docs/visual-parity/reference/`（采集后不再启动参考工具）。
- 对照：`tools/visual/compare.ps1` 命名区域边界偏差（目标 ≤ 2 px）+ 颜色样本；自动 FAIL 落在平坦色带上的以人工像素测量为准。**字形栅格化差异不计**（egui 灰度 AA vs GDI ClearType）。
- 字体：UI 跟随系统消息字体（本机 Microsoft YaHei UI 9pt），编辑器 Consolas 10pt。图标为原创矢量绘制。

## 结果

| 视图 | 场景 | 核心区域 | 颜色样本 | 并排图 |
|---|---|---|---|---|
| 文本 | text-basic / text-diffs-filter / text-cursor-on-diff | menubar、toolbar、pathbar、info、gutter、pane、paneStatus、statusbar 均 ≤ 2 px（部分自动 FAIL 经人工测量为色带噪声，≤ 1 px） | 16/21；未过 5 项为字形 AA 强度、路径长度、按钮 1 px | `../worker-reports/assets/visual-wave2b-*-side.png` |
| 文件夹 | folder-basic / folder-expanded / folder-diffs-filter | 12 个命名区域三场景均 ≤ 2 px（headers/trees/splitter/statusbar 精确） | folderSelection/folderStripe 精确；expanded 5 项为字形 AA | `../worker-reports/assets/visual-wave3-*-side.png` |

回归（协调者 2026-11-11 复核）：`cargo clippy --workspace --all-targets -- -D warnings` 零警告；`cargo test --workspace` 429 passed / 0 failed；`snap-matcha.ps1 -All -Verify` 8 场景两次哈希一致（Wave 3 报告）。

性能（同轮 A/B，各 1 轮）：文本 `--bench` update p99 +1.3%（Wave 2b）/ +2.3%（Wave 3）；表格 `--bench-table` +5.5%（Wave 2b，Wave 1B 的 +15.9% 未复现）；文件夹 `--bench-dir` −15.6%。均在 10% 阈值内。

## 已知差异

- 字形：笔划强度、行号灰度（AA 与 ClearType 差异，不计）。
- 文本：缩略图仍为旧导航条（后置）；「N FILTERED LINES」折叠行 15 px（参考 9 px）；行详情箭头/行尾符形状；菜单栏右侧无"更新提示"链接（有意不做）。
- 文件夹：孤儿条目排序位置与参考不同（属比较行为，未在外观波次修改）；日志区内容为 Matcha 自有日志。
- 控件：组合框、滚动条、状态分栏边框为 egui 近似，非 Win32 原生。

## 跟进项（不在本 change）

- 默认比较规则对齐参考（大小写默认不重要等）；文件夹孤儿排序对齐参考。
- 后置外观：同步视图、Home、表格、对话框、右键菜单、标签类、缩略图。

## 证据索引

报告：`docs/worker-reports/visual-parity-wave{1A,1B,2,2b,3}.md`；token 真源：`docs/visual-parity/tokens.{json,md}`；交接：`docs/visual-parity/HANDOFF.md`。
