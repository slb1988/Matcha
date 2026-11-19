//! 文件在 Matcha 之外被修改后的检测与重载（`docs/features/disk-change-reload.md`）。
//!
//! 视图实现 [`DiskTracked`]：报告每侧的磁盘路径与是否有未保存编辑、保存每侧的
//! [`Tracked`] 基准、能「保持位置」地重载某一侧。决策（自动重载 / 询问 / 文件消失）
//! 全部在 [`plan`] 里，`app.rs` 在窗口重新获得焦点或切换标签页时调用 [`check`]，
//! 每帧调用 [`prompt`] 绘制确认框。
use bc_session::prefs::DiskChangePrefs;
use eframe::egui;
use std::{
    io,
    path::{Path, PathBuf},
    time::SystemTime,
};

/// 判定「磁盘上的文件变了」的依据：修改时间 + 大小（与参考一致，不做内容哈希）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiskStamp {
    pub modified: Option<SystemTime>,
    pub len: u64,
}

/// 读取 `path` 当前的磁盘标识。
pub fn stamp(path: &Path) -> io::Result<DiskStamp> {
    let meta = std::fs::metadata(path)?;
    if !meta.is_file() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a file"));
    }
    Ok(DiskStamp {
        modified: meta.modified().ok(),
        len: meta.len(),
    })
}

/// 一侧的检测状态（视图每侧一个）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tracked {
    /// 载入或保存成功时的磁盘标识；`None` = 不参与检查（剪贴板、读取失败）。
    pub loaded: Option<DiskStamp>,
    /// 用户选择不重载时的磁盘标识：同一标识不再询问。
    pub declined: Option<DiskStamp>,
    /// 已报告过「文件不存在或不可读」（同一状态只报一次）。
    pub gone_reported: bool,
}

impl Tracked {
    /// 刚从 `path` 载入或保存到 `path`：以当前磁盘状态为新基准。
    pub fn reset(&mut self, path: Option<&Path>) {
        *self = Self {
            loaded: path.and_then(|p| stamp(p).ok()),
            ..Self::default()
        };
    }
}

/// 视图报告的一侧。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedSide {
    pub side: usize,
    pub path: PathBuf,
    /// 该侧有未保存编辑（重载会丢弃它们）。
    pub dirty: bool,
}

/// 确认框里的一项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changed {
    pub side: usize,
    pub path: PathBuf,
    pub dirty: bool,
    /// 发现变化时的磁盘标识（拒绝时记入 `declined`）。
    pub stamp: DiskStamp,
}

/// 等待用户回答的「File changed on disk, reload?」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskPrompt {
    pub changed: Vec<Changed>,
}

pub trait DiskTracked {
    /// 参与检查的各侧（有磁盘路径的）；视图忙（后台重载中）时返回空。
    fn disk_sides(&self) -> Vec<TrackedSide>;
    fn disk_state(&mut self, side: usize) -> &mut Tracked;
    fn disk_prompt(&mut self) -> &mut Option<DiskPrompt>;
    /// 重新读入 `side`，保留当前行与滚动位置；成功后该侧 `Tracked` 以新磁盘状态为基准。
    fn disk_reload(&mut self, side: usize) -> Result<(), String>;
    /// 状态栏消息。
    fn disk_message(&mut self, message: String);
}

/// 一次检查的决定（纯数据，便于单测）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    /// 直接重载（自动重载开启且该侧没有未保存编辑）。
    pub reload: Vec<Changed>,
    /// 要询问的（一个确认框列出全部）。
    pub ask: Vec<Changed>,
    /// 不存在或不可读的文件。
    pub gone: Vec<PathBuf>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.reload.is_empty() && self.ask.is_empty() && self.gone.is_empty()
    }
}

