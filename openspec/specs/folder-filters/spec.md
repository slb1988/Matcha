# folder-filters Specification

## Purpose
定义文件夹会话的三类过滤：按名称/属性决定哪些条目参与比较的文件过滤，按比较状态决定显示哪些行的显示过滤，以及决定文件夹何时出现的文件夹显示过滤；掩码语法与参考工具的 File Masks 帮助一致。

## Requirements

### Requirement: 掩码语法
名称掩码 SHALL 支持通配符 `?`（单字符）、`*`（任意多字符）、`[az]`、`[a-z]`、`[!az]`、`[[]`；掩码以 `.` 结尾时 `?`/`*` 不匹配句点（用于筛选无扩展名文件）；`*.*` 也匹配无扩展名文件；多个掩码以分号分隔；前导 `-` 为排除；尾部 `\`（或 `/`）表示文件夹掩码。

#### Scenario: 字符集
- **WHEN** 掩码为 `[a-c]*.txt`
- **THEN** `apple.txt` 匹配，`dog.txt` 不匹配

#### Scenario: 无扩展名
- **WHEN** 掩码为 `*.`
- **THEN** `Makefile` 匹配，`main.c` 不匹配

#### Scenario: 文件夹掩码
- **WHEN** 掩码为 `-Backup\`
- **THEN** 任意位置名为 `Backup` 的文件夹及其全部内容被排除

### Requirement: 相对掩码
掩码 SHALL 支持相对路径限定：`p\f`（f 必须在名为 p 的文件夹下）、`.\f`（f 必须在基准文件夹下）、`...\f`（f 前可有零个或多个父文件夹）。

#### Scenario: 基准下限定
- **WHEN** 掩码为 `.\Windows\*.txt`
- **THEN** 只有基准文件夹直接子目录 `Windows` 内的 `.txt` 受影响，更深层的 `Windows` 文件夹不受影响

#### Scenario: 任意深度
- **WHEN** 掩码为 `...\Windows\*.txt`
- **THEN** 任意深度名为 `Windows` 的文件夹内的 `.txt` 都受影响

### Requirement: 包含与排除的组合
名称过滤 SHALL 分为包含文件、排除文件、包含文件夹、排除文件夹四组；排除优先于包含；只有排除项时隐式包含全部；包含文件的父文件夹与基准文件夹始终包含；工具栏 Filters 编辑框接受分号分隔的简写（`-` 前缀排除）并支持预设下拉；可把当前过滤加入预设并命名。

#### Scenario: 只排除
- **WHEN** Filters 编辑框为 `-*.dcu;-*.ddp`
- **THEN** 除这两类文件外全部参与比较

#### Scenario: 包含文件夹
- **WHEN** 掩码为 `Source\`
- **THEN** 只有名为 `Source` 的文件夹及其子树参与比较

### Requirement: 其他过滤
系统 SHALL 支持按修改日期（早于/晚于，绝对时间或"N 天前"）、大小（小于/大于，B/KB/MB/GB）、文本内容（包含/不包含）、DOS 属性（已设置/未设置）排除文件；"排除受保护的操作系统文件"（系统+隐藏）默认开启；存在其他过滤项时视图底部显示提示条。若过滤判定一侧应隐藏而对侧可见，两侧都显示且应隐藏的一侧为 teal。

#### Scenario: 七天前
- **WHEN** 其他过滤为"早于 7 天前"
- **THEN** 修改时间早于今天 0 点往前 7 天的文件被排除

#### Scenario: 单侧被过滤
- **WHEN** 左侧 `big.bin` 被大小过滤排除而右侧同名文件未被排除
- **THEN** 两侧都显示，左侧以 teal 着色

### Requirement: 显示过滤
视图 SHALL 提供 13 种显示过滤：Show All、Show Differences、Show Same、Show No Orphans、Show Differences but No Orphans、Show Orphans、Show Left Newer、Show Right Newer、Show Left Newer and Left Orphans、Show Right Newer and Right Orphans、Show Left Orphans、Show Right Orphans、Show None（默认隐藏于菜单）；另有 Ignore Unimportant Differences 开关。显示过滤 MUST 不影响比较本身，只影响可见行与"所有可见项"类命令的作用范围。

#### Scenario: 仅左侧较新与左孤儿
- **WHEN** 选择 Show Left Newer and Left Orphans
- **THEN** 只显示左侧较新的行与左侧孤儿行，右侧孤儿与相同行隐藏

### Requirement: 文件夹显示过滤
系统 SHALL 提供：Always Show Folders（除被文件过滤排除者）、Compare Files and Folder Structure（文件夹遵循显示过滤）、Compare Files Only（隐藏空文件夹与内容全被隐藏的文件夹，默认）、Ignore Folder Structure（扁平化，按文件名对齐不分子目录）。

#### Scenario: 扁平化
- **WHEN** 选择 Ignore Folder Structure
- **THEN** 树被隐藏，左右所有文件按文件名配对显示

### Requirement: 临时取消过滤
Suppress Filters SHALL 临时关闭文件过滤与显示过滤，被隐藏的条目以 teal 显示并可被操作；再次切换恢复。

#### Scenario: 取消过滤后操作
- **WHEN** 用户开启 Suppress Filters 并复制一个原本被排除的 `.bak`
- **THEN** 复制成功，关闭后该文件重新隐藏

### Requirement: 过滤与操作范围
对所选文件夹执行复制/移动/删除等操作时 SHALL 默认跳过被过滤掉的内容；可在偏好中改为默认包含隐藏项；操作确认对话框允许逐次覆盖。

#### Scenario: 复制被过滤的文件夹
- **WHEN** 用户复制含被排除 `.obj` 文件的文件夹且未改默认
- **THEN** `.obj` 文件不被复制，确认框提供"包含隐藏项"的覆盖选项
