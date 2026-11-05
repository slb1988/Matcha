//! # bc-formats
//!
//! File format registry and syntax grammar model (BC `TFileFormat*`, `TTextGrammar*`,
//! `TTextInsigList`, `TTextReplacementList`).
//!
//! * [`model`] – data model (`FileFormat`, `GrammarItem`, `ElementKind`, …).
//! * [`bcff`] – `BCFileFormats` XML reader (implemented) / writer (scaffold).
//! * [`builtin_formats`] – the built-in library converted at build time from
//!   `data/bcfileformats.xml` (`BCFILEFORMATS` resource) and embedded as JSON.
//! * [`registry`] – lookup by mask / name (`TFileFormatManager`).
//! * [`grammar`] – tokenizer skeleton (`TTextTokenizer`, `TTextColParser`, `TREParser`).
//! * [`text_rules`] – unimportant-text and replacement rules.
#![allow(unused_variables)] // scaffold: bodies are `todo!()`; remove once implemented
#![forbid(unsafe_code)]

pub mod bcff;
pub mod grammar;
pub mod model;
pub mod registry;
pub mod text_rules;

pub use model::{
    ConvertKind, ElementKind, FileFormat, FormatKind, GrammarItem, GrammarMatch, TableOptions,
    WeightItem, WeightPriority,
};

use std::sync::OnceLock;

/// JSON produced by `build.rs` from `data/bcfileformats.xml`.
const BUILTIN_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/builtin_formats.json"));

/// The built-in format library (25 entries in the reference resource: 19 text, 2 table,
/// 1 picture, 1 media, 1 version, 1 XML). Deserialised once on first use.
pub fn builtin_formats() -> &'static [FileFormat] {
    static CACHE: OnceLock<Vec<FileFormat>> = OnceLock::new();
    CACHE.get_or_init(|| {
        serde_json::from_str(BUILTIN_JSON).expect("embedded built-in formats are valid JSON")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// The single real unit test of the scaffold: the XML parser round-trips the
    /// reference resource and the embedded data matches it.
    #[test]
    fn builtin_library_parses_and_matches_embedded_data() {
        let xml = include_str!("../data/bcfileformats.xml");
        let parsed = bcff::parse_bcfileformats(xml).expect("parse BCFILEFORMATS");
        let embedded = builtin_formats();

        // Build-time conversion must equal a fresh parse.
        assert_eq!(parsed.len(), 25, "reference resource holds 25 top-level formats");
        assert_eq!(&parsed[..], embedded);

        // Count by kind: 19 text + 2 table + 1 picture (documented 22) + media/version/xml.
        let mut by_kind: BTreeMap<&str, usize> = BTreeMap::new();
        for f in embedded {
            *by_kind.entry(f.kind.bc_class()).or_default() += 1;
        }
        assert_eq!(by_kind["TTextFormat"], 19);
        assert_eq!(by_kind["TTableFormat"], 2);
        assert_eq!(by_kind["TPixFormat"], 1);
        assert_eq!(by_kind["TMediaFormat"], 1);
        assert_eq!(by_kind["TVerFormat"], 1);
        assert_eq!(by_kind["TXMLFormat"], 1);

        // The 22 formats listed in the analysis notes are all present with the right kind.
        let expect = |name: &str, kind: FormatKind| {
            assert!(
                embedded.iter().any(|f| f.name == name && f.kind == kind),
                "missing built-in {name} ({kind:?})"
            );
        };
        for name in [
            "BASH", "COBOL", "CPP", "DFM", "HTML", "JAVA", "JS", "MSWORD", "OPENOFFICE", "PAS",
            "PDF", "PERL", "PYTHON", "REG", "RTF", "SORT", "SQL", "VB", "XML",
        ] {
            expect(name, FormatKind::Text);
        }
        expect("CSV", FormatKind::Table);
        expect("MSEXCEL", FormatKind::Table);
        expect("PIX", FormatKind::Picture);

        // All four grammar item classes and all ten element kinds occur.
        let mut classes: BTreeMap<&str, usize> = BTreeMap::new();
        let mut elements: BTreeMap<&str, usize> = BTreeMap::new();
        for item in embedded.iter().flat_map(|f| f.grammar.iter()) {
            *classes.entry(item.matcher.bc_class()).or_default() += 1;
            *elements.entry(item.element.as_str()).or_default() += 1;
        }
        assert_eq!(classes["TBasicTextGrammarItem"], 33);
        assert_eq!(classes["TDelimitedTextGrammarItem"], 42);
        assert_eq!(classes["TListTextGrammarItem"], 36);
        assert_eq!(classes["TColumnTextGrammarItem"], 1);
        assert_eq!(elements.len(), 10);
        assert_eq!(elements["String"], 29);

        // Spot checks against the raw XML.
        let bash = embedded.iter().find(|f| f.name == "BASH").unwrap();
        assert_eq!(bash.masks, vec!["*.sh"]);
        assert_eq!(bash.tab_size, Some(4));
        assert!(matches!(
            &bash.grammar[5].matcher,
            GrammarMatch::Delimited { start, stop: Some(stop), escape: Some('\\'), stop_eol: false, .. }
                if start == "$\"" && stop == "\""
        ));
        assert!(!bash.grammar[1].case_sensitive, "BASH Identifier is CaseSensitive=False");
        assert!(bash.grammar[1].matcher.is_regex());

        let cobol = embedded.iter().find(|f| f.name == "COBOL").unwrap();
        assert!(cobol
            .grammar
            .iter()
            .any(|g| g.matcher == GrammarMatch::Columns { start_col: 73, stop_col: 80 }));

        let dfm = embedded.iter().find(|f| f.name == "DFM").unwrap();
        assert_eq!(dfm.convert, Some(ConvertKind::Dfm));
        assert!(dfm.tab_as_spaces);
        assert!(!dfm.is_case_significant(ElementKind::Keyword));
        assert!(dfm.is_case_significant(ElementKind::String));
        assert_eq!(dfm.weights[0].priority, WeightPriority(4));
        assert_eq!(dfm.weights[0].find_text, "^\\s*object");

        let csv = embedded.iter().find(|f| f.name == "CSV").unwrap();
        let table = csv.table.as_ref().expect("CSV has table options");
        assert_eq!(table.delimiters, vec![',']);
        assert_eq!(table.qualifier, Some('"'));

        let sort = embedded.iter().find(|f| f.name == "SORT").unwrap();
        assert!(sort.masks.is_empty());
        assert_eq!(sort.convert, Some(ConvertKind::Sort));
    }
}
