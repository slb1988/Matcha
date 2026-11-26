//! 脚本需要的少量系统信息：本地时间与区域时间格式（`%date%`/`%time%`/`%fn_time%`、patch 文件头
//! 的修改时间）、系统 ANSI 代码页（无 BOM 且非 UTF-8 的脚本）与提示音（`beep`）。
//!
//! Windows 经 `windows-sys` 取得；其他平台用 UTC、`HH:MM:SS` 与 1252 兜底（macOS 支持落地时
//! 再接本地时区）。这是本 crate 唯一允许 `unsafe` 的模块。

use std::time::{SystemTime, UNIX_EPOCH};

/// 已换算到本地时区的时刻。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTime {
    pub year: u16,
    pub month: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
    /// 本地时间相对 UTC 的偏移（分钟，东正西负）。
    pub offset_minutes: i32,
}

impl LocalTime {
    /// `%date%`：`yyyy-mm-dd`。
    pub fn date(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    /// `%fn_time%`：`hh-mm-ss`。
    pub fn file_name_time(&self) -> String {
        format!("{:02}-{:02}-{:02}", self.hour, self.minute, self.second)
    }

    /// patch 文件头的修改时间：`YYYY-MM-DD HH:MM:SS.000000000 +hhmm`（参考工具恒写 9 个 0）。
    pub fn patch_header(&self) -> String {
        let sign = if self.offset_minutes < 0 { '-' } else { '+' };
        let offset = self.offset_minutes.unsigned_abs();
        format!(
            "{} {:02}:{:02}:{:02}.000000000 {sign}{:02}{:02}",
            self.date(),
            self.hour,
            self.minute,
            self.second,
            offset / 60,
            offset % 60
        )
    }
}

/// 纪元秒 → (年, 月, 日, 时, 分, 秒)，公历（Howard Hinnant 的 civil_from_days）。
pub fn civil(seconds: i64) -> (i64, u16, u16, u16, u16, u16) {
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u16;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u16;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (
        year,
        month,
        day,
        (rest / 3_600) as u16,
        (rest % 3_600 / 60) as u16,
        (rest % 60) as u16,
    )
}

/// 秒级 UTC 纪元时间加偏移后的本地时刻（偏移已知时的纯计算，供测试与非 Windows 使用）。
pub fn local_from_utc(utc_seconds: i64, offset_minutes: i32) -> LocalTime {
    let (year, month, day, hour, minute, second) =
        civil(utc_seconds + i64::from(offset_minutes) * 60);
    LocalTime {
        year: year.clamp(0, i64::from(u16::MAX)) as u16,
        month,
        day,
        hour,
        minute,
        second,
        offset_minutes,
    }
}

fn utc_seconds(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => after.as_secs() as i64,
        Err(before) => -(before.duration().as_secs() as i64) - 1,
    }
}

/// 某一时刻的本地时间（含该时刻适用的夏令时偏移）。
pub fn local(time: SystemTime) -> LocalTime {
    let seconds = utc_seconds(time);
    local_from_utc(seconds, imp::offset_minutes(seconds))
}

/// `%time%`：系统区域设置的默认时间格式（非 Windows：`HH:MM:SS`）。
pub fn regional_time(now: SystemTime) -> String {
    imp::regional_time(&local(now))
}

/// 用户区域设置中的数字与短日期格式（含用户在控制面板里的自定义）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleFormats {
    pub decimal: String,
    pub thousands: String,
    /// 短日期模式，如 `yyyy/M/d`。
    pub short_date: String,
    /// 日期分隔符：取自短日期模式（与 .NET `DateTimeFormatInfo.DateSeparator` 一致），
    /// 模式里没有时用区域的 `LOCALE_SDATE`。
    pub date_separator: String,
}

/// 用户区域格式（非 Windows：`None`，调用方用自己的默认值）。
pub fn locale_formats() -> Option<LocaleFormats> {
    imp::locale_formats()
}

