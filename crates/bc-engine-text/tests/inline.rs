use bc_engine_text::{
    importance::{CompiledComparison, Importance, LineView},
    inline::{inline_diff, inline_diff_classified, InlineOptions},
    rules::ComparisonRules,
};
#[test]
fn word_replacement_insertion_and_joined_fragments() {
    let options = InlineOptions::default();
    let diff = inline_diff("return count + 1;", "return total + 1;", &options);
    assert_eq!(&"return count + 1;"[diff.left[0].range.clone()], "count");
    assert_eq!(&"return total + 1;"[diff.right[0].range.clone()], "total");
    let diff = inline_diff("call()", "call(, verbose=True)", &options);
    assert!(diff.left.is_empty());
    assert_eq!(
        &"call(, verbose=True)"[diff.right[0].range.clone()],
        ", verbose=True"
    );
    let diff = inline_diff("a1b2c", "x1y2c", &options);
    assert_eq!(diff.left[0].range, 0..3);
    assert_eq!(diff.right[0].range, 0..3);
    // Character-level, no word snapping (reference: `{g}etValue`).
    assert_eq!(
        inline_diff("getValue", "setValue", &options).left[0].range,
        0..1
    );
    assert!(inline_diff("same", "same", &options).left.is_empty());
}
#[test]
fn unicode_spans_and_comment_importance() {
    let diff = inline_diff(
        "中旧文",
        "中新文",
        &InlineOptions {
            word_boundaries: false,
            ..Default::default()
        },
    );
    assert_eq!(diff.left[0].range, 3..6);
    let format = bc_formats::builtin_formats()
        .iter()
        .find(|f| f.name == "CPP")
        .unwrap();
    let tokenizer = bc_formats::grammar::Tokenizer::new(format).unwrap();
    let l = "x = 1; // old";
    let r = "x = 1; // new";
    let lt = tokenizer.tokenize_line(l, &mut Default::default());
    let rt = tokenizer.tokenize_line(r, &mut Default::default());
    let mut rules = ComparisonRules::default();
    rules
        .importance
        .unimportant
        .push(bc_formats::ElementKind::Comment);
    let rules = CompiledComparison::new(&rules).unwrap();
    let diff = inline_diff_classified(
        LineView {
            text: l,
            tokens: &lt,
            format: Some(format),
            ..LineView::plain(l)
        },
        LineView {
            text: r,
            tokens: &rt,
            format: Some(format),
            ..LineView::plain(r)
        },
        &rules,
        &Default::default(),
    );
    assert!(diff
        .left
        .iter()
        .all(|s| s.importance == Importance::Unimportant));
}
#[test]
fn mixed_line_keeps_case_only_island_unimportant() {
    let l = "LOWER unchanged context old";
    let r = "lower unchanged context new";
    let mut rules = ComparisonRules::default();
    rules.importance.case_unimportant = true;
    let comparison = CompiledComparison::new(&rules).unwrap();
    let diff = inline_diff_classified(
        LineView::plain(l),
        LineView::plain(r),
        &comparison,
        &Default::default(),
    );
    assert_eq!(diff.left[0].importance, Importance::Unimportant);
    assert_eq!(diff.left.last().unwrap().importance, Importance::Important);
}

#[test]
fn format_intrinsic_case_difference_has_minor_inline_spans() {
    let format = bc_formats::builtin_formats()
        .iter()
        .find(|f| f.name == "PAS")
        .unwrap();
    let tokenizer = bc_formats::grammar::Tokenizer::new(format).unwrap();
    let lt = tokenizer.tokenize_line("BEGIN", &mut Default::default());
    let rt = tokenizer.tokenize_line("begin", &mut Default::default());
    let comparison = CompiledComparison::new(&Default::default()).unwrap();
    let diff = inline_diff_classified(
        LineView {
            text: "BEGIN",
            tokens: &lt,
            format: Some(format),
            ..LineView::plain("BEGIN")
        },
        LineView {
            text: "begin",
            tokens: &rt,
            format: Some(format),
            ..LineView::plain("begin")
        },
        &comparison,
        &Default::default(),
    );
    assert_eq!(diff.left[0].range, 0..5);
    assert_eq!(diff.left[0].importance, Importance::Unimportant);
    assert_eq!(diff.right[0].importance, Importance::Unimportant);
}

