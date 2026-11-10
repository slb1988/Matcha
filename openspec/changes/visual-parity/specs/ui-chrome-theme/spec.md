# Spec Delta

## Purpose

定义所有视图共享的外观基础：字体、DPI、主题 token 与菜单栏、工具栏、路径栏、状态栏、图标等组件。

## ADDED Requirements

### Requirement: 系统字体与度量
GUI SHALL 在运行时加载系统 UI 字体与参考默认编辑器等宽字体（字号以 tokens.json 为准），并为 CJK 提供回退；系统字体缺失时 MUST 回退内置字体而不崩溃。

#### Scenario: 字体缺失回退
- **WHEN** 系统字体路径不存在
- **THEN** 应用正常启动并使用内置字体，日志记录回退

### Requirement: 经典菜单栏
每个视图 SHALL 显示与参考相同顶层顺序的菜单栏，下拉项含助记符、右对齐快捷键、分隔线、勾选/单选标记；参考有而本项目未实现的项 MUST 显示为禁用而非省略。

#### Scenario: 文本视图菜单顺序
- **WHEN** 打开文本比较视图
- **THEN** 菜单栏依次为 Session、File、Edit、Search、View、Tools、Help

### Requirement: 大图标工具栏
工具栏 SHALL 使用图标在上、文字在下的大按钮，支持普通、切换（选中为浅蓝底蓝框）、分体下拉（▾）、禁用四种形态，并按组以竖线分隔；条目与顺序以 tokens.json 对应视图清单为准。

#### Scenario: 显示过滤切换态
- **WHEN** 用户点击 Diffs
- **THEN** Diffs 呈选中态，同组 All/Same 取消选中

### Requirement: 原创图标
图标 SHALL 为本项目原创绘制，尺寸、槽位与主色系与参考一致；MUST NOT 提取或逐像素描摹参考工具资源。

#### Scenario: 图标目录
- **WHEN** 运行 `--snapshot=icon-catalog`
- **THEN** 输出包含全部工具栏与小图标的目录图
