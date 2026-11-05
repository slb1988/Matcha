# 文本比较：参考帮助逐项对照（Wave 4）

## 范围与证据

依据 design D10，直接只读解包本机 `D:\Program Files\Beyond Compare 5\BCompare.chm` 的指定 HTML 到系统临时目录，逐项核对 `commandstext.html`、全部 `sessiontext*.html`、`viewtext.html`，并核对文本格式五个主题、`grammars.html`、`sample_regular_expressions.html`。没有读取 vault、授权/密钥文件，没有写安装目录，也没有为本轮清单运行参考程序。STX 行为证据复用 Wave 3b 的受控观察，修正见 ADR 001。

状态含义：**已实现**＝本期有真实入口/引擎行为及回归；**有意未采纳**＝与本项目本期设计冲突或被等价入口替代，写明理由；**后置**＝有价值但属于明确的后续能力，不宣称当前可用。未把隐藏命令（帮助中的 `*`）忽略。对原 HTML 的 `p_Command` 标签做机械核对：108 个标签（含复合菜单分段）在下文均有映射，遗漏为 0。命令名称仅为内部对照标签，不是参考工具品牌或图标的分发。

主要实现：`crates/bc-gui/src/text/`；正式控件替换 `text_view.rs`。验证：18 个 GUI 测试、122 个 workspace 测试、Myers 30/30、Standard 29/30、两份 110 MB 原生基准；详见 [Wave 4 报告](../worker-reports/text-compare-core-wave4.md)。这里的“后置”没有缩减本 change 的 SHALL 要求；原生 IME 候选窗是单独的环境受限验收项。

## commandstext.html — Session

| 命令 | 状态 | 对应实现或理由 |
|---|---|---|
| Save Session | 后置 | session-and-cli，会话持久化非本 change |
| Save Session As | 后置 | 同上 |
| Session Settings | 已实现 | Settings：对齐、重要性、规则、上下文 |
| Locked | 后置 | 与会话持久化一起实现；不混同文件只读 |
| Clear Session | 有意未采纳 | 当前以 Home 返回路径页，保留未保存确认；不另建空会话命令 |
| Swap Sides | 已实现 | 内容、标题、只读、编辑器、保存目标交换；后台刷新镜像对齐 |
| Reload Files | 已实现 | 各侧 File → Reload，有修改先保存/放弃/取消；不隐式丢失另一侧编辑 |
| Recompare Files | 已实现 | 保留编辑并重算；刻意保留撤销历史，不照搬参考帮助“not undoability” |
| Text Compare Report | 后置 | hex-and-reports |
| Text Compare Info | 已实现 | 常驻状态区三态区块数、左右孤立行、编码、行尾、格式 |
| Compare Files Using | 后置 | 其他视图类型接入后启用 |
| Merge Files | 后置 | text-merge-and-ai，三路合并 |
| Compare Parent Folders | 后置（条件项） | 本分支尚无文件夹视图，按钮禁用；ViewAction 已留接线类型，文件夹线合并时启用 |

## commandstext.html — File

| 命令 | 状态 | 对应实现或理由 |
|---|---|---|
| Open File | 已实现 | 每侧 File → Open，路径框+最近列表；拖放等价入口 |
| Open File on File System * | 已实现 | 本期 Open 只接受本地/系统挂载路径，无需第二个重复命令 |
| Open File Using Profile * | 有意未采纳 | VFS 远程 profile 不在本期 |
| Open Clipboard | 已实现 | RequestPaste → Paste 事件加载指定侧；未保存内容有保护 |
| Open With | 后置 | session-and-cli 外部应用注册 |
| Open With > Associated Application | 后置 | 同上 |
| Open With > Text Edit | 有意未采纳 | 就地编辑已覆盖，不另立单文件编辑产品窗口 |
| Open With > Text Patch | 后置 | 补丁视图不属于本次双向比较 |
| Save File | 已实现 | Ctrl+S/Save active/每侧 Save，仅选中侧标记已保存 |
| Save File As | 已实现 | 新路径成为来源，原文件不覆盖；清除旧保存目标 |
| Save File on File System As * | 已实现 | 当前 Save as 的本地文件入口 |
| Save File Using Profile As * | 有意未采纳 | 无远程 VFS profile |
| Explorer | 后置 | 外壳上下文菜单非本期 |
| Check In | 有意未采纳 | SCC 集成明确非目标 |
| Check Out | 有意未采纳 | SCC 集成明确非目标 |
| Undo Check Out | 有意未采纳 | SCC 集成明确非目标 |

