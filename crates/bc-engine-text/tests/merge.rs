//! Merge model and automatic merge (spec `text-merge-engine`).
use bc_core::{encoding::Encoding, Side};
use bc_engine_text::{
    importance::Importance,
    merge::{auto_merge, MergeChoice, MergeLabels, MergeState},
    rules::{Favor, TextMergeRules, TextRules},
    TextContent, TextMap,
};

fn base(n: usize) -> Vec<String> {
    (1..=n).map(|i| format!("line{i:02}")).collect()
}
/// `base` with 1-based replacements.
fn edit(base: &[String], changes: &[(usize, &str)]) -> Vec<String> {
    let mut out = base.to_vec();
    for (line, text) in changes {
        out[line - 1] = text.to_string();
    }
    out
}
/// `base` with `lines` inserted after 1-based line `after`.
fn insert(base: &[String], after: usize, lines: &[&str]) -> Vec<String> {
    let mut out = base.to_vec();
    for (i, line) in lines.iter().enumerate() {
        out.insert(after + i, line.to_string());
    }
    out
}
fn bytes(lines: &[String]) -> Vec<u8> {
    lines.iter().flat_map(|l| format!("{l}\n").into_bytes()).collect()
}
fn map_of(left: Vec<u8>, right: Vec<u8>, center: Vec<u8>, rules: &TextRules) -> TextMap {
    let content = |b: Vec<u8>| TextContent::from_bytes(b, None).unwrap();
    TextMap::merge(content(left), content(right), content(center), rules.clone()).unwrap()
}
fn map(left: &[String], right: &[String], center: &[String]) -> TextMap {
    map_of(bytes(left), bytes(right), bytes(center), &TextRules::default())
}
fn state(map: &TextMap) -> MergeState {
    MergeState::classify(map, &TextMergeRules::default()).unwrap()
}
fn labels() -> MergeLabels {
    MergeLabels {
        left: "theirs.txt".into(),
        right: "yours.txt".into(),
    }
}
fn output(state: &MergeState, map: &TextMap) -> String {
    String::from_utf8(state.render_output(map, &labels()).unwrap()).unwrap()
}
fn text(lines: &[String]) -> String {
    String::from_utf8(bytes(lines)).unwrap()
}
fn conflict_rows(state: &MergeState) -> Vec<std::ops::Range<usize>> {
    state.conflicts().into_iter().map(|c| c.rows).collect()
}

// ---- 行类型分类 ----

#[test]
fn row_types() {
    let b = base(12);
    let left = edit(&b, &[(3, "L3"), (6, "same"), (9, "L9")]);
    let right = edit(&b, &[(5, "R5"), (6, "same"), (9, "R9")]);
    let m = map(&left, &right, &b);
    let s = state(&m);
    let kind = |line: usize| s.rows[line - 1].choice;
    assert_eq!(kind(1), MergeChoice::Center);
    assert_eq!(kind(3), MergeChoice::Left);
    assert_eq!(kind(5), MergeChoice::Right);
    assert_eq!(kind(6), MergeChoice::Both);
    assert_eq!(kind(9), MergeChoice::Conflict);
    assert_eq!(s.source(2), Side::Left);
    assert_eq!(s.source(4), Side::Right);
    assert_eq!(s.source(5), Side::Left);
    assert_eq!(s.source(8), Side::Center);
}

#[test]
fn a_gap_against_a_line_is_a_change() {
    let b = base(8);
    let mut left = b.clone();
    left.remove(3);
    let m = map(&left, &b, &b);
    let s = state(&m);
    assert_eq!(s.rows[3].choice, MergeChoice::Left);
    assert!(s.conflicts().is_empty());
    assert_eq!(output(&s, &m), text(&left));
}

