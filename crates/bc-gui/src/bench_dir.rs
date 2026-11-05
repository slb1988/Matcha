//! Real native-window folder benchmark; timing includes App::ui, not just row painting.
/// Small uncompressed BMP writer avoids an image-library dependency for test evidence.
pub fn write_bmp(path:&std::path::Path,image:&crate::egui::ColorImage)->std::io::Result<()> {
    use std::io::Write;
    let width=image.size[0] as u32;let height=image.size[1] as u32;
    let stride=(width*3+3)&!3;let bytes=stride*height;
    let mut file=std::io::BufWriter::new(std::fs::File::create(path)?);
    file.write_all(b"BM")?;file.write_all(&(54+bytes).to_le_bytes())?;file.write_all(&[0;4])?;file.write_all(&54u32.to_le_bytes())?;
    file.write_all(&40u32.to_le_bytes())?;file.write_all(&width.to_le_bytes())?;file.write_all(&height.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;file.write_all(&24u16.to_le_bytes())?;file.write_all(&0u32.to_le_bytes())?;
    file.write_all(&bytes.to_le_bytes())?;file.write_all(&[0;16])?;
    let padding=vec![0;(stride-width*3) as usize];
    for y in (0..height as usize).rev() {
        for x in 0..width as usize {let pixel=image.pixels[y*width as usize+x].to_array();file.write_all(&[pixel[2],pixel[1],pixel[0]])?;}
        file.write_all(&padding)?;
    }
    file.flush()
}
use crate::dir_view::DirCompareView;
use std::time::{Duration, Instant};
pub struct DirBench {
    frame: usize,
    samples: Vec<f64>,
    rows: Vec<usize>,
    rng: u64,
    started: Instant,
    pub failed: bool,
}
impl Default for DirBench {
    fn default() -> Self {
        Self {
            frame: 0,
            samples: Vec::new(),
            rows: Vec::new(),
            rng: 0x9e3779b97f4a7c15,
            started: Instant::now(),
            failed: false,
        }
    }
}
impl DirBench {
    pub fn drive(&mut self, view: &mut DirCompareView,ctx:&crate::egui::Context) {
        if view.busy() || !view.snapshot.complete {
            return;
        }
        self.frame += 1;
        if self.frame==1 {
            println!("bench.dir.ready=true");
            ctx.request_screenshot(|image| {
                std::thread::spawn(move || {
                    let path=std::env::temp_dir().join(format!("matcha-dir-bench-{}.bmp",std::process::id()));
                    match write_bmp(&path,&image) {
                        Ok(())=>println!("bench.dir.screenshot={}",path.display()),
                        Err(error)=>eprintln!("benchmark screenshot: {error}"),
                    }
                });
            });
        }
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        let row = if self.frame % 4 == 0 {
            self.rng as usize % view.row_count().max(1)
        } else {
            view.last_stats.top_row + 3
        };
        view.scroll_to_row(row);
    }
    pub fn record(&mut self, elapsed: Duration, view: &DirCompareView) -> bool {
        if self.frame > 30 && !view.busy() && view.snapshot.complete {
            self.samples.push(elapsed.as_secs_f64() * 1000.0);
            self.rows.push(view.last_stats.rows_drawn);
        }
        if self.started.elapsed() > Duration::from_secs(180) && self.samples.is_empty() {
            self.failed = true;
            return true;
        }
        self.samples.len() >= 600
    }
    pub fn summary(&self, view: &DirCompareView) -> String {
        let mut samples = self.samples.clone();
        samples.sort_by(f64::total_cmp);
        let percentile = |p: f64| {
            samples
                .get(((samples.len().saturating_sub(1)) as f64 * p).round() as usize)
                .copied()
                .unwrap_or(f64::INFINITY)
        };
        let p99 = percentile(0.99);
        let pass =
            !self.failed && samples.len() >= 600 && p99 < 4.0 && view.snapshot.files >= 100_000;
        format!("bench.dir.frames={}\nbench.dir.file_count={}\nbench.dir.row_count={}\nbench.dir.update_ms.p50={:.3}\nbench.dir.update_ms.p95={:.3}\nbench.dir.update_ms.p99={p99:.3}\nbench.dir.update_ms.max={:.3}\nbench.dir.rows_drawn.max={}\nbench.dir.wall_seconds={:.3}\nbench.dir.pass={pass}",
            samples.len(),view.snapshot.files,view.row_count(),percentile(0.5),percentile(0.95),percentile(1.0),self.rows.iter().copied().max().unwrap_or(0),self.started.elapsed().as_secs_f64())
    }
}
