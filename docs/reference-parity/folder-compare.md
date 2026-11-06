# 文件夹比较对照清单

## 范围与证据

依据本机帮助 topics：commandsdir、sessiondir（specs/comparison/handling/namefilter/
otherfilters/misc）、dir_how_to_compare、dir_understanding_the_display、
dir_filtering_the_view、dir_reconciling_differences、file_masks、
dlgdiralignwithitem、dlgdirotherfilteritem、dlgdirfind。
Folder Merge / sessiondirmerge* 明确后置，不把三路目录加入本 change。

“已实现”指本分支有可调用的实现与回归，不表示已做每个选项的原生黄金覆盖。
原生黄金为 28 目录对；已支持文件状态 32/32，规则占位 1 行例外；
foldercomp 没有状态属性，只比较其结构。参见 [KNOWN_DIFFS](../../crates/bc-core/tests/golden/dir/KNOWN_DIFFS.md)。

物理桌面输入环境受限；协调者批准 native framebuffer + egui/worker 自动回归等价验收，
人工鼠标/键盘检查仍待负责人复测，不能冒称已点击通过。

## commandsdir：逐条命令

| 命令 | 状态 | 本轮行为 / 边界 |
|---|---|---|
| Tools > Options | 后置 | 全局快捷键/菜单定制与偏好编辑后置 session-and-cli；本轮仅本视图规则/列/工具栏。 |
| Save Session | 后置 | 会话持久化不在本 change，后置 session-and-cli。 |
| Save Session As | 后置 | 同上，不把过滤预设保存误称为保存完整会话。 |
| Session Settings | 已实现 | Rules、Filters、Other filters 与 Base folders 对话框；下方选项表逐项说明边界。 |
| Locked | 后置 | 会话级锁定/只读继承后置；已有文件系统只读属性确认。 |
| Clear Session | 后置 | 可 Home 后新建；同类空会话清除交由统一会话管理。 |
| Swap Sides | 已实现 | 交换两个基准路径并重载；原路径进入 Back/Forward 历史。 |
| Back | 已实现 | 回到前一基准目录对。 |
| Forward | 已实现 | 前进到后一基准目录对。 |
| Browse for Folder | 已实现 | Left Side / Right Side 均可输入并选择最近路径；不用新依赖的原生对话框。 |
| Up One Level | 已实现 | Left Side / Right Side / Both Sides 三种选择。 |
| Folder Compare Report | 后置 | 后置 hex-and-reports；黄金报告由外部参考程序产生，非本应用报告器。 |
| Folder Compare Info | 后置 | 状态栏有文件/目录/可见/选中数；独立统计详情窗口后置。 |
| Merge Base Folders | 后置 | 三路 Folder Merge 明确不在本 change。 |
| Sync Base Folders | 已实现 | 切换到独立 Folder Sync view，默认 Update Right，可更改方法。 |
| Compare Parent Folders | 已实现 | 在新的 folder 子视图比较两侧父目录，原视图不被覆盖。 |
| Open Folder | 已实现 | 展开选中文件夹；需要时在 worker 补扫。 |
| Close Folder | 已实现 | 折叠选中文件夹，不删除扫描缓存。 |
| Open Subfolders | 已实现 | 递归展开选中目录。 |
| Close Subfolders | 已实现 | 递归折叠选中目录。 |
| Set as Base Folder | 已实现 | 选中目录替换其所在侧基准。 |
| Set as Base on Other Side | 已实现 | 选中目录替换对侧基准。 |
| Set as Base Folders | 已实现 | 用两个选中目录重设两侧基准。 |
| Open in New View | 已实现 | 已通过独立 folder 子视图或文件比较浮动视图提供；完整标签管理后置。 |
| Open | 已实现 | 一个配对文件或任选两文件进入文本浮动视图；目录使用 Open folders in new view。 |
| Open With | 后置 | 外部关联/程序注册表后置 session-and-cli；本轮 Open 使用文本原型。 |
| Associated Application | 后置 | 外部程序调用与注册表后置。 |
| Text Edit | 后置 | 等待正式文本控件合并后的编辑入口；当前原型只读。 |
| Text Patch | 后置 | 补丁视图不在本轮；不能把文本原型当补丁编辑器。 |
| Quick Compare | 已实现 | size / CRC / binary 后台结果对话框；rules 模式明确不可用。 |
| Compare To | 已实现 | 先选一项，再点击第二个文件；后台打开独立文本视图。 |
| Align | 已实现 | 通过 Align With / 对齐覆盖对话框完成强制配对，不要求复制原菜单布局。 |
| Align With | 已实现 | 仅允许同一配对父目录；精确覆盖优先，重读不丢失。 |
| Compare Contents | 已实现 | 按选择强制 CRC 或 binary；不暗中改会话准则。 |
| Copy to Right | 已实现 | OpPlan + worker，目标/较新/只读确认分别处理。 |
| Copy to Left | 已实现 | 同上，反方向。 |
| Copy to Side | 已实现 | 先选择目标侧及隐藏项范围，再生成计划。 |
| Copy to Other Side | 已实现 | 按选择侧推导；双侧选择时先询问方向。 |
| Move to Right | 已实现 | 同卷重命名/跨卷复制删除，经确认。 |
| Move to Left | 已实现 | 同上，反方向。 |
| Move to Side | 已实现 | 目标侧与范围对话框。 |
| Move to Other Side | 已实现 | 动态方向，双侧选择先询问。 |
| Exchange | 已实现 | 逐文件暂存/发布/回滚；孤儿移动，不承诺整树事务。 |
| Copy to Folder | 已实现 | 文本路径 + 最近列表；path:none/relative/base。 |
| Move to Folder | 已实现 | 同上，移动语义。 |
| Delete | 已实现 | 回收站/永久模式；Unsupported 回退永远独立确认。 |
| Rename | 已实现 | DOS mask / 正则捕获替换；碰撞拒绝；大小写改名有回归。 |
| Attributes | 已实现 | r/h/a/s 设置与清除。 |
| Touch | 已实现 | 现在/指定时间，或复制对侧时间戳。 |
| Exclude | 已实现 | 按选中路径加入排除；同步可直接排除预览项。 |
| New Folder | 已实现 | 选中目录或当前基准下新建，受侧选择控制。 |
| Copy Filename | 已实现 | 返回完整路径文本，通过 egui clipboard 请求；物理剪贴板见人工复测。 |
| Ignored | 已实现 | 设置/清除忽略差异标记。 |
| Refresh Selection | 已实现 | 只刷新受影响的侧限定路径及祖先摘要。 |
| File Compare Report | 后置 | 后置 hex-and-reports。 |
| Synchronize | 已实现 | 五种方法通过 Sync Base Folders 的预览/Sync Now 执行；不提供静默绕过预览的选区快捷组。 |
| Update Right | 已实现 | 同步方法下拉：左新/左孤儿复制到右。 |
| Update Left | 已实现 | 同步方法下拉：右新/右孤儿复制到左。 |
| Update Both | 已实现 | 双向更新；无方向差异或基线双侧变化为冲突。 |
| Mirror to Right | 已实现 | 左源镜像：复制不同/左孤儿，删除右孤儿。 |
| Mirror to Left | 已实现 | 右源镜像，对称处理。 |
| Source Control | 未采纳 | SCC 集成明确不在范围。 |
| Explorer | 后置 | 操作系统文件管理器右键菜单后置 shell 集成。 |
| Add to Source Control | 未采纳 | SCC 集成明确不在范围。 |
| Remove from Source Control | 未采纳 | SCC 集成明确不在范围。 |
| Get Latest Version | 未采纳 | SCC 集成明确不在范围。 |
| Get | 未采纳 | SCC 集成明确不在范围。 |
| Check In | 未采纳 | SCC 集成明确不在范围。 |
| Check Out | 未采纳 | SCC 集成明确不在范围。 |
| Undo Check Out | 未采纳 | SCC 集成明确不在范围。 |
| Show Differences | 已实现 | DisplayFilter::Mismatches；SCC 的同名命令不采用。 |
| Show History | 未采纳 | SCC 集成明确不在范围。 |
| Show Properties | 未采纳 | SCC 集成明确不在范围。 |
| Run Source Control Application | 未采纳 | SCC 集成明确不在范围。 |
| Expand All | 已实现 | 必要时扫描，再展开；纯视图展开不清同步覆盖。 |
| Collapse All | 已实现 | 折叠全部；保留数据与覆盖。 |
| Select All | 已实现 | 选择可见项，两侧均参与。 |
| Select All Files | 已实现 | 只选择可见文件。 |
| Select Newer | 已实现 | Left Side / Right Side / Both Sides。 |
| Select Orphans | 已实现 | Left Side / Right Side / Both Sides。 |
| Invert Selection | 已实现 | 在当前可见项中反选。 |
| Refresh | 已实现 | GUI 重枚举打开目录；后台核心另提供 refresh_loaded。 |
| Full Refresh | 已实现 | 整个比较递归刷新，仍在 worker。 |
| Next Difference | 已实现 | 向后定位可见差异，不循环。 |
| Previous Difference | 已实现 | 向前定位可见差异，不循环。 |
| Find Filename | 已实现 | 大小写、全词、regex、wrap、up、side；支持子集见选项表。 |
| Find Next Filename | 已实现 | 继续搜索，遵守 wrap/side。 |
| Find Previous Filename | 已实现 | 反向继续搜索。 |
| Show All | 已实现 | DisplayFilter::All。 |
| Show Same | 已实现 | DisplayFilter::Matches。 |
| Show No Orphans | 已实现 | DisplayFilter::NoOrphans。 |
| Show Differences but No Orphans | 已实现 | DisplayFilter::MismatchesNoOrphans。 |
| Show Orphans | 已实现 | DisplayFilter::Orphans。 |
| Show Left Newer | 已实现 | DisplayFilter::LeftNewer。 |
| Show Right Newer | 已实现 | DisplayFilter::RightNewer。 |
| Show Left Newer and Left Orphans | 已实现 | DisplayFilter::LeftNewerOrphans。 |
| Show Right Newer and Right Orphans | 已实现 | DisplayFilter::RightNewerOrphans。 |
| Show Left Orphans | 已实现 | DisplayFilter::LeftOrphans。 |
| Show Right Orphans | 已实现 | DisplayFilter::RightOrphans。 |
| Show None | 已实现 | 默认不列出；View 中启用高级 Show none 后可选。 |
| Always Show Folders | 已实现 | FolderDisplay::AlwaysShow。 |
| Compare Files and Folder Structure | 已实现 | FolderDisplay::FilesAndStructure。 |
| Compare Files Only | 已实现 | 默认 FilesOnly；未扫描/错误目录保留提示。 |
| Ignore Folder Structure | 已实现 | Flatten 虚拟配对；内容与文件操作均使用真实两侧身份。 |
| Ignore Unimportant Differences | 已实现 | 视图级忽略，不暗中重写比较。 |
| Suppress Filters | 已实现 | 临时显示隐藏项并标 teal；不永久改掩码。 |
| Columns | 已实现 | 左右对称列、点击排序、右键列头/菜单切换；部分 provider 字段无值时明确显示不可用。 |
| Legend | 已实现 | 颜色、空心、黄色与中央内容图标解释。 |
| Log | 已实现 | OpLog 虚拟列表与文本保存。 |
| Toolbar | 已实现 | 本视图切换工具栏。 |

