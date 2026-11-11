//! Key layout and menu text: pure functions, no I/O except [`Left::read`]'s
//! file-kind check.
use std::path::{Path, PathBuf};

/// Real registry root (relative to `HKEY_CURRENT_USER`).
pub const ROOT: &str = r"Software\Classes";

/// Brand prefix on every visible menu entry, so the items never look like a
/// co-installed Beyond Compare's.
pub const BRAND: &str = "Matcha: ";

/// Longest file name shown in the "Compare to" label before middle-ellipsizing.
const MAX_NAME: usize = 40;

/// Shell class a verb is registered under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// `*` – every file.
    File,
    /// `Directory` – file-system folders.
    Folder,
}

impl Class {
    pub const ALL: [Class; 2] = [Class::File, Class::Folder];

    /// Subkey of `Software\Classes`.
    pub fn key(self) -> &'static str {
        match self {
            Class::File => "*",
            Class::Folder => "Directory",
        }
    }

    /// Folder when `path` is an existing directory, file otherwise.
    pub fn of(path: &Path) -> Class {
        if path.is_dir() {
            Class::Folder
        } else {
            Class::File
        }
    }

    fn noun(self) -> &'static str {
        match self {
            Class::File => "File",
            Class::Folder => "Folder",
        }
    }
}

/// The three static verbs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    /// "Select Left File/Folder for Compare" – single selection only.
    SelectLeft,
    /// `Compare to "<left>"` – single selection, hidden while nothing (or the other
    /// kind) is pending.
    CompareTo,
    /// "Compare" – multi-selection; Explorer runs it once per item, the pair slot
    /// joins two consecutive invocations.
    Compare,
}

impl Verb {
    pub const ALL: [Verb; 3] = [Verb::SelectLeft, Verb::CompareTo, Verb::Compare];

    /// Verb key name (`<class>\shell\<name>`).
    pub fn key_name(self) -> &'static str {
        match self {
            Verb::SelectLeft => "Matcha.SelectLeft",
            Verb::CompareTo => "Matcha.CompareTo",
            Verb::Compare => "Matcha.Compare",
        }
    }

    /// Command-line switch the verb's command passes (see `bc_script::cli`).
    pub fn switch(self) -> &'static str {
        match self {
            Verb::SelectLeft => "/selectleft",
            Verb::CompareTo => "/compareto",
            Verb::Compare => "/comparepair",
        }
    }

    /// `MultiSelectModel`: `Single` hides the verb when several items are
    /// selected; `Player` shows it for any count (Explorer still starts one
    /// process per item).
    pub fn multi_select_model(self) -> &'static str {
        match self {
            Verb::SelectLeft | Verb::CompareTo => "Single",
            Verb::Compare => "Player",
        }
    }
}

/// `<root>\<class>\shell\<verb>`.
pub fn verb_key(root: &str, class: Class, verb: Verb) -> String {
    format!(r"{root}\{}\shell\{}", class.key(), verb.key_name())
}

/// `<root>\<class>\shell\<verb>\command`.
pub fn command_key(root: &str, class: Class, verb: Verb) -> String {
    verb_key(root, class, verb) + r"\command"
}

/// Every verb key Matcha creates (what unregistration removes).
pub fn all_verb_keys(root: &str) -> Vec<String> {
    Class::ALL
        .into_iter()
        .flat_map(|class| Verb::ALL.map(|verb| verb_key(root, class, verb)))
        .collect()
}

/// Values written on a verb key; any of them not wanted any more is removed.
pub const VERB_VALUES: [&str; 4] = [
    "MUIVerb",
    "Icon",
    "MultiSelectModel",
    "ProgrammaticAccessOnly",
];

/// The pending Left, as far as the menu needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Left {
    pub path: PathBuf,
    pub class: Class,
}

impl Left {
    /// `None` when the path no longer exists (stale state).
    pub fn read(path: PathBuf) -> Option<Left> {
        path.exists().then(|| Left {
            class: Class::of(&path),
            path,
        })
    }
}

