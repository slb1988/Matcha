//! Launch-perf tracing (`MATCHA_TABLE_PERF=1`, alias `MATCHA_PERF=1`):
//! absolute marks and spans on stderr for the open paths of every comparison
//! view. Zero-cost when disabled (one cached env lookup per call site).
//!
//! Set `MATCHA_TABLE_PERF_EXIT=1` (alias `MATCHA_PERF_EXIT=1`) alongside to
//! close the window right after the first frame that renders the ready
//! comparison.
//!
//! Frame-time tracing (`MATCHA_FRAME_PERF=1`): every frame's `ui` time and
//! the interval since the previous frame are recorded; frames over the 16 ms
//! budget are logged as they happen and a histogram is printed on exit.

use std::sync::OnceLock;
use std::time::Instant;

static T0: OnceLock<Instant> = OnceLock::new();
static ENABLED: OnceLock<bool> = OnceLock::new();

pub fn enabled() -> bool {
    *ENABLED.get_or_init(|| {
        std::env::var_os("MATCHA_TABLE_PERF").is_some() || std::env::var_os("MATCHA_PERF").is_some()
    })
}

/// Close the window after the first ready frame (launch measurements).
pub fn exit_after_ready() -> bool {
    std::env::var_os("MATCHA_TABLE_PERF_EXIT").is_some()
        || std::env::var_os("MATCHA_PERF_EXIT").is_some()
}

static FRAMES: OnceLock<bool> = OnceLock::new();

pub fn frames_enabled() -> bool {
    *FRAMES.get_or_init(|| std::env::var_os("MATCHA_FRAME_PERF").is_some())
}

/// Per-frame cost log (`MATCHA_FRAME_PERF=1`); summary printed on drop.
#[derive(Default)]
pub struct FrameLog {
    ui_ms: Vec<f32>,
    interval_ms: Vec<f32>,
    last: Option<Instant>,
}
impl FrameLog {
    /// Frame budget at 60 Hz.
    pub const BUDGET_MS: f32 = 16.0;
    /// Record one frame that started at `started` and spent `ui_ms` in `ui`.
    pub fn record(&mut self, started: Instant, ui_ms: f32, view: &str) {
        if let Some(last) = self.last {
            self.interval_ms
                .push(started.duration_since(last).as_secs_f32() * 1e3);
        }
        self.last = Some(started);
        self.ui_ms.push(ui_ms);
        if ui_ms > Self::BUDGET_MS {
            eprintln!(
                "[frame-perf] t={:>9.1}ms slow frame #{} view={view} ui={ui_ms:.1}ms",
                t0().elapsed().as_secs_f64() * 1e3,
                self.ui_ms.len()
            );
        }
    }
    pub fn summary(&self) -> String {
        let stats = |v: &[f32]| {
            if v.is_empty() {
                return "n=0".to_string();
            }
            let mut s = v.to_vec();
            s.sort_by(f32::total_cmp);
            let q = |p: f64| s[((s.len() - 1) as f64 * p).round() as usize];
            format!(
                "n={} p50={:.2} p90={:.2} p99={:.2} max={:.2}",
                s.len(),
                q(0.5),
                q(0.9),
                q(0.99),
                s[s.len() - 1]
            )
        };
        let over = self.ui_ms.iter().filter(|&&m| m > Self::BUDGET_MS).count();
        format!(
            "[frame-perf] ui_ms {} over_{}ms={over}
[frame-perf] interval_ms {}",
            stats(&self.ui_ms),
            Self::BUDGET_MS,
            stats(&self.interval_ms)
        )
    }
}
impl Drop for FrameLog {
    fn drop(&mut self) {
        if !self.ui_ms.is_empty() {
            eprintln!("{}", self.summary());
        }
    }
}

/// Process-launch anchor; first call wins, so `main` should call it early.
pub fn t0() -> Instant {
    *T0.get_or_init(Instant::now)
}

pub fn mark(label: &str) {
    if enabled() {
        eprintln!(
            "[table-perf] t={:>9.1}ms {label}",
            t0().elapsed().as_secs_f64() * 1e3
        );
    }
}

pub fn span(label: &str, start: Instant) {
    if enabled() {
        eprintln!(
            "[table-perf] t={:>9.1}ms {label} took {:.1}ms",
            t0().elapsed().as_secs_f64() * 1e3,
            start.elapsed().as_secs_f64() * 1e3
        );
    }
}
