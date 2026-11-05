#![allow(dead_code)]
use bc_core::cancel::CancelCheck;
use bc_engine_dir::{
    op_exec::{ConfirmPolicy, OpLog, OpOutcome},
    ops::*,
    DirTree,
};
use bc_jobs::{CancelToken, Confirmation, Interaction, JobContext, JobId, Progress, ProgressSink};
use bc_vfs::*;
use std::{
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::SystemTime,
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
pub struct Temp(pub PathBuf);
impl Temp {
    pub fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "matcha-ops-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(p.join("left")).unwrap();
        fs::create_dir(p.join("right")).unwrap();
        Self(p)
    }
    pub fn write(&self, side: &str, name: &str, bytes: &[u8]) {
        let p = self.0.join(side).join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, bytes).unwrap();
    }
    pub fn path(&self, side: &str, name: &str) -> PathBuf {
        self.0.join(side).join(name)
    }
    pub fn tree(&self, vfs: &VfsManager) -> DirTree {
        let mut t = DirTree::new();
        t.bases = [
            Some(VfsPath::local(&self.path("left", ""))),
            Some(VfsPath::local(&self.path("right", ""))),
            None,
        ];
        bc_engine_dir::refresh::full_refresh(&mut t, vfs, &|| false).unwrap();
        t
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fn clear(path: &Path) {
            if let Ok(items) = fs::read_dir(path) {
                for e in items.flatten() {
                    if e.file_type().is_ok_and(|t| t.is_dir()) {
                        clear(&e.path());
                    } else {
                        let _ = LocalFs::new()
                            .set_attrs(&VfsPath::local(&e.path()), Default::default());
                    }
                }
            }
        }
        clear(&self.0);
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[derive(Default)]
pub struct ProbeFs {
    pub reads: AtomicUsize,
    pub lists: AtomicUsize,
    pub permanent: AtomicUsize,
    pub no_recycle: bool,
    pub deny_delete: Option<String>,
}
impl Vfs for ProbeFs {
    fn scheme(&self) -> &'static str {
        "file"
    }
    fn list(&self, p: &VfsPath) -> VfsResult<Vec<VfsEntry>> {
        self.lists.fetch_add(1, Ordering::Relaxed);
        LocalFs::new().list(p)
    }
    fn metadata(&self, p: &VfsPath) -> VfsResult<VfsEntry> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        LocalFs::new().metadata(p)
    }
    fn open_read(&self, p: &VfsPath) -> VfsResult<Box<dyn std::io::Read + Send>> {
        LocalFs::new().open_read(p)
    }
    fn get(&self, a: &VfsPath, b: &Path) -> VfsResult<()> {
        LocalFs::new().get(a, b)
    }
    fn put(&self, a: &Path, b: &VfsPath) -> VfsResult<()> {
        LocalFs::new().put(a, b)
    }
    fn delete(&self, p: &VfsPath, recycle: bool) -> VfsResult<()> {
        if recycle && self.no_recycle {
            return Err(VfsError::Unsupported("test volume cannot recycle"));
        }
        if self.deny_delete.as_deref() == p.file_name() {
            return Err(VfsError::PermissionDenied(p.clone()));
        }
        if !recycle {
            self.permanent.fetch_add(1, Ordering::Relaxed);
        }
        LocalFs::new().delete(p, recycle)
    }
    fn rename(&self, a: &VfsPath, b: &VfsPath) -> VfsResult<()> {
        LocalFs::new().rename(a, b)
    }
    fn make_dir(&self, p: &VfsPath) -> VfsResult<()> {
        LocalFs::new().make_dir(p)
    }
    fn crc32(&self, p: &VfsPath) -> VfsResult<u32> {
        LocalFs::new().crc32(p)
    }
    fn set_modified(&self, p: &VfsPath, t: SystemTime) -> VfsResult<()> {
        LocalFs::new().set_modified(p, t)
    }
    fn set_attrs(&self, p: &VfsPath, a: FileAttrs) -> VfsResult<()> {
        LocalFs::new().set_attrs(p, a)
    }
    fn watch(&self, p: &VfsPath) -> VfsResult<Box<dyn Watcher>> {
        LocalFs::new().watch(p)
    }
    fn copy_file(
        &self,
        a: &VfsPath,
        b: &VfsPath,
        o: CopyOptions,
        c: &dyn CancelCheck,
        p: &mut dyn FnMut(u64),
    ) -> VfsResult<()> {
        LocalFs::new().copy_file(a, b, o, c, p)
    }
    fn move_file(
        &self,
        a: &VfsPath,
        b: &VfsPath,
        o: CopyOptions,
        c: &dyn CancelCheck,
        p: &mut dyn FnMut(u64),
    ) -> VfsResult<()> {
        LocalFs::new().move_file(a, b, o, c, p)
    }
    fn exchange_files(
        &self,
        a: &VfsPath,
        b: &VfsPath,
        c: &dyn CancelCheck,
        p: &mut dyn FnMut(u64),
    ) -> VfsResult<()> {
        LocalFs::new().exchange_files(a, b, c, p)
    }
    fn remove_empty_dir(&self, p: &VfsPath) -> VfsResult<()> {
        LocalFs::new().remove_empty_dir(p)
    }
}
pub fn manager(probe: Arc<ProbeFs>) -> Arc<VfsManager> {
    let mut manager = VfsManager::new();
    manager.register(probe);
    Arc::new(manager)
}
#[derive(Default)]
pub struct ProgressLog(pub Mutex<Vec<Progress>>);
impl ProgressSink for ProgressLog {
    fn report(&self, _: JobId, p: &Progress) {
        self.0.lock().unwrap().push(p.clone());
    }
}
pub struct Answers {
    pub values: Mutex<VecDeque<Confirmation>>,
    pub questions: Mutex<Vec<String>>,
}
impl Answers {
    pub fn new(values: impl IntoIterator<Item = Confirmation>) -> Arc<Self> {
        Arc::new(Self {
            values: Mutex::new(values.into_iter().collect()),
            questions: Mutex::default(),
        })
    }
}
impl Interaction for Answers {
    fn confirm(&self, q: &str) -> Option<Confirmation> {
        self.questions.lock().unwrap().push(q.into());
        self.values.lock().unwrap().pop_front()
    }
}
pub fn context(answers: Option<Arc<Answers>>) -> JobContext {
    JobContext {
        id: JobId(1),
        cancel: CancelToken::new(),
        progress: Arc::new(ProgressLog::default()),
        interaction: answers.map(|a| a as Arc<dyn Interaction>),
    }
}
pub fn execute(
    plan: &OpPlan,
    vfs: &VfsManager,
    ctx: &JobContext,
    policy: ConfirmPolicy,
) -> (OpOutcome, OpLog) {
    let log = OpLog::default();
    let result = bc_engine_dir::op_exec::execute(plan, vfs, ctx, policy, &log);
    (result, log)
}
pub fn selected(tree: &DirTree, paths: &[&str], side: usize) -> Selection {
    let mut s = Selection::default();
    for p in paths {
        s.insert(tree.find(p).unwrap(), side);
    }
    s
}
pub fn plan(tree: &DirTree, paths: &[&str], side: usize, op: DirOp) -> OpPlan {
    OpPlan::build(
        tree,
        &selected(tree, paths, side),
        &op,
        Default::default(),
        SystemTime::now(),
    )
    .unwrap()
}
