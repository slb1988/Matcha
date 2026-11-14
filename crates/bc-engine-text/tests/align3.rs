//! Three-way alignment jobs and the three-column `TextMap` (spec `three-way-alignment`).
use bc_core::{align::AlignStrategy, cancel::NeverCancel, CoreError, DiffKind, Side};
use bc_engine_text::{
    importance::Importance,
    jobs::{Align3Job, Realign3Job},
    rules::TextRules,
    TextContent, TextMap,
};

type Row = (Option<usize>, Option<usize>, Option<usize>);

fn content(lines: &[&str]) -> TextContent {
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    TextContent::from_bytes(text.into_bytes(), None).unwrap()
}
fn base() -> Vec<String> {
    (1..=12).map(|i| format!("line{i:02}")).collect()
}
fn refs(lines: &[String]) -> Vec<&str> {
    lines.iter().map(String::as_str).collect()
}
fn merge(left: &[&str], right: &[&str], center: &[&str], rules: TextRules) -> TextMap {
    TextMap::merge(content(left), content(right), content(center), rules).unwrap()
}
fn rows(map: &TextMap) -> Vec<Row> {
    map.rows().map(|r| (r.left, r.center, r.right)).collect()
}

#[test]
fn single_side_insert_leaves_gaps_on_center_and_right() {
    let center = base();
    let mut left = center.clone();
    for (i, line) in ["new 1", "new 2", "new 3"].iter().enumerate() {
        left.insert(10 + i, line.to_string());
    }
    let map = merge(&refs(&left), &refs(&center), &refs(&center), TextRules::default());
    let rows = rows(&map);
    assert_eq!(rows.len(), 15);
    assert_eq!(rows[9], (Some(9), Some(9), Some(9)));
    assert_eq!(rows[10..13], [(Some(10), None, None), (Some(11), None, None), (Some(12), None, None)]);
    assert_eq!(rows[13], (Some(13), Some(10), Some(10)));
    assert_eq!(map.text_at(Side::Center, 13).as_deref(), Some(&b"line11"[..]));
    assert_eq!(map.text_at(Side::Left, 10).as_deref(), Some(&b"new 1"[..]));
    assert!(map.text_at(Side::Center, 10).is_none());
    // The change run is one section; untouched rows are `Same`.
    assert_eq!(map.sections().len(), 1);
    assert_eq!(map.sections()[0].rows, 10..13);
    assert_eq!(map.row(0).unwrap().kind, DiffKind::Same);
    assert_eq!(map.row(10).unwrap().importance, Importance::Important);
}

#[test]
fn left_equal_to_center_mirrors_the_center_right_comparison() {
    let center = base();
    let mut right = center.clone();
    right[4] = "line05 changed".into();
    right.remove(8);
    right.insert(2, "inserted".into());
    let three = merge(&refs(&center), &refs(&right), &refs(&center), TextRules::default());
    let two = TextMap::compare(
        content(&refs(&center)),
        content(&refs(&right)),
        TextRules::default(),
    )
    .unwrap();
    let expected: Vec<Row> = two.rows().map(|r| (r.left, r.left, r.right)).collect();
    assert_eq!(rows(&three), expected);
    let kinds = |m: &TextMap| -> Vec<bool> { m.rows().map(|r| r.kind == DiffKind::Same).collect() };
    assert_eq!(kinds(&three), kinds(&two));
}

#[test]
fn a_modified_line_stays_on_the_row_of_its_center_line() {
    let center = base();
    let mut right = center.clone();
    right[4] = "line05 RIGHT".into();
    let map = merge(&refs(&center), &refs(&right), &refs(&center), TextRules::default());
    assert_eq!(rows(&map)[4], (Some(4), Some(4), Some(4)));
    assert_eq!(map.row(4).unwrap().kind, DiffKind::Changed);
    assert_eq!(map.text_at(Side::Right, 4).as_deref(), Some(&b"line05 RIGHT"[..]));
}

#[test]
fn inserts_at_the_same_place_share_rows() {
    let center = base();
    let mut left = center.clone();
    for (i, line) in ["left 1", "left 2", "left 3"].iter().enumerate() {
        left.insert(5 + i, line.to_string());
    }
    let mut right = center.clone();
    right.insert(5, "right 1".into());
    let map = merge(&refs(&left), &refs(&right), &refs(&center), TextRules::default());
    assert_eq!(
        rows(&map)[5..8],
        [(Some(5), None, Some(5)), (Some(6), None, None), (Some(7), None, None)]
    );
    assert_eq!(rows(&map)[8], (Some(8), Some(5), Some(6)));
}

