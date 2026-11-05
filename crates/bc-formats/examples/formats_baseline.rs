//! 无额外依赖的基准入口：cargo run --release -p bc-formats --example formats_baseline
use std::{hint::black_box, time::Instant};

fn main() {
    let start = Instant::now();
    for _ in 0..1_000 {
        black_box(bc_formats::builtin_formats());
    }
    println!("bench.bc-formats.baseline_us={}", start.elapsed().as_micros());
}
