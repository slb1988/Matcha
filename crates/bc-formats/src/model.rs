//! Data model of a file format – BC `TFileFormat` hierarchy.
//!
//! ```text
//! TFileFormat (TSsPersistentParentEx)
//! ├── TBasicFileFormat      → TMediaFormat, TVerFormat
//! ├── TConvertFileFormat    → TPixFormat
//! └── TLineFileFormat       → TTextFormat, TTableFormat   (+ THexFormat/TEditFileFormat, TXMLFormat)
//! TTextGrammarItem (TSsPersistent)
//! ├── TBasicTextGrammarItem        (Text + IsRegExp)        → GrammarMatch::Basic
//! │   ├── TDelimitedTextGrammarItem (Text/StopText/StopEOL/Escape) → GrammarMatch::Delimited
//! │   └── TLinesTextGrammarItem     (not used by built-ins)
//! ├── TListTextGrammarItem         (Items)                 → GrammarMatch::List
//! └── TColumnTextGrammarItem       (Col/StopCol)           → GrammarMatch::Columns
//! TTextWeightItem / TTextWeightList                       → WeightItem
//! ```
//!
//! This file is compiled both into the library and into `build.rs` (via `#[path]`), so it
//! must only depend on `serde` and `std`.

use serde::{Deserialize, Serialize};

/// Which kind of session a format drives – derived from the XML element/class name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FormatKind {
    /// `TTextFormat` – Text Compare / Merge / Edit.
    Text,
    /// `TTableFormat` – Table Compare (CSV, Excel).
    Table,
    /// `TPixFormat` – Picture Compare.
    Picture,
    /// `TMediaFormat` – Media (tag) Compare.
    Media,
    /// `TVerFormat` – Version Compare (PE version resources).
    Version,
    /// `TXMLFormat` – XML-aware text compare (sorted/pretty-printed XML).
    Xml,
}

impl FormatKind {
    /// Delphi class name used as the XML element name.
    pub fn bc_class(self) -> &'static str {
        match self {
            FormatKind::Text => "TTextFormat",
            FormatKind::Table => "TTableFormat",
            FormatKind::Picture => "TPixFormat",
            FormatKind::Media => "TMediaFormat",
            FormatKind::Version => "TVerFormat",
            FormatKind::Xml => "TXMLFormat",
        }
    }

    /// Inverse of [`Self::bc_class`].
    pub fn from_bc_class(name: &str) -> Option<Self> {
        Some(match name {
            "TTextFormat" => FormatKind::Text,
            "TTableFormat" => FormatKind::Table,
            "TPixFormat" => FormatKind::Picture,
            "TMediaFormat" => FormatKind::Media,
            "TVerFormat" => FormatKind::Version,
            "TXMLFormat" => FormatKind::Xml,
            _ => return None,
        })
    }
}

/// Syntax element category – the `ElementStr` of a grammar item. Exactly the ten
/// categories BC uses for highlighting and importance (`TSyntaxElementSigList`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ElementKind {
    Comment,
    Directive,
    EnvironmentVar,
    Identifier,
    Indent,
    Keyword,
    LineNum,
    Number,
    Operator,
    String,
}

impl ElementKind {
    /// All kinds in BC's display order.
    pub const ALL: [ElementKind; 10] = [
        ElementKind::Comment,
        ElementKind::Directive,
        ElementKind::EnvironmentVar,
        ElementKind::Identifier,
        ElementKind::Indent,
        ElementKind::Keyword,
        ElementKind::LineNum,
        ElementKind::Number,
        ElementKind::Operator,
        ElementKind::String,
    ];

    /// The `ElementStr` spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            ElementKind::Comment => "Comment",
            ElementKind::Directive => "Directive",
            ElementKind::EnvironmentVar => "EnvironmentVar",
            ElementKind::Identifier => "Identifier",
            ElementKind::Indent => "Indent",
            ElementKind::Keyword => "Keyword",
            ElementKind::LineNum => "LineNum",
            ElementKind::Number => "Number",
            ElementKind::Operator => "Operator",
            ElementKind::String => "String",
        }
    }

    /// Parse an `ElementStr` value.
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }

    /// Name of the `<…CaseSig>` property that stores per-element case significance.
    pub fn case_sig_property(self) -> &'static str {
        match self {
            ElementKind::Comment => "CommentCaseSig",
            ElementKind::Directive => "DirectiveCaseSig",
            ElementKind::EnvironmentVar => "EnvironmentVarCaseSig",
            ElementKind::Identifier => "IdentifierCaseSig",
            ElementKind::Indent => "IndentCaseSig",
            ElementKind::Keyword => "KeywordCaseSig",
            ElementKind::LineNum => "LineNumCaseSig",
            ElementKind::Number => "NumberCaseSig",
            ElementKind::Operator => "OperatorCaseSig",
            ElementKind::String => "StringCaseSig",
        }
    }
}

/// How a grammar item recognises text. Four matching types
/// (`TBasicTextGrammarItem`, `TDelimitedTextGrammarItem`, `TListTextGrammarItem`,
/// `TColumnTextGrammarItem`). The spec's "Regular Expression" type is
/// [`GrammarMatch::Basic`] with `is_regexp == true`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrammarMatch {
    /// Single token matched by literal text or a regular expression.
    Basic { text: String, is_regexp: bool },
    /// Span from `start` to `stop` (or end of line), with optional escape character.
    Delimited {
        start: String,
        is_regexp: bool,
        stop: Option<String>,
        stop_eol: bool,
        /// Escape character code (`<Escape Value="92"/>` = backslash).
        escape: Option<char>,
    },
    /// Any of a list of words/tokens (keywords, operators). `is_regexp` applies to every item.
    List { items: Vec<String>, is_regexp: bool },
    /// Fixed column range (1-based, inclusive) – COBOL sequence numbers etc.
    Columns { start_col: u32, stop_col: u32 },
}

