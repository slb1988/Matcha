# Spec Delta

## Purpose

在文本比较与文本合并视图中提供 0–9 编号书签，让用户在长文件的多个位置之间快速往返，并规定书签在编辑、重载、换文件时的位置保持规则。

## ADDED Requirements

### Requirement: 放置与移除书签
文本比较与文本合并视图 SHALL 提供 Toggle Bookmark 0–9（快捷键 Shift+Ctrl+0…9，亦在 Search 菜单的子菜单中）。执行 Toggle Bookmark N 时：若当前行已是书签 N 则移除；否则把书签 N 放到当前行（若 N 原在别处则从原处移走）。一行最多一个书签：在已有书签 M 的行上放置 N 时 SHALL 用 N 取代 M。书签挂在活动窗格的当前行上。

#### Scenario: 放置书签
- **GIVEN** 光标在左侧第 120 行
- **WHEN** 用户按 Shift+Ctrl+3
- **THEN** 左侧第 120 行显示书签 3 的标记

#### Scenario: 再次切换即移除
- **GIVEN** 左侧第 120 行是书签 3
- **WHEN** 光标在该行时用户再按 Shift+Ctrl+3
- **THEN** 书签 3 被移除，该行不再有标记

#### Scenario: 移动已有编号
- **GIVEN** 书签 3 在第 120 行
- **WHEN** 光标在第 400 行时用户按 Shift+Ctrl+3
- **THEN** 书签 3 出现在第 400 行，第 120 行不再有标记

#### Scenario: 同一行换编号
- **GIVEN** 第 120 行是书签 3
- **WHEN** 光标在该行时用户按 Shift+Ctrl+5
- **THEN** 第 120 行变为书签 5，书签 3 不再存在

### Requirement: 跳转到书签
视图 SHALL 提供 Go To Bookmark 0–9（快捷键 Ctrl+0…9，亦在 Search 菜单的子菜单中）。执行 Go To Bookmark N 时，若书签 N 存在，SHALL 把书签所在窗格设为活动窗格、光标移到该行行首，并把各窗格滚动到其对齐位置使该行可见；若该行当前被显示过滤隐藏，SHALL 先使其可见。书签 N 不存在时 SHALL 在状态栏提示且不移动光标。

#### Scenario: 跳转
- **GIVEN** 书签 3 在右侧第 400 行，光标在左侧第 10 行
- **WHEN** 用户按 Ctrl+3
- **THEN** 活动窗格变为右侧，光标在第 400 行行首且该行在视口内

#### Scenario: 未设置的书签
- **WHEN** 用户按 Ctrl+7，而书签 7 未设置
- **THEN** 光标不动，状态栏提示书签 7 未设置

#### Scenario: Ctrl+0 可用
- **GIVEN** 书签 0 已设置
- **WHEN** 用户按 Ctrl+0
- **THEN** 跳到书签 0，界面缩放不变

### Requirement: 清除书签
视图 SHALL 提供 Clear Bookmarks，移除本视图的全部书签；没有书签时该命令 MUST 禁用。

#### Scenario: 清除全部
- **GIVEN** 已设置书签 1、3、5
- **WHEN** 用户执行 Search › Clear Bookmarks
- **THEN** 三个标记都消失，Go To Bookmark 1 提示未设置

### Requirement: 书签显示
书签行 SHALL 在其所属窗格中显示带编号的标记，标记 MUST 不遮挡行号栏的复制箭头及其点击区域，且在横向滚动时始终可见。文本比较的缩略导航条 SHALL 在书签对应位置显示刻度。菜单的 Go To Bookmark 子菜单中，未设置的编号 SHALL 显示为禁用。

#### Scenario: 差异行上的书签
- **GIVEN** 某差异区块首行显示复制箭头
- **WHEN** 用户在该行放置书签 2
- **THEN** 行上同时显示书签 2 标记与复制箭头，点击箭头仍执行复制

#### Scenario: 菜单反映已设置的书签
- **GIVEN** 只设置了书签 4
- **WHEN** 用户打开 Search › Go To Bookmark
- **THEN** 只有 Bookmark 4 可用，其余编号禁用

### Requirement: 编辑时书签跟随行
文本比较中，在书签所在侧编辑时，书签 SHALL 跟随其所在行：在其上方插入或删除行时随之上下平移；书签所在行本身被删除时 SHALL 落到删除位置处的行（越过末行则落在末行）。在另一侧编辑 MUST NOT 改变该书签所在的行号。撤销与重做按同样规则平移。

#### Scenario: 上方插入行
- **GIVEN** 书签 1 在左侧第 50 行
- **WHEN** 用户在左侧第 10 行处插入 3 行
- **THEN** 书签 1 位于左侧第 53 行，仍是原来那一行文字

#### Scenario: 书签行被删除
- **GIVEN** 书签 1 在左侧第 50 行
- **WHEN** 用户删除左侧第 48–52 行
- **THEN** 书签 1 位于左侧第 48 行

#### Scenario: 另一侧编辑
- **GIVEN** 书签 1 在左侧第 50 行
- **WHEN** 用户在右侧第 10 行插入 3 行
- **THEN** 书签 1 仍在左侧第 50 行

### Requirement: 重载、换文件与交换两侧
文本比较中，重载某侧 SHALL 保留该侧书签的行号（超出新行数的收敛到末行）；在某侧打开另一个文件或剪贴板内容时 SHALL 清除该侧的全部书签；Swap Sides SHALL 让书签随其内容换到另一侧。书签只在视图打开期间有效，MUST NOT 写入命名会话；关闭并重开会话后没有书签。

#### Scenario: 换文件
- **GIVEN** 左右两侧各有一个书签
- **WHEN** 用户在右侧 Open File 打开另一个文件
- **THEN** 右侧书签被清除，左侧书签保留

#### Scenario: 交换两侧
- **GIVEN** 书签 2 在左侧第 30 行
- **WHEN** 用户执行 Swap Sides
- **THEN** 书签 2 位于右侧第 30 行

#### Scenario: 重开会话
- **WHEN** 用户设置书签后保存命名会话、关闭并重新打开该会话
- **THEN** 视图中没有书签

### Requirement: 合并视图中的书签
文本合并视图中，书签 SHALL 挂在对齐行上并记住放置时的活动窗格；跳转时 SHALL 激活该窗格并把四个窗格滚动到该对齐行。输出窗格的编辑与 Take 操作 MUST NOT 改变书签所在的对齐行。

#### Scenario: 合并视图跳转
- **GIVEN** 用户在 base 窗格某冲突行放置书签 1，随后滚动到文件末尾
- **WHEN** 用户按 Ctrl+1
- **THEN** base 窗格成为活动窗格，四个窗格滚动到该冲突行

#### Scenario: Take 后书签不动
- **GIVEN** 书签 1 在某冲突区段的行上
- **WHEN** 用户对该区段执行 Take Right，使输出行数变化
- **THEN** 书签 1 仍在同一对齐行，跳转后定位到该区段
