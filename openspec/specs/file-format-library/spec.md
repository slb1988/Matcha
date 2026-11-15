# file-format-library Specification

## Purpose
定义文件格式库的行为：内置格式如何按文件名解析、格式如何决定语法着色与重要性、用户如何导入和自定义格式，以及与参考工具格式文件的兼容范围。

## Requirements

### Requirement: 内置格式库
系统 SHALL 内置参考工具的全部 25 种格式定义（19 文本、2 表格、1 图片、1 媒体、1 版本、1 XML），包括每种格式的掩码、语法项、大小写显著性、Tab 设置、对齐权重与转换类型；内置格式对用户只读，但可复制为自定义格式。

#### Scenario: 列出内置格式
- **WHEN** 用户打开格式管理器
- **THEN** 25 种内置格式按名称列出，并标注为内置、不可删除

### Requirement: 按掩码解析格式
打开文件时系统 SHALL 按格式列表顺序以文件名匹配掩码（分号分隔，`*`/`?` 通配，不区分大小写），第一个匹配的同类别格式生效；无匹配时使用"纯文本"（无语法项，全部字符重要）。

#### Scenario: C++ 源文件
- **WHEN** 打开 `main.cpp`
- **THEN** 使用 CPP 格式，状态区显示格式名 `CPP`

#### Scenario: 无匹配
- **WHEN** 打开 `notes.abc`
- **THEN** 以纯文本格式打开

#### Scenario: 用户格式优先
- **WHEN** 用户自定义格式的掩码也包含 `*.cpp` 且排在 CPP 之前
- **THEN** 使用用户格式

### Requirement: 会话级格式覆盖
系统 SHALL 允许用户在比较会话中为当前文件手动选择任意文本格式，覆盖掩码解析结果，并在重新比较后保持。

#### Scenario: 手动切换
- **WHEN** 用户把 `.txt` 文件的格式改为 PYTHON
- **THEN** 立即按 Python 语法着色与分级，重新比较后仍为 PYTHON

### Requirement: 语法项四种匹配类型
格式的语法项 SHALL 支持四种匹配类型：Basic（字面或正则单个记号）、Delimited（起止分隔，可跨行，可转义字符，可到行尾）、List（词表，可正则）、Columns（固定列区间）；每项绑定 10 类元素之一并可设置大小写敏感。

#### Scenario: 跨行注释
- **WHEN** CPP 格式的 `/* … */` 注释跨越 5 行
- **THEN** 这 5 行中注释覆盖的字符全部归为 Comment 元素

#### Scenario: COBOL 列号
- **WHEN** COBOL 格式声明第 73–80 列为 LineNum
- **THEN** 每行该列区间内的字符归为 LineNum，无论内容

#### Scenario: 最长匹配，等长先出现者优先
- **WHEN** 同一位置有多个语法项命中（例如 HTML 的关键字 `<` 与注释起始 `<!--`，或 PYTHON 的 `'` 与 `'''`）
- **THEN** 取命中文本最长的语法项；长度相同时按格式中语法项的先后顺序取靠前者

#### Scenario: 关键字包含在更长标识符中
- **WHEN** 文本为 `while_loop` 且格式同时有关键字 `while` 与标识符正则
- **THEN** 整个 `while_loop` 归为 Identifier，而不是 Keyword 加 Identifier

### Requirement: 导入参考工具格式文件
系统 SHALL 能导入参考工具导出的格式 XML（`BCFileFormats` 根元素），把其中的格式作为用户格式加入；未知的元素被忽略，未知的语法项类型 MUST 报错并指出所在格式而不是静默丢弃。

#### Scenario: 成功导入
- **WHEN** 用户导入包含 3 个自定义文本格式的 XML
- **THEN** 3 个格式出现在列表中，掩码、语法项与大小写设置与源文件一致

#### Scenario: 不支持的语法项
- **WHEN** 导入文件中某格式含有未知语法项类型
- **THEN** 导入中止并提示格式名与类型名

### Requirement: 自定义格式的持久化
用户格式 SHALL 保存在应用自己的设置目录，重启后仍存在；系统 SHALL 提供导出为参考工具可读 XML 的功能（写兼容）。

#### Scenario: 重启保留
- **WHEN** 用户新建格式并重启应用
- **THEN** 该格式仍在列表中且设置不变

### Requirement: 格式影响对齐权重
格式声明的权重规则（正则 + 优先级）SHALL 作为 Smart 对齐的锚点：命中高优先级权重的行优先被配对。

#### Scenario: DFM object 行
- **WHEN** DFM 格式声明 `^\s*object` 为高权重
- **THEN** 两侧的 `object` 行在对齐中优先两两配对
