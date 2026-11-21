# 表格合并（Table Merge）

> 三方合并 xlsx/xlsm 工作簿与 CSV/TSV 表格：left / right 相对共同祖先 center 的改动按**键对齐的行、逐单元格**合并到输出，
> 可无窗口自动合并，也可在界面里解决冲突。
> 状态：**已实现（引擎、命令行、界面）**（2026-11-15 更新）。BC 没有表格合并，语义以本文与
> `openspec/changes/table-merge/design.md` 为准，`docs/reference-parity/` 无对照。

## 功能简介

- 合并调用（`matcha <base> <theirs> <yours> [<output>]`、`/center=`、`/mergeoutput=`，与文本合并同一套词汇）
  的三个输入都是 xlsx/xlsm 工作簿时自动走表格合并；CSV/TSV 默认仍按行做文本合并，加 `/tablemerge` 才按表格合并。
- 行按**键列**对齐（`/keys=` 指定，否则自动推断，推断不出退化为整行作键），列按位置对齐。
- 逐单元格判定未变 / 左改 / 右改 / 同改 / 异改；行级判定左增 / 右增 / 同增 / 双增冲突 / 删除 / 删改冲突。
  两侧改同一行的不同列不冲突，两侧把同一格改成不同值才冲突。
- `/automerge` 无窗口执行：无冲突写出返回 0，有冲突不写返回 101；`/reviewconflicts` 让它在有冲突时转入界面。
- 界面：上排从左到右 BASE / LEFT / RIGHT 三个只读网格，底部可编辑 OUTPUT 网格（标题条不显示路径），共用滚动；`Ctrl+N` / `Ctrl+P`
  逐个走改动区段（含冲突），落点是区段首行的第一个改动格（列随格水平滚动到可见），当前 sheet 走完继续到下一个有改动的 sheet；
  全部走完时状态栏上方弹出 **Difference not found.** 面板并响一声（取代 "No more changes below/above"），遵循 Tools › Options › Next Difference
  （Wrap around 回到第一个/最后一个有改动的 sheet 的首/末改动区段；Show message panel 关闭时静默，规则见 [text-compare.md](text-compare.md)「找不到差异时的消息面板」），
  打开停在第一个冲突（没有冲突则第一个改动）；Take Left/Center/Right、
  接受当前内容、编辑输出格、删除输出行（`Ctrl+Delete`）、撤销/重做、保存（默认写 LEFT 的文件，即本地真实路径；有输出文件 `%r` 时同步复制一份）；多 sheet 工作簿有 sheet 标签，各 sheet 状态独立。
  横向滚动**逐窗格独立**：滚轮只滚指针下的窗格，超宽的窗格底部自带横向滚动条（以列为单位，自动滚动与滚到底都停在整列边界，左侧不留半个格）；
  纵向四窗格共用保持行对齐；方向键 / 改动导航 / 打开落点按各窗格宽度横向自动滚动，保证选中格处处可见。
- 选中格详情条（输出网格之下、状态条之上）显示活动窗格选中格的完整内容：换行、内部滚动、可选中复制；
  悬停任意格显示完整内容 tooltip（长 JSON 等不再被 110 px 列宽截断）。
- 输入网格标题条的路径是只读地址框（`chrome::pathbar::show_readonly`）：可选中、Ctrl+C 复制，
  右键菜单 Copy / Select All，选区内右击保留选区；路径框持焦时方向键/Delete/F2/Enter 留给文本编辑，
  不作用于网格（Ctrl 快捷键照常）。
- 写出：CSV/TSV 逐记录保真（未改字段的拼写、引号、分隔符、行尾、编码/BOM 原样）；xlsx **以 center 工作簿为模板**写出，
  样式（字体/填充/边框/数字格式）、列宽、行高、合并单元格、冻结窗格、条件格式、数据验证、图表、图片、批注、定义名称、
  宏工程原样保留，只重写单元格数据；行增删后公式、合并单元格、定义名称、表格、图表系列的行引用随行重定位（与 Excel 插删行一致）。

## 代码地图

