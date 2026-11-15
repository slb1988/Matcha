# Tasks

## 1. 引擎：三方键对齐与合并模型（bc-engine-table）

- [x] 1.1 实现三方键对齐（评审修订为键令牌连接 + 分桶，见 design D1）：`align::hashes` 复用（孤儿桶、三方重复计数、重复组内按序配对、center 外左右按键互配）；`cargo test -p bc-engine-table` 覆盖同键异位对齐、仅左/右孤儿、双增、重复键场景
- [x] 1.2 实现合并模型：逐（行, 列）五态分类（相同性判定复用 `compare.rs` 与规则）+ 行级结构判定（左增/右增/同增/双增冲突/删除/删改冲突）+ 相邻冲突行聚合为区段（gap=0）；单测覆盖五态全集与结构判定全集
- [x] 1.3 实现默认输出与冲突解决（取 left/right/center 作用于格/行/区段、接受当前内容、删改冲突取删除侧移除行）与 `/iu`+favor 语义；单测对照 spec 场景逐条覆盖
- [x] 1.4 实现 `auto_merge` 纯函数：返回未解决冲突列表与输出模型，有未解决冲突时扣留输出；单测含三份输入相同、全冲突扣留
- [x] 1.5 实现键来源优先级（显式键 > 推断 > 退化整行）并暴露实际键列；单测覆盖显式越界报错、推断失败退化、退化后语义为行增删
- [x] 1.6 更新 `crates/bc-engine-table/README.md` 合并章节（模型与不变量），文档中的示例 API 与代码一致

## 2. 写出：CSV 保真与 xlsx 新写

- [x] 2.1 合并输出构造为以 center 格式为准的新 `TextSource`（整行同源搬运原记录，混合行只替换变化字段）并走 `write_delimited` 保真写回（取 left/right 行按其来源字节形态写出）；round-trip 测试验证未改字段、分隔符、行尾原样
- [x] 2.2 引入 `rust_xlsxwriter`（workspace Cargo.toml）实现 xlsx 新写：先验证公式缓存值写出 API 并按 D3 取舍，再实现逐 sheet 值+公式写出；写出后用 calamine 读回校验值与公式文本
- [x] 2.3 实现多 sheet 合并：按名配对、逐 sheet 独立 MergeState、新增 sheet 取该侧、删除 vs 未变不输出、删改冲突默认保留 center、输出并集按 center 顺序追加新增；workbook 级测试覆盖
- [x] 2.4 写出路径统一暂存 + rename，输出等于某输入路径时正确覆盖；测试覆盖输出=right 与已存在输出文件在扣留时保持原样

## 3. CLI：路由、开关与退出码

- [x] 3.1 `bc-script`：解析 `/tablemerge`、`/keys=<列号,…>`（校验越界/重复），扩展合并参数模型携带格式判定与键，更新用法文本；解析单测覆盖新开关与非法组合
- [x] 3.2 `routing.rs`：Merge3/Merge4/AutoMerge 前做格式分派（xlsx/xlsm→表格合并，文本表格默认文本合并、`/tablemerge` 改走表格，混合格式或不支持格式→100）；改写既有"xlsx 三方报不是文本文件"的路由单测，新增分派矩阵
- [x] 3.3 headless 表格自动合并执行（发起进程内，不转交实例）：退出码 0/101/100、`/force` 报错 100、结果行（含实际键列）与 `/silent`、扣留时不写文件；集成测试覆盖 P4V 形式 `matcha %1 %2 %b %r`
- [x] 3.4 `/reviewconflicts` 有冲突时转 GUI 并携带 headless 已应用状态（favor/键），无冲突时不开窗；`routing`/`main` 层测试对照 text-merge 同名场景
- [x] 3.5 GUI 合并的等待与结果码：`View::TableMerge` 接入既有转交/等待/Outcome 管道，退出码 0/14/101/100；`hand_off` 测试覆盖已有实例转交仍等待
- [x] 3.6 更新 `.agents/skills/matcha-cli/SKILL.md` 与 `docs/features/cli-and-scripting.md`（新开关、路由规则、退出码），文档命令可照抄执行

## 4. GUI：表格合并视图（bc-gui table_merge/）

