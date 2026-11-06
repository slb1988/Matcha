# bc-formats

File-format registry and syntax grammar model. Converts the reference tool's embedded
`BCFILEFORMATS` resource into Rust data at build time and reads/writes user format
libraries in the same XML dialect.

## Responsibilities

- `model` – `FileFormat`, `FormatKind` (6), `GrammarItem` + `GrammarMatch` (4 match
  types), `ElementKind` (10), `ConvertKind`, `WeightItem`, `TableOptions`.
- `bcff` – implemented `parse_bcfileformats` / `write_bcfileformats` (shared with build.rs),
  preserving source XML nodes and patching only changed typed properties.
- `builtin_formats()` – the embedded library: `build.rs` parses `data/bcfileformats.xml`
  and emits JSON into `OUT_DIR`; the library deserialises it once (`OnceLock`).
- `registry` – `FormatRegistry` (lookup by `(kind, name)`, mask resolution, import/export).
- `grammar` – compiled, stateful `Tokenizer`; `line_weight` feeds `AlignInput::weights`.
- `text_rules` – `UnimportantText`, `Replacement`, `ElementImportance`.

Test: `cargo test -p bc-formats` – parses the XML, compares with the embedded data,
checks 25 formats (19 text, 2 table, 1 picture, 1 media, 1 version, 1 XML), all 4
grammar classes, all 10 element kinds and several spot values.
`tests/builtin_patterns.rs` compiles all 74 regex-bearing grammar/weight fields across
all 25 formats through `bc-core::pattern` (including case flags and delimiters).

Lightweight baseline: `cargo run --release -p bc-formats --example formats_baseline`.

## Tokenization, registry and persistence

`Tokenizer` compiles once per format, scans UTF-8 lines with `LineState`, and emits
nonoverlapping byte ranges. Longest lexical match wins; equal lengths use grammar
order. Fixed columns retain their priority even inside lower-priority long tokens.
The coordinator-confirmed rule and its reference-data evidence are documented in
[ADR 004](../../docs/decisions/004-tokenizer-precedence.md); reference parity still
requires the later golden corpus. All 25 formats have hand-authored input/expected
fixtures in `tests/fixtures` (grammar-free formats intentionally emit no tokens).

`FormatRegistry::upsert` now returns `Result`, validates grammar/weight patterns and
appends or replaces in place. `move_to` changes explicit priority. Built-in library
objects stay immutable; same-key user overrides become exportable user definitions.
Resolution uses positive, case-insensitive full-file-name masks only; NameFilter
exclusion/recursive semantics are not introduced here. `FileFormat::masks` stores
individual masks, as returned by XML parsing.

`save_user_dir` / `load_user_dir` persist `user-formats.xml` and `order.json`, including
user overrides and order among built-ins. Missing storage reads as built-ins only;
malformed data/I/O errors are returned, not silently discarded. Files use create-new
staging, sync and atomic rename (the two files are not one atomic transaction).
`user_formats_dir` chooses APPDATA/LOCALAPPDATA on Windows or XDG_CONFIG_HOME/HOME
on other platforms, under `Matcha/formats`; tests use isolated temporary directories.

`FileFormat::source_xml` is serde-defaulted metadata and omitted when None; equality
compares only semantic fields. Existing source markers/defaults/unknown child nodes
survive XML patches; new formats follow [ADR 001](../../docs/decisions/001-stx-write-rule.md).
The built-in XML round-trip test checks both typed/tree equality and line-by-line
content after stripping formatting whitespace, not just successful reparsing.

## TableOptions（Type / Regional）

`model::{TableType, HeaderMode, RegionalSettings, DateOrder}` 承载分隔/固定字符宽度、
连续分隔符、周围空白、自动检测、首行策略与区域设置；宿主可直接绑定格式面板。
新增默认字段不写出 XML，原始树/STX 补丁规则不变，25 个内置格式的语义/树/文本往返全保留。
新属性的名称是本项目格式扩展契约，尚未用参考工具原生保存文件确认互操作；未知节点仍保留。

