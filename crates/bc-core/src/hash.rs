//! Content hashing. BC hashes lines with xxHash for alignment and uses CRC32 for
//! folder-compare criteria (`TVfsGetCRC32Job` on the VFS side).

/// 64-bit content hash of a line or block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContentHash(pub u64);

/// Multiply-mix `Hasher` for maps keyed by `ContentHash`/u64. Keys are xxh64
/// outputs (already well distributed), so SipHash's cost buys nothing here.
#[derive(Debug, Clone, Copy, Default)]
pub struct U64KeyHasher(u64);
impl std::hash::Hasher for U64KeyHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x1000_0000_01b3);
        }
    }
    fn write_u64(&mut self, v: u64) {
        let mut x = v.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        x ^= x >> 29;
        self.0 = x;
    }
}
/// `BuildHasherDefault<U64KeyHasher>` shorthand for map type aliases.
pub type U64KeyBuild = std::hash::BuildHasherDefault<U64KeyHasher>;

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
        if !self.ignore_leading_whitespace
            && !self.ignore_embedded_whitespace
            && !self.ignore_trailing_whitespace
            && !self.ignore_case
        {
            return ContentHash(xxhash_rust::xxh64::xxh64(line, 0));
        }
        let mut normal = Vec::with_capacity(line.len());
        if let Ok(text) = std::str::from_utf8(line) {
            let start = text.len() - text.trim_start_matches(char::is_whitespace).len();
            let end = text.trim_end_matches(char::is_whitespace).len();
            for (i, c) in text.char_indices() {
                if c.is_whitespace() && self.skip_whitespace(i, start, end) {
                    continue;
                }
                let mut buffer = [0; 4];
                if self.ignore_case {
                    for lower in c.to_lowercase() {
                        normal.extend_from_slice(lower.encode_utf8(&mut buffer).as_bytes());
                    }
                } else {
                    normal.extend_from_slice(c.encode_utf8(&mut buffer).as_bytes());
                }
            }
        } else {
            // Raw-byte callers must not merge distinct invalid UTF-8 through lossy decoding.
            let start = line
                .iter()
                .position(|b| !b.is_ascii_whitespace())
                .unwrap_or(line.len());
            let end = line
                .iter()
                .rposition(|b| !b.is_ascii_whitespace())
                .map_or(0, |i| i + 1);
            for (i, &byte) in line.iter().enumerate() {
                if !byte.is_ascii_whitespace() || !self.skip_whitespace(i, start, end) {
                    normal.push(if self.ignore_case {
                        byte.to_ascii_lowercase()
                    } else {
                        byte
                    });
                }
            }
        }
        ContentHash(xxhash_rust::xxh64::xxh64(&normal, 0))
    }

    /// Hash every line of a list (bulk, cache-friendly).
    pub fn hash_lines(&self, lines: &crate::line::LineList) -> Vec<ContentHash> {
        lines.iter().map(|(text, _)| self.hash_line(text)).collect()
    }

    fn skip_whitespace(&self, index: usize, start: usize, end: usize) -> bool {
        (index < start && self.ignore_leading_whitespace)
            || (index >= end && self.ignore_trailing_whitespace)
            || (index >= start && index < end && self.ignore_embedded_whitespace)
    }
}

/// Streaming CRC32 (IEEE) used by folder compare `criteria CRC`.
#[derive(Debug)]
pub struct Crc32 {
    state: u32,
}

impl Default for Crc32 {
    fn default() -> Self {
        Self { state: u32::MAX }
    }
}

impl Crc32 {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.state =
                CRC_TABLE[((self.state ^ u32::from(byte)) & 0xff) as usize] ^ (self.state >> 8);
        }
    }

    pub fn finish(self) -> u32 {
        self.state ^ u32::MAX
    }
}

const CRC_TABLE: [u32; 256] = {
    let mut table = [0; 256];
    let mut i = 0;
    while i < 256 {
        let mut value = i as u32;
        let mut bit = 0;
        while bit < 8 {
            value = (value >> 1) ^ if value & 1 != 0 { 0xedb8_8320 } else { 0 };
            bit += 1;
        }
        table[i] = value;
        i += 1;
    }
    table
};
