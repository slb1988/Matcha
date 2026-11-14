# Design

## Context

text-merge 已落地：`bc-core` 的策略无关 `align3`（两次双方对齐 + 以 center 为轴调和）、`MergeSpec` 参数模型、`/automerge`/`/reviewconflicts` 词汇、退出码 0/14/101/100、单实例转交与调用方等待管道（`remote.rs` 的合并载荷与结果码回传）。表格比较已落地：键列哈希与排序对齐（`align.rs` 的 `hash_row`/`KeyAlign`，序列对齐同样委托 bc-core）、逐格 `CellDiff` 与规则（容差/大小写/公式）、`Sheet` 编辑与 CSV 逐记录保真写回（`store.rs`）、xlsx 经 calamine 只读加载（多 sheet 按需）、GUI 网格渲染（`table/`）。缺口：三方合并语义、xlsx 写出、合并视图与路由分派。动机见 proposal.md。

## Goals / Non-Goals

**Goals:**

- 合并语义只有一份实现：三方键对齐、逐格分类、冲突判定、`auto_merge` 均为不依赖 GUI 的纯函数，CLI 与 GUI 共用（同构 text-merge 的 D2）。
- CLI 词汇与退出码和 text-merge 完全同构；P4V `matcha %1 %2 %b %r` 指向 xlsx 即可用。
- xlsx/xlsm 三方调用自动进表格合并；csv/tsv 保持已发布的文本兜底，`/tablemerge` 显式进入。
- 多 sheet 工作簿逐 sheet 合并，输出为 sheet 并集。

**Non-Goals（一期不做，写明以免实现时摇摆）:**

- 列结构三方合并：任一侧增删列不做列级冲突；列映射按位置对齐（见 D7），多出的列忽略并告警。
- xlsx 样式/图表/图片/列宽的保真写出；公式求值（沿用"不执行公式"的既有立场）。
- 冲突标记（`/force` 的表格等价物）；输出网格的结构编辑（插/删列）。
- 命名会话与 Home 入口；合并视图内修改规则（键与规则在打开时确定，同 text-merge）。
- 文件夹三方合并。

## Decisions

### D1: 三方键对齐 = 键令牌连接 + 分桶定位（评审修订，原方案见下）

**实现采用**：复用双方对齐的 `align::hashes`（键列归一、按公式哈希、出现序号）得到三方各行的键令牌；center 令牌建索引，
left / right 各自查找配对；center 没有的 left 行与 right 行再按令牌互配（这才能识别"两侧新增同键"的同增 / 双增冲突）。
孤儿行按"本侧前一个已配对行所对应的 center 行"分桶（与 `reconcile3` 的分桶规则相同），桶内先 left 后 right-only；
最终行序 = center 物理行序 + 桶内孤儿。

**为什么改**：原方案的 `align3` 调和把同桶左右孤儿**按位置**并排，不按键配对，D2 的"双增冲突（同键异内容）"无法成立；
而为了让键对齐不受行序影响先按键排序，会让输出按键重排、破坏 CSV 保真写回（行序本身也是用户数据）。键是行身份，
令牌连接在唯一键时与排序对齐等价，重复键按出现序配对与双方比较一致。代价：单侧调换行序不合并（行序跟随 center），已写入 feature 文档。

原方案（保留作记录）：复用双方对齐的 `hash_row`（按键列哈希，容差列在精确档跳过）得到三方的键哈希序列；各方序列先按键排序（`KeyAlign` 的既有做法，保证 align 输入有序、重复键相邻），再交给 `bc-core` 的 `align3`：两次双方对齐（left↔center、center↔right）+ 以 center 为轴调和，孤儿行归入桶（桶号 = 之前出现的 center 行数），同桶左右孤儿按位置并排。每一对的配对结果与单独做双方表格比较一致，与 text-merge D1 同一套不变量。重复键：报告 `DuplicateKeys`（三方扩展），重复组内按出现顺序配对。

替代方案：键并集直接配对（跳过 align3）——唯一键时等价，但重复键与无序输入的配对语义要重新发明，且失去与双方比较的一致性保证，弃。

### D2: 单元格五态分类 + 行级结构判定，两层分离

与 text-merge 的"行类型 / 冲突区段"两层同构：

