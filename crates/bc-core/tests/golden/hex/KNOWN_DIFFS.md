# hex-compare 黄金对拍 KNOWN_DIFFS

对拍基线：参考工具 5.0.4.30422（macOS）GUI 报告（纯文本），语料 = `docs/reference-parity/hex-compare/corpus/left.bin|right.bin`（与 `corpus.rs` 生成器逐字节一致）。

**当前无已知差异**：`bc-engine-bin/tests/golden_hex.rs` 四项对拍全绿——

- side-by-side（display-all，line-numbers，24 格/行）：173 行逐格一致（地址标签、字节、占位格、差异判定）。
- display-mismatches：11 行一致（区域自起点重打包 + 16 字节上下文 + 保留区域全部行）。
- summary：四分类计数 4120/32/16/8 一致。
- interleaved（display-all，line-numbers）：双侧字节流逐字节重建一致。

后续对拍若出现正式差异，按行登记：现象 / 期望（参考）/ 实际（Matcha）/ 处置（修复或接受）。
