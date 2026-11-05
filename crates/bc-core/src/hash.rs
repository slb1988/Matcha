//! Content hashing. BC hashes lines with xxHash for alignment and uses CRC32 for
//! folder-compare criteria (`TVfsGetCRC32Job` on the VFS side).

/// 64-bit content hash of a line or block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContentHash(pub u64);

/// Strategy for hashing line content prior to alignment.
///
/// Normalisation flags correspond to the text-session comparison options
/// (ignore whitespace / case / unimportant text) that BC folds into the hash so the
/// aligner can treat equivalent lines as equal.
#[derive(Debug, Clone, Copy, Default)]
pub struct LineHasher {
    pub ignore_leading_whitespace: bool,
    pub ignore_embedded_whitespace: bool,
    pub ignore_trailing_whitespace: bool,
    pub ignore_case: bool,
}

impl LineHasher {
    /// Hash a single line's bytes.
    pub fn hash_line(&self, line: &[u8]) -> ContentHash {
        todo!("bc-core::hash: xxHash64 with normalisation")
    }

    /// Hash every line of a list (bulk, cache-friendly).
    pub fn hash_lines(&self, lines: &crate::line::LineList) -> Vec<ContentHash> {
        todo!("bc-core::hash: bulk hashing")
    }
}

/// Streaming CRC32 (IEEE) used by folder compare `criteria CRC`.
#[derive(Debug, Default)]
pub struct Crc32 {
    state: u32,
}

impl Crc32 {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, bytes: &[u8]) {
        todo!("bc-core::hash: crc32 update")
    }

    pub fn finish(self) -> u32 {
        todo!("bc-core::hash: crc32 finish (final xor of {:#x})", self.state)
    }
}
