use bc_core::{
    diff::{DiffOp, Differ, MyersDiff, PatienceDiff},
    hash::ContentHash,
};

#[test]
fn patience_classic_moved_unique_block() {
    let a = hashes(&[1, 2, 3, 4, 5, 6]);
    let b = hashes(&[3, 4, 5, 1, 2, 6]);
    let ops = PatienceDiff.diff(&a, &b);
    assert_eq!(check(&a, &b, &ops), 4);
    assert_eq!(
        ops,
        vec![
            DiffOp::Delete {
                a_start: 0,
                len: 2,
                b_pos: 0
            },
            DiffOp::Equal {
                a_start: 2,
                b_start: 0,
                len: 3
            },
            DiffOp::Insert {
                b_start: 3,
                len: 2,
                a_pos: 5
            },
            DiffOp::Equal {
                a_start: 5,
                b_start: 5,
                len: 1
            },
        ]
    );
}

#[test]
fn patience_prioritises_unique_anchors_over_repetitive_lcs() {
    let a = hashes(&[1, 1, 1, 2, 3]);
    let b = hashes(&[2, 1, 1, 1, 3]);
    let ops = PatienceDiff.diff(&a, &b);
    assert_eq!(check(&a, &b, &ops), 2);
    assert!(ops.contains(&DiffOp::Equal {
        a_start: 3,
        b_start: 0,
        len: 1
    }));
    assert_eq!(check(&a, &b, &MyersDiff::default().diff(&a, &b)), 4);
}

#[test]
fn patience_recomputes_uniqueness_inside_anchor_gaps() {
    let a = hashes(&[5, 6, 1, 5, 6, 2]);
    let b = hashes(&[6, 5, 1, 6, 5, 2]);
    let ops = PatienceDiff.diff(&a, &b);
    assert_eq!(check(&a, &b, &ops), 4);
    let matches: Vec<_> = ops
        .iter()
        .filter_map(|op| match *op {
            DiffOp::Equal { a_start, len, .. } => Some(&a[a_start..a_start + len]),
            _ => None,
        })
        .flatten()
        .copied()
        .collect();
    assert_eq!(matches, hashes(&[6, 1, 6, 2]));
}

#[test]
fn patience_no_unique_anchors_falls_back_and_handles_empty_inputs() {
    for (a, b) in [
        (&[][..], &[][..]),
        (&[1], &[]),
        (&[], &[2]),
        (&[1, 2, 1, 2], &[2, 1, 2, 1]),
        (&[1, 1], &[2, 2]),
    ] {
        let (a, b) = (hashes(a), hashes(b));
        let ops = PatienceDiff.diff(&a, &b);
        check(&a, &b, &ops);
        assert_eq!(ops, MyersDiff::default().diff(&a, &b));
    }
}

fn hashes(values: &[u64]) -> Vec<ContentHash> {
    values.iter().copied().map(ContentHash).collect()
}
fn check(a: &[ContentHash], b: &[ContentHash], ops: &[DiffOp]) -> usize {
    let (mut ai, mut bi, mut equal) = (0, 0, 0);
    let mut reconstructed = Vec::new();
    for op in ops {
        match *op {
            DiffOp::Equal {
                a_start,
                b_start,
                len,
            } => {
                assert!(len > 0);
                assert_eq!((a_start, b_start), (ai, bi));
                assert_eq!(&a[ai..ai + len], &b[bi..bi + len]);
                reconstructed.extend_from_slice(&a[ai..ai + len]);
                ai += len;
                bi += len;
                equal += len;
            }
            DiffOp::Delete {
                a_start,
                b_pos,
                len,
            } => {
                assert!(len > 0);
                assert_eq!((a_start, b_pos), (ai, bi));
                ai += len;
            }
            DiffOp::Insert {
                b_start,
                a_pos,
                len,
            } => {
                assert!(len > 0);
                assert_eq!((a_pos, b_start), (ai, bi));
                reconstructed.extend_from_slice(&b[bi..bi + len]);
                bi += len;
            }
        }
    }
    assert_eq!((ai, bi), (a.len(), b.len()));
    assert_eq!(reconstructed, b);
    equal
}

