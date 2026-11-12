//! hex-compare 黄金语料（任务 3.1）与 hex-report 对拍（任务 3.2）。
//!
//! 语料由 `golden/hex/corpus.rs` 确定性生成，与 `docs/reference-parity/hex-compare/corpus/`
//! 已提交文件逐字节一致（同一份字节 = 同一条证据链：截图、统计、黄金期望）。

#[path = "golden/hex/corpus.rs"]
mod corpus;

use bc_core::align::{ByteAlign, ByteAlignStrategy};
use bc_core::DiffKind;
use std::path::PathBuf;

fn docs_corpus(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/reference-parity/hex-compare/corpus")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn python_compatible_prng_matches_cpython_vectors() {
    // CPython 3.x 实测：seed(42) → [57, 12, 140, 125, 114, 71]；seed(7) → [165, 77, 202, 24]。
    let mut rng = corpus::PyRandom::new(42);
    let v: Vec<u8> = (0..6).map(|_| rng.randrange_256()).collect();
    assert_eq!(v, [57, 12, 140, 125, 114, 71]);
    let mut rng = corpus::PyRandom::new(7);
    let v: Vec<u8> = (0..4).map(|_| rng.randrange_256()).collect();
    assert_eq!(v, [165, 77, 202, 24]);
}

#[test]
fn generated_corpus_is_byte_identical_to_committed_evidence() {
    let (left, right) = corpus::primary();
    assert_eq!(left.len(), 4160);
    assert_eq!(right.len(), 4144);
    assert_eq!(left, docs_corpus("left.bin"));
    assert_eq!(right, docs_corpus("right.bin"));
    let (l2, r2) = corpus::odd_insert();
    assert_eq!(l2.len(), 2048);
    assert_eq!(r2.len(), 2053);
    assert_eq!(l2, docs_corpus("l2.bin"));
    assert_eq!(r2, docs_corpus("r2.bin"));
}

#[test]
fn primary_corpus_alignment_stats() {
    let (left, right) = corpus::primary();
    let map = ByteAlign.align(&left, &right, ByteAlignStrategy::Complete);
    let (mut same, mut diff, mut lo, mut ro) = (0, 0, 0, 0);
    for row in map.rows() {
        match row.kind {
            DiffKind::Same => same += 1,
            DiffKind::Changed => diff += 1,
            DiffKind::LeftOnly => lo += 1,
            DiffKind::RightOnly => ro += 1,
        }
    }
    assert_eq!((same, diff, lo, ro), (4120, 8, 32, 16));
}