/// 算出本次检查要做什么；`stat` 读磁盘标识（测试注入假的）。会更新各侧的
/// `gone_reported`。确认框已打开或检查关闭时什么都不做。
pub fn plan(
    view: &mut dyn DiskTracked,
    prefs: &DiskChangePrefs,
    stat: &dyn Fn(&Path) -> io::Result<DiskStamp>,
) -> Plan {
    let mut plan = Plan::default();
    if !prefs.check || view.disk_prompt().is_some() {
        return plan;
    }
    for TrackedSide { side, path, dirty } in view.disk_sides() {
        let state = view.disk_state(side);
        let Some(loaded) = state.loaded else {
            continue;
        };
        let now = match stat(&path) {
            Ok(now) => now,
            Err(_) => {
                if !state.gone_reported {
                    state.gone_reported = true;
                    plan.gone.push(path);
                }
                continue;
            }
        };
        state.gone_reported = false;
        if now == loaded || Some(now) == state.declined {
            continue;
        }
        let changed = Changed {
            side,
            path,
            dirty,
            stamp: now,
        };
        if prefs.auto_reload && !dirty {
            plan.reload.push(changed);
        } else {
            plan.ask.push(changed);
        }
    }
    plan
}

/// 检查并执行：自动重载、打开确认框、状态栏提示。
pub fn check(view: &mut dyn DiskTracked, prefs: &DiskChangePrefs) {
    let plan = plan(view, prefs, &stamp);
    apply(view, plan);
}

fn name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn apply(view: &mut dyn DiskTracked, plan: Plan) {
    let mut messages = Vec::new();
    for changed in plan.reload {
        match view.disk_reload(changed.side) {
            Ok(()) => messages.push(format!(
                "Reloaded {} (changed on disk)",
                name(&changed.path)
            )),
            Err(e) => messages.push(format!("Cannot reload {}: {e}", name(&changed.path))),
        }
    }
    for path in plan.gone {
        messages.push(format!(
            "{} no longer exists on disk or cannot be read",
            name(&path)
        ));
    }
    if !messages.is_empty() {
        view.disk_message(messages.join("; "));
    }
    if !plan.ask.is_empty() {
        *view.disk_prompt() = Some(DiskPrompt { changed: plan.ask });
    }
}

/// 用户的回答：`true` = 重载列出的各侧，`false` = 保留当前内容。
pub fn answer(view: &mut dyn DiskTracked, reload: bool) {
    let Some(prompt) = view.disk_prompt().take() else {
        return;
    };
    if reload {
        apply(
            view,
            Plan {
                reload: prompt.changed,
                ..Plan::default()
            },
        );
    } else {
        for changed in prompt.changed {
            view.disk_state(changed.side).declined = Some(changed.stamp);
        }
    }
}

