# Proposal

## Why

text-merge 打通了文本三方合并（`/automerge`、`/reviewconflicts`、P4V 集成、退出码 0/14/101/100），但表格输入被显式排除：今天 `matcha a.xlsx b.xlsx base.xlsx out.xlsx` 会把 xlsx 字节当文本行合并，产出乱码。表格比较已有键对齐、逐格 `CellDiff`、规则与编辑基础设施，缺的是三方合并语义与写出链路。本变更把合并能力扩展到 xlsx/csv：以键对齐做**单元格级**三方合并，CLI 自动合并与 GUI 冲突决策两条腿，复用 text-merge 已建立的 CLI 词汇与等待/退出码管道。

## What Changes

- **三方键对齐**：`bc-engine-table` 新增合并模块——键列哈希（复用双方对齐的哈希逻辑）交给 `bc-core` 已有的策略无关 `align3` 调和为三列行表；键来源为自动推断 + `/keys=<列>` 显式指定，推断失败退化为整行（全部列作键）。一侧新增/删除行、双增同键等结构差异在调和层表达。
- **合并模型与自动合并**：逐（行, 比较列）分类五态（Center/Left/Right/Both/Conflict，同构 text-merge 的 `MergeChoice`）；冲突类别覆盖单元格冲突、双增同键异值、删改冲突（一侧删除、对侧修改）。`auto_merge` 为不依赖 GUI 的纯函数，CLI 与 GUI 共用。未解决冲突的格子默认写 base 值（文本合并默认输出语义）；`/favorleft|right` 照用；**不提供 `/force` 冲突标记的表格等价物**（单元格内无法自然表达标记行）。
- **写出**：输出目标为第 4 个位置参数或 `/mergeoutput=`，可以是新路径，也可以覆盖 left/right 的源路径（暂存 + rename）。CSV/TSV 走既有逐记录保真写回；xlsx 引入 `rust_xlsxwriter` 依赖新写（值 + 公式，**不保**样式/图表/图片/列宽，一期明示限制）。多 sheet 工作簿按名配对逐 sheet 合并，一侧新增 sheet 直接取该侧，输出为 sheet 并集。
- **CLI**：镜像 text-merge 词汇——`/automerge`（进程内无窗口）、`/reviewconflicts`（有冲突转 GUI 决策）、`/favorleft|right`、`/mergeoutput=`、`/keys=`；退出码沿用 0 成功 / 101 有冲突未写出 / 100 错误，GUI 合并页关闭时按其结果码退出。路由：xlsx/xlsm 三方调用进表格合并（现状为乱码，无行为回归）；**csv/tsv 三方保持已发布的文本合并兜底**，走表格合并需显式开关（不 breaking）。
- **GUI 表格合并视图**：上排 left/base/right 三只读网格 + 底部输出网格，冲突导航，Take Left/Base/Right（作用于冲突格及其所在段），保存写出 output。单实例转交、调用方等待、退出码回传原样继承 text-merge 管道。
- **BREAKING（仅命令行返回值）**：xlsx/xlsm 的三方调用原先产出无意义文本合并，现在进表格合并视图/自动合并；csv/tsv 三方行为不变。

不在本变更内：列结构三方合并（一侧增删列时按既有双方列映射处理，不做列级冲突）、xlsx 样式/图表保真写出、表格合并的命名会话与 Home 入口、`/force` 标记、patch/stdin、文件夹三方合并。

## Capabilities

### New Capabilities

- `table-merge-engine`: 三方键对齐（键哈希 + align3 调和 + 孤儿桶）、逐格分类、三类冲突判定、默认输出、冲突解决、自动合并结果与写出（CSV 保真 / xlsx 新写、多 sheet 并集）。
- `table-merge-cli`: 表格合并调用的参数归一与路由（xlsx 自动、csv/tsv 显式）、`/automerge` 无窗口自动合并、`/reviewconflicts` 转 GUI、`/keys=`、`/favorleft|right`、退出码与等待语义。
- `table-merge-view`: GUI 表格合并视图的网格布局、冲突着色与导航、Take 交互、输出编辑与保存、关闭结果码。

### Modified Capabilities

（无——`openspec/specs/` 尚无既有能力规格；CLI 退出码与路由的相关行为以新增 requirement 写入 `table-merge-cli`。）

## Impact

- `crates/bc-engine-table`：新增 `merge.rs`（分类/冲突/自动合并/输出渲染）；`align.rs` 增加三方键对齐入口；`rules.rs` 增加键显式指定来源；`workbook.rs` 增加多 sheet 合并遍历所需的读取面；`store.rs` 写出路径复用。
- `crates/bc-core`：`align3` 已就位，预期**不改动**（若有键哈希序列化的适配需要，仅限新增辅助）。
- 新依赖：`rust_xlsxwriter`（workspace `Cargo.toml`），用于 xlsx 合并结果新写。
- `crates/bc-script`：`cli.rs`（`/keys=` 等开关、`MergeSpec` 扩展或并行 `TableMergeSpec`）、`runner.rs`（表格自动合并执行）。
- `apps/bcompare`：`routing.rs`（三方调用按格式分派：xlsx/xlsm → 表格合并，csv/tsv 默认文本合并 + 显式开关）、`main.rs`（headless 执行与等待）。
- `crates/bc-gui`：新增 `table_merge/`（model/view，复用 `table/` 网格渲染与 `merge/` 的窗格编排概念）；`app.rs`（`View::TableMerge`）；`remote.rs`（合并载荷扩展）。
- 文档：新增 `docs/features/table-merge.md`；更新 `docs/features/README.md`、`table-compare.md`、`text-merge.md`、`cli-and-scripting.md`、`crates/bc-engine-table/README.md`、`.agents/skills/matcha-cli/SKILL.md`、`CHANGELOGS.md`。无参考工具对应功能（BC 无表格合并），`docs/reference-parity/` 不新增对照，语义以 design.md 为准。
- 退出码 0/101/100 与 `/automerge`/`/reviewconflicts` 词汇属既有约定，CI 与 P4V 集成依赖其精确值。
