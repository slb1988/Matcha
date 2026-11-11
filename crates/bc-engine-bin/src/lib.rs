//! # bc-engine-bin
//!
//! Non-text comparison engines scheduled for P2–P4. Only data models and entry points
//! are declared so that `bc-session` can already name every session kind.
//!
//! | module | BC classes |
//! |---|---|
//! | [`hex`] | `THexContent`, `THexMap`, `THexMapAlign`, `THexMapByteAlign`, `THexRules`, `THexCompareSession`, `THexFiltered` — 全部经 REA 符号表/RTTI 证实（macOS 5.0.4.30422 arm64，2026-11-11）；`TBcByteAlign` 为 BCALIGN 单元的独立字节级 differ，Complete 档 <5000 字节区间细化调用之 |
//! | [`pix`] | `TPixContent` (`TTagContent`), `TPixMapTree`, `TPixRules`, `TPixReplacementItem/List`, `TPixColors`, `TPixProcessorThread` |
//! | [`ver`] | `TVerContent`, `TVerMapTree`, `TVerRules`, `TVerCompareSession`, `TVfsGetExeVersionJob` |
//! | [`reg`] | `TRegSession`, `TRegCompareSession`, `TRegRules`, `TRegMapTree`, `TRegistry`, `TRegGroup(s)` |
//! | [`media`] | `TMediaContent`, `TMediaMapTree`, `TMediaRules`, `TMediaCompareSession` |
//! | [`tag`] | shared tag-tree base: `TTagContent`, `TTagMapTree` (`TSsMapTree`), `TTagRules`, `TTagSession` |
#![allow(unused_variables)] // scaffold: bodies are `todo!()`; remove once implemented
#![forbid(unsafe_code)]

/// Shared "tag tree" model used by Picture, Version and Media compare: a tree of named
/// values compared side by side (`TTagMapTree`).
pub mod tag {
    /// A named value on one side.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TagValue {
        pub name: String,
        pub value: String,
        pub unimportant: bool,
    }

    /// One row of a tag comparison.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TagRow {
        pub name: String,
        pub left: Option<String>,
        pub right: Option<String>,
        pub kind: bc_core::DiffKind,
        pub children: Vec<TagRow>,
    }

    /// Tag-tree rules – `TTagRules`.
    #[derive(Debug, Clone, PartialEq, Eq, Default)]
    pub struct TagRules {
        /// Tag names whose differences are unimportant.
        pub unimportant_tags: Vec<String>,
    }

    /// Pair two tag sets by name.
    pub fn align_tags(left: &[TagValue], right: &[TagValue], rules: &TagRules) -> Vec<TagRow> {
        todo!("bc-engine-bin::tag: pair by name")
    }
}

/// Hex Compare (P2).
pub mod hex {
    use bc_core::align::AlignMap;

    /// Byte buffer of one side – `THexContent`.
    #[derive(Debug, Default)]
    pub struct HexContent {
        pub bytes: Vec<u8>,
        pub read_only: bool,
    }

    /// Hex session rules – `THexRules`.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct HexRules {
        /// Align insertions/deletions (false = positional byte compare).
        pub align_bytes: bool,
        pub bytes_per_line: u16,
        /// Skew tolerance for `THexMapByteAlign`.
        ///
        /// REA 证据（`THEXMAPALIGN.RUN` 反汇编）：Complete 档 mismatch 区间两侧均 < 5000 字节时走
        /// `TBcByteAlign` 字节级 diff，否则走 `RUNRANGE` 锚点重同步（skew 硬编码 = 10）；
        /// Fast 档区间不细化，直接逐位对齐。设计 D6 将区间上限归入本规则集。
        pub skew_tolerance: usize,
    }

    impl Default for HexRules {
        fn default() -> Self {
            // skew=10 为 BC 硬编码实测值（RUNRANGE 第 5 参数）
            Self { align_bytes: true, bytes_per_line: 16, skew_tolerance: 10 }
        }
    }

    /// Aligned byte map – `THexMap`.
    #[derive(Debug, Default)]
    pub struct HexMap {
        pub left: HexContent,
        pub right: HexContent,
        pub rules: HexRules,
        pub align: AlignMap,
    }

    impl HexMap {
        /// Run `bc_core::align::ByteAlign` (or positional) and store the result.
        pub fn align(&mut self) {
            todo!("bc-engine-bin::hex: byte alignment")
        }
    }
}

