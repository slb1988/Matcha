use bc_core::Side;
use bc_engine_text::{
    search::{find_next, replace_all, FindOptions},
    TextContent, TextMap,
};
fn content(s: &str) -> TextContent {
    TextContent::from_bytes(s.as_bytes().to_vec(), None).unwrap()
}
#[test]
fn map_supplies_difference_ranges_and_replacements_undo_as_one_step() {
    let mut map = TextMap::compare(
        content("TODO same\nTODO old\nTODO tail"),
        content("TODO same\nTODO changed\nTODO tail"),
        Default::default(),
    )
    .unwrap();
    let options = FindOptions {
        pattern: "TODO".into(),
        differences_only: true,
        ..Default::default()
    };
    assert_eq!(
        map.find_next(Side::Right, (0, 0), &options)
            .unwrap()
            .unwrap()
            .line,
        1
    );
    let original = map.right.to_bytes().unwrap();
    assert_eq!(map.replace_all(Side::Right, &options, "DONE").unwrap(), 1);
    map.right.edits.undo();
    assert_eq!(map.right.to_bytes().unwrap(), original);
    assert!(!map.right.edits.can_undo());
    assert!(find_next(&map.left, Side::Left, (0, 0), &options, None).is_err());
}
#[test]
fn case_whole_word_backwards_regex_and_multiline_replacement() {
    let mut c = content("Cat catfish CAT\r\ncat\r\nlast");
    let options = FindOptions {
        pattern: "cat".into(),
        whole_words: true,
        ..Default::default()
    };
    assert_eq!(
        find_next(&c, Side::Left, (0, 0), &options, None)
            .unwrap()
            .unwrap()
            .range,
        0..3
    );
    let backward = FindOptions {
        backwards: true,
        ..options.clone()
    };
    assert_eq!(
        find_next(&c, Side::Left, (0, 15), &backward, None)
            .unwrap()
            .unwrap()
            .range,
        12..15
    );
    let original = c.to_bytes().unwrap();
    assert_eq!(replace_all(&mut c, &options, "dog\nnew", None).unwrap(), 3);
    assert!(String::from_utf8(c.to_bytes().unwrap())
        .unwrap()
        .contains("dog\r\nnew"));
    c.edits.undo();
    assert_eq!(c.to_bytes().unwrap(), original);
    let regex = FindOptions {
        pattern: "(cat)".into(),
        is_regexp: true,
        ..Default::default()
    };
    replace_all(&mut c, &regex, "<$1>", None).unwrap();
    assert!(String::from_utf8(c.to_bytes().unwrap())
        .unwrap()
        .contains("<Cat>"));
    c.read_only = true;
    assert!(replace_all(&mut c, &regex, "x", None).is_err());
}
#[test]
fn zero_width_regex_replacements_and_unicode_boundaries_progress() {
    let mut c = content("中x");
    let options = FindOptions {
        pattern: "".into(),
        is_regexp: true,
        ..Default::default()
    };
    assert_eq!(replace_all(&mut c, &options, "-", None).unwrap(), 3);
    assert_eq!(c.to_bytes().unwrap(), "-中-x-".as_bytes());
    c.edits.undo();
    assert!(find_next(&c, Side::Left, (0, 1), &options, None).is_err());
}
