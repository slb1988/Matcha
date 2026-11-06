//! Resolve host regional settings once, on a loading worker, without unsafe or
//! mutating the process/user locale. Other platforms keep the documented fallback.
use bc_formats::model::RegionalSettings;
pub fn host() -> RegionalSettings {
    static SETTINGS: std::sync::OnceLock<RegionalSettings> = std::sync::OnceLock::new();
    SETTINGS.get_or_init(resolve).clone()
}
#[cfg(not(windows))]
fn resolve() -> RegionalSettings {
    RegionalSettings::default()
}
#[cfg(windows)]
fn resolve() -> RegionalSettings {
    use bc_formats::model::DateOrder;
    use std::os::windows::process::CommandExt;
    let output=std::process::Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command",
        "[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); $c=[Globalization.CultureInfo]::CurrentCulture; $c.NumberFormat.NumberDecimalSeparator; $c.NumberFormat.NumberGroupSeparator; $c.DateTimeFormat.ShortDatePattern; $c.DateTimeFormat.DateSeparator"])
        .creation_flags(0x08000000).output();
    let Ok(output) = output else {
        return RegionalSettings::default();
    };
    if !output.status.success() {
        return RegionalSettings::default();
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let parts = text.lines().collect::<Vec<_>>();
    if parts.len() < 4 {
        return RegionalSettings::default();
    }
    let mut order = [
        ('y', parts[2].find('y').unwrap_or(usize::MAX)),
        ('m', parts[2].find('M').unwrap_or(usize::MAX)),
        ('d', parts[2].find('d').unwrap_or(usize::MAX)),
    ];
    order.sort_by_key(|(_, p)| *p);
    RegionalSettings {
        use_system: true,
        decimal: parts[0].chars().next().unwrap_or('.'),
        thousands: parts[1].chars().next(),
        date_order: match order[0].0 {
            'm' => DateOrder::Mdy,
            'd' => DateOrder::Dmy,
            _ => DateOrder::Ymd,
        },
        date_separator: parts[3].chars().next().unwrap_or('-'),
    }
}
