//! Ordered built-in/user registry, compiled name masks, and Matcha-owned persistence.
use crate::{
    bcff::{self, FormatError},
    grammar::Tokenizer,
    model::{FileFormat, FormatKind},
};
use bc_core::pattern::{glob_to_pattern, split_mask_list, Pattern};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct FormatRegistry {
    formats: Vec<FileFormat>,
    builtin: Vec<bool>,
    masks: Vec<Vec<Pattern>>,
}

impl FormatRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        for format in crate::builtin_formats() {
            registry.formats.push(format.clone());
            registry.builtin.push(true);
            registry
                .masks
                .push(compile_masks(format).expect("valid built-in masks"));
        }
        registry
    }

    pub fn formats(&self) -> &[FileFormat] {
        &self.formats
    }

    pub fn is_builtin(&self, kind: FormatKind, name: &str) -> bool {
        self.position(kind, name).is_some_and(|i| self.builtin[i])
    }

    /// Append a user definition or replace an existing key in place. Built-in data
    /// stays immutable; a same-key replacement is a user override and is exported.
    pub fn upsert(&mut self, format: FileFormat) -> Result<(), FormatError> {
        Tokenizer::new(&format).map_err(|e| FormatError::BadValue(e.to_string()))?;
        let masks = compile_masks(&format)?;
        if let Some(i) = self.position(format.kind, &format.name) {
            self.formats[i] = format;
            self.builtin[i] = false;
            self.masks[i] = masks;
        } else {
            self.formats.push(format);
            self.builtin.push(false);
            self.masks.push(masks);
        }
        Ok(())
    }

    fn position(&self, kind: FormatKind, name: &str) -> Option<usize> {
        self.formats
            .iter()
            .position(|f| f.kind == kind && f.name == name)
    }

    pub fn get(&self, kind: FormatKind, name: &str) -> Option<&FileFormat> {
        self.position(kind, name).map(|i| &self.formats[i])
    }

    /// Explicit user ordering; persisted separately from compatible format XML.
    pub fn move_to(
        &mut self,
        kind: FormatKind,
        name: &str,
        index: usize,
    ) -> Result<(), FormatError> {
        let current = self
            .position(kind, name)
            .ok_or_else(|| FormatError::Missing(format!("format {name}")))?;
        if index >= self.formats.len() {
            return Err(FormatError::BadValue(
                "format order index out of range".into(),
            ));
        }
        let format = self.formats.remove(current);
        let builtin = self.builtin.remove(current);
        let masks = self.masks.remove(current);
        self.formats.insert(index, format);
        self.builtin.insert(index, builtin);
        self.masks.insert(index, masks);
        Ok(())
    }

    /// Match the final file-name component only. Positive masks are OR-ed within a
    /// format; negative/directory masks are deliberately not NameFilter evaluation.
    pub fn resolve(&self, path: &Path, kind: Option<FormatKind>) -> Option<&FileFormat> {
        let name = path.file_name()?.to_str()?;
        self.formats
            .iter()
            .zip(&self.masks)
            .find_map(|(format, masks)| {
                (kind.is_none_or(|k| k == format.kind)
                    && masks.iter().any(|p| p.match_at(name, 0).is_some()))
                .then_some(format)
            })
    }

    /// Validate the entire import before making it visible (no partial imports).
    pub fn import_xml(&mut self, xml: &str) -> Result<usize, FormatError> {
        let formats = bcff::parse_bcfileformats(xml)?;
        let count = formats.len();
        let mut staging = self.clone();
        for format in formats {
            staging.upsert(format)?;
        }
        *self = staging;
        Ok(count)
    }

    pub fn export_xml(&self) -> Result<String, FormatError> {
        let users: Vec<_> = self
            .formats
            .iter()
            .zip(&self.builtin)
            .filter(|(_, builtin)| !**builtin)
            .map(|(format, _)| format.clone())
            .collect();
        bcff::write_bcfileformats(&users)
    }

    /// Saves only user definitions plus the complete registry order. Each file is
    /// replaced atomically; a crash between files can retain an older order only.
    pub fn save_user_dir(&self, directory: &Path) -> Result<(), FormatError> {
        let xml = self.export_xml()?;
        let order: Vec<_> = self.formats.iter().map(|f| (f.kind, &f.name)).collect();
        let order =
            serde_json::to_vec_pretty(&order).map_err(|e| FormatError::BadValue(e.to_string()))?;
        std::fs::create_dir_all(directory)?;
        atomic_write(&directory.join("user-formats.xml"), xml.as_bytes())?;
        atomic_write(&directory.join("order.json"), &order)?;
        Ok(())
    }

    pub fn load_user_dir(directory: &Path) -> Result<Self, FormatError> {
        let mut registry = Self::with_builtins();
        match std::fs::read_to_string(directory.join("user-formats.xml")) {
            Ok(xml) => {
                registry.import_xml(&xml)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        match std::fs::read(directory.join("order.json")) {
            Ok(data) => {
                let order: Vec<(FormatKind, String)> = serde_json::from_slice(&data)
                    .map_err(|e| FormatError::BadValue(format!("format order: {e}")))?;
                let mut seen = std::collections::HashSet::new();
                let mut index = 0;
                for (kind, name) in order {
                    if seen.insert((kind, name.clone())) && registry.get(kind, &name).is_some() {
                        registry.move_to(kind, &name, index)?;
                        index += 1;
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        Ok(registry)
    }
}

fn compile_masks(format: &FileFormat) -> Result<Vec<Pattern>, FormatError> {
    format
        .masks
        .iter()
        .flat_map(|list| split_mask_list(list))
        .filter(|m| !m.exclude && !m.folder)
        .map(|m| {
            glob_to_pattern(&m.pattern, false)
                .map_err(|e| FormatError::BadValue(format!("format {} mask: {e}", format.name)))
        })
        .collect()
}

/// Platform setting directory; callers can override it using save/load_user_dir.
pub fn user_formats_dir() -> Result<PathBuf, FormatError> {
    #[cfg(windows)]
    let root = std::env::var_os("APPDATA").or_else(|| std::env::var_os("LOCALAPPDATA"));
    #[cfg(not(windows))]
    let root = std::env::var_os("XDG_CONFIG_HOME").or_else(|| {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config").into_os_string())
    });
    let root = root
        .filter(|p| !p.is_empty())
        .ok_or_else(|| FormatError::Missing("user configuration directory".into()))?;
    Ok(PathBuf::from(root).join("Matcha").join("formats"))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), FormatError> {
    use std::{
        io::Write,
        sync::atomic::{AtomicUsize, Ordering},
    };
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let temp = path.with_file_name(format!(
        ".matcha-formats-{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)?;
    let result = (|| -> std::io::Result<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result.map_err(Into::into)
}
