# bc-formats

File-format registry and syntax grammar model. Converts the reference tool's embedded
`BCFILEFORMATS` resource into Rust data at build time and reads/writes user format
libraries in the same XML dialect.

## Responsibilities

- `model` – `FileFormat`, `FormatKind` (6), `GrammarItem` + `GrammarMatch` (4 match
  types), `ElementKind` (10), `ConvertKind`, `WeightItem`, `TableOptions`.
- `bcff` – `parse_bcfileformats` (**implemented**, shared with `build.rs`),
  `write_bcfileformats` (scaffold).
- `builtin_formats()` – the embedded library: `build.rs` parses `data/bcfileformats.xml`
  and emits JSON into `OUT_DIR`; the library deserialises it once (`OnceLock`).
- `registry` – `FormatRegistry` (lookup by `(kind, name)`, mask resolution, import/export).
- `grammar` – `Tokenizer` skeleton producing `Token`s for highlighting / importance.
- `text_rules` – `UnimportantText`, `Replacement`, `ElementImportance`.

Test: `cargo test -p bc-formats` – parses the XML, compares with the embedded data,
checks 25 formats (19 text, 2 table, 1 picture, 1 media, 1 version, 1 XML), all 4
grammar classes, all 10 element kinds and several spot values.
`tests/builtin_patterns.rs` compiles all 74 regex-bearing grammar/weight fields across
all 25 formats through `bc-core::pattern` (including case flags and delimiters).

Lightweight baseline: `cargo run --release -p bc-formats --example formats_baseline`.

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
