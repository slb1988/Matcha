use bc_core::{
    encoding::{self, BomPolicy, Encoding},
    line::LineEnding,
};
use bc_engine_text::TextContent;
#[test]
fn encodings_bom_and_mixed_endings_round_trip() {
    for encoding in [
        Encoding::Utf8,
        Encoding::Utf16Le,
        Encoding::Utf16Be,
        Encoding::CodePage(936),
        Encoding::CodePage(1252),
    ] {
        let text = if encoding == Encoding::CodePage(1252) {
            "café\r\ntwo\nlast"
        } else {
            "中文\r\ntwo\nlast"
        };
        for bom in [false, true] {
            let bytes = encoding::encode(text, encoding, BomPolicy::Preserve, bom).unwrap();
            let mut content = TextContent::from_bytes_with_encoding(
                bytes.clone(),
                None,
                match encoding {
                    Encoding::CodePage(p) => p,
                    _ => 1252,
                },
                Some(encoding),
            )
            .unwrap();
            assert_eq!(content.to_bytes().unwrap(), bytes);
            content.replace_line(1, b"changed").unwrap();
            assert_eq!(
                encoding::decode(&content.to_bytes().unwrap(), encoding).unwrap(),
                text.replace("two", "changed")
            );
            content.edits.undo();
            assert_eq!(content.to_bytes().unwrap(), bytes);
            content.convert_endings(LineEnding::Lf).unwrap();
            assert_eq!(content.lines.borrow().ending(0), Some(LineEnding::Lf));
            content.edits.undo();
            assert_eq!(content.to_bytes().unwrap(), bytes);
        }
    }
    assert!(TextContent::from_bytes(vec![0; 100], None).is_err());
    assert!(TextContent::from_bytes(b"\xef\xbb\xbf".to_vec(), None)
        .unwrap()
        .lines
        .borrow()
        .is_empty());
}
#[test]
fn manual_encoding_counts_errors_and_format_tokens_are_cached_by_revision() {
    let mut content = TextContent::from_bytes(b"caf\xe9".to_vec(), None).unwrap();
    assert_eq!(content.encoding, Encoding::CodePage(1252));
    content.reload_encoding(Encoding::Utf8).unwrap();
    assert_eq!(content.decoding_errors, 1);
    let format = bc_formats::builtin_formats()
        .iter()
        .find(|f| f.name == "CPP")
        .unwrap()
        .clone();
    let mut content =
        TextContent::from_bytes(b"/* first\nlast */ int a;".to_vec(), Some(format)).unwrap();
    let before = content.snapshot();
    let tokens = before.tokens(1);
    assert!(tokens
        .iter()
        .any(|t| t.element == bc_formats::ElementKind::Comment));
    let scanned = before.syntax.scanned_lines();
    content.snapshot().tokens(1);
    assert_eq!(before.syntax.scanned_lines(), scanned);
    content.replace_line(0, b"int first;").unwrap();
    let after = content.snapshot();
    assert_ne!(after.lines.revision(), before.lines.revision());
    assert!(!after
        .tokens(1)
        .iter()
        .any(|t| t.element == bc_formats::ElementKind::Comment));
    assert!(before
        .tokens(1)
        .iter()
        .any(|t| t.element == bc_formats::ElementKind::Comment));
}
#[test]
fn materialized_save_preserves_undo_redo_while_a_snapshot_is_alive() {
    let directory =
        std::env::temp_dir().join(format!("matcha-content-save-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("source.txt");
    let other = directory.join("other.txt");
    std::fs::write(&path, b"first\nlast").unwrap();
    let mut content = TextContent::open(&path, None, 1252).unwrap();
    let external = content.snapshot();
    content.replace_line(0, b"changed").unwrap();
    // Small files are read into memory: a live snapshot holds no file handle, so the
    // same-path save succeeds and the snapshot keeps the content it was taken from.
    content.materialize_and_save(&path).unwrap();
    let before: Vec<&[u8]> = external.lines.iter().map(|(text, _)| text).collect();
    assert_eq!(before, [&b"first"[..], &b"last"[..]]);
    drop(external);
    assert!(!content.is_dirty());
    assert_eq!(std::fs::read(&path).unwrap(), b"changed\nlast");
    content.edits.undo();
    assert_eq!(content.to_bytes().unwrap(), b"first\nlast");
    content.edits.redo();
    content.save_as(&other).unwrap();
    assert_eq!(std::fs::read(&other).unwrap(), b"changed\nlast");
    drop(content);
    std::fs::remove_dir_all(directory).unwrap();
}
#[test]
fn small_files_keep_no_handle_so_other_programs_can_save_them() {
    let directory =
        std::env::temp_dir().join(format!("matcha-content-unlocked-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("source.txt");
    let replacement = directory.join("replacement.txt");
    std::fs::write(&path, b"first\nlast").unwrap();
    let content = TextContent::open(&path, None, 1252).unwrap();
    // Overwrite in place, replace by rename, then delete: all succeed while open.
    std::fs::write(&path, b"overwritten").unwrap();
    std::fs::write(&replacement, b"renamed over").unwrap();
    std::fs::rename(&replacement, &path).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"renamed over");
    std::fs::remove_file(&path).unwrap();
    assert_eq!(content.to_bytes().unwrap(), b"first\nlast");
    drop(content);
    std::fs::remove_dir_all(directory).unwrap();
}