- [x] 4.1 新建 `table_merge/model.rs`：四网格状态模型、按 sheet 懒建 MergeState、切换不丢解决进度、结果码接口；model 单测覆盖懒建与状态保持
- [x] 4.2 视图渲染：复用 `table/` 网格（虚拟滚动、着色、sheet tabs），上排 left/center/right 只读 + 底部输出网格，空隙空白呈现，冲突色标识；view 测试（RawInput/tessellation 风格）验证布局与着色
- [x] 4.3 冲突导航（下一个/上一个区段、计数）与 Take 交互（格/行/区段取 left/right/center、接受当前、删改取删除侧移除行）+ 输出格编辑视为解决；view 测试覆盖 spec 场景
- [x] 4.4 保存与关闭：暂存 + rename 写出、遗留冲突允许保存（默认内容写出）结果码 14、脏关闭提示、从未保存关闭结果码 101；view 测试覆盖
- [x] 4.5 接入 `app.rs`（`View::TableMerge`、标签页标题/图标/脏状态）与 `snapshot.rs`；app 层测试通过

## 5. 集成、语料与文档

- [x] 5.1 golden 语料：`tools/golden` 增加表格合并样本生成与预期（单元格冲突、删改、双增、多 sheet、xlsx 与 CSV 各若干）；golden 运行通过
- [x] 5.2 性能验证：大表（十万行级）三方合并耗时与内存记录在案，无数量级回归；结果写入 change 下的验证记录或 worker report
- [x] 5.3 文档：新增 `docs/features/table-merge.md`（功能简介、验收方式、代码地图），更新 `docs/features/README.md` 索引与 `table-compare.md`/`text-merge.md` 交叉引用、`CHANGELOGS.md`（含 BREAKING 说明）；链接有效、验收命令可执行
- [x] 5.4 全量门禁：`cargo test` 全绿 + `powershell -ExecutionPolicy Bypass -File tools/dev-build.ps1` 通过

## 验证记录

- 5.2（2026-11-14，release，本机）：`cargo test --release -p bc-engine-table --lib merge_scale -- --ignored --nocapture`，
  100k 行 × 10 列、键自动推断：建立合并 0.27 s，输出 sheet 0.14 s，冲突统计 < 1 ms。无数量级问题。
- 5.1（2026-11-14）：`crates/bc-script/tests/golden/table_merge/` 24 例（16 个 CSV/TSV 字节级期望手写，8 个 xlsx 由
  `case.json` 描述、测试时生成），`cargo test -p bc-script --test table_merge_golden` 全部通过；xlsx 输出另用 Excel COM 打开核对。
- 3.4/3.5/4.x（2026-11-15 核对补齐）：视图与管道已随 `0bdc1e3`/`dde6cec`/`c0a2863` 落地（tasks 当时未勾）。本次补：
  删除输出行（引擎 `SheetMerge::remove_row`、模型/视图 `Ctrl+Delete`，对应 spec「输出编辑」）、横向滚动停在整列边界；
  测试 `table_merge::model::tests::{switching_sheets_keeps_each_sheets_resolution, taking_the_delete_side_or_removing_the_row_drops_it}`、
  `view::tests::ctrl_delete_removes_the_output_row`、`app::tests::table_merge_launch_and_hand_off_report_result_codes`（本实例 14、
  转交 101/0）、`routing` 的 `/reviewconflicts` 带表格/键/favor 断言、`hand_off::table_merges_hand_off_and_wait_like_text_merges`
  （真实 exe：xlsx 转交等待 101/0/14、`/reviewconflicts` 无冲突 0 不转交、有冲突带 `/keys=` 转交、`/silent` 101）；快照场景
  `table-merge-basic`/`table-merge-conflict` 已渲染人工查看。spec 同步：view「冲突导航」改为改动区段导航（11-14 用户决定）、
  「输出编辑」删除行语义写实；engine「写出」改为模板写出（D3 二次修订）；cli 场景改为 P4V 参数顺序。
- 5.4（2026-11-15）：`cargo test --workspace` 873 passed / 0 failed / 9 ignored；`tools/dev-build.ps1` 通过。
- 追加任务（用户要求保留样式）：`xlsx_patch` 模板写出 + `formula_refs` 行引用重定位 + 公式格按重定位后文本比较，见 design D3 二次修订。

## Workflow follow-up

- ~~与 text-merge 的 archive 顺序对齐~~：已解决（2026-11-15）——text-merge 尚未归档，直接在其 `text-merge-cli` delta 里把
  "一律按文本合并处理"改为按格式分派（工作簿 → 表格合并，混合 → 100），并把 xlsx 报"不是文本文件"的场景换成二进制输入；
  两个变更谁先归档都不再需要 MODIFIED delta。
- Archive 后验证归档结果与主 specs 一致性。
