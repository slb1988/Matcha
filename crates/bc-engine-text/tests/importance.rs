use bc_engine_text::{
    importance::{propagate_block, CompiledComparison, Importance as I, LineView},
    rules::ComparisonRules,
};
use bc_formats::{
    grammar::{LineState, Tokenizer},
    text_rules::{Replacement, UnimportantText},
    ElementKind as E,
};
fn rules() -> ComparisonRules {
    ComparisonRules::default()
}
#[test]
fn token_case_and_element_importance_follow_specs() {
    let format = bc_formats::builtin_formats()
        .iter()
        .find(|f| f.name == "PAS")
        .unwrap();
    let tokenizer = Tokenizer::new(format).unwrap();
    let l = "BEGIN // old";
    let r = "begin // new";
    let lt = tokenizer.tokenize_line(l, &mut LineState::default());
    let rt = tokenizer.tokenize_line(r, &mut LineState::default());
    let mut rules = rules();
    rules.importance.unimportant.push(E::Comment);
    let compiled = CompiledComparison::new(&rules).unwrap();
    let view = |text, tokens| LineView {
        text,
        tokens,
        format: Some(format),
        ending: bc_core::line::LineEnding::Lf,
    };
    assert_eq!(
        compiled.classify(view(l, &lt), view(r, &rt)),
        I::Unimportant
    );
    assert_eq!(
        compiled.classify(view("BEGIN", &lt[..1]), view("begin", &rt[..1])),
        I::Same
    );
    assert_eq!(
        compiled.classify_orphan(
            view(
                "// old",
                &[bc_formats::grammar::Token {
                    span: 0..6,
                    element: E::Comment,
                    grammar_index: 5
                }]
            ),
            bc_core::Side::Left
        ),
        I::Unimportant
    );
    assert_eq!(
        propagate_block(&[I::Same, I::Unimportant, I::Important]),
        I::Important
    );
}
#[test]
fn whitespace_ending_and_case_flags_are_independent() {
    use bc_core::line::LineEnding as L;
    let mut r = rules();
    r.importance.leading_whitespace_unimportant = true;
    let c = CompiledComparison::new(&r).unwrap();
    assert_eq!(
        c.classify(LineView::plain("  x"), LineView::plain("x")),
        I::Unimportant
    );
    assert_eq!(
        c.classify(LineView::plain("x "), LineView::plain("x")),
        I::Important
    );
    assert_eq!(
        c.classify(
            LineView {
                text: "x",
                ending: L::CrLf,
                ..LineView::plain("x")
            },
            LineView {
                text: "x",
                ending: L::Lf,
                ..LineView::plain("x")
            }
        ),
        I::Same
    );
    assert_eq!(
        c.classify(
            LineView {
                text: "x",
                ending: L::None,
                ..LineView::plain("x")
            },
            LineView {
                text: "x",
                ending: L::Lf,
                ..LineView::plain("x")
            }
        ),
        I::Unimportant
    );
}
#[test]
fn replacement_direction_whole_words_and_unimportant_text() {
    let mut r = rules();
    r.replacements.push(Replacement {
        from: "colour".into(),
        to: "color".into(),
        is_regexp: false,
        case_sensitive: true,
        whole_words: true,
        bidirectional: false,
    });
    let c = CompiledComparison::new(&r).unwrap();
    assert_eq!(
        c.classify(LineView::plain("colour"), LineView::plain("color")),
        I::Unimportant
    );
    assert_eq!(
        c.classify(LineView::plain("color"), LineView::plain("colour")),
        I::Important
    );
    assert_eq!(
        c.classify(LineView::plain("colourful"), LineView::plain("colorful")),
        I::Important
    );
    r.replacements[0].bidirectional = true;
    assert_eq!(
        CompiledComparison::new(&r)
            .unwrap()
            .classify(LineView::plain("color"), LineView::plain("colour")),
        I::Unimportant
    );
    r.replacements = vec![Replacement {
        from: r"\d{4}-\d{2}-\d{2}".into(),
        to: "DATE".into(),
        is_regexp: true,
        case_sensitive: true,
        whole_words: false,
        bidirectional: true,
    }];
    assert_eq!(
        CompiledComparison::new(&r).unwrap().classify(
            LineView::plain("at 2025-01-01"),
            LineView::plain("at 2026-02-03")
        ),
        I::Unimportant
    );
    r.unimportant_text.push(UnimportantText {
        pattern: "TODO.*".into(),
        is_regexp: true,
        case_sensitive: true,
        only_in: None,
    });
    assert_eq!(
        CompiledComparison::new(&r)
            .unwrap()
            .classify(LineView::plain("x TODO abc"), LineView::plain("x TODO def")),
        I::Unimportant
    );
}
