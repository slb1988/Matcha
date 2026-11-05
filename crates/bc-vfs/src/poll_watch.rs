//! Non-Windows fallback; callers choose the polling interval.
use crate::{ChangeEvent, LocalFs, Vfs, VfsEntry, VfsPath, VfsResult, Watcher};
use std::collections::BTreeMap;

pub struct PollWatcher {
    dir: VfsPath,
    previous: BTreeMap<String, VfsEntry>,
}
fn snapshot(dir: &VfsPath) -> VfsResult<BTreeMap<String, VfsEntry>> {
    let mut result = BTreeMap::new();
    let mut pending = vec![(dir.clone(), String::new())];
    while let Some((path, prefix)) = pending.pop() {
        for entry in LocalFs::new().list(&path)? {
            let relative = format!("{prefix}{}", entry.name);
            if entry.kind == crate::EntryKind::Dir {
                pending.push((path.join(&entry.name), format!("{relative}/")));
            }
            result.insert(relative, entry);
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
        for (name, entry) in &current {
            match self.previous.get(name) {
                None => events.push(ChangeEvent::Created(self.dir.join(name))),
                Some(old) if old != entry => {
                    events.push(ChangeEvent::Modified(self.dir.join(name)))
                }
                _ => {}
            }
        }
        for name in self.previous.keys() {
            if !current.contains_key(name) {
                events.push(ChangeEvent::Deleted(self.dir.join(name)));
            }
        }
        self.previous = current;
        events
    }
}
