# hex-compare 黄金语料

`corpus.rs` 是确定性语料生成器（任务 3.1）：CPython 兼容 MT19937 伪随机 + 程序化
改动/插入/删除，与 `docs/reference-parity/hex-compare/corpus/` 已提交文件逐字节一致
（`golden_hex.rs` 断言绑定）。**改语料 = 改代码**：禁止手工编辑 .bin。

- `primary()` → left.bin(4,160B)/right.bin(4,144B)：8 改动 + 16 插入 + 32 删除，
  期望四分类 4120 same / 8 difference / 32 left orphan / 16 right orphan。
- `odd_insert()` → l2.bin(2,048B)/r2.bin(2,053B)：0x123 处 5 字节插入（AA..EE），
  验证行中孤儿与行标签相位。

## 参考期望（已入库）

`expected/` 四份纯文本报告已由参考工具 5.0.4.30422（macOS GUI 报告对话框）生成并入库，
`bc-engine-bin/tests/golden_hex.rs` 四项对拍全绿，无已知差异（`KNOWN_DIFFS.md`）。

若需在 Windows Pro 机重生成（如升级参考版本），用下面的脚本（持 `%TEMP%\matcha-reference-tool.lock`）：

```text
:: gen_expected.bcscript（脚本模式逐行执行；路径按生成机实际位置替换）
load <repo>\docs\reference-parity\hex-compare\corpus\left.bin <repo>\docs\reference-parity\hex-compare\corpus\right.bin
hex-report layout:side-by-side options:display-all,line-numbers output-to:"<repo>\crates\bc-core\tests\golden\hex\expected\sbs-all.txt"
hex-report layout:side-by-side options:display-mismatches,line-numbers output-to:"<repo>\crates\bc-core\tests\golden\hex\expected\sbs-mismatches.txt"
hex-report layout:summary options:display-all output-to:"<repo>\crates\bc-core\tests\golden\hex\expected\summary-all.txt"
hex-report layout:interleaved options:display-all,line-numbers output-to:"<repo>\crates\bc-core\tests\golden\hex\expected\interleaved-all.txt"
```

调用参考工具前持 `%TEMP%\matcha-reference-tool.lock`（互斥纪律见 design D7）。

期望文件入库后，`golden_hex.rs` 增加逐字节对拍断言；正式差异记本目录
`KNOWN_DIFFS.md`（随首个差异创建）。

### 格式要点（实测）

- side-by-side **24 格/行**（与视图默认 16 不同），`=` 标相同块首行、`<>` 标差异块首行，
  块间虚线分隔；孤儿区对侧整格留空。
- display-mismatches 按差异区域自起点重打包（16 字节上下文），保留区域全部行。
- summary 输出四分类计数本地化文案；interleaved 每行 `左地址 右地址 字节 ASCII`。

## 口径备注

- `options:` 还支持二进制字符串表证实、CHM 未载的 `display-mismatches-no-orphans`。
- 报告对话框（GUI）与脚本参数一一对应：布局三选 / 选项 / 包含字节地址(=line-numbers) /
  标题 / 输出=打印机|HTML|纯文本 / 折行=无|词组|字符。
