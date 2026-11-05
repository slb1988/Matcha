use bc_formats::{
    builtin_formats,
    grammar::{LineState, Tokenizer},
    ElementKind as E, GrammarItem, GrammarMatch,
};

fn format(name: &str) -> &'static bc_formats::FileFormat {
    builtin_formats().iter().find(|f| f.name == name).unwrap()
}
fn elements(name: &str, line: &str) -> Vec<(String, E)> {
    Tokenizer::new(format(name))
        .unwrap()
        .tokenize_line(line, &mut LineState::default())
        .into_iter()
        .map(|t| (line[t.span].to_string(), t.element))
        .collect()
}
#[test]
fn cpp_multiline_comment_and_doubled_quote_escape() {
    let tokenizer = Tokenizer::new(format("CPP")).unwrap();
    let mut state = LineState::default();
    let first = tokenizer.tokenize_line("int x; /* first", &mut state);
    assert_eq!(first.last().unwrap().element, E::Comment);
    assert!(state.open_item.is_some());
    let second = tokenizer.tokenize_line(" second", &mut state);
    assert_eq!(second[0].span, 0..7);
    assert_eq!(second[0].element, E::Comment);
    let last = tokenizer.tokenize_line(" end */ int y;", &mut state);
    assert_eq!(last[0].element, E::Comment);
    assert!(last.iter().any(|t| t.element == E::Keyword));
    assert_eq!(state.open_item, None);
    let quoted = elements("CPP", r#"@"a""b" next"#);
    assert_eq!(quoted[0], (r#"@"a""b""#.into(), E::String));
}
#[test]
fn bash_escape_and_keyword_boundaries() {
    let result = elements("BASH", r#"echo "a\"b" $HOME echoing"#);
    assert_eq!(result[0], ("echo".into(), E::Keyword));
    assert_eq!(result[1], (r#""a\"b""#.into(), E::String));
    assert!(result.contains(&("$HOME".into(), E::EnvironmentVar)));
    assert_eq!(result.last().unwrap(), &("echoing".into(), E::Identifier));
}
#[test]
fn cobol_fixed_columns_clip_lower_priority_long_identifier() {
    let text = "A".repeat(85);
    let tokens = Tokenizer::new(format("COBOL"))
        .unwrap()
        .tokenize_line(&text, &mut LineState::default());
    let number = tokens.iter().find(|t| t.element == E::LineNum).unwrap();
    assert_eq!(number.span, 72..80);
    assert!(tokens.windows(2).all(|w| w[0].span.end <= w[1].span.start));
}
#[test]
fn python_triple_quotes_span_lines_even_after_single_quote_rules() {
    let tokenizer = Tokenizer::new(format("PYTHON")).unwrap();
    for quote in ["'''", r#"""""#] {
        let mut state = LineState::default();
        let start = format!("{quote}first");
        let tokens = tokenizer.tokenize_line(&start, &mut state);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].element, E::String);
        assert!(state.open_item.is_some(), "{quote:?}");
        let end = format!("last{quote} return");
        let tokens = tokenizer.tokenize_line(&end, &mut state);
        assert_eq!(tokens[0].element, E::String);
        assert!(tokens.iter().any(|t| t.element == E::Keyword));
        assert_eq!(state.open_item, None);
    }
}
#[test]
fn dfm_keywords_ignore_case_and_lists_take_longest_literal() {
    let result = elements("DFM", "OBJECT Frame: TForm END");
    assert_eq!(result[0], ("OBJECT".into(), E::Keyword));
    assert_eq!(result.last().unwrap(), &("END".into(), E::Keyword));
    let result = elements("PYTHON", "x >>= 1;");
    assert!(result.iter().any(|(t, e)| t == ">>=" && *e == E::Operator));
}
#[test]
fn grammar_priority_and_unicode_columns_and_invalid_patterns() {
    assert_eq!(
        elements("CPP", "while_loop"),
        vec![("while_loop".into(), E::Identifier)]
    );
    assert_eq!(
        elements("PERL", "qq(hi)"),
        vec![("qq(hi)".into(), E::String)]
    );
    assert_eq!(
        elements("VB", "rem note"),
        vec![("rem note".into(), E::Comment)]
    );
    let mut custom = format("DFM").clone();
    custom.grammar = vec![
        GrammarItem {
            element: E::Comment,
            case_sensitive: true,
            matcher: GrammarMatch::Basic {
                text: "x".into(),
                is_regexp: false,
            },
        },
        GrammarItem {
            element: E::String,
            case_sensitive: true,
            matcher: GrammarMatch::Basic {
                text: "xy".into(),
                is_regexp: false,
            },
        },
        GrammarItem {
            element: E::LineNum,
            case_sensitive: true,
            matcher: GrammarMatch::Columns {
                start_col: 3,
                stop_col: 3,
            },
        },
        GrammarItem {
            element: E::Keyword,
            case_sensitive: true,
            matcher: GrammarMatch::Basic {
                text: "xy".into(),
                is_regexp: false,
            },
        },
    ];
    let tokens = Tokenizer::new(&custom)
        .unwrap()
        .tokenize_line("xy中", &mut LineState::default());
    assert_eq!(tokens[0].span, 0..2);
    assert_eq!(tokens[0].element, E::String); // longest; first rule wins equal-length tie
    assert_eq!(tokens[1].span, 2..5);
    custom.grammar[0].matcher = GrammarMatch::Basic {
        text: "(".into(),
        is_regexp: true,
    };
    assert!(Tokenizer::new(&custom)
        .unwrap_err()
        .to_string()
        .contains("DFM"));
}
#[test]
fn dfm_weight_rules_feed_smart_alignment_anchors() {
    use bc_core::{
        align::{AlignInput, Aligner, SmartAlign},
        hash::LineHasher,
        line::LineList,
    };
    let tokenizer = Tokenizer::new(format("DFM")).unwrap();
    let left = LineList::from_bytes(b"object Left: TFoo\nCaption = alpha".to_vec());
    let right = LineList::from_bytes(b"Caption = alphb\nobject Right: TFoo".to_vec());
    let weights = |lines: &LineList| {
        lines
            .iter()
            .map(|(text, _)| tokenizer.line_weight(std::str::from_utf8(text).unwrap()))
            .collect::<Vec<_>>()
    };
    let lw = weights(&left);
    let rw = weights(&right);
    assert_eq!(lw[0].0, 4);
    assert_eq!(rw[1].0, 4);
    let lh = LineHasher::default().hash_lines(&left);
    let rh = LineHasher::default().hash_lines(&right);
    let map = SmartAlign::default().align2(
        AlignInput {
            hashes: &lh,
            lines: Some(&left),
            weights: Some(&lw),
            ..Default::default()
        },
        AlignInput {
            hashes: &rh,
            lines: Some(&right),
            weights: Some(&rw),
            ..Default::default()
        },
    );
    assert_eq!(map.row_of_left(0), map.row_of_right(1));
}

#[test]
fn zero_width_matches_advance_and_eol_delimiters_reset() {
    let mut custom = format("CPP").clone();
    custom.grammar.insert(
        0,
        GrammarItem {
            element: E::Number,
            case_sensitive: true,
            matcher: GrammarMatch::Basic {
                text: "^".into(),
                is_regexp: true,
            },
        },
    );
    let tokenizer = Tokenizer::new(&custom).unwrap();
    let mut state = LineState::default();
    let result = tokenizer.tokenize_line("// comment", &mut state);
    assert_eq!(result.len(), 1);
    assert_eq!(state.open_item, None);
    tokenizer.tokenize_line(r#""unterminated"#, &mut state);
    assert_eq!(state.open_item, None);
}