## Dependency boundary

`bc-core` (persist), `serde`, `serde_json`. Build dependencies: the same three.
No engine, VFS or GUI dependency. Engines depend on this crate for grammar and rules.

## Data notes

| fact | detail |
|---|---|
| source | `data/bcfileformats.xml` = byte-identical copy of the vault's `09_generated/bc5-analysis/bcfileformats.xml` (SHA-256 `bcaf0a29…918c`) |
| counts | 25 formats: `TTextFormat`×19 (BASH COBOL CPP DFM HTML JAVA JS MSWORD OPENOFFICE PAS PDF PERL PYTHON REG RTF SORT SQL VB XML), `TTableFormat`×2 (CSV MSEXCEL), `TPixFormat` PIX, `TMediaFormat` MEDIA, `TVerFormat` EXE, `TXMLFormat` XML |
| grammar items | 112: Basic 33, Delimited 42, List 36, Columns 1 |
| element kinds | Comment 24, Directive 2, EnvironmentVar 1, Identifier 11, Indent 1, Keyword 15, LineNum 2, Number 13, Operator 14, String 29 |
| converters | convertDFM, convertRTF, convertPDF, convertOpenOffice, convertSort, convertMSExcel, convertMSWord |
| defaults | `CaseSensitive` absent ⇒ `true`; `IsRegExp` absent ⇒ `false`; `Escape`/`DlmDelimiters`/`DlmQualifier` are character codes |

## Reference class map (from `vmt_report.txt`)

| this crate | reference classes |
|---|---|
| `FileFormat` / `FormatKind` | `TFileFormat`, `TBasicFileFormat`, `TConvertFileFormat`, `TLineFileFormat`, `TTextFormat`, `TTableFormat`, `TPixFormat`, `TMediaFormat`, `TVerFormat`, `TXMLFormat`, `THexFormat`, `TEditFileFormat` |
| `FormatRegistry` | `TFileFormatManager`, `TFileFormatList`, `TFileFormatsList`, `TFileFormatClassList`, `TFileFormatManagerState` |
| `GrammarItem` / `GrammarMatch` | `TTextGrammarItem`, `TBasicTextGrammarItem`, `TDelimitedTextGrammarItem`, `TListTextGrammarItem`, `TColumnTextGrammarItem`, `TLinesTextGrammarItem`, `TTextGrammar` |
| `grammar::Tokenizer` | `TTextTokenizer`, `TTextColParser`, `TREParser` (all `TTextParser`) |
| `WeightItem` | `TTextWeightItem`, `TTextWeightList` |
| `text_rules::UnimportantText` | `TTextInsigItem`, `TTextInsigList` |
| `text_rules::Replacement` | `TTextReplacementItem`, `TTextReplacementList` |
| `ElementImportance` | `TSyntaxElementSigList`, `TFileElementColorStyleList` |
| settings UI (not here) | `TFileFormatDialog`, `TFileFormatFrame*`, `TTextFormatGrammarFrame`, `TTextGrammarItemDialog`, `TTextInsigItemDialog`, `TTextReplacementItemDialog`, `TTextWeightItemDialog` |

## Open questions

- [Unknown multi-line grammar decision placeholder](../../docs/decisions/002-lines-grammar.md).
- [STX write rule decision placeholder](../../docs/decisions/001-stx-write-rule.md).
- Notes said 22 built-in formats; the resource has 25 (MEDIA/EXE/XML-format extra). All 25
  are embedded; sessions decide which kinds they can use.
- Spec names the fourth match type "RegEx"; data has `Basic` with an `IsRegExp` flag.
  `GrammarMatch::Basic` + `is_regex()` was chosen to stay faithful to the data.
- `DlmDelimiters` is parsed as a comma-separated list of char codes; only single codes
  (`44`, `9`) occur in the built-ins, so multi-delimiter syntax is unverified.