| 层 | 位置 | 内容 |
|---|---|---|
| 引擎 | `crates/bc-engine-table/src/merge.rs` | `SheetMerge`（键连接、单元格/行分类、冲突区段、take/accept/edit、`output_sheet`）、`TableMerge`（多 sheet 配对）、`auto_merge` |
| 写出 | `crates/bc-engine-table/src/xlsx_patch.rs` | `patch_workbook`：以 center 包为模板重写 `<sheetData>`、并入他侧样式、重定位行引用、增删 sheet；`cell_format`/`sheet_layout` 诊断 |
| 行引用 | `crates/bc-engine-table/src/formula_refs.rs` | `RowMap`、`shift_formula`（A1 引用随行移动，删除行 → `#REF!`，范围收缩）、`shift_sqref` |
| 后备写出 | `crates/bc-engine-table/src/xlsx_write.rs` | 模板写出失败时的新写工作簿（`rust_xlsxwriter`，不保样式）、`vba_project` |
| 宿主 | `crates/bc-script/src/table_merge.rs` | `merge_kind`（格式分派，含 ZIP 文件头识别）、`load`（读三方、文本表格表头由 center 决定）、`write_output`、`auto_merge_tables`、`options` |
| 命令行 | `crates/bc-script/src/cli.rs` | `Switch::{TableMerge, Keys}`、`MergeSwitches.table`、`CliArgs::merge_keys` |
| 应用 | `apps/bcompare/src/routing.rs`、`main.rs` | `merge_kind`、`Route::AutoMerge { table }`、`/force` 拒绝、`merge_launch` 带 `table`/`keys` |
| GUI | `crates/bc-gui/src/table_merge/` | 表格合并视图与模型；`app.rs` 的 `View::TableMerge`；`MergeLaunch.table` 决定开哪种合并视图 |
| 引擎测试 | `crates/bc-engine-table/src/merge_tests.rs` | 对齐、五态、结构判定、区段、解决、favor、键推断/退化、多 sheet、CSV 保真、xlsx 往返 |
| 宿主测试 | `crates/bc-script/src/table_merge_tests.rs` | CSV/xlsx 自动合并、扣留、键越界、分派矩阵、VCS 临时文件名 |
| 路由测试 | `apps/bcompare/src/routing.rs`（`table_merges_route_by_format`） | 按格式分派、`/force`、混合格式 |
| GUI 测试 | `crates/bc-gui/src/table_merge/` 下的 `*_tests.rs`；`app_tests.rs`（`table_merge_launch_and_hand_off_report_result_codes`） | 模型与视图命令、多 sheet 状态独立、删除输出行；标签页类型/标题、关页提示、本实例与转交两条路径的结果码 101/14/0 |
| 端到端 | `apps/bcompare/tests/table_merge.rs`、`hand_off.rs`（`table_merges_hand_off_and_wait_like_text_merges`） | 真实 exe：无窗口 0/101/100；xlsx 合并转交后等待并透传 0/14/101；`/reviewconflicts` 无冲突不开窗、有冲突带 `/keys=`/favor 转交 |
| 快照 | `snapshot.rs` 场景 `table-merge-basic`、`table-merge-conflict`（语料 `tools/visual/corpus/table-merge-*.csv`） | `matcha --snapshot=table-merge-conflict --snapshot-out=x.png` |
| 写出测试 | `crates/bc-engine-table/src/xlsx_patch_tests.rs`、`formula_refs.rs` | 样式/列宽/冻结/合并单元格/图表/定义名称保留与行重定位，sheet 增删 |
| 语料 | `crates/bc-script/tests/golden/table_merge/`（24 例）+ `tests/table_merge_golden.rs` | CSV/TSV 字节级期望；xlsx 用 `case.json` 描述三方工作簿（测试时生成，不入库二进制），校验值、公式、格式与版式 |

## 关键语义

- **分派**：三个输入都是工作簿 → 表格合并；都不是 → 文本合并（`/tablemerge` 时表格合并）；混合 → 错误 100。
  工作簿 = 扩展名 xlsx/xlsm，或扩展名不认识但文件头是 ZIP（P4V 等工具的临时文件名）。`/tablemerge` 遇到 xls/ods → 100。
- **键**：`/keys=1,3`（1 起，按顺序为优先级）> 自动推断（center 与各侧分别推断，取三方都唯一的那组列）
  > 容忍全空键的单列推断（非空键三方各自唯一、且与 center 重合过半时取该列；多列满足取重合度高、并列取靠左的）
  > 全部列作键。键值按列规则归一（大小写/空白）后**精确匹配**，容差不参与键匹配。
  空白键行由对齐按序配对（与重复键同一机制）；全部列作键时任何单元格改动都变成“删旧行 + 增新行”。
  实际所用的键显示在界面状态栏与 `/automerge` 结果行（`(keys: 1 (inferred))`）。