/// Executables the verbs point at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Targets {
    /// Program the verb commands run (`matcha-comp.exe`, else `matcha.exe`).
    pub launcher: PathBuf,
    /// Executable whose first icon is the menu icon (`matcha.exe`).
    pub icon: PathBuf,
}

/// Static label of a verb for a class (the Compare-to label depends on the Left).
pub fn label(class: Class, verb: Verb, left: Option<&Left>) -> String {
    match verb {
        Verb::SelectLeft => format!("{BRAND}Select Left {} for Compare", class.noun()),
        Verb::Compare => format!("{BRAND}Compare"),
        Verb::CompareTo => match left {
            Some(left) => format!(
                "{BRAND}Compare to \"{}\"",
                menu_text(&display_name(&left.path))
            ),
            None => format!("{BRAND}Compare to"),
        },
    }
}

/// The Compare-to verb is visible only for the class of the pending Left.
pub fn compare_to_visible(class: Class, left: Option<&Left>) -> bool {
    left.is_some_and(|left| left.class == class)
}

/// Last path component (the whole path for a drive root), middle-ellipsized.
pub fn display_name(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= MAX_NAME {
        return name;
    }
    let half = (MAX_NAME - 1) / 2;
    let head: String = chars[..half].iter().collect();
    let tail: String = chars[chars.len() - half..].iter().collect();
    format!("{head}…{tail}")
}

/// Menu strings treat `&` as a mnemonic marker; double it to show it literally.
pub fn menu_text(text: &str) -> String {
    text.replace('&', "&&")
}

/// One key and the exact values it should hold (`""` = default value).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub values: Vec<(&'static str, String)>,
    /// Values in [`VERB_VALUES`] that must be absent (verb keys only).
    pub absent: Vec<&'static str>,
}

/// The command a verb runs: `"<launcher>" /<switch> "%1"`.
pub fn command(targets: &Targets, verb: Verb) -> String {
    format!(
        "\"{}\" {} \"%1\"",
        targets.launcher.display(),
        verb.switch()
    )
}

/// Verb-key entry (label, icon, selection model, visibility).
pub fn verb_entry(
    root: &str,
    targets: &Targets,
    class: Class,
    verb: Verb,
    left: Option<&Left>,
) -> Entry {
    let mut values = vec![
        ("MUIVerb", label(class, verb, left)),
        ("Icon", format!("{},0", targets.icon.display())),
        ("MultiSelectModel", verb.multi_select_model().to_string()),
    ];
    if verb == Verb::CompareTo && !compare_to_visible(class, left) {
        // Hidden from the menu, still invocable (verified on Windows 11 26100).
        values.push(("ProgrammaticAccessOnly", String::new()));
    }
    let absent = VERB_VALUES
        .into_iter()
        .filter(|name| !values.iter().any(|(n, _)| n == name))
        .collect();
    Entry {
        key: verb_key(root, class, verb),
        values,
        absent,
    }
}

/// Full desired layout: a verb key and a command key per class and verb.
pub fn entries(root: &str, targets: &Targets, left: Option<&Left>) -> Vec<Entry> {
    let mut out = Vec::new();
    for class in Class::ALL {
        for verb in Verb::ALL {
            out.push(verb_entry(root, targets, class, verb, left));
            out.push(Entry {
                key: command_key(root, class, verb),
                values: vec![("", command(targets, verb))],
                absent: Vec::new(),
            });
        }
    }
    out
}