本项目补充：**Save target** 保留原来源路径，后续 Save 写入指定目标；Prefs 备份开关与后缀；原路径保存必须 cancel/join + 全窗格共享 mmap 物化，见 ADR 002。

## commandstext.html — Edit

| 命令 | 状态 | 对应实现或理由 |
|---|---|---|
| Undo | 已实现 | Ctrl+Z，目标侧 EditStack；重新着色 |
| Redo | 已实现 | Ctrl+Y / Ctrl+Shift+Z |
| Align With | 已实现 | View / alignment 中选择，再点击另一侧行；锚点保留至清除 |
| Isolate | 后置 | 当前覆盖模型只有强配锚点；孤立排除约束需要扩展模型 |
| Replacement | 已实现（设置入口） | Settings → Replacements 可新建；从当前差异自动填充的快捷命令后置 |
| Copy to Right * | 已实现 | 当前差异区块或来源侧选中的行，目标侧一次撤销 |
| Copy to Left * | 已实现 | 同上 |
| Copy to Other Side | 已实现（明确方向按钮） | 两个固定方向按钮避免动作方向歧义，不再增加同义动态按钮 |
| Copy Line to Right * | 后置 | 当前可选中该行再 Copy to Right；独立单行快捷命令后置 |
| Copy Line to Left * | 后置 | 同上 |
| Copy Line to Other Side | 后置 | 同上 |
| Increase Indent | 后置 | 批量缩进；当前支持 Tab 插入（尊重格式 tab-as-spaces） |
| Decrease Indent | 后置 | 批量反缩进 |
| Cut | 已实现 | 系统 Cut 事件，选区跨行，写保护检查 |
| Copy | 已实现 | 系统 CopyText 输出；只读侧可复制 |
| Paste | 已实现 | Paste 事件，换行使用目标主导风格 |
| Delete | 已实现 | Delete / Backspace，选区/UTF-8 字符/跨行合并 |
| Delete Line * | 后置 | 当前选区删除可等价完成；独立快捷命令后置 |
| Delete to Start of Line * | 后置 | 当前 Shift+Home 后删除可完成；独立快捷命令后置 |
| Delete to End of Line * | 后置 | 当前 Shift+End 后删除可完成；独立快捷命令后置 |
| Delete Word * | 已实现（键盘） | Ctrl+Delete，按字母数字/下划线词类分界 |
| Delete to Start of Word * | 已实现（键盘） | Ctrl+Backspace |
| Delete to End of Word * | 已实现（键盘） | Ctrl+Delete |
| Insert Line Before * | 后置 | Home+Enter 可编辑，单独命令后置 |
| Insert Line After * | 后置 | End+Enter 可编辑，单独命令后置 |
| Select All | 已实现（有意差异） | Ctrl+A 选择完整文档，而非仅可见行，避免过滤造成意外部分操作 |
| Select Section | 后置 | 当前区块复制不需先选择；独立选区命令后置 |
| Compare Selection to Clipboard | 有意未采纳 | 新比较窗口的剪贴板比较属于 proposal 中暂不采纳项；Open Clipboard 仍按 view spec 实现 |
| Convert File > Trim Trailing Whitespace | 后置 | 批量文本转换不是本期必要编辑行为 |
| Convert File > Leading Spaces to Tabs | 后置 | 同上 |
| Convert File > Tabs To Spaces | 后置 | 同上 |
| Convert File > Line Endings | 已实现 | 每侧 File 菜单 CRLF/LF/CR，全文件一次可撤销编辑 |

## commandstext.html — Search

| 命令 | 状态 | 对应实现或理由 |
|---|---|---|
| Next Difference | 后置（字符级定位） | 当前 Next/F7 按区块定位，满足 view/alignment spec；词内下一差异另做 |
| Previous Difference | 后置（字符级定位） | 同上 |
| Next Difference Section | 已实现 | Next / F7；首尾不循环，明确提示 |
| Previous Difference Section | 已实现 | Previous / Shift+F7；另有 First/Last |
| Next Difference Files | 后置 | 文件夹父子会话导航 |
| Previous Difference Files | 后置 | 同上 |
| Copy File to Right and Open Next Difference | 后置 | 同上，且需文件级覆写确认 |
| Copy File to Left and Open Next Difference | 后置 | 同上 |
| Next Replacement | 后置 | 规则匹配位置专用导航 |
| Previous Replacement | 后置 | 同上 |
| Next Edit | 后置 | 当前 EditStack 不记录可导航编辑标记 |
| Previous Edit | 后置 | 同上 |
| Find | 已实现 | Ctrl+F / Find panel；字面/正则/大小写/全词/方向/仅差异 |
| Replace | 已实现 | 当前命中替换；正则捕获展开 |
| Find Next | 已实现 | Next / F3；零宽命中显式前进，不死循环 |
| Find Previous | 已实现 | Previous / Shift+F3 |
| Go To | 已实现 | 行:列输入框，UTF-8 字符列转字节光标 |
| Toggle Bookmark | 后置 | 独立书签状态/持久化 |
| Go To Bookmark | 后置 | 同上 |
| Clear Bookmarks | 后置 | 同上 |

