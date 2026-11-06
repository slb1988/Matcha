//! Generate repeatable GUI benchmark input, exclusively under the OS temp directory.
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    time::{SystemTime, UNIX_EPOCH},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rows = std::env::args()
        .nth(1)
        .map(|s| s.parse::<usize>())
        .transpose()?
        .unwrap_or(1_000_000);
    let dir = std::env::temp_dir().join(format!(
        "matcha-table-gui-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    fs::create_dir(&dir)?;
    for side in ["left", "right"] {
        let path = dir.join(format!("{side}.csv"));
        let mut file = BufWriter::new(File::create(&path)?);
        writeln!(file, "id,name,code,amount,date,flag,note,total,tag,region")?;
        for i in 0..rows {
            let amount = (i % 1000) as f64
                + 0.25
                + if side == "right" && i.is_multiple_of(100) {
                    0.5
                } else {
                    0.0
                };
            writeln!(
                file,
                "{i},item{},code{},{amount:.2},2026-01-{:02},{},note {},{},tag{},region{}",
                i % 100,
                i % 300,
                i % 28 + 1,
                i.is_multiple_of(2),
                i % 20,
                i % 500,
                i % 12,
                i % 8
            )?;
        }
        file.flush()?;
        println!("{side}={}", path.display());
    }
    println!("cleanup={}", dir.display());
    Ok(())
}