- **重复键**：第 k 次出现的键与另一侧第 k 次出现配对；结果行带 duplicate keys 警告。
- **行序跟随 center**；只在一侧的新行排在它在本侧前一行（已配对行）之后。单侧调换行顺序**不会**合并进结果。
- **列按位置**、宽度取三方最大值：一侧追加的列像普通改动一样并入；窄的一侧缺的格按空格处理。列级插删/移动不做冲突。
- **单元格判定**比较值与公式（`ValuesAndFormulas`），按列规则（容差、大小写、空白、不重要列）；数字格式与样式不参与
  （只改样式不算改动，输出取该格来源侧的样式）。**公式格按“重定位到输出行坐标后的公式文本”比较**，不比缓存值：一侧插删行后
  Excel 已改写的 `SUM(C2:C6)`→`SUM(C2:C5)` 不算改动；一侧把范围扩到新行算该侧改动。
- 默认输出：未变取 center，左改取 left，右改取 right，同改取 left，异改**保留 center 并计为冲突**；
  左增/右增取对应行，同增取 left；删除不输出；**删改冲突**未解决时保留 center 行；**双增冲突**（同键异内容）未解决时两行都不输出。
- **冲突区段** = 相邻含冲突的对齐行（间距 0，不像文本合并那样把相隔 2 行的改动并段）。
- **解决**：对区段（或行）Take 某侧 → 结构冲突按该侧决定行去留，普通行的冲突格取该侧；对单格 Take 只替换该格；
  接受当前内容不改输出只清冲突；编辑输出格视为解决该格。
- **删除输出行**（`Ctrl+Delete` / Edit ▸ Delete Output Row）：该行不再写出，行上的冲突计为已解决；之后对该行
  Take 任一侧（区段或单格）把它带回。双增冲突行不可删除（Take Center 即“两行都不要”），不在输出里的行（已删除、
  双增未解决）只给提示。可撤销。
- **`/iu` 与 favor**：开 `/iu` 后不重要差异按相同处理；仅由不重要差异构成的冲突格（或同增行）在 `/favorleft|right`
  时取 favored 侧，否则取 center。不开 `/iu` 时 favor 不起作用。命令行不提供列规则，所以 CLI 下“不重要”只在规则
  里有不重要列时才出现。
- **多 sheet**：按名字配对（三方各只有一张时不论名字直接配对）；一侧新增的 sheet 取该侧；一侧删除、另一侧未变 → 不输出；
  一侧删除、另一侧有改动 → sheet 级冲突，未解决时输出 center 版本，Take 删除侧即删除该 sheet；
  输出顺序：center 的 sheet，然后 left 新增，再 right 新增。
- **写出**：同目录临时文件再替换，输出可以是某个输入；扣留时已存在的输出文件保持原样；输出的既有内容不参与合并。
  文本表格以 center 的格式（分隔符、引号、编码、BOM、默认行尾）写回：整行来自某侧的行原样搬运该侧记录，
  混合来源的行只替换变化的字段。
- **工作簿写出**以 center 的 xlsx 包为模板：只有每个 sheet 的 `<sheetData>` 重新生成，其余部件原样复制。每格沿用来源侧的样式；
  只有 left/right 有的样式并入 center 的 `styles.xml`（字体/填充/边框/数字格式去重后追加）。行属性（行高、隐藏、分级、行样式）取该行基准侧。
  文本写为内联字符串，公式写为普通公式（共享/数组公式分组不保留），缓存结果照写，并设 `fullCalcOnLoad` 且删除 `calcChain.xml`，
  Excel 打开即重算。行增删后重定位：本 sheet 公式、合并单元格、条件格式/数据验证/超链接/筛选/选区的 `ref`/`sqref`、表格部件、
  定义名称、图表系列；被删行的单格引用变 `#REF!`，范围收缩。只在 left/right 有的 sheet 以该侧 sheet 为模板加入（去掉其图形、表格、
  批注、条件格式等依赖关系的元素），被删除的 sheet 连同部件移除，`localSheetId` 随之调整。输出为 .xlsx 而 center 是宏工作簿时
  去掉宏工程并警告。模板写出失败（包结构异常）时退回新写工作簿并在结果行警告“written without styles”。