#[test]
fn an_unimportant_single_side_change_is_still_taken() {
    let b = base(8);
    let left = edit(&b, &[(5, "    line05")]);
    let mut rules = TextRules::default();
    rules.comparison.importance.leading_whitespace_unimportant = true;
    let m = map_of(bytes(&left), bytes(&b), bytes(&b), &rules);
    let s = state(&m);
    assert_eq!(s.rows[4].choice, MergeChoice::Left);
    assert_eq!(s.rows[4].importance, Importance::Unimportant);
    assert_eq!(output(&s, &m), text(&left));
}

// ---- 冲突区段 ----

#[test]
fn the_same_line_changed_differently_is_a_conflict() {
    let b = base(12);
    let m = map(&edit(&b, &[(5, "L")]), &edit(&b, &[(5, "R")]), &b);
    let s = state(&m);
    assert_eq!(conflict_rows(&s), vec![4..5]);
    assert_eq!(s.conflicts()[0].importance, Importance::Important);
    assert!(s.in_conflict(4) && !s.in_conflict(5));
}

#[test]
fn conflict_gap_bounds_how_far_apart_two_sides_may_change() {
    let b = base(12);
    let rows = |right_line: usize, gap: u32| {
        let m = map(&edit(&b, &[(5, "L")]), &edit(&b, &[(right_line, "R")]), &b);
        let rules = TextMergeRules {
            conflict_gap: gap,
            ..Default::default()
        };
        conflict_rows(&MergeState::classify(&m, &rules).unwrap())
    };
    // Default gap 2: 0, 1 and 2 unchanged rows between are conflicts, 3 are not.
    assert_eq!(rows(6, 2), vec![4..6]);
    assert_eq!(rows(7, 2), vec![4..7]);
    assert_eq!(rows(8, 2), vec![4..8]);
    assert_eq!(rows(9, 2), Vec::<std::ops::Range<usize>>::new());
    // Gap 0: only touching runs.
    assert_eq!(rows(6, 0), vec![4..6]);
    assert_eq!(rows(7, 0), Vec::<std::ops::Range<usize>>::new());
}

#[test]
fn far_apart_changes_do_not_conflict() {
    let b = base(24);
    let left = edit(&b, &[(5, "L")]);
    let right = edit(&b, &[(20, "R")]);
    let m = map(&left, &right, &b);
    let s = state(&m);
    assert!(s.conflicts().is_empty());
    assert_eq!(output(&s, &m), text(&edit(&b, &[(5, "L"), (20, "R")])));
}

#[test]
fn unequal_inserts_at_one_place_are_one_conflict() {
    let b = base(12);
    let left = insert(&b, 5, &["left 1", "left 2", "left 3"]);
    let right = insert(&b, 5, &["right 1"]);
    let m = map(&left, &right, &b);
    let s = state(&m);
    assert_eq!(conflict_rows(&s), vec![5..8]);
}

#[test]
fn identical_changes_are_not_a_conflict() {
    let b = base(12);
    let both = insert(&edit(&b, &[(3, "same")]), 8, &["new a", "new b"]);
    let m = map(&both, &both, &b);
    let s = state(&m);
    assert!(s.conflicts().is_empty());
    assert_eq!(output(&s, &m), text(&both));
}

#[test]
fn an_identical_change_next_to_a_single_side_change_is_not_a_conflict() {
    let b = base(12);
    let left = edit(&b, &[(5, "same")]);
    let right = edit(&b, &[(5, "same"), (6, "R6")]);
    let m = map(&left, &right, &b);
    let s = state(&m);
    assert!(s.conflicts().is_empty());
    assert_eq!(output(&s, &m), text(&right));
}

#[test]
fn opposite_side_runs_chain_into_one_conflict() {
    let b = base(14);
    let left = edit(&b, &[(3, "L3"), (7, "L7")]);
    let right = edit(&b, &[(5, "R5")]);
    let m = map(&left, &right, &b);
    assert_eq!(conflict_rows(&state(&m)), vec![2..7]);
    // Two same-side runs are never joined on their own.
    let left = edit(&b, &[(3, "L3"), (5, "L5")]);
    let right = edit(&b, &[(7, "R7")]);
    let m = map(&left, &right, &b);
    assert_eq!(conflict_rows(&state(&m)), vec![4..7]);
}

