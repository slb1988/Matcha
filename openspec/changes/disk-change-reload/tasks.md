# Tasks

## 1. 功能文档先行

- [x] 1.1 新建 `docs/features/disk-change-reload.md`（模板五节：功能简介、代码地图、关键语义、使用与验证、深入阅读），写清检测时机、询问 / 自动重载 / 拒绝记忆、保持位置、覆盖视图与 Non-Goals；在 `docs/features/README.md` 索引表加一行（状态「进行中」）；验收：文档各语义与 `specs/disk-change-detection/spec.md` 的需求一一对应

## 2. 偏好与设置持久化

- [x] 2.1 `bc-session` `Prefs` 新增 `disk_changes: DiskChangePrefs { check: true, auto_reload: false }`（默认值见 design 决定 6）；单测验证 `Prefs::default()` 的两个默认值；`cargo test -p bc-session` 通过
- [x] 2.2 `bc-gui/src/settings.rs` 读写 `files.check-changed-on-disk` / `files.auto-reload`，非法值退回默认；在现有 settings 单测中加往返与非法值用例；`cargo test -p bc-gui settings` 通过
- [x] 2.3 `dialogs/options.rs`：Tweaks 页改为已接线，放两个复选框，「检查」关闭时「自动重载」置灰；OK / Apply 写回 `Prefs`；单测覆盖默认显示、置灰联动、Apply 后 `Prefs` 更新；`dev-build.ps1` 编译通过；同步更新 `docs/features/gui-shell.md` 偏好持久化小节列出新键

## 3. 检测与决策核心

- [x] 3.1 新增 `bc-gui/src/disk_watch.rs`：`DiskStamp`、`stamp(path)`、`DiskTracked` trait、`check(view, prefs) -> CheckOutcome`（design 决定 1、2）；用假的 `DiskTracked` 实现做单测覆盖：未变化、变化+非 dirty+自动重载→AutoReloaded、变化+dirty+自动重载→Ask 且带丢弃说明、自动重载关闭→Ask、两侧同时变化→一个 Ask 两项、declined 相同→Nothing、declined 后再变→Ask、文件消失→Gone 且同状态只报一次、检查关闭→Nothing、已有确认框→Nothing；`cargo test -p bc-gui disk_watch` 通过

## 4. 文本比较接入

- [x] 4.0 `bc-engine-text` `TextContent::open`：小于 64 MiB 读入内存不留句柄，达到阈值才 `open_mapped`（design 决定 6）；在 `bc-engine-text` 测试中打开文件后立即覆写、改名替换并删除该文件均成功，内容仍为打开时的；`cargo test -p bc-engine-text` 通过；feature 文档记下 ≥64 MiB 仍会被锁的限制
- [x] 4.1 `TextCompareView` 每侧记录 `loaded` / `declined` 标识：`open_side` 成功与 `save` 成功后更新；剪贴板侧无标识；实现 `DiskTracked`；单测：保存后 `check` 返回 Nothing（spec「自身保存不算外部变化」）
- [x] 4.2 实现 `reload_keep_position(s)`（design 决定 5）：不触发 `land_on_first_diff`，对齐完成后恢复当前行 / 滚动并钳制；dirty 侧确认后直接丢弃编辑重载；`view_tests.rs` 用临时文件测试：停在第 N 行、外部追加内容、重载后仍在第 N 行；外部截短到 < N 行后停在末行
- [x] 4.3 确认框（design 决定 4）：标题、文件列表、dirty 说明、Reload / Keep Current；Keep Current 记 `declined`；用 egui 测试帧（注意先跑 2 帧预热）验证点击 Reload 后右侧内容更新、点击 Keep Current 后再次 `check` 不再弹框；`cargo test -p bc-gui text::` 通过

## 5. 表格与十六进制接入

- [x] 5.1 `TableCompareView` 实现 `DiskTracked`：载入 / 保存后记标识；重载中（worker 运行）不参与检查；重载完成恢复选中格与滚动（钳制）；在 `table/session_tests.rs` 加用例：外部修改 CSV 后 `check` + 确认重载，新值出现在网格且选中格保持；`cargo test -p bc-gui table::` 通过
- [x] 5.2 `HexCompareView` 实现 `DiskTracked`：重载后恢复偏移并钳制到新长度；`hex/view_tests.rs` 加用例：外部截短文件后重载，光标落在新末尾内；`cargo test -p bc-gui hex::` 通过

## 6. 触发点

- [x] 6.1 `app.rs`：跟踪 `viewport().focused` 的 false→true 与活动标签切换，对活动标签调 `check`，按结果自动重载 / 打开确认框 / 写状态栏；`View::FolderCompare` 转发给其浮动文件视图；文本合并、表格合并、文件夹视图本身不参与；在 `app_tests.rs` 用模拟输入（`focused` false→true）验证：活动文本标签被检查、后台标签在切换到它时才检查、合并标签不弹框；`cargo test -p bc-gui app` 通过

## 7. 集成检查与记录

- [x] 7.1 `cargo test -p bc-gui -p bc-session` 与 `cargo clippy -p bc-gui` 无新增失败或告警；`dev-build.ps1` 后用 `target\debug\matcha.exe a.txt b.txt` 实机：在记事本修改 b.txt 保存后切回 → 弹框并能重载；开启自动重载后再改 → 静默重载、状态栏提示；编辑左侧未保存时外改左侧 → 仍弹框且说明会丢弃；在文件夹比较里打开文件对重复一次
- [x] 7.2 `docs/features/disk-change-reload.md` 状态改为「已实现（验证方式）」并同步 README 索引；`CHANGELOGS.md` 顶部加条目；验收：文档与实际行为一致

## Workflow follow-up

- 评审通过后执行 `/opsx:archive disk-change-reload`，新建主规格 `openspec/specs/disk-change-detection/spec.md`。
