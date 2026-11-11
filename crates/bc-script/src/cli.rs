//! Command-line parsing – command_line_reference.html (`SsCmdLine`).
//!
//! Switches use `/` on Windows and `-` on macOS/Linux; both are accepted everywhere.
//! Positional arguments are inferred: named session / named workspace / two folders /
//! two files / three files (merge) / four files (merge + output) / `@script` / `.bcpkg`
//! / `.diff|.patch` / `-` (stdin).

/// `/qc=<type>` values (re-exported so callers need not depend on the folder engine).
pub use bc_engine_dir::criteria::ContentCompare;
use bc_session::SessionKind;
use std::path::{Path, PathBuf};

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
    /// `/report=<layout>` – write a text report of `<left> <right>` without a window
    /// (Matcha extension; the value as given, validated like a script `text-report`).
    Report(String),
    /// `/reportoptions=<options>` – the `options:` list of `/report`.
    ReportOptions(String),
    /// `/reportfile=<path>` – write `/report` to a file instead of standard output.
    ReportFile(PathBuf),
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
    /// `/selectleft` `/compareto` `/comparepair` – Explorer context-menu verbs
    /// (Matcha extension); each takes exactly one path.
    Shell(ShellVerb),
    /// `/shellmenu=on|off` – add or remove the Explorer context menu, then exit
    /// (Matcha extension).
    ShellMenu(bool),
}

/// Explorer context-menu verb carried on the command line (see `bc-shell`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellVerb {
    /// `/selectleft <path>` – remember `<path>` as the pending Left.
    SelectLeft,
    /// `/compareto <path>` – compare the pending Left with `<path>`.
    CompareTo,
    /// `/comparepair <path>` – one item of a multi-selection "Compare".
    ComparePair,
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
    Merge3 {
        left: PathBuf,
        right: PathBuf,
        center: PathBuf,
    },
    /// Four files → Text Merge with output.
    Merge4 {
        left: PathBuf,
        right: PathBuf,
        center: PathBuf,
        output: PathBuf,
    },
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
    /// Parse `std::env::args().skip(1)`; relative positionals resolve against the
    /// process working directory.
    pub fn parse<I, S>(args: I) -> Result<CliArgs, CliError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let cwd = std::env::current_dir().unwrap_or_default();
        Self::parse_in(args, &cwd)
    }

    /// 同 [`parse`](Self::parse)，但相对路径按 `cwd` 解析——单实例转交时以调用方进程的
    /// 工作目录为准，而不是常驻实例的。
    pub fn parse_in<I, S>(args: I, cwd: &Path) -> Result<CliArgs, CliError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut switches = Vec::new();
        let mut positional: Vec<String> = Vec::new();
        let mut args = args.into_iter().map(|a| a.as_ref().to_string());
        while let Some(arg) = args.next() {
            // `@script` 之后的参数全部属于脚本。
            if positional.is_empty() && arg.len() > 1 && arg.starts_with('@') {
                let path = resolve(cwd, &arg[1..]);
                return Ok(CliArgs {
                    switches,
                    invocation: Invocation::Script {
                        path,
                        args: args.collect(),
                    },
                });
            }
            match parse_switch(&arg)? {
                Some(switch) => switches.push(switch),
                None => positional.push(arg),
            }
        }
        let report_extras = switches
            .iter()
            .any(|s| matches!(s, Switch::ReportOptions(_) | Switch::ReportFile(_)));
        if report_extras && !switches.iter().any(|s| matches!(s, Switch::Report(_))) {
            return Err(error("/reportoptions and /reportfile require /report"));
        }
        let shell = switches
            .iter()
            .filter(|s| matches!(s, Switch::Shell(_) | Switch::ShellMenu(_)))
            .count();
        let invocation = if shell > 1 {
            return Err(error("only one Explorer menu switch may be given"));
        } else if switches.iter().any(|s| matches!(s, Switch::Shell(_))) {
            // 菜单命令总是恰好一个路径；不做会话名推断（文件可能已不存在）。
            match positional.as_slice() {
                [path] => Invocation::Single(resolve(cwd, path)),
                _ => {
                    return Err(error(
                        "/selectleft, /compareto and /comparepair take exactly one path",
                    ))
                }
            }
        } else if switches.iter().any(|s| matches!(s, Switch::ShellMenu(_))) {
            if !positional.is_empty() {
                return Err(error("/shellmenu does not take paths"));
            }
            Invocation::Home
        } else {
            infer(cwd, positional)?
        };
        Ok(CliArgs {
            switches,
            invocation,
        })
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

    /// Explorer menu verb and its path (`/selectleft` & co.).
    pub fn shell_verb(&self) -> Option<(ShellVerb, &Path)> {
        let verb = self.switches.iter().find_map(|s| match s {
            Switch::Shell(verb) => Some(*verb),
            _ => None,
        })?;
        match &self.invocation {
            Invocation::Single(path) => Some((verb, path.as_path())),
            _ => None,
        }
    }

    /// `/report=…` with its `/reportoptions=` and `/reportfile=`.
    pub fn report(&self) -> Option<ReportSwitches> {
        let layout = self.switches.iter().find_map(|s| match s {
            Switch::Report(layout) => Some(layout.clone()),
            _ => None,
        })?;
        Some(ReportSwitches {
            layout,
            options: self.switches.iter().find_map(|s| match s {
                Switch::ReportOptions(options) => Some(options.clone()),
                _ => None,
            }),
            file: self.switches.iter().find_map(|s| match s {
                Switch::ReportFile(file) => Some(file.clone()),
                _ => None,
            }),
        })
    }

    /// `/shellmenu=on|off`.
    pub fn shell_menu(&self) -> Option<bool> {
        self.switches.iter().find_map(|s| match s {
            Switch::ShellMenu(on) => Some(*on),
            _ => None,
        })
    }
}

