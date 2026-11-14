//! Three-way alignment: `reconcile3` / `Aligner::align3` (center-based).
use bc_core::{
    align::{reconcile3, AlignInput, AlignMap, Aligner, PatienceAlign, QuickAlign, SmartAlign},
    hash::{ContentHash, LineHasher},
    line::LineList,
    DiffKind,
};
use std::collections::BTreeSet;

type Row = (Option<usize>, Option<usize>, Option<usize>);

struct Side {
    lines: LineList,
    hashes: Vec<ContentHash>,
}
impl Side {
    fn new(lines: &[&str]) -> Self {
        let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
        let lines = LineList::from_bytes(text.into_bytes());
        let hashes = LineHasher::default().hash_lines(&lines);
        Self { lines, hashes }
    }
    fn input(&self) -> AlignInput<'_> {
        AlignInput {
            hashes: &self.hashes,
            lines: Some(&self.lines),
            ..Default::default()
        }
    }
}

fn aligners() -> Vec<(&'static str, Box<dyn Aligner>)> {
    vec![
        ("quick", Box::new(QuickAlign)),
        ("smart", Box::new(SmartAlign::default())),
        ("patience", Box::new(PatienceAlign::default())),
    ]
}

fn rows3(map: &AlignMap) -> Vec<Row> {
    map.rows()
        .iter()
        .map(|r| (r.left, r.center, r.right))
        .collect()
}

fn align3(aligner: &dyn Aligner, left: &[&str], center: &[&str], right: &[&str]) -> AlignMap {
    let (l, c, r) = (Side::new(left), Side::new(center), Side::new(right));
    aligner.align3(l.input(), c.input(), r.input())
}

fn base() -> Vec<&'static str> {
    vec!["a", "b", "c", "d", "e", "f"]
}

#[test]
fn single_side_insert_leaves_gaps_on_the_other_two() {
    for (name, aligner) in aligners() {
        let left = ["a", "b", "c", "x1", "x2", "x3", "d", "e", "f"];
        let map = align3(&*aligner, &left, &base(), &base());
        assert_eq!(
            rows3(&map),
            vec![
                (Some(0), Some(0), Some(0)),
                (Some(1), Some(1), Some(1)),
                (Some(2), Some(2), Some(2)),
                (Some(3), None, None),
                (Some(4), None, None),
                (Some(5), None, None),
                (Some(6), Some(3), Some(3)),
                (Some(7), Some(4), Some(4)),
                (Some(8), Some(5), Some(5)),
            ],
            "{name}"
        );
        assert_eq!(map.rows()[3].kind, DiffKind::LeftOnly, "{name}");
        assert_eq!(map.rows()[0].kind, DiffKind::Same, "{name}");
    }
}

#[test]
fn inserts_at_the_same_place_sit_side_by_side() {
    for (name, aligner) in aligners() {
        let left = ["a", "b", "c", "x1", "x2", "x3", "d", "e", "f"];
        let right = ["a", "b", "c", "y1", "d", "e", "f"];
        let map = align3(&*aligner, &left, &base(), &right);
        assert_eq!(
            rows3(&map)[3..6],
            [
                (Some(3), None, Some(3)),
                (Some(4), None, None),
                (Some(5), None, None),
            ],
            "{name}"
        );
        assert_eq!(map.rows()[3].kind, DiffKind::Changed, "{name}");
        assert_eq!(rows3(&map)[6], (Some(6), Some(3), Some(4)), "{name}");
    }
}

#[test]
fn deletion_keeps_the_center_line_with_a_gap() {
    for (name, aligner) in aligners() {
        let left = ["a", "b", "e", "f"];
        let map = align3(&*aligner, &left, &base(), &base());
        assert_eq!(
            rows3(&map),
            vec![
                (Some(0), Some(0), Some(0)),
                (Some(1), Some(1), Some(1)),
                (None, Some(2), Some(2)),
                (None, Some(3), Some(3)),
                (Some(2), Some(4), Some(4)),
                (Some(3), Some(5), Some(5)),
            ],
            "{name}"
        );
        assert_eq!(map.rows()[2].kind, DiffKind::Changed, "{name}");
    }
}

#[test]
fn empty_center_and_identical_inputs() {
    for (name, aligner) in aligners() {
        let map = align3(&*aligner, &["l1", "l2"], &[], &["r1"]);
        assert_eq!(
            rows3(&map),
            vec![(Some(0), None, Some(0)), (Some(1), None, None)],
            "{name}"
        );
        let map = align3(&*aligner, &base(), &base(), &base());
        assert_eq!(map.len(), 6, "{name}");
        assert!(
            map.rows().iter().enumerate().all(|(i, r)| {
                (r.left, r.center, r.right) == (Some(i), Some(i), Some(i))
                    && r.kind == DiffKind::Same
            }),
            "{name}"
        );
        let map = align3(&*aligner, &[], &[], &[]);
        assert!(map.is_empty(), "{name}");
    }
}

