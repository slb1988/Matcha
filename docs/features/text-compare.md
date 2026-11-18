# 文本比较（Text Compare / Edit）

> 核心功能：两个文本文件的差异对齐、着色、编辑与导航。状态：**已实现**（双向比较、编辑、
> 行内差异、重要性；三方合并 FR-T7 见 [text-merge.md](text-merge.md)）。对应 FR-T1…T8。

## 功能简介

- 打开两个文件 → 计算行级对齐（`TextMap`），按 `DiffKind` 着色；支持左/右或上/下布局。
- 行内字符级差异（inline diff，FR-T2）、不重要差异（Unimportant，蓝色语义）与重要性传播。
- 就地编辑：UTF-8 字节光标、选区、IME、undo/redo；保存可独立单侧进行。
- 指针在文本区（行号栏右侧）显示 I 形插入光标，拖选途中保持；行号栏仍是默认箭头（可点的合并箭头为手形）。
  选区为不透明的选区蓝底（`theme::TEXT_SELECTION` / `Palette::text_selection`），盖在差异行底色上也清晰可辨。
- 语法高亮来自文件格式库（见 [sessions-and-formats.md](sessions-and-formats.md)）。显示风格由
  `text/syntax_style.rs` 抽象：每类语法元素（`ElementKind`）一个 `ElementStyle`（颜色/粗体/斜体/
  差异行是否保留），`SyntaxStyle::base()` 为对齐参考的基础风格——正文纯黑、注释灰、预处理指令栗色、
  字符串深绿、**关键字黑色粗体**（Consolas Bold，与常规体等宽，列位置不变；未安装时退回常规体）、环境变量紫色；
  按语言的覆盖写在 `LANGUAGE_OVERRIDES`（按格式名，如 HTML 标签用深蓝常规体），新增语言只加一行。
  单侧行（只有本侧有内容的差异行）整行用差异色着墨：不重要元素（默认注释）用不重要差异色，
  `keep_on_diff` 的元素（注释）在被设为重要时保留自身风格，一眼可辨"这是注释"。
  全局字形 gamma `theme::TEXT_GAMMA` = 1.0（原始覆盖率，egui 浅色模式推荐值；此前 0.7 使 Consolas 看着像粗体）。
- **编辑器字体**（参考：Options › Colors, Fonts › Editor Font）：Tools › Options › Appearance 的 **Editor font**
  选字体族（本机已安装的等宽字体，扫描 `%WINDIR%\Fonts` 与用户字体目录，只读字体表头）与字号（6–36 pt，默认
  Consolas 10 pt）；OK/Apply 后文本比较与文本合并立即换字体，并写入 `%APPDATA%\Matcha\settings.txt`
  （`editor.font.family` / `editor.font.size`，与其他 Options 设置一起，见 [gui-shell.md](gui-shell.md)「偏好持久化」），下次启动沿用。View › Text display 的字号滑块改的是同一偏好。
  粗体（关键字）用该字体的 Bold 面；只有可变字重时用 `wght` 700；都没有则用常规体。
  未安装的字体名退回 Consolas。快照/基准模式不读写该文件。
- **中文与编辑器字体对齐**：编辑器字体缺字（CJK）时回退到微软雅黑，按 GDI 字体链接的做法把回退字体缩放到与编辑器字体
  同行高（雅黑 1.32 em → Consolas 1.17 em，约 0.89 倍）并对齐基线（`fonts::editor_cjk_tweak`）；
  此前中文比英文大一号、下沉约 2 px，底部被行裁掉。界面（菜单、对话框）里的中文不受影响。
- **底部行详情**显示两侧当前行；任一行（含行尾符）超宽时，底部短横换成横向滚动条。
  拖滑块、点轨道或左右按钮、在详情区滚动滚轮，都只会横移这两行；侧别箭头固定，上方窗格不跟随。
  点击或拖动上方任一编辑窗格的横、纵滚动条，只移动视口，不换当前对比行、活动侧或选区；原有编辑焦点也会保留。详情滑块可从任意位置抓住拖动。
  两行都能放下时恢复短横，偏移归零。详情仍是每行最多 4 KiB 的预览，超出的部分以 `…` 标记；文件内容与上方编辑区不截断。
- 调整窗口大小时，两侧窗格、可见行数与滚动条按新的客户区重新布局；窄窗口的工具栏换行，不会把右窗格或行详情的滚动条挤出窗口。放宽到能容下当前行时，详情不再横移。
- 视图投影：All / Differences / Same / Context 区段折叠。
- **打开即定位首个差异**：打开文件对（或在会话里换一侧文件）后，首次比较完成时光标与滚动自动落到第一个差异区段的首行
  （与 First Difference 同口径：开启 Ignore Unimportant 时跳过只有不重要差异的区段）；两侧相同则停在首行。
  滚动时差异上方保留最多 5 行上下文（窗格较矮时不超过可见行数的三分之一），差异不会贴在窗格顶边。
  一次性：比较完成前已编辑或移动过光标则不跳；编辑后的重新对齐、Recompare 不会把光标拉回首个差异。
