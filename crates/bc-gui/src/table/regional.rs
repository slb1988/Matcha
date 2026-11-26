//! Resolve host regional settings once, on a loading worker, in-process and without
//! unsafe or mutating the process/user locale. Other platforms keep the documented fallback.
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
    // 用户区域格式：进程内 GetLocaleInfoEx（bc_script::platform），不启动外部进程。
    let Some(formats) = bc_script::platform::locale_formats() else {
        return RegionalSettings::default();
    };
    let parts = [
        formats.decimal.as_str(),
        formats.thousands.as_str(),
        formats.short_date.as_str(),
        formats.date_separator.as_str(),
    ];
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
