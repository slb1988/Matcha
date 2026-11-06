use bc_engine_text::{conflict::parse_conflict_text, TextContent, TextMap};
#[test]
fn exported_packets_recover_boundaries_lines_states_and_escaped_text() {
    let left = TextContent::from_bytes(b"before\nold\t\\text\nafter\nend".to_vec(), None).unwrap();
    let right = TextContent::from_bytes(
        "before\n新内容\nafter\nchanged end".as_bytes().to_vec(),
        None,
    )
    .unwrap();
    let mut map = TextMap::compare(left, right, Default::default()).unwrap();
    map.rules.ignore_unimportant = true;
    assert!(!map.is_stale());
    let sections = map.sections();
    let ids: Vec<_> = (0..sections.len()).collect();
    let exported = map.to_conflict_text(&ids, 1).unwrap();
    let parsed = parse_conflict_text(&exported).unwrap();
    assert_eq!(parsed.len(), sections.len());
    for (expected, actual) in sections.iter().zip(&parsed) {
        assert_eq!(actual.rows, expected.rows);
        assert_eq!(actual.importance, expected.importance);
        assert!(actual.lines.iter().any(|r| r.row == actual.rows.start));
    }
    assert!(parsed
        .iter()
        .flat_map(|s| &s.lines)
        .any(|r| r.left_text == "old\t\\text"));
    assert!(parse_conflict_text(&exported.replace("ENDSECTION", "BAD")).is_err());
    assert!(parse_conflict_text("MATCHA-CONFLICT\t2\n").is_err());
}
