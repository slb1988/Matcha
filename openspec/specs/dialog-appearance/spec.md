# dialog-appearance Specification

## Purpose
定义对话框、弹出菜单与右键菜单的结构与参考一致，视觉样式统一走设计系统（design D9）。

## Requirements

### Requirement: 对话框布局
会话设置（文本 / 文件夹与同步 / 表格）、Options、File Formats、Go To 对话框 SHALL 使用参考的页签划分与控件分组，并统一为同一对话框框架：标题、下划线页签、页面卡片与底部按钮行（OK / Cancel，需要即时应用的加 Apply）。会话设置 SHALL 编辑草稿，OK / Apply 才生效，Cancel、× 与 Esc 丢弃。文件夹会话设置的页签 SHALL 为 Specs、Comparison、Handling、Name Filters、Other Filters、Misc（参考帮助）。参考有而未实现的控件 MUST 显示为禁用。参考的 Find/Replace 为停靠查找条而非对话框，按查找条实现。

#### Scenario: 文本会话设置
- **WHEN** 打开文本比较的 Session Settings
- **THEN** 页签依次为 Specs、Format、Importance、Alignment，底部为 OK 与 Cancel

#### Scenario: 文件夹会话设置
- **WHEN** 在文件夹比较中打开 Session Settings，修改名称过滤后按 Cancel
- **THEN** 视图的过滤不变；再次修改并按 OK 后过滤生效

### Requirement: 右键与弹出菜单
文本窗格、文件夹树、表格网格的右键菜单 SHALL 与参考条目、顺序、分隔线一致（清单见 `docs/visual-parity/supplemental/*-context.tree.json`）；已实现的条目复用菜单栏同名命令与启用规则，未实现项为禁用。右键点在选区之外时 SHALL 先选中所点项。弹出菜单的边框、阴影与高亮以设计系统为准。

#### Scenario: 文本窗格右键
- **WHEN** 在文本窗格右键
- **THEN** 菜单条目与顺序与参考一致，未实现项为禁用

#### Scenario: 表格网格右键
- **WHEN** 在右侧网格的某个格右键并选择 Modify...
- **THEN** 该格被选中并进入编辑，Copy to 条目指向 Left
