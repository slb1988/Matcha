//! Pure name, metadata, content and view filters. Matching is over relative paths.
use crate::{criteria::attribute_bits, CompareStatus};
use bc_core::{
    cancel::CancelCheck,
    pattern::{split_mask_list, Pattern},
};
use bc_vfs::{EntryKind, FileAttrs, VfsEntry, VfsError, VfsResult};
use std::{
    io::Read,
    time::{Duration, SystemTime},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mask {
    pub pattern: String,
    pub exclude: bool,
    pub folder: bool,
}
#[derive(Debug, Clone)]
struct CompiledMask {
    mask: Mask,
    matcher: Pattern,
    any_parent: bool,
}
#[derive(Debug, Clone, Default)]
pub struct NameFilter {
    masks: Vec<Mask>,
    compiled: Vec<CompiledMask>,
    case_sensitive: bool,
}
impl PartialEq for NameFilter {
    fn eq(&self, other: &Self) -> bool {
        self.masks == other.masks && self.case_sensitive == other.case_sensitive
    }
}
impl Eq for NameFilter {}

fn literal(c: char, source: &mut String) {
    if ".+()[]{}^$|\\*?".contains(c) {
        source.push('\\');
    }
    source.push(c);
}
fn segment(mask: &str) -> Result<String, String> {
    let no_extension = mask.ends_with('.');
    let mask = if no_extension {
        &mask[..mask.len() - 1]
    } else {
        mask
    };
    let mask = if mask == "*.*" { "*" } else { mask };
    let mut source = String::new();
    let mut chars = mask.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' => source.push_str(if no_extension { "[^/.]*" } else { "[^/]*" }),
            '?' => source.push_str(if no_extension { "[^/.]" } else { "[^/]" }),
            '[' => {
                source.push('[');
                if chars.peek() == Some(&'!') {
                    chars.next();
                    source.push_str("^/");
                }
                let mut count = 0;
                loop {
                    match chars.next() {
                        Some(']') if count > 0 => break,
                        Some(c) => {
                            if "[]\\^".contains(c) {
                                source.push('\\');
                            }
                            source.push(c);
                            count += 1;
                        }
                        None => return Err("unterminated or empty mask character set".into()),
                    }
                }
                source.push(']');
            }
            _ => literal(c, &mut source),
        }
    }
    Ok(source)
}
impl NameFilter {
    pub fn parse(spec: &str) -> Result<Self, String> {
        Self::parse_case(spec, false)
    }
    pub fn parse_case(spec: &str, case_sensitive: bool) -> Result<Self, String> {
        let masks: Vec<_> = split_mask_list(spec)
            .into_iter()
            .map(|m| Mask {
                pattern: m.pattern,
                exclude: m.exclude,
                folder: m.folder,
            })
            .collect();
        let mut compiled = Vec::new();
        for mask in &masks {
            let path = mask.pattern.replace('\\', "/");
            if path.contains('\0') || path.split('/').any(|s| s == "..") {
                return Err("invalid relative mask".into());
            }
            let any_parent = path.starts_with(".../");
            let rooted = path.starts_with("./");
            let path = path
                .strip_prefix("./")
                .or_else(|| path.strip_prefix(".../"))
                .unwrap_or(&path);
            let mut source = if rooted {
                "^".to_owned()
            } else {
                "^(?:.*/)?".to_owned()
            };
            let parts: Vec<_> = path.split('/').collect();
            for (index, part) in parts.iter().enumerate() {
                if *part == "..." {
                    source.push_str("(?:[^/]+/)*");
                } else {
                    source.push_str(&segment(part)?);
                    if index + 1 != parts.len() {
                        source.push('/');
                    }
                }
            }
            source.push('$');
            let matcher = Pattern::new(&source, case_sensitive).map_err(|e| e.to_string())?;
            compiled.push(CompiledMask {
                mask: mask.clone(),
                matcher,
                any_parent,
            });
        }
        Ok(Self {
            masks,
            compiled,
            case_sensitive,
        })
    }
    pub fn masks(&self) -> &[Mask] {
        &self.masks
    }
    pub fn to_spec(&self) -> String {
        self.masks
            .iter()
            .map(|m| {
                format!(
                    "{}{}{}",
                    if m.exclude { "-" } else { "" },
                    m.pattern,
                    if m.folder { "\\" } else { "" }
                )
            })
            .collect::<Vec<_>>()
            .join(";")
    }
    pub fn matches(&self, relative: &str, is_dir: bool) -> bool {
        let relative = crate::tree::normalize_path(relative);
        if relative.is_empty() {
            return true;
        } // bases always participate
        let mut folders: Vec<&str> = relative
            .match_indices('/')
            .map(|(index, _)| &relative[..index])
            .collect();
        if is_dir {
            folders.push(&relative);
        }
        let hits = |c: &CompiledMask, path: &str| c.matcher.match_at(path, 0).is_some();
        if self.compiled.iter().any(|c| {
            c.mask.exclude
                && if c.mask.folder {
                    folders.iter().any(|folder| hits(c, folder))
                } else {
                    !is_dir && hits(c, &relative)
                }
        }) {
            return false;
        }
        let has_folders = self.masks.iter().any(|m| m.folder && !m.exclude);
        let folder_included = !has_folders
            || self.compiled.iter().any(|c| {
                !c.mask.exclude && c.mask.folder && folders.iter().any(|folder| hits(c, folder))
            });
        if is_dir {
            return folder_included;
        }
        let includes: Vec<_> = self
            .compiled
            .iter()
            .filter(|c| !c.mask.folder && !c.mask.exclude)
            .collect();
        let matching = includes.iter().any(|c| hits(c, &relative));
        let parent_override = includes.iter().any(|c| c.any_parent && hits(c, &relative));
        (includes.is_empty() || matching) && (folder_included || parent_override)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cutoff {
    OlderThan(SystemTime),
    NewerThan(SystemTime),
    OlderThanDays(u32),
    NewerThanDays(u32),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeLimit {
    SmallerThan(u64),
    LargerThan(u64),
}
/// Supplied by the session: actual local midnight (as an instant), not UTC midnight.
#[derive(Debug, Clone, Copy)]
pub struct FilterClock {
    pub today_midnight: SystemTime,
    pub utc_offset_seconds: i32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentFilter {
    pub text: String,
    pub contains: bool,
    pub case_sensitive: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtherFilters {
    pub cutoff: Option<Cutoff>,
    pub attrib_include: Option<FileAttrs>,
    pub attrib_exclude: Option<FileAttrs>,
    pub size: Option<SizeLimit>,
    pub exclude_protected: bool,
    /// Each item excludes matching files; items combine by OR.
    pub content: Vec<ContentFilter>,
}
impl Default for OtherFilters {
    fn default() -> Self {
        Self {
            cutoff: None,
            attrib_include: None,
            attrib_exclude: None,
            size: None,
            exclude_protected: true,
            content: Vec::new(),
        }
    }
}
impl OtherFilters {
    pub fn has_items(&self) -> bool {
        self.cutoff.is_some()
            || self.attrib_include.is_some()
            || self.attrib_exclude.is_some()
            || self.size.is_some()
            || !self.content.is_empty()
    }
    pub fn matches_metadata(&self, entry: &VfsEntry, clock: FilterClock) -> bool {
        if self.exclude_protected && entry.attrs.hidden && entry.attrs.system {
            return false;
        }
        if entry.kind == EntryKind::Dir {
            return true;
        }
        let bits = attribute_bits(entry.attrs);
        if self
            .attrib_include
            .is_some_and(|a| bits & attribute_bits(a) == 0)
            || self
                .attrib_exclude
                .is_some_and(|a| bits & attribute_bits(a) != 0)
        {
            return false;
        }
        if let Some(time) = entry.modified {
            let older = |limit: SystemTime| time < limit;
            let newer = |limit: SystemTime| time > limit;
            let days_ago = |days: u32| {
                clock
                    .today_midnight
                    .checked_sub(Duration::from_secs(u64::from(days) * 86400))
            };
            let excluded = match self.cutoff {
                Some(Cutoff::OlderThan(t)) => older(t),
                Some(Cutoff::NewerThan(t)) => newer(t),
                Some(Cutoff::OlderThanDays(d)) => days_ago(d).is_some_and(older),
                Some(Cutoff::NewerThanDays(d)) => days_ago(d).is_some_and(newer),
                None => false,
            };
            if excluded {
                return false;
            }
        }
        if let Some(size) = entry.size {
            if match self.size {
                Some(SizeLimit::SmallerThan(n)) => size < n,
                Some(SizeLimit::LargerThan(n)) => size > n,
                None => false,
            } {
                return false;
            }
        }
        true
    }
    /// Scan UTF-8 text with bounded overlap, including needles spanning read boundaries.
    /// Invalid UTF-8 is an explicit error, not silently interpreted as a matching file.
    pub fn matches_content(
        &self,
        reader: &mut dyn Read,
        cancel: &dyn CancelCheck,
    ) -> VfsResult<bool> {
        if self.content.is_empty() {
            return Ok(true);
        }
        let patterns = self
            .content
            .iter()
            .map(|item| {
                Pattern::literal(&item.text, item.case_sensitive)
                    .map_err(|e| VfsError::Other(e.to_string()))
            })
            .collect::<VfsResult<Vec<_>>>()?;
        let keep = self
            .content
            .iter()
            .map(|item| item.text.chars().count() * 4)
            .max()
            .unwrap_or(0);
        let mut found = vec![false; patterns.len()];
        let mut buffer = vec![0; 64 * 1024];
        let mut pending = Vec::new();
        let mut tail = String::new();
        loop {
            if cancel.is_cancelled() {
                return Err(VfsError::Cancelled);
            }
            let n = reader.read(&mut buffer)?;
            pending.extend_from_slice(&buffer[..n]);
            let valid = match std::str::from_utf8(&pending) {
                Ok(text) => text.len(),
                Err(e) if e.error_len().is_none() && n != 0 => e.valid_up_to(),
                Err(e) => {
                    return Err(VfsError::Other(format!(
                        "content filter needs UTF-8 text: {e}"
                    )))
                }
            };
            let text = std::str::from_utf8(&pending[..valid])
                .map_err(|e| VfsError::Other(e.to_string()))?;
            tail.push_str(text);
            for (index, pattern) in patterns.iter().enumerate() {
                found[index] |= pattern.find_at(&tail, 0).is_some();
            }
            if self
                .content
                .iter()
                .enumerate()
                .any(|(i, filter)| filter.contains && found[i])
            {
                return Ok(false);
            }
            let mut cut = tail.len().saturating_sub(keep);
            while !tail.is_char_boundary(cut) {
                cut += 1;
            }
            tail.drain(..cut);
            pending.drain(..valid);
            if n == 0 {
                break;
            }
        }
        Ok(!self
            .content
            .iter()
            .enumerate()
            .any(|(i, f)| found[i] == f.contains))
    }
    pub fn matches_text(&self, text: &str) -> VfsResult<bool> {
        self.matches_content(&mut std::io::Cursor::new(text.as_bytes()), &|| false)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DisplayFilter {
    #[default]
    All,
    Mismatches,
    NoOrphans,
    MismatchesNoOrphans,
    Orphans,
    LeftNewer,
    RightNewer,
    LeftNewerOrphans,
    RightNewerOrphans,
    LeftOrphans,
    RightOrphans,
    Matches,
    None,
}
impl DisplayFilter {
    pub const ALL: [Self; 13] = [
        Self::All,
        Self::Mismatches,
        Self::Matches,
        Self::NoOrphans,
        Self::MismatchesNoOrphans,
        Self::Orphans,
        Self::LeftNewer,
        Self::RightNewer,
        Self::LeftNewerOrphans,
        Self::RightNewerOrphans,
        Self::LeftOrphans,
        Self::RightOrphans,
        Self::None,
    ];
    pub fn script_name(self) -> &'static str {
        match self {
            Self::All => "display-all",
            Self::Mismatches => "display-mismatches",
            Self::NoOrphans => "display-no-orphans",
            Self::MismatchesNoOrphans => "display-mismatches-no-orphans",
            Self::Orphans => "display-orphans",
            Self::LeftNewer => "display-left-newer",
            Self::RightNewer => "display-right-newer",
            Self::LeftNewerOrphans => "display-left-newer-orphans",
            Self::RightNewerOrphans => "display-right-newer-orphans",
            Self::LeftOrphans => "display-left-orphans",
            Self::RightOrphans => "display-right-orphans",
            Self::Matches => "display-matches",
            Self::None => "display-none",
        }
    }
    pub fn shows(self, status: CompareStatus) -> bool {
        use CompareStatus::*;
        match self {
            Self::All => true,
            Self::None => false,
            Self::Mismatches => !matches!(status, Same | Unknown),
            Self::Matches => status == Same,
            Self::NoOrphans => !matches!(status, LeftOrphan | RightOrphan),
            Self::MismatchesNoOrphans => matches!(
                status,
                Different | LeftNewer | RightNewer | SimilarUnimportant | Error
            ),
            Self::Orphans => matches!(status, LeftOrphan | RightOrphan),
            Self::LeftNewer => status == LeftNewer,
            Self::RightNewer => status == RightNewer,
            Self::LeftNewerOrphans => matches!(status, LeftNewer | LeftOrphan),
            Self::RightNewerOrphans => matches!(status, RightNewer | RightOrphan),
            Self::LeftOrphans => status == LeftOrphan,
            Self::RightOrphans => status == RightOrphan,
        }
    }
    pub fn shows_with_importance(self, status: CompareStatus, ignore_unimportant: bool) -> bool {
        self.shows(
            if ignore_unimportant && status == CompareStatus::SimilarUnimportant {
                CompareStatus::Same
            } else {
                status
            },
        )
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FolderDisplay {
    AlwaysShow,
    FilesAndStructure,
    #[default]
    FilesOnly,
    Flatten,
}
impl FolderDisplay {
    pub fn shows(
        self,
        status: CompareStatus,
        has_visible_files: bool,
        display: DisplayFilter,
    ) -> bool {
        match self {
            Self::AlwaysShow => true,
            Self::FilesAndStructure => display.shows(status) || has_visible_files,
            Self::FilesOnly => has_visible_files,
            Self::Flatten => false,
        }
    }
}
/// Side metadata decisions do not mutate comparison results. A hidden side is teal if
/// its partner is visible; suppression restores hidden rows and marks both sides teal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PairVisibility {
    pub visible: bool,
    pub teal: [bool; 2],
}
pub fn pair_visibility(
    present: [bool; 2],
    included: [bool; 2],
    status: CompareStatus,
    display: DisplayFilter,
    suppress: bool,
    ignore_unimportant: bool,
) -> PairVisibility {
    let display_match = display.shows_with_importance(status, ignore_unimportant);
    let visible = suppress || (display_match && (0..2).any(|s| present[s] && included[s]));
    PairVisibility {
        visible,
        teal: std::array::from_fn(|s| visible && present[s] && (!included[s] || !display_match)),
    }
}
