//! Strict, locale-independent numeric/date interpretation.
use crate::CellValue;
use bc_formats::model::{DateOrder, RegionalSettings};

pub const DAY_MS: i64 = 86_400_000;
pub fn parse_number(text: &str, regional: &RegionalSettings) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if regional.thousands == Some(regional.decimal) {
        return None;
    }
    let mut parts = text.split(['e', 'E']);
    let mantissa = parts.next()?;
    let exponent = parts.next();
    if parts.next().is_some() {
        return None;
    }
    if let Some(e) = exponent {
        e.parse::<i32>().ok()?;
    }
    let mantissa = mantissa.strip_prefix(['+', '-']).unwrap_or(mantissa);
    let mut decimal = mantissa.split(regional.decimal);
    let integer = decimal.next()?;
    let fraction = decimal.next();
    if decimal.next().is_some() || fraction.is_some_and(|f| !f.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let mut normalized = String::with_capacity(text.len());
    if text.starts_with('-') {
        normalized.push('-');
    }
    if let Some(separator) = regional.thousands {
        let groups: Vec<_> = integer.split(separator).collect();
        if groups.len() > 1
            && (groups[0].is_empty()
                || groups[0].len() > 3
                || groups[1..].iter().any(|g| g.len() != 3))
        {
            return None;
        }
        for group in groups {
            if !group.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            normalized.push_str(group);
        }
    } else {
        if !integer.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        normalized.push_str(integer);
    }
    if let Some(fraction) = fraction {
        normalized.push('.');
        normalized.push_str(fraction);
    }
    if let Some(e) = exponent {
        normalized.push('e');
        normalized.push_str(e);
    }
    normalized.parse::<f64>().ok().filter(|n| n.is_finite())
}
pub fn parse_date(text: &str, regional: &RegionalSettings) -> Option<i64> {
    let text = text.trim();
    let (date, time) = text
        .split_once(['T', ' '])
        .map_or((text, None), |(d, t)| (d, Some(t)));
    let parts: Vec<_> = date.split(regional.date_separator).collect();
    if parts.len() != 3 {
        return None;
    }
    let a = parts[0].parse::<i32>().ok()?;
    let b = parts[1].parse::<i32>().ok()?;
    let c = parts[2].parse::<i32>().ok()?;
    let (y, m, d) = match regional.date_order {
        DateOrder::Ymd => (a, b, c),
        DateOrder::Mdy => (c, a, b),
        DateOrder::Dmy => (c, b, a),
    };
    let date = date_ms(y, m, d)?;
    let mut offset = 0;
    if let Some(time) = time {
        let fields: Vec<_> = time.split(':').collect();
        if !(2..=3).contains(&fields.len()) {
            return None;
        }
        let h: i64 = fields[0].parse().ok()?;
        let m: i64 = fields[1].parse().ok()?;
        let s: f64 = fields.get(2).unwrap_or(&"0").parse().ok()?;
        if !(0..24).contains(&h) || !(0..60).contains(&m) || !(0.0..60.0).contains(&s) {
            return None;
        }
        offset = h * 3_600_000 + m * 60_000 + (s * 1000.0).round() as i64;
    }
    date.checked_add(offset)
}
pub fn date_ms(year: i32, month: i32, day: i32) -> Option<i64> {
    if !(1..=9999).contains(&year) || !(1..=12).contains(&month) {
        return None;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max = [
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
    ][(month - 1) as usize];
    if day < 1 || day > max {
        return None;
    }
    let y = year - i32::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(i64::from(era * 146097 + doe - 719468) * DAY_MS)
}
pub fn format_date(ms: i64) -> String {
    let z = ms.div_euclid(DAY_MS) + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    y += i64::from(m <= 2);
    let rem = ms.rem_euclid(DAY_MS);
    if rem == 0 {
        format!("{y:04}-{m:02}-{d:02}")
    } else {
        format!(
            "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}.{:03}",
            rem / 3_600_000,
            rem / 60_000 % 60,
            rem / 1000 % 60,
            rem % 1000
        )
    }
}
pub fn interpret(text: &str, regional: &RegionalSettings) -> CellValue {
    if text.is_empty() {
        CellValue::Empty
    } else if let Some(n) = parse_number(text, regional) {
        CellValue::Number(n)
    } else if let Some(d) = parse_date(text, regional) {
        CellValue::Date(d)
    } else if text.eq_ignore_ascii_case("true") {
        CellValue::Bool(true)
    } else if text.eq_ignore_ascii_case("false") {
        CellValue::Bool(false)
    } else {
        CellValue::Text(text.into())
    }
}
