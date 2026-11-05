//! Script filter updates and XML preset model; no GUI or locale globals.
use crate::filter::{Cutoff, FilterClock, NameFilter, OtherFilters, SizeLimit};
use bc_core::persist::{self, Node};
use bc_vfs::FileAttrs;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilterSettings {
    pub names: NameFilter,
    pub other: OtherFilters,
}

fn words(input: &str) -> Result<Vec<String>, String> {
    let mut quoted = false;
    let mut word = String::new();
    let mut result = Vec::new();
    let mut started = false;
    for c in input.chars() {
        if c == '"' {
            quoted = !quoted;
            started = true;
        } else if c.is_whitespace() && !quoted {
            if started {
                result.push(std::mem::take(&mut word));
                started = false;
            }
        } else {
            word.push(c);
            started = true;
        }
    }
    if quoted {
        return Err("unterminated quoted filter argument".into());
    }
    if started {
        result.push(word);
    }
    Ok(result)
}
pub fn parse_size(input: &str) -> Result<u64, String> {
    let upper = input.to_ascii_uppercase();
    let (number, multiplier) = [
        ("TB", 1u64 << 40),
        ("GB", 1 << 30),
        ("MB", 1 << 20),
        ("KB", 1 << 10),
        ("B", 1),
    ]
    .into_iter()
    .find_map(|(suffix, multiplier)| upper.strip_suffix(suffix).map(|n| (n, multiplier)))
    .unwrap_or((&upper, 1));
    let (whole, fraction) = number.split_once('.').unwrap_or((number, ""));
    let whole: u64 = whole.parse().map_err(|_| "invalid size")?;
    if fraction.len() > 9 || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return Err("invalid size fraction".into());
    }
    let fraction_value: u64 = if fraction.is_empty() {
        0
    } else {
        fraction.parse().map_err(|_| "invalid size fraction")?
    };
    whole
        .checked_mul(multiplier)
        .and_then(|v| {
            fraction_value
                .checked_mul(multiplier)
                .and_then(|f| v.checked_add(f / 10u64.pow(fraction.len() as u32)))
        })
        .ok_or_else(|| "size overflow".into())
}
fn direction(input: &str) -> (bool, &str) {
    if let Some(rest) = input.strip_prefix('>') {
        (false, rest)
    } else {
        (true, input.strip_prefix('<').unwrap_or(input))
    }
}
impl FilterSettings {
    /// Update only fields named by this command. Invalid input leaves self unchanged.
    pub fn apply_script(&mut self, input: &str, clock: FilterClock) -> Result<(), String> {
        let mut next = self.clone();
        let tokens = words(input)?;
        let mut masks = Vec::new();
        for token in tokens.iter().skip(usize::from(
            tokens
                .first()
                .is_some_and(|s| s.eq_ignore_ascii_case("filter")),
        )) {
            let lower = token.to_ascii_lowercase();
            if lower == "exclude-protected" {
                next.other.exclude_protected = true;
            } else if lower == "include-protected" {
                next.other.exclude_protected = false;
            } else if let Some(value) = lower.strip_prefix("attrib:") {
                if value == "none" {
                    next.other.attrib_include = None;
                    next.other.attrib_exclude = None;
                } else {
                    let sign = value.chars().next().ok_or("missing attribute sign")?;
                    let attrs =
                        FileAttrs::parse_set(&value[1..]).ok_or("invalid DOS attributes")?;
                    match sign {
                        '+' => next.other.attrib_include = Some(attrs),
                        '-' => next.other.attrib_exclude = Some(attrs),
                        _ => return Err("expected attribute + or -".into()),
                    }
                }
            } else if let Some(value) = lower.strip_prefix("size:") {
                next.other.size = if value == "none" {
                    None
                } else {
                    let (smaller, value) = direction(value);
                    let size = parse_size(value)?;
                    Some(if smaller {
                        SizeLimit::SmallerThan(size)
                    } else {
                        SizeLimit::LargerThan(size)
                    })
                };
            } else if lower.starts_with("cutoff:") {
                let value = &token[7..];
                next.other.cutoff = if value.eq_ignore_ascii_case("none") {
                    None
                } else {
                    let (older, value) = direction(value);
                    let lower = value.to_ascii_lowercase();
                    let days = lower.strip_suffix("days").unwrap_or(&lower);
                    Some(
                        if days.bytes().all(|b| b.is_ascii_digit()) && !days.is_empty() {
                            let days = days.parse().map_err(|_| "invalid relative days")?;
                            if older {
                                Cutoff::OlderThanDays(days)
                            } else {
                                Cutoff::NewerThanDays(days)
                            }
                        } else {
                            let instant = parse_date(value, clock.utc_offset_seconds)?;
                            if older {
                                Cutoff::OlderThan(instant)
                            } else {
                                Cutoff::NewerThan(instant)
                            }
                        },
                    )
                };
            } else {
                masks.push(token.as_str());
            }
        }
        if !masks.is_empty() {
            next.names = NameFilter::parse(&masks.join(" "))?;
        }
        *self = next;
        Ok(())
    }
}

