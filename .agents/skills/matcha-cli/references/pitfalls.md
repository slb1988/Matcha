# Matcha 排障：按症状查原因

## 命令被当作路径

Git Bash / MSYS 会把 `/report=...` 转成 `C:/Program Files/Git/report=...`。该参数随后被当作输入文件，两个文件的比较甚至可能被判为三方合并。
在仓库根目录写 `Tools/matcha/matcha.com -report=...`。需保留 `/` 开关的 Git Bash 命令可设置 `MSYS_NO_PATHCONV=1`；cmd/PowerShell 接受 `/` 和 `-`。

## 报告输出

- **有差异却返回 0**：`/report` 的成功码只说明报告已生成。读取表格报告的 `result:` 看结论；脚本按相同性分支时用 `/qc`。
- **hunk 比 `git diff` 默认输出长**：Matcha 的 patch 使用 6 行上下文（BC 风格），`git diff` 默认 3 行。
- **布局报错 100**：文本报告支持 `patch*`；`side-by-side` / `interleaved` / `statistics` / `xml` 尚未实现。典型报错是 `the side-by-side layout is not supported yet; use layout:patch`。`summary` 总是按表格读取输入，文本文件会被当成单列表格。
- **stdout 为空**：报告需要控制台才能写 stdout，使用 `matcha.com`；写文件用 `/reportfile=`（脚本用 `output-to:<file>`）。

## 表格行与计数

- **单行长达数千字符**：RawData 的单元格可包含整段 JSON。先用 `grep -E '^(result:|===|key:|rows:)' report.txt` 看结构，再用 `grep -A1 '^~ ' report.txt | cut -c1-200` 预览变动行。逐格定位示例见[实测报告](test-rawdata-xlsx-20261112.md)。
- **`same` 数比物理行数少 1~2**：`rows:` 统计不含 `title-row` 指定的标题行；单侧的表尾空行整体忽略，也不改变结论。

## 合并异常

- **P4V 提前结束合并**：`matcha-comp.exe` 转交后立即返回。P4V 的 Merge Application 使用等待保存结果的 `matcha.exe`，见 [P4V 配置](merge.md#p4v-合并工具)。
- **表格合并加 `/force` 返回 100**：文本冲突标记无法写入单元格。用 `/automerge /reviewconflicts` 在冲突时进入界面，或直接打开三方合并界面。