/// Only the Compare-to verb keys' label and visibility (Left changed).
pub fn compare_to_entries(root: &str, left: Option<&Left>) -> Vec<Entry> {
    Class::ALL
        .into_iter()
        .map(|class| {
            let mut values = vec![("MUIVerb", label(class, Verb::CompareTo, left))];
            let mut absent = Vec::new();
            if compare_to_visible(class, left) {
                absent.push("ProgrammaticAccessOnly");
            } else {
                values.push(("ProgrammaticAccessOnly", String::new()));
            }
            Entry {
                key: verb_key(root, class, Verb::CompareTo),
                values,
                absent,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn targets() -> Targets {
        Targets {
            launcher: PathBuf::from(r"C:\M\matcha-comp.exe"),
            icon: PathBuf::from(r"C:\M\matcha.exe"),
        }
    }

    fn file_left(name: &str) -> Left {
        Left {
            path: PathBuf::from(format!(r"C:\work\{name}")),
            class: Class::File,
        }
    }

    #[test]
    fn keys_are_namespaced_under_classes() {
        let keys = all_verb_keys(ROOT);
        assert_eq!(
            keys,
            [
                r"Software\Classes\*\shell\Matcha.SelectLeft",
                r"Software\Classes\*\shell\Matcha.CompareTo",
                r"Software\Classes\*\shell\Matcha.Compare",
                r"Software\Classes\Directory\shell\Matcha.SelectLeft",
                r"Software\Classes\Directory\shell\Matcha.CompareTo",
                r"Software\Classes\Directory\shell\Matcha.Compare",
            ]
        );
        for entry in entries(ROOT, &targets(), None) {
            assert!(entry.key.starts_with(r"Software\Classes\"));
            assert!(entry.key.contains(r"\shell\Matcha."));
        }
        assert_eq!(entries(ROOT, &targets(), None).len(), 12);
    }

    #[test]
    fn commands_quote_the_launcher_and_the_item() {
        let t = targets();
        assert_eq!(
            command(&t, Verb::SelectLeft),
            r#""C:\M\matcha-comp.exe" /selectleft "%1""#
        );
        assert_eq!(
            command(&t, Verb::CompareTo),
            r#""C:\M\matcha-comp.exe" /compareto "%1""#
        );
        assert_eq!(
            command(&t, Verb::Compare),
            r#""C:\M\matcha-comp.exe" /comparepair "%1""#
        );
    }

    #[test]
    fn labels_are_branded_and_follow_the_left() {
        assert_eq!(
            label(Class::File, Verb::SelectLeft, None),
            "Matcha: Select Left File for Compare"
        );
        assert_eq!(
            label(Class::Folder, Verb::SelectLeft, None),
            "Matcha: Select Left Folder for Compare"
        );
        assert_eq!(label(Class::File, Verb::Compare, None), "Matcha: Compare");
        let left = file_left("a & b.txt");
        assert_eq!(
            label(Class::File, Verb::CompareTo, Some(&left)),
            "Matcha: Compare to \"a && b.txt\""
        );
        assert!(compare_to_visible(Class::File, Some(&left)));
        assert!(!compare_to_visible(Class::Folder, Some(&left)));
        assert!(!compare_to_visible(Class::File, None));
    }

    #[test]
    fn long_names_are_ellipsized_in_the_middle() {
        let long = "x".repeat(30) + "-middle-" + &"y".repeat(30) + ".txt";
        let shown = display_name(Path::new(&long));
        assert_eq!(shown.chars().count(), 39);
        assert!(shown.starts_with("xxx") && shown.ends_with("yy.txt") && shown.contains('…'));
        assert_eq!(display_name(Path::new(r"C:\")), r"C:\");
    }

    #[test]
    fn compare_to_is_hidden_without_a_matching_left() {
        let t = targets();
        let hidden = verb_entry(ROOT, &t, Class::File, Verb::CompareTo, None);
        assert!(hidden
            .values
            .contains(&("ProgrammaticAccessOnly", String::new())));
        assert!(hidden.absent.is_empty());
        let left = file_left("a.txt");
        let shown = verb_entry(ROOT, &t, Class::File, Verb::CompareTo, Some(&left));
        assert_eq!(shown.absent, ["ProgrammaticAccessOnly"]);
        let other = verb_entry(ROOT, &t, Class::Folder, Verb::CompareTo, Some(&left));
        assert!(other.absent.is_empty());
        let select = verb_entry(ROOT, &t, Class::File, Verb::SelectLeft, Some(&left));
        assert!(select
            .values
            .contains(&("Icon", r"C:\M\matcha.exe,0".to_string())));
        assert!(select
            .values
            .contains(&("MultiSelectModel", "Single".to_string())));
        let pair = verb_entry(ROOT, &t, Class::File, Verb::Compare, None);
        assert!(pair
            .values
            .contains(&("MultiSelectModel", "Player".to_string())));
    }
}
