# Table Report 真实数据回归语料

12 个来自 ProjectLungfish `Main/RawData` 配表管线的真实 xlsx 用例（P4 历史 revision 对，
`p4 print -o` 落盘），覆盖表格差异报告（`/report=summary`）的主要形态与历史缺陷。
2026-11-13 采集并核验：除两个已修复缺陷外，全部经 openpyxl 独立逐格 diff 交叉验证准确。

## 跑法

```bash
python run_corpus.py              # 全部用例与 baseline.txt 对拍，退出码 = FAIL 数
python run_corpus.py --update     # 行为有意变更后用当前构建重建基线（先人工确认新行为正确）
python run_corpus.py --case <目录名>   # 单跑一个用例
```

matcha 路径：`MATCHA` 环境变量优先，否则 `target/debug/matcha.exe` → `target/release/matcha.exe`。
对拍前会把报告头的 left:/right: 绝对路径归一化；FAIL 时用例目录保留 `actual.txt` 供 diff。

## 用例清单

| 用例目录 | 来源（P4 CL，文件 revision） | 变动语义 | 期望报告要点 |
|---|---|---|---|
| `single-cell-key-inferred` | 148367，PLEntityTagMapping_Character #137→#138 | 改 1 个 InstanceTag 值 | `key: A[EntityId]`；1 changed（`F[InstanceTag]` 单元格 old→new） |
| `key-none-rows-changed` | 148325，EquipmentDefinitionTable #286→#287 | 7 行 AttachConfigs 坐标归零 | `key: (none)`；**7 changed**（曾错报为 7 对 `-`/`+`，2026-11-12 修复） |
| `numeric-tune-comment` | 147024，ExpSourceConfig #1→#2 | 数值调参 + Comment 追加备注 | `key: Id`；5 changed，Factor/Threshold/Comment 逐格 old→new |
| `row-cleared-to-empty` | 148108，BuildingMergeAchievementList #53→#54 | 481 行整行清空（成就退休） | `key: AchievementTag`；1 left-only + 1 right-only（键值被改 → `-`/`+` 对是**正确语义**） |
| `batch-append-keyed` | 147194，MortiseMaterial #53→#54 | 表尾追加 10 行 | `key: A[ID]`；10 right-only，ID 206/207/210–215/246/247 |
| `multi-sheet-workbook` | 147356，CraftMaterial #112→#113 | 5 sheet 工作簿，主表 +3 行 | `sheets: 5 compared, 1 different, 4 same`；中文名备份 sheet 判 same 不干扰 |
| `bigtable-append-nokey` | 148196，PLDataAsset_Phase #390→#391 | 4200+ 行大表追加 3 实体 | `key: (none)`；3 right-only / 4237 same |
| `repair-modify-nokey` | 148775，PLEntityTagMapping_WeaponArmor #102→#103 | 修复 5 行 | `key: (none)`；5 changed |
| `batch-append-nokey` | 148780，PLEntityTagMapping_WeaponArmor #103→#104 | 批次新增 15 行（与上一环成同文件链条） | 15 right-only / 258 same |
| `mid-table-insert` | 148711，EntityInstanceConfig #591→#592 | 表中间行 111-114 插入 4 行 | 4 right-only / 2530 same，插入点下方行零错位 |
| `single-cell-clear` | 148700，DropConfig #102→#103 | 单格清空（DropLibrary2） | 1 changed：`J104: 5000066 -> (empty)` |
| `formula-cache-tail-edit-append` | 148756，StaticMeshProperty #443→#444 | CI daily 重算派生列 + 表尾「末行补齐 + 新增 1 行」 | 11 changed + 1 right-only（表尾 3860↔3860、+3861）；**曾双重出错**，见下 |

## 历史缺陷记录（本语料的 regression 锚点）

`formula-cache-tail-edit-append` 一个用例踩中两个已修复缺陷：

1. **公式无缓存值误报**（2026-11-13 修复，见 CHANGELOGS）：右侧由不重算的工具保存，
   公式单元格只有 `<f>` 没有缓存 `<v/>`，曾被按空值比较 → 884 行大面积误报
   （O 列 LinearDamping 334 格、T 列 线性阻尼查询值 805 格伪差异）。修复后只剩 11 行真实差异。
2. **表尾「修改+新增」配对错位**（2026-11-13 修复）：左末行 3860（LeafPile 被补齐 B/E/F）
   曾错配到右新增行 3861（Feast.Roast.Boar），报出 `TagName LeafPile -> Boar` 伪漂变，
   真改动行反被报成纯新增。修复后 3860↔3860 正确配对为 changed，3861 为 right-only。

## 口径说明（合成探针实证）

- `title-row:1` 指定的标题行不计入 rows 统计；单侧尾空行整体忽略（verdict 不受影响）。
- 因此 `same` 数比文件物理行数少 1~2 属正常口径。
- 本语料只覆盖 `/report=summary` 表格报告；文本 patch、文件夹比较等不在此池。