impl GrammarMatch {
    /// Delphi class of this match type.
    pub fn bc_class(&self) -> &'static str {
        match self {
            GrammarMatch::Basic { .. } => "TBasicTextGrammarItem",
            GrammarMatch::Delimited { .. } => "TDelimitedTextGrammarItem",
            GrammarMatch::List { .. } => "TListTextGrammarItem",
            GrammarMatch::Columns { .. } => "TColumnTextGrammarItem",
        }
    }

    /// Whether the item is pattern based (spec: "Regular Expression" type).
    pub fn is_regex(&self) -> bool {
        matches!(
            self,
            GrammarMatch::Basic {
                is_regexp: true,
                ..
            } | GrammarMatch::Delimited {
                is_regexp: true,
                ..
            } | GrammarMatch::List {
                is_regexp: true,
                ..
            }
        )
    }
}

/// One grammar rule – BC `TTextGrammarItem`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrammarItem {
    /// Element category assigned to matched text.
    pub element: ElementKind,
    /// `CaseSensitive` (defaults to `true` when absent in XML).
    pub case_sensitive: bool,
    pub matcher: GrammarMatch,
}

/// Alignment weight priority `wp1`…`wpN` (`TTextWeightItem.Priority`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WeightPriority(pub u8);

/// Line weight rule – BC `TTextWeightItem`: lines matching `find_text` anchor Smart alignment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeightItem {
    pub find_text: String,
    pub is_regexp: bool,
    pub priority: WeightPriority,
}

/// External/internal conversion applied before comparing (`TConvertFileFormat` / `ConvertKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConvertKind {
    /// `convertDFM` – binary Delphi form → text.
    Dfm,
    /// `convertRTF`
    Rtf,
    /// `convertPDF` – external `PdfToText` pipeline.
    Pdf,
    /// `convertOpenOffice`
    OpenOffice,
    /// `convertSort` – sort lines before comparing.
    Sort,
    /// `convertMSExcel`
    MsExcel,
    /// `convertMSWord`
    MsWord,
}

impl ConvertKind {
    /// The `ConvertKind` enumeration literal in XML.
    pub fn bc_name(self) -> &'static str {
        match self {
            ConvertKind::Dfm => "convertDFM",
            ConvertKind::Rtf => "convertRTF",
            ConvertKind::Pdf => "convertPDF",
            ConvertKind::OpenOffice => "convertOpenOffice",
            ConvertKind::Sort => "convertSort",
            ConvertKind::MsExcel => "convertMSExcel",
            ConvertKind::MsWord => "convertMSWord",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "convertDFM" => ConvertKind::Dfm,
            "convertRTF" => ConvertKind::Rtf,
            "convertPDF" => ConvertKind::Pdf,
            "convertOpenOffice" => ConvertKind::OpenOffice,
            "convertSort" => ConvertKind::Sort,
            "convertMSExcel" => ConvertKind::MsExcel,
            "convertMSWord" => ConvertKind::MsWord,
            _ => return None,
        })
    }
}

/// Delimited-table settings of a `TTableFormat` (`DlmDelimiters` / `DlmQualifier`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableOptions {
    /// Field delimiter characters (`,` for CSV, tab for Excel export).
    pub delimiters: Vec<char>,
    /// Text qualifier (`"`).
    pub qualifier: Option<char>,
}

/// A file format definition – BC `TFileFormat` and subclasses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileFormat {
    /// Format name (`Value` of the root element). **Not unique on its own**: the built-in
    /// library has both a `TXMLFormat` and a `TTextFormat` named `XML`; key by `(kind, name)`.
    pub name: String,
    pub kind: FormatKind,
    /// `<Description>` string list.
    pub description: Vec<String>,
    /// File masks split from `<Mask>` on `;` (e.g. `*.c`, `*.cpp`). Empty for `SORT`.
    pub masks: Vec<String>,
    /// `<Grammar>` items in priority order (earlier wins on overlap).
    pub grammar: Vec<GrammarItem>,
    pub tab_size: Option<u32>,
    pub tab_as_spaces: bool,
    pub convert: Option<ConvertKind>,
    /// Per-element case significance overrides (`<KeywordCaseSig Value="False"/>` …).
    pub case_significance: Vec<(ElementKind, bool)>,
    /// `<Weights>` – alignment anchors.
    pub weights: Vec<WeightItem>,
    /// Present for [`FormatKind::Table`].
    pub table: Option<TableOptions>,
    /// Canonical source tree for minimally invasive XML write-back, not semantics.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_xml: Option<String>,
}

impl PartialEq for FileFormat {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.kind == other.kind
            && self.description == other.description
            && self.masks == other.masks
            && self.grammar == other.grammar
            && self.tab_size == other.tab_size
            && self.tab_as_spaces == other.tab_as_spaces
            && self.convert == other.convert
            && self.case_significance == other.case_significance
            && self.weights == other.weights
            && self.table == other.table
    }
}
impl Eq for FileFormat {}

impl FileFormat {
    /// Case significance for an element kind (default `true`).
    pub fn is_case_significant(&self, kind: ElementKind) -> bool {
        self.case_significance
            .iter()
            .find(|(k, _)| *k == kind)
            .map(|(_, v)| *v)
            .unwrap_or(true)
    }
}