/// Picture Compare (P2).
pub mod pix {
    /// Blend/display mode of the picture view.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum BlendMode {
        #[default]
        SideBySide,
        Blend,
        Difference,
        Tolerance,
        Mismatch,
    }

    /// Picture rules – `TPixRules`.
    #[derive(Debug, Clone, PartialEq, Default)]
    pub struct PixRules {
        /// Per-channel tolerance (0..=255) below which pixels match.
        pub tolerance: u8,
        /// Colour replacements treated as equal (`TPixReplacementList`): `(from, to)` RGBA.
        pub replacements: Vec<([u8; 4], [u8; 4])>,
        pub ignore_alpha: bool,
    }

    /// Decoded image (RGBA8).
    #[derive(Debug, Clone, Default)]
    pub struct Image {
        pub width: u32,
        pub height: u32,
        pub rgba: Vec<u8>,
    }

    /// Pixel comparison result.
    #[derive(Debug, Clone, Default)]
    pub struct PixDiff {
        pub mismatched_pixels: u64,
        pub total_pixels: u64,
        /// Per-pixel mismatch mask.
        pub mask: Vec<bool>,
    }

    /// Compare two images under `rules` (`TPixProcessorThread` work).
    pub fn compare(left: &Image, right: &Image, rules: &PixRules) -> PixDiff {
        todo!("bc-engine-bin::pix: tolerance compare")
    }
}

/// Version Compare (P3) – PE version resources.
pub mod ver {
    use super::tag::TagValue;

    use std::io::{self, Read, Seek, SeekFrom};

    fn invalid() -> io::Error { io::Error::new(io::ErrorKind::InvalidData, "invalid PE version resource") }
    fn u16_at(bytes: &[u8], at: usize) -> io::Result<u16> {
        Ok(u16::from_le_bytes(bytes.get(at..at + 2).ok_or_else(invalid)?.try_into().unwrap()))
    }
    fn u32_at(bytes: &[u8], at: usize) -> io::Result<u32> {
        Ok(u32::from_le_bytes(bytes.get(at..at + 4).ok_or_else(invalid)?.try_into().unwrap()))
    }
    fn read_at(file: &mut std::fs::File, offset: u64, len: usize) -> io::Result<Vec<u8>> {
        if offset.checked_add(len as u64).is_none_or(|end| end > file.metadata().map(|m| m.len()).unwrap_or(0)) { return Err(invalid()); }
        file.seek(SeekFrom::Start(offset))?;
        let mut bytes = vec![0; len];
        file.read_exact(&mut bytes)?;
        Ok(bytes)
    }
    fn align4(n: usize) -> usize { (n + 3) & !3 }

    /// Reads PE32/PE32+ resource tables without loading or executing the target image.
    /// Non-PE files and images with no RT_VERSION return an empty tag list.
    pub fn read_version_info(path: &std::path::Path) -> io::Result<Vec<TagValue>> {
        let mut file = std::fs::File::open(path)?;
        if file.metadata()?.len() < 64 { return Ok(Vec::new()); }
        let dos = read_at(&mut file, 0, 64)?;
        if &dos[..2] != b"MZ" { return Ok(Vec::new()); }
        let pe = u32_at(&dos, 60)? as u64;
        let header = read_at(&mut file, pe, 24)?;
        if &header[..4] != b"PE\0\0" { return Err(invalid()); }
        let sections = u16_at(&header, 6)? as usize;
        let optional_size = u16_at(&header, 20)? as usize;
        let optional = read_at(&mut file, pe + 24, optional_size)?;
        let directory = match u16_at(&optional, 0)? { 0x10b => 96, 0x20b => 112, _ => return Err(invalid()) };
        if optional.len() < directory + 24 { return Ok(Vec::new()); }
        let rva = u32_at(&optional, directory + 16)?;
        let size = u32_at(&optional, directory + 20)? as usize;
        if rva == 0 || size == 0 { return Ok(Vec::new()); }
        if size > 32 * 1024 * 1024 || sections > 1024 { return Err(invalid()); }
        let table = read_at(&mut file, pe + 24 + optional_size as u64, sections * 40)?;
        let map_rva = |address: u32, len: usize| -> io::Result<u64> {
            for section in table.as_chunks::<40>().0 {
                let base = u32_at(section, 12)?;
                let raw_size = u32_at(section, 16)?;
                if let Some(offset) = address.checked_sub(base) {
                    if u64::from(offset) + len as u64 <= u64::from(raw_size) {
                        return Ok(u64::from(u32_at(section, 20)?) + u64::from(offset));
                    }
                }
            }
            Err(invalid())
        };
        let resource_offset = map_rva(rva, size)?;
        let resources = read_at(&mut file, resource_offset, size)?;
        let Some(data_entry) = resource_leaf(&resources, 0, 0)? else { return Ok(Vec::new()); };
        let data_rva = u32_at(&resources, data_entry)?;
        let data_size = u32_at(&resources, data_entry + 4)? as usize;
        if data_size > 4 * 1024 * 1024 { return Err(invalid()); }
        let data = read_at(&mut file, map_rva(data_rva, data_size)?, data_size)?;
        let mut values = std::collections::BTreeMap::new();
        parse_block(&data, 0, data.len(), 0, &mut values)?;
        Ok(values.into_iter().map(|(name, value)| TagValue { name, value, unimportant: false }).collect())
    }

