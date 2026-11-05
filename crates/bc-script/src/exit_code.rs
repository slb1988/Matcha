//! Process exit codes – 1:1 with the command-line reference table (`wBcExitCode`).
//! CI integrations depend on these exact values.

/// Exit codes returned by the console launcher and `/qc` quick compare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum ExitCode {
    /// 0 – Success (same).
    Success = 0,
    /// 1 – Binary same.
    BinarySame = 1,
    /// 2 – Rules-based same.
    RulesBasedSame = 2,
    /// 11 – Binary differences.
    BinaryDifferent = 11,
    /// 12 – Similar.
    Similar = 12,
    /// 13 – Rules-based differences.
    RulesBasedDifferent = 13,
    /// 14 – Conflicts detected.
    ConflictsDetected = 14,
    /// 100 – Unknown error.
    UnknownError = 100,
    /// 101 – Conflicts detected, merge output not written.
    ConflictsMergeNotWritten = 101,
    /// 102 – Launcher unable to wait for the main program to finish.
    LauncherCannotWait = 102,
    /// 103 – Launcher cannot find the main program.
    LauncherCannotFindMain = 103,
    /// 104 – Trial period expired. **Never produced by this project** (no licensing);
    /// kept so the enumeration matches the reference table exactly.
    TrialExpired = 104,
    /// 105 – Error loading script.
    ScriptLoadError = 105,
    /// 106 – Script syntax error.
    ScriptSyntaxError = 106,
    /// 107 – Script failed to load file or folder.
    ScriptLoadFileOrFolderFailed = 107,
}

impl ExitCode {
    /// Every code in table order (15 rows: 0, 1, 2, 11, 12, 13, 14, 100..=107).
    pub const ALL: [ExitCode; 15] = [
        ExitCode::Success,
        ExitCode::BinarySame,
        ExitCode::RulesBasedSame,
        ExitCode::BinaryDifferent,
        ExitCode::Similar,
        ExitCode::RulesBasedDifferent,
        ExitCode::ConflictsDetected,
        ExitCode::UnknownError,
        ExitCode::ConflictsMergeNotWritten,
        ExitCode::LauncherCannotWait,
        ExitCode::LauncherCannotFindMain,
        ExitCode::TrialExpired,
        ExitCode::ScriptLoadError,
        ExitCode::ScriptSyntaxError,
        ExitCode::ScriptLoadFileOrFolderFailed,
    ];

    /// Numeric process exit status.
    pub fn code(self) -> i32 {
        self as i32
    }

    /// Reverse lookup.
    pub fn from_code(code: i32) -> Option<ExitCode> {
        Some(match code {
            0 => ExitCode::Success,
            1 => ExitCode::BinarySame,
            2 => ExitCode::RulesBasedSame,
            11 => ExitCode::BinaryDifferent,
            12 => ExitCode::Similar,
            13 => ExitCode::RulesBasedDifferent,
            14 => ExitCode::ConflictsDetected,
            100 => ExitCode::UnknownError,
            101 => ExitCode::ConflictsMergeNotWritten,
            102 => ExitCode::LauncherCannotWait,
            103 => ExitCode::LauncherCannotFindMain,
            104 => ExitCode::TrialExpired,
            105 => ExitCode::ScriptLoadError,
            106 => ExitCode::ScriptSyntaxError,
            107 => ExitCode::ScriptLoadFileOrFolderFailed,
            _ => return None,
        })
    }

    /// Description from the reference table.
    pub fn description(self) -> &'static str {
        match self {
            ExitCode::Success => "Success",
            ExitCode::BinarySame => "Binary same",
            ExitCode::RulesBasedSame => "Rules-based same",
            ExitCode::BinaryDifferent => "Binary differences",
            ExitCode::Similar => "Similar",
            ExitCode::RulesBasedDifferent => "Rules-based differences",
            ExitCode::ConflictsDetected => "Conflicts detected",
            ExitCode::UnknownError => "Unknown error",
            ExitCode::ConflictsMergeNotWritten => "Conflicts detected, merge output not written",
            ExitCode::LauncherCannotWait => "Launcher unable to wait for the main program to finish",
            ExitCode::LauncherCannotFindMain => "Launcher cannot find the main program",
            ExitCode::TrialExpired => "Trial period expired (not applicable to this project)",
            ExitCode::ScriptLoadError => "Error loading script",
            ExitCode::ScriptSyntaxError => "Script syntax error",
            ExitCode::ScriptLoadFileOrFolderFailed => "Script failed to load file or folder",
        }
    }

    /// `true` for the "same" family (0, 1, 2).
    pub fn is_same(self) -> bool {
        matches!(self, ExitCode::Success | ExitCode::BinarySame | ExitCode::RulesBasedSame)
    }
}

impl From<ExitCode> for std::process::ExitCode {
    fn from(code: ExitCode) -> Self {
        std::process::ExitCode::from(code.code() as u8)
    }
}
