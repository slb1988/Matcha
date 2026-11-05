use bc_core::pattern::{glob_to_pattern, split_mask_list, Pattern};

#[test]
fn search_offsets_captures_replacement_and_case() {
    let pattern = Pattern::new(r"(?P<word>[a-z]+)(\d+)?", false).unwrap();
    assert!(!pattern.case_sensitive());
    assert_eq!(pattern.source(), r"(?P<word>[a-z]+)(\d+)?");
    assert!(pattern.find_at("中 ABC12 def", 1).is_none());
    let m = pattern.find_at("中 ABC12 def", 3).unwrap();
    assert_eq!(m.span, 4..9);
    assert_eq!(m.captures, vec![Some(4..9), Some(4..7), Some(7..9)]);
    assert!(pattern.match_at(" ABC", 0).is_none());
    assert!(pattern.match_at(" ABC", 1).is_some());
    assert_eq!(pattern.find_at("def", 0).unwrap().captures[2], None);
    assert_eq!(pattern.replace_all("a12 B34", "${word}:$2"), "a:12 B:34");
    assert!(pattern.find_at("abc", usize::MAX).is_none());
    assert!(Pattern::new("a", true).unwrap().find_at("A", 0).is_none());
    assert_eq!(Pattern::new("", true).unwrap().replace_all("é", "-"), "-é-");
}

#[test]
fn directional_word_boundaries_and_escaped_literals() {
    let word = Pattern::new(r"\<cat\>", true).unwrap();
    assert!(word.find_at("scatter", 0).is_none());
    assert_eq!(word.find_at(" cat catfish", 0).unwrap().span, 1..4);
    assert!(Pattern::new(r"\>cat", true)
        .unwrap()
        .find_at("cat", 0)
        .is_none());
    assert!(Pattern::new(r"cat\<", true)
        .unwrap()
        .find_at("cat", 0)
        .is_none());
    assert!(
        word.find_at("scat", 1).is_none(),
        "search must not erase prefix context"
    );
    assert!(Pattern::new(r"^cat", true)
        .unwrap()
        .find_at("xcat", 1)
        .is_none());
    assert!(Pattern::new(r"\\<word\\>", true)
        .unwrap()
        .find_at(r"\<word\>", 0)
        .is_some());
    assert!(Pattern::new(r"[\<\>]", true)
        .unwrap()
        .find_at("<", 0)
        .is_some());
    assert!(Pattern::new(r"\<中文\>", true)
        .unwrap()
        .find_at(" 中文 ", 0)
        .is_some());
    for invalid in [r"(a)\1", "(?<=a)b", "(", "\\"] {
        let error = Pattern::new(invalid, true).unwrap_err().to_string();
        assert!(error.contains("pattern"));
    }
}

#[test]
fn mask_list_metadata_and_individual_match_table() {
    let masks = split_mask_list(r"*.c;*.h;-*.bak;obj\");
    assert_eq!(masks.len(), 4);
    assert_eq!(masks[2].pattern, "*.bak");
    assert!(masks[2].exclude);
    assert_eq!(masks[3].pattern, "obj");
    assert!(masks[3].folder);
    let compiled: Vec<_> = masks
        .iter()
        .map(|m| {
            let mask = if m.folder {
                format!("{}\\", m.pattern)
            } else {
                m.pattern.clone()
            };
            glob_to_pattern(&mask, false).unwrap()
        })
        .collect();
    for (name, expected) in [
        ("main.c", [true, false, false, false]),
        ("MAIN.H", [false, true, false, false]),
        ("main.c.bak", [false, false, true, false]),
        ("obj\\", [false, false, false, true]),
        ("OBJ/", [false, false, false, true]),
        ("obj", [false, false, false, false]),
        ("obj/main.c", [false, false, false, false]),
        ("main.cpp", [false, false, false, false]),
    ] {
        for (pattern, yes) in compiled.iter().zip(expected) {
            assert_eq!(pattern.find_at(name, 0).is_some(), yes, "{name}");
        }
    }
    assert!(glob_to_pattern(r"*.c;*.h;-*.bak;obj\", false).is_err());
    assert!(glob_to_pattern("-*.bak", false).is_err());
    assert!(split_mask_list(" ; ; - ; ").is_empty());
    assert!(glob_to_pattern("A?.[x]", true)
        .unwrap()
        .find_at("A中.[x]", 0)
        .is_some());
    assert!(glob_to_pattern("A?.[x]", true)
        .unwrap()
        .find_at("a中.[x]", 0)
        .is_none());
    assert!(glob_to_pattern("*", true)
        .unwrap()
        .find_at("a/b", 0)
        .is_none());
}
