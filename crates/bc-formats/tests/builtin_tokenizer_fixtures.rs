use bc_formats::{
    builtin_formats,
    grammar::{LineState, Tokenizer},
    ElementKind, FormatKind,
};
use serde::Deserialize;
use std::{collections::HashSet, path::Path};

#[derive(Debug, Deserialize)]
struct Expected {
    format: String,
    kind: FormatKind,
    lines: Vec<ExpectedLine>,
}
#[derive(Debug, Deserialize)]
struct ExpectedLine {
    tokens: Vec<ExpectedToken>,
    open: bool,
}
#[derive(Debug, PartialEq, Eq, Deserialize)]
struct ExpectedToken {
    text: String,
    element: ElementKind,
    start: usize,
    end: usize,
}

#[test]
fn all_twenty_five_formats_match_handwritten_token_and_state_fixtures() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    let mut paths: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    assert_eq!(paths.len(), 25);
    let mut seen = HashSet::new();
    for path in paths {
        let expected: Expected = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert!(seen.insert((expected.kind, expected.format.clone())));
        let format = builtin_formats()
            .iter()
            .find(|f| f.kind == expected.kind && f.name == expected.format)
            .unwrap();
        let tokenizer = Tokenizer::new(format).unwrap();
        let input = std::fs::read_to_string(path.with_extension("txt")).unwrap();
        let lines: Vec<_> = input.lines().collect();
        assert_eq!(lines.len(), expected.lines.len(), "{}", path.display());
        let mut state = LineState::default();
        for (i, (line, expect)) in lines.iter().zip(expected.lines).enumerate() {
            let tokens = tokenizer.tokenize_line(line, &mut state);
            assert!(tokens.windows(2).all(|w| w[0].span.end <= w[1].span.start));
            let actual: Vec<_> = tokens
                .into_iter()
                .map(|token| ExpectedToken {
                    text: line[token.span.clone()].into(),
                    element: token.element,
                    start: token.span.start,
                    end: token.span.end,
                })
                .collect();
            assert_eq!(actual, expect.tokens, "{} line {}", path.display(), i + 1);
            assert_eq!(
                state.open_item.is_some(),
                expect.open,
                "{} line {} state",
                path.display(),
                i + 1
            );
        }
    }
    assert_eq!(
        seen,
        builtin_formats()
            .iter()
            .map(|f| (f.kind, f.name.clone()))
            .collect()
    );
}
