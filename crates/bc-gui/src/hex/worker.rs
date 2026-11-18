//! 每视图一个可取消的后台对齐 job（`bc_jobs::Worker`，ADR 008）；与 text 视图同约定：
//! 保存/卸载前 join，绝不泄漏 mmap 快照。
//!
//! 携带 revision 的快照 job：过期结果由 `HexMap::set_alignment` 拒绝。
pub type Worker = bc_jobs::Worker<bc_engine_bin::hex::HexAlignOutput>;

#[cfg(test)]
mod tests {
    use super::*;
    use bc_engine_bin::hex::{HexAlignJob, HexContent, HexMap, HexStats};

    /// 后台执行 → poll 收取 → set_alignment 回填（revision 一致时）。
    #[test]
    fn worker_roundtrip_applies_fresh_output() {
        // 非退化内容（全零内容里单字节翻转会被锚点机制表示为孤儿对——合法对齐）；
        // 确定性伪随机基底上有明确的差异点。
        let mut seed = 0xC0FFEEu64;
        let mut rand = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 32) as u8
        };
        let left: Vec<u8> = (0..256).map(|_| rand()).collect();
        let mut right = left.clone();
        right[0x80] ^= 0xFF;
        let mut map = HexMap {
            left: HexContent::from_bytes(left),
            right: HexContent::from_bytes(right),
            ..Default::default()
        };
        let job = HexAlignJob::from_map(&map);
        let mut worker = Worker::spawn(Box::new(job));
        let output = loop {
            if let Some(result) = worker.poll() {
                break result.unwrap();
            }
            std::thread::yield_now();
        };
        map.set_alignment(output).unwrap();
        assert_eq!(
            map.stats(),
            HexStats {
                same: 255,
                difference: 1,
                ..Default::default()
            }
        );
    }

    /// 过期结果不污染视图模型：快照后编辑 → set_alignment 拒绝。
    #[test]
    fn stale_output_is_dropped() {
        let mut map = HexMap {
            left: HexContent::from_bytes(vec![0u8; 64]),
            right: HexContent::from_bytes(vec![0u8; 64]),
            ..Default::default()
        };
        let job = HexAlignJob::from_map(&map);
        let mut worker = Worker::spawn(Box::new(job));
        map.right.replace(0..0, &[0xFF]).unwrap(); // 编辑触发 revision 前进
        let output = loop {
            if let Some(result) = worker.poll() {
                break result.unwrap();
            }
            std::thread::yield_now();
        };
        assert!(map.set_alignment(output).is_err());
        assert!(map.align.is_empty()); // 旧对齐未被覆盖
    }
}
