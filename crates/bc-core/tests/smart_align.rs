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
fn similar_change_is_paired_but_unrelated_replacements_are_not() {
    let map = compare(
        "before\ntotal = price * qty\nafter",
        "before\ntotal = price * quantity\nafter",
        Default::default(),
        None,
        None,
    );
    assert_eq!(map.len(), 3);
    assert_eq!(map.row(1).unwrap().kind, DiffKind::Changed);
    let map = compare(
        "AAAA\nBBBB\nCCCC",
        "1111\n2222\n3333",
        Default::default(),
        None,
        None,
    );
    assert_eq!(map.len(), 6);
    assert!(map
        .rows()
        .iter()
        .all(|r| r.left.is_none() || r.right.is_none()));
}
#[test]
fn offset_tolerance_and_similarity_threshold_are_honored() {
    let map = compare(
        "value = old",
        "unrelated ZZZ\nvalue = new",
        Default::default(),
        None,
        None,
    );
    assert_eq!(map.row_of_left(0), map.row_of_right(1));
    let map = compare(
        "value = old",
        "unrelated ZZZ\nvalue = new",
        SmartAlignOptions {
            skew_tolerance: 0,
            ..Default::default()
        },
        None,
        None,
    );
    assert_ne!(map.row_of_left(0), map.row_of_right(1));
    let map = compare(
        "value = old",
        "value = new",
        SmartAlignOptions {
            pair_similarity_threshold: 1.0,
            ..Default::default()
        },
        None,
        None,
    );
    assert_eq!(map.len(), 2);
}
#[test]
fn matching_dfm_object_weights_anchor_before_unweighted_caption() {
    let map = compare(
        "object Left: TFoo\nCaption = alpha",
        "Caption = alphb\nobject Right: TFoo",
        Default::default(),
        Some(&[LineWeight(4), LineWeight(0)]),
        Some(&[LineWeight(0), LineWeight(4)]),
    );
    assert_eq!(map.row_of_left(0), map.row_of_right(1));
}
#[test]
fn unicode_case_whitespace_and_long_lines_are_bounded() {
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
    let left = "a".repeat(500_000);
    let right = "b".repeat(500_000);
    assert_eq!(
        compare(&left, &right, Default::default(), None, None).len(),
        2
    );
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
    let options = SmartAlignOptions {
        pair_similarity_threshold: f32::NAN,
        ..Default::default()
    };
    assert!(SmartAlign { options }
        .align2_with_overrides(
            AlignInput::default(),
            AlignInput::default(),
            &Default::default()
        )
        .is_err());
}
