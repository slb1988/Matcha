use bc_core::persist::parse_xml;
use bc_formats::{
    bcff::{parse_bcfileformats, write_bcfileformats},
    builtin_formats, GrammarMatch,
};

#[test]
fn builtin_read_write_read_preserves_model_tree_and_only_formatting_differs() {
    let source = include_str!("../data/bcfileformats.xml");
    let formats = parse_bcfileformats(source).unwrap();
    let written = write_bcfileformats(&formats).unwrap();
    assert_eq!(parse_bcfileformats(&written).unwrap(), formats);
    assert_eq!(parse_xml(source).unwrap(), parse_xml(&written).unwrap());
    let old: Vec<_> = source
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let new: Vec<_> = written
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    assert_eq!(old.len(), new.len(), "line count outside indentation");
    for (i, (a, b)) in old.iter().zip(&new).enumerate() {
        assert_eq!(a, b, "non-whitespace difference on line {}", i + 1);
    }
}
#[test]
fn new_formats_use_canonical_stx_and_semantic_round_trip() {
    let mut formats = builtin_formats().to_vec();
    for f in &mut formats {
        f.source_xml = None;
    }
    let out = write_bcfileformats(&formats).unwrap();
    assert_eq!(parse_bcfileformats(&out).unwrap(), formats);
    let tree = parse_xml(&out).unwrap();
    let first = &tree.children[0];
    assert!(first.stx);
    assert!(first.child("Description").unwrap().children[0].stx);
    let rule = &first.child("Grammar").unwrap().children[0];
    assert!(!rule.stx);
    assert!(rule.child("ElementStr").unwrap().stx);
    assert!(!first.child("Mask").unwrap().stx);
}
#[test]
fn changed_fields_patch_source_without_losing_unknown_nodes_or_markers() {
    let source = r#"<BCFileFormats><TTextFormat Value="Name"><Future Value="opaque"/><TabAsSpaces Value="False"/><Grammar><_0 Value="TBasicTextGrammarItem"><ElementStr Value="&#2;String"/><Text Value="&#2;old"/><FutureRule Value="keep"/></_0></Grammar><Mask Value="*.old"/></TTextFormat></BCFileFormats>"#;
    let mut formats = parse_bcfileformats(source).unwrap();
    formats[0].name = "Renamed".into();
    formats[0].tab_as_spaces = true;
    formats[0].masks = vec!["*.new".into()];
    if let GrammarMatch::Basic { text, .. } = &mut formats[0].grammar[0].matcher {
        *text = "new".into();
    }
    let out = write_bcfileformats(&formats).unwrap();
    assert_eq!(parse_bcfileformats(&out).unwrap(), formats);
    let tree = parse_xml(&out).unwrap();
    let node = &tree.children[0];
    assert!(!node.stx, "read source name marker must be preserved");
    assert_eq!(node.child_value("Future"), Some("opaque"));
    assert_eq!(node.child_value("TabAsSpaces"), Some("True"));
    let rule = &node.child("Grammar").unwrap().children[0];
    assert!(rule.child("Text").unwrap().stx);
    assert_eq!(rule.child_value("Text"), Some("new"));
    assert_eq!(rule.child_value("FutureRule"), Some("keep"));
}
#[test]
fn source_metadata_defaults_and_is_not_semantic_equality() {
    let original = &builtin_formats()[0];
    let mut copy = original.clone();
    copy.source_xml = None;
    assert_eq!(&copy, original);
    let json = serde_json::to_string(&copy).unwrap();
    assert!(!json.contains("source_xml"));
    let parsed: bc_formats::FileFormat = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, *original);
    assert!(parsed.source_xml.is_none());
}
#[test]
fn unknown_grammar_reports_format_and_class_even_without_element() {
    let xml = r#"<BCFileFormats><TTextFormat Value="Custom"><Grammar><_0 Value="TLinesTextGrammarItem"/></Grammar></TTextFormat></BCFileFormats>"#;
    let error = parse_bcfileformats(xml).unwrap_err().to_string();
    assert!(
        error.contains("Custom") && error.contains("TLinesTextGrammarItem"),
        "{error}"
    );
}
