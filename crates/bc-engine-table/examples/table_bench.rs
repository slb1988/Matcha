//! Million-row end-to-end benchmark; all generated inputs live in OS temp only.
use bc_core::{align::AlignmentOverrides, cancel::NeverCancel, CoreError};
use bc_engine_table::{
    columns::ColumnMapping,
    inference::infer_keys,
    jobs::TableAlignJob,
    map::compare_alignment,
    source::{read_path, ReadOptions},
    KeyAlign, TableRules,
};
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::PathBuf,
    time::{Duration, Instant},
};
struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[cfg(windows)]
fn memory() -> (u64, u64) {
    let script=format!("$p=Get-Process -Id {}; Write-Output ($p.WorkingSet64.ToString()+','+$p.PeakWorkingSet64.ToString())",std::process::id());
    let output = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .expect("process memory query");
    assert!(output.status.success(), "memory query failed");
    let text = String::from_utf8(output.stdout).unwrap();
    let mut values = text.trim().split(',');
    (
        values.next().unwrap().parse().unwrap(),
        values.next().unwrap().parse().unwrap(),
    )
}
#[cfg(not(windows))]
fn memory() -> (u64, u64) {
    let status = fs::read_to_string("/proc/self/status").expect("process memory query");
    let value = |label: &str| {
        status
            .lines()
            .find(|l| l.starts_with(label))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0)
            * 1024
    };
    (value("VmRSS:"), value("VmHWM:"))
}
fn phase(name: &str, start: Instant) {
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    let (rss, peak) = memory();
    println!("phase={name} elapsed_ms={ms:.3} rss_bytes={rss} cumulative_peak_rss_bytes={peak}");
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rows = std::env::args()
        .nth(1)
        .map(|s| s.parse::<usize>())
        .transpose()?
        .unwrap_or(1_000_000);
    let temp = Temp(std::env::temp_dir().join(format!(
            "matcha-table-bench-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        )));
    fs::create_dir(&temp.0)?;
    let paths = [temp.0.join("left.csv"), temp.0.join("right.csv")];
    println!(
        "rows={rows} columns=10 preserve_source=true temp={}",
        temp.0.display()
    );
    let start = Instant::now();
    for (side, path) in paths.iter().enumerate() {
        let mut writer = BufWriter::new(File::create(path)?);
        writeln!(
            writer,
            "id,name,code,amount,date,flag,note,total,tag,region"
        )?;
        for i in 0..rows {
            let amount = (i % 1000) as f64
                + 0.25
                + if side == 1 && i.is_multiple_of(100) {
                    0.5
                } else {
                    0.0
                };
            writeln!(
                writer,
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
        writer.flush()?;
        println!(
            "input={} bytes={}",
            path.display(),
            fs::metadata(path)?.len()
        );
    }
    phase("generate", start);
    let start = Instant::now();
    let left = read_path(&paths[0], &ReadOptions::default(), &NeverCancel)?.sheet;
    assert_eq!(left.row_count(), rows);
    assert_eq!(left.column_count(), 10);
    phase("read_left", start);
    let start = Instant::now();
    let right = read_path(&paths[1], &ReadOptions::default(), &NeverCancel)?.sheet;
    assert_eq!(right.row_count(), rows);
    assert_eq!(right.column_count(), 10);
    phase("read_right", start);
    let mut rules = TableRules {
        columns: ColumnMapping::by_number(10, 10),
        ..Default::default()
    };
    let start = Instant::now();
    let suggestion = infer_keys(&left, &right, &rules, &NeverCancel)?;
    assert!(suggestion.apply(&mut rules));
    assert_eq!(rules.key_columns(), vec![0]);
    phase("infer_keys", start);
    println!("key_selection={:?}", rules.keys);
    let overrides = AlignmentOverrides::default();
    let start = Instant::now();
    let alignment = std::thread::scope(|scope| {
        scope
            .spawn(|| KeyAlign.align(&left, &right, &rules, &overrides, &NeverCancel))
            .join()
            .unwrap()
    })?;
    assert_eq!(alignment.map.len(), rows);
    phase("align_background_smart", start);
    let start = Instant::now();
    let output = compare_alignment(&left, &right, &rules, &overrides, alignment, &NeverCancel)?;
    assert_eq!(output.data.row_count(), rows);
    let stats = output
        .data
        .statistics(rules.layers, rules.ignore_unimportant);
    assert_eq!(stats.important, rows.div_ceil(100));
    assert_eq!(stats.same, rows - rows.div_ceil(100));
    phase("compare_cells", start);
    println!(
        "same_rows={} important_rows={} orphan_rows={}",
        stats.same,
        stats.important,
        stats.left_only + stats.right_only
    );
    println!(
        "cell_bytes={} compact_rows_bytes={} packed_states_bytes={}",
        std::mem::size_of::<bc_engine_table::Cell>(),
        output.data.compact_row_bytes(),
        output.data.state_bytes()
    );
    // Immutable snapshots only; never send TableMap's Rc/RefCell edit targets.
    let job = TableAlignJob {
        left: bc_engine_table::SheetStore::Dense(left.clone()),
        right: bc_engine_table::SheetStore::Dense(right.clone()),
        rules,
        overrides,
    };
    let token = bc_jobs::CancelToken::new();
    let worker_token = token.clone();
    let (started, ready) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        started.send(()).unwrap();
        job.execute(&worker_token)
    });
    ready.recv()?;
    std::thread::sleep(Duration::from_millis(10));
    let start = Instant::now();
    token.cancel();
    assert!(matches!(worker.join().unwrap(), Err(CoreError::Cancelled)));
    phase("cancel_to_join", start);
    println!("cancelled=true temp_cleanup=on_drop");
    Ok(())
}
