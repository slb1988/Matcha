# Tasks

## 1. 功能文档先行

- [x] 1.1 新建 `docs/features/text-word-wrap.md`（按 `docs/features/README.md` 的模板），写清开关与默认值、各窗格独立宽度、对齐行高取最大值、折行规则、行号栏与行尾标记位置、点击与选区、↑/↓ 按逻辑行、横向滚动隐藏、10000 视觉行上限、不持久化、无工具栏按钮；`docs/features/README.md` 索引加一行（状态「进行中」），`text-compare.md` 与 `text-merge.md` 加链接；验收：与 `specs/text-word-wrap/spec.md` 各需求一一对应

## 2. 共享折行模块 `bc-gui/src/wrap.rs`

- [x] 2.1 实现 `Breaker`（design D2）：字宽缓存（ASCII 数组 + 其他字符 HashMap，取自 egui `Fonts::glyph_width`）、制表符按制表位展开、空白后可断、CJK 前后可断、无可断点时硬断、每段至少 1 字符、1 px 余量；单测覆盖 spec「折行规则」三个场景（`alpha beta gamma`、25 字符硬折 10/10/5、15 个汉字折成 10+5）以及空行占 1 段、单字符宽于文本区、含制表符的行；`cargo test -p bc-gui wrap` 通过
- [x] 2.2 实现 `count` 的快路径与 `WRAP_CAP = 10000` 截断（design D3）；单测：短 ASCII 行不进入逐字符扫描（计数器断言）、含 `\t` 的短行走精确路径、CJK 不被快路径误判为 1 段、超长行计数为 10000 并报告剩余字符数
- [x] 2.3 实现 `RowHeights`（design D1）：整数单位、前缀和、`y_of` / `display_at`（二分）/ `height` / `set_units` + 前缀重算，以及细折叠行单位；单测：固定单位时与原 `Projection` 固定行高算术逐值相等（含 thin folds）、变高行的 `display_at(y_of(r)) == r`、行内任意 y 落回该行、越界钳制
- [x] 2.4 实现建表预算与估算（design D3）：≤ 16 MB 同步精确，超出时先估算、按视口精确替换、空闲帧分批补齐；单测用合成的 100 万行内容断言：同步路径结果正确，估算路径下视口行为精确值、补齐若干轮后与全精确结果相等

## 3. 文本比较：关闭状态下的等价接入

- [x] 3.1 让 `Projection` 可选持有 `RowHeights`，并把 `render.rs` 中 `max_row`、`shown_end` 等直接用 `row_height` 的算术统一改走 `projection.y_of` / `display_at`；验收：Word Wrap 未开启时 `cargo test -p bc-gui text::` 全部通过、现有文本视觉快照逐像素不变

## 4. 文本比较：Word Wrap 开关与绘制

- [x] 4.1 View › Word Wrap 勾选项启用并接入 `command()`（标签页内状态，默认关闭）；开启时 `need_hscroll = false`、`scroll.x = 0`、丢弃横向滚轮；`view_tests.rs`：菜单勾选状态切换、两个标签页互不影响、开启后横向滚动条消失且 `scroll.x == 0`
- [x] 4.2 按窗格文本区宽度建表，行高取两侧最大值，缺失侧计 1（design D3/D6）；分段绘制：底色、斜线填充、当前行底线铺满整行，行号、书签徽标、复制箭头只在首段，行尾标记在末段之后，截断提示在第 10000 段末；`view_tests.rs`：spec「各窗格按自身宽度折行 / 左右宽度不同」与「对齐行高取各窗格最大值」的场景（左 3 段、右 2 段；行高 4 段时右侧留白；缺失行铺满）、行号只出现一次、CRLF 标记位置
- [x] 4.3 滚动锚定（design D4）：开关、拖动分隔线、调整窗口、切换上下布局后，视口顶端对齐行不变；`view_tests.rs`：顶端第 500 行时改变 `TEXT_SPLIT_X` 宽度，重绘后 `top_row` 仍为 500
- [x] 4.4 点击、拖选、Shift+单击按段命中，超出本侧段数时放在行尾，跨段选区逐段高亮（design D6）；`view_tests.rs`：spec「折行状态下的点击与选区」的三个场景，复制得到的文本与原文一致
- [x] 4.5 光标与查找命中所在视觉行的可见性（design D5）；编辑后受影响行当帧重新计数（design D7）；`view_tests.rs`：60 段长行中按 → 越过视口底部后视图跟随、Find Next 命中第 45 段时可见、↓ 从长行首段直接到下一对齐行、输入使行变成 2 段后下一行同帧下移且两侧仍对齐
- [x] 4.6 运行 `tools/dev-build.ps1` 编译 debug 版，`cargo clippy -p bc-gui` 无新增告警；更新 `docs/features/text-word-wrap.md` 中的文本比较代码地图

