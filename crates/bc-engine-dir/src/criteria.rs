//! Comparison criteria – the `criteria` script command and the Session Settings
//! "Comparison" frame (`TDirSessionComparisonFrame`). Spec §6 pipeline:
//! attrib / version / timestamp[:tolerance;IgnoreDST] / size|CRC|binary|rules-based /
//! timezone / follow-symlinks / ignore-unimportant.

use bc_vfs::FileAttrs;

/// How file contents are compared (mutually exclusive in `criteria`, also `compare` and `/qc=`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ContentCompare {
    /// Size only (no content read).
    #[default]
    Size,
    Crc,
    Binary,
    /// Compare using the file format's rules (text importance etc.).
    RulesBased,
}

/// Timezone handling – `timezone:(ignore|(left|right)(+|-)<0..12>)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimezoneRule {
    #[default]
    None,
    Ignore,
    /// Offset in hours applied to the given side (`true` = left).
    Offset { left: bool, hours: i8 },
}

/// Full criteria set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Criteria {
    /// DOS attributes that must match (`attrib:sh`), `None` = not compared.
    pub attrib: Option<FileAttrs>,
    /// Compare PE version info (`version`).
    pub version: bool,
    /// Compare timestamps; `None` disables.
    pub timestamp: Option<TimestampCriteria>,
    pub content: ContentCompare,
    pub timezone: TimezoneRule,
    pub follow_symlinks: bool,
    /// Treat unimportant differences as matches (`ignore-unimportant`).
    pub ignore_unimportant: bool,
}

/// Timestamp tolerance settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimestampCriteria {
    /// `<n>sec` tolerance.
    pub tolerance_secs: u32,
    /// `IgnoreDST`: differences of exactly one hour match.
    pub ignore_dst: bool,
}

impl Default for Criteria {
    fn default() -> Self {
        Self {
            attrib: None,
            version: false,
            timestamp: Some(TimestampCriteria { tolerance_secs: 2, ignore_dst: false }),
            content: ContentCompare::Size,
            timezone: TimezoneRule::None,
            follow_symlinks: false,
            ignore_unimportant: false,
        }
    }
}

impl Criteria {
    /// Parse the argument string of a `criteria` command.
    pub fn parse_script(args: &str) -> Result<Criteria, String> {
        todo!("bc-engine-dir::criteria: parse criteria arguments")
    }

    /// Whether this criteria set needs file contents (schedules content jobs).
    pub fn needs_content(&self) -> bool {
        !matches!(self.content, ContentCompare::Size)
    }
}