    fn resource_leaf(bytes: &[u8], offset: usize, depth: usize) -> io::Result<Option<usize>> {
        if depth > 3 { return Err(invalid()); }
        let count = usize::from(u16_at(bytes, offset + 12)?) + usize::from(u16_at(bytes, offset + 14)?);
        for index in 0..count {
            let at = offset + 16 + index * 8;
            let name = u32_at(bytes, at)?;
            let target = u32_at(bytes, at + 4)?;
            if depth == 0 && name != 16 { continue; } // RT_VERSION
            if target & 0x8000_0000 != 0 {
                if let Some(leaf) = resource_leaf(bytes, (target & 0x7fff_ffff) as usize, depth + 1)? { return Ok(Some(leaf)); }
            } else {
                if depth == 0 { return Err(invalid()); }
                return Ok(Some(target as usize));
            }
        }
        Ok(None)
    }

    fn parse_block(bytes: &[u8], start: usize, limit: usize, depth: usize, values: &mut std::collections::BTreeMap<String, String>) -> io::Result<usize> {
        if depth > 8 { return Err(invalid()); }
        let len = u16_at(bytes, start)? as usize;
        let value_len = u16_at(bytes, start + 2)? as usize;
        let text = u16_at(bytes, start + 4)? == 1;
        let end = start + len;
        if len < 6 || end > limit || end > bytes.len() { return Err(invalid()); }
        let mut at = start + 6;
        let mut key = Vec::new();
        loop {
            if at + 2 > end { return Err(invalid()); }
            let c = u16_at(bytes, at)?; at += 2;
            if c == 0 { break; }
            key.push(c);
        }
        let key = String::from_utf16(&key).map_err(|_| invalid())?;
        at = align4(at);
        let value_end = at + value_len * if text { 2 } else { 1 };
        if value_len > 0 && value_end > end { return Err(invalid()); }
        if key == "VS_VERSION_INFO" && value_len >= 52 {
            if u32_at(bytes, at)? != 0xfeef04bd { return Err(invalid()); }
            for (name, offset) in [("FileVersion", 8), ("ProductVersion", 16)] {
                let high = u32_at(bytes, at + offset)?;
                let low = u32_at(bytes, at + offset + 4)?;
                values.insert(name.into(), format!("{}.{}.{}.{}", high >> 16, high & 0xffff, low >> 16, low & 0xffff));
            }
        } else if text && value_len > 0 {
            let mut words = Vec::new();
            for i in (at..value_end).step_by(2) { words.push(u16_at(bytes, i)?); }
            while words.last() == Some(&0) { words.pop(); }
            values.insert(key, String::from_utf16(&words).map_err(|_| invalid())?);
        }
        at = align4(value_end);
        while at + 6 <= end {
            let next = parse_block(bytes, at, end, depth + 1, values)?;
            if next <= at { return Err(invalid()); }
            at = align4(next);
        }
        Ok(end)
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn malformed_tables_are_errors_not_panics() {
            for length in 0..64 {
                let bytes = vec![0; length];
                let _ = super::resource_leaf(&bytes, 0, 0);
                assert!(super::parse_block(&bytes, 0, length, 0, &mut Default::default()).is_err());
            }
        }
        #[cfg(windows)]
        #[test]
        fn system_dll_has_version() {
            let path = std::path::PathBuf::from(std::env::var_os("WINDIR").unwrap()).join("System32/kernel32.dll");
            let tags = super::read_version_info(&path).unwrap();
            let version = tags.iter().find(|t| t.name == "FileVersion").unwrap();
            assert!(!version.value.is_empty());
            println!("kernel32.dll FileVersion={}", version.value);
        }
    }
}

/// Registry Compare (P3) – `.reg` files and live hives.
pub mod reg {
    use super::tag::TagValue;

    /// Registry source.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum RegSource {
        /// Exported `.reg` file.
        File(std::path::PathBuf),
        /// Live key path such as `HKEY_CURRENT_USER\Software`.
        Live(String),
    }

    /// Load a registry tree as tag values (keys → nested names).
    pub fn load(source: &RegSource) -> Result<Vec<TagValue>, std::io::Error> {
        todo!("bc-engine-bin::reg: .reg parser / live hive reader")
    }
}

/// Media Compare (P3) – MP3/FLAC/MP4 tags.
pub mod media {
    use super::tag::TagValue;

    /// Read metadata tags from a media file.
    pub fn read_tags(path: &std::path::Path) -> Result<Vec<TagValue>, std::io::Error> {
        todo!("bc-engine-bin::media: ID3/Vorbis/MP4 atom tag reading")
    }
}
