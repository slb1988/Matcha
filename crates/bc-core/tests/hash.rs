use bc_core::{
    hash::{Crc32, LineHasher},
    line::LineList,
};

#[test]
fn whitespace_flags_are_independent() {
    let raw = LineHasher::default();
    assert_ne!(raw.hash_line(b"a b"), raw.hash_line(b"ab"));
    let lead = LineHasher {
        ignore_leading_whitespace: true,
        ..raw
    };
    assert_eq!(lead.hash_line(b" \tA"), lead.hash_line(b"A"));
    assert_ne!(lead.hash_line(b"A "), lead.hash_line(b"A"));
    let trail = LineHasher {
        ignore_trailing_whitespace: true,
        ..raw
    };
    assert_eq!(trail.hash_line(b"A \t"), trail.hash_line(b"A"));
    assert_ne!(trail.hash_line(b" A"), trail.hash_line(b"A"));
    let embedded = LineHasher {
        ignore_embedded_whitespace: true,
        ..raw
    };
    assert_eq!(embedded.hash_line(b"a \t b"), embedded.hash_line(b"ab"));
    assert_ne!(embedded.hash_line(b" ab"), embedded.hash_line(b"ab"));
    assert_ne!(embedded.hash_line(b"ab "), embedded.hash_line(b"ab"));
    assert_eq!(lead.hash_line(b" \t"), lead.hash_line(b""));
}

#[test]
fn unicode_case_and_whitespace_and_raw_bytes() {
    let normal = LineHasher {
        ignore_leading_whitespace: true,
        ignore_embedded_whitespace: true,
        ignore_trailing_whitespace: true,
        ignore_case: true,
    };
    assert_eq!(
        normal.hash_line("　Ä B\t".as_bytes()),
        normal.hash_line("äb".as_bytes())
    );
    assert_ne!(normal.hash_line(b"abc"), normal.hash_line(b"abd"));
    assert_ne!(normal.hash_line(b"\xff"), normal.hash_line(b"\xfe"));
    assert_eq!(normal.hash_line(b" A\xff "), normal.hash_line(b"a\xff"));
    let lines = LineList::from_bytes(b"AB\r\nab".to_vec());
    let hashes = normal.hash_lines(&lines);
    assert_eq!(hashes.len(), 2);
    assert_eq!(hashes[0], hashes[1]);
    assert_eq!(
        LineHasher::default().hash_line(b"").0,
        0xef46_db37_51d8_e999
    );
}

#[test]
fn ieee_crc_vectors_and_streaming() {
    for (bytes, expected) in [
        (&b""[..], 0),
        (b"123456789", 0xcbf4_3926),
        (b"The quick brown fox jumps over the lazy dog", 0x414f_a339),
    ] {
        let mut crc = Crc32::new();
        crc.update(bytes);
        assert_eq!(crc.finish(), expected);
        let mut streaming = Crc32::default();
        for chunk in bytes.chunks(2) {
            streaming.update(chunk);
        }
        assert_eq!(streaming.finish(), expected);
    }
}
