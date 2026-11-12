# ADR 401：外框视觉从「经典 Win32 像素对齐」转向 Matcha 现代设计系统

状态：已实施（2026-11-12）。

## 问题

visual-parity 阶段把 GUI 外框做成了参考工具（BC 5 经典 Win32 外观）的像素级复刻：#F0F0F0 面板、
1 px #A0A0A0 直角边框、#CCE8FF/#99D1FF 蓝色悬停块、21 px 菜单行、渐变标签页等；未专门设计的
弹框（例如"未保存修改"）直接用 egui 默认 `Window`，标题栏、按钮与文案都很粗糙。
用户反馈界面"太丑"，期望 2026 年产品的审美。

## 决定

1. **外框视觉不再追求与参考工具像素一致**。行为语义（菜单清单、快捷键、对齐/比较结果、
   退出码、文件格式）的对齐不受影响，仍以 `docs/reference-parity/` 为准；差异内容色
   （红/蓝差异文字、差异行底色）属于内容语义，保持不变。
2. **设计 token 集中在 `crates/bc-gui/src/chrome/theme.rs`**：冷中性灰表面
   （chrome `#F4F5F7` / 内容白 / 下沉 `#F9FAFB`）、发丝线 `#E1E4E8`、控件描边 `#CBD0D6`、
   三级文字色、品牌强调色"抹茶绿" `#3B7A50`（及 hover/pressed/soft 变体）、危险/警告/信息色、
   圆角 4/6/10/14、对话框与弹出层阴影、UI 字号 13 px、加粗字族 `matcha-strong`。
   旧常量名（`PANEL_BG`、`BORDER`、`HOVER_BLUE`、`CLASSIC_BLUE`）保留为新 token 的别名，
   存量调用点自动跟随；新代码一律引用语义 token，不写颜色字面量。
3. **确认类弹框统一走 `chrome::modal::Modal`**：遮罩 + 居中卡片 + 语气徽标 + 标题/说明 +
   可选条目列表（如 LEFT/RIGHT 未保存文件）+ 底栏按钮。按钮角色：`Primary`（实心强调色，
   Enter 触发）、`Secondary`、`Destructive`（红色幽灵按钮，放在左侧远离默认项）；Esc 触发
   注册的取消项。未保存确认的按钮固定为「Don't Save / Cancel / Save」，标题按触发动作区分
   （closing / quitting / reloading / opening …）。
4. 外框组件（菜单栏、弹出菜单、工具栏、标签条、状态栏、路径框、滚动条、固定布局对话框控件、
   Home 页）改用上述 token：圆角胶囊悬停、选中态强调色浅底、去掉凹凸 3D 边与渐变。
5. 视觉回归用 `--snapshot=<scene>` 截图人工检查；新增 `text-unsaved-modal` 场景。
   原先断言参考像素坐标的测试改为断言相对几何（菜单行高/外框高度变化后的新值）。

## 后果

- `docs/visual-parity/tokens.json` 不再是外框颜色/度量的真源（theme 测试已不读取它）；
  其中的菜单清单（inventories）仍被表格视图测试使用。
- 菜单栏 30 px、工具栏 46 px、弹出菜单行 26 px，比参考略高；内容区相应下移 6 px。
- 快照模式会清空输入（保证截图稳定），且不再被"未保存拦截"阻止自动关闭。
