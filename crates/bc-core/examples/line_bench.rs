//! cargo run --release -p bc-core --example line_bench -- <input-file>
//! Measures the open/index wall time and process private-memory delta of one edit,
//! including retained undo snapshots. Never writes to the source.
use bc_core::{
    edit::EditItem,
    line::{LineEdit, LineList},
};
use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Instant};

#[cfg(windows)]
fn private_bytes() -> std::io::Result<u64> {
    let output = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "(Get-Process -Id {}).PrivateMemorySize64",
                std::process::id()
            ),
        ])
        .output()?;
    if !output.status.success() {
        return Err(std::io::Error::other("Get-Process failed"));
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .map_err(std::io::Error::other)
}

#[cfg(not(windows))]
fn private_bytes() -> std::io::Result<u64> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "private-memory measurement requires Windows",
    ))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("expected input file path")?,
    );
    let bytes = std::fs::metadata(&path)?.len();
    // Warm up measurement infrastructure before timing or taking a baseline.
    let _ = private_bytes();
    let start = Instant::now();
    let lines = LineList::open_mapped(&path)?;
    let open = start.elapsed();
    println!("bench.input_bytes={bytes}");
    println!("bench.backend={:?}", lines.backend());
    println!("bench.lines={}", lines.len());
    println!("bench.open_ms={:.3}", open.as_secs_f64() * 1_000.0);
    if lines.is_empty() {
        return Err("benchmark requires a nonempty file".into());
    }
    let index = lines.len() / 2;
    let target = Rc::new(RefCell::new(lines));
    let before = private_bytes()?;
    let mut edit = LineEdit::replace(&target, index, b"edited benchmark line");
    let after = private_bytes()?;
    let delta = after.saturating_sub(before);
    println!("bench.private_before={before}");
    println!("bench.private_after={after}");
    println!("bench.edit_private_delta={delta}");
    println!(
        "bench.edit_private_percent={:.6}",
        delta as f64 / bytes as f64 * 100.0
    );
    edit.undo();
    edit.redo();
    assert!(open.as_millis() < 200, "open exceeds 200 ms");
    assert!(
        delta < bytes / 10,
        "one-line edit consumes >=10% of input size"
    );
    Ok(())
}