- **单元格类型**（每个对齐行 × 比较列）：Center/Left/Right/Both/Conflict 五态，相同性判定复用 `compare.rs` 的 `CellComparator` 与规则（容差、大小写、空白、按公式），比较层固定为 `ValuesAndFormulas`（评审修订：只比值会丢掉"只改公式、缓存值未变"的改动）。
- **行级结构**：左增 / 右增 / 同增 / 双增冲突（同键异内容）/ 删除 / 删改冲突。判定的"内容不同"归结为该行是否存在 Conflict 格。

默认输出：逐格取 未变→center、左改→left、右改→right、同改→left、异改→center；结构行 左增/右增/同增取对应侧、删除不输出、删改冲突保留 center 行、双增冲突不输出任一新行。冲突区段 = 相邻含冲突对齐行的聚合（一期 gap=0，即只聚合物理相邻；text-merge 的 `conflict_gap` 在表格里语义存疑，见 Open Questions）。

### D3: 写出分层——CSV 保真 / xlsx 新写

- CSV/TSV：合并输出建模为对 center 源 sheet 的一组编辑（`EditItem`），交给既有 `store::save_store` 逐记录保真写回（未改字段、分隔符、行尾原样），暂存 + rename。取 left/right 的行按其来源侧的字节形态写出。
- xlsx：引入 `rust_xlsxwriter` 新写工作簿，逐 sheet 写值与公式。不保留字体/填充/边框/列宽/合并单元格/图表/图片（一期明示限制）。公式缓存值：已验证 `Formula::set_result` 可写，calamine 读回值与公式均一致。
- 评审补充：**数字格式随格写出**（读取时已有 `Style.number_format`，`builtin:N` 用 `set_num_format_index`）——否则日期格写成序列号、百分比写成小数，值虽相等但用户可见内容被破坏；日期值（Unix 毫秒）换算为 Excel 序列日并配日期格式。
- 评审补充：**xlsm 宏工程**：以 `.xlsm` 输出却写普通 xlsx 内容类型时 Excel 拒绝打开。从输入（center 优先）用 `zip` 取 `xl/vbaProject.bin`，输出扩展名为 xlsm（或不是 xlsx）时原样带入；输出 .xlsx 而输入有宏时给警告。
- 替代方案：`umya-spreadsheet` 读写保样式——依赖重、改写整文件的正确性风险高，弃；直接 zip 级外科手术改 sheet XML——复杂且脆弱，弃。

### D4: 路由按格式分派，csv/tsv 不 breaking

`routing.rs` 在 Merge3/Merge4/AutoMerge 分派前做格式检测（`bc_script::table_merge::merge_kind`）：全部输入为工作簿 → 表格合并；全部不是 → 默认文本合并，`/tablemerge` 改走表格合并（任何非工作簿输入都按分隔文本表格读，xls/ods → 100）；工作簿与非工作簿混合 → 100。

评审修订：工作簿识别不能只看扩展名——版本控制工具的临时文件名常带 `#版本` 等后缀（扩展名不是 xlsx），只看扩展名会退回文本合并、产出乱码。识别规则：扩展名 xlsx/xlsm，或扩展名不认识（非 csv/tsv/txt/tab）且文件头为 ZIP `PK`。路由因此读三个输入的前 4 字节（唯一的 I/O）。注意：text-merge（在飞）的 `text-merge-cli` 规格声明"合并调用一律按文本合并处理"且 xlsx 报"不是文本文件"——本变更落地后该 requirement 需要以 MODIFIED delta 修订（谁后 archive 谁负责对齐，见 Risks）。

### D5: CLI 词汇镜像，无 `/force`

开关集合：`/automerge`、`/reviewconflicts`、`/favorleft|right`、`/iu`、`/mergeoutput=`、`/silent`、`/solo`、`/title1..4`，语义与 text-merge-cli 规格逐条对应。新增 `/keys=<列号,…>`（1-based 比较列号，顺序即优先级）。`/force` 在表格合并中报错 100（显式失败优于静默分歧：表格单元格无法表达标记行，CI 误以为拿到带标记输出比直接报错更糟）。退出码：headless 0/101/100；GUI 0/14/101/100（14 = 保存时仍有未解决冲突，冲突格已按默认内容写出）。

### D6: 键来源优先级与暴露

`/keys=` > 既有键推断（`KeySelection::Automatic` 同款逻辑，对 center 与各侧的配对列推断）> 退化全部比较列作键。实际使用的键列对调用方可见：GUI 打开时显示，headless 的结果行包含键信息。推断失败退化整行时合并语义退化为"行增删"（任何单元格改动 = 旧行删除 + 新行增加，天然全部是双增/删除结构判定），行为可预期。