/// 绘制确认框（有待回答的变化时）并处理回答。
pub fn prompt(ctx: &egui::Context, view: &mut dyn DiskTracked) {
    let Some(prompt) = view.disk_prompt().clone() else {
        return;
    };
    use crate::chrome::{
        controls::Role,
        modal::{file_item, Modal, Tone},
    };
    let badge = |side: usize| if side == 0 { "LEFT" } else { "RIGHT" };
    let dirty: Vec<String> = prompt
        .changed
        .iter()
        .filter(|c| c.dirty)
        .map(|c| name(&c.path))
        .collect();
    let mut message = if prompt.changed.len() > 1 {
        "These files were modified outside Matcha.".to_owned()
    } else {
        "This file was modified outside Matcha.".to_owned()
    };
    if !dirty.is_empty() {
        message += &format!(
            " Unsaved changes to {} will be lost if you reload.",
            dirty.join(" and ")
        );
    }
    let chosen = Modal::new(
        "disk-changed",
        Tone::Question,
        "File changed on disk, reload?",
    )
    .message(message)
    .items(
        prompt
            .changed
            .iter()
            .map(|c| file_item(badge(c.side), &c.path)),
    )
    .action("Keep Current", Role::Secondary)
    .action("Reload", Role::Primary)
    .cancel(0)
    .show(ctx);
    match chosen {
        Some(0) => answer(view, false),
        Some(1) => answer(view, true),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::HashMap, time::Duration};

    fn at(secs: u64, len: u64) -> DiskStamp {
        DiskStamp {
            modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(secs)),
            len,
        }
    }

    #[derive(Default)]
    struct Fake {
        sides: Vec<TrackedSide>,
        states: [Tracked; 2],
        prompt: Option<DiskPrompt>,
        reloaded: Vec<usize>,
        message: Option<String>,
    }

    impl Fake {
        fn new(dirty: [bool; 2]) -> Self {
            let mut fake = Self::default();
            for (side, dirty) in dirty.into_iter().enumerate() {
                fake.sides.push(TrackedSide {
                    side,
                    path: PathBuf::from(if side == 0 { "a.txt" } else { "b.txt" }),
                    dirty,
                });
                fake.states[side].loaded = Some(at(100, 10));
            }
            fake
        }
    }

    impl DiskTracked for Fake {
        fn disk_sides(&self) -> Vec<TrackedSide> {
            self.sides.clone()
        }
        fn disk_state(&mut self, side: usize) -> &mut Tracked {
            &mut self.states[side]
        }
        fn disk_prompt(&mut self) -> &mut Option<DiskPrompt> {
            &mut self.prompt
        }
        fn disk_reload(&mut self, side: usize) -> Result<(), String> {
            self.reloaded.push(side);
            self.states[side] = Tracked {
                loaded: Some(at(200, 20)),
                ..Tracked::default()
            };
            Ok(())
        }
        fn disk_message(&mut self, message: String) {
            self.message = Some(message);
        }
    }

    /// 假磁盘：文件名 → 标识；缺失 = 不存在。
    struct Disk(RefCell<HashMap<&'static str, DiskStamp>>);
    impl Disk {
        fn new(entries: &[(&'static str, DiskStamp)]) -> Self {
            Self(RefCell::new(entries.iter().copied().collect()))
        }
        fn stat(&self) -> impl Fn(&Path) -> io::Result<DiskStamp> + '_ {
            move |p| {
                self.0
                    .borrow()
                    .get(p.to_str().unwrap())
                    .copied()
                    .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
            }
        }
    }

    const ASK: DiskChangePrefs = DiskChangePrefs {
        check: true,
        auto_reload: false,
    };
    const AUTO: DiskChangePrefs = DiskChangePrefs {
        check: true,
        auto_reload: true,
    };

    #[test]
    fn unchanged_files_do_nothing() {
        let disk = Disk::new(&[("a.txt", at(100, 10)), ("b.txt", at(100, 10))]);
        let mut view = Fake::new([false, false]);
        assert!(plan(&mut view, &ASK, &disk.stat()).is_empty());
    }

    #[test]
    fn clean_side_reloads_silently_when_auto_reload_is_on() {
        let disk = Disk::new(&[("a.txt", at(100, 10)), ("b.txt", at(150, 10))]);
        let mut view = Fake::new([false, false]);
        let p = plan(&mut view, &AUTO, &disk.stat());
        assert_eq!(p.reload.len(), 1);
        assert_eq!(p.reload[0].side, 1);
        assert!(p.ask.is_empty());
        apply(&mut view, p);
        assert_eq!(view.reloaded, vec![1]);
        assert!(view.prompt.is_none());
        assert!(view.message.unwrap().contains("Reloaded b.txt"));
    }

    #[test]
    fn dirty_side_is_asked_even_with_auto_reload_and_says_edits_are_lost() {
        let disk = Disk::new(&[("a.txt", at(100, 11)), ("b.txt", at(100, 10))]);
        let mut view = Fake::new([true, false]);
        let p = plan(&mut view, &AUTO, &disk.stat());
        assert!(p.reload.is_empty());
        assert_eq!(p.ask.len(), 1);
        assert!(p.ask[0].dirty);
        apply(&mut view, p);
        assert!(view.reloaded.is_empty());
        assert_eq!(view.prompt.as_ref().unwrap().changed[0].side, 0);
    }

    #[test]
    fn changes_are_asked_when_auto_reload_is_off() {
        let disk = Disk::new(&[("a.txt", at(100, 10)), ("b.txt", at(150, 10))]);
        let mut view = Fake::new([false, false]);
        let p = plan(&mut view, &ASK, &disk.stat());
        assert!(p.reload.is_empty());
        assert_eq!(p.ask.len(), 1);
    }

    #[test]
    fn both_sides_changed_make_one_prompt_with_two_items() {
        let disk = Disk::new(&[("a.txt", at(150, 10)), ("b.txt", at(150, 10))]);
        let mut view = Fake::new([false, false]);
        let p = plan(&mut view, &ASK, &disk.stat());
        apply(&mut view, p);
        assert_eq!(view.prompt.as_ref().unwrap().changed.len(), 2);
        answer(&mut view, true);
        assert_eq!(view.reloaded, vec![0, 1]);
        assert!(view.prompt.is_none());
    }

    #[test]
    fn declined_change_is_not_asked_again_until_the_file_changes_again() {
        let disk = Disk::new(&[("a.txt", at(100, 10)), ("b.txt", at(150, 10))]);
        let mut view = Fake::new([false, false]);
        let p = plan(&mut view, &ASK, &disk.stat());
        apply(&mut view, p);
        answer(&mut view, false);
        assert!(view.prompt.is_none() && view.reloaded.is_empty());
        assert_eq!(view.states[1].declined, Some(at(150, 10)));
        assert!(plan(&mut view, &ASK, &disk.stat()).is_empty());
        disk.0.borrow_mut().insert("b.txt", at(160, 12));
        assert_eq!(plan(&mut view, &ASK, &disk.stat()).ask.len(), 1);
    }

    #[test]
    fn vanished_file_is_reported_once_and_never_prompts() {
        let disk = Disk::new(&[("a.txt", at(100, 10))]);
        let mut view = Fake::new([false, false]);
        let p = plan(&mut view, &AUTO, &disk.stat());
        assert_eq!(p.gone, vec![PathBuf::from("b.txt")]);
        assert!(p.ask.is_empty() && p.reload.is_empty());
        apply(&mut view, p);
        assert!(view
            .message
            .as_deref()
            .unwrap()
            .contains("b.txt no longer exists"));
        assert!(view.prompt.is_none());
        assert!(plan(&mut view, &AUTO, &disk.stat()).is_empty());
        // 文件回来且内容不同：按变化处理。
        disk.0.borrow_mut().insert("b.txt", at(300, 3));
        assert_eq!(plan(&mut view, &AUTO, &disk.stat()).reload.len(), 1);
    }

    #[test]
    fn check_off_does_nothing() {
        let disk = Disk::new(&[("a.txt", at(150, 10)), ("b.txt", at(150, 10))]);
        let mut view = Fake::new([false, false]);
        let off = DiskChangePrefs {
            check: false,
            auto_reload: true,
        };
        assert!(plan(&mut view, &off, &disk.stat()).is_empty());
    }

    #[test]
    fn open_prompt_blocks_a_second_one() {
        let disk = Disk::new(&[("a.txt", at(150, 10)), ("b.txt", at(150, 10))]);
        let mut view = Fake::new([false, false]);
        view.prompt = Some(DiskPrompt { changed: vec![] });
        assert!(plan(&mut view, &ASK, &disk.stat()).is_empty());
    }

    #[test]
    fn untracked_sides_are_skipped() {
        let disk = Disk::new(&[("a.txt", at(150, 10)), ("b.txt", at(150, 10))]);
        let mut view = Fake::new([false, false]);
        view.states[0].loaded = None;
        let p = plan(&mut view, &ASK, &disk.stat());
        assert_eq!(p.ask.len(), 1);
        assert_eq!(p.ask[0].side, 1);
    }

    #[test]
    fn stamp_reads_a_real_file_and_rejects_folders() {
        let dir = std::env::temp_dir().join(format!("matcha-disk-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("x.txt");
        std::fs::write(&file, b"hello").unwrap();
        assert_eq!(stamp(&file).unwrap().len, 5);
        assert!(stamp(&dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
