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
    assert_eq!(
        inline_diff("getValue", "setValue", &options).left[0].range,
        0..8
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
fn half_megabyte_lines_fall_back_without_character_diff() {
    let left = "a".repeat(500_000);
    let right = "b".repeat(500_000);
    let start = std::time::Instant::now();
    let diff = inline_diff(&left, &right, &Default::default());
    assert_eq!(diff.left[0].range, 0..500_000);
    assert_eq!(diff.right[0].range, 0..500_000);
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
}