/// ISO YYYY-MM-DD, US M/D/YYYY, and English 'Dec 31, 2012', optionally HH:MM[:SS][am/pm].
/// Local wall time is converted using the explicit caller-supplied UTC offset.
pub fn parse_date(input: &str, utc_offset_seconds: i32) -> Result<SystemTime, String> {
    let normalized = input.replace(',', "").replace('T', " ");
    let parts: Vec<_> = normalized.split_whitespace().collect();
    let first = *parts.first().ok_or("empty date")?;
    let parse = |s: &str| {
        s.parse::<i64>()
            .map_err(|_| "invalid date number".to_owned())
    };
    let (year, month, day, clock) = if first.contains('-') || first.contains('/') {
        let date: Vec<_> = first.split(['-', '/']).collect();
        if date.len() != 3 {
            return Err("invalid date".into());
        }
        let (year, month, day) = if date[0].len() == 4 {
            (parse(date[0])?, parse(date[1])?, parse(date[2])?)
        } else {
            (parse(date[2])?, parse(date[0])?, parse(date[1])?)
        };
        if parts.len() > 2 {
            return Err("unexpected date fields".into());
        }
        (year, month, day, parts.get(1).copied())
    } else {
        if !(3..=4).contains(&parts.len()) {
            return Err("expected month day year".into());
        }
        let months = [
            "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
        ];
        let month = months
            .iter()
            .position(|m| first.to_ascii_lowercase().starts_with(m))
            .ok_or("invalid month")? as i64
            + 1;
        (
            parse(parts[2])?,
            month,
            parse(parts[1])?,
            parts.get(3).copied(),
        )
    };
    if !(1..=9999).contains(&year) || !(1..=12).contains(&month) {
        return Err("invalid calendar date".into());
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ][month as usize - 1];
    if !(1..=max_day).contains(&day) {
        return Err("invalid calendar day".into());
    }
    let mut seconds = 0;
    if let Some(clock) = clock {
        let lower = clock.to_ascii_lowercase();
        let am = lower.ends_with("am");
        let pm = lower.ends_with("pm");
        let clock = lower.trim_end_matches("am").trim_end_matches("pm");
        let fields: Vec<_> = clock.split(':').collect();
        if !(2..=3).contains(&fields.len()) {
            return Err("invalid clock".into());
        }
        let mut hour = parse(fields[0])?;
        let minute = parse(fields[1])?;
        let second = if fields.len() == 3 {
            parse(fields[2])?
        } else {
            0
        };
        if (am || pm) && !(1..=12).contains(&hour) {
            return Err("invalid 12-hour time".into());
        }
        if am || pm {
            hour %= 12;
            if pm {
                hour += 12;
            }
        }
        if !(0..24).contains(&hour) || !(0..60).contains(&minute) || !(0..60).contains(&second) {
            return Err("invalid time".into());
        }
        seconds = hour * 3600 + minute * 60 + second;
    }
    // Gregorian civil date -> days since Unix epoch.
    let adjusted_year = year - i64::from(month <= 2);
    let era = adjusted_year.div_euclid(400);
    let yoe = adjusted_year - era * 400;
    let shifted_month = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * shifted_month + 2) / 5 + day - 1;
    let days = era * 146097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719468;
    let seconds = days * 86400 + seconds - i64::from(utc_offset_seconds);
    if seconds >= 0 {
        UNIX_EPOCH.checked_add(Duration::from_secs(seconds as u64))
    } else {
        UNIX_EPOCH.checked_sub(Duration::from_secs(seconds.unsigned_abs()))
    }
    .ok_or_else(|| "date out of range".into())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterPreset {
    pub name: String,
    pub masks: String,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilterPresets {
    pub items: Vec<FilterPreset>,
}
impl FilterPresets {
    pub fn upsert(&mut self, name: &str, masks: &str) -> Result<(), String> {
        if name.trim().is_empty() {
            return Err("empty preset name".into());
        }
        NameFilter::parse(masks)?;
        let preset = FilterPreset {
            name: name.into(),
            masks: masks.into(),
        };
        if let Some(old) = self.items.iter_mut().find(|p| p.name == name) {
            *old = preset;
        } else {
            self.items.push(preset);
        }
        Ok(())
    }
    pub fn to_xml(&self) -> String {
        let mut root = Node::new("FilterPresets");
        for (index, preset) in self.items.iter().enumerate() {
            let mut item = Node::new(format!("_{index}"));
            for (key, value) in [("Name", &preset.name), ("Masks", &preset.masks)] {
                let mut node = Node::new(key);
                node.value = Some(value.clone());
                item.children.push(node);
            }
            root.children.push(item);
        }
        persist::write_xml(&root, None)
    }
    pub fn from_xml(xml: &str) -> Result<Self, String> {
        let root = persist::parse_xml(xml).map_err(|e| e.to_string())?;
        if root.name != "FilterPresets" {
            return Err("expected FilterPresets root".into());
        }
        let mut result = Self::default();
        for item in root.items() {
            let name = item.child_value("Name").ok_or("preset name missing")?;
            let masks = item.child_value("Masks").ok_or("preset masks missing")?;
            if result.items.iter().any(|p| p.name == name) {
                return Err("duplicate preset name".into());
            }
            result.upsert(name, masks)?;
        }
        Ok(result)
    }
}
