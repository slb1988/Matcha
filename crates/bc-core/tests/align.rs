use bc_core::{
    align::{AlignMap, AlignRow, AlignmentOverrides},
    DiffKind,
};

use bc_core::{
    align::{AlignInput, Aligner, QuickAlign},
    hash::ContentHash,
};

#[test]
fn quick_alignment_matches_identical_and_three_inserted_lines() {
    let left: Vec<_> = (0..20).map(ContentHash).collect();
    let mut right = left.clone();
    let equal = QuickAlign.align2(input(&left), input(&right));
    validate(&equal, 20, 20);
    assert!(equal.rows().iter().all(|r| r.kind == DiffKind::Same));
    right.splice(10..10, [ContentHash(99), ContentHash(98), ContentHash(97)]);
    let map = QuickAlign.align2(input(&left), input(&right));
    validate(&map, 20, 23);
    assert!(map.rows()[10..13]
        .iter()
        .all(|r| r.kind == DiffKind::RightOnly && r.left.is_none()));
    assert_eq!(map.row_of_left(10), Some(13));
    assert!(QuickAlign
        .align2(AlignInput::default(), AlignInput::default())
        .is_empty());
}

#[test]
fn manual_anchors_survive_recompare_and_clear_restores_automatic_alignment() {
    let left: Vec<_> = (0..60).map(ContentHash).collect();
    let right = left.clone();
    let mut anchors = AlignmentOverrides::default();
    anchors.insert(39, 54).unwrap();
    for _ in 0..2 {
        let map = QuickAlign
            .align2_with_overrides(input(&left), input(&right), &anchors)
            .unwrap();
        validate(&map, 60, 60);
        assert_eq!(map.row_of_left(39), map.row_of_right(54));
    }
    anchors.clear();
    let map = QuickAlign
        .align2_with_overrides(input(&left), input(&right), &anchors)
        .unwrap();
    assert_eq!(map.rows(), (0..60).map(same).collect::<Vec<_>>());
}

#[test]
fn no_alignment_pairs_nth_lines_and_preserves_tail() {
    let map = AlignMap::unaligned(&[ContentHash(1), ContentHash(2)], &[ContentHash(9)]);
    validate(&map, 2, 1);
    assert_eq!(map.row(0).unwrap().kind, DiffKind::Changed);
    assert_eq!(map.row(1).unwrap().kind, DiffKind::LeftOnly);
}

#[test]
fn myers_display_packs_only_within_replacement_blocks() {
    let left = [
        ContentHash(1),
        ContentHash(2),
        ContentHash(3),
        ContentHash(4),
        ContentHash(9),
    ];
    let right = [
        ContentHash(1),
        ContentHash(5),
        ContentHash(6),
        ContentHash(9),
    ];
    let map = QuickAlign.align2(input(&left), input(&right));
    validate(&map, 5, 4);
    assert_eq!(
        map.rows().iter().map(|r| r.kind).collect::<Vec<_>>(),
        vec![
            DiffKind::Same,
            DiffKind::Changed,
            DiffKind::Changed,
            DiffKind::LeftOnly,
            DiffKind::Same
        ]
    );
    assert_eq!(map.row(1).unwrap().left, Some(1));
    assert_eq!(map.row(1).unwrap().right, Some(1));
    let raw = QuickAlign.align2_unpaired(input(&left), input(&right));
    validate(&raw, 5, 4);
    assert_eq!(raw.len(), 7);
    assert!(raw.rows().iter().all(|r| r.kind != DiffKind::Changed));
}

fn input(hashes: &[ContentHash]) -> AlignInput<'_> {
    AlignInput {
        hashes,
        ..Default::default()
    }
}

fn same(i: usize) -> AlignRow {
    AlignRow {
        left: Some(i),
        right: Some(i),
        center: None,
        kind: DiffKind::Same,
    }
}
fn validate(map: &AlignMap, n: usize, m: usize) {
    assert_eq!(
        map.rows().iter().filter_map(|r| r.left).collect::<Vec<_>>(),
        (0..n).collect::<Vec<_>>()
    );
    assert_eq!(
        map.rows()
            .iter()
            .filter_map(|r| r.right)
            .collect::<Vec<_>>(),
        (0..m).collect::<Vec<_>>()
    );
    for (i, r) in map.rows().iter().enumerate() {
        if let Some(l) = r.left {
            assert_eq!(map.row_of_left(l), Some(i));
        }
        if let Some(r) = r.right {
            assert_eq!(map.row_of_right(r), Some(i));
        }
    }
}
#[test]
fn reverse_indexes_and_validation() {
    let map = AlignMap::from_rows((0..10).map(same).collect());
    validate(&map, 10, 10);
    assert_eq!(map.row_of_left(10), None);
    assert_eq!(map.row_of_center(0), None);
    assert!(AlignMap::try_from_rows(vec![same(1)]).is_err());
    assert!(AlignMap::try_from_rows(vec![same(0), same(0)]).is_err());
    assert!(AlignMap::try_from_rows(vec![AlignRow::default()]).is_err());
    let center = AlignMap::from_rows(vec![AlignRow {
        center: Some(0),
        ..same(0)
    }]);
    assert_eq!(center.row_of_center(0), Some(0));
}
#[test]
fn navigation_moves_by_blocks_without_wrapping() {
    let mut rows: Vec<_> = (0..10).map(same).collect();
    for i in [1, 2, 5, 6, 8] {
        rows[i].kind = DiffKind::Changed;
    }
    let map = AlignMap::from_rows(rows);
    for (from, next, prev) in [
        (0, Some(1), None),
        (1, Some(5), None),
        (2, Some(5), None),
        (4, Some(5), Some(1)),
        (5, Some(8), Some(1)),
        (6, Some(8), Some(1)),
        (8, None, Some(5)),
        (9, None, Some(8)),
    ] {
        assert_eq!(map.next_difference(from), next);
        assert_eq!(map.prev_difference(from), prev);
    }
    assert_eq!(map.next_difference(10), None);
    assert_eq!(map.prev_difference(10), None);
}
#[test]
fn force_pair_preserves_every_index_in_both_directions() {
    for l in 0..60 {
        for r in 0..60 {
            let mut map = AlignMap::from_rows((0..60).map(same).collect());
            map.force_pair(l, r).unwrap();
            validate(&map, 60, 60);
            assert_eq!(map.row_of_left(l), map.row_of_right(r));
        }
    }
    let mut map = AlignMap::from_rows((0..60).map(same).collect());
    map.force_pair(39, 54).unwrap();
    assert_eq!(map.row_of_left(39), map.row_of_right(54));
    assert!(map.force_pair(60, 0).is_err());
}
#[test]
fn session_overrides_reject_crossing_and_can_be_cleared() {
    let mut anchors = AlignmentOverrides::default();
    anchors.insert(40, 55).unwrap();
    anchors.insert(10, 20).unwrap();
    anchors.insert(40, 55).unwrap();
    assert_eq!(anchors.pairs(), &[(10, 20), (40, 55)]);
    for (l, r) in [(10, 21), (11, 20), (9, 56), (41, 54)] {
        assert!(anchors.insert(l, r).is_err());
    }
    anchors.clear();
    assert!(anchors.pairs().is_empty());
}
