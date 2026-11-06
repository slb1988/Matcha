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
