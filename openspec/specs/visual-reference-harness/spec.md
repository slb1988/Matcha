# visual-reference-harness Specification

## Purpose
定义外观复刻的真源与回归手段：固定语料、参考客户区截图、Matcha 确定性快照、对照工具与 token 文件。

## Requirements

### Requirement: 参考截图采集
系统 SHALL 提供脚本，在持有参考工具互斥锁的前提下，以固定外框尺寸启动参考工具、执行场景操作、裁取客户区并保存 PNG；脚本 MUST 在结束时恢复它改动过的参考工具设置文件并释放锁，MUST NOT 读取授权/密钥文件或写入参考工具安装目录。

#### Scenario: 采集文本基础场景
- **WHEN** 运行采集脚本的 `text-basic` 场景
- **THEN** 生成客户区 PNG，尺寸与 `tokens.json` 中该场景登记的客户区尺寸一致，且锁文件已删除

### Requirement: Matcha 确定性快照
`matcha` SHALL 支持 `--snapshot=<scene>` 与 `--window-size=WxH`，在后台比较完成且无动画/光标闪烁时截取客户区；同一场景连续两次快照 MUST 字节一致。

#### Scenario: 重复快照
- **WHEN** 对 `text-basic` 连续执行两次快照
- **THEN** 两个 PNG 字节一致

### Requirement: 对照工具与 token 真源
系统 SHALL 以 `docs/visual-parity/tokens.json` 为外观真源（颜色、几何、字体、菜单/工具栏/状态栏清单、场景区域），并提供对照工具输出并排图、差异热图与命名区域偏差表。

#### Scenario: 区域偏差报告
- **WHEN** 对一个场景运行对照工具
- **THEN** 报告列出每个命名区域的参考矩形、实测矩形与偏差，超过 ±2 px 的标红