## 5. 文本合并：关闭状态下的等价重构

- [x] 5.1 把 `merge/view.rs` 中的 `scroll.y / row_height`、`rows * row_height`、`max_y` 等固定行高算术收敛到基于 `RowHeights` 的辅助函数（关闭时为固定单位）；验收：`cargo test -p bc-gui merge::` 全部通过、现有合并视图视觉快照逐像素不变（design 风险项，单独提交）

## 6. 文本合并：Word Wrap

- [x] 6.1 View 菜单新增 Word Wrap 勾选项与命令；开启时隐藏共享横向滚动条（`hbar = None`，`max_x = 0`，`scroll.x = 0`），显示不再受 `LINE_LIMIT` 截断，行内差异仍只算前 4096 字节；`merge/view_tests.rs`：菜单存在且默认未勾选、开启后横向滚动条消失
- [x] 6.2 四个窗格各按自身宽度建表，行高取四者最大值；分段绘制、行号首段、来源/冲突底色铺满整行；`hit()` 按段命中，输出窗格编辑后当帧重新计数；`merge/view_tests.rs`：spec 中「合并视图四个窗格」场景（2/3/1/1 → 3 段，下一行同高起始）、点击输入窗格第二段放置光标、在输出窗格输入使行折成 2 段后四窗格同步加高
- [x] 6.3 合并视图的光标可见性与 Find Next 命中可见性（design D5）、宽度变化锚定（design D4）；`merge/view_tests.rs` 覆盖；`dev-build.ps1` 编译通过；更新 `docs/features/text-word-wrap.md` 中的合并视图代码地图

## 7. 性能与超长行

- [ ] 7.1 用 perf probe 测试两侧各约 100 MB / 100 万行（含一部分超宽行）：开启 Word Wrap、改变分隔线宽度、切换上下布局，各自耗时 < 1 秒，缩略条跳到末尾后内容正确；记录数值到 `docs/features/text-word-wrap.md` 的验收小节
- [ ] 7.2 50 MB 单行压缩 JSON：开启后该行为 10000 段并显示截断提示，拖动滚动条与滚轮响应正常，内存峰值相对关闭时的增量记录在功能文档中

## 8. 文档同步与集成检查

- [ ] 8.1 更新 `docs/reference-parity/text-compare.md`（两处 Word Wrap 条目由「后置」改为已实现及差异说明：按逻辑行移动光标、无工具栏按钮）、`docs/reference-parity/text-merge.md`（两处）、`crates/bc-gui/README.md` 后置列表，`docs/features/README.md` 状态改为已实现，`CHANGELOGS.md` 新增条目；验收：`git grep -n "Word Wrap"` 中不再有与实现矛盾的「后置」描述
- [ ] 8.2 `cargo test -p bc-gui` 与 `cargo clippy -p bc-gui` 无新增失败或告警；`dev-build.ps1` 后实机走查：中文长行在左右不同宽度下折行、拖动分隔线顶端行不跳、点击第二段放光标、Visible Whitespace 下行尾标记位置、合并视图四窗格对齐、关闭后恢复横向滚动

## Workflow follow-up

- 评审通过后归档本 change，并确认 `openspec/specs/text-word-wrap/spec.md` 生成且 Purpose 正确。
