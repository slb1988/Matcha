//! Text encoding detection and conversion (FR-T5: UTF-8/16 + common ANSI, BOM handling).

/// Encodings the text engine can read and write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Encoding {
    #[default]
    Utf8,
    Utf16Le,
    Utf16Be,
    /// Windows/ANSI code page (e.g. 1252, 936).
    CodePage(u16),
    /// Detected as binary – shown in Hex compare instead.
    Binary,
}

/// Byte order mark handling for save.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BomPolicy {
    /// Keep whatever the file had.
    #[default]
    Preserve,
    Always,
    Never,
}

/// Result of sniffing a buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    pub encoding: Encoding,
    pub has_bom: bool,
    /// Confidence 0..=100 (BC prompts via `TFILECODEPAGEERRORDIALOG` when low).
    pub confidence: u8,
}

/// BOM wins first. Without a BOM, the first 8 KiB is binary if >1% NUL or
/// more than 10% non-printable ASCII controls (excluding TAB, CR, LF, FF). Otherwise
/// validate the *entire* UTF-8 input before falling back to the user's code page.
pub fn detect(bytes: &[u8], default_code_page: u16) -> Detection {
    for encoding in [Encoding::Utf8, Encoding::Utf16Le, Encoding::Utf16Be] {
        if bytes.starts_with(bom_bytes(encoding)) {
            return Detection {
                encoding,
                has_bom: true,
                confidence: 100,
            };
        }
    }
    let sample = &bytes[..bytes.len().min(8192)];
    let nul = sample.iter().filter(|&&b| b == 0).count();
    let controls = sample
        .iter()
        .filter(|&&b| (b < 32 && !matches!(b, b'\t' | b'\r' | b'\n' | 12)) || b == 127)
        .count();
    let (encoding, confidence) = if nul * 100 > sample.len() || controls * 10 > sample.len() {
        (Encoding::Binary, 100)
    } else if std::str::from_utf8(bytes).is_ok() {
        (Encoding::Utf8, 100)
    } else {
        (Encoding::CodePage(default_code_page), 50)
    };
    Detection {
        encoding,
        has_bom: false,
        confidence,
    }
}

/// Decode, replacing malformed sequences with U+FFFD. Use [`decode_with_errors`]
/// when the caller needs the number of replacements for its status display.
pub fn decode(bytes: &[u8], encoding: Encoding) -> Result<String, crate::CoreError> {
    decode_with_errors(bytes, encoding).map(|(text, _)| text)
}

/// Returns UTF-8 and the number of malformed sequences, not the number of literal
/// U+FFFD characters. Only a BOM matching the requested encoding is consumed.
pub fn decode_with_errors(
    bytes: &[u8],
    encoding: Encoding,
) -> Result<(String, usize), crate::CoreError> {
    let encoding = canonical(encoding);
    let bom = bom_bytes(encoding);
    let bytes = if !bom.is_empty() {
        bytes.strip_prefix(bom).unwrap_or(bytes)
    } else {
        bytes
    };
    let mut text = String::with_capacity(bytes.len());
    let mut errors = 0;
    match encoding {
        Encoding::Utf8 => {
            let mut rest = bytes;
            while !rest.is_empty() {
                match std::str::from_utf8(rest) {
                    Ok(valid) => {
                        text.push_str(valid);
                        break;
                    }
                    Err(e) => {
                        text.push_str(
                            std::str::from_utf8(&rest[..e.valid_up_to()])
                                .expect("validated prefix"),
                        );
                        text.push('\u{fffd}');
                        errors += 1;
                        let consumed =
                            e.valid_up_to() + e.error_len().unwrap_or(rest.len() - e.valid_up_to());
                        rest = &rest[consumed..];
                    }
                }
            }
        }
        Encoding::Utf16Le | Encoding::Utf16Be => {
            let units = bytes.as_chunks::<2>().0.iter().map(|c| {
                if encoding == Encoding::Utf16Le {
                    u16::from_le_bytes([c[0], c[1]])
                } else {
                    u16::from_be_bytes([c[0], c[1]])
                }
            });
            for scalar in char::decode_utf16(units) {
                text.push(scalar.unwrap_or_else(|_| {
                    errors += 1;
                    '\u{fffd}'
                }));
            }
            if bytes.len() % 2 != 0 {
                text.push('\u{fffd}');
                errors += 1;
            }
        }
        Encoding::CodePage(page) => {
            let mut decoder = code_page(page)?.new_decoder_without_bom_handling();
            let mut rest = bytes;
            loop {
                text.reserve(
                    decoder
                        .max_utf8_buffer_length_without_replacement(rest.len())
                        .unwrap_or(rest.len().saturating_mul(4))
                        .max(16),
                );
                let (result, read) =
                    decoder.decode_to_string_without_replacement(rest, &mut text, true);
                rest = &rest[read..];
                match result {
                    encoding_rs::DecoderResult::InputEmpty => break,
                    encoding_rs::DecoderResult::OutputFull => continue,
                    encoding_rs::DecoderResult::Malformed(_, _) => {
                        text.push('\u{fffd}');
                        errors += 1;
                    }
                }
            }
        }
        Encoding::Binary => {
            return Err(crate::CoreError::Invalid(
                "binary input cannot be decoded as text".into(),
            ))
        }
    }
    Ok((text, errors))
}