// ---- 默认输出内容 ----

#[test]
fn close_changes_are_both_taken_but_stay_a_conflict() {
    let b = base(12);
    let m = map(&edit(&b, &[(5, "L5")]), &edit(&b, &[(7, "R7")]), &b);
    let s = state(&m);
    assert_eq!(s.conflicts().len(), 1);
    assert_eq!(output(&s, &m), text(&edit(&b, &[(5, "L5"), (7, "R7")])));
}

#[test]
fn overlapping_changes_keep_the_center() {
    let b = base(12);
    let m = map(&edit(&b, &[(5, "L5")]), &edit(&b, &[(5, "R5")]), &b);
    let s = state(&m);
    assert_eq!(output(&s, &m), text(&b));
}

// ---- 忽略不重要差异与 favor ----

fn case_rules() -> TextRules {
    let mut rules = TextRules::default();
    rules.comparison.importance.case_unimportant = true;
    rules
}
fn merge_rules(ignore: bool, favor: Favor) -> TextMergeRules {
    TextMergeRules {
        ignore_unimportant: ignore,
        favor,
        ..Default::default()
    }
}

#[test]
fn ignoring_unimportant_differences_removes_them_from_the_merge() {
    let b = base(12);
    let left = edit(&b, &[(5, "LINE05")]);
    let right = edit(&b, &[(5, "line05 changed")]);
    let m = map_of(bytes(&left), bytes(&right), bytes(&b), &case_rules());
    // Without /iu the case change is a change: both sides changed differently.
    assert_eq!(state(&m).conflicts().len(), 1);
    let s = MergeState::classify(&m, &merge_rules(true, Favor::None)).unwrap();
    assert_eq!(s.rows[4].choice, MergeChoice::Right);
    assert!(s.conflicts().is_empty());
    assert_eq!(output(&s, &m), text(&right));
    // A lone unimportant change is dropped as well.
    let m = map_of(bytes(&left), bytes(&b), bytes(&b), &case_rules());
    let s = MergeState::classify(&m, &merge_rules(true, Favor::None)).unwrap();
    assert_eq!(s.rows[4].choice, MergeChoice::Center);
    assert_eq!(output(&s, &m), text(&b));
}

#[test]
fn ignored_unimportant_conflicts_take_the_center_or_the_favored_side() {
    let b = base(12);
    let left = edit(&b, &[(5, "LINE05")]);
    let right = edit(&b, &[(5, "Line05")]);
    let m = map_of(bytes(&left), bytes(&right), bytes(&b), &case_rules());
    let plain = state(&m);
    assert_eq!(plain.conflicts().len(), 1);
    assert_eq!(plain.conflicts()[0].importance, Importance::Unimportant);
    for (favor, expected) in [
        (Favor::None, &b),
        (Favor::Left, &left),
        (Favor::Right, &right),
    ] {
        let s = MergeState::classify(&m, &merge_rules(true, favor)).unwrap();
        assert!(s.conflicts().is_empty(), "{favor:?}");
        assert_eq!(output(&s, &m), text(expected), "{favor:?}");
    }
    // Favor alone changes nothing.
    let s = MergeState::classify(&m, &merge_rules(false, Favor::Right)).unwrap();
    assert_eq!(s.conflicts().len(), 1);
    assert_eq!(output(&s, &m), output(&plain, &m));
}

// ---- 冲突解决 ----

