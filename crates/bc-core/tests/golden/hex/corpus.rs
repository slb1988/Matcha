//! hex-compare 黄金语料生成器（任务 3.1）。
//!
//! 与 `docs/reference-parity/hex-compare/corpus/gen_corpus.py` **逐字节一致**：
//! 伪随机源是 CPython 兼容的 MT19937（`random.seed(int)` 的 init_genrand+init_by_array
//! 语义，`randrange(256)` = `getrandbits(9)` 拒绝采样），因此 Rust 生成的语料与
//! Windows 采集截图/统计所用语料是同一份字节。几何参数：
//!
//! - `primary()`：left 4,160B / right 4,144B；8 处单字节改动（0x100/0x104/0x108、
//!   0x200/0x204/0x208/0x20C、0x300）、16 字节插入（0x400，`00..0F`）、32 字节删除
//!   （原始坐标 0x5F0–0x60F，插入后坐标 0x600–0x61F）。期望统计 4120/8/32/16。
//! - `odd_insert()`：left 2,048B / right 2,053B；5 字节插入（0x123，`AA..EE`）。

/// CPython `random.Random` 兼容的 MT19937（仅覆盖 `seed(u32)` + `randrange(256)` 路径）。
pub struct PyRandom {
    mt: [u32; 624],
    index: usize,
}

impl PyRandom {
    const N: usize = 624;

    pub fn new(seed: u32) -> Self {
        // init_genrand(19650218)
        let mut mt = [0u32; Self::N];
        mt[0] = 19650218;
        for i in 1..Self::N {
            mt[i] = 1812433253u32
                .wrapping_mul(mt[i - 1] ^ (mt[i - 1] >> 30))
                .wrapping_add(i as u32);
        }
        // init_by_array([seed])
        let (mut i, j) = (1usize, 0usize);
        for _ in 0..Self::N.max(1) {
            mt[i] = (mt[i] ^ (mt[i - 1] ^ (mt[i - 1] >> 30)).wrapping_mul(1664525))
                .wrapping_add(seed)
                .wrapping_add(j as u32);
            i += 1;
            if i >= Self::N {
                mt[0] = mt[Self::N - 1];
                i = 1;
            }
            // key 长度 1：j 恒为 0
        }
        for _ in 0..Self::N - 1 {
            mt[i] = (mt[i] ^ (mt[i - 1] ^ (mt[i - 1] >> 30)).wrapping_mul(1566083941))
                .wrapping_sub(i as u32);
            i += 1;
            if i >= Self::N {
                mt[0] = mt[Self::N - 1];
                i = 1;
            }
        }
        mt[0] = 0x8000_0000;
        Self { mt, index: Self::N }
    }

    fn next_u32(&mut self) -> u32 {
        const MATRIX_A: u32 = 0x9908_B0DF;
        const UPPER: u32 = 0x8000_0000;
        const LOWER: u32 = 0x7FFF_FFFF;
        if self.index >= Self::N {
            for k in 0..Self::N {
                let y = (self.mt[k] & UPPER) | (self.mt[(k + 1) % Self::N] & LOWER);
                let mut next = self.mt[(k + 397) % Self::N] ^ (y >> 1);
                if y & 1 != 0 {
                    next ^= MATRIX_A;
                }
                self.mt[k] = next;
            }
            self.index = 0;
        }
        let mut y = self.mt[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9D2C_5680;
        y ^= (y << 15) & 0xEFC6_0000;
        y ^= y >> 18;
        y
    }

    /// CPython `getrandbits(k)`（k ≤ 32）：取 32 位输出的高 k 位。
    fn getrandbits(&mut self, k: u32) -> u32 {
        self.next_u32() >> (32 - k)
    }

    /// CPython `randrange(256)`：`_randbelow` = getrandbits(9) 拒绝采样。
    pub fn randrange_256(&mut self) -> u8 {
        loop {
            let r = self.getrandbits(9);
            if r < 256 {
                return r as u8;
            }
        }
    }
}

/// 主语料：`(left.bin, right.bin)`。
pub fn primary() -> (Vec<u8>, Vec<u8>) {
    let mut rng = PyRandom::new(42);
    let mut left = b"MZ".to_vec();
    left.extend((0..58).map(|_| rng.randrange_256()));
    left.extend_from_slice(&0x80u32.to_le_bytes());
    left.extend((0..4096).map(|_| rng.randrange_256()));
    let mut right = left.clone();
    for off in [0x100usize, 0x104, 0x108, 0x200, 0x204, 0x208, 0x20C, 0x300] {
        right[off] ^= 0xFF;
    }
    right.splice(0x400..0x400, 0u8..16); // 插入 16 字节
    right.splice(0x600..0x620, []); // 删除 32 字节（插入后坐标）
    (left, right)
}

/// 奇数长度插入语料：`(l2.bin, r2.bin)`。
pub fn odd_insert() -> (Vec<u8>, Vec<u8>) {
    let mut rng = PyRandom::new(7);
    let left: Vec<u8> = (0..2048).map(|_| rng.randrange_256()).collect();
    let mut right = left.clone();
    right.splice(0x123..0x123, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE]);
    (left, right)
}