/// Encode without silently losing unrepresentable characters. Preserve uses the
/// original detection's `has_bom`; ANSI encodings never receive a Unicode BOM.
pub fn encode(
    text: &str,
    encoding: Encoding,
    bom: BomPolicy,
    original_has_bom: bool,
) -> Result<Vec<u8>, crate::CoreError> {
    let encoding = canonical(encoding);
    let mut output = Vec::new();
    if bom == BomPolicy::Always || (bom == BomPolicy::Preserve && original_has_bom) {
        output.extend_from_slice(bom_bytes(encoding));
    }
    match encoding {
        Encoding::Utf8 => output.extend_from_slice(text.as_bytes()),
        Encoding::Utf16Le | Encoding::Utf16Be => {
            for unit in text.encode_utf16() {
                let bytes = if encoding == Encoding::Utf16Le {
                    unit.to_le_bytes()
                } else {
                    unit.to_be_bytes()
                };
                output.extend_from_slice(&bytes);
            }
        }
        Encoding::CodePage(page) => {
            let (bytes, _, had_errors) = code_page(page)?.encode(text);
            if had_errors {
                return Err(crate::CoreError::Invalid(format!(
                    "text is not representable in code page {page}"
                )));
            }
            output.extend_from_slice(&bytes);
        }
        Encoding::Binary => {
            return Err(crate::CoreError::Invalid(
                "cannot encode text as binary".into(),
            ))
        }
    }
    Ok(output)
}

fn canonical(encoding: Encoding) -> Encoding {
    match encoding {
        Encoding::CodePage(65001) => Encoding::Utf8,
        Encoding::CodePage(1200) => Encoding::Utf16Le,
        Encoding::CodePage(1201) => Encoding::Utf16Be,
        other => other,
    }
}

fn bom_bytes(encoding: Encoding) -> &'static [u8] {
    match encoding {
        Encoding::Utf8 => b"\xef\xbb\xbf",
        Encoding::Utf16Le => b"\xff\xfe",
        Encoding::Utf16Be => b"\xfe\xff",
        _ => b"",
    }
}

/// Explicit Windows code-page to WHATWG label map; unsupported pages are errors.
fn code_page(page: u16) -> Result<&'static encoding_rs::Encoding, crate::CoreError> {
    let label: &[u8] = match page {
        866 => b"ibm866",
        874 => b"windows-874",
        932 => b"shift_jis",
        936 => b"gbk",
        949 => b"euc-kr",
        950 => b"big5",
        1250 => b"windows-1250",
        1251 => b"windows-1251",
        1252 => b"windows-1252",
        1253 => b"windows-1253",
        1254 => b"windows-1254",
        1255 => b"windows-1255",
        1256 => b"windows-1256",
        1257 => b"windows-1257",
        1258 => b"windows-1258",
        20866 => b"koi8-r",
        21866 => b"koi8-u",
        54936 => b"gb18030",
        _ => {
            return Err(crate::CoreError::Invalid(format!(
                "unsupported code page {page}"
            )))
        }
    };
    Ok(encoding_rs::Encoding::for_label(label).expect("built-in encoding label"))
}
