//! # bc-engine-bin
//!
//! Non-text comparison engines scheduled for P2–P4. Only data models and entry points
//! are declared so that `bc-session` can already name every session kind.
//!
//! | module | BC classes |
//! |---|---|
//! | [`hex`] | `THexContent`, `THexMap`, `THexMapAlign`, `THexMapByteAlign` (`TBcByteAlign`), `THexRules`, `THexCompareSession`, `THexFiltered` |
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
        pub skew_tolerance: usize,
    }

    impl Default for HexRules {
        fn default() -> Self {
            Self { align_bytes: true, bytes_per_line: 16, skew_tolerance: 256 }
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

    /// Extract version-info strings from an executable (`TVfsGetExeVersionJob`).
    pub fn read_version_info(path: &std::path::Path) -> Result<Vec<TagValue>, std::io::Error> {
        todo!("bc-engine-bin::ver: VS_VERSIONINFO parsing")
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
