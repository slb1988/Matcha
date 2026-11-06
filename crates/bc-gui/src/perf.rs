//! Launch-perf tracing (`MATCHA_TABLE_PERF=1`): absolute marks and spans on
//! stderr, aimed at the table-compare open path. Zero-cost when disabled
//! (one cached env lookup per call site).
//!
//! Set `MATCHA_TABLE_PERF_EXIT=1` alongside to close the window right after
//! the first frame that renders the ready comparison grid.

use std::sync::OnceLock;
use std::time::Instant;

static T0: OnceLock<Instant> = OnceLock::new();
static ENABLED: OnceLock<bool> = OnceLock::new();

pub fn enabled() -> bool {
    *ENABLED.get_or_init(|| std::env::var_os("MATCHA_TABLE_PERF").is_some())
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