- 鼠标点窗格任意位置（包括只有对侧有内容的空隙行）都会把焦点和活动侧切到该窗格；
  空隙行上光标落到其后本侧第一行行首（`TextCompareView::place_in_gap`）。
- **行号栏复制箭头**（一键合并，外观对齐参考）：每个差异区块只在**首行**画一个 13×9 px、黄色渐变、
  深色描边的块箭头，指向另一侧；区块其余行画一条灰色竖括号线，末行带向右的脚。两侧都画——本侧是空隙
  （只有对侧有内容）的区块也有箭头，点击即用空白覆盖对侧，也就是删除对侧这些行。
  点击箭头把**整个区块**覆盖到另一侧；括号线只是标记，不可点。某侧有选区时，选区所跨的行改为**蓝色**
  选区箭头（选区首行）+ 括号线，点击只把**选中的行**覆盖到对侧对齐行（无对齐行时插入到对应位置），之后清除选区。
  两种都是一次可撤销编辑；点箭头不移动光标、不开始拖选；悬停显示手形光标与提示。
  不能覆盖时箭头为**灰色**（同形状），点击不修改内容，状态栏与悬停提示说明原因：
  目标侧只读——加载时读取文件只读属性（如 Perforce 未 checkout 的文件、p4 打印到临时目录的历史版本），
  该侧信息栏显示下划线的 **Read-only**，点击打开 File properties 可取消只读；编辑后对齐尚未完成时
  （差异着色暂缺）区块箭头暂不显示，选区箭头为灰色，对齐完成后恢复。

## 代码地图

| 层 | 位置 | 内容 |
|---|---|---|
| 引擎 | `crates/bc-engine-text` | `TextContent`（单侧内容+EditStack）、`TextRules`、`TextMap`/`TextMapRow`、`importance`、`inline`、`merge`（已实现，见 [text-merge.md](text-merge.md)）、`jobs`（Align2/Realign2/Align3/Realign3）、`search` |
| GUI | `crates/bc-gui/src/text/` | `view.rs`（控制器/导航/保存）、`render.rs`+`gutter.rs`（自绘）、`syntax_style.rs`（按语言的语法风格）、`editor.rs`、`projection.rs`、`nav_bar.rs`（缩略条）、`worker.rs`（350ms 防抖 Realign2Job） |
| 字体 | `crates/bc-gui/src/chrome/` | `fonts.rs`（字体安装、CJK 回退对齐、运行时换编辑器字体）、`font_catalog.rs`（已安装等宽字体扫描）；`settings.rs`（`settings.txt` 持久化）；`dialogs/options.rs`（Appearance › Editor font） |
| 启动 | `apps/bcompare` | `matcha <left> <right>` 直接进入 |

## 关键语义

- **重要性三态**：`Same / Unimportant / Important`；块级传播规则见 spec §6 与
  `docs/decisions/003-smart-scoring.md`。**默认注释不重要**（与参考默认一致；`ElementImportance::default()`
  的 `unimportant = [Comment]`，GUI、CLI/脚本、文件夹规则比较共用）：只改了注释的行/文件是"不重要差异"。
  可在 Session Settings > Importance 把 Comment 勾回重要；文件夹规则比较用 `comments-important` 开关。
- **差异着色按行**：区块内每行按自身重要性着色（不重要行淡紫底、重要行粉底），与参考一致；
  行本身无分级时退回区块重要性。单侧差异行里，不重要元素（默认注释）的文字用不重要差异色（蓝），
  其余用重要差异色（红）。
- Live `TextContent` 只在 UI 线程；后台 worker 只拿 `snapshot()`（Send/Sync），
  过期修订号的结果直接丢弃。
- diff 显示经过 Myers  packing 与词吸附：见 ADR 005、006。
- 行模型、编码与 mmap 后端在 `bc-core`：见 ADR 001、002。
- 行号栏箭头：判定在 `TextCompareView::gutter_arrow`（种类 Section/Selection + 位置 `GutterMark`
  Arrow/Bracket + 禁用原因），绘制在
  `gutter.rs`，命中在 `render.rs`（箭头区独立于文本行的原始指针处理），点击延迟到本帧行循环之后由
  `apply_gutter_copy` 执行；区块走 `TextMap::copy_section`，选区走与 Copy Line to Other 相同的 `copy_lines`。
- 系统文件拖放：拖到哪一侧（窗格、路径栏、列头均可）就替换哪一侧；规则见 [gui-shell.md](gui-shell.md)「系统文件拖放落点」。

## 使用与验证

```powershell
cargo run --release -p bcompare -- a.txt b.txt                 # GUI
cargo test -p bc-engine-text                                   # 引擎测试
cargo test -p bc-gui --lib text::view::tests::details_          # 行详情绘制与滚动交互
cargo run --release -p bc-engine-text --example text_baseline  # 轻量基线
```

## 深入阅读

- `crates/bc-engine-text/README.md`（API 与缓存/快照约束）
- ADR：`docs/decisions/001…007`（STX 写规则、mmap、智能评分、tokenizer 优先级等）
- 行为对齐：`docs/reference-parity/text-compare.md`、`text-alignment-recovered.md`
- 变更过程：`openspec/changes/text-compare-core/`