/// The `/report` family of switches (Matcha extension).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportSwitches {
    /// `/report=` value: `patch`, `patch-normal`, `patch-context`, `patch-unified`, or
    /// another layout name (rejected as not supported when the report is built).
    pub layout: String,
    /// `/reportoptions=` – same values as a script's `options:`.
    pub options: Option<String>,
    /// `/reportfile=` – `None` writes to standard output.
    pub file: Option<PathBuf>,
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
    const LINES: &[&str] = &[
        "Usage: matcha [<switches>] [<left> [<right> [<center> [<output>]]]]",
        "       matcha [<switches>] @<script> [<script args>]",
        "",
        "Positional arguments:",
        "  (none)                  Home view",
        "  <folder> <folder>       Folder Compare",
        "  <file> <file>           Compare (view chosen by file format, or /fv)",
        "  <l> <r> <center> [<o>]  Text Merge (with output file)",
        "  <name>                  Saved session or workspace",
        "  <file.bcpkg>            Import settings package",
        "  <file.diff|.patch>      Text Patch",
        "  -                       Read stdin as a file",
        "",
        "Switches (/ or - prefix):",
        "  /? /h /help             Show this help",
        "  /automerge              Automatically merge (with /mergeoutput)",
        "  /center=<file>          Center file for merges",
        "  /closescript            Close the script window when finished",
        "  /edit                   Open in edit mode",
        "  /expandall              Expand all folders",
        "  /favorleft /favorright  Favor one side when merging",
        "  /filters=<masks>        File filters for folder sessions",
        "  /force                  Force the merge output to be written",
        "  /fv=<type>              Force the view type (e.g. \"Text Compare\")",
        "  /iu                     Ignore unimportant differences",
        "  /mergeoutput=<path>     Merge output file",
        "  /nobackups              Do not create backup files",
        "  /qc[=size|crc|binary]   Quick compare (rules-based without a value)",
        "  /reviewconflicts        Review merge conflicts",
        "  /ro /ro1 /ro2           Read-only (all / left / right)",
        "  /savetarget=<file>      File to save to",
        "  /silent                 Suppress all interaction",
        "  /solo                   Force a new instance",
        "  /sync                   Open in Folder Sync",
        "  /title1..4=<title>      Pane titles (left, right, center, output)",
        "  /vcs1..4=<path>         Version-control paths",
        "",
        "Reports without a window (Matcha):",
        "  /report=<layout>        Text report of <left> <right>: patch, patch-normal,",
        "                          patch-context or patch-unified",
        "  /reportoptions=<list>   Report options, e.g. ignore-unimportant",
        "  /reportfile=<path>      Write the report to a file (default: standard output)",
        "",
        "Explorer context menu (Matcha):",
        "  /selectleft <path>      Remember <path> as the Left side",
        "  /compareto <path>       Compare the remembered Left with <path>",
        "  /comparepair <path>     One item of a multi-selection Compare",
        "  /shellmenu=on|off       Add or remove the Explorer menu, then exit",
    ];
    let mut text = LINES.join("\n");
    text.push('\n');
    text
}

