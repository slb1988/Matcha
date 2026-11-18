# Matcha 实测报告：Main/RawData xlsx 的 P4 变动对比（2026-11-12）

目的：验证 `Tools/matcha/` 内置的 matcha 三件套可用于 RawData Excel 的版本差异审查，
并把「p4 print 两个 revision → matcha 表格报告」固化为标准配方。

环境：Windows，Git Bash；调用 `Tools/matcha/matcha.com`（README 明确：Git Bash 里要写全 `matcha.com`）。
原始产物（两个 revision 的 xlsx、两份原始报告 txt）保留在 `Main/Saved/MatchaTest/`（不入 P4）。

## 标准配方

```bash
# 1. 找 RawData xlsx 的最近 P4 变动
p4 changes -m 15 -s submitted "Main/RawData/....xlsx"
p4 describe -s <CL> | grep '\.xlsx'        # 拿到 文件#revision

# 2. p4 print -o 把改动前后两个 revision 落成独立文件（二进制安全，不动工作区）
p4 print -q -o Main/Saved/MatchaTest/xxx_rN.xlsx   "Main/RawData/<path>.xlsx#<N>"
p4 print -q -o Main/Saved/MatchaTest/xxx_rN+1.xlsx "Main/RawData/<path>.xlsx#<N+1>"

# 3. 单元格级差异报告；title-row:1 把第 1 行表头附进列名（A → A[EntityId]）
Tools/matcha/matcha.com -report=summary -reportoptions=title-row:1 old.xlsx new.xlsx
```

注意：Git Bash/MSYS 下开关一律用 `-` 前缀（`/report=...` 会被路径转换吃掉）。

## 用例 1：PLEntityTagMapping_Character.xlsx #137 → #138（CL 148367 "hill chameleon mapping v1"）

命令：

```bash
Tools/matcha/matcha.com -report=summary -reportoptions=title-row:1 \
  PLEntityTagMapping_Character_r137.xlsx PLEntityTagMapping_Character_r138.xlsx
```

输出（全文 11 行，差异极小）：

```text
MATCHA-TABLE-REPORT 1
result: different
sheets: 1 compared, 1 different, 0 same, 0 left-only, 0 right-only

=== sheet "PLEntityTagMapping": different
key: A[EntityId]
rows: 1 changed, 0 unimportant, 0 left-only, 0 right-only, 200 same
~ A[EntityId]=4069  (left row 70, right row 70)
    F[InstanceTag] (F70): Instance.Character.Monster.FillerMisc.Chameleon -> Instance.Character.Monster.Chameleon
```

解读：键列自动推断成功（`key: A[EntityId]`），直接给出「EntityId=4069 这行的 InstanceTag
从 FillerMisc.Chameleon 改成 Chameleon」，与 CL 描述一致。这是键列可推断时的理想形态。

## 用例 2：EquipmentDefinitionTable.xlsx #286 → #287（CL 148325 "Fix VFX proxy component tags"）

命令同上（报告原文 `Main/Saved/MatchaTest/report_equipdef_r286_r287.txt`）。结构行：

```text
=== sheet "EquipmentDefinitionTable": different
key: (none; rows aligned by content)
rows: 0 changed, 0 unimportant, 7 left-only, 7 right-only, 3174 same
```

解读：键列推断失败（首列是 `Name` 文本且含重复/空值），退化为「按内容对齐」。

**2026-11-12 初版行为**：7 处改动显示为 7 对 `-`/`+`（假删除+新增），每对左右行号相同
（如 `left row 1272 / right row 1272`），实际含义 = 该行被修改，但无单元格级定位。

**2026-11-12 复验（matcha 修正后，exe 重建于 17:44）**：对齐后内容不同的行直接显示为
`~` 改动行并附单元格明细，结构行变为：

```text
key: (none; rows aligned by content)
rows: 7 changed, 0 unimportant, 0 left-only, 0 right-only, 3174 same
```

7 个 `~` 行：1172 / 1194 / 1272 / 1362 / 1384 / 1471 / 3100（左右行号一致），
每行附单元格级差异，形如 `    K[AttachConfigs] (K1172): <old JSON> -> <new JSON>`。
报告原文：`report_equipdef_r286_r287.txt`（修正前）/ `report_equipdef_r286_r287_v2.txt`（修正后）。

本例改动内容：7 行的 `K[AttachConfigs]` 第二个 AttachConfig 变化——6 行是
AttachLocation/AttachRotation/Scale 重置为 `0.0/0.0/1.0`（外加 `0`→`0.0` 的导出器格式归一），
1 行（1194）是坐标改到另一组非零值。方向与 CL 描述「Fix VFX proxy component tags」一致。

**经验**：`key: (none; …)` 时 `~` 行即改动行；单元格是整段 JSON 时，把该单元格的
`old -> new` 两侧再喂一次文本 diff（或正则提取数值段）定位字段级差异。

## /qc 在 xlsx 上的结论粒度

```text
matcha.com -qc r137.xlsx r138.xlsx   →  Binary differences, exit=11
matcha.com -qc r137.xlsx r137.xlsx   →  Rules-based same,  exit=2
```

