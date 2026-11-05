use bc_core::{
    cancel::NeverCancel,
    encoding::{encode, BomPolicy, Encoding},
};
use bc_engine_table::{
    regional::{parse_date, parse_number, DAY_MS},
    source::{read_text, write_delimited, ReadOptions, Records},
    Cell, CellValue, Sheet, Style,
};
use bc_formats::{
    model::{DateOrder, HeaderMode, RegionalSettings, TableType},
    TableOptions,
};
use std::{
    io::{self, Cursor, Read},
    sync::Arc,
};
fn parse(text: &str, format: TableOptions) -> Sheet {
    read_text(
        Cursor::new(text),
        &ReadOptions {
            format,
            header: Some(HeaderMode::Data),
            ..Default::default()
        },
        &NeverCancel,
    )
    .unwrap()
    .sheet
}
#[test]
fn model_is_columnar_and_snapshot_isolation_and_styles() {
    let mut s = Sheet::new("data");
    s.push_row(vec![Cell::number(1.0), Cell::text("x")]);
    s.push_row(vec![Cell::number(2.0)]);
    assert_eq!(s.column(1).unwrap()[1], Cell::default());
    let old = s.clone();
    let rev = old.revision();
    let style = Style {
        number_format: Some("0.00".into()),
    };
    assert_eq!(s.intern_style(style.clone()), s.intern_style(style));
    s.set_cell(0, 0, Cell::number(3.0)).unwrap();
    assert_eq!(old.cell(0, 0).unwrap(), &Cell::number(1.0));
    assert_ne!(rev, s.revision());
    fn send_sync<T: Send + Sync>() {}
    send_sync::<Sheet>();
}
#[test]
fn rfc4180_multiline_quotes_empty_fields_and_mixed_endings() {
    let input = "id,name,note\r\n1,\"a,b\",\"line1\nline2\"\n2,\"a\"\"b\",\r3,,last";
    let s = parse(input, TableOptions::default());
    assert_eq!(s.row_count(), 4);
    assert_eq!(s.cell(1, 2).unwrap().display(), "line1\nline2");
    assert_eq!(s.cell(2, 1).unwrap().display(), "a\"b");
    let mut saved = Vec::new();
    write_delimited(&s, &mut saved).unwrap();
    assert_eq!(saved, input.as_bytes());
    assert!(read_text(
        Cursor::new("\"broken"),
        &ReadOptions::default(),
        &NeverCancel
    )
    .is_err());
}
#[test]
fn delimited_options_and_fixed_unicode_characters() {
    let format = TableOptions {
        delimiters: vec![' '],
        consecutive_delimiters: true,
        ..Default::default()
    };
    assert_eq!(parse("a   b  c", format).column_count(), 3);
    let s = parse(
        " a ; \" b ; c \" ;d",
        TableOptions {
            delimiters: vec![';'],
            trim_surrounding_whitespace: true,
            ..Default::default()
        },
    );
    assert_eq!(s.cell(0, 0).unwrap().display(), "a");
    assert_eq!(s.cell(0, 1).unwrap().display(), " b ; c ");
    let s = parse(
        "猫🙂a xy\n短",
        TableOptions {
            table_type: TableType::Fixed,
            column_widths: vec![3, 2, 3],
            ..Default::default()
        },
    );
    assert_eq!(s.cell(0, 0).unwrap().display(), "猫🙂a");
    assert_eq!(s.cell(1, 1).unwrap(), &Cell::default());
}
#[test]
fn detection_headers_overrides_and_regional_numbers_dates() {
    let r = read_text(
        Cursor::new("id\tname\tprice\n1\tapple\t1.00\n2\tpear\t2.00\n"),
        &ReadOptions {
            format: TableOptions {
                detect_delimiter: true,
                ..Default::default()
            },
            ..Default::default()
        },
        &NeverCancel,
    )
    .unwrap();
    assert_eq!(r.diagnostics.detected_delimiter, Some('\t'));
    assert!(r.diagnostics.header_detected);
    assert_eq!(r.sheet.row_count(), 2);
    assert_eq!(r.sheet.headers.as_ref().unwrap()[0], "id");
    assert_eq!(
        parse("id,name\n1,a", TableOptions::default()).row_count(),
        2
    );
    let regional = RegionalSettings {
        use_system: false,
        decimal: ',',
        thousands: Some('.'),
        date_order: DateOrder::Dmy,
        date_separator: '/',
    };
    assert_eq!(parse_number("1.234,50", &regional), Some(1234.5));
    assert_eq!(parse_number("12.34,50", &regional), None);
    assert_eq!(
        parse_date("02/01/2026", &regional).unwrap() - parse_date("01/01/2026", &regional).unwrap(),
        DAY_MS
    );
    assert!(parse_date("31/02/2026", &regional).is_none());
    let r = read_text(
        Cursor::new("1.234,50;01/02/2026"),
        &ReadOptions {
            format: TableOptions {
                delimiters: vec![';'],
                regional,
                ..Default::default()
            },
            ..Default::default()
        },
        &NeverCancel,
    )
    .unwrap();
    assert_eq!(r.sheet.cell(0, 0).unwrap().value, CellValue::Number(1234.5));
    assert_eq!(r.sheet.cell(0, 1).unwrap().display(), "2026-02-01");
}
#[test]
fn encodings_ctrl_z_and_whole_input_detection() {
    for encoding in [
        Encoding::Utf8,
        Encoding::Utf16Le,
        Encoding::Utf16Be,
        Encoding::CodePage(936),
    ] {
        let bytes = encode("编号,值\r\n1,猫\r\n", encoding, BomPolicy::Always, false).unwrap();
        let r = read_text(
            Cursor::new(&bytes),
            &ReadOptions {
                encoding: Some(encoding),
                header: Some(HeaderMode::Names),
                ..Default::default()
            },
            &NeverCancel,
        )
        .unwrap();
        assert_eq!(r.sheet.cell(0, 1).unwrap().display(), "猫");
        let mut out = Vec::new();
        write_delimited(&r.sheet, &mut out).unwrap();
        assert_eq!(out, bytes);
    }
    let r = read_text(
        Cursor::new("a,b\n1,2\u{1a}ignored\n"),
        &ReadOptions {
            ctrl_z_eof: true,
            ..Default::default()
        },
        &NeverCancel,
    )
    .unwrap();
    assert_eq!(r.sheet.row_count(), 1);
    let tiny = read_text(
        Cursor::new(b"1\x1a\0\0\xff"),
        &ReadOptions {
            ctrl_z_eof: true,
            ..Default::default()
        },
        &NeverCancel,
    )
    .unwrap();
    assert_eq!(tiny.sheet.cell(0, 0).unwrap(), &Cell::number(1.0));
    let mut bytes = vec![b'a'; 9000];
    bytes.extend_from_slice(b",caf\xe9");
    let r = read_text(
        Cursor::new(bytes),
        &ReadOptions {
            header: Some(HeaderMode::Data),
            ..Default::default()
        },
        &NeverCancel,
    )
    .unwrap();
    assert_eq!(r.sheet.cell(0, 1).unwrap().display(), "café");
}
#[test]
fn saving_one_field_preserves_lexical_spelling_and_delimiters() {
    let input = "id ; price ; name\r\n001 ; 1.00 ; \"a\"\n002;2.000;b";
    let mut s = read_text(
        Cursor::new(input),
        &ReadOptions {
            format: TableOptions {
                delimiters: vec![';'],
                trim_surrounding_whitespace: true,
                ..Default::default()
            },
            ..Default::default()
        },
        &NeverCancel,
    )
    .unwrap()
    .sheet;
    s.set_cell(0, 2, Cell::text("a;b")).unwrap();
    let mut out = Vec::new();
    write_delimited(&s, &mut out).unwrap();
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "id ; price ; name\r\n001 ; 1.00 ;\"a;b\"\n002;2.000;b"
    );
}
#[test]
fn parsing_cancels_without_returning_partial_model() {
    assert!(matches!(
        read_text(Cursor::new("a,b\n1,2"), &ReadOptions::default(), &|| true),
        Err(bc_core::CoreError::Cancelled)
    ));
    let text: Arc<str> = "x".into();
    assert_eq!(Cell::text(text).display(), "x");
}
/// Synthetic 1 GiB stream: no file fixture and never allocates the full source.
#[test]
#[ignore = "1 GiB streaming stress; run explicitly in release"]
fn gigabyte_record_stream_is_bounded() {
    struct Repeated {
        remaining: u64,
    }
    impl Read for Repeated {
        fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
            let n = b.len().min(self.remaining as usize);
            for v in &mut b[..n] {
                *v = b'a';
            }
            // Lines are exactly 4096 bytes, independently of reader buffer size.
            for (i, v) in b[..n].iter_mut().enumerate() {
                if (self.remaining - i as u64) % 4096 == 1 {
                    *v = b'\n';
                }
            }
            self.remaining -= n as u64;
            Ok(n)
        }
    }
    let mut records = Records::new(
        io::BufReader::new(Repeated { remaining: 1 << 30 }),
        TableOptions::default(),
        false,
    )
    .unwrap();
    let mut rows = 0;
    while let Some(record) = records.next_record(&NeverCancel).unwrap() {
        assert_eq!(record.fields[0].len(), 4095);
        rows += 1;
    }
    assert_eq!(rows, 262_144);
}
