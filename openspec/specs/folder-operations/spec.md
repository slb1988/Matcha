# folder-operations Specification

## Purpose
定义在文件夹比较视图中对选区执行的文件操作及其确认、日志、后台执行与刷新行为（FR-D3），与参考工具 Actions 菜单和 `copy/copyto/move/moveto/delete/rename/attrib/touch` 脚本命令一致。

## Requirements

### Requirement: 操作命令集
系统 SHALL 对当前选区提供：Copy to Left/Right、Copy to Side（提示选侧）、Copy to Other Side（动态）、Move to Left/Right/Side/Other Side、Exchange（两侧互换）、Copy to Folder、Move to Folder、Delete、Rename（支持多文件掩码重命名与正则重命名）、Attributes（DOS 属性增删）、Touch（复制对侧时间戳或设为指定时间/现在）、Exclude（把所选文件或其类型加入排除）、New Folder、Copy Filename（复制路径列表到剪贴板）、Ignored（抑制所选条目的差异）。

#### Scenario: 复制到右侧
- **WHEN** 选中左侧 3 个文件执行 Copy to Right
- **THEN** 三个文件被复制到右侧对应相对路径，缺失的父目录被创建，操作后三行状态重新评估

#### Scenario: 正则重命名
- **WHEN** 对 `img_01.png`…`img_09.png` 执行正则重命名 `img_(\d+)` → `photo_$1`
- **THEN** 文件被重命名为 `photo_01.png`…，视图中的行随之更新

#### Scenario: 标记忽略
- **WHEN** 用户对一行执行 Ignored
- **THEN** 该行按"相同"着色并不再计入差异，直到取消标记

### Requirement: 确认与冲突处理
覆盖、删除与移动 SHALL 弹出确认，提供 Yes / No / Yes to All / No to All；覆盖较新文件时明确提示；`/silent` 或脚本的 `option confirm:` 决定默认答案；删除默认走回收站（可关），网络/不支持的位置自动退回永久删除并在日志注明。

#### Scenario: 覆盖较新文件
- **WHEN** 复制会覆盖右侧一个比源更新的文件
- **THEN** 确认框指出目标较新，用户可选 Yes to All 跳过后续提示

#### Scenario: 回收站不可用
- **WHEN** 删除网络共享上的文件且回收站开启
- **THEN** 执行永久删除并在日志记录"回收站不可用"

### Requirement: 复制保真
复制 SHALL 按会话设置保留时间戳与属性，可选保留创建时间与 NTFS 权限；复制文件夹时遵守当前过滤（见 folder-filters），可逐次覆盖；目标为只读文件时按确认结果处理。

#### Scenario: 保留时间戳
- **WHEN** 复制一个文件到右侧
- **THEN** 目标文件的修改时间与源相同，复制后该行状态为相同

### Requirement: 后台执行、进度与取消
文件操作 SHALL 作为后台任务执行，界面显示进度（当前文件、已完成/总数、字节），可取消；取消后已完成的部分保留，未开始的不执行，日志记录中断点。

#### Scenario: 取消大复制
- **WHEN** 复制 5 000 个文件进行到一半时用户取消
- **THEN** 已复制的文件保留，剩余不再复制，日志记录已完成数量

### Requirement: 操作日志
每次操作 SHALL 在 Log 面板追加逐项记录（操作、源、目标、结果、错误原因），并可保存为文本；脚本 `log` 命令复用同一日志流。

#### Scenario: 失败记录
- **WHEN** 删除一个被占用的文件失败
- **THEN** 日志记录该文件与错误信息，其他项继续处理

### Requirement: 操作后刷新
操作完成后系统 SHALL 只刷新受影响的条目及其祖先文件夹的汇总状态，不重新扫描整棵树；Refresh / Full Refresh 可手动触发局部/全量刷新；可选"自动刷新"周期性重新比较。

#### Scenario: 局部刷新
- **WHEN** 在 10 万文件的树中复制 1 个文件
- **THEN** 只有该行与其祖先更新，其他行不重新读取
