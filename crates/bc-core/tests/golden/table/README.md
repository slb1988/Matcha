# 表格黄金语料

30 个用例：27 个 CSV 比较、3 个由 CSV 源数据生成工作簿的比较（含一个双 sheet 用例）。首行明确作为数据参与参考报告比较，不把它误当为已验证 GUI 自动表头设置。

每例包含 `case.json`、`left.csv`、`right.csv`、`native-rules.xml` 和一份参考 HTML。`native-rules.xml` 仅包含合成会话的 Rules，不是私人会话/格式配置的备份。xlsx 由 `tools/golden/gen_table_cases.py` 或测试中的 `support/xlsx.rs` 临时生成，不入库二进制工作簿。

## 原生报告的无损文本封装

通常直接保存 `expected.html`。`027-quoted-newline` 的原生报告在两个多行字段中含 NUL，因此保存为 **`expected.html.json`（一个 JSON 字符串）**；解码后才交给同一个 HTML 解析器。每个用例必须恰好有一种报告，不能留下两个互相覆盖的版本。

- 封装只转义传输字符，不修改 HTML 的颜色、class、行序、文本或统计；不是把差异归一化成相同。
- 此目录的 `.gitattributes` 对 CSV/HTML/JSON/XML 禁止自动换行转换，并把原生 CRLF 识别为合法行尾；仍检查真正的尾随空白，不为通过检查改写原始语料。
- 027 解码结果为 3,952 字节、两个 NUL；本次捕获的 SHA-256：`d29a2c51f9661aaf68d1273038a7bcc748a5a6dacd105e5c71baf7b586640825`。收口时与原件逐字节校验通过。重新采集的时间/路径可能改变该摘要，不以此固定未来报告的非语义元数据。
- 原始含 NUL 的 HTML 留在本机 `target/wave2-closeout/027-expected.native.html`，不提交。回归从已提交 JSON 读取，不依赖该本地副本。
- HTML 中保留原生 `BcImages/*.png` 引用以维持原文；**不提交这些参考工具图标**。解析器忽略装饰性图片，不要求浏览器图片资源完整。

## 验证与再生成

```text
cargo test -p bc-engine-table --test golden_table -- --nocapture
```

同时核对行状态与单元格三态；本次为 81/83（97.59%）、236/242（97.52%）。仅允许 [KNOWN_DIFFS.md](KNOWN_DIFFS.md) 中完整签名锁定的 `024-unaligned`，其他差异会失败。

生成器为 `tools/golden/gen_table_expected.ps1`，依赖同目录的共享 `reference_lock.ps1`。只有经授权运行参考工具时才使用；指定 `-ReferenceExe`，按需指定 `-SessionsPath`。它备份指定会话文件、持共享互斥锁、仅加入专用合成会话，并在 finally 恢复及核对字节。含 NUL 的报告自动写 JSON 包装，其他报告直接复制，并删除该用例过期的另一种表示。

本次收口只复跑已捕获语料的回归、验证脚本语法及 JSON 往返，**没有重新运行参考工具采集链路**；5.1 不据此勾选。备份、参考工具图标、私人配置、生成的 xlsx 和构建日志均不得入库。
