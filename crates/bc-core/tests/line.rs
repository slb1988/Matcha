use bc_core::line::{EndingStyle, LineEnding, LineList, LineParser, LineWriter};

#[test]
fn byte_exact_round_trips() {
    for bytes in [
        &b""[..],
        b"a",
        b"a\r\nb\r\n",
        b"a\nb\n",
        b"a\rb\r",
        b"one\r\ntwo\nthree\rlast",
        b"\n",
        b"\r\n",
        b"\r\r\n\n",
        b"\xff\n\0",
    ] {
        let lines = LineList::from_bytes(bytes.to_vec());
        assert_eq!(lines.to_bytes(), bytes);
        assert_eq!(lines.iter().count(), lines.len());
    }
    assert_eq!(LineList::from_bytes(vec![]).len(), 0);
    assert_eq!(LineList::from_bytes(b"a\n".to_vec()).len(), 1);
    let no_eol = LineList::from_bytes(b"a".to_vec());
    assert_eq!(no_eol.len(), 1);
    assert_eq!(no_eol.ending(0), Some(LineEnding::None));
    assert_eq!(no_eol.line_bytes(1), None);
}

#[test]
fn style_and_dominant_ending_are_distinct() {
    for (bytes, style, dominant) in [
        (&b"one\n\ntwo\r\n"[..], EndingStyle::Mixed, LineEnding::Lf),
        (b"one\r\ntwo\r\n", EndingStyle::Windows, LineEnding::CrLf),
        (b"a\rb\r", EndingStyle::Mac, LineEnding::Cr),
        (b"a\n", EndingStyle::Unix, LineEnding::Lf),
        (b"none", EndingStyle::Windows, LineEnding::CrLf),
        (b"a\nb\r", EndingStyle::Mixed, LineEnding::Lf),
    ] {
        let lines = LineList::from_bytes(bytes.to_vec());
        assert_eq!(lines.ending_style(), style);
        assert_eq!(lines.dominant_ending(), dominant);
    }
}

#[test]
fn parser_options_and_writer_override() {
    let (lines, style) = LineParser {
        accept_cr: false,
        limit: 0,
    }
    .parse(b"a\rb\r\nc");
    assert_eq!(style, EndingStyle::Windows);
    assert_eq!(lines[0].span, 0..3);
    assert_eq!(lines[1].ending, LineEnding::None);
    let (limited, _) = LineParser {
        accept_cr: true,
        limit: 2,
    }
    .parse(b"abcd\n");
    assert_eq!(limited[0].span, 0..2);
    assert_eq!(limited[0].ending, LineEnding::None);
    let mut out = Vec::new();
    LineWriter {
        force_ending: Some(LineEnding::Lf),
    }
    .write(&LineList::from_bytes(b"a\r\nb\rc".to_vec()), &mut out)
    .unwrap();
    assert_eq!(out, b"a\nb\nc\n");
    let mut full = &mut [0u8; 0][..];
    assert!(LineWriter::default()
        .write(&LineList::from_bytes(b"a".to_vec()), &mut full)
        .is_err());
}