SCC 子菜单的 **Show Differences** 同样“未采纳”；不要与显示过滤的同名命令混淆。
Left Side / Right Side / Both Sides 子选项在对应命令行中列出；帮助中的 >、/、* 为排版/标注，
不是漏掉的独立命令。

## sessiondir* / 显示与过滤选项

| 选项 | 状态 | 实现 / 原因 |
|---|---|---|
| Specs：左右基准目录 | 已实现 | 文本框与最近列表；路径验证在 worker |
| Specs：Disable editing、对子视图继承 | 后置 | 会话级保护/继承统一放到 session-and-cli；不可与 DOS readonly 属性混同 |
| Specs：Description | 后置 | 会话描述与持久化模型未接线 |
| Comparison：Compare file size | 已实现 | Criteria.compare_size；no-size 为内部扩展拼写 |
| Compare timestamps / second tolerance | 已实现 | 默认 2 秒；黄金内/边界/外三例 |
| Ignore daylight saving difference | 已实现 | 恰好 ±1h，开启/关闭均有黄金 |
| Ignore timezone differences | 已实现 | 整小时倍数；timezone:ignore 黄金 |
| timezone:left/right ±hours | 已实现（API） | 解析/往返；参考程序拒绝文档 left+6 示例，采集限制不伪造期望 |
| Compare filename case | 已实现 | filename-case 内部字段；与 case-sensitive pairing 独立 |
| Archive / System / Hidden / Read-only 属性比较 | 已实现 | attrib:a/s/h/r；归档默认不比较 |
| Align different extensions | 已实现 | 配对键选项 |
| Align Unicode normalization forms | 已实现 | Windows NFC；非 Windows 明确 Unsupported |
| Compare contents：CRC | 已实现 | 流式，可取消；列数据可按需缓存 |
| Compare contents：Binary | 已实现 | 分块早停；不同内容保留快测较新方向 |
| Rules-based comparison | 后置 / 明确例外 | 本分支 Unsupported，合并文本线后接通 |
| Skip if quick tests same | 已实现 | 自动比较可零读跳过；不显示虚假的内容相等图标 |
| Compare versions | 已实现 | PE 资源，size 模式也能调度版本测试 |
| Override quick test results | 已实现 | 内容相同可覆盖快测；内容不同不抹掉 newer 方向 |
| Automatically scan subfolders | 已实现 | worker 扫描，顶层先发布 |
| Automatically scan top-level orphans | 已实现 | 默认关闭；Flatten / Sync 或显式展开会补扫所需内容 |
| Expand subfolders on load | 已实现 | handling 选项 |
| Only expand subfolders with differences | 已实现 | 状态选择后展开 |
| Archive handling：As files | 已实现 | 当前 provider 将压缩文件视作普通文件 |
| Archive：As folders once opened / always | 后置 | 非目标 VFS provider |
| Touch local files when copying to FTP | 后置 | 远程 VFS 不在范围 |
| Follow symbolic links / junctions | 已实现 | 默认不跟随；显式跟随后有祖先循环保护与错误角标 |
| Maintain DOS 8.3 aliases | 后置 | design D10 指定 |
| Copy NTFS file permissions | 后置 | 模型保留，规划明确 Unsupported，UI 不假装可用 |
| Copy creation dates | 已实现 | Windows 可选创建时间，目录元数据后序应用 |
| Automatic refresh | 已实现 | watcher→worker 局部刷新，overflow 全扫 |
| Name Filters：Include/Exclude files/folders 四组 | 已实现 | 掩码编辑框同一模型、排除优先 |
| Clear name filters | 已实现 | 清空编辑框并 Apply |
| Add To Presets / preset name / mask | 已实现 | XML 预设模型与本应用设置目录保存 |
| Other Filters 提示条 | 已实现 | Other filters 有配置时提示 |
| Exclude protected OS files | 已实现 | 默认系统+隐藏排除，可脚本字段切换 |
| 单侧被过滤时 teal | 已实现 | 同行两侧可见，隐藏侧 teal |
| Older than / Newer than | 已实现 | 绝对时间或相对本地比较日零点 |
| Day(s) ago | 已实现 | FilterClock，不把 UTC 零点伪装成本地零点 |
| Smaller / Larger：B/KB/MB/GB | 已实现 | 大小解析与单位回归 |
| Text containing / not containing | 已实现 | UTF-8 流式文本，不进行有损解码掩盖错误 |
| DOS attribute is set / not set | 已实现 | attrib include/exclude 字段 |
| Other Filter：source control status | 未采纳 | SCC 明确排除 |
| Alignment Override：名称、DOS mask、regex、folder limit | 已实现 | Rules 对话框与 Align With；侧限定刷新防止别名遮蔽 |
| Enabled file formats / per-session format override | 后置 | 规则比较/正式文件视图接线随文本合并与会话项目 |
| Find：case / whole word / regex / wrap / up / side | 已实现 | 复用 Pattern；unsupported regex 报错，不静默错配 |
| PCRE 回溯引用 / look-around | 未采纳 | 沿用核心线性 regex 子集，不新增 PCRE 依赖 |
| Insert regex element / RE Examples 辅助按钮 | 后置 | 有表达式输入与错误提示，未复制帮助编辑器 |
| 名称后的 DOS r/h/a/s | 已实现 | 实际属性后缀 |
| Gray / Black / Purple / Red / Teal / Yellow | 已实现 | FolderColors；前四色取帮助色块，通用自绘图形 |
| Hollow、混合红紫、链接箭头、错误标记 | 已实现 | 只按节点数据绘制，不复制参考图标 |
| Owner / group / VCS 列数据 | 后置 | 列可选择但 local provider 不提供，显示不可用而非伪造 |
| 日期显示格式 | 已实现 | 明确标 UTC；本地化格式偏好后置 |
| Ctrl 点击 / 中央列双侧 / Shift 范围及限定侧 | 已实现 | 可见行选择，Shift 左右保留整侧选区 |
| 鼠标 swipe 拖选跨中央列 | 后置 | 当前用 Ctrl/Shift 点击范围；不声称所有原生手势完全一致 |
| 比较结果另存为报告 / 打印 | 后置 | hex-and-reports |

## 当前接口入口

- UI：bc-gui/dir_view.rs、dialogs/*、filters_bar、legend、log_panel。
- 树/过滤/内容：bc-engine-dir 的 tree/view/criteria/content/filter。
- 操作：OpPlan → FileOpJob / OpOutcome → 局部刷新；副作用一律后台。
- 新建子视图保留原基准；完整标签页与配置继承不在本轮，不改文本原型。