/// 短日期模式中第一个日期字段之后的分隔符（跳过引号内文字）。
pub fn pattern_date_separator(pattern: &str) -> Option<String> {
    let mut chars = pattern.chars().peekable();
    let mut seen_field = false;
    let mut separator = String::new();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                for q in chars.by_ref() {
                    if q == '\'' {
                        break;
                    }
                }
            }
            'y' | 'M' | 'd' | 'g' => {
                if !separator.is_empty() {
                    return Some(separator);
                }
                seen_field = true;
            }
            c if seen_field && !c.is_alphanumeric() => separator.push(c),
            _ => {}
        }
    }
    None
}

/// 系统 ANSI 代码页（非 Windows：1252）。
pub fn ansi_code_page() -> u16 {
    imp::ansi_code_page()
}

/// 系统提示音（非 Windows：向标准错误写 BEL）。
pub fn beep() {
    imp::beep()
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod imp {
    use super::LocalTime;
    use windows_sys::Win32::Foundation::{FILETIME, SYSTEMTIME};
    use windows_sys::Win32::Globalization::{
        GetACP, GetLocaleInfoEx, GetTimeFormatEx, LOCALE_SDATE, LOCALE_SDECIMAL, LOCALE_SSHORTDATE,
        LOCALE_STHOUSAND,
    };
    use windows_sys::Win32::System::Diagnostics::Debug::MessageBeep;
    use windows_sys::Win32::System::Time::{FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime};

    /// 1601-01-01 到 1970-01-01 的秒数（FILETIME 纪元差）。
    const EPOCH_DIFFERENCE: i64 = 11_644_473_600;

    fn system_time(seconds: i64) -> Option<SYSTEMTIME> {
        let ticks = u64::try_from(seconds + EPOCH_DIFFERENCE).ok()? * 10_000_000;
        let file_time = FILETIME {
            dwLowDateTime: ticks as u32,
            dwHighDateTime: (ticks >> 32) as u32,
        };
        let mut utc = unsafe { std::mem::zeroed::<SYSTEMTIME>() };
        // SAFETY: 两个指针都指向本函数内有效的结构体。
        (unsafe { FileTimeToSystemTime(&file_time, &mut utc) } != 0).then_some(utc)
    }

    fn seconds_of(time: &SYSTEMTIME) -> i64 {
        let days = days_from_civil(
            i64::from(time.wYear),
            i64::from(time.wMonth),
            i64::from(time.wDay),
        );
        days * 86_400
            + i64::from(time.wHour) * 3_600
            + i64::from(time.wMinute) * 60
            + i64::from(time.wSecond)
    }

    fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
        let year = if month <= 2 { year - 1 } else { year };
        let era = year.div_euclid(400);
        let yoe = year - era * 400;
        let mp = (month + 9) % 12;
        let doy = (153 * mp + 2) / 5 + day - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    pub fn offset_minutes(utc_seconds: i64) -> i32 {
        let Some(utc) = system_time(utc_seconds) else {
            return 0;
        };
        let mut local = unsafe { std::mem::zeroed::<SYSTEMTIME>() };
        // SAFETY: 时区参数为 NULL 表示当前时区；输入输出指针有效。
        if unsafe { SystemTimeToTzSpecificLocalTime(std::ptr::null(), &utc, &mut local) } == 0 {
            return 0;
        }
        ((seconds_of(&local) - seconds_of(&utc)) / 60) as i32
    }

    pub fn regional_time(time: &LocalTime) -> String {
        let system = SYSTEMTIME {
            wYear: time.year,
            wMonth: time.month,
            wDayOfWeek: 0,
            wDay: time.day,
            wHour: time.hour,
            wMinute: time.minute,
            wSecond: time.second,
            wMilliseconds: 0,
        };
        let mut buffer = [0u16; 128];
        // SAFETY: NULL 区域名 = 用户默认区域；NULL 格式 = 区域默认格式；缓冲区长度如实传入。
        let written = unsafe {
            GetTimeFormatEx(
                std::ptr::null(),
                0,
                &system,
                std::ptr::null(),
                buffer.as_mut_ptr(),
                buffer.len() as i32,
            )
        };
        if written <= 1 {
            return format!("{:02}:{:02}:{:02}", time.hour, time.minute, time.second);
        }
        String::from_utf16_lossy(&buffer[..written as usize - 1])
    }

    fn locale_info(kind: u32) -> Option<String> {
        let mut buffer = [0u16; 128];
        // SAFETY: NULL 区域名 = 用户默认区域；缓冲区长度（字符数）如实传入。
        let written = unsafe {
            GetLocaleInfoEx(
                std::ptr::null(),
                kind,
                buffer.as_mut_ptr(),
                buffer.len() as i32,
            )
        };
        (written > 0).then(|| String::from_utf16_lossy(&buffer[..written as usize - 1]))
    }

    pub fn locale_formats() -> Option<super::LocaleFormats> {
        let short_date = locale_info(LOCALE_SSHORTDATE)?;
        let date_separator = super::pattern_date_separator(&short_date)
            .or_else(|| locale_info(LOCALE_SDATE))
            .unwrap_or_else(|| "/".into());
        Some(super::LocaleFormats {
            decimal: locale_info(LOCALE_SDECIMAL)?,
            thousands: locale_info(LOCALE_STHOUSAND).unwrap_or_default(),
            short_date,
            date_separator,
        })
    }

    pub fn ansi_code_page() -> u16 {
        // SAFETY: 无参数。
        u16::try_from(unsafe { GetACP() }).unwrap_or(1252)
    }

    pub fn beep() {
        // SAFETY: 无指针参数；0 = MB_OK 的默认提示音。
        unsafe { MessageBeep(0) };
    }
}

#[cfg(not(windows))]
mod imp {
    use super::LocalTime;

    pub fn offset_minutes(_: i64) -> i32 {
        0
    }

    pub fn regional_time(time: &LocalTime) -> String {
        format!("{:02}:{:02}:{:02}", time.hour, time.minute, time.second)
    }

    pub fn locale_formats() -> Option<super::LocaleFormats> {
        None
    }

    pub fn ansi_code_page() -> u16 {
        1252
    }

    pub fn beep() {
        use std::io::Write;
        let _ = std::io::stderr().write_all(b"\x07");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_separator_follows_the_short_date_pattern() {
        assert_eq!(pattern_date_separator("yyyy/M/d").as_deref(), Some("/"));
        assert_eq!(pattern_date_separator("dd.MM.yyyy").as_deref(), Some("."));
        assert_eq!(pattern_date_separator("M-d-yy").as_deref(), Some("-"));
        assert_eq!(pattern_date_separator("yyyy'年'M'月'd'日'"), None);
    }

    #[cfg(windows)]
    #[test]
    fn user_locale_formats_are_available() {
        let formats = locale_formats().expect("locale formats");
        assert!(!formats.decimal.is_empty() && !formats.short_date.is_empty());
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil(0), (1970, 1, 1, 0, 0, 0));
        // 2026-11-11 07:41:49 UTC
        assert_eq!(civil(1_794_382_909), (2026, 11, 11, 7, 41, 49));
        assert_eq!(civil(951_782_400), (2000, 2, 29, 0, 0, 0));
        assert_eq!(civil(-1), (1969, 12, 31, 23, 59, 59));
    }

    #[test]
    fn formats_follow_the_reference() {
        let time = local_from_utc(1_794_382_909, 8 * 60);
        assert_eq!(time.date(), "2026-11-11");
        assert_eq!(time.file_name_time(), "15-41-49");
        assert_eq!(time.patch_header(), "2026-11-11 15:41:49.000000000 +0800");
        assert_eq!(
            local_from_utc(0, -(5 * 60 + 30)).patch_header(),
            "1969-12-31 18:30:00.000000000 -0530"
        );
    }

    #[test]
    fn local_time_of_now_is_consistent() {
        let now = local(SystemTime::now());
        assert!((1..=12).contains(&now.month) && (1..=31).contains(&now.day));
        assert!(now.offset_minutes.abs() <= 14 * 60);
        assert!(!regional_time(SystemTime::now()).is_empty());
        assert!(ansi_code_page() > 0);
    }
}
