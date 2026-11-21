# Spec Delta

## MODIFIED Requirements

### Requirement: 差异导航与定位
视图 SHALL 提供 Next/Previous Difference Bytes（Ctrl+N / Ctrl+P）在差异字节间导航，以及 Go To 对话框（字节地址，十进制或十六进制、可选侧）。导航目标滚入视野并定位光标。越过首尾时按 `difference-navigation-feedback` 的规则处理（消息面板 / 提示音 / 回绕）。

#### Scenario: 差异间往返
- **WHEN** 光标位于首个差异之前，连续执行 Next Difference Bytes，未开启回绕
- **THEN** 依次定位到每个差异字节；越过最后一个后光标不动，弹出 "Difference not found." 消息面板

#### Scenario: 开启回绕
- **WHEN** 已开启 Wrap around to first difference，光标位于最后一个差异字节，执行 Next Difference Bytes
- **THEN** 光标定位到第一个差异字节并滚入视野

#### Scenario: Go To 十六进制地址
- **WHEN** 用户在 Go To 中选择 Hex 并输入 `400`，侧选择 Right
- **THEN** 右侧光标定位到右 0x400，左侧同步到配对位置，目标滚入视野
