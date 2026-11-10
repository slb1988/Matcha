# Spec Delta

## Purpose

定义对话框、弹出菜单与右键菜单外观与参考一致。

## ADDED Requirements

### Requirement: 对话框布局
会话设置（文本/文件夹/表格）、Options、Find/Replace、File Formats、Go To 对话框 SHALL 使用参考的窗口尺寸、页签/左侧树、控件布局与 OK/Cancel/Help 按钮行；参考有而未实现的控件 MUST 显示为禁用。

#### Scenario: 文本会话设置
- **WHEN** 打开文本比较的 Session Settings
- **THEN** 对话框尺寸与页签顺序与参考一致

### Requirement: 右键与弹出菜单
文本窗格、文件夹树、表格网格的右键菜单 SHALL 与参考条目、顺序、分隔线一致；弹出菜单边框、阴影、高亮色以 tokens.json 为准。

#### Scenario: 文本窗格右键
- **WHEN** 在文本窗格右键
- **THEN** 菜单条目与顺序与参考一致，未实现项为禁用
