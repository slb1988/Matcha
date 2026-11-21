# Spec Delta

## MODIFIED Requirements

### Requirement: 差异导航
系统 SHALL 支持从当前位置跳到上一个/下一个差异区块（连续的非"相同"行构成一个区块），到达首尾时按 `difference-navigation-feedback` 的规则处理：默认提示 "Difference not found." 并保持位置；开启回绕时回到另一端的差异区块。

#### Scenario: 下一差异
- **WHEN** 光标位于第 1 个差异区块且用户执行"下一差异"
- **THEN** 视图滚动到第 2 个差异区块的首行并使其可见

#### Scenario: 已在最后一个差异
- **WHEN** 光标位于最后一个差异区块且用户执行"下一差异"，未开启回绕
- **THEN** 位置不变，状态栏上方弹出 "Difference not found." 消息面板并播放提示音

#### Scenario: 开启回绕后越过最后一个差异
- **WHEN** 已开启 Wrap around to first difference，光标位于最后一个差异区块且用户执行"下一差异"
- **THEN** 视图定位到第 1 个差异区块的首行