#[test]
fn reconcile_rejects_mismatched_centers() {
    let (l, c, r) = (
        Side::new(&["a"]),
        Side::new(&["a", "b"]),
        Side::new(&["a"]),
    );
    let a = QuickAlign.align2(l.input(), c.input());
    let b = QuickAlign.align2(l.input(), r.input());
    assert!(reconcile3(&a, &b).is_err());
}

/// Deterministic generator: small alphabets make repeats and crossings common.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self, bound: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) % bound
    }
    fn mutate(&mut self, center: &[String]) -> Vec<String> {
        let mut out = Vec::new();
        for line in center {
            match self.next(10) {
                0 => {}
                1 => out.push(format!("{line}'")),
                2 => {
                    out.push(format!("n{}", self.next(4)));
                    out.push(line.clone());
                }
                3 => {
                    out.push(line.clone());
                    out.push(format!("n{}", self.next(4)));
                    out.push(format!("n{}", self.next(4)));
                }
                _ => out.push(line.clone()),
            }
        }
        out
    }
}

fn pairs(map: &AlignMap, first: fn(&Row) -> Option<usize>, second: fn(&Row) -> Option<usize>) -> BTreeSet<(usize, usize)> {
    rows3(map)
        .iter()
        .filter_map(|row| Some((first(row)?, second(row)?)))
        .collect()
}

#[test]
fn invariants_hold_on_generated_inputs() {
    let mut rng = Lcg(0x5EED);
    for round in 0..300 {
        let center: Vec<String> = (0..rng.next(14))
            .map(|i| format!("c{}", if rng.next(4) == 0 { rng.next(3) } else { i + 10 }))
            .collect();
        let left = rng.mutate(&center);
        let right = rng.mutate(&center);
        fn as_refs(v: &[String]) -> Vec<&str> {
            v.iter().map(String::as_str).collect()
        }
        let (l, c, r) = (
            Side::new(&as_refs(&left)),
            Side::new(&as_refs(&center)),
            Side::new(&as_refs(&right)),
        );
        for (name, aligner) in aligners() {
            let map = aligner.align3(l.input(), c.input(), r.input());
            let context = format!("{name} round {round}: {left:?} / {center:?} / {right:?}");
            // I1: every side is an ordered partition of its lines.
            let rows = map.rows().to_vec();
            assert!(AlignMap::try_from_rows(rows.clone()).is_ok(), "{context}");
            let count = |f: fn(&Row) -> Option<usize>| rows3(&map).iter().filter_map(f).count();
            assert_eq!(count(|r| r.0), left.len(), "{context}");
            assert_eq!(count(|r| r.1), center.len(), "{context}");
            assert_eq!(count(|r| r.2), right.len(), "{context}");
            // I2: pairs are exactly those of the two 2-way alignments.
            let a = aligner.align2(l.input(), c.input());
            let b = aligner.align2(c.input(), r.input());
            let two_way = |m: &AlignMap| -> BTreeSet<(usize, usize)> {
                m.rows()
                    .iter()
                    .filter_map(|row| Some((row.left?, row.right?)))
                    .collect()
            };
            assert_eq!(pairs(&map, |r| r.0, |r| r.1), two_way(&a), "{context}");
            assert_eq!(pairs(&map, |r| r.1, |r| r.2), two_way(&b), "{context}");
            // Orphans never share a row with a center line they are not paired with,
            // and a row without a center line never crosses its neighbours.
            let mut last_center = None;
            for row in rows3(&map) {
                if let Some(center) = row.1 {
                    assert!(last_center < Some(center), "{context}");
                    last_center = Some(center);
                }
            }
        }
        // I3: left == center degenerates to the center/right alignment.
        for (name, aligner) in aligners() {
            let map = aligner.align3(c.input(), c.input(), r.input());
            let b = aligner.align2(c.input(), r.input());
            let expected: Vec<Row> = b
                .rows()
                .iter()
                .map(|row| (row.left, row.left, row.right))
                .collect();
            assert_eq!(rows3(&map), expected, "{name} round {round} (left == center)");
            let map = aligner.align3(l.input(), c.input(), c.input());
            let a = aligner.align2(l.input(), c.input());
            let expected: Vec<Row> = a
                .rows()
                .iter()
                .map(|row| (row.left, row.right, row.right))
                .collect();
            assert_eq!(rows3(&map), expected, "{name} round {round} (right == center)");
        }
    }
}