## commandstext.html — View

| 命令 | 状态 | 对应实现或理由 |
|---|---|---|
| Show All | 已实现 | All |
| Show Differences | 已实现 | Differences；来源行号不重新编号 |
| Show Same | 已实现 | Same |
| Show None * | 有意未采纳 | 空白视图无分析收益；本期只保留规范的四种过滤 |
| Show Context | 已实现 | 前后 N 行，默认 3；折叠分隔符点击展开 |
| Ignore Unimportant Differences | 已实现 | 颜色、导航、过滤、计数投影；原文本不变 |
| Ignored | 后置 | 任意选区临时忽略的区段模型独立于元素规则 |
| Visible Whitespace | 已实现 | 空格/Tab/EOL 标记；EOF 无换行标记和状态提示 |
| Line Numbers | 已实现 | 可显示/隐藏，空隙不伪造来源行号 |
| Syntax Highlighting | 已实现 | 可开关；语法 token 的文字颜色，差异背景独立 |
| Word Wrap | 后置 | 本期固定行高虚拟化；长行水平窗口保证性能，变高行索引另做 |
| Display Font * | 后置（字体族） | 目前固定等宽字体，Windows CJK 系统后备；任意字体族选择后置 |
| Increase Display Font Size * | 已实现 | View / alignment 字号滑块 |
| Decrease Display Font Size * | 已实现 | 同上 |
| Reset Display Font Size * | 后置（独立命令） | 当前手动滑块回到 13；独立重置快捷键后置 |
| Side-by-Side Layout | 已实现 | Over-Under 未勾选 |
| Over-Under Layout | 已实现 | 左在上、右在下；共用双轴偏移 |
| Webpages | 有意未采纳 | 不引入浏览器/远程页面执行面；文本控件不展示网页 |
| Thumbnail | 已实现 | 常驻色条、可视区框和当前行；隐藏开关后置 |
| Text Details | 后置 | 独立底部文字详情面板 |
| Hex Details | 后置 | hex-and-reports |
| Alignment Details | 后置 | 字符对齐可编辑详情面板 |
| Ruler | 后置 | 详情尺，非基础行号栏 |
| File Info | 已实现 | 每侧标题/格式选择与双侧状态区常驻；隐藏开关后置 |
| Toolbar | 已实现 | 命令栏常驻；定制/隐藏工具栏后置 |

帮助顶部的 **Tools > Options > Commands**（自定义键位、隐藏/显示菜单/工具栏）后置到全局偏好扩展；当前键位见 GUI README。这里没有把“存在同义操作”冒称独立快捷命令已实现。

## sessiontext*.html — 比较会话各选项

