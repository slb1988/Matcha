# 文件夹同步对照清单

依据 commandsdirsync、sessiondirsync（specs/ops/comparison/handling/namefilter/
otherfilters/misc）、dir_sync_how_to_sync、dir_sync_understanding_the_display。
通用比较/过滤参数参见 [folder-compare](folder-compare.md)，下表不把继承参数遗漏为“已全做”。

## commandsdirsync：逐条命令

| 命令 | 状态 | 本轮行为 / 边界 |
|---|---|---|
| Tools > Options | 后置 | 全局快捷键/菜单定制与偏好编辑后置 session-and-cli；本轮仅本视图规则/列/工具栏。 |
| Save Session | 后置 | 会话持久化不在本 change，后置 session-and-cli。 |
| Save Session As | 后置 | 同上，不把过滤预设保存误称为保存完整会话。 |
| Session Settings | 已实现 | Rules、Filters、Other filters 与 Base folders 对话框；下方选项表逐项说明边界。 |
| Locked | 后置 | 会话级锁定/只读继承后置；已有文件系统只读属性确认。 |
| Clear Session | 后置 | 可 Home 后新建；同类空会话清除交由统一会话管理。 |
| Swap Sides | 已实现 | 交换两个基准路径并重载；原路径进入 Back/Forward 历史。 |
| Folder Sync Info | 已实现 | 预览及执行后的复制/删除/创建/跳过/失败/冲突统计。 |
| Run Sync | 已实现 | Sync Now 执行当前快照，不按新状态偷偷重造计划。 |
| Compare Base Folders | 已实现 | 从同步切回文件夹比较。 |
| Compare Parent Folders | 已实现 | 在新的 folder 子视图比较两侧父目录，原视图不被覆盖。 |
| Leave Alone | 已实现 | 逐项覆盖；文件夹覆盖作用于后代。 |
| Copy Right to Left | 已实现 | 预览覆盖，执行前仍复核源/目标与确认。 |
| Copy Left to Right | 已实现 | 同上，反方向。 |
| Delete Left | 已实现 | 预览覆盖，确认后执行。 |
| Delete Right | 已实现 | 同上，右侧。 |
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
| Exclude | 已实现 | 按选中路径加入排除；同步可直接排除预览项。 |
| Copy Filename | 已实现 | 返回完整路径文本，通过 egui clipboard 请求；物理剪贴板见人工复测。 |
| Refresh Selection | 已实现 | 只刷新受影响的侧限定路径及祖先摘要。 |
| Explorer | 后置 | 操作系统文件管理器右键菜单后置 shell 集成。 |
| Expand All | 已实现 | 必要时扫描，再展开；纯视图展开不清同步覆盖。 |
| Collapse All | 已实现 | 折叠全部；保留数据与覆盖。 |
| Select All | 已实现 | 选择可见项，两侧均参与。 |
| Invert Selection | 已实现 | 在当前可见项中反选。 |
| Refresh | 已实现 | GUI 重枚举打开目录；后台核心另提供 refresh_loaded。 |
| Full Refresh | 已实现 | 整个比较递归刷新，仍在 worker。 |
| Next Conflict | 已实现 | 按计划导航并展开必要祖先，不循环。 |
| Previous Conflict | 已实现 | 反向冲突导航，不循环。 |
| Find Filename | 已实现 | 大小写、全词、regex、wrap、up、side；支持子集见选项表。 |
| Find Next Filename | 已实现 | 继续搜索，遵守 wrap/side。 |
| Find Previous Filename | 已实现 | 反向继续搜索。 |
| Ignore Unimportant Differences | 已实现 | 视图级忽略，不暗中重写比较。 |
| Suppress Filters | 已实现 | 临时显示隐藏项并标 teal；不永久改掩码。 |
| Columns | 已实现 | 左右对称列、点击排序、右键列头/菜单切换；部分 provider 字段无值时明确显示不可用。 |
| Legend | 已实现 | 颜色、空心、黄色与中央内容图标解释。 |
| Log | 已实现 | OpLog 虚拟列表与文本保存。 |
| Toolbar | 已实现 | 本视图切换工具栏。 |

同步视图不提供 Ignore Folder Structure；这也不在原同步菜单中。
Scope/确认、日志、进度和取消共用 Wave B，不存在另一个绕过确认的同步执行器。

## 会话参数与显示图标