- **没有冲突标记形式**：`/force` 在表格合并里报错 100（`/reviewconflicts` 也不能让 `/force` 生效）。
- xlsx 首行按数据参与合并（不当表头），所以表头行改名也按单元格合并；CSV 的表头由 center 判定一次，按同一设置读 left/right，
  表头名逐列三方取舍。

### `/automerge` 退出码

| 码 | 结果 |
|---|---|
| 0 | 无冲突，已写出 |
| 101 | 有冲突，未写出；已存在的输出文件保持原样 |
| 100 | 参数、输入或写出错误（缺输出目标、输入缺失/不可读、混合格式、`/keys=` 越界、`/force`） |

界面合并退出码与文本合并相同：0 最后一次保存无未解决冲突 / 14 仍有冲突（冲突格按默认内容写出）/ 101 从未保存 / 100 打不开。

## 使用与验证

```bat
matcha base.xlsx theirs.xlsx yours.xlsx result.xlsx                          :: 界面表格合并
matcha /automerge base.xlsx theirs.xlsx yours.xlsx result.xlsx               :: 无窗口：0 / 101 / 100
matcha /automerge /keys=1 base.xlsx theirs.xlsx yours.xlsx result.xlsx       :: 指定键列
matcha /automerge /reviewconflicts base.xlsx theirs.xlsx yours.xlsx out.xlsx :: 无冲突不开窗，有冲突进界面
matcha /tablemerge /automerge base.csv theirs.csv yours.csv out.csv          :: CSV 按表格合并
cargo test -p bc-engine-table --lib merge
cargo test -p bc-script --lib table_merge
cargo test -p bcompare
cargo test -p bc-gui --lib table_merge
cargo test -p bc-script --test table_merge_golden
```

语料：`crates/bc-script/tests/golden/table_merge/<NN-name>/case.json`（`description`、`keys`、`exit`、`conflicts`、`key_label`、
xlsx 例的 `left`/`base`/`right` 工作簿描述与 `expect.formulas|formats|layout`）。`MATCHA_BLESS=1` 按当前输出重写期望文件
（审阅 diff 后再提交）；`MATCHA_GOLDEN_KEEP=<dir>` 把每例的输入与输出留在目录里，便于用 Excel 打开核对。
目录带 `.gitattributes`（`-text`），CRLF/LF/BOM 按字节入库。

P4V：Arguments 与文本合并相同，`%b %1 %2 %r`（或 `/automerge /reviewconflicts %b %1 %2 %r`），指向 xlsx 时自动走表格合并；
CSV 想按表格合并时在前面加 `/tablemerge`。

验收：准备 base / left / right 三个 xlsx（或 CSV），分别构造单侧改格、两侧改同行不同列、两侧改同格异值、
一侧删行另一侧改行、两侧新增同键异内容、一侧新增 sheet；`/automerge` 核对退出码与输出，界面里核对着色、冲突数、
Take/接受/编辑与保存后的退出码。

## 尚未实现

- 公式求值（缓存结果按来源写，打开时由 Excel 重算）；单元格内富文本分段（写成整格纯文本，单元格字体保留）；
  数组公式、共享公式分组；批注锚点、透视表、`docProps/app.xml` 的 sheet 标题不随行增删更新；
  从 left/right 加入的新 sheet 不带其图形、表格与批注；R1C1 / 外部工作簿引用不重定位。
- 列级三方合并（插删列、列移动的冲突语义）；输出网格插入新行（删除输出行已支持）。
- 合并视图内修改键与列规则；合并的命名会话与 Home 入口（比较视图的命名会话见 [saved-sessions.md](saved-sessions.md)）；容差键匹配。
- 冲突区段的 `conflict_gap`（相近冲突并段）。

## 深入阅读

- 变更过程与设计：`openspec/changes/table-merge/`（design.md D1–D10，含评审后的修订）
- 文本合并（同一套命令行词汇与等待/退出码管道）：[text-merge.md](text-merge.md)
- 表格比较（读取、键推断、单元格比较规则）：[table-compare.md](table-compare.md)、`crates/bc-engine-table/README.md`
- 命令行总览：[cli-and-scripting.md](cli-and-scripting.md)