| 主题/每项 | 状态 | 实现或理由 |
|---|---|---|
| sessiontext：Specs / Format / Importance / Alignment / Replacements | 已实现 | Settings、每侧 File/格式菜单和路径对话框；不复制原对话框标签页布局 |
| sessiontextspecs：左/右文件路径 | 已实现 | Home 与每侧 Open |
| sessiontextspecs：Disable editing | 已实现 | 每侧 Read-only；初次读取文件权限 |
| sessiontextspecs：Description | 后置 | 会话持久化时保存说明 |
| sessiontextformats：左右格式 | 已实现 | Detected / Plain text / 任意文本格式 |
| sessiontextformats：Detected | 已实现 | 注册表顺序+文件掩码 |
| sessiontextformats：Encoding overrides / None | 已实现 | 初次自动检测、每侧手动重解码；当前菜单使用显式编码，不额外显示 None 标签 |
| sessiontextimportance：Unimportance list | 已实现 | 字面/正则、大小写、可限定语法元素 |
| sessiontextimportance：Grammar elements | 已实现 | 10 类元素重要性复选框，会话覆盖 |
| sessiontextimportance：Leading whitespace | 已实现 | 独立开关 |
| sessiontextimportance：Embedded whitespace | 已实现 | 独立开关 |
| sessiontextimportance：Trailing whitespace | 已实现 | 独立开关 |
| sessiontextimportance：Everything else | 有意未采纳（开关） | 本期规范未分类文字始终重要，不能一键忽略全部未分类内容 |
| sessiontextimportance：Character case | 已实现 | 全局开关，语法元素固有大小写优先 |
| sessiontextimportance：Edit Grammar | 已实现（格式管理器入口） | 内置只读，复制用户格式后编辑 XML，验证后原子保存 |
| sessiontextimportance：Orphan lines are always important | 有意未采纳（开关） | 本期 spec 明确允许纯不重要元素孤立行降级；不再提供覆盖它的开关 |
| sessiontextimportance：Compare line endings | 已实现 | “Line endings unimportant”的反向语义；默认忽略，EOF 亦遵守 |
| sessiontextalignment：Never align differences | 已实现 | 使用未打包骨架，不是 Unaligned 的别名 |
| sessiontextalignment：Unaligned | 已实现 | 第 n 行对第 n 行 |
| sessiontextalignment：Standard | 已实现 | 默认；有界相似配对，唯一已知差异不相似替换见 KNOWN_DIFFS |
| sessiontextalignment：Myers O(ND) | 已实现 | 内部 Quick，默认展示打包；不暴露内部名 |
| sessiontextalignment：Patience Diff | 已实现 | UI 显示 Patience |
| sessiontextalignment：Skew tolerance | 已实现 | 会话参数传给 SmartAlignOptions |
| sessiontextalignment：Use closeness matching | 已实现 | 独立 use_closeness 布尔，默认 true；关闭 Standard 时保留孤立行 |
| sessiontextalignment：Align With 提示 | 已实现 | 独立有序锚点，Clear manual alignment |
| sessiontextreplacements：New Replacement Item | 已实现 | from/to、正则/字面、大小写、全词、双向；只影响比较不改文件 |

### sessiontextmerge*.html（通配扫描中的三路条目）

这些页面确实读过，不混算为当前二路实现。下列逐项均**后置到 text-merge-and-ai**：

- `sessiontextmerge.html`：Specs、Format、Importance、Alignment 标签页。
- `sessiontextmergespecs.html`：输入文件、Merge to、Description。
- `sessiontextmergeformats.html`：输入格式、Detected、各输入 Encoding overrides、None。
- `sessiontextmergeimportance.html`：Unimportance list、Grammar elements、Leading/Embedded/Trailing whitespace、Everything else、Character case、Edit Grammar、Orphan lines are always important、Compare line endings。二路对应行为不表示三路输出已有实现。
- `sessiontextmergealignment.html`：Never align differences、Unaligned、Standard、Myers O(ND)、Patience Diff、Skew tolerance、Use closeness matching、手动 Align With；以及 **A merge conflict is → Only different change lines / Left change and right change separated by lines（最大行距）**。冲突范围与三路模型尚未实现。

## viewtext.html — 概览中的行为

| 项 | 状态 | 证据/理由 |
|---|---|---|
| 左右 / 上下布局 | 已实现 | RawInput/tessellation 回归，修复过继承水平布局造成上下窗格横排的问题 |
| 本地硬盘/系统网络路径 | 已实现 | std::path + TextContent::open；权限/I/O 错误可见 |
| 压缩包内文件路径 | 后置 | VFS provider |
| ftp/http/远程服务/网页内容 | 有意未采纳 | 本期只用本地/挂载路径，不自动联网 |
| Open Clipboard | 已实现 | 指定侧加载，不是额外比较窗口 |
| 两侧滚动同步 | 已实现 | 共用 x/y 偏移，预先钳制程序跳转，空隙等高 |
| 重要红、不重要蓝、行浅背景/区间深背景 | 已实现 | DiffColors；混合区块重要性传播 |
| 插入/删除颜色 | 已实现（项目明确差异） | 按本 spec 默认绿色，不照搬参考工具全部重要红色 |
| 规则与格式控制分类 | 已实现 | TextMap/CompiledComparison；手动格式立即重算 |
| 颜色可配置 | 已实现 | 全局 Prefs 由 App 传入，Settings 即时生效 |
| 缩略图按文件位置、可视框、当前行、点击定位 | 已实现 | 512 桶有界缩略绘制，不按百万行每帧重绘；70% 定位测试 |
| 底部 Text/Hex/Alignment Details | 后置 | 独立详情编辑器，不影响当前双窗格功能 |
| Word Wrap | 后置 | 见上表，固定行高与有界水平窗口 |
| 原行尾/无最终换行 | 已实现 | EOF 标记、双侧状态差异提示、编码/逐行保存回归 |