| 选项 / 显示 | 状态 | 本轮实现 / 边界 |
|---|---|---|
| Left / Right base folders | 已实现 | 本地路径输入与最近列表 |
| Remote base folders / Browse remote service | 后置 | 远程 VFS 非目标 |
| Description | 后置 | 会话持久化 |
| Presets：Update Left | 已实现 | 右新与右孤儿→左 |
| Update Right | 已实现 | 左新与左孤儿→右 |
| Update Both | 已实现 | 双向更新；基线双侧变化/无方向差异为冲突 |
| Mirror to Left | 已实现 | 右源镜像并删除左孤儿 |
| Mirror to Right | 已实现 | 左源镜像并删除右孤儿 |
| Custom operations | 已实现 | 左新/右新/左孤儿/右孤儿/不同五类映射 |
| Rules | 已实现 | 共用 Comparison/Handling 对话框 |
| Filters | 已实现 | 名称、其他过滤、保护文件默认及提示条 |
| Accept / preview changes | 已实现 | 参数改变后台重建快照，忙时不可运行旧预览 |
| Double-click pair | 已实现 | 文本原型子视图，非正式编辑器 |
| Exclude selected operations | 已实现 | 计划覆盖，不把它混成永久文件删除 |
| Sync Now | 已实现 | 明确按钮，逐项确认与可取消 |
| Sync empty folders | 已实现 | create-empty；必要非空父目录仍会创建并保真 |
| Only affect visible files | 已实现 | display/file filter 作用域，不取 viewport 裁剪区域 |
| Expand subfolders on load | 已实现 | 共享 handling |
| Sync within archives | 后置 | archive VFS 未实施 |
| Delete to recycle bin if possible | 已实现 | 默认启用；不可用时独立询问，取消不等于永久删除许可 |
| Touch local files when copying to FTP | 后置 | 远程 provider 未实施 |
| Follow symbolic links | 已实现 | 默认不跟随；跟随循环报错且不可强制规划破坏性动作 |
| Maintain DOS 8.3 aliases | 后置 | design D10 |
| Copy NTFS permissions | 后置 | 模型保留 / Unsupported |
| Copy creation dates | 已实现 | Windows 可选保留 |
| Automatic refresh | 已实现 | 后台 watcher 驱动 |
| Comparison：size / timestamps / tolerance / DST / timezone | 已实现 | 共享 Criteria，详见比较清单与 golden 采集限制 |
| Filename case / attributes / different extension / NFC | 已实现 | 共享配对/快速准则 |
| CRC / binary / skip quick same / version / override | 已实现 | 共享内容 job，不把跳过当作已读相等 |
| Rules-based | 后置 / 明确例外 | 待文本分支合并 |
| Include/Exclude files/folders | 已实现 | 四组 NameFilter |
| Other filters：date/size/text/attributes/protected | 已实现 | 共用 OtherFilters |
| Alignment overrides | 已实现 | 共享规则与手动对齐 |
| Enabled file format associations | 后置 | 正式规则比较与会话接线 |
| Copy right to left 图标 | 已实现 | 中央 R -> L |
| Copy left to right 图标 | 已实现 | 中央 L -> R |
| Delete left / Delete right 图标 | 已实现 | 中央 Delete L / Delete R |
| Leave alone 图标 | 已实现 | 中央 Leave |
| Folder contains copy / delete / mixed actions | 已实现 | 独立汇总 >、<、xL、xR、!，不由单一文件状态猜测 |
| Conflict navigation | 已实现 | 必要时展开祖先；不环绕；显式覆盖解决导航冲突 |
| Sync statistics | 已实现 | 预览数与实际执行结果分开，失败/取消/刷新错误不丢 |
| Baseline persistence across restart | 后置 | 本视图持有；会话项目负责持久化 |
| Physical desktop interaction | 环境受限 | 已获准以真实 native framebuffer + egui/worker 回归等价验收，负责人仍需人工复测 |

## 脚本与安全边界

支持 sync [visible] [create-empty] update|mirror:left->right|right->left|all；
mirror:all 在 parse/build/序列化校验中报错。自定义方法没有原生脚本拼写时返回错误，
不输出伪命令。

文件夹覆盖先整体验证再应用到后代。任何读取失败或循环目录不得靠覆盖强行成为可复制源。
基线只更新成功且已解决的条目；失败/取消不推进，未解决冲突不会在下次运行被“洗白”。
实际操作仍受 metadata 复核、目标碰撞、只读与永久删除回退确认约束。

本轮没有运行参考工具的写操作同步去改用户数据；同步行为通过系统临时目录副作用测试、
fake Interaction 回放与 UI worker 回归验证。报告不把比较 XML 黄金称作原生同步副作用黄金。
