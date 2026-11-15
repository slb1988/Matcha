//! 持久化布局：分隔线位置等界面尺寸（键 → `f32`），见 `docs/features/resizable-splitters.md`。
//!
//! 文件 `layout.txt` 与 `recent-text.txt` 同目录：首行 [`HEADER`]，之后每行 `键\t值`。
//! 进程内首次访问时读入；[`save`] 整体写回（拖动结束时调用）。快照/基准模式经
//! [`disable_persistence`] 只用默认值、不读不写；单元测试中存储为线程内，测试之间互不影响。
use std::collections::BTreeMap;

const HEADER: &str = "# matcha layout v1";

struct Store {
    values: BTreeMap<String, f32>,
    loaded: bool,
    persist: bool,
}

impl Store {
    const fn new(persist: bool) -> Self {
        Store {
            values: BTreeMap::new(),
            loaded: false,
            persist,
        }
    }
    fn ensure_loaded(&mut self) {
        if !self.loaded {
            self.loaded = true;
            if self.persist {
                if let Some(text) = file().and_then(|f| std::fs::read_to_string(f).ok()) {
                    self.values = parse(&text);
                }
            }
        }
    }
}

#[cfg(not(test))]
static STORE: std::sync::Mutex<Store> = std::sync::Mutex::new(Store::new(true));

#[cfg(not(test))]
fn with<R>(f: impl FnOnce(&mut Store) -> R) -> R {
    let mut store = STORE.lock().unwrap_or_else(|e| e.into_inner());
    store.ensure_loaded();
    f(&mut store)
}

#[cfg(test)]
thread_local! {
    static STORE: std::cell::RefCell<Store> = const { std::cell::RefCell::new(Store::new(false)) };
}

#[cfg(test)]
fn with<R>(f: impl FnOnce(&mut Store) -> R) -> R {
    STORE.with(|s| {
        let mut store = s.borrow_mut();
        store.ensure_loaded();
        f(&mut store)
    })
}

fn file() -> Option<std::path::PathBuf> {
    bc_formats::registry::user_formats_dir()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("layout.txt")))
}

/// Stored value for `key` (finite values only).
pub fn get(key: &str) -> Option<f32> {
    with(|s| s.values.get(key).copied())
}

/// Update `key` in memory; call [`save`] to write it out.
pub fn set(key: &str, value: f32) {
    if value.is_finite() {
        with(|s| {
            s.values.insert(key.to_owned(), value);
        });
    }
}

/// Forget `key` (back to the caller's default); call [`save`] to write it out.
pub fn remove(key: &str) {
    with(|s| {
        s.values.remove(key);
    });
}

/// Write the whole store to disk (no-op when persistence is off).
pub fn save() {
    let (persist, text) = with(|s| (s.persist, format(&s.values)));
    if !persist {
        return;
    }
    if let Some(file) = file() {
        if let Some(parent) = file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(file, text);
    }
}

/// Deterministic runs (snapshots, benchmarks): defaults only, never read or write the file.
pub fn disable_persistence() {
    with(|s| {
        s.persist = false;
        s.values.clear();
    });
}

fn parse(text: &str) -> BTreeMap<String, f32> {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some(HEADER) {
        return BTreeMap::new();
    }
    lines
        .filter_map(|line| {
            let (key, value) = line.split_once('\t')?;
            let value: f32 = value.trim().parse().ok()?;
            (!key.is_empty() && value.is_finite()).then(|| (key.to_owned(), value))
        })
        .collect()
}

fn format(values: &BTreeMap<String, f32>) -> String {
    let mut text = format!("{HEADER}\n");
    for (key, value) in values {
        text += &format!("{key}\t{value}\n");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_file_round_trips_and_ignores_bad_lines() {
        let mut values = BTreeMap::new();
        values.insert("text.split.x".to_owned(), 0.375);
        values.insert("home.tree.w".to_owned(), 312.0);
        let text = format(&values);
        assert!(text.starts_with("# matcha layout v1\n"));
        assert_eq!(parse(&text), values);
        let noisy = format!("{text}garbage\nkey\tNaN\n\t1\nfuture.key\t2.5\n");
        let parsed = parse(&noisy);
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed["future.key"], 2.5);
        assert!(parse("text.split.x\t0.3\n").is_empty(), "header required");
    }

    #[test]
    fn layout_store_is_in_memory_under_test() {
        assert_eq!(get("layout.test"), None);
        set("layout.test", 0.25);
        assert_eq!(get("layout.test"), Some(0.25));
        set("layout.test", f32::NAN);
        assert_eq!(get("layout.test"), Some(0.25));
        save();
        remove("layout.test");
        assert_eq!(get("layout.test"), None);
    }
}
