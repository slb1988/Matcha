# Spec Delta

## Purpose

定义文件夹比较与文件夹同步视图逐区域外观与参考一致。

## ADDED Requirements

### Requirement: 文件夹视图布局
文件夹比较视图 SHALL 呈现参考同序的菜单栏、工具栏、过滤栏、每侧路径栏、双树（列头 Name/Size/Modified、行高、展开符、文件夹/文件小图标、名称颜色语义）、中缝比较符号与状态栏；区域顺序与内容 MUST 与参考一致，几何与视觉样式以设计系统为准（design D9）。

#### Scenario: 孤儿着色
- **WHEN** 左侧有右侧不存在的文件
- **THEN** 该文件名以参考的孤儿颜色显示，右侧对应行为空

### Requirement: 文件夹同步外观
文件夹同步视图与操作确认对话框 SHALL 按参考的区域结构呈现，保留换肤前全部功能入口；对话框标题与选项 MUST 使用可读文字（如 Copy to Folder、Both sides、Keep relative folder structure），不得显示内部枚举名。

#### Scenario: 同步视图结构
- **WHEN** 渲染 `folder-sync` 快照场景并与参考图并排
- **THEN** 菜单栏、工具栏、路径区、操作区、双树、日志与状态栏的顺序与内容与参考一致

#### Scenario: 操作对话框文字
- **WHEN** 对选中项执行 Copy to Folder
- **THEN** 对话框标题为 Copy to Folder，目标与路径选项以文字呈现
