# Spec Delta

## Purpose

定义文件夹同步会话：五种标准同步方法与自定义方法、同步前预览与逐项动作覆盖、执行与日志，以及与 `sync` 脚本命令的对应关系（FR-D4）。

## ADDED Requirements

### Requirement: 同步方法
系统 SHALL 提供 Update Left、Update Right、Update Both、Mirror to Left、Mirror to Right 五种预设方法及自定义方法。Update 复制较新与孤儿到目标侧；Update Both 双向；Mirror 使目标与源一致：复制不同项与源侧孤儿，删除目标侧孤儿。

#### Scenario: Update Right
- **WHEN** 左侧有较新文件 A 与孤儿 B，右侧有孤儿 C
- **THEN** 预览为：A 复制到右，B 复制到右，C 保持不变

#### Scenario: Mirror to Right
- **WHEN** 同上
- **THEN** 预览为：A 复制到右，B 复制到右，C 删除

#### Scenario: 自定义方法
- **WHEN** 用户为"左侧较新/右侧较新/左孤儿/右孤儿/不同"各自指定动作
- **THEN** 预览按该映射生成

### Requirement: 预览与动作覆盖
同步执行前 SHALL 显示每个受影响条目的计划动作（右→左复制、左→右复制、删除左、删除右、保持不变），文件夹行汇总其内容的动作；用户可对任意条目覆盖为 Leave Alone / Copy Right to Left / Copy Left to Right / Delete Left / Delete Right，或 Exclude 跳过；"仅影响可见文件"与"创建空文件夹"为会话选项。

#### Scenario: 覆盖单项
- **WHEN** 用户把一个计划删除的右侧文件改为 Leave Alone
- **THEN** 执行时该文件不被删除，其他计划不变

#### Scenario: 文件夹汇总
- **WHEN** 某文件夹内同时有待复制与待删除的文件
- **THEN** 该文件夹行显示"含复制与删除"的汇总图标

### Requirement: 冲突
Update Both 下两侧都较新（或两侧都不同且无法判定方向）的条目 SHALL 标为冲突并默认 Leave Alone；提供上一/下一冲突导航；同步统计显示冲突数。

#### Scenario: 双向冲突
- **WHEN** Update Both 且同一文件两侧都在上次同步后修改
- **THEN** 该项为冲突、动作为 Leave Alone，Next Conflict 可定位到它

### Requirement: 执行与日志
Run Sync（Sync Now）SHALL 按预览执行，复用 folder-operations 的确认、进度、取消与日志语义；执行后重新比较并显示剩余差异；同步统计（Folder Sync Info）给出复制/删除/跳过/失败计数。

#### Scenario: 执行后状态
- **WHEN** Update Right 执行成功
- **THEN** 原计划的条目全部变为相同，日志逐项记录，统计显示复制数

### Requirement: 与脚本一致
内部同步 API SHALL 直接对应脚本 `sync [visible] [create-empty] (update|mirror):(left->right|right->left|all)`，`mirror:all` 为非法并报错。

#### Scenario: 脚本镜像
- **WHEN** 内部 API 收到 `sync create-empty mirror:left->right`
- **THEN** 生成与 GUI Mirror to Right 相同的计划，并创建空文件夹

#### Scenario: 非法方向
- **WHEN** 收到 `mirror:all`
- **THEN** 返回错误，不执行任何操作