#[test]
fn half_megabyte_lines_fall_back_without_character_diff() {
    let left = "a".repeat(500_000);
    let right = "b".repeat(500_000);
    let start = std::time::Instant::now();
    let diff = inline_diff(&left, &right, &Default::default());
    assert_eq!(diff.left[0].range, 0..500_000);
    assert_eq!(diff.right[0].range, 0..500_000);
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
}

/// Reference captures (Beyond Compare 5 side-by-side HTML report, 2026-11-11): `{…}`
/// marks a difference span; adjacent reference spans are written as one.
#[test]
fn character_spans_match_reference_captures() {
    let cases = [
        ("heasdlo", "hello", "he{asd}lo", "he{l}lo"),
        (
            "return count + 1;",
            "return total + 1;",
            "return {count} + 1;",
            "return {total} + 1;",
        ),
        ("a1b2c", "x1y2c", "{a1b}2c", "{x1y}2c"),
        ("getValue", "setValue", "{g}etValue", "{s}etValue"),
        (
            "abcXdefYghi",
            "abcZdefWghi",
            "abc{X}def{Y}ghi",
            "abc{Z}def{W}ghi",
        ),
        (
            "abcXdeffYghi",
            "abcZdeffWghi",
            "abc{X}deff{Y}ghi",
            "abc{Z}deff{W}ghi",
        ),
        (
            "the quick brown fox",
            "the quack brown fix",
            "the qu{i}ck brown f{o}x",
            "the qu{a}ck brown f{i}x",
        ),
        ("abcdef", "uvwxyz", "{abcdef}", "{uvwxyz}"),
        ("中旧文", "中新文", "中{旧}文", "中{新}文"),
        ("abXcdYef", "abZcdWef", "ab{X}cd{Y}ef", "ab{Z}cd{W}ef"),
        ("ab(X.Y)cd", "ab(Z.W)cd", "ab({X.Y})cd", "ab({Z.W})cd"),
        ("aa X Y bb", "aa Z W bb", "aa {X} {Y} bb", "aa {Z} {W} bb"),
        ("xABCDE", "xFGHIJ", "x{ABCDE}", "x{FGHIJ}"),
        ("count", "mount", "{c}ount", "{m}ount"),
        ("abcd", "axcy", "a{bcd}", "a{xcy}"),
        (
            "hello world",
            "help wordl",
            "hel{lo} worl{d}",
            "hel{p} wor{d}l",
        ),
        ("ABCDEx", "FGHIJx", "{ABCDE}x", "{FGHIJ}x"),
        (
            "aaaXXXbbYYYccc",
            "aaaZZZbbWWWccc",
            "aaa{XXX}bb{YYY}ccc",
            "aaa{ZZZ}bb{WWW}ccc",
        ),
        (
            "aaaXXXbYYYccc",
            "aaaZZZbWWWccc",
            "aaa{XXXbYYY}ccc",
            "aaa{ZZZbWWW}ccc",
        ),
        ("aXbYc", "aZbWc", "a{XbY}c", "a{ZbW}c"),
        ("abcdef", "abQcRdef", "ab{c}def", "ab{QcR}def"),
    ];
    fn mark(text: &str, spans: &[bc_engine_text::inline::InlineSpan]) -> String {
        let mut out = String::new();
        let mut at = 0;
        for span in spans {
            out.push_str(&text[at..span.range.start]);
            out.push('{');
            out.push_str(&text[span.range.clone()]);
            out.push('}');
            at = span.range.end;
        }
        out.push_str(&text[at..]);
        out.replace("}{", "")
    }
    let mut failures = Vec::new();
    for (left, right, want_left, want_right) in cases {
        let diff = inline_diff(left, right, &InlineOptions::default());
        let got = (mark(left, &diff.left), mark(right, &diff.right));
        if got != (want_left.to_string(), want_right.to_string()) {
            failures.push(format!(
                "{left} | {right}: got {got:?}, want ({want_left:?}, {want_right:?})"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
