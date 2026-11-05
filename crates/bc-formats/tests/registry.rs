use bc_formats::{
    builtin_formats,
    registry::{user_formats_dir, FormatRegistry},
    FormatKind,
};
use std::path::{Path, PathBuf};
fn custom(name: &str) -> bc_formats::FileFormat {
    let mut format = builtin_formats()
        .iter()
        .find(|f| f.name == "CPP")
        .unwrap()
        .clone();
    format.name = name.into();
    format.masks = vec!["*.cpp".into(), "*.cxx".into()];
    format
}
#[test]
fn masks_are_case_insensitive_first_registered_and_kind_filtered() {
    let mut registry = FormatRegistry::new();
    registry.upsert(custom("first")).unwrap();
    registry.upsert(custom("second")).unwrap();
    assert_eq!(
        registry
            .resolve(Path::new("src/MAIN.CPP"), Some(FormatKind::Text))
            .unwrap()
            .name,
        "first"
    );
    assert_eq!(
        registry.resolve(Path::new("test.cxx"), None).unwrap().name,
        "first"
    );
    assert!(registry.resolve(Path::new("notes.abc"), None).is_none());
    assert!(registry
        .resolve(Path::new("main.cpp"), Some(FormatKind::Table))
        .is_none());
    registry.move_to(FormatKind::Text, "second", 0).unwrap();
    assert_eq!(
        registry.resolve(Path::new("MAIN.CPP"), None).unwrap().name,
        "second"
    );
}
#[test]
fn user_override_keeps_key_position_and_builtin_library_immutable() {
    let mut registry = FormatRegistry::with_builtins();
    assert!(registry.is_builtin(FormatKind::Text, "CPP"));
    let mut cpp = custom("CPP");
    cpp.tab_size = Some(7);
    registry.upsert(cpp.clone()).unwrap();
    assert_eq!(registry.formats().len(), 25);
    assert!(!registry.is_builtin(FormatKind::Text, "CPP"));
    assert_eq!(registry.get(FormatKind::Text, "CPP").unwrap(), &cpp);
    assert_ne!(
        builtin_formats()
            .iter()
            .find(|f| f.name == "CPP")
            .unwrap()
            .tab_size,
        Some(7)
    );
    let xml = registry.export_xml().unwrap();
    let mut imported = FormatRegistry::new();
    assert_eq!(imported.import_xml(&xml).unwrap(), 1);
    assert_eq!(imported.formats(), &[cpp]);
}
#[test]
fn imports_are_atomic_and_unknown_grammar_names_format_and_type() {
    let mut registry = FormatRegistry::with_builtins();
    let before = registry.formats().to_vec();
    let xml = r#"<BCFileFormats><TTextFormat Value="Good"/><TTextFormat Value="Bad"><Grammar><_0 Value="TUnknownGrammar"/></Grammar></TTextFormat></BCFileFormats>"#;
    let error = registry.import_xml(xml).unwrap_err().to_string();
    assert!(error.contains("Bad") && error.contains("TUnknownGrammar"));
    assert_eq!(registry.formats(), before);
    let invalid = r#"<BCFileFormats><TTextFormat Value="Regex"><Grammar><_0 Value="TBasicTextGrammarItem"><ElementStr Value="String"/><IsRegExp Value="True"/><Text Value="("/></_0></Grammar></TTextFormat></BCFileFormats>"#;
    assert!(registry
        .import_xml(invalid)
        .unwrap_err()
        .to_string()
        .contains("Regex"));
    assert_eq!(registry.formats(), before);
}
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "matcha-formats-registry-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
#[test]
fn user_definitions_and_order_survive_reload_and_repeated_atomic_save() {
    let temp = Temp::new();
    let mut registry = FormatRegistry::load_user_dir(&temp.0).unwrap();
    assert_eq!(registry.formats().len(), 25);
    assert!(!temp.0.exists());
    registry.upsert(custom("Preferred")).unwrap();
    registry.move_to(FormatKind::Text, "Preferred", 0).unwrap();
    registry.save_user_dir(&temp.0).unwrap();
    registry.save_user_dir(&temp.0).unwrap();
    let loaded = FormatRegistry::load_user_dir(&temp.0).unwrap();
    assert_eq!(loaded.formats(), registry.formats());
    assert_eq!(
        loaded.resolve(Path::new("main.cpp"), None).unwrap().name,
        "Preferred"
    );
    assert!(!loaded.is_builtin(FormatKind::Text, "Preferred"));
    assert!(loaded.is_builtin(FormatKind::Text, "CPP"));
    assert!(user_formats_dir()
        .unwrap()
        .ends_with(Path::new("Matcha").join("formats")));
    assert_eq!(
        std::fs::read_dir(&temp.0).unwrap().count(),
        2,
        "no staging files left"
    );
}
#[test]
fn export_import_all_new_formats_preserves_semantics() {
    let mut registry = FormatRegistry::new();
    for format in builtin_formats() {
        registry.upsert(format.clone()).unwrap();
    }
    let mut roundtrip = FormatRegistry::new();
    assert_eq!(
        roundtrip
            .import_xml(&registry.export_xml().unwrap())
            .unwrap(),
        25
    );
    assert_eq!(roundtrip.formats(), registry.formats());
    assert!(roundtrip.get(FormatKind::Xml, "XML").is_some());
    assert!(roundtrip.get(FormatKind::Text, "XML").is_some());
}