## 格式帮助与正则（D10 补充扫描）

| 主题/选项 | 状态 | 说明 |
|---|---|---|
| formattext：General / Conversion / Grammar / Misc | 已实现（模型/精简入口） | File formats XML 管理器，不照搬原生四页设计 |
| formattextgeneral：Mask / Description | 已实现 | 用户格式 XML 属性；内置只读、复制后自定义 |
| formattextgrammar：语法元素、语法项、New、项目顺序 | 已实现 | 四种 GrammarMatch；最长匹配/等长优先规则按 ADR 004，非机械先匹配 |
| formattextgrammar：Line weights | 已实现 | XML 权重与 Smart 锚点；独立权重表 GUI 后置 |
| grammars：比较重要性与 syntax highlighting、Edit Grammar | 已实现 | 分词缓存+元素表+用户格式 XML 编辑；不承诺嵌套/context-sensitive grammar |
| formattextconversion：plain text 直接比较 | 已实现 | 编码检测、逐行模型 |
| formattextconversion：Conversion method / 结构化或专有格式提取 | 后置 | DOC 等转换器定义可读，但未声称已实现转换执行 |
| formattextconversion：External program / Loading / Saving | 后置 | 外部程序白名单与安全执行契约需另做 |
| formattextconversion：Unicode/ANSI filename encoding、%s/%t/%o | 后置 | 随外部转换器实现；不执行导入 XML 中的程序 |
| formattextconversion：exit code=0 且输出非空 | 后置 | 随转换器执行器验证 |
| formattextconversion：Disable editing（转换结果） | 后置 | 当前普通文本只读已实现，转换管线尚无 |
| formattextconversion：Encoding / code page | 已实现 | 自动检测+手动 UTF-8/UTF-16/936/1252；其他引擎支持页的 GUI 枚举扩展后置 |
| formattextconversion：Ctrl+Z indicates EOF | 有意未采纳 | 不默默截断文件；物理字节/行尾保持 |
| formattextconversion：characters per line limit / 保存去人工断行 | 有意未采纳 | 保留物理行，长行只做绘制窗口和行内 diff 降级；黄金专用格式设置见 ADR 007 |
| formattextconversion：Trim trailing whitespace / Convert leading spaces to tabs on save | 有意未采纳 | 保存不引入无关改动；显式批量转换命令另行后置 |
| formattextmisc：Insert spaces instead of tabs | 已实现 | 编辑器按 format.tab_as_spaces 插入 |
| formattextmisc：Tab stop | 已实现 | 编辑器与显示均使用格式 tab_size；默认 4 格，范围 1–32 |
| formattextmisc：Lines are independent | 后置 | 本期规范连续差异按区块操作，不添加记录模式 |
| formattextmisc：Column-based data | 后置 | 固定列语法分词已有；逐列比较模式属于独立策略扩展 |
| sample_regular_expressions：^abc / abc$ / ^abc$ | 已实现 | 行首/行末锚点 |
| sample_regular_expressions：^\\s*abc / ^\\s*end;?\\s*$ | 已实现 | 空白、重复、可选 |
| sample_regular_expressions：abc\|def / a(b\|c)d | 已实现 | 分支、捕获组 |
| sample_regular_expressions：a(b\|c)d\\1（反向引用匹配） | 有意未采纳 | regex 后端不支持回溯引用，明确报错；替换中的 $1/${name} 捕获展开已支持 |
| sample_regular_expressions：^、$、\\s、*、?、\|、( ) | 已实现 | 对应上列 pattern 回归 |
| sample_regular_expressions：\\1 | 有意未采纳 | 不把不支持的反向引用静默当字面字符串 |

其他 `formathex*` / `formatpix*` / `formattable*` / `formatexternalgeneral` 属于非文本视图；本次解包通配包含这些文件，但不作为文本比较已完成能力。该清单不宣称完成整个产品的命令/选项体系。

## 专项验证边界

- 两侧只有可见行进入 galley；长行每次只排版 4096 字节窗口；原文和可编辑范围未截断。
- 原生窗口实际运行与 framebuffer 截图已验证；RawInput 验证焦点、IME Preedit 不改文本、Commit 一次可撤销、IME 光标输出、复制粘贴/拖放与绘制三角化。
- **Windows 中文输入法候选窗：桌面锁定，环境受限待人工验收。** 协调者明确接受自动回归后勾选 8.4，但要求保留此待验事项；不得把它写成已人工验收。
- 未实现项均在上表明确记录，不用启用的空按钮或成功提示伪装已有功能。
