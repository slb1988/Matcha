//! Exercise all regex-bearing fields across the entire embedded library without a
//! core -> formats dependency (the formats crate already depends on core).
use bc_core::pattern::Pattern;
use bc_formats::{builtin_formats, GrammarMatch};

#[test]
fn every_builtin_regular_expression_compiles() {
    let formats = builtin_formats();
    assert_eq!(formats.len(), 25);
    let mut checked = 0;
    for format in formats {
        let mut compile = |source: &str, case: bool| {
            Pattern::new(source, case)
                .unwrap_or_else(|e| panic!("{} pattern {source:?}: {e}", format.name));
            checked += 1;
        };
        for item in &format.grammar {
            match &item.matcher {
                GrammarMatch::Basic {
                    text,
                    is_regexp: true,
                } => compile(text, item.case_sensitive),
                GrammarMatch::Delimited {
                    start,
                    stop,
                    is_regexp: true,
                    ..
                } => {
                    compile(start, item.case_sensitive);
                    if let Some(stop) = stop {
                        compile(stop, item.case_sensitive);
                    }
                }
                GrammarMatch::List {
                    items,
                    is_regexp: true,
                } => {
                    for text in items {
                        compile(text, item.case_sensitive);
                    }
                }
                _ => {}
            }
        }
        for weight in &format.weights {
            if weight.is_regexp {
                compile(&weight.find_text, true);
            }
        }
    }
    assert_eq!(checked, 74, "complete regex-bearing built-in model");
    println!("compiled {checked} patterns across 25 formats");
}
