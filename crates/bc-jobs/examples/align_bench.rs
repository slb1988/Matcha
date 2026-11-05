//! cargo run --release -p bc-jobs --example align_bench -- <left> <right>
//! Background alignment with the real CancelToken; inputs are never written.
use bc_core::{
    align::{AlignInput, QuickAlign, SmartAlign},
    cancel::CancelCheck,
    hash::LineHasher,
    line::LineList,
    CoreError,
};
use bc_jobs::CancelToken;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if paths.len() != 2 {
        return Err("expected left and right input paths".into());
    }
    let start = Instant::now();
    let left = LineList::open_mapped(&paths[0])?;
    let right = LineList::open_mapped(&paths[1])?;
    println!(
        "bench.load_ms={:.3}",
        start.elapsed().as_secs_f64() * 1000.0
    );
    println!(
        "bench.input_bytes={},{}",
        std::fs::metadata(&paths[0])?.len(),
        std::fs::metadata(&paths[1])?.len()
    );
    let start = Instant::now();
    let lh = LineHasher::default().hash_lines(&left);
    let rh = LineHasher::default().hash_lines(&right);
    println!(
        "bench.hash_ms={:.3}",
        start.elapsed().as_secs_f64() * 1000.0
    );
    for smart in [false, true] {
        let name = if smart { "smart" } else { "quick" };
        let token = CancelToken::new();
        let run = |cancel: &dyn CancelCheck| {
            let l = AlignInput {
                hashes: &lh,
                lines: Some(&left),
                ..Default::default()
            };
            let r = AlignInput {
                hashes: &rh,
                lines: Some(&right),
                ..Default::default()
            };
            if smart {
                SmartAlign::default().align2_cancellable(l, r, &Default::default(), cancel)
            } else {
                QuickAlign.align2_cancellable(l, r, &Default::default(), cancel)
            }
        };
        let start = Instant::now();
        let map = std::thread::scope(|scope| {
            scope
                .spawn(|| run(&token))
                .join()
                .expect("alignment worker")
        })?;
        let elapsed = start.elapsed();
        println!("bench.{name}.seconds={:.6}", elapsed.as_secs_f64());
        println!("bench.{name}.rows={}", map.len());
        assert!(elapsed.as_secs_f64() < if smart { 15.0 } else { 5.0 });
        drop(map);
        let (tx, rx) = mpsc::sync_channel(1);
        let announced = AtomicBool::new(false);
        std::thread::scope(|scope| {
            let check = || {
                if !announced.swap(true, Ordering::SeqCst) {
                    tx.send(()).unwrap();
                }
                CancelCheck::is_cancelled(&token)
            };
            let worker = scope.spawn(move || run(&check));
            rx.recv().unwrap();
            std::thread::sleep(Duration::from_millis(10));
            let start = Instant::now();
            token.cancel();
            let result = worker.join().expect("cancellation worker");
            assert!(
                matches!(result, Err(CoreError::Cancelled)),
                "must cancel a running comparison"
            );
            let latency = start.elapsed();
            println!(
                "bench.{name}.cancel_ms={:.3}",
                latency.as_secs_f64() * 1000.0
            );
            assert!(latency < Duration::from_secs(1));
        });
    }
    Ok(())
}