#[test]
fn taking_a_side_replaces_the_whole_section() {
    let b = base(12);
    let left = insert(&b, 5, &["left 1", "left 2", "left 3"]);
    let right = insert(&edit(&b, &[(11, "R11")]), 5, &["right 1"]);
    let left = edit(&left, &[(14, "L11")]);
    let m = map(&left, &right, &b);
    let mut s = state(&m);
    assert_eq!(s.conflicts().len(), 2);
    let first = s.conflicts()[0].clone();
    s.resolve(&first, Side::Right);
    assert_eq!(s.conflicts().len(), 1);
    let out = output(&s, &m);
    assert!(out.contains("right 1\n") && !out.contains("left "));
    // The other section is untouched: still unresolved, still the center there.
    assert!(out.contains("line11\n"));
    let second = s.conflicts()[0].clone();
    s.resolve(&second, Side::Center);
    assert!(s.conflicts().is_empty());
    assert_eq!(output(&s, &m), text(&insert(&b, 5, &["right 1"])));
}

#[test]
fn accepting_keeps_the_content_and_clears_the_conflict() {
    let b = base(12);
    let m = map(&edit(&b, &[(5, "L5")]), &edit(&b, &[(7, "R7")]), &b);
    let mut s = state(&m);
    let before = output(&s, &m);
    let rows = s.conflicts()[0].rows.clone();
    assert_eq!(s.section_at(5), Some(rows.clone()));
    s.accept(&rows);
    assert!(s.conflicts().is_empty());
    assert_eq!(output(&s, &m), before);
    s.set_resolved(&rows, false);
    assert_eq!(s.conflicts().len(), 1);
}

// ---- 自动合并结果 ----

#[test]
fn auto_merge_outcomes() {
    let b = base(12);
    let rules = TextMergeRules::default();
    // No conflict.
    let m = map(&edit(&b, &[(2, "L2")]), &edit(&b, &[(10, "R10")]), &b);
    let outcome = auto_merge(&m, &rules, &labels()).unwrap();
    assert!(outcome.conflicts.is_empty() && !outcome.withheld);
    assert_eq!(outcome.output, bytes(&edit(&b, &[(2, "L2"), (10, "R10")])));
    assert!(outcome.warnings.is_empty());
    // Conflict, not forced: withheld.
    let m = map(&edit(&b, &[(5, "L")]), &edit(&b, &[(5, "R")]), &b);
    let outcome = auto_merge(&m, &rules, &labels()).unwrap();
    assert_eq!(outcome.conflicts.len(), 1);
    assert!(outcome.withheld && outcome.output.is_empty());
    // Identical inputs.
    let m = map(&b, &b, &b);
    let outcome = auto_merge(&m, &rules, &labels()).unwrap();
    assert!(outcome.conflicts.is_empty());
    assert_eq!(outcome.output, bytes(&b));
}

// ---- 冲突标记 ----

fn forced() -> TextMergeRules {
    TextMergeRules {
        force_conflict_markers: true,
        ..Default::default()
    }
}

#[test]
fn forced_markers_wrap_each_unresolved_conflict() {
    let b = base(5);
    let m = map(&edit(&b, &[(3, "A")]), &edit(&b, &[(3, "B")]), &b);
    let outcome = auto_merge(&m, &forced(), &labels()).unwrap();
    assert_eq!(outcome.conflicts.len(), 1);
    assert!(!outcome.withheld);
    assert_eq!(
        String::from_utf8(outcome.output).unwrap(),
        "line01\nline02\n<<<<<<< theirs.txt\nA\n=======\nB\n>>>>>>> yours.txt\nline04\nline05\n"
    );
}

#[test]
fn markers_with_a_deleted_side_and_a_close_change() {
    let b = base(6);
    let mut left = b.clone();
    left.drain(2..4);
    let right = edit(&b, &[(3, "R3")]);
    let m = map(&left, &right, &b);
    let outcome = auto_merge(&m, &forced(), &labels()).unwrap();
    assert_eq!(
        String::from_utf8(outcome.output).unwrap(),
        "line01\nline02\n<<<<<<< theirs.txt\n=======\nR3\nline04\n>>>>>>> yours.txt\nline05\nline06\n"
    );
}

