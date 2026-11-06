# 105 — 文件夹原生黄金的证据与例外

## 采集

- 原生版本：5.0.6.30713；程序与生成器 SHA-256 见语料 provenance.json。
- 28 组小型合成目录对，均实际运行 /solo /silent 脚本；无授权/密钥文件访问。
- 每次运行先 CreateNew 独占 %TEMP%/matcha-reference-tool.lock；
  检查没有其他参考进程，超时不删除他人的锁。
- 临时命名会话必须直接放在 BCSessions 根下，不是 TSessionFolder/Items。
  Rules 容器带 Value="TDirRules"；实测 State/DirHandling="dirFlatten"。
- 只备份/写入 BCSessions.xml，finally 恢复原始字节与 mtime 并逐字节验证；
  不读取或修改 BCFileFormats.xml、其他设置或任何授权文件。
- 输出 XML 只替换临时基准路径；时间/状态不伪造。输入行尾按字节保留。

## 一致率

原生 filecomp.status 与候选逐配对行比较：
- 全部 33 文件行中 32 行一致：96.969697%。
- 已支持范围 32 / 32：100%。
- 明确批准的例外 1 行：rules-comments，原生 diff，本分支 Unsupported。
- 原生 foldercomp 没有 status，另外核对 4 个目录结构行，不编造颜色/重要性结果。

协调者批准按该已知规则例外验收 7.2；不得把它算成“整体100%”。
回归仅允许该一个已知行，更多差异失败；以后接通规则比较也需更新例外断言。

## 额外观测

- binary 不同 + 左侧较新：原生报告 ltnewer，已修复内容比较抹掉 newer 方向的问题。
- Flatten 重名：即使存在相同完整相对路径，原生仍按相对路径顺序配对，
  不优先内容相同/相同路径；已入库对应样本。
- 文档 timezone:left+6 在该原生版本返回 106 Unexpected argument；
  不伪造它的期望，不计入有效 oracle 分母。timezone:ignore 正常采集。

## UI 验收边界

当前 Windows 桌面不允许物理输入，未尝试解锁。
经协调者确认，native framebuffer 与真实 egui/worker 交互回归作为本波等价证据；
物理鼠标/键盘另列“环境受限，待负责人复测”，不是黄金状态已知差异。
