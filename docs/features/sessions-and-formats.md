# 会话 / 工作区与文件格式库（Sessions & Formats）

> 会话模型与参考工具 XML 生态的兼容层。状态：**已实现**（数据模型、13 种会话、
> `BCSessions.xml` 读写与会话库管理、内置 25 格式库与 BCFF XML 读写；
> 参考工具写出的会话文件可逐字节往返）（2026-11-18 更新）。对应 FR-X1、spec §5。

## 功能简介

**会话（bc-session）**

- `SessionKind`：13 种会话类型，含 XML 类名、`/fv=` 命令行拼写与按类型的默认规则。
- `SessionSpecs`（左右/中/输出 + 标题 + VCS 路径 + 只读标记）、`SessionRules`
  （按族的规则枚举）、`Session`（保留未知 XML 节点以便写回）、`Stamp`（`LastModified`，无时区本地时间）。
- `BCSessions.xml` 读写（`xml.rs`）：根下直接放会话与 `TSessionFolder`，文件夹条目在 `<Items>` 里按原顺序；
  无 `Value` 的会话是按类型的默认会话；未知类（如 `TExternalSession`）与未知元素原位保留。
  `<Rules>` 只建模文本对齐四键（`AlignKind`/`NeverAlignDiffs`/`SkewTolerance`/`UseCloseness`），
  未改动时原样写回、改动时只改这四键；`<State>` 存为有序键值（已知 `ShowContext`、`DirHandling`、
  `SortSide`、`DisplayFilter`）。
- 会话库（`SessionManager`）：`%APPDATA%\Matcha\BCSessions.xml`（与 `settings.txt`、`formats\` 同目录），
  缺文件 = 空树；`upsert` 按文件夹路径插入/替换且全树名称唯一，`remove` 删除，`save` 原子替换写盘。
- 全局偏好由 GUI 写 `settings.txt`（见 [gui-shell.md](gui-shell.md)「偏好持久化」）；不导入参考工具的 `BCPreferences.xml`。

**文件格式库（bc-formats）**

- 内置格式库：build.rs 把 `data/bcfileformats.xml`（25 个顶层条目）转成内嵌 JSON。
- 模型：`FileFormat`、`FormatKind`（6 种）、`GrammarItem`+`GrammarMatch`（Basic/
  Delimited/List/Columns；正则 = `Basic{is_regexp:true}`）、`ElementKind`（10 种）、
  `ConvertKind`、`WeightItem`、`TableOptions`。
- BCFF 读写：保留源 XML 节点、只补丁变更过的类型化属性；注册表以 `(kind, name)`
  为键（`XML` 一名两用）。
- GUI `format_panel.rs`：内置格式只读列表、复制/新建用户格式、事务化 XML 编辑、导入导出。

## 代码地图

| 层 | 位置 | 内容 |
|---|---|---|
| 会话 | `crates/bc-session/src` | `session.rs`、`folder.rs`、`workspace.rs`、`prefs.rs`、`state.rs`、`xml.rs` |
| 格式 | `crates/bc-formats` | `model`、`bcff`（parse/write，与 build.rs 共享）、内置库数据 |
| GUI | `crates/bc-gui/src/text/format_panel.rs`、`text/session_dialog.rs` | 格式管理与会话对话框 |

## 关键语义

- **兼容优先**：会话/格式 XML 必须能被参考工具读取、也能读回其写出的文件；
  未知节点保留不丢。会话文件除首行注释（写 `Produced by Matcha`）与换行符外逐字节往返。
- 规则只持久化文本对齐四键；其余会话设置（重要性、替换、表格/文件夹规则等）的参考键名未知，暂不写盘。
- STX 写规则是已知未决问题：见 ADR 001 与 bc-formats README 的 Open questions。
- 名称不唯一：格式注册键是 `(kind, name)`。

## 使用与验证

```powershell
cargo test -p bc-formats      # 内置库测试（断言 22 个有名格式 + 总数 25）
cargo test -p bc-session      # 夹具 tests/fixtures/BCSessions.xml（结构取自参考工具真实文件，路径匿名化）
# 本机真实样本（git 忽略）逐字节往返：
$env:MATCHA_SESSION_SAMPLES = "tools/golden/backup"; cargo test -p bc-session --test sessions_xml
```

## 深入阅读

- `crates/bc-session/README.md`、`crates/bc-formats/README.md`
- ADR：`docs/decisions/001-stx-write-rule.md`、`002-lines-grammar.md`、`004-tokenizer-precedence.md`
- README"Discrepancies"表（内置格式 22 vs 25、命令 28 vs 29 等）
