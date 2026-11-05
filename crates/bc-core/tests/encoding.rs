use bc_core::encoding::{decode, decode_with_errors, detect, encode, BomPolicy, Encoding};

#[test]
fn utf_bom_and_no_bom_round_trips() {
    let text = "Hello 中文 🙂\r\nlast";
    for encoding in [Encoding::Utf8, Encoding::Utf16Le, Encoding::Utf16Be] {
        for had_bom in [true, false] {
            let bytes = encode(text, encoding, BomPolicy::Preserve, had_bom).unwrap();
            let detected = detect(&bytes, 1252);
            if had_bom || encoding == Encoding::Utf8 {
                assert_eq!(detected.encoding, encoding);
                assert_eq!(detected.has_bom, had_bom);
            }
            assert_eq!(decode(&bytes, encoding).unwrap(), text);
            assert_eq!(decode_with_errors(&bytes, encoding).unwrap().1, 0);
            assert_eq!(
                encode(text, encoding, BomPolicy::Preserve, had_bom).unwrap(),
                bytes
            );
            assert_eq!(
                encode(text, encoding, BomPolicy::Never, had_bom).unwrap(),
                encode(text, encoding, BomPolicy::Never, false).unwrap()
            );
            assert_eq!(
                encode(text, encoding, BomPolicy::Always, had_bom).unwrap(),
                encode(text, encoding, BomPolicy::Preserve, true).unwrap()
            );
        }
    }
    assert_eq!(detect(b"", 936).encoding, Encoding::Utf8);
}

#[test]
fn legacy_pages_and_explicit_errors() {
    for (text, page, bytes) in [
        ("中文", 936, &b"\xd6\xd0\xce\xc4"[..]),
        ("café €", 1252, b"caf\xe9 \x80"),
    ] {
        assert_eq!(detect(bytes, page).encoding, Encoding::CodePage(page));
        assert_eq!(decode(bytes, Encoding::CodePage(page)).unwrap(), text);
        assert_eq!(
            encode(text, Encoding::CodePage(page), BomPolicy::Always, true).unwrap(),
            bytes
        );
    }
    for page in [
        866, 874, 932, 936, 949, 950, 1250, 1251, 1252, 1253, 1254, 1255, 1256, 1257, 1258, 20866,
        21866, 54936,
    ] {
        let bytes = encode("ASCII\n", Encoding::CodePage(page), BomPolicy::Never, false).unwrap();
        assert_eq!(decode(&bytes, Encoding::CodePage(page)).unwrap(), "ASCII\n");
    }
    assert!(decode(b"x", Encoding::CodePage(37))
        .unwrap_err()
        .to_string()
        .contains("37"));
    assert!(encode("中", Encoding::CodePage(1252), BomPolicy::Never, false).is_err());
    assert_eq!(
        decode(b"\xef\xbb\xbfx", Encoding::CodePage(65001)).unwrap(),
        "x"
    );
}

#[test]
fn malformed_sequences_have_visible_placeholders_and_exact_counts() {
    assert_eq!(
        decode_with_errors(b"a\xffb\xc3", Encoding::Utf8).unwrap(),
        ("a�b�".into(), 2)
    );
    assert_eq!(
        decode_with_errors("�".as_bytes(), Encoding::Utf8).unwrap(),
        ("�".into(), 0)
    );
    assert_eq!(
        decode_with_errors(b"\xff\xfe\x00\xd8\x41\x00\xff", Encoding::Utf16Le).unwrap(),
        ("�A�".into(), 2)
    );
    assert_eq!(
        decode_with_errors(b"\xfe\xff\xdc\x00", Encoding::Utf16Be).unwrap(),
        ("�".into(), 1)
    );
    assert_eq!(
        decode_with_errors(b"\x81", Encoding::CodePage(936)).unwrap(),
        ("�".into(), 1)
    );
    assert_eq!(
        decode_with_errors(b"\x81\x20", Encoding::CodePage(936)).unwrap(),
        ("� ".into(), 1)
    );
}

#[test]
fn binary_sniff_is_bounded_and_bom_has_priority() {
    assert_eq!(detect(&[0; 100], 1252).encoding, Encoding::Binary);
    assert_eq!(detect(&[1; 100], 1252).encoding, Encoding::Binary);
    assert_eq!(detect(b"a\t\r\n\x0c", 1252).encoding, Encoding::Utf8);
    let mut bytes = vec![b'a'; 100];
    bytes[0] = 0; // exactly 1%, threshold is strictly greater
    assert_eq!(detect(&bytes, 1252).encoding, Encoding::Utf8);
    bytes[1] = 0;
    assert_eq!(detect(&bytes, 1252).encoding, Encoding::Binary);
    let mut text = vec![b'x'; 8192];
    text.extend_from_slice(&[0; 100]);
    assert_eq!(detect(&text, 1252).encoding, Encoding::Utf8);
    text.push(0xff); // validate UTF-8 beyond the binary-sniff window
    assert_eq!(detect(&text, 1252).encoding, Encoding::CodePage(1252));
    assert_eq!(detect(b"\xff\xfe\0\0", 936).encoding, Encoding::Utf16Le);
    assert!(decode(b"bin", Encoding::Binary).is_err());
    assert!(encode("bin", Encoding::Binary, BomPolicy::Never, false).is_err());
}