fn lcs(a: &[ContentHash], b: &[ContentHash]) -> usize {
    let mut row = vec![0; b.len() + 1];
    for x in a {
        let mut diagonal = 0;
        for (j, y) in b.iter().enumerate() {
            let old = row[j + 1];
            row[j + 1] = if x == y {
                diagonal + 1
            } else {
                row[j + 1].max(row[j])
            };
            diagonal = old;
        }
    }
    row[b.len()]
}

#[test]
fn basic_and_interleaved_cases() {
    let differ = MyersDiff::default();
    for (a, b) in [
        (&[][..], &[][..]),
        (&[], &[1, 2]),
        (&[1, 2], &[]),
        (&[1, 2, 3], &[1, 2, 3]),
        (&[1, 2], &[3, 4]),
        (&[1, 2, 3, 4], &[1, 9, 2, 8, 3, 7, 4]),
        (&[1, 2, 1, 3], &[2, 1, 2, 3]),
        (&[1, 2, 3], &[4, 2, 5]),
        (&[1, 2, 3, 4, 5, 6], &[6, 5, 4, 3, 2, 1]),
    ] {
        let (a, b) = (hashes(a), hashes(b));
        let ops = differ.diff(&a, &b);
        assert_eq!(check(&a, &b, &ops), lcs(&a, &b));
        assert_eq!(ops, differ.diff(&a, &b));
    }
    assert_eq!(
        differ.diff(&hashes(&[1, 2]), &hashes(&[1, 2])),
        vec![DiffOp::Equal {
            a_start: 0,
            b_start: 0,
            len: 2
        }]
    );
}

#[test]
fn thousand_random_sequences_match_naive_lcs() {
    let mut seed = 0x1234_5678_9abc_def0_u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let exact = MyersDiff {
        block_fallback_threshold: 0,
    };
    for round in 0..1000 {
        let n = (next() % 80) as usize;
        let m = (next() % 80) as usize;
        let alphabet = 2 + next() % 20;
        let a: Vec<_> = (0..n).map(|_| ContentHash(next() % alphabet)).collect();
        let b: Vec<_> = (0..m).map(|_| ContentHash(next() % alphabet)).collect();
        let ops = exact.diff(&a, &b);
        assert_eq!(
            check(&a, &b, &ops),
            lcs(&a, &b),
            "round {round}: {a:?} -> {b:?}"
        );
    }
}

#[test]
fn exhaustive_short_binary_sequences() {
    let sequences: Vec<Vec<_>> = (0..=5)
        .flat_map(|len| {
            (0..1 << len).map(move |bits| (0..len).map(|i| ContentHash((bits >> i) & 1)).collect())
        })
        .collect();
    let differ = MyersDiff {
        block_fallback_threshold: 0,
    };
    for a in &sequences {
        for b in &sequences {
            assert_eq!(check(a, b, &differ.diff(a, b)), lcs(a, b));
        }
    }
}

#[test]
fn common_edges_are_trimmed_before_fallback() {
    let a: Vec<_> = (0..100_000).map(ContentHash).collect();
    let mut b = a.clone();
    b.insert(50_000, ContentHash(u64::MAX));
    let ops = MyersDiff {
        block_fallback_threshold: 1,
    }
    .diff(&a, &b);
    assert_eq!(check(&a, &b, &ops), a.len());
    assert_eq!(ops.len(), 3);
}

#[test]
fn huge_block_fallback_preserves_ranges_and_realigns_shifted_content() {
    let a: Vec<_> = (0..100_000).map(|i| ContentHash(i * 7919)).collect();
    let mut b = a.clone();
    b[0] = ContentHash(u64::MAX);
    b[99_999] = ContentHash(u64::MAX - 1);
    b.insert(32_000, ContentHash(u64::MAX - 2));
    let ops = MyersDiff::default().diff(&a, &b);
    assert!(check(&a, &b, &ops) > 99_000);
    let different: Vec<_> = (1_000_000_000..1_000_100_000).map(ContentHash).collect();
    assert_eq!(
        check(&a, &different, &MyersDiff::default().diff(&a, &different)),
        0
    );
}

#[test]
fn forced_fallback_covers_empty_unbalanced_and_repeated_windows() {
    let differ = MyersDiff {
        block_fallback_threshold: 3,
    };
    for (a, b) in [
        (vec![ContentHash(1); 100], vec![ContentHash(2); 80]),
        (
            (0..100).map(ContentHash).collect(),
            vec![ContentHash(2); 10],
        ),
    ] {
        check(&a, &b, &differ.diff(&a, &b));
    }
}
