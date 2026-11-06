//! Directory quick tests and the script criteria contract.
use crate::CompareStatus;
use bc_vfs::{FileAttrs, VfsEntry};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ContentCompare {
    #[default]
    Size,
    Crc,
    Binary,
    RulesBased,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimezoneRule {
    #[default]
    None,
    Ignore,
    Offset {
        left: bool,
        hours: i8,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Criteria {
    pub attrib: Option<FileAttrs>,
    pub version: bool,
    pub timestamp: Option<TimestampCriteria>,
    pub content: ContentCompare,
    pub timezone: TimezoneRule,
    pub follow_symlinks: bool,
    pub ignore_unimportant: bool,
    pub compare_size: bool,
    pub compare_filename_case: bool,
    pub skip_content_if_quick_same: bool,
    pub content_overrides_quick: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimestampCriteria {
    pub tolerance_secs: u32,
    pub ignore_dst: bool,
}
impl Default for TimestampCriteria {
    fn default() -> Self {
        Self {
            tolerance_secs: 2,
            ignore_dst: false,
        }
    }
}
impl Default for Criteria {
    fn default() -> Self {
        Self {
            attrib: None,
            version: false,
            timestamp: Some(TimestampCriteria::default()),
            content: ContentCompare::Size,
            timezone: TimezoneRule::None,
            follow_symlinks: false,
            ignore_unimportant: false,
            compare_size: true,
            compare_filename_case: false,
            skip_content_if_quick_same: false,
            content_overrides_quick: true,
        }
    }
}
fn nanos(time: SystemTime) -> i128 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_nanos() as i128,
        Err(e) => -(e.duration().as_nanos() as i128),
    }
}
pub fn attribute_bits(a: FileAttrs) -> u8 {
    u8::from(a.archive)
        | u8::from(a.system) << 1
        | u8::from(a.hidden) << 2
        | u8::from(a.readonly) << 3
}
pub fn attribute_string(a: FileAttrs) -> String {
    [
        (a.archive, 'a'),
        (a.system, 's'),
        (a.hidden, 'h'),
        (a.readonly, 'r'),
    ]
    .into_iter()
    .filter_map(|(yes, c)| yes.then_some(c))
    .collect()
}

impl Criteria {
    pub fn needs_content(&self) -> bool {
        self.content != ContentCompare::Size
    }

    pub fn quick(&self, left: Option<&VfsEntry>, right: Option<&VfsEntry>) -> CompareStatus {
        use CompareStatus::*;
        let (left, right) = match (left, right) {
            (Some(l), Some(r)) => (l, r),
            (Some(_), None) => return LeftOrphan,
            (None, Some(_)) => return RightOrphan,
            _ => return Unknown,
        };
        if left.error.is_some() || right.error.is_some() {
            return Error;
        }
        if left.kind != right.kind {
            return Different;
        }
        if left.kind == bc_vfs::EntryKind::Dir {
            return Same;
        }
        let mut different = self.compare_size && left.size != right.size;
        different |= self.compare_filename_case
            && left.name != right.name
            && left.name.to_lowercase() == right.name.to_lowercase();
        if let Some(mask) = self.attrib {
            different |= (attribute_bits(left.attrs) ^ attribute_bits(right.attrs))
                & attribute_bits(mask)
                != 0;
        }
        if let Some(time) = self.timestamp {
            if let (Some(l), Some(r)) = (left.modified, right.modified) {
                let mut delta = nanos(l) - nanos(r);
                if let TimezoneRule::Offset { left, hours } = self.timezone {
                    delta += i128::from(hours) * 3600 * 1_000_000_000 * if left { 1 } else { -1 };
                }
                let difference = delta.unsigned_abs();
                let hour = 3600 * 1_000_000_000u128;
                let equal = difference <= u128::from(time.tolerance_secs) * 1_000_000_000
                    || (time.ignore_dst && difference == hour)
                    || (self.timezone == TimezoneRule::Ignore && difference.is_multiple_of(hour));
                if !equal {
                    return if delta > 0 { LeftNewer } else { RightNewer };
                }
            } else if left.modified != right.modified {
                return Unknown;
            }
        }
        if different {
            Different
        } else {
            Same
        }
    }

    pub fn combine(
        &self,
        quick: CompareStatus,
        content: Option<CompareStatus>,
        versions_equal: Option<bool>,
    ) -> CompareStatus {
        use CompareStatus::*;
        if matches!(quick, LeftOrphan | RightOrphan | Error | Unknown) {
            return quick;
        }
        if versions_equal == Some(false) {
            return if matches!(quick, LeftNewer | RightNewer) {
                quick
            } else {
                Different
            };
        }
        let result = match content {
            Some(Error) => Error,
            Some(Different) if matches!(quick, LeftNewer | RightNewer) => quick,
            Some(value) if self.content_overrides_quick || quick == Same => value,
            _ => quick,
        };
        if self.ignore_unimportant && result == SimilarUnimportant {
            Same
        } else {
            result
        }
    }

    /// Replaces the script-exposed criteria; omitted timestamp/attributes are disabled.
    /// Extra named switches round-trip GUI-only toggles without losing information.
    pub fn parse_script(args: &str) -> Result<Self, String> {
        let mut result = Self {
            timestamp: None,
            ..Self::default()
        };
        let mut content_seen = false;
        for original in args.split_whitespace().filter(|s| *s != "criteria") {
            let token = original.to_ascii_lowercase();
            match token.as_str() {
                "version" => result.version = true,
                "follow-symlinks" => result.follow_symlinks = true,
                "ignore-unimportant" => result.ignore_unimportant = true,
                "filename-case" => result.compare_filename_case = true,
                "no-size" => result.compare_size = false,
                "skip-quick-same" => result.skip_content_if_quick_same = true,
                "no-content-override" => result.content_overrides_quick = false,
                "size" | "crc" | "binary" | "rules-based" => {
                    if content_seen {
                        return Err("content criteria are mutually exclusive".into());
                    }
                    content_seen = true;
                    result.content = match token.as_str() {
                        "crc" => ContentCompare::Crc,
                        "binary" => ContentCompare::Binary,
                        "rules-based" => ContentCompare::RulesBased,
                        _ => ContentCompare::Size,
                    };
                }
                "timestamp" => result.timestamp = Some(TimestampCriteria::default()),
                _ if token.starts_with("timestamp:") => {
                    let mut time = TimestampCriteria::default();
                    for part in token[10..].split(';').filter(|s| !s.is_empty()) {
                        if part == "ignoredst" {
                            time.ignore_dst = true;
                        } else {
                            time.tolerance_secs = part
                                .strip_suffix("sec")
                                .ok_or("expected seconds suffix")?
                                .parse()
                                .map_err(|_| "invalid timestamp tolerance")?;
                        }
                    }
                    result.timestamp = Some(time);
                }
                _ if token.starts_with("attrib:") => {
                    result.attrib =
                        Some(FileAttrs::parse_set(&token[7..]).ok_or("invalid DOS attributes")?)
                }
                _ if token.starts_with("timezone:") => {
                    let value = &token[9..];
                    result.timezone = if value == "ignore" {
                        TimezoneRule::Ignore
                    } else {
                        let (left, hours) = if let Some(n) = value.strip_prefix("left") {
                            (true, n)
                        } else if let Some(n) = value.strip_prefix("right") {
                            (false, n)
                        } else {
                            return Err("invalid timezone side".into());
                        };
                        if !hours.starts_with(['+', '-']) {
                            return Err("expected timezone sign".into());
                        }
                        let hours: i8 = hours.parse().map_err(|_| "invalid timezone offset")?;
                        if !(-12..=12).contains(&hours) {
                            return Err("timezone offset exceeds 12 hours".into());
                        }
                        TimezoneRule::Offset { left, hours }
                    };
                }
                _ => return Err(format!("unknown criterion: {original}")),
            }
        }
        Ok(result)
    }
    pub fn to_script(&self) -> String {
        let mut fields = Vec::new();
        if let Some(a) = self.attrib {
            fields.push(format!("attrib:{}", attribute_string(a)));
        }
        if self.version {
            fields.push("version".into());
        }
        if let Some(t) = self.timestamp {
            fields.push(format!(
                "timestamp:{}sec{}",
                t.tolerance_secs,
                if t.ignore_dst { ";IgnoreDST" } else { "" }
            ));
        }
        fields.push(
            match self.content {
                ContentCompare::Size => "size",
                ContentCompare::Crc => "CRC",
                ContentCompare::Binary => "binary",
                ContentCompare::RulesBased => "rules-based",
            }
            .into(),
        );
        match self.timezone {
            TimezoneRule::None => {}
            TimezoneRule::Ignore => fields.push("timezone:ignore".into()),
            TimezoneRule::Offset { left, hours } => fields.push(format!(
                "timezone:{}{hours:+}",
                if left { "left" } else { "right" }
            )),
        }
        for (enabled, name) in [
            (self.follow_symlinks, "follow-symlinks"),
            (self.ignore_unimportant, "ignore-unimportant"),
            (self.compare_filename_case, "filename-case"),
            (!self.compare_size, "no-size"),
            (self.skip_content_if_quick_same, "skip-quick-same"),
            (!self.content_overrides_quick, "no-content-override"),
        ] {
            if enabled {
                fields.push(name.into());
            }
        }
        fields.join(" ")
    }
}
