# AGENTS.md

Rust workspace：Matcha（Beyond Compare 风格比较工具）。主程序为 `apps/bcompare`，产出可执行文件 `matcha`。

## 分支纪律

- **不要随便创建分支**。默认在当前分支（通常是 `master`）上直接工作。
- 只有在用户明确要求时才创建新分支；创建前先说明理由并征得同意。
- 合并、推送等破坏性/发布性操作同样先确认再执行。

## 编译发布纪律

- **修改代码要阶段性进行编译发布**：每完成一个可编译的逻辑阶段（而非攒一大堆改动），就执行一次 release 构建验证。
- 最低要求：自动生成 `target\release\deps\matcha.exe`：

  ```powershell
  cargo build --release -p bcompare
  ```

- 构建失败必须当场修复，不要把编译错误留给下一阶段。
- 需要完整发布包（含 `matcha-comp`/`matcha-compc`、按平台布局组装到 `target/dist/`）时使用：

  ```powershell
  cargo xtask dist
  ```

## 常用命令

- 构建（release）：`cargo build --release`
- 测试：`cargo test`
- 发布打包：`cargo xtask dist`（`--debug` 打 debug 包）
