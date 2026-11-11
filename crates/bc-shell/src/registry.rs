//! Registry access behind a trait: [`MemRegistry`] for tests, [`Hkcu`] (Windows)
//! for the real per-user hive. Keys are paths relative to `HKEY_CURRENT_USER`;
//! value name `""` is the default value. Only `REG_SZ` is used.
use crate::layout::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::io;

pub trait Registry {
    /// String value; `None` when the key or the value is missing.
    fn get(&self, key: &str, name: &str) -> io::Result<Option<String>>;
    fn exists(&self, key: &str) -> io::Result<bool>;
    /// Create the key (and its parents) if needed and write a `REG_SZ` value.
    fn set(&mut self, key: &str, name: &str, value: &str) -> io::Result<()>;
    /// Missing key or value is not an error.
    fn remove_value(&mut self, key: &str, name: &str) -> io::Result<()>;
    /// Delete the key with all its subkeys; missing is not an error.
    fn remove_tree(&mut self, key: &str) -> io::Result<()>;
    /// Delete the key when it has neither values nor subkeys.
    fn remove_if_empty(&mut self, key: &str) -> io::Result<()>;
}

/// Bring every entry to its exact values; returns how many values changed
/// (0 = already up to date, so repeated registration is a no-op).
pub fn apply(reg: &mut dyn Registry, entries: &[Entry]) -> io::Result<usize> {
    let mut changes = 0;
    for entry in entries {
        for (name, value) in &entry.values {
            if reg.get(&entry.key, name)?.as_deref() != Some(value.as_str()) {
                reg.set(&entry.key, name, value)?;
                changes += 1;
            }
        }
        for name in &entry.absent {
            if reg.get(&entry.key, name)?.is_some() {
                reg.remove_value(&entry.key, name)?;
                changes += 1;
            }
        }
    }
    Ok(changes)
}

/// In-memory, case-insensitive registry for unit tests.
#[derive(Debug, Default, Clone)]
pub struct MemRegistry {
    keys: BTreeSet<String>,
    values: BTreeMap<(String, String), String>,
    /// Number of `set` / `remove_*` calls that changed something.
    pub writes: usize,
}

fn norm(text: &str) -> String {
    text.to_ascii_lowercase()
}

impl MemRegistry {
    /// All keys (lower-cased) under `prefix`, including `prefix` itself.
    pub fn keys_under(&self, prefix: &str) -> Vec<String> {
        let prefix = norm(prefix);
        self.keys
            .iter()
            .filter(|k| **k == prefix || k.starts_with(&format!(r"{prefix}\")))
            .cloned()
            .collect()
    }
}

impl Registry for MemRegistry {
    fn get(&self, key: &str, name: &str) -> io::Result<Option<String>> {
        Ok(self.values.get(&(norm(key), norm(name))).cloned())
    }
    fn exists(&self, key: &str) -> io::Result<bool> {
        Ok(self.keys.contains(&norm(key)))
    }
    fn set(&mut self, key: &str, name: &str, value: &str) -> io::Result<()> {
        let key = norm(key);
        let mut path = String::new();
        for part in key.split('\\') {
            if !path.is_empty() {
                path.push('\\');
            }
            path.push_str(part);
            self.keys.insert(path.clone());
        }
        self.values.insert((key, norm(name)), value.to_string());
        self.writes += 1;
        Ok(())
    }
    fn remove_value(&mut self, key: &str, name: &str) -> io::Result<()> {
        if self.values.remove(&(norm(key), norm(name))).is_some() {
            self.writes += 1;
        }
        Ok(())
    }
    fn remove_tree(&mut self, key: &str) -> io::Result<()> {
        let doomed = self.keys_under(key);
        if doomed.is_empty() {
            return Ok(());
        }
        self.writes += 1;
        self.values.retain(|(k, _), _| !doomed.contains(k));
        self.keys.retain(|k| !doomed.contains(k));
        Ok(())
    }
    fn remove_if_empty(&mut self, key: &str) -> io::Result<()> {
        let k = norm(key);
        let has_values = self.values.keys().any(|(owner, _)| *owner == k);
        if self.keys.contains(&k) && !has_values && self.keys_under(key).len() == 1 {
            self.keys.remove(&k);
            self.writes += 1;
        }
        Ok(())
    }
}

#[cfg(windows)]
pub use crate::windows::Hkcu;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mem_registry_is_case_insensitive_and_prunes() {
        let mut reg = MemRegistry::default();
        reg.set(r"A\B\C", "", "x").unwrap();
        assert!(reg.exists(r"a\b").unwrap());
        assert_eq!(reg.get(r"a\B\c", "").unwrap().as_deref(), Some("x"));
        reg.remove_if_empty(r"A\B").unwrap();
        assert!(reg.exists(r"A\B").unwrap(), "has a subkey");
        reg.remove_tree(r"A\B\C").unwrap();
        reg.remove_if_empty(r"A\B").unwrap();
        assert!(!reg.exists(r"A\B").unwrap());
        assert!(reg.exists("A").unwrap());
    }
}
