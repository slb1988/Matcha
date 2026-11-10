# Spec Delta

## Purpose

定义文件夹比较与文件夹同步视图逐区域外观与参考一致。

## ADDED Requirements

### Requirement: 文件夹视图布局
文件夹比较视图 SHALL 呈现参考同序的菜单栏、工具栏、过滤栏、每侧路径栏、双树（列头 Name/Size/Modified、行高、展开符、文件夹/文件小图标、名称颜色语义）、中缝比较符号与状态栏；命名区域偏差 MUST ≤ 2 px。

#### Scenario: 孤儿着色
- **WHEN** 左侧有右侧不存在的文件
- **THEN** 该文件名以参考的孤儿颜色显示，右侧对应行为空

### Requirement: 文件夹同步外观
文件夹同步视图与操作确认对话框 SHALL 按参考布局与颜色呈现，保留换肤前全部功能入口。

#### Scenario: 同步视图几何
- **WHEN** 对 `folder-sync` 场景运行对照工具
- **THEN** 所有命名区域偏差 ≤ 2 px
