use bc_core::persist::{parse_xml, write_xml, Node, XmlHeader};
#[test]
fn source_markers_unknown_nodes_and_escaped_values_round_trip() {
    let source = r#"<Root Value="&#2;名 &amp; &quot; &lt; &gt; '"><Unknown Value="&#2;"/><Text Value="plain"/><Empty/></Root>"#;
    let node = parse_xml(source).unwrap();
    let out = write_xml(&node, None);
    assert_eq!(parse_xml(&out).unwrap(), node);
    assert!(out.contains("&#2;名"));
    assert!(out.contains(r#"<Text Value="plain"/>"#));
    let header = XmlHeader {
        version: 2,
        min_version: 1,
        comment: Some("Matcha -- formats".into()),
    };
    let text = write_xml(&node, Some(&header));
    assert!(text.contains(r#"Version="2" MinVersion="1""#));
    assert!(text.contains("<!-- Matcha - - formats -->"));
    assert_eq!(parse_xml(&text).unwrap(), node);
}
#[test]
fn unicode_xml_names_preserve_combining_marks() {
    let original = parse_xml("<根><e\u{301} Value=\"a\"/></根>").unwrap();
    assert_eq!(parse_xml(&write_xml(&original, None)).unwrap(), original);
}

#[test]
fn empty_value_and_attribute_whitespace_are_not_lost() {
    let mut root = Node::new("Root");
    root.children.push(Node {
        name: "Text".into(),
        value: Some(" a\tb\nc\r ".into()),
        stx: false,
        children: vec![],
    });
    root.children.push(Node {
        name: "Blank".into(),
        value: Some(String::new()),
        stx: true,
        children: vec![],
    });
    assert_eq!(parse_xml(&write_xml(&root, None)).unwrap(), root);
}
