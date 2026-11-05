//! `BCFileFormats` XML ↔ [`FileFormat`] mapping.
//!
//! Reads the embedded `BCFILEFORMATS` resource and user `BCFileFormats.xml` /
//! `.bcpkg` payloads (same schema). Shared between the library and `build.rs`.
//! The reader follows the "ignore unknown elements" compatibility rule: unknown
//! *format classes* are skipped, unknown *properties* are ignored, but an unknown
//! grammar item class is an error because dropping it would silently change highlighting.

use crate::model::{
    ConvertKind, ElementKind, FileFormat, FormatKind, GrammarItem, GrammarMatch, TableOptions,
    WeightItem, WeightPriority,
};
use bc_core::persist::{self, Node, PersistError};

/// Errors from reading or writing format XML.
#[derive(Debug)]
pub enum FormatError {
    Persist(PersistError),
    UnknownGrammarClass(String),
    UnknownElement(String),
    Missing(String),
    BadValue(String),
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormatError::Persist(e) => write!(f, "{e}"),
            FormatError::UnknownGrammarClass(c) => write!(f, "unknown grammar item class {c}"),
            FormatError::UnknownElement(e) => write!(f, "unknown ElementStr {e:?}"),
            FormatError::Missing(m) => write!(f, "missing {m}"),
            FormatError::BadValue(v) => write!(f, "bad value {v}"),
        }
    }
}

impl std::error::Error for FormatError {}

impl From<PersistError> for FormatError {
    fn from(e: PersistError) -> Self {
        FormatError::Persist(e)
    }
}

/// Root element name of the format library document.
pub const ROOT_ELEMENT: &str = "BCFileFormats";

/// Parse a whole `BCFileFormats` document.
pub fn parse_bcfileformats(xml: &str) -> Result<Vec<FileFormat>, FormatError> {
    let root = persist::parse_xml(xml)?;
    if root.name != ROOT_ELEMENT {
        return Err(FormatError::Missing(format!("<{ROOT_ELEMENT}> root (found <{}>)", root.name)));
    }
    let mut formats = Vec::new();
    for child in &root.children {
        if let Some(kind) = FormatKind::from_bc_class(&child.name) {
            formats.push(format_from_node(kind, child)?);
        }
        // Unknown classes (e.g. THexFormat in user files) are skipped for read-compatibility.
    }
    Ok(formats)
}

/// Convert one `<TTextFormat Value="…">` style node.
pub fn format_from_node(kind: FormatKind, node: &Node) -> Result<FileFormat, FormatError> {
    let name = node
        .value_str()
        .ok_or_else(|| FormatError::Missing(format!("Value on <{}>", node.name)))?
        .to_string();

    let description = node.child("Description").map(Node::item_values).unwrap_or_default();

    let masks = node
        .child_value("Mask")
        .map(|m| m.split(';').filter(|s| !s.is_empty()).map(str::to_string).collect())
        .unwrap_or_default();

    let mut grammar = Vec::new();
    if let Some(g) = node.child("Grammar") {
        for item in g.items() {
            grammar.push(grammar_item_from_node(item)?);
        }
    }

    let tab_size = node.child_int("TabSize")?.map(|v| v as u32);
    let tab_as_spaces = node.child_bool("TabAsSpaces")?.unwrap_or(false);

    let convert = match node.child_value("ConvertKind") {
        None => None,
        Some(v) => Some(
            ConvertKind::parse(v).ok_or_else(|| FormatError::BadValue(format!("ConvertKind {v:?}")))?,
        ),
    };

    let mut case_significance = Vec::new();
    for kind in ElementKind::ALL {
        if let Some(v) = node.child_bool(kind.case_sig_property())? {
            case_significance.push((kind, v));
        }
    }

    let mut weights = Vec::new();
    if let Some(w) = node.child("Weights") {
        for item in w.items() {
            weights.push(weight_from_node(item)?);
        }
    }

    let table = if kind == FormatKind::Table {
        Some(TableOptions {
            delimiters: node
                .child_value("DlmDelimiters")
                .map(parse_char_codes)
                .transpose()?
                .unwrap_or_default(),
            qualifier: node
                .child_value("DlmQualifier")
                .map(parse_char_code)
                .transpose()?,
        })
    } else {
        None
    };

    Ok(FileFormat {
        name,
        kind,
        description,
        masks,
        grammar,
        tab_size,
        tab_as_spaces,
        convert,
        case_significance,
        weights,
        table,
    })
}

/// Convert one `<_N Value="T…TextGrammarItem">` node.
pub fn grammar_item_from_node(node: &Node) -> Result<GrammarItem, FormatError> {
    let class = node.value_str().unwrap_or("");
    let element_str = node
        .child_value("ElementStr")
        .ok_or_else(|| FormatError::Missing("ElementStr".into()))?;
    let element = ElementKind::parse(element_str)
        .ok_or_else(|| FormatError::UnknownElement(element_str.to_string()))?;
    let case_sensitive = node.child_bool("CaseSensitive")?.unwrap_or(true);
    let is_regexp = node.child_bool("IsRegExp")?.unwrap_or(false);
    let text = || node.child_value("Text").unwrap_or("").to_string();

    let matcher = match class {
        "TBasicTextGrammarItem" => GrammarMatch::Basic { text: text(), is_regexp },
        "TDelimitedTextGrammarItem" => GrammarMatch::Delimited {
            start: text(),
            is_regexp,
            stop: node.child_value("StopText").map(str::to_string),
            stop_eol: node.child_bool("StopEOL")?.unwrap_or(false),
            escape: node.child_value("Escape").map(parse_char_code).transpose()?,
        },
        "TListTextGrammarItem" => GrammarMatch::List {
            items: node.child("Items").map(Node::item_values).unwrap_or_default(),
            is_regexp,
        },
        "TColumnTextGrammarItem" => GrammarMatch::Columns {
            start_col: node.child_int("Col")?.unwrap_or(1) as u32,
            stop_col: node.child_int("StopCol")?.unwrap_or(0) as u32,
        },
        other => return Err(FormatError::UnknownGrammarClass(other.to_string())),
    };

    Ok(GrammarItem { element, case_sensitive, matcher })
}

fn weight_from_node(node: &Node) -> Result<WeightItem, FormatError> {
    let priority = node.child_value("Priority").unwrap_or("wp1");
    let level = priority
        .strip_prefix("wp")
        .and_then(|n| n.parse::<u8>().ok())
        .ok_or_else(|| FormatError::BadValue(format!("Priority {priority:?}")))?;
    Ok(WeightItem {
        find_text: node.child_value("FindText").unwrap_or("").to_string(),
        is_regexp: node.child_bool("IsRegExp")?.unwrap_or(false),
        priority: WeightPriority(level),
    })
}

fn parse_char_code(s: &str) -> Result<char, FormatError> {
    s.trim()
        .parse::<u32>()
        .ok()
        .and_then(char::from_u32)
        .ok_or_else(|| FormatError::BadValue(format!("char code {s:?}")))
}

fn parse_char_codes(s: &str) -> Result<Vec<char>, FormatError> {
    s.split(',').filter(|p| !p.trim().is_empty()).map(parse_char_code).collect()
}

/// Serialise formats back to a `BCFileFormats` document (write-compatibility target,
/// spec §5). Depends on `bc_core::persist::write_xml`.
pub fn write_bcfileformats(formats: &[FileFormat]) -> Result<String, FormatError> {
    todo!("bc-formats::bcff: build Node tree and call bc_core::persist::write_xml")
}