### D7: 列映射一期按位置

三方的列按位置（列号）对齐为同一组比较列，**宽度取三方最大值**（评审修订：原方案"超出 center 列数的列忽略并告警"会静默丢掉一侧追加的列）；窄的一侧补齐空格，于是一侧追加的列与普通单元格改动同样合并，删末列表现为该侧把格清空。列级插删/移动不做冲突语义（Non-Goal）。

### D9: 文本表格的表头由 center 决定一次（评审补充）

CSV 表头自动检测是逐文件启发式；三方各自检测可能不一致（例如一侧把某列改成全数字），导致行号整体错位一行。
读取时先读 center（按格式设置/自动检测），再以同一结果（Names / Data）强制读 left 与 right。表头名逐列三方取舍
（与 center 原表头相同时复用原始表头记录写回）。xlsx 不取表头：首行按数据合并、原样写回。

### D10: 键推断须三方唯一（评审补充）

`infer_keys` 只对两侧打分。合并时对 (center, left) 与 (center, right) 各推断一次，候选列组还须在第三方也唯一才采用，
否则退化为全部列作键——避免某侧出现重复键时把不同实体错配为一行。

### D8: GUI 复用双层资产

- 网格渲染复用 `table/`（虚拟滚动、按 `CellDiff` 着色、sheet tabs），合并视图是新的 `table_merge/`（model + view），编排概念借自 `merge/`（上排输入、底部输出、冲突导航、结果码）。
- 状态模型：`MergeState`（引擎）× sheet 数；GUI 持有各 sheet 的独立状态，切换不丢解决进度。
- 单实例转交/等待/结果码：不动 `remote.rs` 协议，`View::TableMerge` 实现与 `TextMerge` 相同的结果码接口；`/reviewconflicts` 转入时把 headless 已应用的 favor 等状态带入初始 `MergeState`。

## Risks / Trade-offs

- [xlsx 写出丢样式/图表，覆盖输入时尤其明显] → 文档（feature 文档 + `--help`）明示；输出目标等于某个输入且为 xlsx 时在结果行/stderr 给出提示；后续版本可评估保真写出库。
- [键推断错误导致合并语义错误（错把不同实体配成一行）] → 实际使用的键列在 GUI 与结果行可见；`/keys=` 显式覆盖；退化行为确定（整行作键）。
- [`rust_xlsxwriter` 可能无法保留公式缓存值] → 任务中先行验证 API；不支持则只写公式文本，行为与"无缓存值公式"的既有读取语义兼容，不阻塞。
- [text-merge 在飞，其"一律文本合并" requirement 与本变更 D4 冲突] → 两个 change 的 archive 顺序若 text-merge 在前，本变更 archive 前补一个对 `text-merge-cli` 的 MODIFIED delta（按格式分派）；反之则在 text-merge archive 时处理。两 change 代码层面无重叠文件（merge/ vs table_merge/、引擎分属 text/table），冲突仅限规格。
- [align3 在大表上的性能（三方排序 + 哈希 + 调和）] → 复用已测性能的双方管线；任务含 bench（`--bench-table` 思路的三方扩展或 golden 样本），退化阈值沿用既有。
- [多 sheet 合并的状态内存（每 sheet 一份 MergeState + ComparedData）] → sheet 状态懒建（首次切换到该 sheet 才合并），与 workbook 的按需加载一致。

## Migration Plan

无数据迁移。CLI 行为变化两处：xlsx 三方调用由"报不是文本文件/产出乱码文本合并"变为表格合并（修复性质）；新增 `/tablemerge`、`/keys=` 开关。csv/tsv 三方行为不变。发布时按 CHANGELOGS 记录 BREAKING（仅命令行返回值类）。

## Open Questions

- 冲突区段是否需要 `conflict_gap` 类参数（把相距很近的行级冲突并段）？一期 gap=0，按使用反馈再加。
- ~~`rust_xlsxwriter` 写公式缓存值的确切 API 形态~~：已解决，`Formula::set_result`（D3）。
- 输出网格后续是否支持结构编辑（插/删行）？一期仅单元格编辑 + 结构冲突取舍。
- 键匹配不支持容差（容差只影响单元格相同性）；有需求再引入双方对齐的 `tolerant_tokens` 三方版本。
