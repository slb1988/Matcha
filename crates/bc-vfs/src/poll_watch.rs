//! Non-Windows fallback; callers choose the polling interval.
use crate::{ChangeEvent, LocalFs, Vfs, VfsEntry, VfsPath, VfsResult, Watcher};
use std::{
    collections::{BTreeMap, BTreeSet},
    os::unix::fs::MetadataExt,
};

#[derive(Clone)]
struct Snapshot {
    entry: VfsEntry,
    identity: (u64, u64),
}
pub struct PollWatcher {
    dir: VfsPath,
    previous: BTreeMap<String, Snapshot>,
}
fn snapshot(dir: &VfsPath) -> VfsResult<BTreeMap<String, Snapshot>> {
    let mut result = BTreeMap::new();
    let mut pending = vec![(dir.clone(), String::new())];
    while let Some((path, prefix)) = pending.pop() {
        for entry in LocalFs::new().list(&path)? {
            let relative = format!("{prefix}{}", entry.name);
            let full = path.join(&entry.name);
            if entry.kind == crate::EntryKind::Dir && entry.error.is_none() {
                pending.push((full.clone(), format!("{relative}/")));
            }
            let meta = std::fs::symlink_metadata(crate::local::local(&full)?)?;
            result.insert(
                relative,
                Snapshot {
                    entry,
                    identity: (meta.dev(), meta.ino()),
                },
            );
        }
    }
    Ok(result)
}
impl PollWatcher {
    pub fn new(dir: &VfsPath) -> VfsResult<Self> {
        Ok(Self {
            dir: dir.clone(),
            previous: snapshot(dir)?,
        })
    }
}
impl Watcher for PollWatcher {
    fn poll(&mut self) -> Vec<ChangeEvent> {
        let Ok(current) = snapshot(&self.dir) else {
            return vec![ChangeEvent::Overflow];
        };
        let mut events = Vec::new();
        let mut renamed = BTreeSet::new();
        for (name, item) in &current {
            match self.previous.get(name) {
                None => {
                    let old = self.previous.iter().find(|(old_name, old)| {
                        !current.contains_key(*old_name)
                            && !renamed.contains(*old_name)
                            && old.identity == item.identity
                    });
                    if let Some((old_name, _)) = old {
                        renamed.insert(old_name.clone());
                        events.push(ChangeEvent::Renamed {
                            from: self.dir.join(old_name),
                            to: self.dir.join(name),
                        });
                    } else {
                        events.push(ChangeEvent::Created(self.dir.join(name)));
                    }
                }
                Some(old) if old.entry != item.entry || old.identity != item.identity => {
                    events.push(ChangeEvent::Modified(self.dir.join(name)))
                }
                _ => {}
            }
        }
        for name in self.previous.keys() {
            if !current.contains_key(name) && !renamed.contains(name) {
                events.push(ChangeEvent::Deleted(self.dir.join(name)));
            }
        }
        self.previous = current;
        events
    }
}
