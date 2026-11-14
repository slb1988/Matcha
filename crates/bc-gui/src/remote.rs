//! 单实例转交进入 UI 线程的入口：监听线程经 [`RemoteSender`] 投递，`MatchaApp` 在
//! `App::logic`（窗口最小化时也会调用）里取出并开新标签页。
use eframe::egui;
use std::path::PathBuf;
use std::sync::{mpsc, Arc, Mutex};

/// 被转交会话的结束方式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// 标签页已关闭（含应用退出时随之关闭），附会话结果码：合并页为 0 / 14 / 101，
    /// 其余视图恒为 0。
    Closed(i32),
    /// 未能打开比较。
    Failed(String),
}

/// 会话结束时调用一次（`matcha-compc` 等待的就是它）。
pub type Completion = Box<dyn FnOnce(Outcome) + Send>;

/// 启动标签页的结果码落点（`run_with_remote` 以它作为返回值）。
pub type ResultSlot = Arc<Mutex<Option<i32>>>;

/// 一次转交：`merge` 开合并标签页；否则两侧路径都有则开比较标签页，都没有只把窗口提到前台。
pub struct RemoteOpen {
    pub left: Option<PathBuf>,
    pub right: Option<PathBuf>,
    pub merge: Option<Box<crate::merge::MergeLaunch>>,
    pub done: Option<Completion>,
}

type Waker = Arc<Mutex<Option<egui::Context>>>;

/// 监听线程侧；可跨线程共享。
#[derive(Clone)]
pub struct RemoteSender {
    tx: mpsc::Sender<RemoteOpen>,
    waker: Waker,
}

impl RemoteSender {
    /// 投递并唤醒 egui 事件循环；UI 已退出时原样退回请求。
    pub fn send(&self, open: RemoteOpen) -> Result<(), RemoteOpen> {
        self.tx.send(open).map_err(|e| e.0)?;
        if let Some(ctx) = &*self.waker.lock().unwrap() {
            ctx.request_repaint();
        }
        Ok(())
    }
}

/// UI 侧。
pub struct RemoteInbox {
    rx: mpsc::Receiver<RemoteOpen>,
    waker: Waker,
}

impl RemoteInbox {
    /// 记下上下文供监听线程唤醒；窗口创建前已到达的请求靠这次重绘处理。
    pub fn attach(&self, ctx: &egui::Context) {
        *self.waker.lock().unwrap() = Some(ctx.clone());
        ctx.request_repaint();
    }
    pub fn drain(&self) -> Vec<RemoteOpen> {
        self.rx.try_iter().collect()
    }
}

pub fn channel() -> (RemoteSender, RemoteInbox) {
    let (tx, rx) = mpsc::channel();
    let waker = Waker::default();
    (
        RemoteSender {
            tx,
            waker: waker.clone(),
        },
        RemoteInbox { rx, waker },
    )
}