#[test]
fn markers_stay_on_their_own_lines_without_a_final_newline() {
    let center = b"a\nb".to_vec();
    let m = map_of(b"a\nL".to_vec(), b"a\nR".to_vec(), center, &TextRules::default());
    let outcome = auto_merge(&m, &forced(), &labels()).unwrap();
    assert_eq!(
        String::from_utf8(outcome.output).unwrap(),
        "a\n<<<<<<< theirs.txt\nL\n=======\nR\n>>>>>>> yours.txt\n"
    );
}

// ---- 输出的行尾与编码 ----

#[test]
fn every_line_keeps_the_ending_of_its_source() {
    let b = base(8);
    let crlf = |lines: &[String]| bytes(lines).iter().flat_map(|&c| {
        if c == b'\n' { vec![b'\r', b'\n'] } else { vec![c] }
    }).collect::<Vec<u8>>();
    let left = edit(&b, &[(5, "line05 LEFT")]);
    let m = map_of(bytes(&left), crlf(&b), crlf(&b), &TextRules::default());
    let outcome = auto_merge(&m, &TextMergeRules::default(), &labels()).unwrap();
    assert!(outcome.conflicts.is_empty());
    let expected: Vec<u8> = b
        .iter()
        .enumerate()
        .flat_map(|(i, line)| {
            if i == 4 {
                b"line05 LEFT\n".to_vec()
            } else {
                format!("{line}\r\n").into_bytes()
            }
        })
        .collect();
    assert_eq!(outcome.output, expected);
    assert_eq!(outcome.warnings, vec!["the output has mixed line endings".to_string()]);
}

#[test]
fn a_line_that_moves_off_the_end_of_its_file_gets_an_ending() {
    let m = map_of(b"a\nb".to_vec(), b"a\nb\nc\n".to_vec(), b"a\nb".to_vec(), &TextRules::default());
    let outcome = auto_merge(&m, &TextMergeRules::default(), &labels()).unwrap();
    assert!(outcome.conflicts.is_empty());
    assert_eq!(outcome.output, b"a\nb\nc\n");
}

#[test]
fn the_output_encoding_is_a_three_way_pick() {
    // cp1252 "café" is not valid UTF-8, so the sides load as code page 1252.
    let ansi = b"caf\xe9\nline02\nline03\n".to_vec();
    let mut utf8_bom = b"\xef\xbb\xbf".to_vec();
    utf8_bom.extend_from_slice("café\nline02\nline03\n".as_bytes());
    let mut right = ansi.clone();
    right.extend_from_slice(b"line04\n");
    let m = map_of(utf8_bom.clone(), right, ansi.clone(), &TextRules::default());
    assert_eq!(m.center.as_ref().unwrap().encoding, Encoding::CodePage(1252));
    let outcome = auto_merge(&m, &TextMergeRules::default(), &labels()).unwrap();
    assert!(outcome.conflicts.is_empty());
    let mut expected = utf8_bom.clone();
    expected.extend_from_slice(b"line04\n");
    assert_eq!(outcome.output, expected);
    assert!(outcome.warnings.is_empty());

    // Both sides left the center encoding in different ways: keep the center, warn; a
    // character the center encoding cannot hold is an error, not a lossy write.
    let utf16: Vec<u8> = [0xff, 0xfe]
        .into_iter()
        .chain("café\nline02\nline03\n".encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    let m = map_of(utf16.clone(), utf8_bom.clone(), ansi.clone(), &TextRules::default());
    let outcome = auto_merge(&m, &TextMergeRules::default(), &labels()).unwrap();
    assert_eq!(outcome.output, ansi);
    assert_eq!(outcome.warnings.len(), 1);
    let mut wide = utf8_bom;
    wide.extend_from_slice("中文\n".as_bytes());
    let m = map_of(utf16, wide, ansi, &TextRules::default());
    assert!(auto_merge(&m, &TextMergeRules::default(), &labels()).is_err());
}