`/qc` 对 xlsx 只给二进制级结论（xlsx 是 zip，单元格级相近也会 binary different）。
要「单元格级是否相同」看 `-report=summary` 的 `result:` / `rows:` 行；`/report` 模式退出码恒 0。

## 大 JSON 单元格报告的速读法

RawData 导出的 xlsx 常含整段 JSON 单元格，summary 里 `-`/`+` 行可达数千字符。先看结构再取细节：

```bash
grep -E '^(MATCHA-TABLE-REPORT|===|key:|rows:)' report.txt     # 结构
grep -c '^~ ' report.txt                                       # 改动行数
grep -A6 '^~ ' report.txt | cut -c1-220                        # 改动行截断预览
```

## 结论

- matcha 对 RawData xlsx 的 P4 变动审查完全可用：无窗口、退出码/报告双通道、键列可推断时精确到单元格。
- 文本文件走 `-report=patch-unified`；xlsx/CSV 走 `-report=summary -reportoptions=title-row:1`。
- 键列推断失败时按内容对齐，改动行显示为 `~` 并附整单元格 old→new；报告粒度到单元格为止，
  JSON 大单元格内的字段级定位需对该单元格两侧内容再做一次文本 diff。

## 批次 2：五个代表性用例（2026-11-12）

目的：覆盖「数值调参 / 清空行 / 批量新增 / 多 sheet / 大表追加」五种形态。
原始 xlsx 与报告在 `Main/Saved/MatchaTest/batch2/`；统一命令：

```bash
# 在仓库根目录执行（相对 client 路径依赖 cwd；否则用 //CyanCookOfficialDepot/MainDev/... 全路径）
p4 print -q -o <old>.xlsx "Main/RawData/<path>.xlsx#<N>"
p4 print -q -o <new>.xlsx "Main/RawData/<path>.xlsx#<N+1>"
Tools/matcha/matcha.com -report=summary -reportoptions=title-row:1 -reportfile=<out>.txt <old>.xlsx <new>.xlsx
```

### 用例 3：ExpSourceConfig.xlsx #1→#2（CL 147024 "EXP config tune"）—— 单元格数值调参

结构：`key: Id`，`rows: 5 changed, 14 same`。5 个 `~` 行（Id=5/6/7/9/11），单元格级 old→new：

```text
~ Id=5  (left row 8, right row 8)
    Factor (E8): 0.1 -> 0.002
    AccumulateThreshold (F8): 100 -> 2500
    Comment (H8): 超重移动→体魄:IsOverweight直查,…系数TODO待策划定 -> …| 2026-11-08 修:…
~ Id=9  (left row 11, right row 11)
    Factor (E11): 0.01 -> 0.002
    AccumulateThreshold (F11): 1 -> 2500
```

评价：**最理想形态**——键列推断成功 + 数值调参，直接给出「哪条来源、哪个字段、旧值→新值」，
连策划在 Comment 列追加的修复备注都完整呈现。

### 用例 4：BuildingMergeAchievementList.xlsx #53→#54（CL 148108 "Building - Retire"）—— 清空行

结构：`key: AchievementTag`，`rows: 1 left-only, 1 right-only, 497 same`。

```text
- AchievementTag=Achievement.Show.RoadandBridges.Overpass  (left row 481)
+ AchievementTag=(empty)  (right row 481)
    Id=(empty) | Name=(empty) | … | ExpToGive=(empty) | AchievementName=(empty)
```

结论：该 CL 实际是「清空 481 行」而非删行。键列值本身被改动时显示为 `-`/`+` 对是**正确语义**
（键变了就不再是同一行），与对齐失败的退化形态要区分开——本例键列推断是成功的。

### 用例 5：MortiseMaterial.xlsx #53→#54（CL 147194 "Update WeaponFix and ArmorFix"）—— 批量新增行

结构：`key: A[ID]`，`rows: 10 right-only, 144 same`；第二 sheet「~数据处理」same。
新增 ID：206、207、210–215、246、247，落在行 145–154（表尾追加），每个 `+` 行自带键值，一目了然。

### 用例 6：CraftMaterial.xlsx #112→#113（CL 147356 "Update Handle"）—— 多 sheet 工作簿

结构：`sheets: 5 compared, 1 different, 4 same`。逐 sheet 出结论，含 `~720备份`、
`~610备份2（结束后恢复）` 等中文名备份 sheet（其中两个键列推断失败但正确判 same，未误报）。
主 sheet `key: Name`，3 个 `+` 行：

```text
+ Name=OneHandedMelee_Entity.Handle.Rebel  (right row 603)
+ Name=Shield_Entity.Handle.Rebel          (right row 604)
+ Name=Torch_Entity.Handle.Rebel           (right row 605)
```

### 用例 7：PLDataAsset_Phase.xlsx #390→#391（CL 148196 EntityAdd 每日批次）—— 大表追加

结构：主 sheet 4237 same + `3 right-only`（行 4238–4240），`key: (none)` 但按内容对齐成功。
新增实体：`Entity.Goaf`、`Entity.StickyBall`、`Entity.Bandage`。
4200+ 行大表只报 3 行差异，信噪比好；`+` 行缩进行附完整单元格内容（如 GrantedSensitiveTags 列表）。