#[test]
fn unimportant_trailing_whitespace_still_pairs_the_lines() {
    let center = base();
    let mut left = center.clone();
    left[4] = "line05   ".into();
    let mut rules = TextRules::default();
    rules.comparison.importance.trailing_whitespace_unimportant = true;
    let map = merge(&refs(&left), &refs(&center), &refs(&center), rules);
    assert_eq!(map.row_count(), 12);
    assert_eq!(rows(&map)[4], (Some(4), Some(4), Some(4)));
    assert_eq!(map.row(4).unwrap().importance, Importance::Unimportant);
}

#[test]
fn unaligned_pairs_the_nth_lines_of_all_three() {
    let mut rules = TextRules::default();
    rules.alignment.strategy = AlignStrategy::None;
    let map = merge(&["a", "b", "c"], &["x"], &["a", "q"], rules);
    assert_eq!(
        rows(&map),
        vec![
            (Some(0), Some(0), Some(0)),
            (Some(1), Some(1), None),
            (Some(2), None, None),
        ]
    );
}

#[test]
fn every_strategy_keeps_each_side_an_ordered_partition() {
    let center = base();
    let mut left = center.clone();
    left[1] = "line02 L".into();
    left.remove(6);
    let mut right = center.clone();
    right.insert(3, "r new".into());
    right[9] = "line10 R".into();
    for strategy in [
        AlignStrategy::Quick,
        AlignStrategy::Smart,
        AlignStrategy::Patience,
        AlignStrategy::None,
    ] {
        let mut rules = TextRules::default();
        rules.alignment.strategy = strategy;
        let map = merge(&refs(&left), &refs(&right), &refs(&center), rules);
        let rows = rows(&map);
        for (side, len) in [(0, left.len()), (1, center.len()), (2, right.len())] {
            let seen: Vec<usize> = rows
                .iter()
                .filter_map(|r| [r.0, r.1, r.2][side])
                .collect();
            assert_eq!(seen, (0..len).collect::<Vec<_>>(), "{strategy:?} side {side}");
        }
        assert!(rows.iter().all(|r| r.0.is_some() || r.1.is_some() || r.2.is_some()));
    }
}

#[test]
fn realign_equals_a_full_alignment() {
    let center = base();
    let mut left = center.clone();
    left[2] = "line03 L".into();
    let mut right = center.clone();
    right.insert(7, "r new".into());
    let mut map = merge(&refs(&left), &refs(&right), &refs(&center), TextRules::default());
    let before = rows(&map);
    let job = Align3Job::from_map(&map).unwrap();
    let full = job.execute(&NeverCancel).unwrap();
    let again = Realign3Job {
        base: job,
        dirty_rows: 2..3,
    }
    .execute(&NeverCancel)
    .unwrap();
    assert_eq!(full.row_count(), again.row_count());
    map.set_alignment(again).unwrap();
    let realigned = rows(&map);
    map.set_alignment(full).unwrap();
    assert_eq!(realigned, rows(&map));
    assert_eq!(realigned, before);
}

#[test]
fn cancellation_publishes_nothing() {
    let center = base();
    let mut left = center.clone();
    left[2] = "line03 L".into();
    let map = merge(&refs(&left), &refs(&center), &refs(&center), TextRules::default());
    let before = rows(&map);
    let job = Align3Job::from_map(&map).unwrap();
    let cancelled = || true;
    assert!(matches!(job.execute(&cancelled), Err(CoreError::Cancelled)));
    assert_eq!(rows(&map), before);
    assert!(!map.is_stale());
}

#[test]
fn a_two_way_map_has_no_center_and_is_unaffected() {
    let two = TextMap::compare(content(&["a", "b"]), content(&["a", "c"]), TextRules::default())
        .unwrap();
    assert!(Align3Job::from_map(&two).is_none());
    assert!(two.rows().all(|r| r.center.is_none()));
    assert!(two.text_at(Side::Center, 0).is_none());
    assert!(!two.is_stale());
}
