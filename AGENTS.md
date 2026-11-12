# AGENTS.md

Rust workspace：Matcha（Beyond Compare 风格比较工具）。主程序为 `apps/bcompare`，产出可执行文件 `matcha`。

## 分支纪律

- **不要随便创建分支**。默认在当前分支（通常是 `master`）上直接工作。
- 只有在用户明确要求时才创建新分支；创建前先说明理由并征得同意。
- 合并、推送等破坏性/发布性操作同样先确认再执行。

## 文档纪律

- **功能文档在 `docs/features/`**：每个用户可感知的功能一篇（索引与维护约定见 `docs/features/README.md`）。这是 AI/新人了解功能的入口：先读这里，再按链接进 crate README 与 ADR。
- **新功能落地前**：先建/更新对应 feature 文档（功能简介、验收方式），再写代码；文件名 kebab-case，尽量与 openspec change 同名。
- **功能行为变化时**（语义、开关、退出码、UI 交互、文件格式任一）：同一次提交同步更新对应 feature 文档；文档与代码冲突视为缺陷。
- **分工不双写**：feature 文档只写功能全貌+代码地图；crate 内部 API 留在 `crates/*/README.md`，决策原因留在 `docs/decisions/`，参考工具对齐证据留在 `docs/reference-parity/`。

## 编译发布纪律

- **每次修改完成后自动编译 debug 版**（无需用户再要求）：每完成一个可编译的逻辑阶段（而非攒一大堆改动）就执行

  ```powershell
  powershell -ExecutionPolicy Bypass -File tools/dev-build.ps1
  ```

  - 等价于 `cargo build -p bcompare`，产物 `target\debug\matcha.exe`；不打包、不改版本号。
  - 脚本会先自动结束从本仓库 `target\debug\` 运行的 `matcha` 进程（否则 exe 被占用导致链接失败）；release/dist/安装版不受影响。
- 构建失败必须当场修复，不要把编译错误留给下一阶段。
- release 构建与打包只在需要时执行（用户要求发布/打包时）：`cargo build --release -p bcompare`；完整发布包（含 `matcha-comp`/`matcha-compc`、按平台布局组装到 `target/dist/`）用 `cargo xtask dist`。

## 常用命令

- 构建（release）：`cargo build --release`
- 测试：`cargo test`
- 日常编译（debug）：`powershell -ExecutionPolicy Bypass -File tools/dev-build.ps1`
- 发布打包：`cargo xtask dist`（`--debug` 打 debug 包）
- 发布到 GitHub Releases：`powershell -ExecutionPolicy Bypass -File .agents/skills/matcha-publish/publish.ps1`（版本号见 `.agents/skills/matcha-publish/config.json`，发布成功后最后一位自动 +1 并同步 workspace 版本；`-DryRun` 只检查不发布）

## 常用文档入口

- 功能索引：`docs/features/README.md`（文本/文件夹/表格比较、CLI 与脚本、报告、会话与格式、GUI 架构）
- CLI 速查：`.agents/skills/matcha-cli/SKILL.md`
- 项目总览：`README.md`；决策记录：`docs/decisions/`