### 批次 2 小结

| 形态 | 用例（文件 / CL） | 报告表现 |
|---|---|---|
| 数值调参（键列推断成功） | ExpSourceConfig / 147024 | `~` + 单元格 old→new |
| 清空行（键值被改） | BuildingMergeAchievementList / 148108 | `-`/`+` 对，正确语义 |
| 批量新增（键列推断成功） | MortiseMaterial / 147194 | `+` 带键值列表 |
| 多 sheet 工作簿 | CraftMaterial / 147356 | 逐 sheet 结论，备份 sheet 不干扰 |
| 大表追加（键列推断失败） | PLDataAsset_Phase / 148196 | 按内容对齐，`+` 行附完整内容 |

## 批次 3：最新 5 个 CL 的准确性交叉验证（2026-11-13）

方法：matcha 报告之外，用 `Main/Saved/MatchaTest/batch3/verify_matcha.py` 以 openpyxl 独立逐格 diff
（difflib 行对齐 + 值级比对），双向核对「行数统计 / 改动行身份 / 单元格 old→new 值 / 增删行号」。

用例（2026-11-13 最新 5 个触及 RawData xlsx 的 CL）：

| CL | 文件与 revision | 变动性质 |
|---|---|---|
| 148775 Entity Pipeline - Repair | PLEntityTagMapping_WeaponArmor #102→#103 | 5 行修复 |
| 148780 EntityAdd 每日批次 | PLEntityTagMapping_WeaponArmor #103→#104 | +15 行（与上一环成同文件链条） |
| 148756 StaticMeshProperty daily | StaticMeshProperty #443→#444 | 883 行批量变化 + 表尾新增 |
| 148711 building door instances | EntityInstanceConfig #591→#592 | 表中间插入 4 行（行 111-114） |
| 148700 burn drop 50pct | DropConfig #102→#103 | 1 格改动 |

### 交叉验证结果

| 用例 | matcha 报告 | openpyxl 复核 | 结论 |
|---|---|---|---|
| wa #102→#103 | 5 changed / 253 same | 逐格一致 | ✅ 准确 |
| wa #103→#104 | 15 right-only / 258 same | 逐格一致 | ✅ 准确 |
| smp | 884 changed + 1 right-only / 2976 same | 883 行逐格一致；表尾 1 行配对错位 | ⚠️ 见下 |
| eic | 4 right-only（行 111-114）/ 2530 same | 中间插入对齐完美，两侧 2530 行不错位 | ✅ 准确 |
| drop | 1 changed（J104: 5000066 → empty） | 一致 | ✅ 准确 |

### smp 表尾配对错位（真实缺陷）

左末行 3860 = `Entity.LeafPile`（仅 A/M 两列有值）；右侧 3860 = LeafPile 补齐了 B/E/F，
3861 = 新增 `Entity.Feast.Roast.Boar`。正确配对：3860↔3860（改动）+ 3861 新增。
matcha 实际输出：`+ (right row 3860)`（把改完的 LeafPile 当纯新增）+
`~ (left 3860, right 3861): TagName Entity.LeafPile -> Entity.Feast.Roast.Boar`（伪键值漂变）。
触发条件：表尾「末行被修改 + 其后紧跟新增行」。差异总量碰巧不变（884+1），但末两行语义误导。

**后续（2026-11-13）**：Matcha 当日构建已同时修复两点——公式无缓存值被按空值比较的大面积误报
（884 行 → 11 行真实差异 + 1 行新增）与本配对错位（3860↔3860 正确配对，3861 纯新增），
见 Matcha 仓库 CHANGELOGS 2026-11-13。两个修复的回归锚点已收入 Matcha 仓库
`docs/reference-parity/table-report/corpus/`（12 用例 + run_corpus.py 对拍脚本，全部 PASS）。
已随 11-13 11:42 部署构建修复并复核（Matcha 仓库 12 用例回归全部 PASS）；
SKILL.md 对应坑条目已移除，历史记录留本文档。

### smp 暴露的数据事实（副产品）

CL 148756 把 O 列 LinearDamping（334 格）与 T 列 线性阻尼查询值（805 格，含原 #N/A）整体清空，
另 F/E 列约 20 格、B 列 1 格变化。需找 CI owner 确认线性阻尼两列清空是否预期。

### 行数统计口径（合成探针证实）

- title-row:1 指定的标题行不计入 rows 统计（5 行探针：1 changed + 3 same）。
- 单侧尾空行整体忽略，甚至不改 verdict（5 行 vs 6 行探针判 same）。
- 因此 `same` 数比物理行数少 1~2 是正常口径，不是漏报。

### 批次 3 结论

5 个最新 CL 中 4 个完全准确；smp 在表尾「修改+新增」组合处配对错位（单元格值本身零误差，错的是行配对）。
matcha 报告可直接用于配表审查；看到表尾出现「键值整体漂变」的 `~` 行时，先人工复核末几行。
