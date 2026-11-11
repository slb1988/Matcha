//! 任务 7.1：256 MB 语料 Fast 档对齐的性能与可取消性（手动跑：`--ignored`）。
//!
//! 设计 D1：Fast 面向超大文件。该基准记录耗时与取消响应，数字回填 worker-report。

use bc_core::align::{ByteAlign, ByteAlignStrategy};
use bc_core::cancel::CancelCheck;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 确定性伪随机大块内容（xorshift64 逐 8 字节填充，比逐字节快 8 倍）。
fn big_bytes(seed: u64, len: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut s = seed.max(1);
    while out.len() < len {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        out.extend_from_slice(&s.to_le_bytes());
    }
    out.truncate(len);
    out
}

#[test]
#[ignore = "256 MB 基准：手动 cargo test --release -p bc-engine-bin -- --ignored"]
fn fast_align_256mb_is_cancellable_and_bounded() {
    const SIZE: usize = 256 * 1024 * 1024;
    let left = big_bytes(0x5EED_0001, SIZE);
    let mut right = big_bytes(0x5EED_0001, SIZE);
    // 中部 4 KB 插入 + 尾部一段改动。
    right.splice(128 * 1024 * 1024..128 * 1024 * 1024, vec![0xAB; 4096]);
    for b in &mut right[SIZE - 1024..] {
        *b ^= 0xFF;
    }

    // 1) 完整对齐计时。
    let start = Instant::now();
    let map = ByteAlign.align(&left, &right, ByteAlignStrategy::Fast);
    let elapsed = start.elapsed();
    let (mut same, mut lo, mut ro) = (0usize, 0usize, 0usize);
    for row in map.rows() {
        match row.kind {
            bc_core::DiffKind::Same => same += 1,
            bc_core::DiffKind::LeftOnly => lo += 1,
            bc_core::DiffKind::RightOnly => ro += 1,
            bc_core::DiffKind::Changed => {}
        }
    }
    eprintln!("[7.1] Fast 256MB align: {elapsed:?}; same={same} left_orphan={lo} right_orphan={ro}");
    // 2) 取消响应：在对齐中置取消标志，要求及时返回（< 5s）。
    let flag = Arc::new(AtomicBool::new(false));
    struct Flag(Arc<AtomicBool>);
    impl CancelCheck for Flag {
        fn is_cancelled(&self) -> bool {
            self.0.load(Ordering::Relaxed)
        }
    }
    let cancel_flag = Flag(flag.clone());
    let start = Instant::now();
    std::thread::spawn({
        let flag = flag.clone();
        move || {
            std::thread::sleep(Duration::from_millis(200));
            flag.store(true, Ordering::Relaxed);
        }
    });
    let result = ByteAlign.align_cancellable(&left, &right, ByteAlignStrategy::Fast, &cancel_flag);
    let cancel_elapsed = start.elapsed();
    eprintln!("[7.1] cancel latency: {cancel_elapsed:?}");
    assert!(matches!(result, Err(bc_core::CoreError::Cancelled)));
    assert!(cancel_elapsed < Duration::from_secs(5), "取消响应过慢: {cancel_elapsed:?}");

    // 1) 完整对齐计时与结构断言。
    // Fast 档分段允许比 Complete 粗糙（spec 明示）；不变量由 AlignMap 构造保证，
    // 这里锚定数量级：孤儿总量 ∈ [6,144, 24,576]（4096 插入 + 尾部 1024×2 的双侧孤儿），
    // same 字节数不低于“尺寸 − 24K”。
    assert!(same > SIZE - 24 * 1024, "same={same}");
    assert!((6144..=24 * 1024).contains(&(lo + ro)), "orphans={}", lo + ro);
}
