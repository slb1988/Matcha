//! Command-line parsing – command_line_reference.html (`SsCmdLine`).
//!
//! Switches use `/` on Windows and `-` on macOS/Linux; both are accepted everywhere.
//! Positional arguments are inferred: named session / named workspace / two folders /
//! two files / three files (merge) / four files (merge + output) / `@script` / `.bcpkg`
//! / `.diff|.patch` / `-` (stdin).

use bc_engine_dir::criteria::ContentCompare;
use bc_session::SessionKind;
use std::path::PathBuf;

/// Every documented switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Switch {
    /// `/?` `/h` `/help`
    Help,
    /// `/automerge`
    AutoMerge,
    /// `/center=<file>`
    Center(PathBuf),
    /// `/closescript`
    CloseScript,
    /// `/edit`
    Edit,
    /// `/expandall`
    ExpandAll,
    /// `/favorleft`
    FavorLeft,
    /// `/favorright`
    FavorRight,
    /// `/filters=<masks>`
    Filters(String),
    /// `/force`
    Force,
    /// `/fv=<type>`
    ForceView(SessionKind),
    /// `/iu`
    IgnoreUnimportant,
    /// `/mergeoutput=<path>`
    MergeOutput(PathBuf),
    /// `/nobackups`
    NoBackups,
    /// `/qc[=size|crc|binary]` – `None` = rules-based.
    QuickCompare(Option<ContentCompare>),
    /// `/reviewconflicts`
    ReviewConflicts,
    /// `/ro` (`/readonly`) – all panes read-only.
    ReadOnly,
    /// `/ro1` (`/lro`)
    ReadOnlyLeft,
    /// `/ro2` (`/rro`)
    ReadOnlyRight,
    /// `/savetarget=<file>`
    SaveTarget(PathBuf),
    /// `/silent`
    Silent,
    /// `/solo`
    Solo,
    /// `/sync`
    Sync,
    /// `/title1..4=<title>` (`lefttitle`, `righttitle`, `centertitle`, `outputtitle`); index 0-based.
    Title(u8, String),
    /// `/vcs1..4=<path>`; index 0-based.
    Vcs(u8, String),
}

/// What the positional arguments denote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    /// No arguments → Home view.
    Home,
    /// Named saved session.
    Session(String),
    /// Named workspace.
    Workspace(String),
    /// Two folders → Folder Compare.
    Folders(PathBuf, PathBuf),
    /// Two files → view chosen by file format (or `/fv`).
    Files(PathBuf, PathBuf),
    /// Three files → Text Merge (left, right, center).
    Merge3 { left: PathBuf, right: PathBuf, center: PathBuf },
    /// Four files → Text Merge with output.
    Merge4 { left: PathBuf, right: PathBuf, center: PathBuf, output: PathBuf },
    /// `@script [args…]`
    Script { path: PathBuf, args: Vec<String> },
    /// `.bcpkg` → import settings.
    Package(PathBuf),
    /// `.diff` / `.patch` → Text Patch view.
    Patch(PathBuf),
    /// `-` → read stdin as a file.
    Stdin,
    /// A single path (file or folder) – opens with an empty other side.
    Single(PathBuf),
}

/// Parsed command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliArgs {
    pub switches: Vec<Switch>,
    pub invocation: Invocation,
}

impl CliArgs {
    /// Parse `std::env::args().skip(1)`.
    pub fn parse<I, S>(args: I) -> Result<CliArgs, CliError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        todo!("bc-script::cli: switch parsing + positional inference")
    }

    pub fn has(&self, pred: impl Fn(&Switch) -> bool) -> bool {
        self.switches.iter().any(pred)
    }

    /// `/silent` – suppress all interaction.
    pub fn silent(&self) -> bool {
        self.has(|s| matches!(s, Switch::Silent))
    }

    /// `/solo` – force a new instance.
    pub fn solo(&self) -> bool {
        self.has(|s| matches!(s, Switch::Solo))
    }
}

/// Command-line error → usage text + exit code 100.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliError {
    pub message: String,
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CliError {}

/// Usage text printed by `/?` on a console (wording aligned with the reference, spec §5).
pub fn usage() -> String {
    todo!("bc-script::cli: usage text")
}
