# Spec Delta

## Purpose

规定各比较与合并视图在「下一个/上一个差异」类导航找不到目标时的统一反馈（消息面板与提示音），以及 Options › Next Difference 中控制该反馈和回绕的偏好。

## ADDED Requirements

### Requirement: 适用的导航命令
本能力 SHALL 适用于以下命令（下称「差异导航」）：文本比较的 Next/Previous Difference 与 Next/Previous Difference Section；表格比较的 Next/Previous Difference Cell、Difference Row、Edit；十六进制比较的 Next/Previous Difference Bytes；文本合并与表格合并的 Next/Previous Change Section；文件夹比较（含文件夹同步）的 Next/Previous Difference 与冲突导航。First/Last Difference 不属于差异导航。

#### Scenario: 表格的编辑导航同样适用
- **WHEN** 表格比较中光标已在最后一处编辑上，用户执行 Next Edit
- **THEN** 按本能力的"找不到"规则反馈

#### Scenario: First Difference 不回绕
- **WHEN** 文本比较开启了回绕，用户执行 Last Difference 后再执行 First Difference
- **THEN** 光标按原语义分别落到最后一个和第一个差异，不涉及回绕

### Requirement: 找不到差异时弹出消息面板
差异导航找不到目标且「Show message panel」开启时，视图 SHALL 保持光标与选区不变，并在状态栏正上方显示消息面板：浅黄底、信息图标、文字 **Difference not found.**。所有视图 MUST 使用同一文案，状态栏 MUST NOT 再显示"已无更多差异"类旧文字。面板出现时，视图内容区 SHALL 上移让出面板高度，不得遮挡状态栏。

#### Scenario: 越过最后一个差异
- **WHEN** 十六进制比较中光标位于最后一个差异字节，用户按 Ctrl+N
- **THEN** 光标不动，状态栏上方出现 "Difference not found." 面板

#### Scenario: 越过第一个改动区段
- **WHEN** 文本合并中光标位于第一个改动区段，用户执行 Previous Change Section
- **THEN** 光标不动，出现 "Difference not found." 面板

#### Scenario: 两侧完全相同
- **WHEN** 表格比较的两个文件完全相同，用户执行 Next Difference Row
- **THEN** 出现 "Difference not found." 面板

### Requirement: 提示音
消息面板每次因差异导航而出现时，系统 SHALL 播放一次系统默认提示音；面板已显示时再次找不到，SHALL 重新弹出并再响一次。「Show message panel」关闭时 MUST NOT 播放提示音。

#### Scenario: 连续两次找不到
- **WHEN** 用户在最后一个差异上连按两次 Next Difference Section
- **THEN** 面板保持显示，提示音响两次

### Requirement: 面板关闭时机
消息面板 SHALL 在用户的下一次按键、文字输入或鼠标按下时关闭；鼠标移动、滚轮与窗口重绘 MUST NOT 关闭面板。如果这次操作本身又触发了"找不到"，面板 SHALL 重新出现。

#### Scenario: 按方向键关闭
- **WHEN** 面板显示中，用户按下方向键
- **THEN** 面板消失，内容区恢复原高度

#### Scenario: 移动鼠标不关闭
- **WHEN** 面板显示中，用户只移动鼠标或滚动滚轮
- **THEN** 面板仍然显示

### Requirement: 关闭消息面板偏好
Options › Next Difference SHALL 提供 **Show message panel** 复选框，默认勾选。取消勾选后，差异导航找不到目标时 SHALL 既不显示面板也不播放提示音，光标保持不变。

#### Scenario: 关闭后静默
- **WHEN** 用户取消 Show message panel 并确定，然后在最后一个差异上执行 Next Difference
- **THEN** 光标不动，没有面板，也没有提示音

### Requirement: 回绕偏好
Options › Next Difference SHALL 提供 **Wrap around to first difference** 复选框，默认不勾选。勾选后，向下的差异导航找不到目标时 SHALL 定位到该命令意义下的第一个目标，向上找不到时 SHALL 定位到最后一个目标，此时 MUST NOT 显示面板或播放提示音。没有任何目标时仍按"找不到"处理。

#### Scenario: 向下回绕
- **WHEN** 已勾选回绕，文本比较中光标在最后一个差异区段，用户按 Ctrl+N
- **THEN** 光标定位到第一个差异区段并滚入视野，没有面板

#### Scenario: 向上回绕
- **WHEN** 已勾选回绕，文件夹比较中焦点在第一个差异条目，用户执行 Previous Difference
- **THEN** 焦点移到最后一个差异条目

#### Scenario: 没有目标时回绕无效
- **WHEN** 已勾选回绕，十六进制比较的两个文件完全相同，用户按 Ctrl+N
- **THEN** 出现 "Difference not found." 面板

### Requirement: 偏好持久化
两个偏好 SHALL 与其他 Options 设置一起保存到用户设置文件，下次启动沿用；OK/Apply 生效，Cancel 不改变已保存值。快照/基准模式 MUST NOT 读写这两个设置。

#### Scenario: 重启后保留
- **WHEN** 用户勾选 Wrap around to first difference 并点 OK，然后重启 Matcha
- **THEN** Options › Next Difference 中该项仍为勾选，导航按回绕执行

#### Scenario: Cancel 不保存
- **WHEN** 用户取消 Show message panel 后点 Cancel
- **THEN** 找不到差异时仍显示面板
