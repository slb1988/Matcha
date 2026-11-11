use bc_core::{
    align::{AlignInput, Aligner, LineWeight, SmartAlign, SmartAlignOptions},
    hash::LineHasher,
    line::LineList,
    DiffKind,
};

fn compare(
    left: &str,
    right: &str,
    options: SmartAlignOptions,
    lw: Option<&[LineWeight]>,
    rw: Option<&[LineWeight]>,
) -> bc_core::align::AlignMap {
    let l = LineList::from_bytes(left.as_bytes().to_vec());
    let r = LineList::from_bytes(right.as_bytes().to_vec());
    let lh = LineHasher::default().hash_lines(&l);
    let rh = LineHasher::default().hash_lines(&r);
    let map = SmartAlign { options }.align2(
        AlignInput {
            hashes: &lh,
            lines: Some(&l),
            weights: lw,
            ..Default::default()
        },
        AlignInput {
            hashes: &rh,
            lines: Some(&r),
            weights: rw,
            ..Default::default()
        },
    );
    assert_eq!(
        map.rows().iter().filter_map(|r| r.left).collect::<Vec<_>>(),
        (0..l.len()).collect::<Vec<_>>()
    );
    assert_eq!(
        map.rows()
            .iter()
            .filter_map(|r| r.right)
            .collect::<Vec<_>>(),
        (0..r.len()).collect::<Vec<_>>()
    );
    map
}
#[test]
fn similar_change_is_paired_and_unrelated_equal_blocks_tie_into_pairs() {
    let map = compare(
        "before
total = price * qty
after",
        "before
total = price * quantity
after",
        Default::default(),
        None,
        None,
    );
    assert_eq!(map.len(), 3);
    assert_eq!(map.row(1).unwrap().kind, DiffKind::Changed);
    // Closeness ties keep the pair: completely dissimilar equal-count blocks pair up.
    let map = compare(
        "AAAA
BBBB
CCCC",
        "1111
2222
3333",
        Default::default(),
        None,
        None,
    );
    assert_eq!(map.len(), 3);
    assert!(map.rows().iter().all(|r| r.kind == DiffKind::Changed));
}
#[test]
fn closeness_prefers_the_similar_line_over_position() {
    let map = compare(
        "value = old",
        "unrelated ZZZ
value = new",
        Default::default(),
        None,
        None,
    );
    assert_eq!(map.row_of_left(0), map.row_of_right(1));
    assert_eq!(map.len(), 2);
    // Without closeness the first lines pair by position, no similarity check.
    let map = compare(
        "value = old",
        "unrelated ZZZ
value = new",
        SmartAlignOptions {
            use_closeness: false,
            ..Default::default()
        },
        None,
        None,
    );
    assert_eq!(map.row_of_left(0), map.row_of_right(0));
    assert_eq!(map.row(1).unwrap().kind, DiffKind::RightOnly);
    let map = compare(
        "value = old",
        "unrelated ZZZ
value = new",
        SmartAlignOptions {
            never_align_diffs: true,
            ..Default::default()
        },
        None,
        None,
    );
    assert_eq!(map.len(), 3);
    assert!(map.rows().iter().all(|r| r.kind != DiffKind::Changed));
}
#[test]
fn matching_dfm_object_weights_anchor_before_unweighted_caption() {
    let map = compare(
        "object Left: TFoo
Caption = alpha",
        "Caption = alphb
object Right: TFoo",
        Default::default(),
        Some(&[LineWeight(4), LineWeight(0)]),
        Some(&[LineWeight(0), LineWeight(4)]),
    );
    assert_eq!(map.row_of_left(0), map.row_of_right(1));
    // Unweighted, the closer caption lines win the single monotone pair.
    let map = compare(
        "object Left: TFoo
Caption = alpha",
        "Caption = alphb
object Right: TFoo",
        Default::default(),
        None,
        None,
    );
    assert_eq!(map.row_of_left(1), map.row_of_right(0));
}
#[test]
fn unicode_case_whitespace_and_long_lines_are_handled() {
    let map = compare(
        "Ä Value = alpha",
        "ä value=alpha",
        SmartAlignOptions {
            case_tolerant: true,
            ..Default::default()
        },
        None,
        None,
    );
    assert_eq!(map.len(), 1);
    // Long lines with no common byte score the -40 floor, tying two gaps: still paired.
    let left = "a".repeat(500_000);
    let right = "b".repeat(500_000);
    let map = compare(&left, &right, Default::default(), None, None);
    assert_eq!(map.len(), 1);
    assert_eq!(map.row(0).unwrap().kind, DiffKind::Changed);
}
#[test]
fn oversized_blocks_skip_the_dp_and_pair_by_position() {
    let n = bc_core::align::CLOSENESS_MAX_SIDE + 1;
    let left: String = (0..n)
        .map(|i| {
            format!(
                "left {i}
"
            )
        })
        .collect();
    let right: String = (0..n)
        .map(|i| {
            format!(
                "RIGHT#{i}
"
            )
        })
        .collect();
    let map = compare(&left, &right, Default::default(), None, None);
    assert_eq!(map.len(), n);
    assert!(map
        .rows()
        .iter()
        .enumerate()
        .all(|(i, r)| r.left == Some(i) && r.right == Some(i) && r.kind == DiffKind::Changed));
}
#[test]
fn hashes_only_callers_keep_quick_output_and_bad_metadata_is_rejected() {
    use bc_core::hash::ContentHash;
    let l = [ContentHash(1)];
    let r = [ContentHash(2)];
    let map = SmartAlign::default().align2(
        AlignInput {
            hashes: &l,
            ..Default::default()
        },
        AlignInput {
            hashes: &r,
            ..Default::default()
        },
    );
    assert_eq!(map.len(), 2);
    let lines = LineList::from_bytes(
        b"a
b"
        .to_vec(),
    );
    assert!(SmartAlign::default()
        .align2_with_overrides(
            AlignInput {
                hashes: &l,
                lines: Some(&lines),
                ..Default::default()
            },
            AlignInput::default(),
            &Default::default()
        )
        .is_err());
}
