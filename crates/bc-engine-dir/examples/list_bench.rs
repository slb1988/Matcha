//! Two generated 100,000-file trees; no fixture or output is stored in the repository.
use bc_engine_dir::{DirTree, DisplayFilter, CompareStatus};
use bc_vfs::{LocalFs, Vfs, VfsPath};
use std::{fs, path::PathBuf, time::Instant};
struct Temp(PathBuf);
impl Drop for Temp { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::temp_dir().join(format!("matcha-list-bench-{}", std::process::id()));
    fs::create_dir(&path)?;
    let temp = Temp(path);
    let paths = [temp.0.join("left"), temp.0.join("right")];
    let generation = Instant::now();
    for directory in 0..100 {
        for path in &paths { fs::create_dir_all(path.join(format!("dir{directory:03}")))?; }
        for file in 0..1000 {
            for path in &paths { fs::File::create(path.join(format!("dir{directory:03}/file{file:04}.txt")))?; }
        }
    }
    println!("generation.seconds={:.6}", generation.elapsed().as_secs_f64());
    let bases = paths.map(|p| VfsPath::local(&p));
    let vfs = LocalFs::new();
    let mut tree = DirTree::new();
    let start = Instant::now();
    for (side, base) in bases.iter().enumerate() { tree.apply_listing(side, "", vfs.list(base)?)?; }
    let rows = tree.visible_rows(DisplayFilter::All);
    let top = start.elapsed();
    println!("top_level.rows={}", rows.len());
    println!("top_level.seconds={:.6}", top.as_secs_f64());
    assert!(top.as_secs_f64() < 1.0);
    let mut listing_seconds = 0.0;
    let mut state_seconds = 0.0;
    for directory in 0..100 {
        let relative = format!("dir{directory:03}");
        for (side, base) in bases.iter().enumerate() {
            let t = Instant::now();
            let entries = vfs.list(&base.join(&relative))?;
            listing_seconds += t.elapsed().as_secs_f64();
            let t = Instant::now();
            tree.apply_listing(side, &relative, entries)?;
            state_seconds += t.elapsed().as_secs_f64();
        }
    }
    println!("enumeration.200000_files.seconds={listing_seconds:.6}");
    println!("pairing_and_quick.seconds={state_seconds:.6}");
    println!("full_tree.seconds={:.6}", start.elapsed().as_secs_f64());
    assert_eq!(tree.count_status(CompareStatus::Same), 100_000);
    tree.set_expanded(None, true);
    let t = Instant::now();
    assert_eq!(tree.visible_rows(DisplayFilter::All).len(), 100_100);
    println!("expanded_rows.seconds={:.6}", t.elapsed().as_secs_f64());
    Ok(())
}
