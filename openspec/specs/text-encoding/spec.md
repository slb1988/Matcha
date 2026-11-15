# text-encoding Specification

## Purpose
定义文本文件在读入与保存时对字符编码、字节序标记（BOM）和行尾的处理，保证"打开即正确显示、保存不引入无关改动"。

## Requirements

### Requirement: 编码自动检测
打开文件时系统 SHALL 按以下顺序判定编码：BOM（UTF-8/UTF-16LE/UTF-16BE）→ 有效 UTF-8 → 用户默认 ANSI 代码页；检测到大量 NUL 字节或不可打印字节时判定为二进制并提示改用 Hex 比较。

#### Scenario: 带 BOM 的 UTF-16
- **WHEN** 文件以 `FF FE` 开头
- **THEN** 以 UTF-16LE 解码并记录"含 BOM"

#### Scenario: 无 BOM 的合法 UTF-8
- **WHEN** 文件无 BOM 且全部字节构成合法 UTF-8 序列
- **THEN** 以 UTF-8 解码

#### Scenario: GBK 文本
- **WHEN** 文件无 BOM、不是合法 UTF-8，用户默认代码页为 936
- **THEN** 以 GBK 解码，状态区显示所用代码页

#### Scenario: 二进制文件
- **WHEN** 文件前 8 KB 内 NUL 字节占比超过阈值
- **THEN** 不以文本打开，提示切换到 Hex 比较

### Requirement: 用户可改写编码
系统 SHALL 允许用户为任一侧手动指定编码并重新解码，不修改文件；解码失败的字节 MUST 以可见占位符显示并在状态区计数。

#### Scenario: 切换代码页
- **WHEN** 用户把右侧编码从 1252 改为 UTF-8
- **THEN** 右侧内容按 UTF-8 重新显示，左侧不受影响

### Requirement: 保存保持编码与 BOM
保存时系统 SHALL 使用该侧当前编码写出，并按策略处理 BOM：默认"保持原样"（原文件有则写、无则不写），用户可改为"总是"或"从不"。

#### Scenario: 原文件无 BOM 的 UTF-8
- **WHEN** 用户编辑后保存一个无 BOM 的 UTF-8 文件
- **THEN** 输出文件仍无 BOM，未编辑的字节与原文件逐字节相同

### Requirement: 逐行行尾保留
系统 SHALL 记录每一行自己的行尾（CRLF / LF / CR / 无），保存时原样写回；用户新插入的行使用文件的主导行尾风格；可一键把整个文件统一为某种风格。

#### Scenario: 混合行尾文件
- **WHEN** 文件前半 CRLF、后半 LF，用户只修改了第 3 行的文字
- **THEN** 保存后除第 3 行文字外所有字节不变，包括两段不同的行尾

#### Scenario: 统一行尾
- **WHEN** 用户选择"转换为 LF"
- **THEN** 所有行尾改为 LF，操作可撤销

### Requirement: 末行无换行
系统 SHALL 区分"末行以换行结束"与"末行无换行"，在视图中以标记显示差异，并在保存时保持原状态。

#### Scenario: 仅末尾换行不同
- **WHEN** 两份文本只在是否以换行结束上不同
- **THEN** "行尾风格不重要"开启（默认）时末行归为"相同"并在状态区提示；关闭时末行为重要差异；其他行相同
