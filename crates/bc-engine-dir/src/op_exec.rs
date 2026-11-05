//! Job execution consumes an immutable plan and publishes durable per-item outcomes.
use crate::{ops::*, tree::NodeId, DirTree};
use bc_jobs::{Confirmation, JobContext, Progress};
use bc_vfs::{FileAttrs, VfsEntry, VfsError, VfsManager, VfsPath, VfsResult};
use std::{
    collections::BTreeMap,
    io::Write,
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConfirmPolicy {
    #[default]
    Prompt,
    YesToAll,
    NoToAll,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpState {
    Started,
    Completed,
    Skipped,
    Failed,
    Cancelled,
    Info,
}
#[derive(Debug, Clone)]
pub struct OpLogEntry {
    pub index: usize,
    pub action: OpAction,
    pub source: Option<VfsPath>,
    pub destination: Option<VfsPath>,
    pub state: OpState,
    pub message: String,
}
#[derive(Debug, Clone, Default)]
pub struct OpLog(Arc<Mutex<Vec<OpLogEntry>>>);
impl OpLog {
    pub fn snapshot(&self) -> Vec<OpLogEntry> {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
    pub fn append(&self, entry: OpLogEntry) {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).push(entry);
    }
    pub fn write_to(&self, mut writer: impl Write) -> std::io::Result<()> {
        for e in self.snapshot() {
            writeln!(
                writer,
                "{}\t{:?}\t{}\t{}\t{:?}\t{}",
                e.index,
                e.action,
                e.source.as_ref().map_or(String::new(), ToString::to_string),
                e.destination
                    .as_ref()
                    .map_or(String::new(), ToString::to_string),
                e.state,
                e.message.replace(['\r', '\n'], " ")
            )?;
        }
        Ok(())
    }
}
#[derive(Debug, Clone)]
pub struct SessionEffect {
    pub node: NodeId,
    pub revision: u64,
    pub action: OpAction,
    pub relative: String,
    pub is_dir: bool,
}
#[derive(Debug, Clone, Default)]
pub struct OpOutcome {
    pub completed: usize,
    pub skipped: usize,
    pub failed: usize,
    pub cancelled: bool,
    pub bytes_written: u64,
    pub affected: Vec<Location>,
    pub effects: Vec<SessionEffect>,
    pub filenames: Vec<String>,
}
impl OpOutcome {
    pub fn apply_session(
        &self,
        tree: &mut DirTree,
        filters: &mut crate::filter_script::FilterSettings,
    ) -> Result<(), String> {
        let mut masks = filters.names.to_spec();
        for effect in &self.effects {
            if tree.node(effect.node).map(|n| n.revision) != Some(effect.revision) {
                continue;
            }
            match effect.action {
                OpAction::Ignored(value) => tree.set_ignored(effect.node, value),
                OpAction::Exclude => {
                    tree.set_excluded(effect.node, true);
                    if !masks.is_empty() {
                        masks.push(';');
                    }
                    let escaped = effect
                        .relative
                        .chars()
                        .map(|c| match c {
                            '[' => "[[]".into(),
                            '*' => "[*]".into(),
                            '?' => "[?]".into(),
                            _ => c.to_string(),
                        })
                        .collect::<String>();
                    masks.push_str(&format!(
                        "-./{escaped}{}",
                        if effect.is_dir { "/" } else { "" }
                    ));
                }
                _ => {}
            }
        }
        filters.names = crate::NameFilter::parse(&masks)?;
        Ok(())
    }
    pub fn filename_text(&self) -> String {
        self.filenames.join("\r\n")
    }
}
fn fingerprint_equal(a: &VfsEntry, b: &VfsEntry) -> bool {
    a.kind == b.kind
        && a.size == b.size
        && a.modified == b.modified
        && a.attrs == b.attrs
        && a.error == b.error
}
pub(crate) fn metadata_optional(vfs: &VfsManager, path: &VfsPath) -> VfsResult<Option<VfsEntry>> {
    match vfs.resolve(path)?.metadata(path) {
        Ok(e) => Ok(Some(e)),
        Err(VfsError::NotFound(_)) => Ok(None),
        Err(VfsError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
fn same_snapshot(expected: Option<&VfsEntry>, actual: Option<&VfsEntry>) -> bool {
    match (expected, actual) {
        (None, None) => true,
        (Some(a), Some(b)) => fingerprint_equal(a, b),
        _ => false,
    }
}
fn merged_attrs(old: FileAttrs, set: FileAttrs, clear: FileAttrs) -> FileAttrs {
    FileAttrs {
        readonly: (old.readonly || set.readonly) && !clear.readonly,
        hidden: (old.hidden || set.hidden) && !clear.hidden,
        system: (old.system || set.system) && !clear.system,
        archive: (old.archive || set.archive) && !clear.archive,
        unix_mode: old.unix_mode,
    }
}
struct Confirmations<'a> {
    ctx: &'a JobContext,
    policy: ConfirmPolicy,
    cache: BTreeMap<ConfirmKind, bool>,
}
impl Confirmations<'_> {
    fn permit(&mut self, kind: ConfirmKind, item: &OpItem) -> bool {
        let fallback = kind == ConfirmKind::PermanentFallback;
        if !fallback {
            if let Some(value) = self.cache.get(&kind) {
                return *value;
            }
            match self.policy {
                ConfirmPolicy::YesToAll => return true,
                ConfirmPolicy::NoToAll => return false,
                ConfirmPolicy::Prompt => {}
            }
        }
        let question = format!(
            "{kind:?}: {:?}: {} -> {}{}",
            item.action,
            item.source
                .as_ref()
                .map_or(String::new(), |p| p.path.to_string()),
            item.destination
                .as_ref()
                .map_or(String::new(), |p| p.path.to_string()),
            if fallback {
                "; recycle unavailable/declined; independently confirm PERMANENT deletion"
            } else {
                ""
            }
        );
        let answer = self
            .ctx
            .interaction
            .as_ref()
            .and_then(|i| i.confirm(&question))
            .unwrap_or(Confirmation::No);
        if !fallback {
            match answer {
                Confirmation::YesToAll => {
                    self.cache.insert(kind, true);
                }
                Confirmation::NoToAll => {
                    self.cache.insert(kind, false);
                }
                _ => {}
            }
        }
        matches!(answer, Confirmation::Yes | Confirmation::YesToAll)
    }
}
/// Errors are per item. Cancellation returns the partial result so completed effects
/// and affected paths cannot be lost through a generic JobError.
pub fn execute(
    plan: &OpPlan,
    vfs: &VfsManager,
    ctx: &JobContext,
    policy: ConfirmPolicy,
    log: &OpLog,
) -> OpOutcome {
    let mut outcome = OpOutcome::default();
    let mut confirmations = Confirmations {
        ctx,
        policy,
        cache: BTreeMap::new(),
    };
    let mut changed = Vec::<Location>::new();
    for (index, original) in plan.items.iter().enumerate() {
        let event = |state, message: String| {
            log.append(OpLogEntry {
                index,
                action: original.action.clone(),
                source: original.source.as_ref().map(|p| p.path.clone()),
                destination: original.destination.as_ref().map(|p| p.path.clone()),
                state,
                message,
            })
        };
        if ctx.cancel.is_cancelled() {
            outcome.cancelled = true;
            event(
                OpState::Cancelled,
                format!("cancelled after {} completed items", outcome.completed),
            );
            break;
        }
        event(OpState::Started, String::new());
        let current_file = original
            .source
            .as_ref()
            .or(original.destination.as_ref())
            .map_or(String::new(), |p| p.path.to_string());
        ctx.report(Progress {
            current: index as u64,
            total: Some(plan.items.len() as u64),
            message: format!(
                "item {}/{}; bytes={}; file={current_file}; {:?}",
                index + 1,
                plan.items.len(),
                outcome.bytes_written,
                original.action
            ),
        });
        let mut item = original.clone();
        let result = (|| -> VfsResult<bool> {
            let session = matches!(
                item.action,
                OpAction::Exclude | OpAction::Ignored(_) | OpAction::CopyFilename
            );
            if !session {
                if let Some(source) = &item.source {
                    let actual = metadata_optional(vfs, &source.path)?;
                    // Child operations legitimately change their parent directory mtime.
                    let directory_cleanup = matches!(
                        item.action,
                        OpAction::RemoveEmptyDir { .. }
                            | OpAction::EnsureDir
                            | OpAction::PreserveDir
                    ) && item
                        .source_entry
                        .as_ref()
                        .is_some_and(|e| e.kind == bc_vfs::EntryKind::Dir);
                    if directory_cleanup {
                        if actual.as_ref().map(|e| e.kind) != Some(bc_vfs::EntryKind::Dir) {
                            return Err(VfsError::Other(
                                "source directory changed since planning".into(),
                            ));
                        }
                    } else if !same_snapshot(item.source_entry.as_ref(), actual.as_ref()) {
                        return Err(VfsError::Other(
                            "source changed since planning; replan required".into(),
                        ));
                    }
                }
                if let Some(dest) = &item.destination {
                    let actual = metadata_optional(vfs, &dest.path)?;
                    let same_directory = item.action == OpAction::EnsureDir
                        && actual
                            .as_ref()
                            .is_some_and(|e| e.kind == bc_vfs::EntryKind::Dir);
                    if item.target_known
                        && !same_directory
                        && !same_snapshot(item.target_entry.as_ref(), actual.as_ref())
                    {
                        return Err(VfsError::Other(
                            "destination changed since planning; replan required".into(),
                        ));
                    }
                    item.target_entry = actual;
                    if item.action == OpAction::Rename {
                        if let Some(source) = &item.source {
                            if path_key(&source.path) == path_key(&dest.path) {
                                let from = source
                                    .path
                                    .to_local_path()
                                    .ok_or(VfsError::Unsupported("non-local case rename"))?;
                                let to = dest
                                    .path
                                    .to_local_path()
                                    .ok_or(VfsError::Unsupported("non-local case rename"))?;
                                if std::fs::canonicalize(from)? != std::fs::canonicalize(to)? {
                                    return Err(VfsError::Other(
                                        "case rename would replace another file".into(),
                                    ));
                                }
                                item.target_entry = None;
                            }
                        }
                    }
                }
                item.confirmations = super::ops::confirmations(&item);
                for kind in item.confirmations.clone() {
                    if !confirmations.permit(kind, &item) {
                        return Ok(false);
                    }
                }
            }
            if ctx.cancel.is_cancelled() {
                return Err(VfsError::Cancelled);
            }
            let source = item.source.as_ref();
            let destination = item.destination.as_ref();
            let mut report_bytes = |delta: u64| {
                outcome.bytes_written += delta;
                ctx.report(Progress {
                    current: index as u64,
                    total: Some(plan.items.len() as u64),
                    message: format!(
                        "item {}/{}; bytes={}; file={current_file}; {:?}",
                        index + 1,
                        plan.items.len(),
                        outcome.bytes_written,
                        item.action
                    ),
                });
            };
            // Mutations may partially complete before an error/cancellation. Refresh both
            // endpoints conservatively; reads and session-only effects need no disk refresh.
            if !session {
                if let Some(p) = source {
                    changed.push(p.clone());
                }
                if let Some(p) = destination {
                    changed.push(p.clone());
                }
            }
            let mut cleared = None;
            if matches!(
                item.action,
                OpAction::Copy | OpAction::Move | OpAction::Rename
            ) && item.target_entry.as_ref().is_some_and(|e| e.attrs.readonly)
            {
                let path = &destination.unwrap().path;
                let old = item.target_entry.as_ref().unwrap().attrs;
                vfs.resolve(path)?.set_attrs(
                    path,
                    FileAttrs {
                        readonly: false,
                        ..old
                    },
                )?;
                cleared = Some((path.clone(), old));
            }
            let perform = (|| -> VfsResult<bool> {
                match &item.action {
                    OpAction::Copy | OpAction::Move => {
                        let from = &source.unwrap().path;
                        let to = &destination.unwrap().path;
                        if !from.scheme().eq_ignore_ascii_case(to.scheme()) {
                            return Err(VfsError::Unsupported("cross-provider transfer"));
                        }
                        let options = bc_vfs::CopyOptions {
                            overwrite: item.target_entry.is_some(),
                            ..plan.options.copy
                        };
                        let provider = vfs.resolve(from)?;
                        if item.action == OpAction::Copy {
                            provider.copy_file(
                                from,
                                to,
                                options,
                                &ctx.cancel,
                                &mut report_bytes,
                            )?;
                        } else {
                            provider.move_file(
                                from,
                                to,
                                options,
                                &ctx.cancel,
                                &mut report_bytes,
                            )?;
                        }
                    }
                    OpAction::Rename => {
                        let from = &source.unwrap().path;
                        let to = &destination.unwrap().path;
                        if item.target_entry.is_some() {
                            return Err(VfsError::Other(
                                "rename destination exists; no destructive replacement".into(),
                            ));
                        }
                        vfs.resolve(from)?.rename(from, to)?;
                    }
                    OpAction::Exchange => {
                        let from = &source.unwrap().path;
                        let to = &destination.unwrap().path;
                        vfs.resolve(from)?.exchange_files(
                            from,
                            to,
                            &ctx.cancel,
                            &mut report_bytes,
                        )?;
                    }
                    OpAction::Delete { recycle } => {
                        let path = &source.unwrap().path;
                        let provider = vfs.resolve(path)?;
                        let old = item.source_entry.as_ref().map(|e| e.attrs);
                        if old.is_some_and(|a| a.readonly) {
                            provider.set_attrs(
                                path,
                                FileAttrs {
                                    readonly: false,
                                    ..old.unwrap()
                                },
                            )?;
                        }
                        let deletion = provider.delete(path, *recycle);
                        let result = match deletion {
                            Err(VfsError::Unsupported(_)) if *recycle => {
                                event(OpState::Info,"recycle unavailable or declined; independent permanent-delete confirmation required".into());
                                if confirmations.permit(ConfirmKind::PermanentFallback, &item) {
                                    provider.delete(path, false).map(|_| true)
                                } else {
                                    Ok(false)
                                }
                            }
                            other => other.map(|_| true),
                        };
                        if !matches!(result, Ok(true)) {
                            if let Some(attrs) = old {
                                let _ = provider.set_attrs(path, attrs);
                            }
                        }
                        return result;
                    }
                    OpAction::EnsureDir => {
                        let path = &destination.unwrap().path;
                        vfs.resolve(path)?.make_dir(path)?;
                    }
                    OpAction::PreserveDir => {
                        let from = &source.unwrap().path;
                        let to = &destination.unwrap().path;
                        vfs.resolve(from)?
                            .copy_metadata(from, to, plan.options.copy)?;
                        // Moving children changes the source directory mtime; use the
                        // timestamp captured in the reviewed plan, not that new mtime.
                        if plan.options.copy.timestamps {
                            if let Some(time) = item.source_entry.as_ref().and_then(|e| e.modified)
                            {
                                vfs.resolve(to)?.set_modified(to, time)?;
                            }
                        }
                    }
                    OpAction::RemoveEmptyDir { recycle } => {
                        let path = &source.unwrap().path;
                        let provider = vfs.resolve(path)?;
                        if !provider.list(path)?.is_empty() {
                            event(
                                OpState::Info,
                                "directory retained: hidden, skipped or new children remain".into(),
                            );
                            return Ok(false);
                        }
                        if *recycle {
                            match provider.delete(path, true) {
                                Err(VfsError::Unsupported(_)) => {
                                    event(OpState::Info,"recycle unavailable for empty directory; independent confirmation required".into());
                                    if !confirmations.permit(ConfirmKind::PermanentFallback, &item)
                                    {
                                        return Ok(false);
                                    }
                                    provider.remove_empty_dir(path)?;
                                }
                                other => other?,
                            }
                        } else {
                            provider.remove_empty_dir(path)?;
                        }
                    }
                    OpAction::Touch(when) => {
                        let path = &source.unwrap().path;
                        vfs.resolve(path)?.set_modified(path, *when)?;
                    }
                    OpAction::Attrib { set, clear } => {
                        let path = &source.unwrap().path;
                        let old = item.source_entry.as_ref().unwrap().attrs;
                        vfs.resolve(path)?
                            .set_attrs(path, merged_attrs(old, *set, *clear))?;
                    }
                    OpAction::Exclude | OpAction::Ignored(_) => {
                        outcome.effects.push(SessionEffect {
                            node: item.node,
                            revision: item.revision,
                            action: item.action.clone(),
                            relative: source.unwrap().relative.clone(),
                            is_dir: item
                                .source_entry
                                .as_ref()
                                .is_some_and(|e| e.kind == bc_vfs::EntryKind::Dir),
                        });
                    }
                    OpAction::CopyFilename => {
                        outcome.filenames.push(source.unwrap().path.to_string())
                    }
                }
                Ok(true)
            })();
            if perform.is_err() {
                if let Some((path, attrs)) = cleared {
                    let _ = vfs.resolve(&path).and_then(|p| p.set_attrs(&path, attrs));
                }
            }
            perform
        })();
        match result {
            Ok(true) => {
                outcome.completed += 1;
                event(OpState::Completed, "completed".into());
            }
            Ok(false) => {
                outcome.skipped += 1;
                event(
                    OpState::Skipped,
                    "not authorized or directory retained".into(),
                );
            }
            Err(VfsError::Cancelled) => {
                outcome.cancelled = true;
                event(
                    OpState::Cancelled,
                    format!("cancelled after {} completed items", outcome.completed),
                );
                break;
            }
            Err(e) => {
                outcome.failed += 1;
                event(OpState::Failed, e.to_string());
            }
        }
        ctx.report(Progress {
            current: (index + 1) as u64,
            total: Some(plan.items.len() as u64),
            message: format!(
                "completed={}, skipped={}, failed={}, bytes={}",
                outcome.completed, outcome.skipped, outcome.failed, outcome.bytes_written
            ),
        });
    }
    // Keep the destination spelling for a case-only rename, not the old source spelling.
    let mut changed_by_path = BTreeMap::new();
    for place in changed {
        changed_by_path.insert((place.side, path_key(&place.path)), place);
    }
    outcome.affected = changed_by_path.into_values().collect();
    outcome
}
