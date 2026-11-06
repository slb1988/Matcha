//! cargo run --release -p bc-engine-text --example text_map_bench -- <left> <right>
use bc_engine_text::{jobs::Align2Job, TextContent, TextMap};
use std::{path::PathBuf, time::Instant};
#[cfg(windows)]
fn private_bytes() -> std::io::Result<u64> {
    let out = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "(Get-Process -Id {}).PrivateMemorySize64",
                std::process::id()
            ),
        ])
        .output()?;
    if !out.status.success() {
        return Err(std::io::Error::other("memory sampler failed"));
    }
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .map_err(std::io::Error::other)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 2 {
        return Err("expected two input paths".into());
    }
    #[cfg(windows)]
    let baseline = private_bytes()?;
    let start = Instant::now();
    let left = TextContent::open(&args[0], None, 1252)?;
    let right = TextContent::open(&args[1], None, 1252)?;
    println!(
        "bench.content_load_ms={:.3}",
        start.elapsed().as_secs_f64() * 1000.0
    );
    let mut map = TextMap::new(left, right, None, Default::default());
    let job = Align2Job::from_map(&map);
    let start = Instant::now();
    let output = std::thread::spawn(move || job.execute(&bc_core::cancel::NeverCancel))
        .join()
        .expect("worker")?;
    map.set_alignment(output)?;
    println!(
        "bench.text_map_ms={:.3}",
        start.elapsed().as_secs_f64() * 1000.0
    );
    println!("bench.rows={}", map.row_count());
    println!("bench.map_storage_bytes={}", map.storage_bytes());
    println!("bench.difference_sections={:?}", map.difference_counts());
    #[cfg(windows)]
    {
        let growth = private_bytes()?.saturating_sub(baseline);
        let input = std::fs::metadata(&args[0])?.len() + std::fs::metadata(&args[1])?.len();
        println!("bench.private_growth_bytes={growth}");
        println!(
            "bench.private_input_ratio={:.6}",
            growth as f64 / input as f64
        );
        assert!(growth <= 2 * input, "private memory budget exceeded");
    }
    Ok(())
}
