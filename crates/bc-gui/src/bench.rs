//! 正式文本控件的滚动基准 (`matcha --bench`)。
//!
//! Drives the dual-pane view through a fixed scroll pattern, records per-frame cost and
//! prints a summary to stdout before closing the window. Results feed
//! `docs/ui-framework-decision.md`.

use std::time::{Duration, Instant};

/// Save the app framebuffer without a desktop capture API or an image dependency.
pub fn save_screenshot(
    path: &std::path::Path,
    image: &eframe::egui::ColorImage,
) -> std::io::Result<()> {
    use std::io::Write;
    let width = image.width() as u32;
    let height = image.height() as u32;
    let stride = (width * 3 + 3) & !3;
    let size = 54 + stride * height;
    let mut header = vec![0u8; 54];
    header[..2].copy_from_slice(b"BM");
    header[2..6].copy_from_slice(&size.to_le_bytes());
    header[10..14].copy_from_slice(&54u32.to_le_bytes());
    header[14..18].copy_from_slice(&40u32.to_le_bytes());
    header[18..22].copy_from_slice(&width.to_le_bytes());
    header[22..26].copy_from_slice(&(-(height as i32)).to_le_bytes());
    header[26..28].copy_from_slice(&1u16.to_le_bytes());
    header[28..30].copy_from_slice(&24u16.to_le_bytes());
    let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);
    file.write_all(&header)?;
    let mut row = vec![0u8; stride as usize];
    for y in 0..height as usize {
        for x in 0..width as usize {
            let c = image.pixels[y * width as usize + x];
            row[x * 3..x * 3 + 3].copy_from_slice(&[c.b(), c.g(), c.r()]);
        }
        file.write_all(&row)?;
    }
    file.flush()
}

/// Benchmark configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchOptions {
    /// Frames to measure after warm-up.
    pub frames: usize,
    /// Warm-up frames excluded from statistics.
    pub warmup: usize,
}

impl Default for BenchOptions {
    fn default() -> Self {
        Self {
            frames: 600,
            warmup: 30,
        }
    }
}

/// Scroll pattern per frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Advance a few rows (reading pace).
    Smooth(usize),
    /// Jump to an absolute row (scrollbar drag / go-to-line).
    Jump(usize),
}

/// Running benchmark state.
#[derive(Debug)]
pub struct BenchState {
    pub options: BenchOptions,
    frame: usize,
    update_times: Vec<Duration>,
    cpu_times: Vec<f32>,
    rows_drawn: Vec<usize>,
    rng: u64,
    pub started: Instant,
}

impl BenchState {
    pub fn new(options: BenchOptions) -> Self {
        Self {
            options,
            frame: 0,
            update_times: Vec::new(),
            cpu_times: Vec::new(),
            rows_drawn: Vec::new(),
            rng: 0x9E37_79B9_7F4A_7C15,
            started: Instant::now(),
        }
    }

    /// Next scroll step. Mix: 3 smooth frames then one jump, over `total_rows`.
    pub fn next_step(&mut self, total_rows: usize) -> Step {
        self.frame += 1;
        if self.frame.is_multiple_of(4) {
            // xorshift64*
            self.rng ^= self.rng >> 12;
            self.rng ^= self.rng << 25;
            self.rng ^= self.rng >> 27;
            let r = self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D);
            Step::Jump((r % total_rows.max(1) as u64) as usize)
        } else {
            Step::Smooth(3)
        }
    }

    /// Record one measured frame.
    pub fn record(&mut self, update_time: Duration, cpu_usage: Option<f32>, rows_drawn: usize) {
        if self.frame <= self.options.warmup {
            return;
        }
        self.update_times.push(update_time);
        if let Some(c) = cpu_usage {
            self.cpu_times.push(c);
        }
        self.rows_drawn.push(rows_drawn);
    }

    pub fn is_done(&self) -> bool {
        self.frame >= self.options.warmup + self.options.frames
    }

    /// Human readable summary (also machine-greppable `key=value` lines).
    pub fn summary(
        &self,
        left_bytes: usize,
        right_bytes: usize,
        load_left: Duration,
        load_right: Duration,
        index_bytes: usize,
    ) -> String {
        let mut upd: Vec<f64> = self
            .update_times
            .iter()
            .map(|d| d.as_secs_f64() * 1e3)
            .collect();
        upd.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut cpu: Vec<f64> = self.cpu_times.iter().map(|&c| c as f64 * 1e3).collect();
        cpu.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let pct = |v: &[f64], p: f64| -> f64 {
            if v.is_empty() {
                return 0.0;
            }
            let idx = ((v.len() - 1) as f64 * p).round() as usize;
            v[idx]
        };
        let mean = |v: &[f64]| -> f64 {
            if v.is_empty() {
                0.0
            } else {
                v.iter().sum::<f64>() / v.len() as f64
            }
        };
        let rows = mean(
            &self
                .rows_drawn
                .iter()
                .map(|&r| r as f64)
                .collect::<Vec<_>>(),
        );
        let wall = self.started.elapsed().as_secs_f64();
        let measured = self.update_times.len();
        format!(
            "bench.frames_measured={measured}\n\
             bench.wall_seconds={wall:.2}\n\
             bench.fps_overall={:.1}\n\
             bench.left_mb={:.1} bench.right_mb={:.1}\n\
             bench.load_left_ms={:.0} bench.load_right_ms={:.0} bench.index_mb={:.1}\n\
             bench.update_ms.mean={:.3} p50={:.3} p95={:.3} p99={:.3} max={:.3}\n\
             bench.frame_cpu_ms.mean={:.3} p50={:.3} p95={:.3} p99={:.3} max={:.3}\n\
             bench.rows_drawn_per_frame={rows:.0}",
            (self.frame as f64) / wall.max(1e-9),
            left_bytes as f64 / 1e6,
            right_bytes as f64 / 1e6,
            load_left.as_secs_f64() * 1e3,
            load_right.as_secs_f64() * 1e3,
            index_bytes as f64 / 1e6,
            mean(&upd),
            pct(&upd, 0.5),
            pct(&upd, 0.95),
            pct(&upd, 0.99),
            pct(&upd, 1.0),
            mean(&cpu),
            pct(&cpu, 0.5),
            pct(&cpu, 0.95),
            pct(&cpu, 0.99),
            pct(&cpu, 1.0),
        )
    }
}