fn error(message: impl Into<String>) -> CliError {
    CliError {
        message: message.into(),
    }
}

fn resolve(cwd: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

/// 识别开关；`None` 表示位置参数。`-` 前缀在任何平台都是开关；`/` 前缀在 Windows 上
/// 是开关，在 unix 上只有已知开关名才算（否则 `/home/...` 会被误判）。
fn parse_switch(arg: &str) -> Result<Option<Switch>, CliError> {
    let Some(body) = arg
        .strip_prefix("--")
        .or_else(|| arg.strip_prefix('-'))
        .or_else(|| arg.strip_prefix('/'))
    else {
        return Ok(None);
    };
    if body.is_empty() {
        // 单独的 `-` 表示 stdin。
        return Ok(None);
    }
    let (name, value) = match body.split_once('=') {
        Some((n, v)) => (n, Some(v)),
        None => (body, None),
    };
    let lower = name.to_ascii_lowercase();
    let flag = |s: Switch| match value {
        None => Ok(Some(s)),
        Some(_) => Err(error(format!("switch {name} does not take a value"))),
    };
    let need = || match value {
        Some(v) if !v.is_empty() => Ok(v.to_string()),
        _ => Err(error(format!("switch {name} requires a value"))),
    };
    // `/title1..4` 与 `lefttitle` 等别名 → 0 起的窗格序号。
    let indexed = |prefix: &str, aliases: [&str; 4]| -> Option<u8> {
        if let Some(Ok(n @ 1..=4)) = lower.strip_prefix(prefix).map(str::parse::<u8>) {
            return Some(n - 1);
        }
        aliases.iter().position(|a| *a == lower).map(|i| i as u8)
    };
    match lower.as_str() {
        "?" | "h" | "help" => flag(Switch::Help),
        "automerge" => flag(Switch::AutoMerge),
        "center" => Ok(Some(Switch::Center(PathBuf::from(need()?)))),
        "closescript" => flag(Switch::CloseScript),
        "edit" => flag(Switch::Edit),
        "expandall" => flag(Switch::ExpandAll),
        "favorleft" => flag(Switch::FavorLeft),
        "favorright" => flag(Switch::FavorRight),
        "filters" => Ok(Some(Switch::Filters(need()?))),
        "force" => flag(Switch::Force),
        "fv" => {
            let kind = need()?;
            SessionKind::from_force_view_name(&kind)
                .map(|k| Some(Switch::ForceView(k)))
                .ok_or_else(|| error(format!("unknown view type for /fv: {kind}")))
        }
        "iu" => flag(Switch::IgnoreUnimportant),
        "mergeoutput" | "mo" => Ok(Some(Switch::MergeOutput(PathBuf::from(need()?)))),
        "nobackups" => flag(Switch::NoBackups),
        "qc" => {
            let kind = match value.map(str::to_ascii_lowercase).as_deref() {
                None | Some("") | Some("rules") | Some("rules-based") => None,
                Some("size") => Some(ContentCompare::Size),
                Some("crc") => Some(ContentCompare::Crc),
                Some("binary") => Some(ContentCompare::Binary),
                Some(other) => return Err(error(format!("unknown /qc type: {other}"))),
            };
            Ok(Some(Switch::QuickCompare(kind)))
        }
        "report" => Ok(Some(Switch::Report(need()?))),
        "reportoptions" => Ok(Some(Switch::ReportOptions(need()?))),
        "reportfile" => Ok(Some(Switch::ReportFile(PathBuf::from(need()?)))),
        "reviewconflicts" => flag(Switch::ReviewConflicts),
        "ro" | "readonly" => flag(Switch::ReadOnly),
        "ro1" | "lro" => flag(Switch::ReadOnlyLeft),
        "ro2" | "rro" => flag(Switch::ReadOnlyRight),
        "savetarget" => Ok(Some(Switch::SaveTarget(PathBuf::from(need()?)))),
        "silent" => flag(Switch::Silent),
        "solo" => flag(Switch::Solo),
        "sync" => flag(Switch::Sync),
        "selectleft" => flag(Switch::Shell(ShellVerb::SelectLeft)),
        "compareto" => flag(Switch::Shell(ShellVerb::CompareTo)),
        "comparepair" => flag(Switch::Shell(ShellVerb::ComparePair)),
        "shellmenu" => match value.map(str::to_ascii_lowercase).as_deref() {
            Some("on") => Ok(Some(Switch::ShellMenu(true))),
            Some("off") => Ok(Some(Switch::ShellMenu(false))),
            _ => Err(error("/shellmenu expects =on or =off")),
        },
        _ => {
            if let Some(i) = indexed(
                "title",
                ["lefttitle", "righttitle", "centertitle", "outputtitle"],
            ) {
                Ok(Some(Switch::Title(
                    i,
                    value.unwrap_or_default().to_string(),
                )))
            } else if let Some(i) =
                indexed("vcs", ["leftvcs", "rightvcs", "centervcs", "outputvcs"])
            {
                Ok(Some(Switch::Vcs(i, need()?)))
            } else if arg.starts_with('/') && !cfg!(windows) {
                // unix 绝对路径。
                Ok(None)
            } else {
                Err(error(format!("unknown switch: {arg}")))
            }
        }
    }
}

/// 位置参数推断：按个数，再按 `-`、扩展名与目录判断。
fn infer(cwd: &Path, positional: Vec<String>) -> Result<Invocation, CliError> {
    let ext = |p: &Path| {
        p.extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default()
    };
    let mut paths = positional.iter().map(|p| resolve(cwd, p));
    let mut next = || paths.next().unwrap_or_default();
    Ok(match positional.len() {
        0 => Invocation::Home,
        1 => {
            let raw = &positional[0];
            let path = next();
            if raw == "-" {
                Invocation::Stdin
            } else if ext(&path) == "bcpkg" {
                Invocation::Package(path)
            } else if matches!(ext(&path).as_str(), "diff" | "patch") {
                Invocation::Patch(path)
            } else if path.exists() || raw.contains(['/', '\\', ':']) {
                Invocation::Single(path)
            } else {
                // 既不存在也不像路径：按已保存会话名处理（会话/工作区由打开方查找）。
                Invocation::Session(raw.clone())
            }
        }
        2 => {
            let (left, right) = (next(), next());
            if left.is_dir() && right.is_dir() {
                Invocation::Folders(left, right)
            } else {
                Invocation::Files(left, right)
            }
        }
        3 => Invocation::Merge3 {
            left: next(),
            right: next(),
            center: next(),
        },
        4 => Invocation::Merge4 {
            left: next(),
            right: next(),
            center: next(),
            output: next(),
        },
        n => {
            return Err(error(format!(
                "too many arguments ({n}); at most four paths are accepted"
            )))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cwd() -> PathBuf {
        std::env::temp_dir().join("matcha-cli-cwd")
    }
    fn parse(args: &[&str]) -> CliArgs {
        CliArgs::parse_in(args, &cwd()).unwrap()
    }

    #[test]
    fn no_arguments_open_home() {
        assert_eq!(parse(&[]).invocation, Invocation::Home);
    }

    #[test]
    fn two_folders_infer_folder_compare_otherwise_files() {
        let dir = std::env::temp_dir();
        let d = dir.to_string_lossy().to_string();
        assert_eq!(
            parse(&[&d, &d]).invocation,
            Invocation::Folders(dir.clone(), dir.clone())
        );
        assert_eq!(
            parse(&["a.txt", "sub/b.txt"]).invocation,
            Invocation::Files(cwd().join("a.txt"), cwd().join("sub/b.txt"))
        );
    }

    #[test]
    fn switches_accept_both_prefixes_and_any_case() {
        let cli = parse(&[
            "/SOLO",
            "-ro2",
            "/title1=Base",
            "/qc=crc",
            "/fv=Text Compare",
            "x/a",
            "x/b",
        ]);
        assert!(cli.solo());
        assert_eq!(
            cli.switches,
            vec![
                Switch::Solo,
                Switch::ReadOnlyRight,
                Switch::Title(0, "Base".into()),
                Switch::QuickCompare(Some(ContentCompare::Crc)),
                Switch::ForceView(SessionKind::TextCompare),
            ]
        );
        assert!(matches!(cli.invocation, Invocation::Files(..)));
        assert_eq!(parse(&["/qc"]).switches, vec![Switch::QuickCompare(None)]);
        assert_eq!(
            parse(&["/righttitle=R"]).switches,
            vec![Switch::Title(1, "R".into())]
        );
        assert_eq!(parse(&["/?"]).switches, vec![Switch::Help]);
    }

    #[test]
    fn script_takes_the_remaining_arguments() {
        let cli = parse(&["/silent", "@run.txt", "a", "/b"]);
        assert!(cli.silent());
        assert_eq!(
            cli.invocation,
            Invocation::Script {
                path: cwd().join("run.txt"),
                args: vec!["a".into(), "/b".into()]
            }
        );
    }

    #[test]
    fn positional_kinds() {
        assert_eq!(parse(&["-"]).invocation, Invocation::Stdin);
        assert!(matches!(
            parse(&["x.bcpkg"]).invocation,
            Invocation::Package(_)
        ));
        assert!(matches!(
            parse(&["fix.PATCH"]).invocation,
            Invocation::Patch(_)
        ));
        assert_eq!(
            parse(&["MySession"]).invocation,
            Invocation::Session("MySession".into())
        );
        assert!(matches!(
            parse(&["dir/missing.txt"]).invocation,
            Invocation::Single(_)
        ));
        assert!(matches!(
            parse(&["a", "b", "c"]).invocation,
            Invocation::Merge3 { .. }
        ));
        assert!(matches!(
            parse(&["a", "b", "c", "d"]).invocation,
            Invocation::Merge4 { .. }
        ));
    }

    #[test]
    fn explorer_menu_switches_take_one_path() {
        let cli = parse(&["/selectleft", "C:/x/My Session"]);
        assert_eq!(cli.switches, vec![Switch::Shell(ShellVerb::SelectLeft)]);
        // 不存在、不像路径的单个参数也不按会话名处理。
        let cli = parse(&["-compareto", "missing"]);
        assert_eq!(
            cli.shell_verb(),
            Some((ShellVerb::CompareTo, cwd().join("missing").as_path()))
        );
        assert_eq!(
            parse(&["/COMPAREPAIR", "a"]).shell_verb().map(|(v, _)| v),
            Some(ShellVerb::ComparePair)
        );
        assert_eq!(parse(&["/shellmenu=OFF"]).shell_menu(), Some(false));
        assert_eq!(parse(&["/shellmenu=on"]).shell_menu(), Some(true));
        assert_eq!(parse(&["a", "b"]).shell_verb(), None);
        let bad = |args: &[&str]| CliArgs::parse_in(args, &cwd()).is_err();
        assert!(bad(&["/selectleft"]));
        assert!(bad(&["/selectleft", "a", "b"]));
        assert!(bad(&["/selectleft", "/compareto", "a"]));
        assert!(bad(&["/selectleft=a"]));
        assert!(bad(&["/shellmenu"]));
        assert!(bad(&["/shellmenu=maybe"]));
        assert!(bad(&["/shellmenu=on", "a"]));
    }

    #[test]
    fn rejects_bad_switches_and_too_many_paths() {
        let bad = |args: &[&str]| CliArgs::parse_in(args, &cwd()).is_err();
        assert!(bad(&["-bogus"]));
        assert!(bad(&["/solo=1"]));
        assert!(bad(&["/fv=Nope"]));
        assert!(bad(&["/qc=fast"]));
        assert!(bad(&["/mergeoutput"]));
        assert!(bad(&["a", "b", "c", "d", "e"]));
        assert!(usage().contains("/solo"));
    }

    #[test]
    fn report_switches() {
        let cli = parse(&[
            "/report=patch-unified",
            "/reportoptions=ignore-unimportant",
            "/reportfile=out.patch",
            "a.txt",
            "b.txt",
        ]);
        assert_eq!(
            cli.report(),
            Some(ReportSwitches {
                layout: "patch-unified".into(),
                options: Some("ignore-unimportant".into()),
                file: Some("out.patch".into()),
            })
        );
        assert_eq!(
            cli.invocation,
            Invocation::Files(cwd().join("a.txt"), cwd().join("b.txt"))
        );
        let plain = parse(&["-report=patch", "a", "b"]).report().unwrap();
        assert_eq!((plain.options, plain.file), (None, None));
        assert_eq!(parse(&["a", "b"]).report(), None);
        let bad = |args: &[&str]| CliArgs::parse_in(args, &cwd()).is_err();
        assert!(bad(&["/report", "a", "b"]), "needs a value");
        assert!(bad(&["/reportfile=x", "a", "b"]), "needs /report");
        assert!(
            bad(&["/reportoptions=ignore-unimportant", "a", "b"]),
            "needs /report"
        );
        for text in [
            "/report=<layout>",
            "/reportoptions=<list>",
            "/reportfile=<path>",
        ] {
            assert!(usage().contains(text), "{text}");
        }
    }
}
