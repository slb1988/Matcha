//! Engine-side launch-perf spans (`MATCHA_TABLE_PERF=1`). Durations only;
//! correlation to the GUI timeline comes from the surrounding worker spans.

use std::sync::OnceLock;
use std::time::Instant;

static ENABLED: OnceLock<bool> = OnceLock::new();

pub fn enabled() -> bool {
    *ENABLED.get_or_init(|| std::env::var_os("MATCHA_TABLE_PERF").is_some())
}

pub fn span(label: &str, start: Instant) {
    if enabled() {
        eprintln!(
            "[table-perf] engine: {label} took {:.1}ms",
            start.elapsed().as_secs_f64() * 1e3
        );
    }
}

/// Conservative worker count for the chunked engine phases (row-text build,
/// verify filter, compare): one 4096-row quantum per worker, capped at 8 and
/// at the machine's parallelism, so the GUI and the OS keep cores. All users
/// employ scoped threads (no pool, no new dependency); outputs are assembled
/// in chunk order, so the count never affects results.
pub(crate) fn phase_threads(rows: usize) -> usize {
    let available = std::thread::available_parallelism().map_or(1, |n| n.get());
    (rows / 4096).clamp(1, available.min(8))
}
