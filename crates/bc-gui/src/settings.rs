//! 持久化偏好：Tools › Options 的全部设置（差异颜色、编辑器字体、默认 Tab 宽度与代码页、
//! 备份），见 `docs/features/gui-shell.md`「偏好持久化」。
//!
//! 文件 `settings.txt` 与 `layout.txt` 同目录：首行 [`HEADER`]，之后每行 `键\t值`；
//! 未识别的键原样保留（新版本写入的键不会被旧版本丢掉）。快照/基准模式经
//! [`disable_persistence`] 只用默认值、不读不写；单元测试中从不读写磁盘。
use bc_session::prefs::{DiffColors, EditorFont, Prefs};
use std::collections::BTreeMap;

const HEADER: &str = "# matcha settings v1";
const FONT_FAMILY: &str = "editor.font.family";
const FONT_SIZE: &str = "editor.font.size";
const TAB_SIZE: &str = "text.default-tab-size";
const CODE_PAGE: &str = "text.default-code-page";
const BACKUPS: &str = "backups.enabled";
const BACKUP_SUFFIX: &str = "backups.suffix";
/// Colour keys (`#RRGGBB`) and the `DiffColors` field each one maps to.
type ColorField = fn(&mut DiffColors) -> &mut [u8; 3];
const COLORS: [(&str, ColorField); 4] = [
    ("colors.important", |c| &mut c.important),
    ("colors.unimportant", |c| &mut c.unimportant),
    ("colors.inserted", |c| &mut c.inserted),
    ("colors.conflict", |c| &mut c.conflict),
];

static PERSIST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(!cfg!(test));

fn file() -> Option<std::path::PathBuf> {
    bc_formats::registry::user_formats_dir()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("settings.txt")))
}

fn persist() -> bool {
    PERSIST.load(std::sync::atomic::Ordering::Relaxed)
}

/// Deterministic runs (snapshots, benchmarks): defaults only, never read or write the file.
pub fn disable_persistence() {
    PERSIST.store(false, std::sync::atomic::Ordering::Relaxed);
}

fn read_file() -> BTreeMap<String, String> {
    if !persist() {
        return BTreeMap::new();
    }
    file()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .map(|t| parse(&t))
        .unwrap_or_default()
}

/// Saved preferences over the defaults.
pub fn load() -> Prefs {
    let mut prefs = Prefs::default();
    apply(&read_file(), &mut prefs);
    prefs
}

/// Write the persisted subset of `prefs` (no-op when persistence is off).
pub fn save(prefs: &Prefs) {
    if !persist() {
        return;
    }
    let mut values = read_file();
    store(prefs, &mut values);
    if let Some(file) = file() {
        if let Some(parent) = file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(file, format(&values));
    }
}

fn apply(values: &BTreeMap<String, String>, prefs: &mut Prefs) {
    if let Some(family) = values.get(FONT_FAMILY).filter(|f| !f.trim().is_empty()) {
        prefs.editor_font.family = family.trim().to_owned();
    }
    if let Some(size) = values
        .get(FONT_SIZE)
        .and_then(|s| s.trim().parse::<u8>().ok())
        .filter(|s| EditorFont::SIZE_RANGE.contains(s))
    {
        prefs.editor_font.size_pt = size;
    }
    if let Some(n) = values
        .get(TAB_SIZE)
        .and_then(|s| s.trim().parse::<u32>().ok())
        .filter(|n| *n <= 64)
    {
        prefs.default_tab_size = n;
    }
    if let Some(n) = values.get(CODE_PAGE).and_then(|s| s.trim().parse().ok()) {
        prefs.default_code_page = n;
    }
    match values.get(BACKUPS).map(|s| s.trim()) {
        Some("true") => prefs.backups.enabled = true,
        Some("false") => prefs.backups.enabled = false,
        _ => {}
    }
    if let Some(suffix) = values.get(BACKUP_SUFFIX) {
        prefs.backups.extension = (!suffix.is_empty()).then(|| suffix.clone());
    }
    for (key, field) in COLORS {
        if let Some(rgb) = values.get(key).and_then(|v| parse_rgb(v)) {
            *field(&mut prefs.colors) = rgb;
        }
    }
}

fn store(prefs: &Prefs, values: &mut BTreeMap<String, String>) {
    values.insert(FONT_FAMILY.into(), prefs.editor_font.family.clone());
    values.insert(FONT_SIZE.into(), prefs.editor_font.size_pt.to_string());
    values.insert(TAB_SIZE.into(), prefs.default_tab_size.to_string());
    values.insert(CODE_PAGE.into(), prefs.default_code_page.to_string());
    values.insert(BACKUPS.into(), prefs.backups.enabled.to_string());
    values.insert(
        BACKUP_SUFFIX.into(),
        prefs.backups.extension.clone().unwrap_or_default(),
    );
    let mut colors = prefs.colors;
    for (key, field) in COLORS {
        let [r, g, b] = *field(&mut colors);
        values.insert(key.into(), format!("#{r:02X}{g:02X}{b:02X}"));
    }
}

/// `#RRGGBB` → bytes.
fn parse_rgb(text: &str) -> Option<[u8; 3]> {
    let hex = text.trim().strip_prefix('#')?;
    if hex.len() != 6 || !hex.is_ascii() {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

fn parse(text: &str) -> BTreeMap<String, String> {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some(HEADER) {
        return BTreeMap::new();
    }
    lines
        .filter_map(|line| {
            let (key, value) = line.split_once('\t')?;
            (!key.is_empty()).then(|| (key.to_owned(), value.to_owned()))
        })
        .collect()
}

fn format(values: &BTreeMap<String, String>) -> String {
    let mut text = format!("{HEADER}\n");
    for (key, value) in values {
        // 值不含换行/制表符（字体名来自系统字体表）；防御性替换为空格。
        let value = value.replace(['\t', '\r', '\n'], " ");
        text += &format!("{key}\t{value}\n");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_keep_unknown_keys() {
        let mut prefs = Prefs::default();
        prefs.editor_font = EditorFont {
            family: "Cascadia Mono".into(),
            size_pt: 12,
        };
        prefs.colors.important = [0x12, 0xAB, 0xEF];
        prefs.colors.inserted = [0, 0, 0];
        prefs.default_tab_size = 8;
        prefs.default_code_page = 936;
        prefs.backups.enabled = true;
        prefs.backups.extension = Some(".bak".into());
        let mut values = parse("# matcha settings v1\nfuture.key\tkept\n");
        store(&prefs, &mut values);
        let text = format(&values);
        assert!(text.starts_with("# matcha settings v1\n"));
        assert!(text.contains("future.key\tkept\n"));
        let mut back = Prefs::default();
        apply(&parse(&text), &mut back);
        assert!(text.contains("colors.important\t#12ABEF\n"));
        assert_eq!(back, prefs);
    }

    #[test]
    fn bad_values_fall_back_to_defaults() {
        let mut prefs = Prefs::default();
        apply(
            &parse(
                "# matcha settings v1\neditor.font.size\t99\neditor.font.family\t  \n\
                 text.default-tab-size\t65\ntext.default-code-page\tx\nbackups.enabled\tyes\n\
                 colors.important\t#12345\ncolors.unimportant\tred\n",
            ),
            &mut prefs,
        );
        assert_eq!(prefs, Prefs::default());
        assert!(
            parse("editor.font.size\t12\n").is_empty(),
            "header required"
        );
    }

    #[test]
    fn tests_never_touch_the_settings_file() {
        assert!(!persist());
        assert_eq!(load(), Prefs::default());
        save(&Prefs::default());
    }
}
