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
    // Allocation-free structural validation. Every rejection is one of the
    // original checks; all failure paths return None, so their relative
    // order cannot change the outcome, and only survivors allocate below.
    if let Some(separator) = regional.thousands {
        let mut groups = integer.split(separator).peekable();
        let first = groups.next().expect("split yields at least one part");
        if groups.peek().is_some() {
            // Multi-group: first 1..=3 digits, every later group exactly 3.
            if first.is_empty()
                || first.len() > 3
                || !first.bytes().all(|b| b.is_ascii_digit())
            {
                return None;
            }
            for group in groups {
                if group.len() != 3 || !group.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
            }
        } else if !first.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
    } else if !integer.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut normalized = String::with_capacity(text.len());
    if text.starts_with('-') {
        normalized.push('-');
    }
    if let Some(separator) = regional.thousands {
        for group in integer.split(separator) {
            normalized.push_str(group);
        }
    } else {
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
    // Iterator form of the original split().collect() + field-count checks:
    // exactly three date fields, no Vec allocation on rejection.
    let mut parts = date.split(regional.date_separator);
    let (Some(a), Some(b), Some(c)) = (parts.next(), parts.next(), parts.next()) else {
        return None;
    };
    if parts.next().is_some() {
        return None;
    }
    let a = a.parse::<i32>().ok()?;
    let b = b.parse::<i32>().ok()?;
    let c = c.parse::<i32>().ok()?;
    let (y, m, d) = match regional.date_order {
        DateOrder::Ymd => (a, b, c),
        DateOrder::Mdy => (c, a, b),
        DateOrder::Dmy => (c, b, a),
    };
    let date = date_ms(y, m, d)?;
    let mut offset = 0;
    if let Some(time) = time {
        // Same iterator treatment: two or three fields, no allocation.
        let mut fields = time.split(':');
        let (Some(h), Some(m)) = (fields.next(), fields.next()) else {
            return None;
        };
        let s = fields.next().unwrap_or("0");
        if fields.next().is_some() {
            return None;
        }
        let h: i64 = h.parse().ok()?;
        let m: i64 = m.parse().ok()?;
        let s: f64 = s.parse().ok()?;
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
        return CellValue::Empty;
    }
    // Digit gate: parse_number only succeeds when the mantissa contributes at
    // least one ASCII digit to the normalized string it parses ("nan"/"inf"
    // are structurally impossible there and the final f64 parse is
    // is_finite-filtered), and parse_date requires three i32 integer fields —
    // so digit-free text can only be Bool/Text, and skipping both parsers
    // cannot change the outcome. Digit-bearing text runs both parsers in the
    // original order; the Bool/Text tail is shared by both arms.
    if text.bytes().any(|b| b.is_ascii_digit()) {
        if let Some(n) = parse_number(text, regional) {
            return CellValue::Number(n);
        }
        if let Some(d) = parse_date(text, regional) {
            return CellValue::Date(d);
        }
    }
    if text.eq_ignore_ascii_case("true") {
        CellValue::Bool(true)
    } else if text.eq_ignore_ascii_case("false") {
        CellValue::Bool(false)
    } else {
        CellValue::Text(text.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pre-gate evaluation chain over the ORIGINAL (pre-refactor) parsers,
    /// kept as the equivalence oracle for both the digit gate and the
    /// allocation-free rewrite.
    fn ungated_interpret(text: &str, regional: &RegionalSettings) -> CellValue {
        if text.is_empty() {
            CellValue::Empty
        } else if let Some(n) = orig_parse_number(text, regional) {
            CellValue::Number(n)
        } else if let Some(d) = orig_parse_date(text, regional) {
            CellValue::Date(d)
        } else if text.eq_ignore_ascii_case("true") {
            CellValue::Bool(true)
        } else if text.eq_ignore_ascii_case("false") {
            CellValue::Bool(false)
        } else {
            CellValue::Text(text.into())
        }
    }

    /// Verbatim pre-refactor parse_number (allocating validation order).
    fn orig_parse_number(text: &str, regional: &RegionalSettings) -> Option<f64> {
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
        if decimal.next().is_some()
            || fraction.is_some_and(|f| !f.bytes().all(|b| b.is_ascii_digit()))
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

    /// Verbatim pre-refactor parse_date (Vec-collecting version).
    fn orig_parse_date(text: &str, regional: &RegionalSettings) -> Option<i64> {
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

    /// Exhaustive short strings over a structural alphabet: the rewritten
    /// parsers must accept/reject/valuate identically to the originals, and
    /// interpret must match the original chain, under both regional styles.
    #[test]
    fn rewritten_parsers_match_originals_exhaustively() {
        let european = RegionalSettings {
            use_system: false,
            decimal: ',',
            thousands: Some('.'),
            date_order: DateOrder::Dmy,
            date_separator: '/',
        };
        let alphabet = ['0', '1', '9', '-', ',', '.', ':', 'e', ' ', 'T', 'a', 't'];
        let regionals = [RegionalSettings::default(), european];
        let mut text = String::new();
        for len in 0..=4 {
            let mut idx = vec![0usize; len];
            loop {
                text.clear();
                text.extend(idx.iter().map(|&i| alphabet[i]));
                for regional in &regionals {
                    assert_eq!(
                        parse_number(&text, regional),
                        orig_parse_number(&text, regional),
                        "number {text:?}"
                    );
                    assert_eq!(
                        parse_date(&text, regional),
                        orig_parse_date(&text, regional),
                        "date {text:?}"
                    );
                    assert_eq!(
                        interpret(&text, regional),
                        ungated_interpret(&text, regional),
                        "interpret {text:?}"
                    );
                }
                // odometer over the alphabet
                let mut pos = len;
                let mut advanced = false;
                while pos > 0 {
                    pos -= 1;
                    idx[pos] += 1;
                    if idx[pos] < alphabet.len() {
                        advanced = true;
                        break;
                    }
                    idx[pos] = 0;
                }
                if !advanced {
                    break;
                }
            }
        }
    }

    #[test]
    fn digit_gate_matches_ungated_interpretation() {
        let european = RegionalSettings {
            use_system: false,
            decimal: ',',
            thousands: Some('.'),
            date_order: DateOrder::Dmy,
            date_separator: '/',
        };
        let cases = [
            // Digit-free fast-reject classes: plain text, separator-bearing
            // text, bool spellings, empty.
            "", "true", "false", "TRUE", "False", "abc", "hello world", "N/A",
            "a,b,c", "ab-cd", "a/b/c", "--", "..", "+", "-", "e", "E", "inf",
            "nan", "Infinity", "  padded  ", "一列文本", "ünïcode", ".", ",",
            "true false", "null", "none", "yes",
            // Digit-bearing: number/date paths must be unchanged.
            "0", "123", "-45", "+6", "1.5", ".5", "5.", "1,000", "1.000",
            "1e5", "1E-3", "2e", "e5", "2024-01-15", "2024-13-01",
            "2024-01-15 10:30", "2024-01-15T10:30:45", "99/02/03", "1:2:3",
            "0.25", "0.250", "true1", "1true", "abc123", "3.14.15",
            "1 000", " 42 ", "1,2,3",
            // Allocation-elimination edge cases: digit-bearing separator-free
            // text, separator-bearing non-dates, thousands/exponent/sign
            // boundaries, structural rejects of every shape.
            "item0", "note 9", "1-2", "a-b-c", "1-2-3-4", "-01-15",
            "2024-01-15 ", "2024-01-15 25:00", "2024-01-15 1:2:3:4",
            "1,00", "1,0000", "12,34,56", ",000", "1,", "1..5", "..",
            "1e", "1e+", "1e-", "1e5e6", "--5", "++5", "+-5", "0.0.0",
            "1,234,567", "1.234.567", "007", "0x10", "1_000", "٣",
        ];
        for regional in [RegionalSettings::default(), european] {
            for text in cases {
                assert_eq!(
                    interpret(text, &regional),
                    ungated_interpret(text, &regional),
                    "input {text:?} under {regional:?}"
                );
            }
        }
        // Spot-check the fast-rejected classes land exactly where the ungated
        // chain did (not merely "same as oracle" tautology).
        let regional = RegionalSettings::default();
        assert_eq!(interpret("", &regional), CellValue::Empty);
        assert_eq!(interpret("true", &regional), CellValue::Bool(true));
        assert_eq!(interpret("FALSE", &regional), CellValue::Bool(false));
        assert_eq!(
            interpret("abc", &regional),
            CellValue::Text("abc".into())
        );
        assert_eq!(
            interpret("a,b,c", &regional),
            CellValue::Text("a,b,c".into())
        );
        assert_eq!(interpret("123", &regional), CellValue::Number(123.0));
        assert_eq!(
            interpret("2024-01-15", &regional),
            ungated_interpret("2024-01-15", &regional)
        );
        assert!(matches!(
            interpret("2024-01-15", &regional),
            CellValue::Date(_)
        ));
    }

    /// Hand-computed values pinning the allocation-free rewrite of
    /// parse_number/parse_date (date_ms is unchanged and serves as oracle).
    #[test]
    fn allocation_free_parsers_keep_values() {
        let regional = RegionalSettings::default(); // '.' decimal, ',' thousands, '-' dates, Ymd
        assert_eq!(parse_number("1,000", &regional), Some(1000.0));
        assert_eq!(parse_number("1,234,567", &regional), Some(1234567.0));
        assert_eq!(parse_number("1.5", &regional), Some(1.5));
        assert_eq!(parse_number(".5", &regional), Some(0.5));
        assert_eq!(parse_number("5.", &regional), Some(5.0));
        assert_eq!(parse_number("-45", &regional), Some(-45.0));
        assert_eq!(parse_number("+6", &regional), Some(6.0));
        assert_eq!(parse_number("1e5", &regional), Some(100000.0));
        assert_eq!(parse_number("1E-3", &regional), Some(0.001));
        assert_eq!(parse_number("-1,234.5e2", &regional), Some(-123450.0));
        // Structural rejects (unchanged).
        for bad in [
            "item0", "1,00", "1,0000", "12,34,56", ",000", "1,", "1..5",
            "1e", "1e+", "1e5e6", "--5", "+-5", "0.0.0", "0x10", "1_000",
        ] {
            assert_eq!(parse_number(bad, &regional), None, "{bad}");
        }
        let european = RegionalSettings {
            use_system: false,
            decimal: ',',
            thousands: Some('.'),
            date_order: DateOrder::Dmy,
            date_separator: '/',
        };
        assert_eq!(parse_number("1.000", &european), Some(1000.0));
        assert_eq!(parse_number("1,5", &european), Some(1.5));
        assert_eq!(parse_number("1.00", &european), None);
        // Dates: exactly three separated fields, optional time.
        let day = date_ms(2024, 1, 15);
        assert_eq!(parse_date("2024-01-15", &regional), day);
        assert_eq!(
            parse_date("2024-01-15 10:30", &regional),
            day.map(|d| d + 10 * 3_600_000 + 30 * 60_000)
        );
        assert_eq!(
            parse_date("2024-01-15T10:30:45", &regional),
            day.map(|d| d + 10 * 3_600_000 + 30 * 60_000 + 45_000)
        );
        assert_eq!(parse_date("15/01/2024", &european), day);
        for bad in [
            "item0", "1-2", "a-b-c", "1-2-3-4", "-01-15", "2024-13-01",
            "2024-01-15 25:00", "2024-01-15 1:2:3:4", "2024-01-15 1",
        ] {
            assert_eq!(parse_date(bad, &regional), None, "{bad}");
        }
        // trim() runs before splitting: padded dates stay valid.
        assert_eq!(parse_date("2024-01-15 ", &regional), day);
        // interpret composes the two identically.
        assert_eq!(interpret("1,000", &regional), CellValue::Number(1000.0));
        assert_eq!(
            interpret("2024-01-15 10:30", &regional),
            CellValue::Date(day.unwrap() + 10 * 3_600_000 + 30 * 60_000)
        );
        assert_eq!(
            interpret("item0", &regional),
            CellValue::Text("item0".into())
        );
    }
}
