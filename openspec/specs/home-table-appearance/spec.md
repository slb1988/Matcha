# home-table-appearance Specification

## Purpose
定义 Home 会话启动页与表格比较视图逐区域外观与参考一致。

## Requirements

### Requirement: Home 会话启动页
Home SHALL 呈现参考布局：左侧会话树（分组与最近会话）、右侧新会话类型图标网格及其文字、菜单与工具栏；仅显示本项目已实现的会话类型为可用，其余按参考位置显示为禁用。

#### Scenario: 新会话网格
- **WHEN** 打开 Home
- **THEN** 新会话类型按参考顺序与网格排列，未实现类型为禁用态

### Requirement: 表格比较外观
表格比较视图 SHALL 按参考呈现菜单、工具栏、路径栏、信息头、网格（表头、行号、行状态色、差异单元格色、键列标记）、sheet 标签（多 sheet 工作簿时位于网格与状态栏之间，带比较状态点）、行详情与状态栏；区域顺序与内容 MUST 与参考一致，几何与视觉样式以设计系统为准（design D9）；本项目超越功能的入口放在不破坏参考布局的位置（菜单或分体下拉）。

#### Scenario: 表格基础场景结构
- **WHEN** 渲染 `table-basic` 快照场景并与参考图并排
- **THEN** 区域顺序与内容与参考一致，`--bench-table` 帧时间不超过 16 ms 预算

#### Scenario: 多 sheet 标签
- **WHEN** 比较两个各含三个 sheet 的工作簿
- **THEN** 网格与状态栏之间出现三个 sheet 标签，当前 sheet 高亮，点击切换
